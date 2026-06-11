#![allow(unused_imports)]
// namespaces_gui.rs — `Gui` namespace: backend nativo de ventana de píxeles.
//
// Backend: winit (ventana + input + IME, cross-platform) + softbuffer (presentar un
// framebuffer CPU u32) + cosmic-text (texto real, Unicode) + image (imágenes) +
// arboard (portapapeles). Sustituye al backend minifb/font8x8.
//
// ── ARQUITECTURA DE DOS HILOS (cross-platform: Windows/macOS/Linux) ──────────────
// winit EXIGE que el EventLoop viva en el HILO PRINCIPAL (en macOS es obligatorio).
// Pero el intérprete de serez-code corre en un hilo aparte de 64 MB (recursión).
// Solución: el hilo MAIN posee el EventLoop+ventana+surface (`GuiMain`); el hilo del
// intérprete dibuja en un canvas LOCAL (`GuiState`) y se comunican por `GUI_HOST`
// (un `Arc<GuiHost>` con `Mutex<SharedInner>` + `Condvar`):
//   - El intérprete dibuja libre en su canvas local (sin locks).
//   - `Gui.present()` copia el canvas → estado compartido, pide un frame (present_seq++)
//     y espera (Condvar) a que el main lo sirva (blit + present) y le devuelva el input.
//   - El hilo MAIN bombea eventos con `pump_app_events` (llena el input compartido) y
//     atiende los present (blit del canvas compartido a la surface). Ver gui_host_main_loop.
//
// El modelo de uso de serez-ui NO cambia: `Gui.open` / `while(isOpen){clear;..;present()}`.
//
// drawText dibuja en rejilla monoespaciada de 8*scale px/char con glifos reales de
// cosmic-text → serez-ui (cursor = 8*scale) no cambia y se ven ñ/acentos/Unicode.

use std::collections::{HashMap, HashSet, VecDeque};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Ime, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::platform::pump_events::EventLoopExtPumpEvents;
use winit::window::{CursorIcon, Window, WindowId};

use softbuffer::{Context, Surface};
use cosmic_text::{Attrs, Buffer as TextBuffer, Color as TextColor, Family, FontSystem, Metrics, Shaping, SwashCache};

use crate::ast::{self};
use crate::region::{ObjectData, OwnedValue};
use super::EvalResult;

// ── Estado compartido entre el hilo intérprete y el hilo main ─────────────────────

/// Snapshot de input que el hilo main produce y el intérprete lee cada frame.
#[derive(Clone, Default)]
struct InputSnapshot {
    keys_down: HashSet<String>,
    shift: bool,
    ctrl: bool,
    alt: bool,
    sup: bool,
    mouse_x: i32,
    mouse_y: i32,
    mouse_l: bool,
    mouse_r: bool,
    mouse_m: bool,
    mouse_pressed: bool,
    keys_pressed: Vec<String>,
    keys_repeated: Vec<String>,
    keys_released: Vec<String>,
    chars_typed: String,
    scroll_x: i64,
    scroll_y: i64,
}

/// Comandos del intérprete → hilo main.
enum GuiCmd {
    Open { title: String, w: u32, h: u32 },
    Close,
    SetTitle(String),
    SetCursor(String),
    SetImePosition(i32, i32),
}

struct SharedInner {
    cmds: VecDeque<GuiCmd>,
    // interp → main
    present_seq: u64,
    canvas: Vec<u32>,
    canvas_w: usize,
    canvas_h: usize,
    interp_done: bool,
    exit_code: i32,
    // main → interp
    done_seq: u64,
    window_ready: bool,
    window_open: bool,
    should_close: bool,
    open_failed: bool,
    win_w: usize,
    win_h: usize,
    input: InputSnapshot,
}

impl SharedInner {
    fn new() -> Self {
        SharedInner {
            cmds: VecDeque::new(),
            present_seq: 0,
            canvas: Vec::new(),
            canvas_w: 0,
            canvas_h: 0,
            interp_done: false,
            exit_code: 0,
            done_seq: 0,
            window_ready: false,
            window_open: false,
            should_close: false,
            open_failed: false,
            win_w: 0,
            win_h: 0,
            input: InputSnapshot::default(),
        }
    }
}

/// Canal compartido global GUI. Lo inicializa `main` antes de lanzar el intérprete.
pub struct GuiHost {
    inner: Mutex<SharedInner>,
    cv: Condvar,
}

impl GuiHost {
    pub fn new() -> Self {
        GuiHost { inner: Mutex::new(SharedInner::new()), cv: Condvar::new() }
    }
    /// Llamado por el hilo intérprete al terminar, para que el hilo main salga.
    pub fn signal_interp_done(&self, code: i32) {
        let mut g = self.inner.lock().unwrap();
        g.interp_done = true;
        g.exit_code = code;
        self.cv.notify_all();
    }
    pub fn exit_code(&self) -> i32 {
        self.inner.lock().unwrap().exit_code
    }
}

pub static GUI_HOST: OnceLock<Arc<GuiHost>> = OnceLock::new();

fn host() -> Option<&'static Arc<GuiHost>> {
    GUI_HOST.get()
}

// ── Recursos del lado intérprete ──────────────────────────────────────────────────

struct ImageData {
    w: usize,
    h: usize,
    px: Vec<u32>,
}

struct Glyph {
    cells: Vec<(i32, i32, u8)>,
    advance: i32,
}

/// Tipografía a nivel intérprete (independiente de la ventana): carga de .ttf/.otf,
/// familia actual y cache de glifos por (familia, char, escala). La familia 0 es la
/// monoespaciada por defecto y conserva la rejilla fija de 8*scale px/char (compat
/// con serez-ui); las familias custom usan el **advance real** del glifo → texto
/// proporcional de verdad.
pub struct GuiFonts {
    font_system: FontSystem,
    swash_cache: SwashCache,
    glyphs: HashMap<(u32, char, i32), Glyph>,
    families: Vec<String>, // [0] = "" (default monospace)
    current: u32,
}

impl GuiFonts {
    fn new() -> Self {
        GuiFonts {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
            glyphs: HashMap::new(),
            families: vec![String::new()],
            current: 0,
        }
    }

    /// ¿Existe la familia (instalada en el sistema o cargada con loadFont)?
    fn has_family(&self, name: &str) -> bool {
        self.font_system
            .db()
            .faces()
            .any(|f| f.families.iter().any(|(n, _)| n == name))
    }

    /// Activa una familia ("" / "default" / "monospace" = la default). false si no existe.
    fn set_family(&mut self, name: &str) -> bool {
        if name.is_empty() || name.eq_ignore_ascii_case("default") || name.eq_ignore_ascii_case("monospace") {
            self.current = 0;
            return true;
        }
        if !self.has_family(name) {
            return false;
        }
        if let Some(idx) = self.families.iter().position(|f| f == name) {
            self.current = idx as u32;
        } else {
            self.families.push(name.to_string());
            self.current = (self.families.len() - 1) as u32;
        }
        true
    }

    /// Carga un .ttf/.otf y devuelve el nombre real de familia, o None si falló.
    fn load_font_file(&mut self, path: &str) -> Option<String> {
        let bytes = std::fs::read(path).ok()?;
        let before = self.font_system.db().faces().count();
        self.font_system.db_mut().load_font_data(bytes);
        let db = self.font_system.db();
        if db.faces().count() == before {
            return None;
        }
        db.faces()
            .nth(before)
            .and_then(|f| f.families.first().map(|(n, _)| n.clone()))
    }

    fn ensure_glyph(&mut self, ch: char, scale: i32) {
        let key = (self.current, ch, scale);
        if self.glyphs.contains_key(&key) {
            return;
        }
        let size = (8 * scale).max(8) as f32;
        let metrics = Metrics::new(size, size * 1.25);
        let mut buf = TextBuffer::new(&mut self.font_system, metrics);
        buf.set_size(&mut self.font_system, Some(size * 4.0), Some(size * 2.0));
        let family_name;
        let attrs = if self.current == 0 {
            Attrs::new().family(Family::Monospace)
        } else {
            family_name = self.families[self.current as usize].clone();
            Attrs::new().family(Family::Name(&family_name))
        };
        buf.set_text(&mut self.font_system, &ch.to_string(), &attrs, Shaping::Advanced, None);
        buf.shape_until_scroll(&mut self.font_system, false);
        // Advance real (suma de anchos de layout); la familia default fija la rejilla.
        let mut adv = 0.0f32;
        for run in buf.layout_runs() {
            for g in run.glyphs.iter() {
                adv += g.w;
            }
        }
        let advance = if self.current == 0 {
            8 * scale
        } else if adv > 0.0 {
            adv.round() as i32
        } else {
            ((size * 0.5).round() as i32).max(1)
        };
        let mut cells: Vec<(i32, i32, u8)> = Vec::new();
        buf.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            TextColor::rgb(255, 255, 255),
            |gx, gy, gw, gh, col| {
                let a = col.a();
                if a == 0 {
                    return;
                }
                let mut yy = 0;
                while yy < gh as i32 {
                    let mut xx = 0;
                    while xx < gw as i32 {
                        cells.push((gx + xx, gy + yy, a));
                        xx += 1;
                    }
                    yy += 1;
                }
            },
        );
        self.glyphs.insert(key, Glyph { cells, advance });
    }

    /// Ancho en px de `text` con la familia actual (proporcional si es custom).
    fn measure(&mut self, text: &str, scale: i32) -> i64 {
        let scale = scale.max(1);
        if self.current == 0 {
            return text.chars().count() as i64 * 8 * scale as i64;
        }
        let mut w: i64 = 0;
        for ch in text.chars() {
            if ch.is_control() {
                continue;
            }
            self.ensure_glyph(ch, scale);
            if let Some(gl) = self.glyphs.get(&(self.current, ch, scale)) {
                w += gl.advance as i64;
            }
        }
        w
    }
}

/// Estado GUI del lado del intérprete: canvas local + snapshot de input.
pub struct GuiState {
    open: bool,
    canvas: Vec<u32>,
    width: usize,
    height: usize,
    win_w: usize,
    win_h: usize,
    bg: u32,
    clip: (i32, i32, i32, i32),
    clip_stack: Vec<(i32, i32, i32, i32)>,
    images: HashMap<i64, ImageData>,
    next_image: i64,
    clipboard: Option<arboard::Clipboard>,
    input: InputSnapshot,
    open_time: std::time::Instant,   // para Gui.time()
}

impl GuiState {
    fn new(w: usize, h: usize) -> Self {
        GuiState {
            open: true,
            canvas: vec![0u32; w.max(1) * h.max(1)],
            width: w.max(1),
            height: h.max(1),
            win_w: w.max(1),
            win_h: h.max(1),
            bg: 0,
            clip: (0, 0, w.max(1) as i32, h.max(1) as i32),
            clip_stack: Vec::new(),
            images: HashMap::new(),
            next_image: 1,
            clipboard: None,
            input: InputSnapshot::default(),
            open_time: std::time::Instant::now(),
        }
    }

    /// Envía el canvas al hilo main, pide un frame y espera el handshake; refresca
    /// el snapshot de input y el tamaño de ventana.
    fn present(&mut self, host: &GuiHost) {
        let mut g = host.inner.lock().unwrap();
        g.canvas.clear();
        g.canvas.extend_from_slice(&self.canvas);
        g.canvas_w = self.width;
        g.canvas_h = self.height;
        g.present_seq += 1;
        let want = g.present_seq;
        host.cv.notify_all();
        while g.done_seq < want && g.window_open && !g.should_close {
            g = host.cv.wait(g).unwrap();
        }
        self.input = g.input.clone();
        self.win_w = g.win_w.max(1);
        self.win_h = g.win_h.max(1);
        self.open = g.window_open && !g.should_close;
    }

    // ── Dibujo (canvas local, honra clip) ──────────────────────────────────────
    #[inline]
    fn put(&mut self, x: i32, y: i32, color: u32) {
        let (cx0, cy0, cx1, cy1) = self.clip;
        if x < cx0 || y < cy0 || x >= cx1 || y >= cy1 {
            return;
        }
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        self.canvas[(y as usize) * self.width + x as usize] = color;
    }

    #[inline]
    fn blend(&mut self, x: i32, y: i32, r: u8, g: u8, b: u8, a: u32) {
        if a == 0 {
            return;
        }
        if a >= 255 {
            self.put(x, y, ((r as u32) << 16) | ((g as u32) << 8) | b as u32);
            return;
        }
        let (cx0, cy0, cx1, cy1) = self.clip;
        if x < cx0 || y < cy0 || x >= cx1 || y >= cy1 {
            return;
        }
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        let idx = (y as usize) * self.width + x as usize;
        let dst = self.canvas[idx];
        let inv = 255 - a;
        let dr = (dst >> 16) & 0xff;
        let dg = (dst >> 8) & 0xff;
        let db = dst & 0xff;
        let nr = (r as u32 * a + dr * inv) / 255;
        let ng = (g as u32 * a + dg * inv) / 255;
        let nb = (b as u32 * a + db * inv) / 255;
        self.canvas[idx] = (nr << 16) | (ng << 8) | nb;
    }

    fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        let (cx0, cy0, cx1, cy1) = self.clip;
        let x0 = x.max(0).max(cx0);
        let y0 = y.max(0).max(cy0);
        let x1 = (x + w).min(self.width as i32).min(cx1);
        let y1 = (y + h).min(self.height as i32).min(cy1);
        let mut yy = y0;
        while yy < y1 {
            let row = (yy as usize) * self.width;
            let mut xx = x0;
            while xx < x1 {
                self.canvas[row + xx as usize] = color;
                xx += 1;
            }
            yy += 1;
        }
    }

    fn blend_rect(&mut self, x: i32, y: i32, w: i32, h: i32, r: u8, g: u8, b: u8, a: u32) {
        let mut yy = y;
        while yy < y + h {
            let mut xx = x;
            while xx < x + w {
                self.blend(xx, yy, r, g, b, a);
                xx += 1;
            }
            yy += 1;
        }
    }

    fn draw_line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, color: u32) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.put(x0, y0, color);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    fn draw_text(&mut self, fonts: &mut GuiFonts, x: i32, y: i32, text: &str, scale: i32, rgb: u32) {
        let scale = scale.max(1);
        let r = ((rgb >> 16) & 0xff) as u8;
        let g = ((rgb >> 8) & 0xff) as u8;
        let b = (rgb & 0xff) as u8;
        let fam = fonts.current;
        let mut pen = x;
        for ch in text.chars() {
            if ch.is_control() {
                if fam == 0 {
                    pen += 8 * scale; // compat: la rejilla siempre avanza una celda
                }
                continue;
            }
            if ch == ' ' && fam == 0 {
                pen += 8 * scale;
                continue;
            }
            fonts.ensure_glyph(ch, scale);
            if let Some(gl) = fonts.glyphs.get(&(fam, ch, scale)) {
                let advance = gl.advance;
                if ch != ' ' {
                    let cells = gl.cells.clone();
                    for (gx, gy, a) in cells {
                        self.blend(pen + gx, y + gy, r, g, b, a as u32);
                    }
                }
                pen += advance;
            } else if fam == 0 {
                pen += 8 * scale;
            }
        }
    }

    /// Rectángulo relleno con esquinas redondeadas antialiased (radio en px).
    fn fill_round_rect(&mut self, x: i32, y: i32, w: i32, h: i32, radius: i32, color: u32) {
        let r = radius.min(w / 2).min(h / 2).max(0);
        if r == 0 {
            self.fill_rect(x, y, w, h, color);
            return;
        }
        self.fill_rect(x, y + r, w, h - 2 * r, color);
        self.fill_rect(x + r, y, w - 2 * r, r, color);
        self.fill_rect(x + r, y + h - r, w - 2 * r, r, color);
        let cr = ((color >> 16) & 0xff) as u8;
        let cg = ((color >> 8) & 0xff) as u8;
        let cb = (color & 0xff) as u8;
        let rf = r as f32;
        let mut dy = 0;
        while dy < r {
            let mut dx = 0;
            while dx < r {
                let fx = rf - (dx as f32 + 0.5);
                let fy = rf - (dy as f32 + 0.5);
                let dist = (fx * fx + fy * fy).sqrt();
                let cov = (rf - dist + 0.5).clamp(0.0, 1.0);
                let a = (cov * 255.0) as u32;
                if a > 0 {
                    self.blend(x + dx, y + dy, cr, cg, cb, a);
                    self.blend(x + w - 1 - dx, y + dy, cr, cg, cb, a);
                    self.blend(x + dx, y + h - 1 - dy, cr, cg, cb, a);
                    self.blend(x + w - 1 - dx, y + h - 1 - dy, cr, cg, cb, a);
                }
                dx += 1;
            }
            dy += 1;
        }
    }

    /// Rectángulo de solo contorno (1px, clipeado).
    fn draw_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        self.fill_rect(x,         y,         w,     1,     color);
        self.fill_rect(x,         y + h - 1, w,     1,     color);
        self.fill_rect(x,         y + 1,     1,     h - 2, color);
        self.fill_rect(x + w - 1, y + 1,     1,     h - 2, color);
    }

    /// Círculo relleno antialiased (scanline + AA en el borde).
    fn fill_circle(&mut self, cx: i32, cy: i32, r: i32, color: u32) {
        if r <= 0 { return; }
        let cr = ((color >> 16) & 0xff) as u8;
        let cg = ((color >> 8) & 0xff) as u8;
        let cb = (color & 0xff) as u8;
        let rf = r as f32;
        let mut dy = -r;
        while dy <= r {
            let mut dx = -r;
            while dx <= r {
                let dist = ((dx * dx + dy * dy) as f32).sqrt();
                let cov = (rf - dist + 0.5).clamp(0.0, 1.0);
                let a = (cov * 255.0) as u32;
                if a > 0 { self.blend(cx + dx, cy + dy, cr, cg, cb, a); }
                dx += 1;
            }
            dy += 1;
        }
    }

    fn draw_image(&mut self, x: i32, y: i32, handle: i64) {
        let img = match self.images.get(&handle) {
            Some(im) => (im.w, im.h, im.px.clone()),
            None => return,
        };
        let (iw, ih, px) = img;
        let mut yy = 0;
        while yy < ih as i32 {
            let mut xx = 0;
            while xx < iw as i32 {
                let p = px[(yy as usize) * iw + xx as usize];
                let a = (p >> 24) & 0xff;
                let r = ((p >> 16) & 0xff) as u8;
                let g = ((p >> 8) & 0xff) as u8;
                let b = (p & 0xff) as u8;
                self.blend(x + xx, y + yy, r, g, b, a);
                xx += 1;
            }
            yy += 1;
        }
    }
}

// ── Lado del hilo MAIN: ventana + EventLoop ───────────────────────────────────────

struct GuiMain {
    host: Arc<GuiHost>,
    window: Option<Rc<Window>>,
    context: Option<Context<Rc<Window>>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    session_active: bool,
    close_requested: bool,
    // input — nivel
    keys_down: HashSet<String>,
    mods: ModifiersState,
    mouse_x: i32,
    mouse_y: i32,
    mouse_l: bool,
    mouse_r: bool,
    mouse_m: bool,
    prev_serviced_mouse_l: bool,
    // input — acumuladores por frame
    keys_pressed: Vec<String>,
    keys_repeated: Vec<String>,
    keys_released: Vec<String>,
    chars_typed: String,
    scroll_x: f32,
    scroll_y: f32,
    last_present: u64,
}

impl GuiMain {
    fn new(host: Arc<GuiHost>) -> Self {
        GuiMain {
            host,
            window: None,
            context: None,
            surface: None,
            session_active: false,
            close_requested: false,
            keys_down: HashSet::new(),
            mods: ModifiersState::empty(),
            mouse_x: -1,
            mouse_y: -1,
            mouse_l: false,
            mouse_r: false,
            mouse_m: false,
            prev_serviced_mouse_l: false,
            keys_pressed: Vec::new(),
            keys_repeated: Vec::new(),
            keys_released: Vec::new(),
            chars_typed: String::new(),
            scroll_x: 0.0,
            scroll_y: 0.0,
            last_present: 0,
        }
    }

    /// Crea la ventana + surface a partir de un comando Open pendiente.
    fn ensure_window(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let open = {
            let mut g = self.host.inner.lock().unwrap();
            let pos = g.cmds.iter().position(|c| matches!(c, GuiCmd::Open { .. }));
            match pos {
                Some(p) => match g.cmds.remove(p) {
                    Some(GuiCmd::Open { title, w, h }) => Some((title, w, h)),
                    _ => None,
                },
                None => None,
            }
        };
        let (title, w, h) = match open {
            Some(v) => v,
            None => return,
        };
        let attrs = Window::default_attributes()
            .with_title(title)
            .with_inner_size(LogicalSize::new(w as f64, h as f64));
        let window = match el.create_window(attrs) {
            Ok(win) => Rc::new(win),
            Err(_) => {
                let mut g = self.host.inner.lock().unwrap();
                g.open_failed = true;
                self.host.cv.notify_all();
                return;
            }
        };
        window.set_ime_allowed(true);
        let context = match Context::new(window.clone()) {
            Ok(c) => c,
            Err(_) => {
                let mut g = self.host.inner.lock().unwrap();
                g.open_failed = true;
                self.host.cv.notify_all();
                return;
            }
        };
        let surface = match Surface::new(&context, window.clone()) {
            Ok(s) => s,
            Err(_) => {
                let mut g = self.host.inner.lock().unwrap();
                g.open_failed = true;
                self.host.cv.notify_all();
                return;
            }
        };
        let size = window.inner_size();
        self.context = Some(context);
        self.surface = Some(surface);
        self.window = Some(window);
        let mut g = self.host.inner.lock().unwrap();
        g.window_ready = true;
        g.window_open = true;
        g.should_close = false;
        g.win_w = size.width.max(1) as usize;
        g.win_h = size.height.max(1) as usize;
        self.host.cv.notify_all();
    }

    /// Atiende comandos + present pendientes (llamado tras cada pump).
    fn service(&mut self) {
        let host = self.host.clone();
        let mut canvas_to_blit: Option<(Vec<u32>, usize, usize)> = None;
        {
            let mut g = host.inner.lock().unwrap();
            // Comandos (Open lo maneja ensure_window).
            let mut keep: VecDeque<GuiCmd> = VecDeque::new();
            while let Some(cmd) = g.cmds.pop_front() {
                match cmd {
                    GuiCmd::Open { .. } => keep.push_back(cmd),
                    GuiCmd::Close => self.close_requested = true,
                    GuiCmd::SetTitle(t) => {
                        if let Some(win) = &self.window { win.set_title(&t); }
                    }
                    GuiCmd::SetCursor(c) => {
                        if let Some(win) = &self.window { win.set_cursor(cursor_icon(&c)); }
                    }
                    GuiCmd::SetImePosition(x, y) => {
                        if let Some(win) = &self.window {
                            use winit::dpi::PhysicalPosition;
                            win.set_ime_cursor_area(
                                PhysicalPosition::new(x, y),
                                winit::dpi::PhysicalSize::new(2, 16),
                            );
                        }
                    }
                }
            }
            g.cmds = keep;
            // Present pendiente → blit.
            if g.present_seq > self.last_present {
                canvas_to_blit = Some((g.canvas.clone(), g.canvas_w, g.canvas_h));
                self.last_present = g.present_seq;
                g.done_seq = g.present_seq;
                g.input = self.take_input();
            }
            // Tamaño de ventana → compartido.
            if let Some(win) = &self.window {
                let s = win.inner_size();
                g.win_w = s.width.max(1) as usize;
                g.win_h = s.height.max(1) as usize;
            }
            // Cierre.
            if self.close_requested || g.interp_done {
                g.should_close = true;
                g.window_open = false;
                self.session_active = false;
            }
            host.cv.notify_all();
        }
        if let Some((canvas, cw, ch)) = canvas_to_blit {
            self.blit(&canvas, cw, ch);
        }
        if !self.session_active {
            self.window = None;
            self.surface = None;
            self.context = None;
            self.close_requested = false;
        }
    }

    fn take_input(&mut self) -> InputSnapshot {
        let pressed = self.mouse_l && !self.prev_serviced_mouse_l;
        self.prev_serviced_mouse_l = self.mouse_l;
        let snap = InputSnapshot {
            keys_down: self.keys_down.clone(),
            shift: self.mods.shift_key(),
            ctrl: self.mods.control_key(),
            alt: self.mods.alt_key(),
            sup: self.mods.super_key(),
            mouse_x: self.mouse_x,
            mouse_y: self.mouse_y,
            mouse_l: self.mouse_l,
            mouse_r: self.mouse_r,
            mouse_m: self.mouse_m,
            mouse_pressed: pressed,
            keys_pressed: std::mem::take(&mut self.keys_pressed),
            keys_repeated: std::mem::take(&mut self.keys_repeated),
            keys_released: std::mem::take(&mut self.keys_released),
            chars_typed: std::mem::take(&mut self.chars_typed),
            scroll_x: self.scroll_x as i64,
            scroll_y: self.scroll_y as i64,
        };
        self.scroll_x = 0.0;
        self.scroll_y = 0.0;
        snap
    }

    fn blit(&mut self, canvas: &[u32], cw: usize, ch: usize) {
        let window = match self.window.as_ref() {
            Some(w) => w,
            None => return,
        };
        let size = window.inner_size();
        let (bw, bh) = (size.width as usize, size.height as usize);
        if bw == 0 || bh == 0 {
            return;
        }
        let surface = match self.surface.as_mut() {
            Some(s) => s,
            None => return,
        };
        if let (Some(nw), Some(nh)) = (NonZeroU32::new(bw as u32), NonZeroU32::new(bh as u32)) {
            let _ = surface.resize(nw, nh);
        }
        let mut buffer = match surface.buffer_mut() {
            Ok(b) => b,
            Err(_) => return,
        };
        for y in 0..bh {
            let brow = y * bw;
            if y < ch {
                let crow = y * cw;
                let n = bw.min(cw);
                buffer[brow..brow + n].copy_from_slice(&canvas[crow..crow + n]);
                for x in n..bw {
                    buffer[brow + x] = 0;
                }
            } else {
                for x in 0..bw {
                    buffer[brow + x] = 0;
                }
            }
        }
        let _ = buffer.present();
    }
}

impl ApplicationHandler for GuiMain {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.ensure_window(event_loop);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.ensure_window(event_loop);
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.close_requested = true;
            }
            WindowEvent::ModifiersChanged(m) => {
                self.mods = m.state();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let name = key_name(&event.logical_key);
                match event.state {
                    ElementState::Pressed => {
                        if let Some(n) = name.clone() {
                            if !event.repeat {
                                self.keys_pressed.push(n.clone());
                                self.keys_down.insert(n.clone());
                            }
                            self.keys_repeated.push(n);
                        }
                        if let Some(t) = &event.text {
                            for c in t.chars() {
                                if !c.is_control() {
                                    self.chars_typed.push(c);
                                }
                            }
                        }
                    }
                    ElementState::Released => {
                        if let Some(n) = name {
                            self.keys_released.push(n.clone());
                            self.keys_down.remove(&n);
                        }
                    }
                }
            }
            WindowEvent::Ime(Ime::Commit(s)) => {
                for c in s.chars() {
                    if !c.is_control() {
                        self.chars_typed.push(c);
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_x = position.x as i32;
                self.mouse_y = position.y as i32;
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let down = state == ElementState::Pressed;
                match button {
                    MouseButton::Left => self.mouse_l = down,
                    MouseButton::Right => self.mouse_r = down,
                    MouseButton::Middle => self.mouse_m = down,
                    _ => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => match delta {
                MouseScrollDelta::LineDelta(dx, dy) => {
                    self.scroll_x += dx;
                    self.scroll_y += dy;
                }
                MouseScrollDelta::PixelDelta(p) => {
                    self.scroll_x += (p.x as f32) / 12.0;
                    self.scroll_y += (p.y as f32) / 12.0;
                }
            },
            _ => {}
        }
    }
}

/// Bucle del hilo MAIN: idle hasta un Open o interp_done; por sesión, bombea winit
/// y atiende los present hasta que la ventana se cierre. El EventLoop se crea una
/// sola vez (en el hilo principal — válido en Windows/macOS/Linux) y se reutiliza.
pub fn gui_host_main_loop(host: Arc<GuiHost>) {
    let mut event_loop: Option<EventLoop<()>> = None;
    let mut app = GuiMain::new(host.clone());

    loop {
        // ── Idle: esperar un comando Open o que el intérprete termine ──────────
        {
            let mut g = host.inner.lock().unwrap();
            loop {
                if g.interp_done {
                    return;
                }
                if g.cmds.iter().any(|c| matches!(c, GuiCmd::Open { .. })) {
                    break;
                }
                // Descartar comandos sueltos (no-Open) mientras no hay ventana.
                g.cmds.retain(|c| matches!(c, GuiCmd::Open { .. }));
                g = host.cv.wait(g).unwrap();
            }
        }

        // ── Crear el EventLoop una sola vez (en el hilo principal) ─────────────
        if event_loop.is_none() {
            match EventLoop::new() {
                Ok(el) => event_loop = Some(el),
                Err(_) => {
                    let mut g = host.inner.lock().unwrap();
                    g.open_failed = true;
                    g.cmds.clear();
                    host.cv.notify_all();
                    continue;
                }
            }
        }

        // ── Sesión: bombear + atender hasta cerrar la ventana ──────────────────
        app.session_active = true;
        app.close_requested = false;
        app.last_present = 0;
        {
            let mut g = host.inner.lock().unwrap();
            g.window_ready = false;
            g.done_seq = 0;
            g.present_seq = 0;
        }
        let el = event_loop.as_mut().unwrap();
        let mut guard = 0u32;
        while app.session_active {
            let _ = el.pump_app_events(Some(Duration::from_millis(4)), &mut app);
            app.service();
            // Salvaguarda: si la ventana no se creó (open_failed), no girar para siempre.
            if app.window.is_none() {
                guard += 1;
                let failed = host.inner.lock().unwrap().open_failed;
                if failed || guard > 250 {
                    app.session_active = false;
                }
            } else {
                guard = 0;
            }
        }
    }
}

/// Mapea una tecla lógica de winit a su nombre canónico de serez-code.
fn key_name(key: &Key) -> Option<String> {
    let s = match key {
        Key::Named(nk) => match nk {
            NamedKey::Enter => "Enter",
            NamedKey::Escape => "Esc",
            NamedKey::Space => "Space",
            NamedKey::Backspace => "Backspace",
            NamedKey::Tab => "Tab",
            NamedKey::Delete => "Delete",
            NamedKey::ArrowLeft => "Left",
            NamedKey::ArrowRight => "Right",
            NamedKey::ArrowUp => "Up",
            NamedKey::ArrowDown => "Down",
            NamedKey::Home => "Home",
            NamedKey::End => "End",
            NamedKey::PageUp => "PageUp",
            NamedKey::PageDown => "PageDown",
            NamedKey::Shift => "Shift",
            NamedKey::Control => "Ctrl",
            NamedKey::Alt => "Alt",
            NamedKey::Super => "Super",
            _ => return None,
        }
        .to_string(),
        Key::Character(c) => {
            let lower = c.to_lowercase();
            if lower.is_empty() {
                return None;
            }
            lower
        }
        _ => return None,
    };
    Some(s)
}

fn cursor_icon(name: &str) -> CursorIcon {
    match name {
        "text" | "ibeam" => CursorIcon::Text,
        "hand" | "pointer" => CursorIcon::Pointer,
        "crosshair" => CursorIcon::Crosshair,
        "wait" | "progress" => CursorIcon::Progress,
        "move" | "all-scroll" => CursorIcon::Move,
        "ew-resize" | "col-resize" => CursorIcon::EwResize,
        "ns-resize" | "row-resize" => CursorIcon::NsResize,
        "not-allowed" => CursorIcon::NotAllowed,
        _ => CursorIcon::Default,
    }
}

impl super::Evaluator {

    // ── Gui ─────────────────────────────────────────────────────────────────────

    pub(super) fn eval_gui_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        if !self.permissions.contains("Gui") {
            eprintln!(
                "❌ ERROR: 'Gui' requires permission 'Gui' — declare it in serez.json \
                 (\"permissions\": [\"Gui\", ...]) or with `use permissions {{ Gui }}`"
            );
            return EvalResult::Error;
        }

        match dot_call.method.as_str() {

            "open" => {
                if dot_call.arguments.len() != 3 {
                    eprintln!("❌ ERROR: Gui.open(title, width, height) requires 3 arguments");
                    return EvalResult::Error;
                }
                let title = match self.gui_str_arg(&dot_call.arguments[0]) {
                    Some(s) => s,
                    None => { eprintln!("❌ ERROR: Gui.open title must be a string"); return EvalResult::Error; }
                };
                let w = match self.gui_int_arg(&dot_call.arguments[1]) {
                    Some(v) if v > 0 => v as u32,
                    _ => { eprintln!("❌ ERROR: Gui.open width must be a positive integer"); return EvalResult::Error; }
                };
                let h = match self.gui_int_arg(&dot_call.arguments[2]) {
                    Some(v) if v > 0 => v as u32,
                    _ => { eprintln!("❌ ERROR: Gui.open height must be a positive integer"); return EvalResult::Error; }
                };
                let host = match host() {
                    Some(h) => h.clone(),
                    None => { eprintln!("❌ ERROR: Gui.open: GUI host not initialized"); return EvalResult::Error; }
                };
                let (ww, wh) = {
                    let mut g = host.inner.lock().unwrap();
                    g.open_failed = false;
                    g.window_ready = false;
                    g.should_close = false;
                    g.cmds.push_back(GuiCmd::Open { title, w, h });
                    host.cv.notify_all();
                    while !g.window_ready && !g.open_failed {
                        g = host.cv.wait(g).unwrap();
                    }
                    if g.open_failed {
                        drop(g);
                        eprintln!("❌ ERROR: Gui.open: failed to create window");
                        return EvalResult::Error;
                    }
                    (g.win_w.max(1), g.win_h.max(1))
                };
                self.gui_state = Some(GuiState::new(ww, wh));
                EvalResult::Value(self.null_ref)
            }

            "isOpen" => {
                let open = host().map(|h| {
                    let g = h.inner.lock().unwrap();
                    g.window_open && !g.should_close
                }).unwrap_or(false) && self.gui_state.is_some();
                EvalResult::Value(if open { self.true_ref } else { self.false_ref })
            }

            "size" => {
                let (w, h) = self.gui_state.as_ref().map(|s| (s.win_w as i64, s.win_h as i64)).unwrap_or((0, 0));
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: Some("int".to_string()),
                    elements: vec![OwnedValue::Integer(w), OwnedValue::Integer(h)],
                }))
            }

            "present" => {
                let host = match host() { Some(h) => h.clone(), None => { eprintln!("❌ ERROR: Gui.present: no GUI host"); return EvalResult::Error; } };
                match self.gui_state.as_mut() {
                    Some(st) => { st.present(&host); EvalResult::Value(self.null_ref) }
                    None => { eprintln!("❌ ERROR: Gui.present: no window open"); EvalResult::Error }
                }
            }

            "clear" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Gui.clear(color) requires 1 argument");
                    return EvalResult::Error;
                }
                let color = match self.gui_int_arg(&dot_call.arguments[0]) {
                    Some(v) => (v as u32) & 0x00FF_FFFF,
                    None => { eprintln!("❌ ERROR: Gui.clear color must be an integer 0xRRGGBB"); return EvalResult::Error; }
                };
                match self.gui_state.as_mut() {
                    Some(st) => {
                        // Reconciliar el canvas con el tamaño de ventana del último present.
                        if st.win_w != st.width || st.win_h != st.height {
                            st.width = st.win_w.max(1);
                            st.height = st.win_h.max(1);
                            st.canvas = vec![color; st.width * st.height];
                        }
                        st.bg = color;
                        for px in st.canvas.iter_mut() { *px = color; }
                        st.clip = (0, 0, st.width as i32, st.height as i32);
                        st.clip_stack.clear();
                        EvalResult::Value(self.null_ref)
                    }
                    None => { eprintln!("❌ ERROR: Gui.clear: no window open (call Gui.open first)"); EvalResult::Error }
                }
            }

            "fillRect" => {
                let (x, y, w, h, color) = match self.gui_rect_args(dot_call) {
                    Some(v) => v, None => return EvalResult::Error,
                };
                match self.gui_state.as_mut() {
                    Some(st) => { st.fill_rect(x as i32, y as i32, w as i32, h as i32, color); EvalResult::Value(self.null_ref) }
                    None => { eprintln!("❌ ERROR: Gui.fillRect: no window open"); EvalResult::Error }
                }
            }

            "fillRectAlpha" => {
                if dot_call.arguments.len() != 6 {
                    eprintln!("❌ ERROR: Gui.fillRectAlpha(x, y, w, h, color, alpha) requires 6 arguments");
                    return EvalResult::Error;
                }
                let x = self.gui_int_arg(&dot_call.arguments[0]);
                let y = self.gui_int_arg(&dot_call.arguments[1]);
                let w = self.gui_int_arg(&dot_call.arguments[2]);
                let h = self.gui_int_arg(&dot_call.arguments[3]);
                let c = self.gui_int_arg(&dot_call.arguments[4]);
                let a = self.gui_int_arg(&dot_call.arguments[5]);
                let (x, y, w, h, color, alpha) = match (x, y, w, h, c, a) {
                    (Some(x), Some(y), Some(w), Some(h), Some(c), Some(a)) =>
                        (x as i32, y as i32, w as i32, h as i32, (c as u32) & 0x00FF_FFFF, a.clamp(0, 255) as u32),
                    _ => { eprintln!("❌ ERROR: Gui.fillRectAlpha requires 6 integers"); return EvalResult::Error; }
                };
                match self.gui_state.as_mut() {
                    Some(st) => {
                        let r = ((color >> 16) & 0xff) as u8;
                        let g = ((color >> 8) & 0xff) as u8;
                        let b = (color & 0xff) as u8;
                        st.blend_rect(x, y, w, h, r, g, b, alpha);
                        EvalResult::Value(self.null_ref)
                    }
                    None => { eprintln!("❌ ERROR: Gui.fillRectAlpha: no window open"); EvalResult::Error }
                }
            }

            "setPixel" => {
                if dot_call.arguments.len() != 3 {
                    eprintln!("❌ ERROR: Gui.setPixel(x, y, color) requires 3 arguments");
                    return EvalResult::Error;
                }
                let x = self.gui_int_arg(&dot_call.arguments[0]);
                let y = self.gui_int_arg(&dot_call.arguments[1]);
                let c = self.gui_int_arg(&dot_call.arguments[2]);
                let (x, y, color) = match (x, y, c) {
                    (Some(x), Some(y), Some(c)) => (x as i32, y as i32, (c as u32) & 0x00FF_FFFF),
                    _ => { eprintln!("❌ ERROR: Gui.setPixel requires 3 integers"); return EvalResult::Error; }
                };
                match self.gui_state.as_mut() {
                    Some(st) => { st.put(x, y, color); EvalResult::Value(self.null_ref) }
                    None => { eprintln!("❌ ERROR: Gui.setPixel: no window open"); EvalResult::Error }
                }
            }

            "drawLine" => {
                if dot_call.arguments.len() != 5 {
                    eprintln!("❌ ERROR: Gui.drawLine(x0, y0, x1, y1, color) requires 5 arguments");
                    return EvalResult::Error;
                }
                let x0 = self.gui_int_arg(&dot_call.arguments[0]);
                let y0 = self.gui_int_arg(&dot_call.arguments[1]);
                let x1 = self.gui_int_arg(&dot_call.arguments[2]);
                let y1 = self.gui_int_arg(&dot_call.arguments[3]);
                let c  = self.gui_int_arg(&dot_call.arguments[4]);
                let (x0, y0, x1, y1, color) = match (x0, y0, x1, y1, c) {
                    (Some(a), Some(b), Some(c), Some(d), Some(e)) => (a as i32, b as i32, c as i32, d as i32, (e as u32) & 0x00FF_FFFF),
                    _ => { eprintln!("❌ ERROR: Gui.drawLine requires 5 integers"); return EvalResult::Error; }
                };
                match self.gui_state.as_mut() {
                    Some(st) => { st.draw_line(x0, y0, x1, y1, color); EvalResult::Value(self.null_ref) }
                    None => { eprintln!("❌ ERROR: Gui.drawLine: no window open"); EvalResult::Error }
                }
            }

            "drawText" => {
                if dot_call.arguments.len() != 5 {
                    eprintln!("❌ ERROR: Gui.drawText(x, y, text, scale, color) requires 5 arguments");
                    return EvalResult::Error;
                }
                let x     = self.gui_int_arg(&dot_call.arguments[0]);
                let y     = self.gui_int_arg(&dot_call.arguments[1]);
                let text  = self.gui_str_arg(&dot_call.arguments[2]);
                let scale = self.gui_int_arg(&dot_call.arguments[3]);
                let c     = self.gui_int_arg(&dot_call.arguments[4]);
                let (x, y, text, scale, color) = match (x, y, text, scale, c) {
                    (Some(x), Some(y), Some(t), Some(s), Some(c)) =>
                        (x as i32, y as i32, t, s.max(1) as i32, (c as u32) & 0x00FF_FFFF),
                    _ => { eprintln!("❌ ERROR: Gui.drawText requires (int, int, string, int, int)"); return EvalResult::Error; }
                };
                if self.gui_state.is_none() {
                    eprintln!("❌ ERROR: Gui.drawText: no window open");
                    return EvalResult::Error;
                }
                if self.gui_fonts.is_none() { self.gui_fonts = Some(GuiFonts::new()); }
                let fonts = self.gui_fonts.as_mut().unwrap();
                let st = self.gui_state.as_mut().unwrap();
                st.draw_text(fonts, x, y, &text, scale, color);
                EvalResult::Value(self.null_ref)
            }

            "measureText" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Gui.measureText(text, scale) requires 2 arguments");
                    return EvalResult::Error;
                }
                let text  = self.gui_str_arg(&dot_call.arguments[0]);
                let scale = self.gui_int_arg(&dot_call.arguments[1]);
                let (text, scale) = match (text, scale) {
                    (Some(t), Some(s)) => (t, s.max(1)),
                    _ => { eprintln!("❌ ERROR: Gui.measureText requires (string, int)"); return EvalResult::Error; }
                };
                // Familia default → aritmética de rejilla (sin tocar FontSystem);
                // familia custom → ancho real por advances (proporcional).
                let w = match self.gui_fonts.as_mut() {
                    Some(f) if f.current != 0 => f.measure(&text, scale as i32),
                    _ => text.chars().count() as i64 * 8 * scale,
                };
                let h = 8 * scale;
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: Some("int".to_string()),
                    elements: vec![OwnedValue::Integer(w), OwnedValue::Integer(h)],
                }))
            }

            "loadFont" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Gui.loadFont(path) requires 1 argument");
                    return EvalResult::Error;
                }
                let path = match self.gui_str_arg(&dot_call.arguments[0]) {
                    Some(s) => s,
                    None => { eprintln!("❌ ERROR: Gui.loadFont path must be a string"); return EvalResult::Error; }
                };
                if self.gui_fonts.is_none() { self.gui_fonts = Some(GuiFonts::new()); }
                match self.gui_fonts.as_mut().unwrap().load_font_file(&path) {
                    Some(family) => EvalResult::Value(self.alloc(ObjectData::Str(family))),
                    None => { eprintln!("❌ ERROR: Gui.loadFont: could not load font file '{}'", path); EvalResult::Error }
                }
            }

            "setFont" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Gui.setFont(family) requires 1 argument");
                    return EvalResult::Error;
                }
                let name = match self.gui_str_arg(&dot_call.arguments[0]) {
                    Some(s) => s,
                    None => { eprintln!("❌ ERROR: Gui.setFont family must be a string"); return EvalResult::Error; }
                };
                if self.gui_fonts.is_none() {
                    // Sin FontSystem todavía: el reset a default no necesita crearlo.
                    if name.is_empty() || name.eq_ignore_ascii_case("default") || name.eq_ignore_ascii_case("monospace") {
                        return EvalResult::Value(self.true_ref);
                    }
                    self.gui_fonts = Some(GuiFonts::new());
                }
                let ok = self.gui_fonts.as_mut().unwrap().set_family(&name);
                EvalResult::Value(if ok { self.true_ref } else { self.false_ref })
            }

            "font" => {
                let name = self.gui_fonts.as_ref()
                    .map(|f| f.families[f.current as usize].clone())
                    .unwrap_or_default();
                EvalResult::Value(self.alloc(ObjectData::Str(name)))
            }

            "fillRoundRect" => {
                if dot_call.arguments.len() != 6 {
                    eprintln!("❌ ERROR: Gui.fillRoundRect(x, y, w, h, radius, color) requires 6 arguments");
                    return EvalResult::Error;
                }
                let mut vals = [0i64; 6];
                for (i, slot) in vals.iter_mut().enumerate() {
                    match self.gui_int_arg(&dot_call.arguments[i]) {
                        Some(v) => *slot = v,
                        None => { eprintln!("❌ ERROR: Gui.fillRoundRect requires 6 integers"); return EvalResult::Error; }
                    }
                }
                let color = (vals[5] as u32) & 0x00FF_FFFF;
                match self.gui_state.as_mut() {
                    Some(st) => {
                        st.fill_round_rect(vals[0] as i32, vals[1] as i32, vals[2] as i32, vals[3] as i32, vals[4] as i32, color);
                        EvalResult::Value(self.null_ref)
                    }
                    None => { eprintln!("❌ ERROR: Gui.fillRoundRect: no window open"); EvalResult::Error }
                }
            }

            "time" => {
                // Milisegundos desde que se abrió la ventana — para animaciones y blink del caret.
                // Devuelve 0 si no hay ventana abierta.
                let ms = self.gui_state.as_ref()
                    .map(|s| s.open_time.elapsed().as_millis() as i64)
                    .unwrap_or(0);
                EvalResult::Value(self.int_ref(ms))
            }

            "drawRect" => {
                if dot_call.arguments.len() != 5 {
                    eprintln!("❌ ERROR: Gui.drawRect(x, y, w, h, color) requires 5 arguments");
                    return EvalResult::Error;
                }
                let mut vals = [0i64; 5];
                for (i, slot) in vals.iter_mut().enumerate() {
                    match self.gui_int_arg(&dot_call.arguments[i]) {
                        Some(v) => *slot = v,
                        None => { eprintln!("❌ ERROR: Gui.drawRect requires 5 integers"); return EvalResult::Error; }
                    }
                }
                let color = (vals[4] as u32) & 0x00FF_FFFF;
                match self.gui_state.as_mut() {
                    Some(st) => { st.draw_rect(vals[0] as i32, vals[1] as i32, vals[2] as i32, vals[3] as i32, color); EvalResult::Value(self.null_ref) }
                    None => { eprintln!("❌ ERROR: Gui.drawRect: no window open"); EvalResult::Error }
                }
            }

            "fillCircle" => {
                if dot_call.arguments.len() != 4 {
                    eprintln!("❌ ERROR: Gui.fillCircle(cx, cy, radius, color) requires 4 arguments");
                    return EvalResult::Error;
                }
                let mut vals = [0i64; 4];
                for (i, slot) in vals.iter_mut().enumerate() {
                    match self.gui_int_arg(&dot_call.arguments[i]) {
                        Some(v) => *slot = v,
                        None => { eprintln!("❌ ERROR: Gui.fillCircle requires 4 integers"); return EvalResult::Error; }
                    }
                }
                let color = (vals[3] as u32) & 0x00FF_FFFF;
                match self.gui_state.as_mut() {
                    Some(st) => { st.fill_circle(vals[0] as i32, vals[1] as i32, vals[2] as i32, color); EvalResult::Value(self.null_ref) }
                    None => { eprintln!("❌ ERROR: Gui.fillCircle: no window open"); EvalResult::Error }
                }
            }

            "setImePosition" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Gui.setImePosition(x, y) requires 2 arguments");
                    return EvalResult::Error;
                }
                let x = self.gui_int_arg(&dot_call.arguments[0]);
                let y = self.gui_int_arg(&dot_call.arguments[1]);
                let (x, y) = match (x, y) {
                    (Some(x), Some(y)) => (x as i32, y as i32),
                    _ => { eprintln!("❌ ERROR: Gui.setImePosition requires 2 integers"); return EvalResult::Error; }
                };
                if let Some(host) = host() {
                    let mut g = host.inner.lock().unwrap();
                    g.cmds.push_back(GuiCmd::SetImePosition(x, y));
                }
                EvalResult::Value(self.null_ref)
            }

            "mouse" => {
                let (mx, my) = self.gui_state.as_ref().map(|s| (s.input.mouse_x as i64, s.input.mouse_y as i64)).unwrap_or((0, 0));
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: Some("int".to_string()),
                    elements: vec![OwnedValue::Integer(mx), OwnedValue::Integer(my)],
                }))
            }

            "mouseDown" => {
                let down = self.gui_state.as_ref().map(|s| s.input.mouse_l).unwrap_or(false);
                EvalResult::Value(if down { self.true_ref } else { self.false_ref })
            }

            "mouseRightDown" => {
                let down = self.gui_state.as_ref().map(|s| s.input.mouse_r).unwrap_or(false);
                EvalResult::Value(if down { self.true_ref } else { self.false_ref })
            }

            "mouseMiddleDown" => {
                let down = self.gui_state.as_ref().map(|s| s.input.mouse_m).unwrap_or(false);
                EvalResult::Value(if down { self.true_ref } else { self.false_ref })
            }

            "mousePressed" => {
                let pressed = self.gui_state.as_ref().map(|s| s.input.mouse_pressed).unwrap_or(false);
                EvalResult::Value(if pressed { self.true_ref } else { self.false_ref })
            }

            "scroll" => {
                let (dx, dy) = self.gui_state.as_ref().map(|s| (s.input.scroll_x, s.input.scroll_y)).unwrap_or((0, 0));
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: Some("int".to_string()),
                    elements: vec![OwnedValue::Integer(dx), OwnedValue::Integer(dy)],
                }))
            }

            "keyDown" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Gui.keyDown(name) requires 1 argument");
                    return EvalResult::Error;
                }
                let name = match self.gui_str_arg(&dot_call.arguments[0]) {
                    Some(s) => s,
                    None => { eprintln!("❌ ERROR: Gui.keyDown name must be a string"); return EvalResult::Error; }
                };
                let down = match self.gui_state.as_ref() {
                    Some(st) => match name.as_str() {
                        "Shift" => st.input.shift,
                        "Ctrl" | "Control" => st.input.ctrl,
                        "Alt" => st.input.alt,
                        "Super" => st.input.sup,
                        _ => st.input.keys_down.contains(&name),
                    },
                    None => false,
                };
                EvalResult::Value(if down { self.true_ref } else { self.false_ref })
            }

            "keysPressed" => { let v = self.gui_state.as_ref().map(|s| s.input.keys_pressed.clone()).unwrap_or_default(); self.gui_str_array(v) }
            "keysRepeated" => { let v = self.gui_state.as_ref().map(|s| s.input.keys_repeated.clone()).unwrap_or_default(); self.gui_str_array(v) }
            "keysReleased" => { let v = self.gui_state.as_ref().map(|s| s.input.keys_released.clone()).unwrap_or_default(); self.gui_str_array(v) }

            "charsTyped" => {
                let s = self.gui_state.as_ref().map(|s| s.input.chars_typed.clone()).unwrap_or_default();
                EvalResult::Value(self.alloc(ObjectData::Str(s)))
            }

            "clipboardGet" => {
                let text = match self.gui_state.as_mut() {
                    Some(st) => {
                        if st.clipboard.is_none() { st.clipboard = arboard::Clipboard::new().ok(); }
                        st.clipboard.as_mut().and_then(|c| c.get_text().ok()).unwrap_or_default()
                    }
                    None => String::new(),
                };
                EvalResult::Value(self.alloc(ObjectData::Str(text)))
            }

            "clipboardSet" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Gui.clipboardSet(text) requires 1 argument");
                    return EvalResult::Error;
                }
                let text = self.gui_str_arg(&dot_call.arguments[0]).unwrap_or_default();
                if let Some(st) = self.gui_state.as_mut() {
                    if st.clipboard.is_none() { st.clipboard = arboard::Clipboard::new().ok(); }
                    if let Some(c) = st.clipboard.as_mut() { let _ = c.set_text(text); }
                }
                EvalResult::Value(self.null_ref)
            }

            "loadImage" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Gui.loadImage(path) requires 1 argument");
                    return EvalResult::Error;
                }
                let path = match self.gui_str_arg(&dot_call.arguments[0]) {
                    Some(s) => s,
                    None => { eprintln!("❌ ERROR: Gui.loadImage path must be a string"); return EvalResult::Error; }
                };
                let decoded = match image::open(&path) {
                    Ok(img) => img.to_rgba8(),
                    Err(e) => { eprintln!("❌ ERROR: Gui.loadImage: {}", e); return EvalResult::Error; }
                };
                let (w, h) = (decoded.width() as usize, decoded.height() as usize);
                let mut px = Vec::with_capacity(w * h);
                for pixel in decoded.pixels() {
                    let [r, g, b, a] = pixel.0;
                    px.push(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32);
                }
                match self.gui_state.as_mut() {
                    Some(st) => {
                        let id = st.next_image;
                        st.next_image += 1;
                        st.images.insert(id, ImageData { w, h, px });
                        EvalResult::Value(self.int_ref(id))
                    }
                    None => { eprintln!("❌ ERROR: Gui.loadImage: no window open"); EvalResult::Error }
                }
            }

            "drawImage" => {
                if dot_call.arguments.len() != 3 {
                    eprintln!("❌ ERROR: Gui.drawImage(x, y, handle) requires 3 arguments");
                    return EvalResult::Error;
                }
                let x = self.gui_int_arg(&dot_call.arguments[0]);
                let y = self.gui_int_arg(&dot_call.arguments[1]);
                let hnd = self.gui_int_arg(&dot_call.arguments[2]);
                let (x, y, hnd) = match (x, y, hnd) {
                    (Some(x), Some(y), Some(h)) => (x as i32, y as i32, h),
                    _ => { eprintln!("❌ ERROR: Gui.drawImage requires 3 integers"); return EvalResult::Error; }
                };
                match self.gui_state.as_mut() {
                    Some(st) => { st.draw_image(x, y, hnd); EvalResult::Value(self.null_ref) }
                    None => { eprintln!("❌ ERROR: Gui.drawImage: no window open"); EvalResult::Error }
                }
            }

            "imageSize" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Gui.imageSize(handle) requires 1 argument");
                    return EvalResult::Error;
                }
                let hnd = match self.gui_int_arg(&dot_call.arguments[0]) {
                    Some(h) => h,
                    None => { eprintln!("❌ ERROR: Gui.imageSize handle must be an integer"); return EvalResult::Error; }
                };
                let (w, h) = self.gui_state.as_ref()
                    .and_then(|s| s.images.get(&hnd))
                    .map(|im| (im.w as i64, im.h as i64))
                    .unwrap_or((0, 0));
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: Some("int".to_string()),
                    elements: vec![OwnedValue::Integer(w), OwnedValue::Integer(h)],
                }))
            }

            "pushClip" => {
                if dot_call.arguments.len() != 4 {
                    eprintln!("❌ ERROR: Gui.pushClip(x, y, w, h) requires 4 arguments");
                    return EvalResult::Error;
                }
                let x = self.gui_int_arg(&dot_call.arguments[0]);
                let y = self.gui_int_arg(&dot_call.arguments[1]);
                let w = self.gui_int_arg(&dot_call.arguments[2]);
                let h = self.gui_int_arg(&dot_call.arguments[3]);
                let (x, y, w, h) = match (x, y, w, h) {
                    (Some(x), Some(y), Some(w), Some(h)) => (x as i32, y as i32, w as i32, h as i32),
                    _ => { eprintln!("❌ ERROR: Gui.pushClip requires 4 integers"); return EvalResult::Error; }
                };
                if let Some(st) = self.gui_state.as_mut() {
                    st.clip_stack.push(st.clip);
                    let (cx0, cy0, cx1, cy1) = st.clip;
                    let nx0 = x.max(cx0);
                    let ny0 = y.max(cy0);
                    let nx1 = (x + w).min(cx1);
                    let ny1 = (y + h).min(cy1);
                    st.clip = (nx0, ny0, nx1.max(nx0), ny1.max(ny0));
                }
                EvalResult::Value(self.null_ref)
            }

            "popClip" => {
                if let Some(st) = self.gui_state.as_mut() {
                    if let Some(prev) = st.clip_stack.pop() {
                        st.clip = prev;
                    } else {
                        st.clip = (0, 0, st.width as i32, st.height as i32);
                    }
                }
                EvalResult::Value(self.null_ref)
            }

            "setTitle" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Gui.setTitle(text) requires 1 argument");
                    return EvalResult::Error;
                }
                let t = self.gui_str_arg(&dot_call.arguments[0]).unwrap_or_default();
                if let Some(h) = host() {
                    let mut g = h.inner.lock().unwrap();
                    g.cmds.push_back(GuiCmd::SetTitle(t));
                    h.cv.notify_all();
                }
                EvalResult::Value(self.null_ref)
            }

            "setCursor" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Gui.setCursor(style) requires 1 argument");
                    return EvalResult::Error;
                }
                let name = self.gui_str_arg(&dot_call.arguments[0]).unwrap_or_default();
                if let Some(h) = host() {
                    let mut g = h.inner.lock().unwrap();
                    g.cmds.push_back(GuiCmd::SetCursor(name));
                    h.cv.notify_all();
                }
                EvalResult::Value(self.null_ref)
            }

            "close" => {
                if let Some(h) = host() {
                    let mut g = h.inner.lock().unwrap();
                    g.cmds.push_back(GuiCmd::Close);
                    h.cv.notify_all();
                }
                self.gui_state = None;
                EvalResult::Value(self.null_ref)
            }

            _ => { eprintln!("❌ ERROR: Unknown Gui method '{}'", dot_call.method); EvalResult::Error }
        }
    }

    // ── arg helpers ──────────────────────────────────────────────────────────────

    fn gui_int_arg(&mut self, expr: &ast::Expression) -> Option<i64> {
        match self.eval_expression(expr) {
            EvalResult::Value(v) => match self.resolve(v).cloned() {
                Some(ObjectData::Integer(n)) => Some(n),
                _ => None,
            },
            _ => None,
        }
    }

    fn gui_str_arg(&mut self, expr: &ast::Expression) -> Option<String> {
        match self.eval_expression(expr) {
            EvalResult::Value(v) => match self.resolve(v).cloned() {
                Some(ObjectData::Str(s)) => Some(s),
                _ => None,
            },
            _ => None,
        }
    }

    fn gui_str_array(&mut self, names: Vec<String>) -> EvalResult {
        let mut elems: Vec<OwnedValue> = Vec::with_capacity(names.len());
        for n in names { elems.push(OwnedValue::Str(n)); }
        EvalResult::Value(self.alloc(ObjectData::Array {
            element_type: Some("string".to_string()),
            elements: elems,
        }))
    }

    fn gui_rect_args(&mut self, dot_call: &ast::DotCallExpression) -> Option<(i64, i64, i64, i64, u32)> {
        if dot_call.arguments.len() != 5 {
            eprintln!("❌ ERROR: Gui.fillRect(x, y, w, h, color) requires 5 arguments");
            return None;
        }
        let x = self.gui_int_arg(&dot_call.arguments[0]);
        let y = self.gui_int_arg(&dot_call.arguments[1]);
        let w = self.gui_int_arg(&dot_call.arguments[2]);
        let h = self.gui_int_arg(&dot_call.arguments[3]);
        let c = self.gui_int_arg(&dot_call.arguments[4]);
        match (x, y, w, h, c) {
            (Some(x), Some(y), Some(w), Some(h), Some(c)) => Some((x, y, w, h, (c as u32) & 0x00FF_FFFF)),
            _ => { eprintln!("❌ ERROR: Gui.fillRect requires 5 integers"); None }
        }
    }
}
