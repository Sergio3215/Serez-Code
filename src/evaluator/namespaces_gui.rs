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
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::platform::pump_events::EventLoopExtPumpEvents;
use winit::window::{CursorIcon, Icon, UserAttentionType, Window, WindowId, WindowLevel};

use softbuffer::{Context, Surface};
use cosmic_text::{Attrs, Buffer as TextBuffer, Color as TextColor, Family, FontSystem, Metrics, Shaping, Style as FontStyle, SwashCache, Weight};

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
    focused: bool,          // ¿la ventana tiene foco? (WindowEvent::Focused)
    mouse_in: bool,         // ¿el cursor está sobre la ventana? (CursorEntered/Left)
    mouse_back: bool,       // botón "atrás" del mouse (MouseButton::Back)
    mouse_fwd: bool,        // botón "adelante" del mouse (MouseButton::Forward)
    dropped_files: Vec<String>, // archivos soltados este frame (DroppedFile) — consumido por frame
    ime_preedit: String,    // composición IME en curso (Ime::Preedit), "" si no hay
    hovered_files: Vec<String>,        // archivos arrastrados SOBRE la ventana (antes de soltar) — nivel
    touches: Vec<(u64, u8, i32, i32)>, // toques este frame: (id, fase 0=start/1=move/2=end/3=cancel, x, y)
    pinch_delta: f64,                  // gesto de pinch/zoom acumulado este frame
}

/// Comandos del intérprete → hilo main.
enum GuiCmd {
    Open { title: String, w: u32, h: u32 },
    Close,
    SetTitle(String),
    SetCursor(String),
    SetImePosition(i32, i32),
    // Control de ventana (winit) — aditivo, ruteado por el hilo main en service().
    SetMinSize(u32, u32),
    SetResizable(bool),
    SetFullscreen(bool),
    SetMaximized(bool),
    SetPosition(i32, i32),
    SetDecorations(bool),
    SetMaxSize(u32, u32),
    DragWindow,                       // mover ventana borderless (winit::drag_window)
    SetAlwaysOnTop(bool),
    SetMinimized(bool),
    RequestAttention(bool),           // flash de taskbar
    SetCursorVisible(bool),
    SetWindowIcon(Vec<u8>, u32, u32), // rgba, w, h (vacío = quitar ícono)
    // Diálogo de archivo nativo (rfd) — ejecutado por el hilo main; el resultado
    // vuelve por SharedInner.dialog_result / dialog_seq (handshake como present).
    FileDialog { save: bool, filter_name: String, filter_exts: Vec<String>, default_name: String },
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
    scale_factor: f64,        // HiDPI: factor de escala del monitor (winit)
    input_epoch: u64,         // sube en cada evento de input (para idleWait)
    dialog_seq: u64,          // handshake de FileDialog (main → interp)
    dialog_done: u64,
    dialog_result: Option<String>,
    input: InputSnapshot,
    // caché de scroll asíncrono
    last_presented_canvas: Vec<u32>,
    last_presented_w: usize,
    last_presented_h: usize,
    virtual_scroll_y: i32,
    bg_color: u32,
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
            scale_factor: 1.0,
            input_epoch: 0,
            dialog_seq: 0,
            dialog_done: 0,
            dialog_result: None,
            input: InputSnapshot::default(),
            last_presented_canvas: Vec::new(),
            last_presented_w: 0,
            last_presented_h: 0,
            virtual_scroll_y: 0,
            bg_color: 0xFFFFFFFF,
        }
    }
}

/// Canal compartido global GUI. Lo inicializa `main` antes de lanzar el intérprete.
pub struct GuiHost {
    inner: Mutex<SharedInner>,
    cv: Condvar,
    // Despierta el pump del hilo main desde el intérprete (al presentar). Permite que
    // el pump duerma con timeout largo en reposo (CPU ~0) y reaccione al instante.
    proxy: Mutex<Option<EventLoopProxy<()>>>,
}

impl GuiHost {
    pub fn new() -> Self {
        GuiHost { inner: Mutex::new(SharedInner::new()), cv: Condvar::new(), proxy: Mutex::new(None) }
    }
    /// Despierta el bucle de eventos del hilo main (si hay proxy instalado).
    fn wake_main(&self) {
        if let Some(px) = self.proxy.lock().unwrap().as_ref() {
            let _ = px.send_event(());
        }
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
    // Clave: (familia, char, escala, estilo). estilo: bit0=bold, bit1=italic.
    glyphs: HashMap<(u32, char, i32, u8), Glyph>,
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

    fn ensure_glyph(&mut self, ch: char, scale: i32, style: u8) {
        let key = (self.current, ch, scale, style);
        if self.glyphs.contains_key(&key) {
            return;
        }
        let size = (8 * scale).max(8) as f32;
        let metrics = Metrics::new(size, size * 1.25);
        let mut buf = TextBuffer::new(&mut self.font_system, metrics);
        buf.set_size(&mut self.font_system, Some(size * 4.0), Some(size * 2.0));
        let family_name;
        let mut attrs = if self.current == 0 {
            Attrs::new().family(Family::Monospace)
        } else {
            family_name = self.families[self.current as usize].clone();
            Attrs::new().family(Family::Name(&family_name))
        };
        if style & 1 != 0 { attrs = attrs.weight(Weight::BOLD); }
        if style & 2 != 0 { attrs = attrs.style(FontStyle::Italic); }
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
            self.ensure_glyph(ch, scale, 0);
            if let Some(gl) = self.glyphs.get(&(self.current, ch, scale, 0)) {
                w += gl.advance as i64;
            }
        }
        w
    }

    /// Posiciones x acumuladas en los límites de carácter (long = nº de chars + 1;
    /// [0] = 0, [i] = x tras i chars). Para situar caret/selección con fuente
    /// proporcional. Coincide con el avance de draw_text/measure.
    fn advances(&mut self, text: &str, scale: i32) -> Vec<i64> {
        let scale = scale.max(1);
        let mut out = vec![0i64];
        let mut x = 0i64;
        if self.current == 0 {
            for _ in text.chars() {
                x += 8 * scale as i64;
                out.push(x);
            }
            return out;
        }
        for ch in text.chars() {
            if !ch.is_control() {
                self.ensure_glyph(ch, scale, 0);
                if let Some(gl) = self.glyphs.get(&(self.current, ch, scale, 0)) {
                    x += gl.advance as i64;
                }
            }
            out.push(x);
        }
        out
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
    scale_factor: f64,               // HiDPI (refrescado en present desde el main)
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
            scale_factor: 1.0,
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
        g.bg_color = self.bg;
        g.present_seq += 1;
        let want = g.present_seq;
        host.cv.notify_all();
        drop(g);
        host.wake_main();   // saca al pump de su espera larga → sirve el frame ya
        let mut g = host.inner.lock().unwrap();
        while g.done_seq < want && g.window_open && !g.should_close {
            g = host.cv.wait(g).unwrap();
        }
        self.input = g.input.clone();
        self.win_w = g.win_w.max(1);
        self.win_h = g.win_h.max(1);
        self.scale_factor = g.scale_factor;
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

    fn draw_text(&mut self, fonts: &mut GuiFonts, x: i32, y: i32, text: &str, scale: i32, rgb: u32, style: u8) {
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
            fonts.ensure_glyph(ch, scale, style);
            if let Some(gl) = fonts.glyphs.get(&(fam, ch, scale, style)) {
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

    /// Imagen escalada a (dw, dh) por muestreo de vecino más cercano, con alpha global
    /// extra (0–255) multiplicada sobre el alpha del pixel. dw/dh <= 0 → no dibuja.
    fn draw_image_scaled(&mut self, x: i32, y: i32, handle: i64, dw: i32, dh: i32, galpha: u32) {
        let img = match self.images.get(&handle) {
            Some(im) => (im.w, im.h, im.px.clone()),
            None => return,
        };
        let (iw, ih, px) = img;
        if dw <= 0 || dh <= 0 || iw == 0 || ih == 0 { return; }
        let ga = galpha.min(255);
        let mut dy = 0;
        while dy < dh {
            let sy = (dy as usize * ih) / dh as usize;
            let mut dx = 0;
            while dx < dw {
                let sx = (dx as usize * iw) / dw as usize;
                let p = px[sy.min(ih - 1) * iw + sx.min(iw - 1)];
                let a = (((p >> 24) & 0xff) * ga) / 255;
                let r = ((p >> 16) & 0xff) as u8;
                let g = ((p >> 8) & 0xff) as u8;
                let b = (p & 0xff) as u8;
                self.blend(x + dx, y + dy, r, g, b, a);
                dx += 1;
            }
            dy += 1;
        }
    }

    /// Relleno con gradiente lineal entre dos colores 0xRRGGBB. vertical=true interpola
    /// de arriba (c1) a abajo (c2); false de izquierda a derecha. Respeta el clip.
    fn fill_gradient(&mut self, x: i32, y: i32, w: i32, h: i32, c1: u32, c2: u32, vertical: bool) {
        if w <= 0 || h <= 0 { return; }
        let r1 = ((c1 >> 16) & 0xff) as i32; let g1 = ((c1 >> 8) & 0xff) as i32; let b1 = (c1 & 0xff) as i32;
        let r2 = ((c2 >> 16) & 0xff) as i32; let g2 = ((c2 >> 8) & 0xff) as i32; let b2 = (c2 & 0xff) as i32;
        let span = if vertical { h } else { w };
        let denom = (span - 1).max(1);
        let mut yy = 0;
        while yy < h {
            let mut xx = 0;
            while xx < w {
                let t = if vertical { yy } else { xx };
                let r = (r1 + (r2 - r1) * t / denom) as u32;
                let g = (g1 + (g2 - g1) * t / denom) as u32;
                let b = (b1 + (b2 - b1) * t / denom) as u32;
                self.put(x + xx, y + yy, (r << 16) | (g << 8) | b);
                xx += 1;
            }
            yy += 1;
        }
    }

    /// Box-blur in-place de una región del canvas (radio en px, 2 pasadas). Para
    /// paneles esmerilados / sombras suaves. Coste O(w*h*radio); radio acotado.
    fn blur_region(&mut self, x: i32, y: i32, w: i32, h: i32, radius: i32) {
        let (cx0, cy0, cx1, cy1) = self.clip;
        let x0 = x.max(0).max(cx0); let y0 = y.max(0).max(cy0);
        let x1 = (x + w).min(self.width as i32).min(cx1);
        let y1 = (y + h).min(self.height as i32).min(cy1);
        if x1 <= x0 || y1 <= y0 { return; }
        let rad = radius.clamp(1, 32);
        let rw = (x1 - x0) as usize; let rh = (y1 - y0) as usize;
        // Extrae la región a buffers RGB.
        let mut rr = vec![0i32; rw * rh];
        let mut gg = vec![0i32; rw * rh];
        let mut bb = vec![0i32; rw * rh];
        for j in 0..rh {
            let row = (y0 as usize + j) * self.width + x0 as usize;
            for i in 0..rw {
                let p = self.canvas[row + i];
                rr[j * rw + i] = ((p >> 16) & 0xff) as i32;
                gg[j * rw + i] = ((p >> 8) & 0xff) as i32;
                bb[j * rw + i] = (p & 0xff) as i32;
            }
        }
        // Pasada horizontal y vertical (separable).
        let blur_pass = |src: &Vec<i32>, w: usize, h: usize, horiz: bool| -> Vec<i32> {
            let mut out = vec![0i32; w * h];
            let r = rad as usize;
            if horiz {
                for j in 0..h {
                    for i in 0..w {
                        let lo = i.saturating_sub(r);
                        let hi = (i + r).min(w - 1);
                        let mut sum = 0i32;
                        for k in lo..=hi { sum += src[j * w + k]; }
                        out[j * w + i] = sum / (hi - lo + 1) as i32;
                    }
                }
            } else {
                for i in 0..w {
                    for j in 0..h {
                        let lo = j.saturating_sub(r);
                        let hi = (j + r).min(h - 1);
                        let mut sum = 0i32;
                        for k in lo..=hi { sum += src[k * w + i]; }
                        out[j * w + i] = sum / (hi - lo + 1) as i32;
                    }
                }
            }
            out
        };
        let rr = blur_pass(&blur_pass(&rr, rw, rh, true), rw, rh, false);
        let gg = blur_pass(&blur_pass(&gg, rw, rh, true), rw, rh, false);
        let bb = blur_pass(&blur_pass(&bb, rw, rh, true), rw, rh, false);
        for j in 0..rh {
            let row = (y0 as usize + j) * self.width + x0 as usize;
            for i in 0..rw {
                let p = ((rr[j * rw + i] as u32) << 16) | ((gg[j * rw + i] as u32) << 8) | bb[j * rw + i] as u32;
                self.canvas[row + i] = p;
            }
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
    focused: bool,              // nivel: ¿ventana enfocada?
    cursor_in: bool,            // nivel: ¿cursor sobre la ventana?
    mouse_back: bool,           // nivel: botón back
    mouse_fwd: bool,            // nivel: botón forward
    dropped_files: Vec<String>, // acumulador por frame: archivos soltados
    ime_preedit: String,        // nivel: composición IME en curso
    hovered_files: Vec<String>, // nivel: archivos arrastrados sobre la ventana
    touches: Vec<(u64, u8, i32, i32)>, // acumulador por frame: toques
    pinch_delta: f64,           // acumulador por frame: pinch/zoom
    last_present: u64,
    pending_input: bool,   // hubo input desde el último service → despertar idleWait
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
            focused: true,
            cursor_in: false,
            mouse_back: false,
            mouse_fwd: false,
            dropped_files: Vec::new(),
            ime_preedit: String::new(),
            hovered_files: Vec::new(),
            touches: Vec::new(),
            pinch_delta: 0.0,
            last_present: 0,
            pending_input: false,
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
        // El diálogo de archivo se ejecuta FUERA del lock (bloquea hasta que el usuario
        // elige) para no congelar al intérprete que espera el handshake.
        let mut pending_dialog: Option<(u64, bool, String, Vec<String>, String)> = None;
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
                    GuiCmd::SetMinSize(w, h) => {
                        if let Some(win) = &self.window {
                            let s = if w == 0 || h == 0 { None } else { Some(LogicalSize::new(w as f64, h as f64)) };
                            win.set_min_inner_size(s);
                        }
                    }
                    GuiCmd::SetResizable(b) => {
                        if let Some(win) = &self.window { win.set_resizable(b); }
                    }
                    GuiCmd::SetFullscreen(b) => {
                        if let Some(win) = &self.window {
                            win.set_fullscreen(if b { Some(winit::window::Fullscreen::Borderless(None)) } else { None });
                        }
                    }
                    GuiCmd::SetMaximized(b) => {
                        if let Some(win) = &self.window { win.set_maximized(b); }
                    }
                    GuiCmd::SetPosition(x, y) => {
                        if let Some(win) = &self.window {
                            win.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
                        }
                    }
                    GuiCmd::SetDecorations(b) => {
                        if let Some(win) = &self.window { win.set_decorations(b); }
                    }
                    GuiCmd::SetMaxSize(w, h) => {
                        if let Some(win) = &self.window {
                            let s = if w == 0 || h == 0 { None } else { Some(LogicalSize::new(w as f64, h as f64)) };
                            win.set_max_inner_size(s);
                        }
                    }
                    GuiCmd::DragWindow => {
                        if let Some(win) = &self.window { let _ = win.drag_window(); }
                    }
                    GuiCmd::SetAlwaysOnTop(b) => {
                        if let Some(win) = &self.window {
                            win.set_window_level(if b { WindowLevel::AlwaysOnTop } else { WindowLevel::Normal });
                        }
                    }
                    GuiCmd::SetMinimized(b) => {
                        if let Some(win) = &self.window { win.set_minimized(b); }
                    }
                    GuiCmd::RequestAttention(b) => {
                        if let Some(win) = &self.window {
                            win.request_user_attention(if b { Some(UserAttentionType::Informational) } else { None });
                        }
                    }
                    GuiCmd::SetCursorVisible(b) => {
                        if let Some(win) = &self.window { win.set_cursor_visible(b); }
                    }
                    GuiCmd::SetWindowIcon(rgba, w, h) => {
                        if let Some(win) = &self.window {
                            let icon = if rgba.is_empty() { None } else { Icon::from_rgba(rgba, w, h).ok() };
                            win.set_window_icon(icon);
                        }
                    }
                    GuiCmd::FileDialog { save, filter_name, filter_exts, default_name } => {
                        pending_dialog = Some((g.dialog_seq, save, filter_name, filter_exts, default_name));
                    }
                }
            }
            g.cmds = keep;
            // Hubo input → subir epoch para despertar a Gui.idleWait().
            if self.pending_input {
                g.input_epoch = g.input_epoch.wrapping_add(1);
                self.pending_input = false;
            }
            // Present pendiente → blit.
            if g.present_seq > self.last_present {
                canvas_to_blit = Some((g.canvas.clone(), g.canvas_w, g.canvas_h));
                self.last_present = g.present_seq;
                g.done_seq = g.present_seq;
                g.input = self.take_input();

                // Actualizar caché de scroll asíncrono
                g.last_presented_canvas = g.canvas.clone();
                g.last_presented_w = g.canvas_w;
                g.last_presented_h = g.canvas_h;
                g.virtual_scroll_y = 0;
            }
            // Tamaño de ventana + factor de escala (HiDPI) → compartido.
            if let Some(win) = &self.window {
                let s = win.inner_size();
                g.win_w = s.width.max(1) as usize;
                g.win_h = s.height.max(1) as usize;
                g.scale_factor = win.scale_factor();
            }
            // Cierre.
            if self.close_requested || g.interp_done {
                g.should_close = true;
                g.window_open = false;
                self.session_active = false;
            }
            host.cv.notify_all();
        }
        // Diálogo nativo (fuera del lock): bloquea hasta elegir, luego publica el resultado.
        if let Some((want, save, fname, fexts, defname)) = pending_dialog {
            let result = run_file_dialog(save, &fname, &fexts, &defname);
            let mut g = host.inner.lock().unwrap();
            g.dialog_result = result;
            g.dialog_done = want;
            host.cv.notify_all();
        }
        if let Some((canvas, cw, ch)) = canvas_to_blit {
            let (vy, bg) = {
                let g = self.host.inner.lock().unwrap();
                (g.virtual_scroll_y, g.bg_color)
            };
            self.blit(&canvas, cw, ch, vy, bg);
        }
        if !self.session_active {
            self.window = None;
            self.surface = None;
            self.context = None;
            self.close_requested = false;
        }
    }

    /// Sirve el frame pendiente + re-blit del último canvas. Para `Resized`/
    /// `RedrawRequested` durante el modal loop de resize (mantiene la ventana viva).
    fn service_and_repaint(&mut self) {
        self.service();
        let snap = {
            let g = self.host.inner.lock().unwrap();
            (g.canvas.clone(), g.canvas_w, g.canvas_h, g.virtual_scroll_y, g.bg_color)
        };
        if snap.1 > 0 && snap.2 > 0 {
            self.blit(&snap.0, snap.1, snap.2, snap.3, snap.4);
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
            focused: self.focused,
            mouse_in: self.cursor_in,
            mouse_back: self.mouse_back,
            mouse_fwd: self.mouse_fwd,
            dropped_files: std::mem::take(&mut self.dropped_files),
            ime_preedit: self.ime_preedit.clone(),
            hovered_files: self.hovered_files.clone(),   // nivel: persiste mientras hay hover
            touches: std::mem::take(&mut self.touches),  // per-frame
            pinch_delta: self.pinch_delta,               // per-frame
        };
        self.scroll_x = 0.0;
        self.scroll_y = 0.0;
        self.pinch_delta = 0.0;
        snap
    }

    fn blit(&mut self, canvas: &[u32], cw: usize, ch: usize, offset_y: i32, bg: u32) {
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
        let n = bw.min(cw);
        for y in 0..bh {
            let brow = y * bw;
            let virtual_y = y as i32 + offset_y;
            if virtual_y >= 0 && (virtual_y as usize) < ch {
                let crow = (virtual_y as usize) * cw;
                buffer[brow..brow + n].copy_from_slice(&canvas[crow..crow + n]);
                for x in n..bw {
                    buffer[brow + x] = bg;
                }
            } else {
                for x in 0..bw {
                    buffer[brow + x] = bg;
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

    // Despertador del intérprete (proxy.send_event en present()): solo necesita sacar al
    // pump de su espera larga; service() — llamado tras el pump — sirve el frame.
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: ()) {}

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // Solo el INPUT REAL (teclado/mouse/IME/modificadores) y el resize del usuario
        // marcan input pendiente → el próximo service() sube input_epoch y despierta a
        // Gui.idleWait(). RedrawRequested NO: lo dispara nuestro propio blit, y marcarlo
        // haría que idleWait se despierte solo cada frame (CPU alta en reposo).
        match event {
            WindowEvent::CloseRequested => {
                self.close_requested = true;
            }
            // Repintar/reflowar DURANTE el arrastre de redimensión. En Windows, el modal
            // move/size loop de Win32 hace que `pump_app_events` no retorne, así que
            // `service()` no se llama desde el bucle principal y el intérprete queda
            // bloqueado en `present()` → la ventana no reflowaba hasta soltar (regresión
            // vs minifb, de un solo hilo). winit SÍ despacha estos eventos durante el modal
            // loop: servirlos blitea el frame pendiente y libera el `present()`, que
            // re-renderiza al tamaño vivo. Re-blit del último canvas para no mostrar basura.
            WindowEvent::Resized(_) => {
                self.pending_input = true;   // resize del usuario → despierta idleWait
                self.service_and_repaint();  // re-blit: muestra contenido al nuevo tamaño ya
            }
            WindowEvent::RedrawRequested => {
                // Lo dispara nuestro propio blit (surface.present postea WM_PAINT). Solo
                // service(): bliteamos SÓLO si hay un frame nuevo pendiente. Un re-blit
                // incondicional aquí re-postearía WM_PAINT → tormenta de repintado (CPU alta).
                self.service();
            }
            WindowEvent::ModifiersChanged(m) => {
                self.pending_input = true;
                self.mods = m.state();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.pending_input = true;
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
            WindowEvent::Ime(ime) => {
                self.pending_input = true;
                match ime {
                    // Composición en curso (CJK): la guardamos para que serez-ui la pinte.
                    Ime::Preedit(text, _) => { self.ime_preedit = text; }
                    Ime::Commit(s) => {
                        self.ime_preedit.clear();
                        for c in s.chars() {
                            if !c.is_control() {
                                self.chars_typed.push(c);
                            }
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pending_input = true;
                self.mouse_x = position.x as i32;
                self.mouse_y = position.y as i32;
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.pending_input = true;
                let down = state == ElementState::Pressed;
                match button {
                    MouseButton::Left => self.mouse_l = down,
                    MouseButton::Right => self.mouse_r = down,
                    MouseButton::Middle => self.mouse_m = down,
                    MouseButton::Back => self.mouse_back = down,
                    MouseButton::Forward => self.mouse_fwd = down,
                    _ => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.pending_input = true;
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, dy) => dy,
                    MouseScrollDelta::PixelDelta(p) => (p.y as f32) / 12.0,
                };
                if dy != 0.0 {
                    let mut g = self.host.inner.lock().unwrap();
                    if !g.last_presented_canvas.is_empty() {
                        let delta_pixels = (-dy * 40.0) as i32;
                        g.virtual_scroll_y = (g.virtual_scroll_y + delta_pixels).clamp(
                            0,
                            (g.last_presented_h as i32 - g.win_h as i32).max(0),
                        );
                        let canvas = g.last_presented_canvas.clone();
                        let cw = g.last_presented_w;
                        let ch = g.last_presented_h;
                        let vy = g.virtual_scroll_y;
                        let bg = g.bg_color;
                        drop(g);
                        self.blit(&canvas, cw, ch, vy, bg);
                    }
                }
                match delta {
                    MouseScrollDelta::LineDelta(dx, dy) => {
                        self.scroll_x += dx;
                        self.scroll_y += dy;
                    }
                    MouseScrollDelta::PixelDelta(p) => {
                        self.scroll_x += (p.x as f32) / 12.0;
                        self.scroll_y += (p.y as f32) / 12.0;
                    }
                }
            }
            WindowEvent::Focused(b) => {
                self.pending_input = true;
                self.focused = b;
            }
            WindowEvent::CursorEntered { .. } => {
                self.pending_input = true;
                self.cursor_in = true;
            }
            WindowEvent::CursorLeft { .. } => {
                self.pending_input = true;
                self.cursor_in = false;
            }
            WindowEvent::DroppedFile(path) => {
                self.pending_input = true;
                self.hovered_files.clear();   // el drop termina el hover
                self.dropped_files.push(path.to_string_lossy().into_owned());
            }
            WindowEvent::HoveredFile(path) => {
                self.pending_input = true;
                self.hovered_files.push(path.to_string_lossy().into_owned());
            }
            WindowEvent::HoveredFileCancelled => {
                self.pending_input = true;
                self.hovered_files.clear();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.pending_input = true;   // service() relee win.scale_factor() y Resized ajusta el tamaño
            }
            WindowEvent::Touch(t) => {
                self.pending_input = true;
                let code: u8 = match t.phase {
                    winit::event::TouchPhase::Started => 0,
                    winit::event::TouchPhase::Moved => 1,
                    winit::event::TouchPhase::Ended => 2,
                    winit::event::TouchPhase::Cancelled => 3,
                };
                self.touches.push((t.id, code, t.location.x as i32, t.location.y as i32));
            }
            WindowEvent::PinchGesture { delta, .. } => {
                self.pending_input = true;
                self.pinch_delta += delta;
            }
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
                Ok(el) => {
                    // Proxy para que el intérprete despierte el pump al presentar.
                    *host.proxy.lock().unwrap() = Some(el.create_proxy());
                    event_loop = Some(el);
                }
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
            // En reposo el pump duerme hasta 200ms (CPU ~0); el intérprete lo despierta
            // al instante vía proxy en cada present(), y los eventos del SO también. Hasta
            // que exista la ventana se bombea rápido (el guard de 250 = ~1s de presupuesto).
            let timeout = if app.window.is_some() {
                Duration::from_millis(200)
            } else {
                Duration::from_millis(4)
            };
            let _ = el.pump_app_events(Some(timeout), &mut app);
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

/// Muestra un diálogo nativo de abrir/guardar archivo (rfd, bloqueante en el hilo
/// main). Devuelve la ruta elegida o None si se canceló.
fn run_file_dialog(save: bool, filter_name: &str, filter_exts: &[String], default_name: &str) -> Option<String> {
    let mut dlg = rfd::FileDialog::new();
    if !filter_exts.is_empty() {
        let exts: Vec<&str> = filter_exts.iter().map(|s| s.as_str()).collect();
        let name = if filter_name.is_empty() { "Archivos" } else { filter_name };
        dlg = dlg.add_filter(name, &exts);
    }
    if save {
        if !default_name.is_empty() { dlg = dlg.set_file_name(default_name); }
        dlg.save_file().map(|p| p.to_string_lossy().to_string())
    } else {
        dlg.pick_file().map(|p| p.to_string_lossy().to_string())
    }
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
                // Aditivo: 5 args = (x,y,text,scale,color) estilo normal (como antes);
                //          6 args = + style (0=normal, 1=bold, 2=italic, 3=bold+italic).
                let n = dot_call.arguments.len();
                if n != 5 && n != 6 {
                    eprintln!("❌ ERROR: Gui.drawText(x, y, text, scale, color [, style]) requires 5 or 6 arguments");
                    return EvalResult::Error;
                }
                let x     = self.gui_int_arg(&dot_call.arguments[0]);
                let y     = self.gui_int_arg(&dot_call.arguments[1]);
                let text  = self.gui_str_arg(&dot_call.arguments[2]);
                let scale = self.gui_int_arg(&dot_call.arguments[3]);
                let c     = self.gui_int_arg(&dot_call.arguments[4]);
                let style = if n == 6 { self.gui_int_arg(&dot_call.arguments[5]).unwrap_or(0) } else { 0 };
                let (x, y, text, scale, color) = match (x, y, text, scale, c) {
                    (Some(x), Some(y), Some(t), Some(s), Some(c)) =>
                        (x as i32, y as i32, t, s.max(1) as i32, (c as u32) & 0x00FF_FFFF),
                    _ => { eprintln!("❌ ERROR: Gui.drawText requires (int, int, string, int, int [, int])"); return EvalResult::Error; }
                };
                if self.gui_state.is_none() {
                    eprintln!("❌ ERROR: Gui.drawText: no window open");
                    return EvalResult::Error;
                }
                if self.gui_fonts.is_none() { self.gui_fonts = Some(GuiFonts::new()); }
                let fonts = self.gui_fonts.as_mut().unwrap();
                let st = self.gui_state.as_mut().unwrap();
                st.draw_text(fonts, x, y, &text, scale, color, (style.clamp(0, 3)) as u8);
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

            "focused" => {
                let v = self.gui_state.as_ref().map(|s| s.input.focused).unwrap_or(true);
                EvalResult::Value(if v { self.true_ref } else { self.false_ref })
            }
            "mouseInWindow" => {
                let v = self.gui_state.as_ref().map(|s| s.input.mouse_in).unwrap_or(false);
                EvalResult::Value(if v { self.true_ref } else { self.false_ref })
            }
            "mouseBackDown" => {
                let v = self.gui_state.as_ref().map(|s| s.input.mouse_back).unwrap_or(false);
                EvalResult::Value(if v { self.true_ref } else { self.false_ref })
            }
            "mouseForwardDown" => {
                let v = self.gui_state.as_ref().map(|s| s.input.mouse_fwd).unwrap_or(false);
                EvalResult::Value(if v { self.true_ref } else { self.false_ref })
            }
            // Archivos soltados sobre la ventana este frame (rutas). Requiere permiso File para leerlos.
            "droppedFiles" => { let v = self.gui_state.as_ref().map(|s| s.input.dropped_files.clone()).unwrap_or_default(); self.gui_str_array(v) }
            // Composición IME en curso (CJK), "" si no hay. serez-ui la dibuja en el caret.
            "imePreedit" => {
                let s = self.gui_state.as_ref().map(|s| s.input.ime_preedit.clone()).unwrap_or_default();
                EvalResult::Value(self.alloc(ObjectData::Str(s)))
            }
            // Archivos arrastrados SOBRE la ventana (antes de soltar) — para resaltar zonas de drop.
            "hoveredFiles" => { let v = self.gui_state.as_ref().map(|s| s.input.hovered_files.clone()).unwrap_or_default(); self.gui_str_array(v) }
            // Toques activos este frame, aplanado: [id, fase, x, y, ...] (fase: 0=start 1=move 2=end 3=cancel).
            "touches" => {
                let ts = self.gui_state.as_ref().map(|s| s.input.touches.clone()).unwrap_or_default();
                let mut elems: Vec<OwnedValue> = Vec::new();
                for (id, code, x, y) in ts {
                    elems.push(OwnedValue::Integer(id as i64));
                    elems.push(OwnedValue::Integer(code as i64));
                    elems.push(OwnedValue::Integer(x as i64));
                    elems.push(OwnedValue::Integer(y as i64));
                }
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: Some("int".to_string()), elements: elems }))
            }
            // Delta de pinch/zoom acumulado este frame (decimal; 0 si no hubo gesto).
            "pinchDelta" => {
                let d = self.gui_state.as_ref().map(|s| s.input.pinch_delta).unwrap_or(0.0);
                EvalResult::Value(self.alloc(ObjectData::Decimal(d)))
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
                // Aditivo: 3 args = (x,y,handle) tamaño nativo (como antes);
                //          5 args = (x,y,handle,w,h) escalado;
                //          6 args = (x,y,handle,w,h,alpha) escalado + alpha global 0–255.
                let n = dot_call.arguments.len();
                if n != 3 && n != 5 && n != 6 {
                    eprintln!("❌ ERROR: Gui.drawImage requires (x,y,handle) | (x,y,handle,w,h) | (x,y,handle,w,h,alpha)");
                    return EvalResult::Error;
                }
                let mut vals = [0i64; 6];
                vals[5] = 255; // alpha por defecto
                for i in 0..n {
                    match self.gui_int_arg(&dot_call.arguments[i]) {
                        Some(v) => vals[i] = v,
                        None => { eprintln!("❌ ERROR: Gui.drawImage requires integers"); return EvalResult::Error; }
                    }
                }
                let (x, y, hnd) = (vals[0] as i32, vals[1] as i32, vals[2]);
                match self.gui_state.as_mut() {
                    Some(st) => {
                        if n == 3 {
                            st.draw_image(x, y, hnd);
                        } else {
                            st.draw_image_scaled(x, y, hnd, vals[3] as i32, vals[4] as i32, (vals[5].clamp(0, 255)) as u32);
                        }
                        EvalResult::Value(self.null_ref)
                    }
                    None => { eprintln!("❌ ERROR: Gui.drawImage: no window open"); EvalResult::Error }
                }
            }

            "fillGradient" => {
                if dot_call.arguments.len() != 7 {
                    eprintln!("❌ ERROR: Gui.fillGradient(x, y, w, h, color1, color2, vertical) requires 7 arguments");
                    return EvalResult::Error;
                }
                let mut vals = [0i64; 6];
                for (i, slot) in vals.iter_mut().enumerate() {
                    match self.gui_int_arg(&dot_call.arguments[i]) {
                        Some(v) => *slot = v,
                        None => { eprintln!("❌ ERROR: Gui.fillGradient requires 6 integers (x,y,w,h,color1,color2) + vertical(bool)"); return EvalResult::Error; }
                    }
                }
                // `vertical` acepta bool (true=vertical) o int (≠0).
                let vertical = match self.gui_bool_arg(&dot_call.arguments[6]) {
                    Some(b) => b,
                    None => self.gui_int_arg(&dot_call.arguments[6]).map(|v| v != 0).unwrap_or(true),
                };
                let c1 = (vals[4] as u32) & 0x00FF_FFFF;
                let c2 = (vals[5] as u32) & 0x00FF_FFFF;
                match self.gui_state.as_mut() {
                    Some(st) => { st.fill_gradient(vals[0] as i32, vals[1] as i32, vals[2] as i32, vals[3] as i32, c1, c2, vertical); EvalResult::Value(self.null_ref) }
                    None => { eprintln!("❌ ERROR: Gui.fillGradient: no window open"); EvalResult::Error }
                }
            }

            "blur" => {
                if dot_call.arguments.len() != 5 {
                    eprintln!("❌ ERROR: Gui.blur(x, y, w, h, radius) requires 5 arguments");
                    return EvalResult::Error;
                }
                let mut vals = [0i64; 5];
                for (i, slot) in vals.iter_mut().enumerate() {
                    match self.gui_int_arg(&dot_call.arguments[i]) {
                        Some(v) => *slot = v,
                        None => { eprintln!("❌ ERROR: Gui.blur requires 5 integers"); return EvalResult::Error; }
                    }
                }
                match self.gui_state.as_mut() {
                    Some(st) => { st.blur_region(vals[0] as i32, vals[1] as i32, vals[2] as i32, vals[3] as i32, vals[4] as i32); EvalResult::Value(self.null_ref) }
                    None => { eprintln!("❌ ERROR: Gui.blur: no window open"); EvalResult::Error }
                }
            }

            "scaleFactor" => {
                let sf = self.gui_state.as_ref().map(|s| s.scale_factor).unwrap_or(1.0);
                EvalResult::Value(self.alloc(ObjectData::Decimal(sf)))
            }

            "textAdvances" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Gui.textAdvances(text, scale) requires 2 arguments");
                    return EvalResult::Error;
                }
                let text  = self.gui_str_arg(&dot_call.arguments[0]);
                let scale = self.gui_int_arg(&dot_call.arguments[1]);
                let (text, scale) = match (text, scale) {
                    (Some(t), Some(s)) => (t, s.max(1) as i32),
                    _ => { eprintln!("❌ ERROR: Gui.textAdvances requires (string, int)"); return EvalResult::Error; }
                };
                let xs = match self.gui_fonts.as_mut() {
                    Some(f) => f.advances(&text, scale),
                    None => {
                        let mut v = vec![0i64];
                        let mut x = 0i64;
                        for _ in text.chars() { x += 8 * scale as i64; v.push(x); }
                        v
                    }
                };
                let elements: Vec<OwnedValue> = xs.into_iter().map(OwnedValue::Integer).collect();
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: Some("int".to_string()), elements }))
            }

            "setMinSize" | "setResizable" | "setFullscreen" | "maximize" | "setPosition" | "setDecorations"
            | "setMaxSize" | "setAlwaysOnTop" | "minimize" | "requestAttention" | "setCursorVisible" => {
                let m = dot_call.method.clone();
                return self.gui_window_control(&m, dot_call);
            }
            // Mover una ventana borderless (llamar en mousedown sobre la barra custom).
            "dragWindow" => {
                if let Some(host) = host() {
                    host.inner.lock().unwrap().cmds.push_back(GuiCmd::DragWindow);
                    host.cv.notify_all();
                }
                EvalResult::Value(self.null_ref)
            }
            // Ícono de la ventana desde un archivo de imagen ("" = quitar).
            "setWindowIcon" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Gui.setWindowIcon(path) requires 1 argument");
                    return EvalResult::Error;
                }
                let path = match self.gui_str_arg(&dot_call.arguments[0]) {
                    Some(s) => s,
                    None => { eprintln!("❌ ERROR: Gui.setWindowIcon path must be a string"); return EvalResult::Error; }
                };
                let cmd = if path.is_empty() {
                    GuiCmd::SetWindowIcon(Vec::new(), 0, 0)
                } else {
                    match image::open(&path) {
                        Ok(img) => {
                            let rgba = img.to_rgba8();
                            let (w, h) = (rgba.width(), rgba.height());
                            GuiCmd::SetWindowIcon(rgba.into_raw(), w, h)
                        }
                        Err(e) => { eprintln!("❌ ERROR: Gui.setWindowIcon: {}", e); return EvalResult::Error; }
                    }
                };
                if let Some(host) = host() {
                    host.inner.lock().unwrap().cmds.push_back(cmd);
                    host.cv.notify_all();
                }
                EvalResult::Value(self.null_ref)
            }

            "openFileDialog" | "saveFileDialog" => {
                return self.gui_file_dialog(dot_call.method == "saveFileDialog", dot_call);
            }

            "idleWait" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Gui.idleWait(maxMs) requires 1 argument");
                    return EvalResult::Error;
                }
                let ms = match self.gui_int_arg(&dot_call.arguments[0]) {
                    Some(v) => v.max(0) as u64,
                    None => { eprintln!("❌ ERROR: Gui.idleWait maxMs must be an integer"); return EvalResult::Error; }
                };
                if let Some(host) = host() {
                    let g = host.inner.lock().unwrap();
                    let base = g.input_epoch;
                    let deadline = std::time::Duration::from_millis(ms);
                    let _ = host.cv.wait_timeout_while(g, deadline, |s| {
                        s.input_epoch == base && s.window_open && !s.should_close
                    });
                }
                EvalResult::Value(self.null_ref)
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

    fn gui_bool_arg(&mut self, expr: &ast::Expression) -> Option<bool> {
        match self.eval_expression(expr) {
            EvalResult::Value(v) => match self.resolve(v).cloned() {
                Some(ObjectData::Boolean(b)) => Some(b),
                _ => None,
            },
            _ => None,
        }
    }

    /// Control de ventana (setMinSize/setResizable/setFullscreen/maximize/setPosition/
    /// setDecorations): valida args y encola el GuiCmd para el hilo main.
    fn gui_window_control(&mut self, method: &str, dot_call: &ast::DotCallExpression) -> EvalResult {
        let cmd = match method {
            "setMinSize" | "setMaxSize" => {
                if dot_call.arguments.len() != 2 { eprintln!("❌ ERROR: Gui.{}(w, h) requires 2 arguments", method); return EvalResult::Error; }
                let w = self.gui_int_arg(&dot_call.arguments[0]);
                let h = self.gui_int_arg(&dot_call.arguments[1]);
                match (w, h) {
                    (Some(w), Some(h)) => {
                        let (w, h) = (w.max(0) as u32, h.max(0) as u32);
                        if method == "setMinSize" { GuiCmd::SetMinSize(w, h) } else { GuiCmd::SetMaxSize(w, h) }
                    }
                    _ => { eprintln!("❌ ERROR: Gui.{} requires 2 integers", method); return EvalResult::Error; }
                }
            }
            "setPosition" => {
                if dot_call.arguments.len() != 2 { eprintln!("❌ ERROR: Gui.setPosition(x, y) requires 2 arguments"); return EvalResult::Error; }
                let x = self.gui_int_arg(&dot_call.arguments[0]);
                let y = self.gui_int_arg(&dot_call.arguments[1]);
                match (x, y) {
                    (Some(x), Some(y)) => GuiCmd::SetPosition(x as i32, y as i32),
                    _ => { eprintln!("❌ ERROR: Gui.setPosition requires 2 integers"); return EvalResult::Error; }
                }
            }
            _ => {
                if dot_call.arguments.len() != 1 { eprintln!("❌ ERROR: Gui.{}(bool) requires 1 argument", method); return EvalResult::Error; }
                let b = match self.gui_bool_arg(&dot_call.arguments[0]) {
                    Some(b) => b,
                    None => { eprintln!("❌ ERROR: Gui.{} requires a boolean", method); return EvalResult::Error; }
                };
                match method {
                    "setResizable"     => GuiCmd::SetResizable(b),
                    "setFullscreen"    => GuiCmd::SetFullscreen(b),
                    "maximize"         => GuiCmd::SetMaximized(b),
                    "setDecorations"   => GuiCmd::SetDecorations(b),
                    "setAlwaysOnTop"   => GuiCmd::SetAlwaysOnTop(b),
                    "minimize"         => GuiCmd::SetMinimized(b),
                    "requestAttention" => GuiCmd::RequestAttention(b),
                    "setCursorVisible" => GuiCmd::SetCursorVisible(b),
                    _ => return EvalResult::Error,
                }
            }
        };
        if let Some(host) = host() {
            host.inner.lock().unwrap().cmds.push_back(cmd);
            host.cv.notify_all();
        }
        EvalResult::Value(self.null_ref)
    }

    /// Diálogo de archivo nativo. open: (filterName, extsCsv) ; save: (filterName,
    /// extsCsv, defaultName). Devuelve la ruta elegida o "" si se canceló. Bloquea
    /// (el hilo main muestra el diálogo modal) vía handshake dialog_seq/dialog_done.
    fn gui_file_dialog(&mut self, save: bool, dot_call: &ast::DotCallExpression) -> EvalResult {
        if self.gui_state.is_none() {
            eprintln!("❌ ERROR: Gui file dialog: no window open");
            return EvalResult::Error;
        }
        let n = dot_call.arguments.len();
        let filter_name  = if n >= 1 { self.gui_str_arg(&dot_call.arguments[0]).unwrap_or_default() } else { String::new() };
        let exts_csv     = if n >= 2 { self.gui_str_arg(&dot_call.arguments[1]).unwrap_or_default() } else { String::new() };
        let default_name = if save && n >= 3 { self.gui_str_arg(&dot_call.arguments[2]).unwrap_or_default() } else { String::new() };
        let filter_exts: Vec<String> = exts_csv.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        let host = match host() { Some(h) => h.clone(), None => { eprintln!("❌ ERROR: Gui file dialog: no GUI host"); return EvalResult::Error; } };
        let want = {
            let mut g = host.inner.lock().unwrap();
            g.dialog_seq += 1;
            g.dialog_result = None;
            g.cmds.push_back(GuiCmd::FileDialog { save, filter_name, filter_exts, default_name });
            g.dialog_seq
        };
        host.cv.notify_all();
        let result = {
            let mut g = host.inner.lock().unwrap();
            while g.dialog_done < want && g.window_open && !g.should_close {
                g = host.cv.wait(g).unwrap();
            }
            g.dialog_result.take()
        };
        EvalResult::Value(self.alloc(ObjectData::Str(result.unwrap_or_default())))
    }
}
