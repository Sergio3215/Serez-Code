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
use winit::window::{CursorIcon, CustomCursor, Icon, UserAttentionType, Window, WindowId, WindowLevel};

use softbuffer::{Context, Surface};
use cosmic_text::{Attrs, Buffer as TextBuffer, Color as TextColor, Family, FontSystem, Metrics, Shaping, Style as FontStyle, SwashCache, Weight};

use crate::ast::{self};
use crate::region::{ObjectData, OwnedValue};
use super::svg;
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

/// Datos de un monitor conectado (lado main → interp). Se cachean: solo se
/// recolectan al abrir la ventana y cuando cambia el factor de escala.
#[derive(Clone, Default)]
struct MonitorInfo {
    x: i32,      // posición física de la esquina sup-izquierda
    y: i32,
    w: u32,      // resolución física
    h: u32,
    scale: f64,  // factor de escala (HiDPI)
    name: String, // nombre del monitor ("" si no disponible)
}

/// Recolecta los monitores disponibles desde la ventana (hilo main).
fn collect_monitors(win: &Window) -> Vec<MonitorInfo> {
    win.available_monitors()
        .map(|m| {
            let pos = m.position();
            let size = m.size();
            MonitorInfo {
                x: pos.x,
                y: pos.y,
                w: size.width,
                h: size.height,
                scale: m.scale_factor(),
                name: m.name().unwrap_or_default(),
            }
        })
        .collect()
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
    SetCustomCursor(Vec<u8>, u32, u32, u32, u32), // rgba, w, h, hotspot_x, hotspot_y (rgba vacío = cursor por defecto)
    // Diálogo de archivo nativo (rfd) — ejecutado por el hilo main; el resultado
    // vuelve por SharedInner.dialog_result / dialog_seq (handshake como present).
    FileDialog { save: bool, filter_name: String, filter_exts: Vec<String>, default_name: String },
    // ── Multi-ventana (aditivo): ventanas EXTRA con id ≥ 1; la ventana clásica
    //    de Gui.open es la id 0 y conserva su protocolo intacto. ──
    OpenExtra { id: u32, title: String, w: u32, h: u32 },
    CloseExtra { id: u32 },
    SetTitleExtra { id: u32, title: String },
}

/// Estado compartido de UNA ventana extra (id ≥ 1). Protocolo de present
/// idéntico al de la ventana clásica, pero por entrada del mapa.
#[derive(Default)]
struct ExtraShared {
    canvas: Vec<u32>,
    canvas_w: usize,
    canvas_h: usize,
    bg_color: u32,
    present_seq: u64,
    done_seq: u64,
    window_ready: bool,
    window_open: bool,
    should_close: bool,
    open_failed: bool,
    win_w: usize,
    win_h: usize,
    input: InputSnapshot,
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
    win_x: i32,               // posición outer de la ventana (refrescada cada present)
    win_y: i32,
    monitors: Vec<MonitorInfo>, // monitores conectados (cacheado, refrescado por el main)
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
    virtual_scroll_x: i32,
    bg_color: u32,
    /// Ventanas extra (multi-ventana), por id ≥ 1.
    extra: HashMap<u32, ExtraShared>,
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
            win_x: 0,
            win_y: 0,
            monitors: Vec::new(),
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
            virtual_scroll_x: 0,
            bg_color: 0xFFFFFFFF,
            extra: HashMap::new(),
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

// ── Modo retenido (scene graph) ────────────────────────────────────────────────
// Nodos persistentes que el core redibuja en Rust: el .sz los declara una vez y
// luego solo muta propiedades (nodeSet). renderScene() redibuja el canvas SOLO
// si la escena está sucia; si no, re-presenta el frame anterior (recoge input
// sin pagar el redibujado). El ahorro grande es no re-ejecutar el árbol de
// dibujo interpretado cada frame.

enum SceneNodeKind {
    Rect { w: i32, h: i32 },
    RectAlpha { w: i32, h: i32, alpha: u32 },
    RectOutline { w: i32, h: i32 },
    RoundRect { w: i32, h: i32, radius: i32 },
    Circle { r: i32 },
    Line { x2: i32, y2: i32 },
    Polygon { points: Vec<i32> },
    Polyline { points: Vec<i32>, width: i32 },
    Text { text: String, scale: i32, font: String, style: u8, spacing: i32 },
    Image { handle: i64 },
    // Marcadores de clipping: se ejecutan en orden de dibujo (z, id), igual
    // que pushClip/popClip en modo inmediato.
    ClipPush { w: i32, h: i32 },
    ClipPop,
}

struct SceneNode {
    id: i64,
    kind: SceneNodeKind,
    x: i32,
    y: i32,
    color: u32,
    z: i32,
    visible: bool,
    // Clip por-nodo (x0,y0,x1,y1 en coords de canvas), independiente del z-order.
    // El motor de primitivos lo usa para recortar subárboles scrolleados: como los
    // fondos van a z<0 y el sort por (z,id) los separa de un ClipPush a z=0, el
    // stack no los alcanza. `None` = sin recorte propio (sigue el stack ClipPush/Pop
    // de la API manual). Ver renderScene.
    clip: Option<(i32, i32, i32, i32)>,
}

struct Glyph {
    cells: Vec<(i32, i32, u8)>,
    advance: i32,
}

// ─────────────────────────────────────────────────────────────────────────────
// Motor de primitivos (Fase 0/1): CSS nativo + layout/emit en Rust. Reemplaza el
// walk interpretado de serez-ui (renderer_gui.sz/css.sz) por UNA pasada nativa.
// Gui.loadStylesheet(src)->handle ; Gui.renderTree(root, sheet, w, h[, ctx])->#regions.
// El árbol de primitivos es un Array anidado [tag, [[prop,val]…], [hijo|texto…]].
// Ver PROPUESTA_RENDER_PRIMITIVOS_CORE.md.
// ─────────────────────────────────────────────────────────────────────────────

/// Selector simple: tag y/o `.clase` y/o `#id` (o universal `*`). Un nodo casa si
/// TODAS las partes presentes casan (Fase 2: habilita el lowering widget→div/span).
struct Selector {
    universal: bool,
    tag: Option<String>,
    class: Option<String>,
    id: Option<String>,
}

/// Una regla CSS: selector + condición opcional (var op val) + decls.
struct CssRule {
    sel: Selector,
    cond: Option<(String, String, String)>,
    decls: Vec<(String, String)>,
}

/// Hoja de estilo nativa (port de css.sz). Match por tag/clase/id/`*` + condición reactiva.
pub struct NativeStylesheet {
    rules: Vec<CssRule>,
}

fn parse_selector(s: &str) -> Selector {
    let s = s.trim();
    if s == "*" {
        return Selector { universal: true, tag: None, class: None, id: None };
    }
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut tag = None;
    let mut class = None;
    let mut id = None;
    if i < chars.len() && chars[i] != '.' && chars[i] != '#' {
        let mut t = String::new();
        while i < chars.len() && chars[i] != '.' && chars[i] != '#' { t.push(chars[i]); i += 1; }
        if !t.is_empty() { tag = Some(t); }
    }
    while i < chars.len() {
        let kind = chars[i];
        i += 1;
        let mut name = String::new();
        while i < chars.len() && chars[i] != '.' && chars[i] != '#' { name.push(chars[i]); i += 1; }
        if kind == '.' && !name.is_empty() { class = Some(name); }
        else if kind == '#' && !name.is_empty() { id = Some(name); }
    }
    Selector { universal: false, tag, class, id }
}

fn selector_matches(sel: &Selector, tag: &str, classes: &[&str], id: Option<&str>) -> bool {
    if sel.universal { return true; }
    if let Some(t) = &sel.tag { if t != tag { return false; } }
    if let Some(c) = &sel.class { if !classes.iter().any(|x| x == c) { return false; } }
    if let Some(i) = &sel.id { if id != Some(i.as_str()) { return false; } }
    sel.tag.is_some() || sel.class.is_some() || sel.id.is_some()
}

impl NativeStylesheet {
    /// Props aplicables al nodo (tag+clases+id), última gana, dado el ctx [(nombre,valor)].
    fn props_for_node(&self, tag: &str, classes: &[&str], id: Option<&str>, ctx: &[(String, String)]) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        for r in &self.rules {
            if !selector_matches(&r.sel, tag, classes, id) {
                continue;
            }
            if let Some((v, op, val)) = &r.cond {
                if !css_cond_eval(v, op, val, ctx) {
                    continue;
                }
            }
            for (p, val) in &r.decls {
                if let Some(slot) = out.iter_mut().find(|(pp, _)| pp == p) {
                    slot.1 = val.clone();
                } else {
                    out.push((p.clone(), val.clone()));
                }
            }
        }
        out
    }
}

fn css_is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '*'
}

fn css_cond_eval(var: &str, op: &str, val: &str, ctx: &[(String, String)]) -> bool {
    let lv = match ctx.iter().find(|(n, _)| n == var) {
        Some((_, v)) => v,
        None => return false,
    };
    if let (Ok(l), Ok(r)) = (lv.trim().parse::<f64>(), val.trim().parse::<f64>()) {
        return match op {
            "==" => l == r, "!=" => l != r, "<" => l < r,
            ">" => l > r, "<=" => l <= r, ">=" => l >= r, _ => false,
        };
    }
    let r = val.trim().trim_matches(|c| c == '\'' || c == '"');
    match op { "==" => lv == r, "!=" => lv != r, _ => false }
}

fn css_color(raw: &str) -> Option<u32> {
    let s = raw.trim();
    if let Some(hex) = s.strip_prefix('#') {
        let hex = if hex.len() == 3 {
            let b = hex.as_bytes();
            format!("{0}{0}{1}{1}{2}{2}", b[0] as char, b[1] as char, b[2] as char)
        } else {
            hex.to_string()
        };
        if hex.len() != 6 { return None; }
        return u32::from_str_radix(&hex, 16).ok().map(|c| c & 0x00FF_FFFF);
    }
    Some(match s {
        "white" => 0xffffff, "black" => 0x000000, "red" => 0xff0000, "green" => 0x008000,
        "blue" => 0x0000ff, "yellow" => 0xffff00, "gray" | "grey" => 0x808080,
        _ => return None,
    })
}

fn parse_cond(sraw: &str) -> (String, String, String) {
    for op in ["==", "!=", "<=", ">="] {
        if let Some(idx) = sraw.find(op) {
            return (sraw[..idx].trim().to_string(), op.to_string(), sraw[idx + 2..].trim().to_string());
        }
    }
    for op in ["<", ">"] {
        if let Some(idx) = sraw.find(op) {
            return (sraw[..idx].trim().to_string(), op.to_string(), sraw[idx + 1..].trim().to_string());
        }
    }
    (sraw.trim().to_string(), String::new(), String::new())
}

/// Parser CSS (port de parseCss): selectores tag/"*" + (cond) + { decls }. Salta
/// comentarios y bloques :import/:font.
fn parse_css(src: &str) -> NativeStylesheet {
    let s: Vec<char> = src.chars().collect();
    let n = s.len();
    let mut i = 0usize;
    let mut rules: Vec<CssRule> = Vec::new();

    fn skip(s: &[char], mut i: usize) -> usize {
        loop {
            let mut adv = false;
            while i < s.len() && (s[i] == ' ' || s[i] == '\t' || s[i] == '\n' || s[i] == '\r') { i += 1; adv = true; }
            if i + 1 < s.len() && s[i] == '/' && s[i + 1] == '*' {
                i += 2;
                while i + 1 < s.len() && !(s[i] == '*' && s[i + 1] == '/') { i += 1; }
                i = (i + 2).min(s.len());
                adv = true;
            }
            if i + 1 < s.len() && s[i] == '/' && s[i + 1] == '/' {
                i += 2;
                while i < s.len() && s[i] != '\n' { i += 1; }
                adv = true;
            }
            if !adv { break; }
        }
        i
    }

    while i < n {
        i = skip(&s, i);
        if i >= n { break; }
        if s[i] == ':' {
            let mut j = i + 1;
            while j < n && css_is_name_char(s[j]) { j += 1; }
            i = skip(&s, j);
            if i < n && s[i] == '{' {
                while i < n && s[i] != '}' { i += 1; }
                if i < n { i += 1; }
            }
            continue;
        }
        let mut sel = String::new();
        while i < n && (css_is_name_char(s[i]) || s[i] == '.' || s[i] == '#') { sel.push(s[i]); i += 1; }
        if sel.is_empty() { i += 1; continue; }
        i = skip(&s, i);
        let mut cond = None;
        if i < n && s[i] == '(' {
            i += 1;
            let mut cs = String::new();
            while i < n && s[i] != ')' { cs.push(s[i]); i += 1; }
            if i < n { i += 1; }
            cond = Some(parse_cond(&cs));
        }
        i = skip(&s, i);
        let mut decls: Vec<(String, String)> = Vec::new();
        if i < n && s[i] == '{' {
            i += 1;
            while i < n && s[i] != '}' {
                i = skip(&s, i);
                if i >= n || s[i] == '}' { break; }
                let mut prop = String::new();
                while i < n && s[i] != ':' && s[i] != '}' && s[i] != ';' { prop.push(s[i]); i += 1; }
                if i < n && s[i] == ':' {
                    i += 1;
                    let mut val = String::new();
                    while i < n && s[i] != ';' && s[i] != '}' { val.push(s[i]); i += 1; }
                    if i < n && s[i] == ';' { i += 1; }
                    let pn = prop.trim();
                    if !pn.is_empty() { decls.push((pn.to_string(), val.trim().to_string())); }
                } else if i < n && s[i] == ';' {
                    i += 1;
                } else {
                    break;
                }
            }
            if i < n && s[i] == '}' { i += 1; }
        }
        rules.push(CssRule { sel: parse_selector(&sel), cond, decls });
    }
    NativeStylesheet { rules }
}

// ── Layout + emit de primitivos (Fase 0) ──────────────────────────────────────

fn prim_is_text_tag(tag: &str) -> bool {
    matches!(tag, "p" | "span" | "label" | "b" | "i" | "strong" | "em"
        | "h1" | "h2" | "h3" | "h4" | "h5" | "h6")
}
fn prim_default_scale(tag: &str) -> i32 {
    // textbox (Input/Textarea) por defecto a 16px como el camino interpretado; el
    // resto del texto a 8px salvo títulos. La hoja puede sobreescribir con font-scale.
    match tag { "h1" => 3, "h2" => 2, "h3" => 2, "textbox" => 2, _ => 1 }
}
/// Carga una imagen raster (png/jpg/…) desde `path` y la registra en el store (con caché
/// por ruta+dims). `req_w`/`req_h` ≤ 0 = auto: se toma el tamaño natural; si solo uno es
/// >0, el otro se deriva por aspecto. Devuelve (handle, w_usado, h_usado), o None si la
/// ruta no existe o no decodifica. Reusa la infra de `loadImageBytes` (crate `image`).
fn prim_load_raster(st: &mut GuiState, path: &str, req_w: i32, req_h: i32) -> Option<(i64, i32, i32)> {
    let bytes = std::fs::read(path).ok()?;
    let decoded = image::load_from_memory(&bytes).ok()?;
    let (nw, nh) = (decoded.width() as i32, decoded.height() as i32);
    if nw <= 0 || nh <= 0 { return None; }
    // Dimensiones objetivo: explícitas, derivadas por aspecto, o naturales.
    let (tw, th) = match (req_w > 0, req_h > 0) {
        (true, true)   => (req_w, req_h),
        (true, false)  => (req_w, (req_w * nh / nw).max(1)),
        (false, true)  => ((req_h * nw / nh).max(1), req_h),
        (false, false) => (nw, nh),
    };
    let key = (path.to_string(), tw, th);
    if let Some(&ih) = st.raster_cache.get(&key) { return Some((ih, tw, th)); }
    let scaled = if tw == nw && th == nh {
        decoded.to_rgba8()
    } else {
        decoded.resize_exact(tw as u32, th as u32, image::imageops::FilterType::Triangle).to_rgba8()
    };
    let mut px = Vec::with_capacity((tw * th) as usize);
    for p in scaled.pixels() {
        let [r, g, b, a] = p.0;
        px.push(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32);
    }
    let ih = st.next_image;
    st.next_image += 1;
    st.images.insert(ih, ImageData { w: tw as usize, h: th as usize, px });
    st.raster_cache.insert(key, ih);
    Some((ih, tw, th))
}

fn prim_eff_style(sheet: Option<&NativeStylesheet>, ctx: &[(String, String)], tag: &str,
    classes: &[&str], id: Option<&str>, inline: &[OwnedValue]) -> Vec<(String, String)> {
    let mut out = sheet.map(|s| s.props_for_node(tag, classes, id, ctx)).unwrap_or_default();
    for pair in inline {
        if let OwnedValue::Array { elements, .. } = pair {
            if elements.len() >= 2 {
                if let OwnedValue::Str(p) = &elements[0] {
                    // class/id/onClick no son props de estilo; el resto sí (inline override).
                    if p == "class" || p == "id" || p == "onClick" { continue; }
                    let vs = match &elements[1] {
                        OwnedValue::Str(s) => s.clone(),
                        OwnedValue::Integer(k) => k.to_string(),
                        _ => String::new(),
                    };
                    if let Some(slot) = out.iter_mut().find(|(pp, _)| pp == p) { slot.1 = vs; }
                    else { out.push((p.clone(), vs)); }
                }
            }
        }
    }
    out
}

/// Valor string de un atributo del nodo (class/id/…), o None.
fn prim_attr(inline: &[OwnedValue], name: &str) -> Option<String> {
    for pair in inline {
        if let OwnedValue::Array { elements, .. } = pair {
            if elements.len() >= 2 {
                if let OwnedValue::Str(p) = &elements[0] {
                    if p == name {
                        return match &elements[1] {
                            OwnedValue::Str(s) => Some(s.clone()),
                            OwnedValue::Integer(k) => Some(k.to_string()),
                            _ => None,
                        };
                    }
                }
            }
        }
    }
    None
}

/// El valor de `onClick` del nodo (típicamente una función), clonado, o None.
fn prim_find_onclick(inline: &[OwnedValue]) -> Option<OwnedValue> {
    for pair in inline {
        if let OwnedValue::Array { elements, .. } = pair {
            if elements.len() >= 2 {
                if let OwnedValue::Str(p) = &elements[0] {
                    if p == "onClick" { return Some(elements[1].clone()); }
                }
            }
        }
    }
    None
}

/// Región de hit-testing devuelta a `.sz`: caja + onClick (para enrutar clicks).
struct PrimRegion {
    tag: String,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    onclick: Option<OwnedValue>,
}
fn sget<'a>(st: &'a [(String, String)], name: &str) -> Option<&'a str> {
    st.iter().find(|(p, _)| p == name).map(|(_, v)| v.as_str())
}
fn snum(st: &[(String, String)], name: &str, d: i32) -> i32 {
    // Acepta el sufijo `px` (`gap: 10px`, `border-radius: 4px`) y lo ignora,
    // igual que prim_dim y prim_box_sides: todo el motor trabaja en px enteros.
    sget(st, name)
        .and_then(|v| v.trim().trim_end_matches("px").trim().parse::<i32>().ok())
        .unwrap_or(d)
}
/// Shorthand CSS de 1–4 valores (`padding: 8 14`) → (top, right, bottom, left),
/// como en la web: 1=todos, 2=vert/horiz, 3=top/horiz/bottom, 4=t r b l.
/// Los props por-lado (`padding-top`, …) pisan al shorthand.
fn prim_box_sides(st: &[(String, String)], name: &str) -> (i32, i32, i32, i32) {
    let (mut t, mut r, mut b, mut l) = (0, 0, 0, 0);
    if let Some(v) = sget(st, name) {
        let n: Vec<i32> = v.split_whitespace()
            .filter_map(|x| x.trim_end_matches("px").parse::<i32>().ok())
            .collect();
        match n.len() {
            1 => { t = n[0]; r = n[0]; b = n[0]; l = n[0]; }
            2 => { t = n[0]; b = n[0]; r = n[1]; l = n[1]; }
            3 => { t = n[0]; r = n[1]; l = n[1]; b = n[2]; }
            _ if n.len() >= 4 => { t = n[0]; r = n[1]; b = n[2]; l = n[3]; }
            _ => {}
        }
    }
    (snum(st, &format!("{}-top", name), t),
     snum(st, &format!("{}-right", name), r),
     snum(st, &format!("{}-bottom", name), b),
     snum(st, &format!("{}-left", name), l))
}
fn scol(st: &[(String, String)], name: &str) -> Option<u32> {
    sget(st, name).and_then(css_color)
}

fn prim_push_rect(st: &mut GuiState, x: i32, y: i32, w: i32, h: i32, color: u32, z: i32) {
    if w <= 0 || h <= 0 { return; }
    let id = st.next_node;
    st.next_node += 1;
    let clip = st.prim_clip;
    st.scene.push(SceneNode { id, kind: SceneNodeKind::Rect { w, h }, x, y, color, z, visible: true, clip });
}
fn prim_push_roundrect(st: &mut GuiState, x: i32, y: i32, w: i32, h: i32, radius: i32, color: u32, z: i32) {
    if w <= 0 || h <= 0 { return; }
    let id = st.next_node;
    st.next_node += 1;
    let clip = st.prim_clip;
    st.scene.push(SceneNode { id, kind: SceneNodeKind::RoundRect { w, h, radius }, x, y, color, z, visible: true, clip });
}
/// Rect (redondeado si radius>0). Helper interno sin borde.
fn prim_push_fill(st: &mut GuiState, x: i32, y: i32, w: i32, h: i32, color: u32, z: i32, radius: i32) {
    if radius > 0 { prim_push_roundrect(st, x, y, w, h, radius, color, z); }
    else { prim_push_rect(st, x, y, w, h, color, z); }
}
/// Fondo de un box: `border-radius` + `border-width`/`border-color`. El borde se pinta
/// con la técnica de inset (color de borde al tamaño completo, relleno adentro), que
/// funciona con las primitivas existentes y respeta esquinas redondeadas.
fn prim_push_bg(st: &mut GuiState, x: i32, y: i32, w: i32, h: i32, color: u32, z: i32, radius: i32, border_w: i32, border_col: Option<u32>) {
    if border_w > 0 {
        if let Some(bc) = border_col {
            prim_push_fill(st, x, y, w, h, bc, z, radius);
            let ir = (radius - border_w).max(0);
            prim_push_fill(st, x + border_w, y + border_w, w - 2 * border_w, h - 2 * border_w, color, z, ir);
            return;
        }
    }
    prim_push_fill(st, x, y, w, h, color, z, radius);
}
fn prim_push_text(st: &mut GuiState, x: i32, y: i32, text: &str, scale: i32, color: u32, style: u8, font: &str, z: i32, spacing: i32) {
    if text.is_empty() { return; }
    let id = st.next_node;
    st.next_node += 1;
    let clip = st.prim_clip;
    st.scene.push(SceneNode {
        id, kind: SceneNodeKind::Text { text: text.to_string(), scale, font: font.to_string(), style, spacing },
        x, y, color, z, visible: true, clip,
    });
}
/// Emite un nodo de imagen (reusado para SVG rasterizado): blit con alpha + clip.
fn prim_push_image(st: &mut GuiState, x: i32, y: i32, handle: i64, z: i32) {
    let id = st.next_node;
    st.next_node += 1;
    let clip = st.prim_clip;
    st.scene.push(SceneNode {
        id, kind: SceneNodeKind::Image { handle }, x, y, color: 0, z, visible: true, clip,
    });
}
fn prim_push_circle(st: &mut GuiState, cx: i32, cy: i32, r: i32, color: u32, z: i32) {
    if r <= 0 { return; }
    let id = st.next_node;
    st.next_node += 1;
    let clip = st.prim_clip;
    st.scene.push(SceneNode { id, kind: SceneNodeKind::Circle { r }, x: cx, y: cy, color, z, visible: true, clip });
}
fn prim_push_line(st: &mut GuiState, x1: i32, y1: i32, x2: i32, y2: i32, color: u32, z: i32) {
    let id = st.next_node;
    st.next_node += 1;
    let clip = st.prim_clip;
    st.scene.push(SceneNode { id, kind: SceneNodeKind::Line { x2, y2 }, x: x1, y: y1, color, z, visible: true, clip });
}
fn prim_push_poly(st: &mut GuiState, pts: Vec<i32>, color: u32, z: i32, filled: bool, width: i32) {
    let id = st.next_node;
    st.next_node += 1;
    let clip = st.prim_clip;
    let kind = if filled { SceneNodeKind::Polygon { points: pts } } else { SceneNodeKind::Polyline { points: pts, width: width.max(1) } };
    st.scene.push(SceneNode { id, kind, x: 0, y: 0, color, z, visible: true, clip });
}
/// Parsea "x1,y1 x2,y2 …" (o separado por espacios/comas) desplazado por (ox,oy).
fn prim_parse_points(s: &str, ox: i32, oy: i32) -> Vec<i32> {
    let nums: Vec<i32> = s.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .filter_map(|t| t.trim().parse::<f32>().ok().map(|f| f as i32))
        .collect();
    let mut out = Vec::with_capacity(nums.len());
    let mut i = 0;
    while i + 1 < nums.len() {
        out.push(nums[i] + ox);
        out.push(nums[i + 1] + oy);
        i += 2;
    }
    out
}

/// Interseca (x0,y0,x1,y1) con el clip de primitivos activo (si hay). Todo en coords
/// de canvas. Devuelve un rect no-vacío-normalizado (x1≥x0, y1≥y0).
fn prim_clip_intersect(prev: Option<(i32, i32, i32, i32)>, x0: i32, y0: i32, x1: i32, y1: i32) -> (i32, i32, i32, i32) {
    match prev {
        Some((ax0, ay0, ax1, ay1)) => {
            let nx0 = x0.max(ax0);
            let ny0 = y0.max(ay0);
            (nx0, ny0, x1.min(ax1).max(nx0), y1.min(ay1).max(ny0))
        }
        None => (x0, y0, x1, y1),
    }
}

/// Ancho en px de un texto para el layout de primitivos: proporcional si hay una
/// familia de fuente custom activa; rejilla monospace 8*scale si no hay fuentes o
/// es la familia default (mantiene compat con serez-ui sin `setFont`).
fn prim_text_px(fonts: &mut Option<GuiFonts>, s: &str, scale: i32, style: u8) -> i32 {
    match fonts.as_mut() {
        Some(f) => f.text_width(s, scale, style) as i32,
        None => s.chars().filter(|c| !c.is_control()).count() as i32 * (8 * scale.max(1)),
    }
}
/// Ancho en px de los primeros `n` caracteres de `text` con la familia `font` (para
/// posicionar caret/selección de forma consistente con el dibujado).
fn prim_prefix_px(fonts: &mut Option<GuiFonts>, text: &str, n: usize, scale: i32, style: u8, font: &str) -> i32 {
    let prefix: String = text.chars().take(n).collect();
    let prev = if !font.is_empty() { fonts.as_mut().map(|f| { let p = f.current; f.set_family(font); p }) } else { None };
    let w = prim_text_px(fonts, &prefix, scale, style);
    if let (Some(p), Some(f)) = (prev, fonts.as_mut()) { f.current = p; }
    w
}
fn prim_char_px(fonts: &mut Option<GuiFonts>, ch: char, scale: i32, style: u8) -> i32 {
    match fonts.as_mut() {
        Some(f) => f.char_width(ch, scale, style) as i32,
        None => if ch.is_control() { 0 } else { 8 * scale.max(1) },
    }
}

/// Word-wrap de `text` (respeta '\n') en ≤ `cap` líneas de ancho `avail`. Anchos
/// proporcionales si hay familia custom (si no, rejilla). Rompe por carácter las
/// palabras más anchas que `avail`. Corta temprano al llegar a `cap` (virtualización:
/// un textbox de 10 KB con 6 filas visibles no maqueta las 10 KB, solo 6 líneas).
fn prim_wrap_lines(fonts: &mut Option<GuiFonts>, text: &str, avail: i32, scale: i32, style: u8, cap: i32) -> Vec<String> {
    let avail = avail.max(1);
    let space_w = prim_text_px(fonts, " ", scale, style).max(1);
    let mut lines: Vec<String> = Vec::new();
    for para in text.split('\n') {
        if lines.len() as i32 >= cap { return lines; }
        if para.is_empty() { lines.push(String::new()); continue; }
        let mut cur = String::new();
        let mut cur_w = 0i32;
        for word in para.split(' ') {
            if lines.len() as i32 >= cap { return lines; }
            let ww = prim_text_px(fonts, word, scale, style);
            // Si la palabra no cabe en lo que queda de la línea actual, cierra la línea.
            if !cur.is_empty() && cur_w + space_w + ww > avail {
                lines.push(std::mem::take(&mut cur));
                cur_w = 0;
            }
            if cur.is_empty() && ww > avail {
                // Palabra sola más ancha que la línea: romper por carácter.
                let mut chunk = String::new();
                let mut chunk_w = 0i32;
                for ch in word.chars() {
                    let cw = prim_char_px(fonts, ch, scale, style);
                    if !chunk.is_empty() && chunk_w + cw > avail {
                        lines.push(std::mem::take(&mut chunk));
                        chunk_w = 0;
                        if lines.len() as i32 >= cap { return lines; }
                    }
                    chunk.push(ch);
                    chunk_w += cw;
                }
                cur = chunk;
                cur_w = chunk_w;
            } else {
                if !cur.is_empty() { cur.push(' '); cur_w += space_w; }
                cur.push_str(word);
                cur_w += ww;
            }
        }
        lines.push(cur);
    }
    if lines.is_empty() { lines.push(String::new()); }
    lines
}

/// Ajusta (word-wrap) y emite texto en `avail_w`, hasta `cap` líneas (virtualización).
/// Devuelve el alto consumido. `style`: bit0=bold, bit1=italic. Mide proporcional si
/// hay una familia de fuente custom activa (si no, rejilla monospace 8*scale).
fn prim_emit_text(st: &mut GuiState, fonts: &mut Option<GuiFonts>, text: &str, x: i32, y: i32, avail_w: i32, scale: i32, line_h: i32, color: u32, style: u8, cap: i32, font: &str, z: i32, align: u8, spacing: i32) -> i32 {
    // Familia por nodo: mide+dibuja con `font` (restaura la familia previa al salir).
    let prev = if !font.is_empty() {
        fonts.as_mut().map(|f| { let p = f.current; f.set_family(font); p })
    } else { None };
    let lines = prim_wrap_lines(fonts, text, avail_w, scale, style, cap);
    let n = (lines.len() as i32).min(cap).max(1);
    let mut line = 0i32;
    for seg in lines.iter() {
        if line >= cap { break; }
        if !seg.is_empty() {
            // text-align: 1=center, 2=right → desplaza la línea dentro de `avail_w`.
            let lx = if align == 0 { x } else {
                let cc = seg.chars().count() as i32;
                let lw = prim_text_px(fonts, seg, scale, style) + (cc - 1).max(0) * spacing;
                let free = (avail_w - lw).max(0);
                if align == 1 { x + free / 2 } else { x + free }
            };
            prim_push_text(st, lx, y + line * line_h, seg, scale, color, style, font, z, spacing);
        }
        line += 1;
    }
    if let (Some(p), Some(f)) = (prev, fonts.as_mut()) { f.current = p; }
    n * line_h
}

/// Bits de estilo de texto (bold/italic) desde el tag y las props.
fn prim_text_style(tag: &str, style: &[(String, String)]) -> u8 {
    let mut s = 0u8;
    // bold: tag, font-weight:bold|bolder, o numérico >=600 (como CSS).
    let fw = sget(style, "font-weight");
    let fw_bold = fw == Some("bold") || fw == Some("bolder")
        || fw.and_then(|v| v.trim().parse::<i32>().ok()).map(|n| n >= 600).unwrap_or(false);
    if tag == "b" || tag == "strong" || fw_bold { s |= 1; }
    if tag == "i" || tag == "em" || sget(style, "font-style") == Some("italic") { s |= 2; }
    // text-decoration: underline (bit2) / line-through (bit3). Admite ambos.
    if let Some(td) = sget(style, "text-decoration") {
        if td.contains("underline") { s |= 0b100; }
        if td.contains("line-through") { s |= 0b1000; }
    }
    s
}

/// Extrae (tag, inline-style, children) de un nodo OwnedValue [tag, [..], [..]].
fn prim_node_parts(o: &OwnedValue) -> Option<(&str, &[OwnedValue], &[OwnedValue])> {
    if let OwnedValue::Array { elements, .. } = o {
        if elements.len() >= 3 {
            if let OwnedValue::Str(tag) = &elements[0] {
                let style = if let OwnedValue::Array { elements, .. } = &elements[1] { elements.as_slice() } else { &[] };
                let kids = if let OwnedValue::Array { elements, .. } = &elements[2] { elements.as_slice() } else { &[] };
                return Some((tag.as_str(), style, kids));
            }
        }
    }
    None
}

/// Reparte `free` px libres en un eje flex de `n` items según justify-content. Devuelve
/// (offset_inicial, separación_extra_entre_items); el `gap` de CSS se suma aparte. Solo
/// aplica cuando hay espacio libre (ningún hijo crece).
fn prim_justify(mode: &str, free: i32, n: i32) -> (i32, i32) {
    if free <= 0 || n <= 0 { return (0, 0); }
    match mode {
        "flex-end" | "end" => (free, 0),
        "center" => (free / 2, 0),
        "space-between" => if n > 1 { (0, free / (n - 1)) } else { (0, 0) },
        "space-around" => { let unit = free / (2 * n); (unit, unit * 2) }
        "space-evenly" => { let unit = free / (n + 1); (unit, unit) }
        _ => (0, 0), // flex-start / start / default
    }
}

/// Dimensión CSS: px (con o sin sufijo "px"), `N%` de `base`, o `auto`/ausente = -1.
fn prim_dim(st: &[(String, String)], name: &str, base: i32) -> i32 {
    match sget(st, name) {
        Some(v) => {
            let v = v.trim();
            if v.eq_ignore_ascii_case("auto") { return -1; }
            if let Some(p) = v.strip_suffix('%') {
                return p.trim().parse::<f32>().ok().map(|pc| (base as f32 * pc / 100.0) as i32).unwrap_or(-1);
            }
            v.trim_end_matches("px").trim().parse::<i32>().ok().unwrap_or(-1)
        }
        None => -1,
    }
}
/// `border: 1px solid #333` → (ancho, color). Ignora el estilo (solid/…).
fn prim_border_shorthand(s: Option<&str>) -> (i32, Option<u32>) {
    match s {
        Some(v) => {
            let mut w = 0;
            let mut c = None;
            for tok in v.split_whitespace() {
                if let Ok(n) = tok.trim_end_matches("px").parse::<i32>() { w = n; }
                else if let Some(col) = css_color(tok) { c = Some(col); }
            }
            (w, c)
        }
        None => (0, None),
    }
}

// ═══ Recorrido del árbol de primitivos ════════════════════════════════════════
//
// `Gui.renderTree` recorre el árbol [tag, [[prop,val]…], [hijos…]] en UNA pasada:
// cada nodo resuelve su estilo, calcula su caja, emite sus nodos de escena y
// devuelve el alto que consumió en el flujo. No hay segunda pasada de layout
// (por eso align-items se corrige con un post-pase sobre lo ya emitido).
//
// Mapa del código (para tocar una pieza sin releer el resto):
//   PrimCtx        — referencias compartidas del recorrido (escena, fuentes, CSS, regions)
//   PrimFrame      — lo que el padre le da al hijo: posición, ancho disponible,
//                    profundidad/z y el containing block (para absolute)
//   PrimStyle      — estilo efectivo del nodo ya resuelto a valores tipados
//   PrimBox        — la caja final del nodo (posición, ancho, interior, z)
//   prim_render    — UN nodo: caja + region + dispatch por tag
//   prim_draw_*    — hojas: texto, hr, formas vectoriales, textbox, img/svg
//   prim_layout_*  — contenedores: fila flex, caja con scroll, bloque vertical
//   prim_flex_plan — reparto de anchos de la fila flex (crece/fijo/contenido)

/// Referencias compartidas por todo el recorrido. Agrupadas para no arrastrar
/// seis parámetros por cada llamada recursiva.
struct PrimCtx<'a> {
    sheet: Option<&'a NativeStylesheet>,
    svgs: &'a [svg::ParsedSvg],
    ctx: &'a [(String, String)],
    fonts: &'a mut Option<GuiFonts>,
    st: &'a mut GuiState,
    regions: &'a mut Vec<PrimRegion>,
}

/// Lo que el PADRE le entrega a un nodo: dónde ubicarlo, cuánto ancho tiene,
/// a qué profundidad/banda z está, y el containing block del ancestro
/// posicionado más cercano (destino de los hijos `position:absolute`).
#[derive(Clone, Copy)]
struct PrimFrame {
    x: i32,
    y: i32,
    avail_w: i32,
    depth: i32, // profundidad en el árbol: ordena fondos (ancestro detrás de hijo)
    z_off: i32, // banda de overlay acumulada por los z-index de los ancestros
    cb_x: i32,  // containing block (ancestro posicionado): origen…
    cb_y: i32,
    cb_w: i32,  // …y dimensiones (-1 = alto desconocido, layout de una pasada)
    cb_h: i32,
}

/// La caja YA resuelta de un nodo: posición final, ancho, interior (sin padding)
/// y las z que le tocan a su contenido y a su fondo.
#[derive(Clone, Copy)]
struct PrimBox {
    x: i32,
    y: i32,
    w: i32,
    content_x: i32, // x + padding-left
    content_w: i32, // w - padding horizontal
    z: i32,         // z del contenido (texto/formas)
    bg_z: i32,      // z de los fondos: banda + profundidad - 100 (siempre detrás)
}

/// Estilo efectivo del nodo resuelto a valores tipados. `props` conserva el
/// prop-list completo (hoja + inline) para los atributos que solo usa un tag
/// concreto (rows/caret/src/points/…) vía `get`/`num`/`col`.
struct PrimStyle {
    props: Vec<(String, String)>,
    scale: i32,                        // font-scale (glifo base = 8*scale px)
    text_col: u32,
    bg: Option<u32>,
    border_w: i32,
    border_col: Option<u32>,
    radius: i32,
    gap: i32,
    pad: (i32, i32, i32, i32),         // top, right, bottom, left
    mar: (i32, i32, i32, i32),
    font_fam: String,                  // "" = familia activa (Gui.setFont)
    talign: u8,                        // 0 izq, 1 centro, 2 der
    tspacing: i32,                     // letter-spacing
    tstyle: u8,                        // bits: bold/italic/subrayado/tachado
    ex_w: i32,                         // width propio (px resueltos) o -1 = auto
    hgt: i32,                          // height propio o -1 = auto
    absolute: bool,
    relative: bool,
}

impl PrimStyle {
    /// Resuelve el estilo EFECTIVO: reglas de la hoja que matchean el nodo
    /// (tag/.clase/#id + condición reactiva, "última gana") pisadas por el
    /// estilo inline, y de ahí los valores tipados que consume el layout.
    /// `width` se resuelve contra el ancho disponible; `height` contra la
    /// ventana (gap conocido: % de height vs padre no está soportado).
    fn resolve(tag: &str, style_inline: &[OwnedValue], avail_w: i32, win_h: i32,
               sheet: Option<&NativeStylesheet>, ctx: &[(String, String)]) -> PrimStyle {
        let class_str = prim_attr(style_inline, "class").unwrap_or_default();
        let classes: Vec<&str> = class_str.split_whitespace().collect();
        let id_str = prim_attr(style_inline, "id");
        let props = prim_eff_style(sheet, ctx, tag, &classes, id_str.as_deref(), style_inline);

        // Borde: props sueltas o shorthand `border: 1px solid #333`.
        let (bsh_w, bsh_c) = prim_border_shorthand(sget(&props, "border"));
        let pos = sget(&props, "position");
        let absolute = pos == Some("absolute");
        let relative = pos == Some("relative");
        PrimStyle {
            scale: snum(&props, "font-scale", prim_default_scale(tag)),
            text_col: scol(&props, "color").unwrap_or(0xffffff),
            bg: scol(&props, "background-color").or_else(|| scol(&props, "background")),
            border_w: snum(&props, "border-width", bsh_w),
            border_col: scol(&props, "border-color").or(bsh_c),
            radius: snum(&props, "border-radius", 0),
            gap: snum(&props, "gap", 0),
            pad: prim_box_sides(&props, "padding"),
            mar: prim_box_sides(&props, "margin"),
            font_fam: sget(&props, "font-family")
                .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                .unwrap_or_default(),
            talign: match sget(&props, "text-align") {
                Some("center") => 1,
                Some("right") | Some("end") => 2,
                _ => 0,
            },
            tspacing: snum(&props, "letter-spacing", 0),
            tstyle: prim_text_style(tag, &props),
            ex_w: prim_dim(&props, "width", avail_w),
            hgt: prim_dim(&props, "height", win_h),
            absolute,
            relative,
            props,
        }
    }

    fn get(&self, name: &str) -> Option<&str> { sget(&self.props, name) }
    fn num(&self, name: &str, d: i32) -> i32 { snum(&self.props, name, d) }
    fn col(&self, name: &str) -> Option<u32> { scol(&self.props, name) }
}

/// Concatena los hijos string directos: el texto plano del nodo.
fn prim_join_text(kids: &[OwnedValue]) -> String {
    let mut t = String::new();
    for k in kids {
        if let OwnedValue::Str(s) = k { t.push_str(s); }
    }
    t
}

/// Posición de un nodo `absolute` contra su containing block: `left`/`top`
/// mandan; sin ellos, `right`/`bottom` posicionan desde el borde opuesto
/// (necesitan width/height explícitos: el layout es de una pasada y el tamaño
/// auto no se conoce todavía). Sin nada: el origen del containing block.
fn prim_abs_pos(sty: &PrimStyle, f: &PrimFrame, win_h: i32) -> (i32, i32) {
    let x = if sty.get("left").is_some() {
        f.cb_x + prim_dim(&sty.props, "left", f.avail_w).max(0)
    } else if sty.get("right").is_some() && sty.ex_w >= 0 && f.cb_w >= 0 {
        f.cb_x + f.cb_w - prim_dim(&sty.props, "right", f.cb_w).max(0) - sty.ex_w
    } else {
        f.cb_x
    };
    let y = if sty.get("top").is_some() {
        f.cb_y + prim_dim(&sty.props, "top", win_h).max(0)
    } else if sty.get("bottom").is_some() && sty.hgt >= 0 && f.cb_h >= 0 {
        f.cb_y + f.cb_h - prim_dim(&sty.props, "bottom", f.cb_h).max(0) - sty.hgt
    } else {
        f.cb_y
    };
    (x, y)
}

/// Desplazamiento visual de `position:relative` (como en la web): `left`/`top`
/// corren el DIBUJO del nodo; el flujo de los hermanos no se entera. `right`/
/// `bottom` equivalen al lado opuesto en negativo.
fn prim_rel_offset(sty: &PrimStyle, avail_w: i32, win_h: i32) -> (i32, i32) {
    let dx = if sty.get("left").is_some() {
        prim_dim(&sty.props, "left", avail_w)
    } else if sty.get("right").is_some() {
        -prim_dim(&sty.props, "right", avail_w)
    } else { 0 };
    let dy = if sty.get("top").is_some() {
        prim_dim(&sty.props, "top", win_h)
    } else if sty.get("bottom").is_some() {
        -prim_dim(&sty.props, "bottom", win_h)
    } else { 0 };
    (dx, dy)
}

/// Layout + emit de UN nodo del árbol. Devuelve el alto que consumió en el
/// flujo vertical (márgenes incluidos); `position:absolute` devuelve 0 (fuera
/// de flujo). Pasos: estilo → caja → region (pre-orden) → dispatch por tag.
fn prim_render(tag: &str, style_inline: &[OwnedValue], kids: &[OwnedValue], f: PrimFrame, cx: &mut PrimCtx) -> i32 {
    let win_h = cx.st.win_h as i32;
    let sty = PrimStyle::resolve(tag, style_inline, f.avail_w, win_h, cx.sheet, cx.ctx);
    // display:none → no se dibuja ni ocupa flujo (como en la web).
    if sty.get("display") == Some("none") { return 0; }
    let (mt, mr, mb, ml) = sty.mar;

    // ── 1. Posición de la caja ──────────────────────────────────────────────
    // absolute → contra el containing block; relative → corrimiento visual;
    // normal → donde lo puso el padre, corrido por el margen.
    let (x, y) = if sty.absolute {
        prim_abs_pos(&sty, &f, win_h)
    } else if sty.relative {
        let (dx, dy) = prim_rel_offset(&sty, f.avail_w, win_h);
        (f.x + ml + dx, f.y + mt + dy)
    } else {
        (f.x + ml, f.y + mt)
    };
    // width explícito = caja compacta (acotada al disponible, salvo absolute);
    // sin width = todo el disponible menos márgenes.
    let box_w = if sty.ex_w >= 0 {
        if sty.absolute { sty.ex_w } else { sty.ex_w.min(f.avail_w) }
    } else {
        (f.avail_w - ml - mr).max(1)
    };

    // ── 2. Caja resuelta + z ────────────────────────────────────────────────
    // z-index abre una BANDA de overlay para todo el subárbol; los fondos van
    // debajo del contenido y los ancestros detrás de los descendientes.
    let z_off = f.z_off + sty.num("z-index", 0);
    let b = PrimBox {
        x,
        y,
        w: box_w,
        content_x: x + sty.pad.3,
        content_w: (box_w - sty.pad.3 - sty.pad.1).max(1),
        z: z_off,
        bg_z: z_off + f.depth - 100,
    };
    // Marco de los hijos: si este nodo está posicionado, pasa a ser el
    // containing block de sus descendientes absolute.
    let positioned = sty.absolute || sty.relative;
    let kf = PrimFrame {
        x: b.content_x,
        y: b.y + sty.pad.0,
        avail_w: b.content_w,
        depth: f.depth + 1,
        z_off,
        cb_x: if positioned { x } else { f.cb_x },
        cb_y: if positioned { y } else { f.cb_y },
        cb_w: if positioned { box_w } else { f.cb_w },
        cb_h: if positioned { sty.hgt } else { f.cb_h },
    };

    // ── 3. Region en PRE-ORDEN ──────────────────────────────────────────────
    // (padre antes que hijos = el orden en que un framework recorre su árbol);
    // el alto se completa al final, cuando ya se midió el contenido.
    let region_idx = cx.regions.len();
    cx.regions.push(PrimRegion {
        tag: tag.to_string(),
        x, y, w: box_w, h: 0,
        onclick: prim_find_onclick(style_inline),
    });

    // ── 4. Dispatch por tag ─────────────────────────────────────────────────
    let total_h = if prim_is_text_tag(tag) {
        prim_draw_text_tag(kids, &sty, b, cx)
    } else if tag == "hr" {
        prim_draw_hr(&sty, b, cx)
    } else if tag == "circle" || tag == "line" || tag == "polyline" || tag == "polygon" {
        prim_draw_shape(tag, &sty, b, cx)
    } else if tag == "textbox" {
        prim_draw_textbox(kids, &sty, b, cx)
    } else if tag == "svg" || tag == "img" {
        prim_draw_media(&sty, b, cx)
    } else {
        prim_layout_container(tag, kids, &sty, b, kf, cx)
    };

    cx.regions[region_idx].h = total_h;
    if sty.absolute { 0 } else { mt + total_h + mb }
}

// ── Hojas (nodos que dibujan contenido propio) ─────────────────────────────────

/// Texto (p/span/h1…/label): word-wrap dentro de la caja + fondo opcional.
fn prim_draw_text_tag(kids: &[OwnedValue], sty: &PrimStyle, b: PrimBox, cx: &mut PrimCtx) -> i32 {
    let text = prim_join_text(kids);
    let line_h = sty.num("line-height", 8 * sty.scale + 4);
    let h = prim_emit_text(cx.st, cx.fonts, &text, b.content_x, b.y + sty.pad.0, b.content_w,
        sty.scale, line_h, sty.text_col, sty.tstyle, i32::MAX, sty.font_fam.as_str(), b.z, sty.talign, sty.tspacing);
    let total = if sty.hgt >= 0 { sty.hgt } else { h + sty.pad.0 + sty.pad.2 };
    if let Some(c) = sty.bg {
        prim_push_bg(cx.st, b.x, b.y, b.w, total, c, b.bg_z, sty.radius, sty.border_w, sty.border_col);
    }
    total
}

/// Separador horizontal: línea de 2px del ancho de la caja.
fn prim_draw_hr(sty: &PrimStyle, b: PrimBox, cx: &mut PrimCtx) -> i32 {
    let col = sty.col("color").unwrap_or(0x334155);
    prim_push_rect(cx.st, b.x, b.y + sty.pad.0, b.w, 2, col, b.z);
    2 + sty.pad.0 + sty.pad.2
}

/// Primitivas vectoriales (knobs, dots, gráficos): circle / line / polyline /
/// polygon. Las coordenadas (`x1..y2`, `points`) son relativas a la caja.
/// OJO: polyline/polygon no consumen alto de flujo salvo `height` explícito.
fn prim_draw_shape(tag: &str, sty: &PrimStyle, b: PrimBox, cx: &mut PrimCtx) -> i32 {
    if tag == "circle" {
        let r = sty.num("r", (b.w / 2).max(1));
        let col = sty.col("color").or(sty.bg).unwrap_or(0xffffff);
        prim_push_circle(cx.st, b.x + r, b.y + r, r, col, b.z);
        if sty.hgt >= 0 { sty.hgt } else { 2 * r }
    } else if tag == "line" {
        let col = sty.col("color").unwrap_or(0xffffff);
        let (x1, y1) = (sty.num("x1", 0), sty.num("y1", 0));
        let (x2, y2) = (sty.num("x2", 0), sty.num("y2", 0));
        prim_push_line(cx.st, b.x + x1, b.y + y1, b.x + x2, b.y + y2, col, b.z);
        if sty.hgt >= 0 { sty.hgt } else { y1.max(y2) }
    } else {
        let col = sty.col("color").or(sty.bg).unwrap_or(0xffffff);
        let pts = sty.get("points").map(|s| prim_parse_points(s, b.x, b.y)).unwrap_or_default();
        let sw = sty.num("stroke-width", 2);
        prim_push_poly(cx.st, pts, col, b.z, tag == "polygon", sw);
        if sty.hgt >= 0 { sty.hgt } else { 0 }
    }
}

/// Caja de texto EDITABLE (Input/Textarea bajan acá). El `.sz` pasa el estado
/// de edición como props (`caret`/`sel-start`/`sel-end`/`focused`) y el core lo
/// PINTA (modelo §5.1). Virtualiza: solo maqueta las `rows` líneas visibles,
/// recortadas a la caja con clip-por-nodo.
fn prim_draw_textbox(kids: &[OwnedValue], sty: &PrimStyle, b: PrimBox, cx: &mut PrimCtx) -> i32 {
    let rows = sty.num("rows", 6);
    let text = prim_join_text(kids);
    // line-height de TEXTO real (como los tags de texto), no la rejilla vieja
    // 12*scale+6 que inflaba la caja y derramaba caret/selección.
    let line_h = sty.num("line-height", 8 * sty.scale + 4);
    let box_pad = 6;
    // `height` explícito manda (permite alinear Input=34px con los demás
    // controles de una fila); sin él, alto derivado de las filas.
    let box_h = if sty.hgt >= 0 { sty.hgt } else { rows * line_h + 2 * box_pad };
    // Padding vertical real: centra el bloque de filas en el alto de la caja.
    let pad_y = ((box_h - rows * line_h) / 2).max(0);
    prim_push_bg(cx.st, b.x, b.y, b.w, box_h, sty.bg.unwrap_or(0x1e293b), b.bg_z, sty.radius, sty.border_w, sty.border_col);
    let saved_clip = cx.st.prim_clip;
    cx.st.prim_clip = Some(prim_clip_intersect(saved_clip, b.x, b.y, b.x + b.w, b.y + box_h));
    prim_emit_text(cx.st, cx.fonts, &text, b.content_x + box_pad, b.y + pad_y, (b.content_w - 2 * box_pad).max(1),
        sty.scale, line_h, sty.text_col, sty.tstyle, rows, sty.font_fam.as_str(), b.z, 0, sty.tspacing);
    // Caret + selección: asume 1 línea (el caso Input); multi-línea es
    // aproximado. El alto de las marcas es el del GLIFO (8*scale)+2, no line-height.
    let fline = text.split('\n').next().unwrap_or("");
    let caret = sty.num("caret", -1);
    let sel_a = sty.num("sel-start", -1);
    let sel_b = sty.num("sel-end", -1);
    let focused = sty.get("focused") == Some("true");
    let tx0 = b.content_x + box_pad;
    let ty0 = b.y + pad_y;
    let mark_h = 8 * sty.scale + 2;
    if sel_a >= 0 && sel_b >= 0 && sel_a != sel_b {
        let (a, bb) = if sel_a < sel_b { (sel_a, sel_b) } else { (sel_b, sel_a) };
        let ax = tx0 + prim_prefix_px(cx.fonts, fline, a as usize, sty.scale, sty.tstyle, sty.font_fam.as_str());
        let bx = tx0 + prim_prefix_px(cx.fonts, fline, bb as usize, sty.scale, sty.tstyle, sty.font_fam.as_str());
        // El highlight va ENTRE el fondo (bg_z) y el texto (z): bg_z + 50.
        prim_push_rect(cx.st, ax, ty0, (bx - ax).max(1), mark_h, 0x2563eb, b.bg_z + 50);
    }
    if focused && caret >= 0 {
        let cxp = tx0 + prim_prefix_px(cx.fonts, fline, caret as usize, sty.scale, sty.tstyle, sty.font_fam.as_str());
        prim_push_rect(cx.st, cxp, ty0, 2, mark_h, sty.text_col, b.z);
    }
    cx.st.prim_clip = saved_clip;
    box_h
}

/// `svg`/`img`. `src` numérico = handle de `Gui.loadSvg` (se rasteriza a la caja,
/// con caché por handle+dims); `src` string = RUTA a imagen raster (png/jpg…,
/// con caché; auto-size = tamaño natural). Sin src o si falla: placeholder.
fn prim_draw_media(sty: &PrimStyle, b: PrimBox, cx: &mut PrimCtx) -> i32 {
    let src = sty.get("src").map(|s| s.to_string());
    let svg_handle = src.as_deref().and_then(|s| s.trim().parse::<i64>().ok());
    let has_w = sty.get("width").is_some();
    let has_h = sty.get("height").is_some();
    let mut drawn = false;
    let mut drawn_h = sty.num("height", 48);
    if let Some(hnd) = svg_handle {
        let w = sty.num("width", 48).min(b.w);
        let hh = sty.num("height", 48);
        if hnd >= 1 && (hnd as usize) <= cx.svgs.len() && w > 0 && hh > 0 {
            let key = (hnd, w, hh);
            let img_handle = if let Some(&ih) = cx.st.svg_cache.get(&key) {
                ih
            } else {
                let px = svg::rasterize(&cx.svgs[(hnd - 1) as usize], w as u32, hh as u32);
                let ih = cx.st.next_image;
                cx.st.next_image += 1;
                cx.st.images.insert(ih, ImageData { w: w as usize, h: hh as usize, px });
                cx.st.svg_cache.insert(key, ih);
                ih
            };
            if let Some(c) = sty.bg {
                prim_push_bg(cx.st, b.x, b.y, w, hh, c, b.bg_z, sty.radius, sty.border_w, sty.border_col);
            }
            prim_push_image(cx.st, b.x, b.y, img_handle, b.z);
            drawn = true;
            drawn_h = hh;
        }
    } else if let Some(path) = src.as_deref() {
        // Dimensiones auto = tamaño natural (aspecto conservado si solo hay una).
        let req_w = if has_w { sty.num("width", 0).min(b.w) } else { 0 };
        let req_h = if has_h { sty.num("height", 0) } else { 0 };
        if let Some((img_handle, iw, ih)) = prim_load_raster(cx.st, path, req_w, req_h) {
            if let Some(c) = sty.bg {
                prim_push_bg(cx.st, b.x, b.y, iw, ih, c, b.bg_z, sty.radius, sty.border_w, sty.border_col);
            }
            prim_push_image(cx.st, b.x, b.y, img_handle, b.z);
            drawn = true;
            drawn_h = ih;
        }
    }
    if !drawn {
        let w = sty.num("width", 48).min(b.w);
        let hh = sty.num("height", 48);
        prim_push_bg(cx.st, b.x, b.y, w, hh, sty.bg.unwrap_or(0x334155), b.bg_z, sty.radius, sty.border_w, sty.border_col);
        drawn_h = hh;
    }
    drawn_h
}

// ── Contenedores ───────────────────────────────────────────────────────────────

/// Contenedor (div/row/section/main…): elige el layout (fila flex, scroll o
/// bloque vertical), aplica el alto explícito y pinta el fondo.
fn prim_layout_container(tag: &str, kids: &[OwnedValue], sty: &PrimStyle, b: PrimBox, kf: PrimFrame, cx: &mut PrimCtx) -> i32 {
    // flex-row: tag `row`, direction:row, o display:flex SIN flex-direction:column
    // (column cae al layout de bloque vertical, como en la web).
    let is_flex = sty.get("display") == Some("flex");
    let col_dir = sty.get("flex-direction") == Some("column") || sty.get("direction") == Some("column");
    let dir_row = (tag == "row" || sty.get("direction") == Some("row") || (is_flex && !col_dir)) && !col_dir;
    let scroll = sty.get("overflow") == Some("scroll") && sty.hgt > 0;

    let mut total_h = if dir_row {
        prim_layout_row(kids, sty, b, kf, cx)
    } else if scroll {
        prim_layout_scroll(kids, sty, b, kf, cx) // emite su fondo antes de recortar
    } else {
        prim_layout_block(kids, sty, b, kf, cx)
    };
    // Alto explícito manda (caja de alto fijo; el contenido puede sobrar, sin clip).
    if sty.hgt > 0 { total_h = sty.hgt; }
    // Fondo (el del scroll ya se emitió arriba, no duplicar).
    if !scroll {
        if let Some(c) = sty.bg { prim_container_bg(sty, b, total_h, c, cx); }
    }
    total_h
}

/// Fondo del contenedor. Con `opacity` (0..1) pinta un RectAlpha translúcido
/// (la primitiva alpha es rectangular: pierde radius/borde — caso backdrop);
/// sin opacity, el fondo normal con borde/radius.
fn prim_container_bg(sty: &PrimStyle, b: PrimBox, h: i32, color: u32, cx: &mut PrimCtx) {
    if let Some(op_s) = sty.get("opacity") {
        if let Ok(op) = op_s.trim().parse::<f32>() {
            let alpha = (op.clamp(0.0, 1.0) * 255.0) as u32;
            let clip = cx.st.prim_clip;
            let id = cx.st.scene_add(SceneNodeKind::RectAlpha { w: b.w, h, alpha }, b.x, b.y, color);
            if let Some(node) = cx.st.scene.iter_mut().find(|n| n.id == id) {
                node.z = b.bg_z;
                node.clip = clip;
            }
            return;
        }
    }
    prim_push_bg(cx.st, b.x, b.y, b.w, h, color, b.bg_z, sty.radius, sty.border_w, sty.border_col);
}

/// Renderiza un hijo de flujo vertical (nodo, o texto suelto con el estilo del
/// contenedor) y devuelve el alto que consumió.
fn prim_flow_child(k: &OwnedValue, kf: PrimFrame, sty: &PrimStyle, b: PrimBox, cx: &mut PrimCtx) -> i32 {
    if let Some((ct, cs, ck)) = prim_node_parts(k) {
        prim_render(ct, cs, ck, kf, cx)
    } else if let OwnedValue::Str(s) = k {
        prim_emit_text(cx.st, cx.fonts, s, kf.x, kf.y, kf.avail_w, sty.scale, 8 * sty.scale + 4,
            sty.text_col, sty.tstyle, i32::MAX, sty.font_fam.as_str(), b.z, sty.talign, sty.tspacing)
    } else {
        0
    }
}

/// Bloque vertical (el layout por defecto): apila los hijos hacia abajo.
/// `gap` se aplica solo ENTRE hijos (como en la web, no después del último).
fn prim_layout_block(kids: &[OwnedValue], sty: &PrimStyle, b: PrimBox, kf: PrimFrame, cx: &mut PrimCtx) -> i32 {
    let mut cy = kf.y;
    let mut first = true;
    for k in kids {
        if !first { cy += sty.gap; }
        first = false;
        cy += prim_flow_child(k, PrimFrame { y: cy, ..kf }, sty, b, cx);
    }
    (cy - kf.y).max(0) + sty.pad.0 + sty.pad.2
}

/// Caja con scroll: alto fijo, hijos desplazados por `scrollY` y RECORTADOS a
/// la caja con clip-por-nodo (independiente del z-order: también recorta los
/// fondos de los hijos, que van a z<0).
fn prim_layout_scroll(kids: &[OwnedValue], sty: &PrimStyle, b: PrimBox, kf: PrimFrame, cx: &mut PrimCtx) -> i32 {
    let box_h = sty.hgt;
    if let Some(c) = sty.bg {
        prim_push_bg(cx.st, b.x, b.y, b.w, box_h, c, b.bg_z, sty.radius, sty.border_w, sty.border_col);
    }
    let scroll_y = sty.num("scrollY", 0).max(0);
    let saved_clip = cx.st.prim_clip;
    cx.st.prim_clip = Some(prim_clip_intersect(saved_clip, b.x, b.y, b.x + b.w, b.y + box_h));
    let mut cy = kf.y - scroll_y;
    let mut first = true;
    for k in kids {
        if !first { cy += sty.gap; }
        first = false;
        cy += prim_flow_child(k, PrimFrame { y: cy, ..kf }, sty, b, cx);
    }
    cx.st.prim_clip = saved_clip;
    box_h
}

// ── Fila flex ──────────────────────────────────────────────────────────────────

/// Un hijo en el plan de reparto de la fila flex.
struct FlexKid {
    grow: i32,        // peso de crecimiento (`flex`); 0 = no crece
    base: i32,        // ancho base en px (width fijo o contenido medido)
    abs: bool,        // position:absolute = fuera de flujo (no ocupa columna ni gap)
    from_width: bool, // el base salió del `width` del hijo: al renderizarlo se le
                      // pasa el ancho del CONTENEDOR para que un `%` no se
                      // re-aplique sobre el slot (50% de 300 = 150, no 50% de 150)
}

/// Estilo efectivo de un HIJO (hoja + inline), para leer flex/width/position
/// ANTES de renderizarlo: el plan de la fila se decide primero.
fn prim_child_style(cx: &PrimCtx, tag: &str, inline: &[OwnedValue]) -> Vec<(String, String)> {
    let cclass = prim_attr(inline, "class").unwrap_or_default();
    let cclasses: Vec<&str> = cclass.split_whitespace().collect();
    let cid = prim_attr(inline, "id");
    prim_eff_style(cx.sheet, cx.ctx, tag, &cclasses, cid.as_deref(), inline)
}

/// Ancho de CONTENIDO de un hijo de texto para el plan flex (shrink-to-fit):
/// su texto medido con su fuente/escala + su padding y margen horizontales,
/// acotado al ancho del contenedor. El base incluye los márgenes (es el slot
/// completo): prim_render les resta ml/mr al ancho útil después.
fn prim_text_fit(tag: &str, cstyle: &[(String, String)], kids: &[OwnedValue], cap_w: i32, fonts: &mut Option<GuiFonts>) -> i32 {
    let scale = snum(cstyle, "font-scale", prim_default_scale(tag));
    let ts = prim_text_style(tag, cstyle);
    let fam = sget(cstyle, "font-family")
        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
        .unwrap_or_default();
    let (_, pr, _, pl) = prim_box_sides(cstyle, "padding");
    let (_, mr, _, ml) = prim_box_sides(cstyle, "margin");
    let txt = prim_join_text(kids);
    let tw = prim_prefix_px(fonts, &txt, txt.chars().count(), scale, ts, fam.as_str());
    (tw + pl + pr + ml + mr).min(cap_w).max(1)
}

/// Arma el plan de la fila: qué hijo crece, cuál es fijo y cuál se encoge a su
/// contenido. Reglas (web-like, adaptadas al motor):
///   `flex: N`             → crece con peso N (0 = fijo a su base)
///   `width: px/%`         → ancho fijo (el % es del ancho del contenedor)
///   texto sin flex/width  → SHRINK-TO-FIT: se mide su contenido, no crece —
///                           así justify-content tiene espacio libre que repartir
///                           (la flecha del Dropdown pegada al borde derecho)
///   div sin flex/width    → crece y llena (default histórico del motor)
fn prim_flex_plan(kids: &[OwnedValue], sty: &PrimStyle, content_w: i32, cx: &mut PrimCtx) -> Vec<FlexKid> {
    let mut plan: Vec<FlexKid> = Vec::with_capacity(kids.len());
    for k in kids {
        let fk = if let Some((ct, cs, ck)) = prim_node_parts(k) {
            let cstyle = prim_child_style(cx, ct, cs);
            if sget(&cstyle, "position") == Some("absolute") {
                // Fuera de flujo (como en CSS): no participa del reparto ni del gap.
                FlexKid { grow: 0, base: 0, abs: true, from_width: false }
            } else {
                let flex = sget(&cstyle, "flex").and_then(|s| s.trim().parse::<i32>().ok());
                // width admite px (con o sin sufijo) y `%` del ancho del contenedor.
                let width = match prim_dim(&cstyle, "width", content_w) {
                    w if w >= 0 => Some(w),
                    _ => None,
                };
                match (flex, width) {
                    (Some(fx), w) => FlexKid { grow: fx.max(0), base: w.unwrap_or(0).max(0), abs: false, from_width: false },
                    (None, Some(w)) => FlexKid { grow: 0, base: w.max(0), abs: false, from_width: true },
                    (None, None) if prim_is_text_tag(ct) =>
                        FlexKid { grow: 0, base: prim_text_fit(ct, &cstyle, ck, content_w, cx.fonts), abs: false, from_width: false },
                    (None, None) => FlexKid { grow: 1, base: 0, abs: false, from_width: false },
                }
            }
        } else if let OwnedValue::Str(s) = k {
            // Texto suelto: misma regla shrink-to-fit que un span (se dibuja con
            // la fuente/escala del contenedor).
            let tw = prim_prefix_px(cx.fonts, s, s.chars().count(), sty.scale, sty.tstyle, sty.font_fam.as_str());
            FlexKid { grow: 0, base: tw.min(content_w).max(1), abs: false, from_width: false }
        } else {
            FlexKid { grow: 1, base: 0, abs: false, from_width: false }
        };
        plan.push(fk);
    }
    plan
}

/// Fila flex (eje principal horizontal). Reparte anchos según el plan, aplica
/// justify-content cuando nadie crece (espacio libre = todos fijos) y alinea en
/// el eje cruzado con un post-pase (el alto de cada hijo se conoce al dibujarlo).
fn prim_layout_row(kids: &[OwnedValue], sty: &PrimStyle, b: PrimBox, kf: PrimFrame, cx: &mut PrimCtx) -> i32 {
    let plan = prim_flex_plan(kids, sty, b.content_w, cx);
    let ncols = (plan.iter().filter(|k| !k.abs).count().max(1)) as i32;
    let total_gap = sty.gap * (ncols - 1).max(0);
    let total_grow: i32 = plan.iter().map(|k| k.grow).sum();
    let sum_base: i32 = plan.iter().map(|k| k.base).sum();

    // Anchos finales + hueco inicial/extra según justify-content.
    let mut widths: Vec<i32> = Vec::with_capacity(plan.len());
    let start_off;
    let extra_between;
    if total_grow > 0 {
        // Alguien crece: los growers absorben el sobrante; no queda espacio libre.
        let free_for_grow = (b.content_w - sum_base - total_gap).max(0);
        for k in &plan {
            let w = if k.abs { 0 }
                else if k.grow > 0 { (k.base + free_for_grow * k.grow / total_grow).max(1) }
                else { k.base.max(1) };
            widths.push(w);
        }
        start_off = 0;
        extra_between = 0;
    } else {
        // Todos fijos: justify-content reparte el espacio libre.
        for k in &plan { widths.push(if k.abs { 0 } else { k.base.max(1) }); }
        let used: i32 = widths.iter().sum::<i32>() + total_gap;
        let free = (b.content_w - used).max(0);
        let justify = sty.get("justify-content").or_else(|| sty.get("justify")).unwrap_or("flex-start");
        let (s, e) = prim_justify(justify, free, ncols);
        start_off = s;
        extra_between = e;
    }

    let align = sty.get("align-items").or_else(|| sty.get("align")).unwrap_or("").to_string();
    let want_align = align == "center" || align == "flex-end" || align == "end";

    let mut cxr = b.content_x + start_off;
    let mut max_h = 0;
    // Rango de nodos/regions por hijo, para alinear en vertical tras conocer max_h.
    let mut ranges: Vec<(usize, usize, usize, usize, i32)> = Vec::new();
    for (i, k) in kids.iter().enumerate() {
        // Fuera de flujo: se renderiza con el ancho del contenedor (para %/left)
        // sin ocupar columna ni gap, y no participa de align-items.
        if plan.get(i).map(|p| p.abs).unwrap_or(false) {
            if let Some((ct, cs, ck)) = prim_node_parts(k) {
                prim_render(ct, cs, ck, PrimFrame { x: cxr, avail_w: b.content_w, ..kf }, cx);
            }
            continue;
        }
        let each = widths.get(i).copied().unwrap_or(1);
        let sc0 = cx.st.scene.len();
        let rg0 = cx.regions.len();
        let h = if let Some((ct, cs, ck)) = prim_node_parts(k) {
            // Si el slot salió del `width` del hijo, se le pasa el ancho del
            // CONTENEDOR: así su `width: 50%` se resuelve otra vez contra lo
            // mismo que usó el plan (y no contra el slot, que lo achicaría).
            let child_avail = if plan.get(i).map(|p| p.from_width).unwrap_or(false) { b.content_w } else { each };
            prim_render(ct, cs, ck, PrimFrame { x: cxr, avail_w: child_avail, ..kf }, cx)
        } else if let OwnedValue::Str(s) = k {
            prim_emit_text(cx.st, cx.fonts, s, cxr, kf.y, each, sty.scale, 8 * sty.scale + 4,
                sty.text_col, sty.tstyle, i32::MAX, sty.font_fam.as_str(), b.z, sty.talign, sty.tspacing)
        } else { 0 };
        if h > max_h { max_h = h; }
        if want_align { ranges.push((sc0, cx.st.scene.len(), rg0, cx.regions.len(), h)); }
        cxr += each + sty.gap + extra_between;
    }
    // align-items (post-pase): desplaza cada hijo dentro de la banda de cruce.
    // La banda es el alto EXPLÍCITO del contenedor (menos padding) si lo hay —
    // p. ej. un backdrop full-height centrando una caja — o el hijo más alto.
    if want_align {
        let band = if sty.hgt > 0 { (sty.hgt - sty.pad.0 - sty.pad.2).max(max_h) } else { max_h };
        for (sc0, sc1, rg0, rg1, h) in ranges {
            let off = if align == "center" { (band - h) / 2 } else { band - h };
            if off > 0 {
                for n in cx.st.scene[sc0..sc1].iter_mut() { n.y += off; }
                for r in cx.regions[rg0..rg1].iter_mut() { r.y += off; }
            }
        }
    }
    max_h + sty.pad.0 + sty.pad.2
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

    /// Ancho en px de `text` con la familia actual y `style` (bit0=bold, bit1=italic).
    /// Como `measure`, pero considerando el estilo (bold/italic ensanchan el glifo).
    /// Familia default → rejilla 8*scale; familia custom → advance real proporcional.
    fn text_width(&mut self, text: &str, scale: i32, style: u8) -> i64 {
        let scale = scale.max(1);
        if self.current == 0 {
            return text.chars().filter(|c| !c.is_control()).count() as i64 * 8 * scale as i64;
        }
        let mut w: i64 = 0;
        for ch in text.chars() {
            w += self.char_width(ch, scale, style);
        }
        w
    }

    /// Ancho de avance de un solo carácter con la familia/estilo actuales.
    fn char_width(&mut self, ch: char, scale: i32, style: u8) -> i64 {
        let scale = scale.max(1);
        if self.current == 0 {
            return if ch.is_control() { 0 } else { 8 * scale as i64 };
        }
        if ch.is_control() {
            return 0;
        }
        self.ensure_glyph(ch, scale, style);
        self.glyphs
            .get(&(self.current, ch, scale, style))
            .map(|g| g.advance as i64)
            .unwrap_or(0)
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
/// Campos POR VENTANA de GuiState, intercambiables con `switch_to`: GuiState
/// siempre representa "la ventana seleccionada"; las demás viven aquí. Así los
/// ~50 métodos de dibujo no saben nada de multi-ventana.
struct WinSlot {
    open: bool,
    canvas: Vec<u32>,
    width: usize,
    height: usize,
    win_w: usize,
    win_h: usize,
    bg: u32,
    clip: (i32, i32, i32, i32),
    clip_stack: Vec<(i32, i32, i32, i32)>,
    input: InputSnapshot,
    scale_factor: f64,
    win_x: i32,
    win_y: i32,
    // La escena retained es POR VENTANA (cada ventana tiene su scene graph);
    // los ids de nodo sí son globales (next_node no se swapea).
    scene: Vec<SceneNode>,
    scene_dirty: bool,
}

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
    win_x: i32,                      // posición outer de la ventana (refrescada en present)
    win_y: i32,
    monitors: Vec<MonitorInfo>,      // monitores conectados (refrescados en present)
    // ── Multi-ventana ──
    current_win: u32,                    // ventana seleccionada (0 = la de Gui.open)
    bg_windows: HashMap<u32, WinSlot>,   // ventanas NO seleccionadas
    next_win_id: u32,                    // ids para Gui.openWindow (≥ 1)
    // ── Modo retenido ──
    scene: Vec<SceneNode>,               // nodos persistentes (una escena, se
                                         // dibuja sobre la ventana seleccionada)
    next_node: i64,
    scene_dirty: bool,
    // Clip activo del motor de primitivos DURANTE renderTree (scratch, no por-ventana:
    // siempre None entre frames porque prim_render lo balancea). Se estampa en cada
    // nodo emitido para recortar subárboles scrolleados sin depender del z-order.
    prim_clip: Option<(i32, i32, i32, i32)>,
    // Caché de SVGs rasterizados: (handle_svg, w, h) → handle de imagen en `images`.
    // Evita re-rasterizar con tiny-skia cada frame; solo al cambiar de tamaño/handle.
    svg_cache: HashMap<(i64, i32, i32), i64>,
    // Caché de imágenes RASTER (png/jpg…) cargadas por RUTA desde el primitivo `img`:
    // (ruta, w, h) → handle en `images`. Evita releer+decodificar+escalar cada frame.
    raster_cache: HashMap<(String, i32, i32), i64>,
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
            win_x: 0,
            win_y: 0,
            monitors: Vec::new(),
            current_win: 0,
            bg_windows: HashMap::new(),
            next_win_id: 1,
            scene: Vec::new(),
            next_node: 1,
            scene_dirty: true,
            prim_clip: None,
            svg_cache: HashMap::new(),
            raster_cache: HashMap::new(),
        }
    }

    /// Añade un nodo a la escena y devuelve su id.
    fn scene_add(&mut self, kind: SceneNodeKind, x: i32, y: i32, color: u32) -> i64 {
        let id = self.next_node;
        self.next_node += 1;
        self.scene.push(SceneNode { id, kind, x, y, color, z: 0, visible: true, clip: None });
        self.scene_dirty = true;
        id
    }

    /// Redibuja la escena en el canvas (orden: z, luego inserción).
    fn scene_render(&mut self, fonts: &mut GuiFonts, bg: u32) {
        // Reconciliar canvas al tamaño de la ventana (como Gui.clear).
        if self.win_w != self.width || self.win_h != self.height {
            self.width = self.win_w.max(1);
            self.height = self.win_h.max(1);
            self.canvas = vec![bg; self.width * self.height];
        }
        self.bg = bg;
        for px in self.canvas.iter_mut() {
            *px = bg;
        }
        self.clip = (0, 0, self.width as i32, self.height as i32);
        self.clip_stack.clear();

        // take + devolver: dibujar necesita &mut self mientras se itera la escena.
        let mut nodes = std::mem::take(&mut self.scene);
        nodes.sort_by_key(|n| (n.z, n.id));
        for n in &nodes {
            if !n.visible {
                continue;
            }
            // Clip por-nodo (motor de primitivos): recorta este nodo a su rect propio,
            // intersecado con el clip del stack. Los marcadores ClipPush/ClipPop mutan
            // el clip del stack de forma persistente, así que NO se envuelven aquí.
            let is_clip_marker = matches!(n.kind, SceneNodeKind::ClipPush { .. } | SceneNodeKind::ClipPop);
            let saved_clip = self.clip;
            if !is_clip_marker {
                if let Some((cx0, cy0, cx1, cy1)) = n.clip {
                    let (bx0, by0, bx1, by1) = self.clip;
                    let nx0 = cx0.max(bx0);
                    let ny0 = cy0.max(by0);
                    self.clip = (nx0, ny0, cx1.min(bx1).max(nx0), cy1.min(by1).max(ny0));
                }
            }
            match &n.kind {
                SceneNodeKind::Rect { w, h } => self.fill_rect(n.x, n.y, *w, *h, n.color),
                SceneNodeKind::RectAlpha { w, h, alpha } => {
                    let r = ((n.color >> 16) & 0xff) as u8;
                    let g = ((n.color >> 8) & 0xff) as u8;
                    let b = (n.color & 0xff) as u8;
                    self.blend_rect(n.x, n.y, *w, *h, r, g, b, *alpha);
                }
                SceneNodeKind::RectOutline { w, h } => self.draw_rect(n.x, n.y, *w, *h, n.color),
                SceneNodeKind::RoundRect { w, h, radius } => {
                    self.fill_round_rect(n.x, n.y, *w, *h, *radius, n.color)
                }
                SceneNodeKind::Circle { r } => self.fill_circle(n.x, n.y, *r, n.color),
                SceneNodeKind::Line { x2, y2 } => self.draw_line(n.x, n.y, *x2, *y2, n.color),
                SceneNodeKind::Polygon { points } => {
                    let pts: Vec<(i32, i32)> = points.chunks(2)
                        .filter(|c| c.len() == 2)
                        .map(|c| (c[0], c[1]))
                        .collect();
                    self.fill_polygon(&pts, n.color);
                }
                SceneNodeKind::Polyline { points, width } => {
                    let mut i = 0;
                    while i + 3 < points.len() {
                        self.draw_thick_line(points[i], points[i + 1], points[i + 2], points[i + 3], *width, n.color);
                        i += 2;
                    }
                }
                SceneNodeKind::Text { text, scale, font, style, spacing } => {
                    if font.is_empty() {
                        self.draw_text(fonts, n.x, n.y, text, *scale, n.color, *style, *spacing);
                    } else {
                        // Fuente por nodo: fijar y restaurar la familia actual.
                        let prev = fonts.current;
                        fonts.set_family(font);
                        self.draw_text(fonts, n.x, n.y, text, *scale, n.color, *style, *spacing);
                        fonts.current = prev;
                    }
                }
                SceneNodeKind::Image { handle } => self.draw_image(n.x, n.y, *handle),
                SceneNodeKind::ClipPush { w, h } => {
                    self.clip_stack.push(self.clip);
                    let (cx0, cy0, cx1, cy1) = self.clip;
                    let nx0 = n.x.max(cx0);
                    let ny0 = n.y.max(cy0);
                    let nx1 = (n.x + *w).min(cx1);
                    let ny1 = (n.y + *h).min(cy1);
                    self.clip = (nx0, ny0, nx1.max(nx0), ny1.max(ny0));
                }
                SceneNodeKind::ClipPop => {
                    self.clip = self.clip_stack.pop()
                        .unwrap_or((0, 0, self.width as i32, self.height as i32));
                }
            }
            // Restaura el clip del stack tras un nodo dibujable (el clip por-nodo era
            // solo para él). Los marcadores dejan su mutación del stack intacta.
            if !is_clip_marker {
                self.clip = saved_clip;
            }
        }
        // Un ClipPush sin su ClipPop no debe dejar el clip pegado.
        self.clip = (0, 0, self.width as i32, self.height as i32);
        self.clip_stack.clear();
        self.scene = nodes;
        self.scene_dirty = false;
    }

    /// Extrae los campos por-ventana actuales como un WinSlot.
    fn take_slot(&mut self) -> WinSlot {
        WinSlot {
            open: self.open,
            canvas: std::mem::take(&mut self.canvas),
            width: self.width,
            height: self.height,
            win_w: self.win_w,
            win_h: self.win_h,
            bg: self.bg,
            clip: self.clip,
            clip_stack: std::mem::take(&mut self.clip_stack),
            input: std::mem::take(&mut self.input),
            scale_factor: self.scale_factor,
            win_x: self.win_x,
            win_y: self.win_y,
            scene: std::mem::take(&mut self.scene),
            scene_dirty: self.scene_dirty,
        }
    }

    fn put_slot(&mut self, s: WinSlot) {
        self.open = s.open;
        self.canvas = s.canvas;
        self.width = s.width;
        self.height = s.height;
        self.win_w = s.win_w;
        self.win_h = s.win_h;
        self.bg = s.bg;
        self.clip = s.clip;
        self.clip_stack = s.clip_stack;
        self.input = s.input;
        self.scale_factor = s.scale_factor;
        self.win_x = s.win_x;
        self.win_y = s.win_y;
        self.scene = s.scene;
        self.scene_dirty = s.scene_dirty;
    }

    /// Cambia la ventana seleccionada intercambiando los campos por-ventana.
    /// Devuelve false si `id` no existe.
    fn switch_to(&mut self, id: u32) -> bool {
        if id == self.current_win {
            return true;
        }
        let target = match self.bg_windows.remove(&id) {
            Some(t) => t,
            None => return false,
        };
        let old = self.take_slot();
        self.bg_windows.insert(self.current_win, old);
        self.put_slot(target);
        self.current_win = id;
        true
    }

    /// Present de una ventana EXTRA (la seleccionada, id ≥ 1): mismo handshake
    /// que `present`, contra su entrada del mapa `extra`.
    fn present_extra(&mut self, host: &GuiHost, id: u32) {
        let mut g = host.inner.lock().unwrap();
        let want = {
            let e = match g.extra.get_mut(&id) {
                Some(e) => e,
                None => { self.open = false; return; }
            };
            e.canvas.clear();
            e.canvas.extend_from_slice(&self.canvas);
            e.canvas_w = self.width;
            e.canvas_h = self.height;
            e.bg_color = self.bg;
            e.present_seq += 1;
            e.present_seq
        };
        host.cv.notify_all();
        drop(g);
        host.wake_main();
        let mut g = host.inner.lock().unwrap();
        loop {
            let (done, alive) = match g.extra.get(&id) {
                Some(e) => (e.done_seq, e.window_open && !e.should_close),
                None => (want, false),
            };
            if done >= want || !alive {
                break;
            }
            g = host.cv.wait(g).unwrap();
        }
        if let Some(e) = g.extra.get(&id) {
            self.input = e.input.clone();
            self.win_w = e.win_w.max(1);
            self.win_h = e.win_h.max(1);
            self.open = e.window_open && !e.should_close;
        } else {
            self.open = false;
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
        self.win_x = g.win_x;
        self.win_y = g.win_y;
        self.monitors = g.monitors.clone();
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

    /// `style` es bitfield: bit0=bold, bit1=italic (afectan al glifo), bit2=subrayado,
    /// bit3=tachado (decoraciones: líneas dibujadas sobre el ancho del texto, NO afectan
    /// la forma del glifo ni la caché). `letter_spacing` = px extra entre caracteres.
    fn draw_text(&mut self, fonts: &mut GuiFonts, x: i32, y: i32, text: &str, scale: i32, rgb: u32, style: u8, letter_spacing: i32) {
        let scale = scale.max(1);
        let r = ((rgb >> 16) & 0xff) as u8;
        let g = ((rgb >> 8) & 0xff) as u8;
        let b = (rgb & 0xff) as u8;
        let glyph_style = style & 0b11;          // solo bold/italic cambian el glifo
        let underline = style & 0b100 != 0;
        let strike    = style & 0b1000 != 0;
        let fam = fonts.current;
        let mut pen = x;
        let mut first = true;
        for ch in text.chars() {
            if !first {
                pen += letter_spacing;           // espaciado entre caracteres (no antes del 1º)
            }
            first = false;
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
            fonts.ensure_glyph(ch, scale, glyph_style);
            if let Some(gl) = fonts.glyphs.get(&(fam, ch, scale, glyph_style)) {
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
        // Decoraciones: líneas horizontales a lo ancho del texto ([x, pen)).
        if (underline || strike) && pen > x {
            let size = 8 * scale;
            let thick = scale.max(1);
            if underline {
                self.fill_rect(x, y + size - thick, pen - x, thick, rgb); // bajo la línea base
            }
            if strike {
                self.fill_rect(x, y + size / 2 - thick / 2, pen - x, thick, rgb); // a media altura
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

    /// Línea de grosor `width` px: estampa discos antialiased a lo largo del trazo
    /// (extremos y juntas redondeados). width<=1 cae a draw_line (1px exacto).
    fn draw_thick_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, width: i32, color: u32) {
        if width <= 1 {
            self.draw_line(x0, y0, x1, y1, color);
            return;
        }
        let r = width / 2;
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let (mut x, mut y) = (x0, y0);
        loop {
            self.fill_circle(x, y, r, color);
            if x == x1 && y == y1 { break; }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; x += sx; }
            if e2 <= dx { err += dx; y += sy; }
        }
    }

    /// Contorno de círculo de 1px (midpoint), respetando el clip.
    fn draw_circle(&mut self, cx: i32, cy: i32, r: i32, color: u32) {
        if r <= 0 { return; }
        let mut x = r;
        let mut y = 0;
        let mut err = 1 - r;
        while x >= y {
            self.put(cx + x, cy + y, color);
            self.put(cx + y, cy + x, color);
            self.put(cx - y, cy + x, color);
            self.put(cx - x, cy + y, color);
            self.put(cx - x, cy - y, color);
            self.put(cx - y, cy - x, color);
            self.put(cx + y, cy - x, color);
            self.put(cx + x, cy - y, color);
            y += 1;
            if err < 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err += 2 * (y - x) + 1;
            }
        }
    }

    /// Relleno sólido de un polígono arbitrario (regla par-impar, scanline).
    /// `pts` = vértices en orden; el último se cierra con el primero.
    fn fill_polygon(&mut self, pts: &[(i32, i32)], color: u32) {
        if pts.len() < 3 { return; }
        let (cx0, cy0, cx1, cy1) = self.clip;
        let mut ymin = i32::MAX;
        let mut ymax = i32::MIN;
        for &(_, y) in pts {
            ymin = ymin.min(y);
            ymax = ymax.max(y);
        }
        ymin = ymin.max(0).max(cy0);
        ymax = ymax.min(self.height as i32 - 1).min(cy1 - 1);
        let n = pts.len();
        let mut xs: Vec<i32> = Vec::new();
        let mut y = ymin;
        while y <= ymax {
            xs.clear();
            let yf = y as f32 + 0.5;
            for i in 0..n {
                let (ax, ay) = pts[i];
                let (bx, by) = pts[(i + 1) % n];
                let (ayf, byf) = (ay as f32, by as f32);
                if (ayf <= yf && byf > yf) || (byf <= yf && ayf > yf) {
                    let t = (yf - ayf) / (byf - ayf);
                    xs.push((ax as f32 + t * (bx - ax) as f32).round() as i32);
                }
            }
            xs.sort_unstable();
            let row = (y as usize) * self.width;
            let mut k = 0;
            while k + 1 < xs.len() {
                let xa = xs[k].max(0).max(cx0);
                let xb = (xs[k + 1] - 1).min(self.width as i32 - 1).min(cx1 - 1);
                let mut x = xa;
                while x <= xb {
                    self.canvas[row + x as usize] = color;
                    x += 1;
                }
                k += 2;
            }
            y += 1;
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

/// Input acumulado de una ventana EXTRA (subconjunto útil: mouse + teclado +
/// scroll + foco; gestos/drop/IME quedan en la ventana principal).
#[derive(Default)]
struct ExtraAccum {
    keys_down: HashSet<String>,
    keys_pressed: Vec<String>,
    keys_repeated: Vec<String>,
    keys_released: Vec<String>,
    chars_typed: String,
    mouse_x: i32,
    mouse_y: i32,
    mouse_l: bool,
    mouse_r: bool,
    mouse_m: bool,
    prev_l: bool,
    /// Presses de botón izquierdo (EVENTOS) desde el último take_input: un
    /// click corto entre dos presents no se pierde (el nivel sí se perdería).
    clicks: u32,
    scroll_x: f32,
    scroll_y: f32,
    focused: bool,
    cursor_in: bool,
}

impl ExtraAccum {
    fn take_input(&mut self, mods: &ModifiersState) -> InputSnapshot {
        let pressed = self.clicks > 0 || (self.mouse_l && !self.prev_l);
        self.clicks = 0;
        self.prev_l = self.mouse_l;
        let snap = InputSnapshot {
            keys_down: self.keys_down.clone(),
            shift: mods.shift_key(),
            ctrl: mods.control_key(),
            alt: mods.alt_key(),
            sup: mods.super_key(),
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
            ..InputSnapshot::default()
        };
        self.scroll_x = 0.0;
        self.scroll_y = 0.0;
        snap
    }
}

/// Una ventana extra viva en el hilo main.
struct ExtraWin {
    window: Rc<Window>,
    _context: Context<Rc<Window>>,
    surface: Surface<Rc<Window>, Rc<Window>>,
    accum: ExtraAccum,
    last_present: u64,
    close_requested: bool,
}

struct GuiMain {
    host: Arc<GuiHost>,
    window: Option<Rc<Window>>,
    context: Option<Context<Rc<Window>>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    session_active: bool,
    close_requested: bool,
    // ── multi-ventana ──
    extras: HashMap<u32, ExtraWin>,
    extra_ids: HashMap<WindowId, u32>,
    pending_extra_opens: Vec<(u32, String, u32, u32)>,
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
    monitors_dirty: bool,  // pedir recolección de monitores (al abrir + ScaleFactorChanged)
    // Cursor custom pendiente de aplicar: se crea en about_to_wait (necesita el
    // ActiveEventLoop). rgba vacío = restaurar cursor por defecto. None = nada pendiente.
    pending_cursor: Option<(Vec<u8>, u32, u32, u32, u32)>,
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
            extras: HashMap::new(),
            extra_ids: HashMap::new(),
            pending_extra_opens: Vec::new(),
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
            monitors_dirty: true,
            pending_cursor: None,
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
        // Posición outer + monitores iniciales (recolectados antes de tomar el lock).
        let (wx, wy) = self.window.as_ref()
            .and_then(|w| w.outer_position().ok())
            .map(|p| (p.x, p.y))
            .unwrap_or((0, 0));
        let mons = self.window.as_ref().map(|w| collect_monitors(w)).unwrap_or_default();
        self.monitors_dirty = false;
        let mut g = self.host.inner.lock().unwrap();
        g.window_ready = true;
        g.window_open = true;
        g.should_close = false;
        g.win_w = size.width.max(1) as usize;
        g.win_h = size.height.max(1) as usize;
        g.win_x = wx;
        g.win_y = wy;
        g.monitors = mons;
        self.host.cv.notify_all();
    }

    /// Crea las ventanas EXTRA pendientes (necesita el ActiveEventLoop).
    fn ensure_extra_windows(&mut self, el: &ActiveEventLoop) {
        let pending = std::mem::take(&mut self.pending_extra_opens);
        for (id, title, w, h) in pending {
            let attrs = Window::default_attributes()
                .with_title(title)
                .with_inner_size(LogicalSize::new(w as f64, h as f64));
            let created = el.create_window(attrs).ok().map(Rc::new).and_then(|window| {
                let context = Context::new(window.clone()).ok()?;
                let surface = Surface::new(&context, window.clone()).ok()?;
                Some((window, context, surface))
            });
            let mut g = self.host.inner.lock().unwrap();
            match created {
                Some((window, context, surface)) => {
                    let size = window.inner_size();
                    if let Some(e) = g.extra.get_mut(&id) {
                        e.window_ready = true;
                        e.window_open = true;
                        e.should_close = false;
                        e.win_w = size.width.max(1) as usize;
                        e.win_h = size.height.max(1) as usize;
                    }
                    self.extra_ids.insert(window.id(), id);
                    self.extras.insert(id, ExtraWin {
                        window,
                        _context: context,
                        surface,
                        accum: ExtraAccum { focused: true, ..ExtraAccum::default() },
                        last_present: 0,
                        close_requested: false,
                    });
                }
                None => {
                    if let Some(e) = g.extra.get_mut(&id) {
                        e.open_failed = true;
                    }
                }
            }
            self.host.cv.notify_all();
        }
    }

    /// Atiende presents/cierres de las ventanas EXTRA (con el lock ya tomado).
    fn service_extras(&mut self, g: &mut SharedInner) {
        let mut to_drop: Vec<u32> = Vec::new();
        for (id, win) in self.extras.iter_mut() {
            let shared = match g.extra.get_mut(id) {
                Some(s) => s,
                None => { to_drop.push(*id); continue; }
            };
            // Cierre pedido por el usuario (X) o por el intérprete.
            if win.close_requested || shared.should_close || g.interp_done || !self.session_active {
                shared.window_open = false;
                shared.should_close = true;
                // Despierta a un present_extra que esté esperando este frame.
                shared.done_seq = shared.present_seq;
                to_drop.push(*id);
                continue;
            }
            // Tamaño vivo.
            let s = win.window.inner_size();
            shared.win_w = s.width.max(1) as usize;
            shared.win_h = s.height.max(1) as usize;
            // Present pendiente → blit + input propio de esa ventana.
            if shared.present_seq > win.last_present {
                win.last_present = shared.present_seq;
                shared.done_seq = shared.present_seq;
                shared.input = win.accum.take_input(&self.mods);
                let canvas = shared.canvas.clone();
                let (cw, ch, bg) = (shared.canvas_w, shared.canvas_h, shared.bg_color);
                blit_plain(&win.window, &mut win.surface, &canvas, cw, ch, bg);
            }
        }
        for id in to_drop {
            if let Some(w) = self.extras.remove(&id) {
                self.extra_ids.remove(&w.window.id());
            }
        }
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
                    // El cursor custom necesita el ActiveEventLoop (create_custom_cursor),
                    // que no está aquí: lo dejamos pendiente y se aplica en about_to_wait.
                    GuiCmd::SetCustomCursor(rgba, w, h, hx, hy) => {
                        self.pending_cursor = Some((rgba, w, h, hx, hy));
                    }
                    GuiCmd::FileDialog { save, filter_name, filter_exts, default_name } => {
                        pending_dialog = Some((g.dialog_seq, save, filter_name, filter_exts, default_name));
                    }
                    // multi-ventana
                    GuiCmd::OpenExtra { id, title, w, h } => {
                        self.pending_extra_opens.push((id, title, w, h));
                    }
                    GuiCmd::CloseExtra { id } => {
                        if let Some(win) = self.extras.get_mut(&id) {
                            win.close_requested = true;
                        }
                    }
                    GuiCmd::SetTitleExtra { id, title } => {
                        if let Some(win) = self.extras.get(&id) {
                            win.window.set_title(&title);
                        }
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
                g.virtual_scroll_x = 0;
            }
            // Tamaño de ventana + posición outer + factor de escala (HiDPI) → compartido.
            // Los monitores SOLO se recolectan cuando están marcados sucios (al abrir o
            // al cambiar la escala), no cada frame: available_monitors() enumera el SO.
            let want_monitors = self.monitors_dirty;
            if let Some(win) = &self.window {
                let s = win.inner_size();
                g.win_w = s.width.max(1) as usize;
                g.win_h = s.height.max(1) as usize;
                g.scale_factor = win.scale_factor();
                if let Ok(pos) = win.outer_position() {
                    g.win_x = pos.x;
                    g.win_y = pos.y;
                }
                if want_monitors {
                    g.monitors = collect_monitors(win);
                }
            }
            if want_monitors {
                self.monitors_dirty = false;
            }
            // Cierre.
            if self.close_requested || g.interp_done {
                g.should_close = true;
                g.window_open = false;
                self.session_active = false;
            }
            // Ventanas extra: presents/cierres/tamaños (después del bloque de
            // cierre, para que la muerte de la sesión las libere ya).
            self.service_extras(&mut g);
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
            let (vx, vy, bg) = {
                let g = self.host.inner.lock().unwrap();
                (g.virtual_scroll_x, g.virtual_scroll_y, g.bg_color)
            };
            self.blit(&canvas, cw, ch, vx, vy, bg);
        }
        if !self.session_active {
            self.window = None;
            self.surface = None;
            self.context = None;
            self.close_requested = false;
            // La sesión terminó: las ventanas extra mueren con ella (ya
            // marcadas cerradas en service_extras).
            self.extras.clear();
            self.extra_ids.clear();
            self.pending_extra_opens.clear();
        }
    }

    /// Sirve el frame pendiente + re-blit del último canvas. Para `Resized`/
    /// `RedrawRequested` durante el modal loop de resize (mantiene la ventana viva).
    fn service_and_repaint(&mut self) {
        self.service();
        let snap = {
            let g = self.host.inner.lock().unwrap();
            (g.canvas.clone(), g.canvas_w, g.canvas_h, g.virtual_scroll_x, g.virtual_scroll_y, g.bg_color)
        };
        if snap.1 > 0 && snap.2 > 0 {
            self.blit(&snap.0, snap.1, snap.2, snap.3, snap.4, snap.5);
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

    fn blit(&mut self, canvas: &[u32], cw: usize, ch: usize, offset_x: i32, offset_y: i32, bg: u32) {
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
                if offset_x == 0 {
                    // Fast-path sin scroll horizontal: copia de fila en bloque (como antes).
                    buffer[brow..brow + n].copy_from_slice(&canvas[crow..crow + n]);
                    for x in n..bw {
                        buffer[brow + x] = bg;
                    }
                } else {
                    // Con scroll horizontal: desplazar columnas (bg fuera del canvas).
                    for x in 0..bw {
                        let virtual_x = x as i32 + offset_x;
                        buffer[brow + x] = if virtual_x >= 0 && (virtual_x as usize) < cw {
                            canvas[crow + virtual_x as usize]
                        } else {
                            bg
                        };
                    }
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

/// Blit simple (sin scroll virtual) para ventanas extra.
fn blit_plain(
    window: &Rc<Window>,
    surface: &mut Surface<Rc<Window>, Rc<Window>>,
    canvas: &[u32],
    cw: usize,
    ch: usize,
    bg: u32,
) {
    let size = window.inner_size();
    let (bw, bh) = (size.width as usize, size.height as usize);
    if bw == 0 || bh == 0 {
        return;
    }
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
        if y < ch {
            let crow = y * cw;
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

impl ApplicationHandler for GuiMain {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.ensure_window(event_loop);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.ensure_window(event_loop);
        self.ensure_extra_windows(event_loop);
        // Aplicar un cursor custom pendiente: requiere el ActiveEventLoop para crearlo.
        if let Some((rgba, w, h, hx, hy)) = self.pending_cursor.take() {
            if let Some(win) = &self.window {
                if rgba.is_empty() {
                    win.set_cursor(CursorIcon::Default);
                } else if let Ok(src) = CustomCursor::from_rgba(rgba, w as u16, h as u16, hx as u16, hy as u16) {
                    let cursor = event_loop.create_custom_cursor(src);
                    win.set_cursor(cursor);
                }
            }
        }
    }

    // Despertador del intérprete (proxy.send_event en present()): solo necesita sacar al
    // pump de su espera larga; service() — llamado tras el pump — sirve el frame.
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: ()) {}

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // ── Ruteo multi-ventana: los eventos de una ventana EXTRA van a su
        //    acumulador propio (mouse/teclado/scroll/foco/cierre). ──
        if let Some(&wid) = self.extra_ids.get(&_id) {
            self.pending_input = true;
            match event {
                WindowEvent::ModifiersChanged(m) => {
                    self.mods = m.state();
                    return;
                }
                WindowEvent::Resized(_) | WindowEvent::RedrawRequested => {
                    self.service();
                    return;
                }
                _ => {}
            }
            if let Some(win) = self.extras.get_mut(&wid) {
                let acc = &mut win.accum;
                match event {
                    WindowEvent::CloseRequested => win.close_requested = true,
                    WindowEvent::KeyboardInput { event, .. } => {
                        let name = key_name(&event.logical_key);
                        match event.state {
                            ElementState::Pressed => {
                                if let Some(n) = name.clone() {
                                    if !event.repeat {
                                        acc.keys_pressed.push(n.clone());
                                        acc.keys_down.insert(n.clone());
                                    }
                                    acc.keys_repeated.push(n);
                                }
                                if let Some(t) = &event.text {
                                    for c in t.chars() {
                                        if !c.is_control() {
                                            acc.chars_typed.push(c);
                                        }
                                    }
                                }
                            }
                            ElementState::Released => {
                                if let Some(n) = name {
                                    acc.keys_released.push(n.clone());
                                    acc.keys_down.remove(&n);
                                }
                            }
                        }
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        acc.mouse_x = position.x as i32;
                        acc.mouse_y = position.y as i32;
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        let down = state == ElementState::Pressed;
                        match button {
                            MouseButton::Left => {
                                acc.mouse_l = down;
                                if down {
                                    acc.clicks += 1;
                                }
                            }
                            MouseButton::Right => acc.mouse_r = down,
                            MouseButton::Middle => acc.mouse_m = down,
                            _ => {}
                        }
                    }
                    WindowEvent::MouseWheel { delta, .. } => {
                        let (dx, dy) = match delta {
                            MouseScrollDelta::LineDelta(dx, dy) => (dx, dy),
                            MouseScrollDelta::PixelDelta(p) => ((p.x as f32) / 12.0, (p.y as f32) / 12.0),
                        };
                        acc.scroll_x += dx;
                        acc.scroll_y += dy;
                    }
                    WindowEvent::Focused(b) => acc.focused = b,
                    WindowEvent::CursorEntered { .. } => acc.cursor_in = true,
                    WindowEvent::CursorLeft { .. } => acc.cursor_in = false,
                    _ => {}
                }
            }
            return;
        }
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
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(dx, dy) => (dx, dy),
                    MouseScrollDelta::PixelDelta(p) => ((p.x as f32) / 12.0, (p.y as f32) / 12.0),
                };
                // Compositing predictivo: desplaza el canvas cacheado (vertical Y horizontal)
                // sin esperar al intérprete (CPU 0). El próximo present() lo resetea y redibuja.
                if dx != 0.0 || dy != 0.0 {
                    let mut g = self.host.inner.lock().unwrap();
                    if !g.last_presented_canvas.is_empty() {
                        if dy != 0.0 {
                            let dpy = (-dy * 40.0) as i32;
                            g.virtual_scroll_y = (g.virtual_scroll_y + dpy).clamp(
                                0,
                                (g.last_presented_h as i32 - g.win_h as i32).max(0),
                            );
                        }
                        if dx != 0.0 {
                            let dpx = (-dx * 40.0) as i32;
                            g.virtual_scroll_x = (g.virtual_scroll_x + dpx).clamp(
                                0,
                                (g.last_presented_w as i32 - g.win_w as i32).max(0),
                            );
                        }
                        let canvas = g.last_presented_canvas.clone();
                        let cw = g.last_presented_w;
                        let ch = g.last_presented_h;
                        let vx = g.virtual_scroll_x;
                        let vy = g.virtual_scroll_y;
                        let bg = g.bg_color;
                        drop(g);
                        self.blit(&canvas, cw, ch, vx, vy, bg);
                    }
                }
                // Acumular para el intérprete (Gui.scrollX/scrollY).
                self.scroll_x += dx;
                self.scroll_y += dy;
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
                self.monitors_dirty = true;  // pudo cambiar la config de pantallas → recolectar de nuevo
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
                    return self.rt_err_kind("TypeError", "Gui.open(title, width, height) requires 3 arguments");
                }
                let title = match self.gui_str_arg(&dot_call.arguments[0]) {
                    Some(s) => s,
                    None => { return self.rt_err_kind("TypeError", "Gui.open title must be a string"); }
                };
                let w = match self.gui_int_arg(&dot_call.arguments[1]) {
                    Some(v) if v > 0 => v as u32,
                    _ => { return self.rt_err_kind("TypeError", "Gui.open width must be a positive integer"); }
                };
                let h = match self.gui_int_arg(&dot_call.arguments[2]) {
                    Some(v) if v > 0 => v as u32,
                    _ => { return self.rt_err_kind("TypeError", "Gui.open height must be a positive integer"); }
                };
                let host = match host() {
                    Some(h) => h.clone(),
                    None => { return self.rt_err_kind("GuiError", "Gui.open: GUI host not initialized"); }
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
                        return self.rt_err_kind("GuiError", "Gui.open: failed to create window");
                    }
                    (g.win_w.max(1), g.win_h.max(1))
                };
                self.gui_state = Some(GuiState::new(ww, wh));
                EvalResult::Value(self.null_ref)
            }

            // ── Multi-ventana ────────────────────────────────────────────────
            // Gui.openWindow(title, w, h) -> id (int ≥ 1). Requiere la ventana
            // principal abierta (Gui.open): su sesión mantiene vivo el event loop.
            "openWindow" => {
                if dot_call.arguments.len() != 3 {
                    return self.rt_err_kind("TypeError", "Gui.openWindow(title, width, height) requires 3 arguments");
                }
                let title = match self.gui_str_arg(&dot_call.arguments[0]) {
                    Some(s) => s,
                    None => { return self.rt_err_kind("TypeError", "Gui.openWindow title must be a string"); }
                };
                let w = match self.gui_int_arg(&dot_call.arguments[1]) {
                    Some(v) if v > 0 => v as u32,
                    _ => { return self.rt_err_kind("TypeError", "Gui.openWindow width must be a positive integer"); }
                };
                let h = match self.gui_int_arg(&dot_call.arguments[2]) {
                    Some(v) if v > 0 => v as u32,
                    _ => { return self.rt_err_kind("TypeError", "Gui.openWindow height must be a positive integer"); }
                };
                if self.gui_state.is_none() {
                    return self.rt_err_kind("GuiError", "Gui.openWindow: open the primary window first (Gui.open)");
                }
                let host = match host() {
                    Some(hh) => hh.clone(),
                    None => { return self.rt_err_kind("GuiError", "Gui.openWindow: GUI host not initialized"); }
                };
                let id = {
                    let st = self.gui_state.as_mut().unwrap();
                    let id = st.next_win_id;
                    st.next_win_id += 1;
                    id
                };
                let (ww, wh) = {
                    let mut g = host.inner.lock().unwrap();
                    g.extra.insert(id, ExtraShared::default());
                    g.cmds.push_back(GuiCmd::OpenExtra { id, title, w, h });
                    host.cv.notify_all();
                    drop(g);
                    host.wake_main();
                    let mut g = host.inner.lock().unwrap();
                    loop {
                        let (ready, failed) = g.extra.get(&id)
                            .map(|e| (e.window_ready, e.open_failed))
                            .unwrap_or((false, true));
                        if ready || failed {
                            if failed {
                                g.extra.remove(&id);
                                drop(g);
                                return self.rt_err_kind("GuiError", "Gui.openWindow: failed to create window");
                            }
                            break;
                        }
                        g = host.cv.wait(g).unwrap();
                    }
                    let e = g.extra.get(&id).unwrap();
                    (e.win_w.max(1), e.win_h.max(1))
                };
                let st = self.gui_state.as_mut().unwrap();
                st.bg_windows.insert(id, WinSlot {
                    open: true,
                    canvas: vec![0u32; ww * wh],
                    width: ww,
                    height: wh,
                    win_w: ww,
                    win_h: wh,
                    bg: 0,
                    clip: (0, 0, ww as i32, wh as i32),
                    clip_stack: Vec::new(),
                    input: InputSnapshot::default(),
                    scale_factor: st.scale_factor,
                    win_x: 0,
                    win_y: 0,
                    scene: Vec::new(),
                    scene_dirty: true,
                });
                EvalResult::Value(self.alloc(ObjectData::Integer(id as i64)))
            }

            // Gui.selectWindow(id): el dibujo y el input pasan a esa ventana
            // (0 = la principal). Todas las primitivas existentes la respetan.
            "selectWindow" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Gui.selectWindow(id) requires 1 argument");
                }
                let id = match self.gui_int_arg(&dot_call.arguments[0]) {
                    Some(v) if v >= 0 => v as u32,
                    _ => { return self.rt_err_kind("TypeError", "Gui.selectWindow id must be a non-negative integer"); }
                };
                match self.gui_state.as_mut() {
                    Some(st) => {
                        if st.switch_to(id) {
                            EvalResult::Value(self.null_ref)
                        } else {
                            self.rt_err_kind("GuiError", &format!("Gui.selectWindow: unknown window id {}", id))
                        }
                    }
                    None => self.rt_err_kind("GuiError", "Gui.selectWindow: no window open"),
                }
            }

            "currentWindow" => {
                let id = self.gui_state.as_ref().map(|s| s.current_win as i64).unwrap_or(0);
                EvalResult::Value(self.alloc(ObjectData::Integer(id)))
            }

            // Gui.closeWindow(id): cierra una ventana extra (id ≥ 1). Si era la
            // seleccionada, la selección vuelve a la principal (0).
            "closeWindow" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Gui.closeWindow(id) requires 1 argument");
                }
                let id = match self.gui_int_arg(&dot_call.arguments[0]) {
                    Some(v) if v >= 1 => v as u32,
                    _ => { return self.rt_err_kind("TypeError", "Gui.closeWindow id must be an integer >= 1"); }
                };
                let host = match host() {
                    Some(hh) => hh.clone(),
                    None => { return self.rt_err_kind("GuiError", "Gui.closeWindow: no GUI host"); }
                };
                if let Some(st) = self.gui_state.as_mut() {
                    if st.current_win == id {
                        st.switch_to(0);
                    }
                    st.bg_windows.remove(&id);
                }
                let mut g = host.inner.lock().unwrap();
                if let Some(e) = g.extra.get_mut(&id) {
                    e.should_close = true;
                }
                g.cmds.push_back(GuiCmd::CloseExtra { id });
                host.cv.notify_all();
                drop(g);
                host.wake_main();
                EvalResult::Value(self.null_ref)
            }

            // ── Modo retenido (scene graph) ──────────────────────────────────
            "nodeRect" | "nodeCircle" | "nodeLine" | "nodeText" | "nodeImage" => {
                let method = dot_call.method.as_str();
                if self.gui_state.is_none() {
                    return self.rt_err_kind("GuiError", &format!("Gui.{}: no window open", method));
                }
                let node = match method {
                    "nodeRect" => {
                        if dot_call.arguments.len() != 5 {
                            return self.rt_err_kind("TypeError", "Gui.nodeRect(x, y, w, h, color) requires 5 arguments");
                        }
                        let a: Vec<Option<i64>> = dot_call.arguments.iter().map(|e| self.gui_int_arg(e)).collect();
                        match (a[0], a[1], a[2], a[3], a[4]) {
                            (Some(x), Some(y), Some(w), Some(h), Some(c)) =>
                                Some((SceneNodeKind::Rect { w: w as i32, h: h as i32 }, x as i32, y as i32, (c as u32) & 0xFF_FFFF)),
                            _ => None,
                        }
                    }
                    "nodeCircle" => {
                        if dot_call.arguments.len() != 4 {
                            return self.rt_err_kind("TypeError", "Gui.nodeCircle(x, y, r, color) requires 4 arguments");
                        }
                        let a: Vec<Option<i64>> = dot_call.arguments.iter().map(|e| self.gui_int_arg(e)).collect();
                        match (a[0], a[1], a[2], a[3]) {
                            (Some(x), Some(y), Some(r), Some(c)) =>
                                Some((SceneNodeKind::Circle { r: r as i32 }, x as i32, y as i32, (c as u32) & 0xFF_FFFF)),
                            _ => None,
                        }
                    }
                    "nodeLine" => {
                        if dot_call.arguments.len() != 5 {
                            return self.rt_err_kind("TypeError", "Gui.nodeLine(x1, y1, x2, y2, color) requires 5 arguments");
                        }
                        let a: Vec<Option<i64>> = dot_call.arguments.iter().map(|e| self.gui_int_arg(e)).collect();
                        match (a[0], a[1], a[2], a[3], a[4]) {
                            (Some(x), Some(y), Some(x2), Some(y2), Some(c)) =>
                                Some((SceneNodeKind::Line { x2: x2 as i32, y2: y2 as i32 }, x as i32, y as i32, (c as u32) & 0xFF_FFFF)),
                            _ => None,
                        }
                    }
                    "nodeText" => {
                        if dot_call.arguments.len() != 5 {
                            return self.rt_err_kind("TypeError", "Gui.nodeText(x, y, text, scale, color) requires 5 arguments");
                        }
                        let x = self.gui_int_arg(&dot_call.arguments[0]);
                        let y = self.gui_int_arg(&dot_call.arguments[1]);
                        let t = self.gui_str_arg(&dot_call.arguments[2]);
                        let s = self.gui_int_arg(&dot_call.arguments[3]);
                        let c = self.gui_int_arg(&dot_call.arguments[4]);
                        match (x, y, t, s, c) {
                            (Some(x), Some(y), Some(t), Some(s), Some(c)) =>
                                Some((SceneNodeKind::Text {
                                    text: t,
                                    scale: s.max(1) as i32,
                                    font: String::new(),
                                    style: 0,
                                    spacing: 0,
                                }, x as i32, y as i32, (c as u32) & 0xFF_FFFF)),
                            _ => None,
                        }
                    }
                    _ => {
                        if dot_call.arguments.len() != 3 {
                            return self.rt_err_kind("TypeError", "Gui.nodeImage(x, y, imageId) requires 3 arguments");
                        }
                        let a: Vec<Option<i64>> = dot_call.arguments.iter().map(|e| self.gui_int_arg(e)).collect();
                        match (a[0], a[1], a[2]) {
                            (Some(x), Some(y), Some(h)) =>
                                Some((SceneNodeKind::Image { handle: h }, x as i32, y as i32, 0)),
                            _ => None,
                        }
                    }
                };
                match node {
                    Some((kind, x, y, color)) => {
                        let st = self.gui_state.as_mut().unwrap();
                        let id = st.scene_add(kind, x, y, color);
                        EvalResult::Value(self.alloc(ObjectData::Integer(id)))
                    }
                    None => self.rt_err_kind("TypeError", &format!("Gui.{}: invalid argument types", method)),
                }
            }

            // Constructores de nodos con paridad de primitivas (serez-ui):
            // nodeRoundRect(x,y,w,h,radius,color), nodeRectAlpha(x,y,w,h,color,alpha),
            // nodeRectOutline(x,y,w,h,color), nodePolygon(points,color),
            // nodePolyline(points,width,color), nodeClipPush(x,y,w,h), nodeClipPop()
            "nodeRoundRect" | "nodeRectAlpha" | "nodeRectOutline" | "nodeClipPush" => {
                let method = dot_call.method.as_str();
                let want = match method {
                    "nodeRoundRect" | "nodeRectAlpha" => 6,
                    "nodeRectOutline" => 5,
                    _ => 4, // nodeClipPush
                };
                if dot_call.arguments.len() != want {
                    return self.rt_err_kind("TypeError", &format!("Gui.{} requires {} arguments", method, want));
                }
                let mut vals = vec![0i64; want];
                for (i, slot) in vals.iter_mut().enumerate() {
                    match self.gui_int_arg(&dot_call.arguments[i]) {
                        Some(v) => *slot = v,
                        None => { return self.rt_err_kind("TypeError", &format!("Gui.{}: all arguments must be integers", method)); }
                    }
                }
                if self.gui_state.is_none() {
                    return self.rt_err_kind("GuiError", &format!("Gui.{}: no window open", method));
                }
                let (kind, color) = match method {
                    "nodeRoundRect" => (
                        SceneNodeKind::RoundRect { w: vals[2] as i32, h: vals[3] as i32, radius: vals[4] as i32 },
                        (vals[5] as u32) & 0xFF_FFFF,
                    ),
                    "nodeRectAlpha" => (
                        SceneNodeKind::RectAlpha { w: vals[2] as i32, h: vals[3] as i32, alpha: vals[5].clamp(0, 255) as u32 },
                        (vals[4] as u32) & 0xFF_FFFF,
                    ),
                    "nodeRectOutline" => (
                        SceneNodeKind::RectOutline { w: vals[2] as i32, h: vals[3] as i32 },
                        (vals[4] as u32) & 0xFF_FFFF,
                    ),
                    _ => (
                        SceneNodeKind::ClipPush { w: vals[2] as i32, h: vals[3] as i32 },
                        0,
                    ),
                };
                let st = self.gui_state.as_mut().unwrap();
                let id = st.scene_add(kind, vals[0] as i32, vals[1] as i32, color);
                EvalResult::Value(self.alloc(ObjectData::Integer(id)))
            }

            "nodePolygon" | "nodePolyline" => {
                let method = dot_call.method.as_str();
                let want = if method == "nodePolygon" { 2 } else { 3 };
                if dot_call.arguments.len() != want {
                    return self.rt_err_kind("TypeError", &format!("Gui.{} requires {} arguments", method, want));
                }
                let pts = match self.gui_int_vec_arg(&dot_call.arguments[0]) {
                    Some(p) => p.iter().map(|v| *v as i32).collect::<Vec<i32>>(),
                    None => { return self.rt_err_kind("TypeError", &format!("Gui.{} points must be a flat int array [x0,y0,x1,y1,…]", method)); }
                };
                let (kind, color) = if method == "nodePolygon" {
                    let c = match self.gui_int_arg(&dot_call.arguments[1]) {
                        Some(v) => (v as u32) & 0xFF_FFFF,
                        None => { return self.rt_err_kind("TypeError", "Gui.nodePolygon color must be an integer"); }
                    };
                    (SceneNodeKind::Polygon { points: pts }, c)
                } else {
                    let w = self.gui_int_arg(&dot_call.arguments[1]);
                    let c = self.gui_int_arg(&dot_call.arguments[2]);
                    match (w, c) {
                        (Some(w), Some(c)) => (
                            SceneNodeKind::Polyline { points: pts, width: w.max(1) as i32 },
                            (c as u32) & 0xFF_FFFF,
                        ),
                        _ => { return self.rt_err_kind("TypeError", "Gui.nodePolyline requires (int[], int, int)"); }
                    }
                };
                match self.gui_state.as_mut() {
                    Some(st) => {
                        let id = st.scene_add(kind, 0, 0, color);
                        EvalResult::Value(self.alloc(ObjectData::Integer(id)))
                    }
                    None => self.rt_err_kind("GuiError", &format!("Gui.{}: no window open", method)),
                }
            }

            "nodeClipPop" => {
                match self.gui_state.as_mut() {
                    Some(st) => {
                        let id = st.scene_add(SceneNodeKind::ClipPop, 0, 0, 0);
                        EvalResult::Value(self.alloc(ObjectData::Integer(id)))
                    }
                    None => self.rt_err_kind("GuiError", "Gui.nodeClipPop: no window open"),
                }
            }

            // Gui.nodeSet(id, prop, value): muta una propiedad de un nodo.
            // props int: x, y, w, h, r, x2, y2, color, z, scale, image, radius,
            //            alpha, width, style, spacing
            // props especiales: text/font (string), visible (bool), points (int[])
            "nodeSet" => {
                if dot_call.arguments.len() != 3 {
                    return self.rt_err_kind("TypeError", "Gui.nodeSet(id, prop, value) requires 3 arguments");
                }
                let id = match self.gui_int_arg(&dot_call.arguments[0]) {
                    Some(v) => v,
                    None => { return self.rt_err_kind("TypeError", "Gui.nodeSet id must be an integer"); }
                };
                let prop = match self.gui_str_arg(&dot_call.arguments[1]) {
                    Some(p) => p,
                    None => { return self.rt_err_kind("TypeError", "Gui.nodeSet prop must be a string"); }
                };
                // El tipo del valor depende de la prop.
                enum V { I(i64), S(String), B(bool), P(Vec<i32>) }
                let value = match prop.as_str() {
                    "text" | "font" => self.gui_str_arg(&dot_call.arguments[2]).map(V::S),
                    "visible" => self.gui_bool_arg(&dot_call.arguments[2]).map(V::B),
                    "points" => self.gui_int_vec_arg(&dot_call.arguments[2])
                        .map(|p| V::P(p.iter().map(|v| *v as i32).collect())),
                    _ => self.gui_int_arg(&dot_call.arguments[2]).map(V::I),
                };
                let value = match value {
                    Some(v) => v,
                    None => { return self.rt_err_kind("TypeError", &format!("Gui.nodeSet: wrong value type for prop '{}'", prop)); }
                };
                let st = match self.gui_state.as_mut() {
                    Some(s) => s,
                    None => { return self.rt_err_kind("GuiError", "Gui.nodeSet: no window open"); }
                };
                let node = match st.scene.iter_mut().find(|n| n.id == id) {
                    Some(n) => n,
                    None => { return self.rt_err_kind("GuiError", &format!("Gui.nodeSet: unknown node id {}", id)); }
                };
                let ok = match (&prop[..], &value, &mut node.kind) {
                    ("x", V::I(v), _) => { node.x = *v as i32; true }
                    ("y", V::I(v), _) => { node.y = *v as i32; true }
                    ("color", V::I(v), _) => { node.color = (*v as u32) & 0xFF_FFFF; true }
                    ("z", V::I(v), _) => { node.z = *v as i32; true }
                    ("visible", V::B(v), _) => { node.visible = *v; true }
                    ("w", V::I(v), SceneNodeKind::Rect { w, .. })
                    | ("w", V::I(v), SceneNodeKind::RectAlpha { w, .. })
                    | ("w", V::I(v), SceneNodeKind::RectOutline { w, .. })
                    | ("w", V::I(v), SceneNodeKind::RoundRect { w, .. })
                    | ("w", V::I(v), SceneNodeKind::ClipPush { w, .. }) => { *w = *v as i32; true }
                    ("h", V::I(v), SceneNodeKind::Rect { h, .. })
                    | ("h", V::I(v), SceneNodeKind::RectAlpha { h, .. })
                    | ("h", V::I(v), SceneNodeKind::RectOutline { h, .. })
                    | ("h", V::I(v), SceneNodeKind::RoundRect { h, .. })
                    | ("h", V::I(v), SceneNodeKind::ClipPush { h, .. }) => { *h = *v as i32; true }
                    ("radius", V::I(v), SceneNodeKind::RoundRect { radius, .. }) => { *radius = *v as i32; true }
                    ("alpha", V::I(v), SceneNodeKind::RectAlpha { alpha, .. }) => { *alpha = (*v).clamp(0, 255) as u32; true }
                    ("r", V::I(v), SceneNodeKind::Circle { r }) => { *r = *v as i32; true }
                    ("x2", V::I(v), SceneNodeKind::Line { x2, .. }) => { *x2 = *v as i32; true }
                    ("y2", V::I(v), SceneNodeKind::Line { y2, .. }) => { *y2 = *v as i32; true }
                    ("points", V::P(v), SceneNodeKind::Polygon { points }) => { *points = v.clone(); true }
                    ("points", V::P(v), SceneNodeKind::Polyline { points, .. }) => { *points = v.clone(); true }
                    ("width", V::I(v), SceneNodeKind::Polyline { width, .. }) => { *width = (*v).max(1) as i32; true }
                    ("text", V::S(v), SceneNodeKind::Text { text, .. }) => { *text = v.clone(); true }
                    ("scale", V::I(v), SceneNodeKind::Text { scale, .. }) => { *scale = (*v).max(1) as i32; true }
                    ("font", V::S(v), SceneNodeKind::Text { font, .. }) => { *font = v.clone(); true }
                    ("style", V::I(v), SceneNodeKind::Text { style, .. }) => { *style = (*v).clamp(0, 15) as u8; true }
                    ("spacing", V::I(v), SceneNodeKind::Text { spacing, .. }) => { *spacing = *v as i32; true }
                    ("image", V::I(v), SceneNodeKind::Image { handle }) => { *handle = *v; true }
                    _ => false,
                };
                if !ok {
                    return self.rt_err_kind("TypeError", &format!("Gui.nodeSet: prop '{}' does not apply to this node", prop));
                }
                st.scene_dirty = true;
                EvalResult::Value(self.null_ref)
            }

            "nodeDelete" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Gui.nodeDelete(id) requires 1 argument");
                }
                let id = match self.gui_int_arg(&dot_call.arguments[0]) {
                    Some(v) => v,
                    None => { return self.rt_err_kind("TypeError", "Gui.nodeDelete id must be an integer"); }
                };
                let existed = match self.gui_state.as_mut() {
                    Some(st) => {
                        let before = st.scene.len();
                        st.scene.retain(|n| n.id != id);
                        let removed = st.scene.len() != before;
                        if removed { st.scene_dirty = true; }
                        removed
                    }
                    None => false,
                };
                EvalResult::Value(if existed { self.true_ref } else { self.false_ref })
            }

            "sceneClear" => {
                if let Some(st) = self.gui_state.as_mut() {
                    st.scene.clear();
                    st.scene_dirty = true;
                }
                EvalResult::Value(self.null_ref)
            }

            "nodeCount" => {
                let n = self.gui_state.as_ref().map(|s| s.scene.len() as i64).unwrap_or(0);
                EvalResult::Value(self.alloc(ObjectData::Integer(n)))
            }

            // Gui.renderScene(bgColor): redibuja la escena SOLO si está sucia (o
            // la ventana cambió de tamaño) y presenta. Devuelve true si redibujó.
            "renderScene" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Gui.renderScene(bgColor) requires 1 argument");
                }
                let bg = match self.gui_int_arg(&dot_call.arguments[0]) {
                    Some(v) => (v as u32) & 0xFF_FFFF,
                    None => { return self.rt_err_kind("TypeError", "Gui.renderScene bgColor must be an integer 0xRRGGBB"); }
                };
                let host = match host() {
                    Some(h) => h.clone(),
                    None => { return self.rt_err_kind("GuiError", "Gui.renderScene: no GUI host"); }
                };
                if self.gui_state.is_none() {
                    return self.rt_err_kind("GuiError", "Gui.renderScene: no window open");
                }
                if self.gui_fonts.is_none() { self.gui_fonts = Some(GuiFonts::new()); }
                let fonts = self.gui_fonts.as_mut().unwrap();
                let st = self.gui_state.as_mut().unwrap();
                let redrew = st.scene_dirty || st.win_w != st.width || st.win_h != st.height;
                if redrew {
                    st.scene_render(fonts, bg);
                }
                if st.current_win == 0 {
                    st.present(&host);
                } else {
                    let id = st.current_win;
                    st.present_extra(&host, id);
                }
                EvalResult::Value(if redrew { self.true_ref } else { self.false_ref })
            }

            "isOpen" => {
                let open = match self.gui_state.as_ref() {
                    None => false,
                    Some(st) if st.current_win == 0 => host().map(|h| {
                        let g = h.inner.lock().unwrap();
                        g.window_open && !g.should_close
                    }).unwrap_or(false),
                    Some(st) => host().map(|h| {
                        let g = h.inner.lock().unwrap();
                        g.extra.get(&st.current_win)
                            .map(|e| e.window_open && !e.should_close)
                            .unwrap_or(false)
                    }).unwrap_or(false),
                };
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
                let host = match host() { Some(h) => h.clone(), None => { return self.rt_err_kind("GuiError", "Gui.present: no GUI host"); } };
                match self.gui_state.as_mut() {
                    Some(st) => {
                        if st.current_win == 0 {
                            st.present(&host);
                        } else {
                            let id = st.current_win;
                            st.present_extra(&host, id);
                        }
                        EvalResult::Value(self.null_ref)
                    }
                    None => { self.rt_err_kind("GuiError", "Gui.present: no window open") }
                }
            }

            "clear" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Gui.clear(color) requires 1 argument");
                }
                let color = match self.gui_int_arg(&dot_call.arguments[0]) {
                    Some(v) => (v as u32) & 0x00FF_FFFF,
                    None => { return self.rt_err_kind("TypeError", "Gui.clear color must be an integer 0xRRGGBB"); }
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
                    None => { self.rt_err_kind("GuiError", "Gui.clear: no window open (call Gui.open first)") }
                }
            }

            "fillRect" => {
                let (x, y, w, h, color) = match self.gui_rect_args(dot_call) {
                    Some(v) => v, None => return EvalResult::Error,
                };
                match self.gui_state.as_mut() {
                    Some(st) => { st.fill_rect(x as i32, y as i32, w as i32, h as i32, color); EvalResult::Value(self.null_ref) }
                    None => { self.rt_err_kind("GuiError", "Gui.fillRect: no window open") }
                }
            }

            "fillRectAlpha" => {
                if dot_call.arguments.len() != 6 {
                    return self.rt_err_kind("TypeError", "Gui.fillRectAlpha(x, y, w, h, color, alpha) requires 6 arguments");
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
                    _ => { return self.rt_err_kind("TypeError", "Gui.fillRectAlpha requires 6 integers"); }
                };
                match self.gui_state.as_mut() {
                    Some(st) => {
                        let r = ((color >> 16) & 0xff) as u8;
                        let g = ((color >> 8) & 0xff) as u8;
                        let b = (color & 0xff) as u8;
                        st.blend_rect(x, y, w, h, r, g, b, alpha);
                        EvalResult::Value(self.null_ref)
                    }
                    None => { self.rt_err_kind("GuiError", "Gui.fillRectAlpha: no window open") }
                }
            }

            "setPixel" => {
                if dot_call.arguments.len() != 3 {
                    return self.rt_err_kind("TypeError", "Gui.setPixel(x, y, color) requires 3 arguments");
                }
                let x = self.gui_int_arg(&dot_call.arguments[0]);
                let y = self.gui_int_arg(&dot_call.arguments[1]);
                let c = self.gui_int_arg(&dot_call.arguments[2]);
                let (x, y, color) = match (x, y, c) {
                    (Some(x), Some(y), Some(c)) => (x as i32, y as i32, (c as u32) & 0x00FF_FFFF),
                    _ => { return self.rt_err_kind("TypeError", "Gui.setPixel requires 3 integers"); }
                };
                match self.gui_state.as_mut() {
                    Some(st) => { st.put(x, y, color); EvalResult::Value(self.null_ref) }
                    None => { self.rt_err_kind("GuiError", "Gui.setPixel: no window open") }
                }
            }

            "drawLine" => {
                if dot_call.arguments.len() != 5 {
                    return self.rt_err_kind("TypeError", "Gui.drawLine(x0, y0, x1, y1, color) requires 5 arguments");
                }
                let x0 = self.gui_int_arg(&dot_call.arguments[0]);
                let y0 = self.gui_int_arg(&dot_call.arguments[1]);
                let x1 = self.gui_int_arg(&dot_call.arguments[2]);
                let y1 = self.gui_int_arg(&dot_call.arguments[3]);
                let c  = self.gui_int_arg(&dot_call.arguments[4]);
                let (x0, y0, x1, y1, color) = match (x0, y0, x1, y1, c) {
                    (Some(a), Some(b), Some(c), Some(d), Some(e)) => (a as i32, b as i32, c as i32, d as i32, (e as u32) & 0x00FF_FFFF),
                    _ => { return self.rt_err_kind("TypeError", "Gui.drawLine requires 5 integers"); }
                };
                match self.gui_state.as_mut() {
                    Some(st) => { st.draw_line(x0, y0, x1, y1, color); EvalResult::Value(self.null_ref) }
                    None => { self.rt_err_kind("GuiError", "Gui.drawLine: no window open") }
                }
            }

            "drawText" => {
                // Aditivo: 5 args = (x,y,text,scale,color) estilo normal (como antes);
                //          6 args = + style (bitfield: 1=bold, 2=italic, 4=subrayado, 8=tachado);
                //          7 args = + letterSpacing (px extra entre caracteres).
                // Nota: measureText/textAdvances NO incluyen letterSpacing; si se usa, sumar
                // (nChars-1)*spacing aparte para situar el caret.
                let n = dot_call.arguments.len();
                if n < 5 || n > 7 {
                    return self.rt_err_kind("TypeError", "Gui.drawText(x, y, text, scale, color [, style [, letterSpacing]]) requires 5 to 7 arguments");
                }
                let x     = self.gui_int_arg(&dot_call.arguments[0]);
                let y     = self.gui_int_arg(&dot_call.arguments[1]);
                let text  = self.gui_str_arg(&dot_call.arguments[2]);
                let scale = self.gui_int_arg(&dot_call.arguments[3]);
                let c     = self.gui_int_arg(&dot_call.arguments[4]);
                let style = if n >= 6 { self.gui_int_arg(&dot_call.arguments[5]).unwrap_or(0) } else { 0 };
                let spacing = if n == 7 { self.gui_int_arg(&dot_call.arguments[6]).unwrap_or(0) } else { 0 };
                let (x, y, text, scale, color) = match (x, y, text, scale, c) {
                    (Some(x), Some(y), Some(t), Some(s), Some(c)) =>
                        (x as i32, y as i32, t, s.max(1) as i32, (c as u32) & 0x00FF_FFFF),
                    _ => { return self.rt_err_kind("TypeError", "Gui.drawText requires (int, int, string, int, int [, int [, int]])"); }
                };
                if self.gui_state.is_none() {
                    return self.rt_err_kind("GuiError", "Gui.drawText: no window open");
                }
                if self.gui_fonts.is_none() { self.gui_fonts = Some(GuiFonts::new()); }
                let fonts = self.gui_fonts.as_mut().unwrap();
                let st = self.gui_state.as_mut().unwrap();
                st.draw_text(fonts, x, y, &text, scale, color, (style.clamp(0, 15)) as u8, spacing as i32);
                EvalResult::Value(self.null_ref)
            }

            "measureText" => {
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind("TypeError", "Gui.measureText(text, scale) requires 2 arguments");
                }
                let text  = self.gui_str_arg(&dot_call.arguments[0]);
                let scale = self.gui_int_arg(&dot_call.arguments[1]);
                let (text, scale) = match (text, scale) {
                    (Some(t), Some(s)) => (t, s.max(1)),
                    _ => { return self.rt_err_kind("TypeError", "Gui.measureText requires (string, int)"); }
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

            // ── Motor de primitivos (Fase 0/1) ──────────────────────────────────
            "loadStylesheet" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Gui.loadStylesheet(src) requires 1 argument");
                }
                let src = match self.gui_str_arg(&dot_call.arguments[0]) {
                    Some(s) => s,
                    None => { return self.rt_err_kind("TypeError", "Gui.loadStylesheet src must be a string"); }
                };
                self.gui_stylesheets.push(parse_css(&src));
                let handle = self.gui_stylesheets.len() as i64; // 1-based
                EvalResult::Value(self.alloc(ObjectData::Integer(handle)))
            }

            // Parsea markup SVG (o lee un archivo .svg) → handle rasterizable con el
            // primitivo `svg` (["svg", [["src", handle],["width",W],["height",H]], []]).
            "loadSvg" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Gui.loadSvg(srcOrPath) requires 1 argument");
                }
                let arg = match self.gui_str_arg(&dot_call.arguments[0]) {
                    Some(s) => s,
                    None => { return self.rt_err_kind("TypeError", "Gui.loadSvg argument must be a string"); }
                };
                // Markup directo si empieza por '<'; si no, se trata como ruta de archivo.
                let markup = if arg.trim_start().starts_with('<') {
                    arg
                } else {
                    match std::fs::read_to_string(&arg) {
                        Ok(s) => s,
                        Err(e) => { return self.rt_err_kind("IOError", format!("Gui.loadSvg: could not read '{}': {}", arg, e)); }
                    }
                };
                match svg::parse(&markup) {
                    Some(p) => {
                        self.gui_svgs.push(p);
                        let handle = self.gui_svgs.len() as i64; // 1-based
                        EvalResult::Value(self.alloc(ObjectData::Integer(handle)))
                    }
                    None => self.rt_err_kind("GuiError", "Gui.loadSvg: could not parse SVG (empty or unsupported)"),
                }
            }

            // Layout + match CSS + emit escena, todo nativo. root = árbol de primitivos
            // (Array anidado [tag, [[prop,val]…], [hijo|texto…]]). Devuelve #regions.
            "renderTree" => {
                if dot_call.arguments.len() < 4 || dot_call.arguments.len() > 5 {
                    return self.rt_err_kind("TypeError", "Gui.renderTree(root, sheet, w, h[, ctx]) requires 4-5 arguments");
                }
                let root_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => v,
                    other => return other,
                };
                let sheet_h = self.gui_int_arg(&dot_call.arguments[1]).unwrap_or(0);
                let w = self.gui_int_arg(&dot_call.arguments[2]).unwrap_or(0) as i32;
                let h = self.gui_int_arg(&dot_call.arguments[3]).unwrap_or(0) as i32;
                let ctx: Vec<(String, String)> = if dot_call.arguments.len() >= 5 {
                    self.gui_read_ctx(&dot_call.arguments[4])
                } else { Vec::new() };
                if self.gui_state.is_none() {
                    return self.rt_err_kind("GuiError", "Gui.renderTree: no window open");
                }
                let mut st = self.gui_state.take().unwrap();
                // Sacamos las fuentes afuera para medir texto proporcional durante el
                // layout (borrow mutable de un local, ajeno al arena de self).
                let mut fonts = self.gui_fonts.take();
                st.scene.clear();
                st.prim_clip = None;
                st.win_w = w.max(1) as usize;
                st.win_h = h.max(1) as usize;
                st.scene_dirty = true;
                let mut regions: Vec<PrimRegion> = Vec::new();
                {
                    let sheet_ref = if sheet_h >= 1 {
                        self.gui_stylesheets.get((sheet_h - 1) as usize)
                    } else { None };
                    let svgs_ref: &[svg::ParsedSvg] = &self.gui_svgs;
                    if let Some(ObjectData::Array { elements, .. }) = self.resolve(root_ref) {
                        if elements.len() >= 3 {
                            if let OwnedValue::Str(tag) = &elements[0] {
                                let style = if let OwnedValue::Array { elements, .. } = &elements[1] { elements.as_slice() } else { &[] };
                                let kids = if let OwnedValue::Array { elements, .. } = &elements[2] { elements.as_slice() } else { &[] };
                                // Marco raíz: toda la ventana; el containing block
                                // inicial (para absolute sin ancestro posicionado)
                                // es la ventana misma.
                                let root = PrimFrame {
                                    x: 0, y: 0, avail_w: w,
                                    depth: 0, z_off: 0,
                                    cb_x: 0, cb_y: 0, cb_w: w, cb_h: h,
                                };
                                let mut pcx = PrimCtx {
                                    sheet: sheet_ref,
                                    svgs: svgs_ref,
                                    ctx: &ctx,
                                    fonts: &mut fonts,
                                    st: &mut st,
                                    regions: &mut regions,
                                };
                                prim_render(tag.as_str(), style, kids, root, &mut pcx);
                            }
                        }
                    }
                }
                self.gui_fonts = fonts;
                self.gui_state = Some(st);
                // Regions → Array de [tag, x, y, w, h, onClick|null] para hit-testing en .sz.
                let mut arr: Vec<OwnedValue> = Vec::with_capacity(regions.len());
                for r in regions {
                    let onclick = r.onclick.unwrap_or(OwnedValue::Null);
                    arr.push(OwnedValue::Array {
                        element_type: None,
                        elements: vec![
                            OwnedValue::Str(r.tag),
                            OwnedValue::Integer(r.x as i64),
                            OwnedValue::Integer(r.y as i64),
                            OwnedValue::Integer(r.w as i64),
                            OwnedValue::Integer(r.h as i64),
                            onclick,
                        ],
                    });
                }
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: arr }))
            }

            "loadFont" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Gui.loadFont(path) requires 1 argument");
                }
                let path = match self.gui_str_arg(&dot_call.arguments[0]) {
                    Some(s) => s,
                    None => { return self.rt_err_kind("TypeError", "Gui.loadFont path must be a string"); }
                };
                if self.gui_fonts.is_none() { self.gui_fonts = Some(GuiFonts::new()); }
                match self.gui_fonts.as_mut().unwrap().load_font_file(&path) {
                    Some(family) => EvalResult::Value(self.alloc(ObjectData::Str(family))),
                    None => { self.rt_err_kind("GuiError", format!("Gui.loadFont: could not load font file '{}'", path)) }
                }
            }

            "setFont" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Gui.setFont(family) requires 1 argument");
                }
                let name = match self.gui_str_arg(&dot_call.arguments[0]) {
                    Some(s) => s,
                    None => { return self.rt_err_kind("TypeError", "Gui.setFont family must be a string"); }
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
                    return self.rt_err_kind("TypeError", "Gui.fillRoundRect(x, y, w, h, radius, color) requires 6 arguments");
                }
                let mut vals = [0i64; 6];
                for (i, slot) in vals.iter_mut().enumerate() {
                    match self.gui_int_arg(&dot_call.arguments[i]) {
                        Some(v) => *slot = v,
                        None => { return self.rt_err_kind("TypeError", "Gui.fillRoundRect requires 6 integers"); }
                    }
                }
                let color = (vals[5] as u32) & 0x00FF_FFFF;
                match self.gui_state.as_mut() {
                    Some(st) => {
                        st.fill_round_rect(vals[0] as i32, vals[1] as i32, vals[2] as i32, vals[3] as i32, vals[4] as i32, color);
                        EvalResult::Value(self.null_ref)
                    }
                    None => { self.rt_err_kind("GuiError", "Gui.fillRoundRect: no window open") }
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
                    return self.rt_err_kind("TypeError", "Gui.drawRect(x, y, w, h, color) requires 5 arguments");
                }
                let mut vals = [0i64; 5];
                for (i, slot) in vals.iter_mut().enumerate() {
                    match self.gui_int_arg(&dot_call.arguments[i]) {
                        Some(v) => *slot = v,
                        None => { return self.rt_err_kind("TypeError", "Gui.drawRect requires 5 integers"); }
                    }
                }
                let color = (vals[4] as u32) & 0x00FF_FFFF;
                match self.gui_state.as_mut() {
                    Some(st) => { st.draw_rect(vals[0] as i32, vals[1] as i32, vals[2] as i32, vals[3] as i32, color); EvalResult::Value(self.null_ref) }
                    None => { self.rt_err_kind("GuiError", "Gui.drawRect: no window open") }
                }
            }

            "fillCircle" => {
                if dot_call.arguments.len() != 4 {
                    return self.rt_err_kind("TypeError", "Gui.fillCircle(cx, cy, radius, color) requires 4 arguments");
                }
                let mut vals = [0i64; 4];
                for (i, slot) in vals.iter_mut().enumerate() {
                    match self.gui_int_arg(&dot_call.arguments[i]) {
                        Some(v) => *slot = v,
                        None => { return self.rt_err_kind("TypeError", "Gui.fillCircle requires 4 integers"); }
                    }
                }
                let color = (vals[3] as u32) & 0x00FF_FFFF;
                match self.gui_state.as_mut() {
                    Some(st) => { st.fill_circle(vals[0] as i32, vals[1] as i32, vals[2] as i32, color); EvalResult::Value(self.null_ref) }
                    None => { self.rt_err_kind("GuiError", "Gui.fillCircle: no window open") }
                }
            }

            // Contorno de círculo (1px).
            "drawCircle" => {
                if dot_call.arguments.len() != 4 {
                    return self.rt_err_kind("TypeError", "Gui.drawCircle(cx, cy, radius, color) requires 4 arguments");
                }
                let mut vals = [0i64; 4];
                for (i, slot) in vals.iter_mut().enumerate() {
                    match self.gui_int_arg(&dot_call.arguments[i]) {
                        Some(v) => *slot = v,
                        None => { return self.rt_err_kind("TypeError", "Gui.drawCircle requires 4 integers"); }
                    }
                }
                let color = (vals[3] as u32) & 0x00FF_FFFF;
                match self.gui_state.as_mut() {
                    Some(st) => { st.draw_circle(vals[0] as i32, vals[1] as i32, vals[2] as i32, color); EvalResult::Value(self.null_ref) }
                    None => { self.rt_err_kind("GuiError", "Gui.drawCircle: no window open") }
                }
            }

            // Línea de grosor configurable (extremos/juntas redondeados, antialiased).
            "drawLineThick" => {
                if dot_call.arguments.len() != 6 {
                    return self.rt_err_kind("TypeError", "Gui.drawLineThick(x0, y0, x1, y1, width, color) requires 6 arguments");
                }
                let mut vals = [0i64; 6];
                for (i, slot) in vals.iter_mut().enumerate() {
                    match self.gui_int_arg(&dot_call.arguments[i]) {
                        Some(v) => *slot = v,
                        None => { return self.rt_err_kind("TypeError", "Gui.drawLineThick requires 6 integers"); }
                    }
                }
                let color = (vals[5] as u32) & 0x00FF_FFFF;
                match self.gui_state.as_mut() {
                    Some(st) => { st.draw_thick_line(vals[0] as i32, vals[1] as i32, vals[2] as i32, vals[3] as i32, vals[4] as i32, color); EvalResult::Value(self.null_ref) }
                    None => { self.rt_err_kind("GuiError", "Gui.drawLineThick: no window open") }
                }
            }

            // Polilínea: segmentos conectados a partir de un arreglo plano [x0,y0,x1,y1,…].
            // Gui.drawPolyline(points, width, color).
            "drawPolyline" => {
                if dot_call.arguments.len() != 3 {
                    return self.rt_err_kind("TypeError", "Gui.drawPolyline(points, width, color) requires 3 arguments");
                }
                let pts = match self.gui_int_vec_arg(&dot_call.arguments[0]) {
                    Some(p) => p,
                    None => { return self.rt_err_kind("TypeError", "Gui.drawPolyline points must be a flat int array [x0,y0,x1,y1,…]"); }
                };
                let width = self.gui_int_arg(&dot_call.arguments[1]);
                let color = self.gui_int_arg(&dot_call.arguments[2]);
                let (width, color) = match (width, color) {
                    (Some(w), Some(c)) => (w.max(1) as i32, (c as u32) & 0x00FF_FFFF),
                    _ => { return self.rt_err_kind("TypeError", "Gui.drawPolyline requires (int[], int, int)"); }
                };
                match self.gui_state.as_mut() {
                    Some(st) => {
                        let mut i = 0;
                        while i + 3 < pts.len() {
                            st.draw_thick_line(pts[i] as i32, pts[i + 1] as i32, pts[i + 2] as i32, pts[i + 3] as i32, width, color);
                            i += 2;
                        }
                        EvalResult::Value(self.null_ref)
                    }
                    None => { self.rt_err_kind("GuiError", "Gui.drawPolyline: no window open") }
                }
            }

            // Polígono relleno (regla par-impar) a partir de un arreglo plano [x0,y0,x1,y1,…].
            // Gui.fillPolygon(points, color).
            "fillPolygon" => {
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind("TypeError", "Gui.fillPolygon(points, color) requires 2 arguments");
                }
                let pts = match self.gui_int_vec_arg(&dot_call.arguments[0]) {
                    Some(p) => p,
                    None => { return self.rt_err_kind("TypeError", "Gui.fillPolygon points must be a flat int array [x0,y0,x1,y1,…]"); }
                };
                let color = match self.gui_int_arg(&dot_call.arguments[1]) {
                    Some(c) => (c as u32) & 0x00FF_FFFF,
                    None => { return self.rt_err_kind("TypeError", "Gui.fillPolygon color must be an integer"); }
                };
                let verts: Vec<(i32, i32)> = pts.chunks_exact(2).map(|c| (c[0] as i32, c[1] as i32)).collect();
                match self.gui_state.as_mut() {
                    Some(st) => { st.fill_polygon(&verts, color); EvalResult::Value(self.null_ref) }
                    None => { self.rt_err_kind("GuiError", "Gui.fillPolygon: no window open") }
                }
            }

            "setImePosition" => {
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind("TypeError", "Gui.setImePosition(x, y) requires 2 arguments");
                }
                let x = self.gui_int_arg(&dot_call.arguments[0]);
                let y = self.gui_int_arg(&dot_call.arguments[1]);
                let (x, y) = match (x, y) {
                    (Some(x), Some(y)) => (x as i32, y as i32),
                    _ => { return self.rt_err_kind("TypeError", "Gui.setImePosition requires 2 integers"); }
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
                    return self.rt_err_kind("TypeError", "Gui.keyDown(name) requires 1 argument");
                }
                let name = match self.gui_str_arg(&dot_call.arguments[0]) {
                    Some(s) => s,
                    None => { return self.rt_err_kind("TypeError", "Gui.keyDown name must be a string"); }
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
            // Posición outer de la ventana en píxeles físicos: [x, y]. Para centrar /
            // recordar dónde estaba la ventana, o posicionar relativo a un monitor.
            "windowPosition" => {
                let (x, y) = self.gui_state.as_ref().map(|s| (s.win_x, s.win_y)).unwrap_or((0, 0));
                let elems = vec![OwnedValue::Integer(x as i64), OwnedValue::Integer(y as i64)];
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: Some("int".to_string()), elements: elems }))
            }
            // Monitores conectados: array de dicts {x, y, width, height, scale, name}
            // (posición + resolución en píxeles físicos). Para multi-monitor y centrado.
            "monitors" => {
                let mons = self.gui_state.as_ref().map(|s| s.monitors.clone()).unwrap_or_default();
                let mut elems: Vec<OwnedValue> = Vec::with_capacity(mons.len());
                for m in mons {
                    elems.push(OwnedValue::Dict {
                        key_type: "string".to_string(),
                        value_type: "any".to_string(),
                        entries: vec![
                            (OwnedValue::Str("x".to_string()), OwnedValue::Integer(m.x as i64)),
                            (OwnedValue::Str("y".to_string()), OwnedValue::Integer(m.y as i64)),
                            (OwnedValue::Str("width".to_string()), OwnedValue::Integer(m.w as i64)),
                            (OwnedValue::Str("height".to_string()), OwnedValue::Integer(m.h as i64)),
                            (OwnedValue::Str("scale".to_string()), OwnedValue::Decimal(m.scale)),
                            (OwnedValue::Str("name".to_string()), OwnedValue::Str(m.name)),
                        ],
                    });
                }
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: Some("dict".to_string()), elements: elems }))
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
                    return self.rt_err_kind("TypeError", "Gui.clipboardSet(text) requires 1 argument");
                }
                let text = self.gui_str_arg(&dot_call.arguments[0]).unwrap_or_default();
                if let Some(st) = self.gui_state.as_mut() {
                    if st.clipboard.is_none() { st.clipboard = arboard::Clipboard::new().ok(); }
                    if let Some(c) = st.clipboard.as_mut() { let _ = c.set_text(text); }
                }
                EvalResult::Value(self.null_ref)
            }
            // Lee una imagen del portapapeles (RGBA) y la registra como handle (como loadImage).
            // Devuelve 0 si el portapapeles no contiene una imagen.
            "clipboardGetImage" => {
                let img = match self.gui_state.as_mut() {
                    Some(st) => {
                        if st.clipboard.is_none() { st.clipboard = arboard::Clipboard::new().ok(); }
                        st.clipboard.as_mut().and_then(|c| c.get_image().ok())
                    }
                    None => None,
                };
                match img {
                    Some(im) => {
                        let (w, h) = (im.width, im.height);
                        let bytes = im.bytes; // Cow<[u8]> en RGBA
                        let mut px = Vec::with_capacity(w * h);
                        let mut i = 0;
                        while i + 3 < bytes.len() {
                            let r = bytes[i] as u32;
                            let g = bytes[i + 1] as u32;
                            let b = bytes[i + 2] as u32;
                            let a = bytes[i + 3] as u32;
                            px.push((a << 24) | (r << 16) | (g << 8) | b);
                            i += 4;
                        }
                        match self.gui_state.as_mut() {
                            Some(st) => {
                                let id = st.next_image;
                                st.next_image += 1;
                                st.images.insert(id, ImageData { w, h, px });
                                EvalResult::Value(self.int_ref(id))
                            }
                            None => EvalResult::Value(self.int_ref(0)),
                        }
                    }
                    None => EvalResult::Value(self.int_ref(0)),
                }
            }
            // Copia una imagen (por handle, como devuelve loadImage/loadImageBytes) al portapapeles.
            "clipboardSetImage" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Gui.clipboardSetImage(handle) requires 1 argument");
                }
                let hnd = match self.gui_int_arg(&dot_call.arguments[0]) {
                    Some(h) => h,
                    None => { return self.rt_err_kind("TypeError", "Gui.clipboardSetImage handle must be an integer"); }
                };
                // ARGB u32 (interno) → RGBA bytes (lo que espera arboard).
                let rgba = self.gui_state.as_ref().and_then(|s| s.images.get(&hnd)).map(|im| {
                    let mut bytes = Vec::with_capacity(im.px.len() * 4);
                    for &p in &im.px {
                        bytes.push(((p >> 16) & 0xff) as u8); // r
                        bytes.push(((p >> 8) & 0xff) as u8);  // g
                        bytes.push((p & 0xff) as u8);         // b
                        bytes.push(((p >> 24) & 0xff) as u8); // a
                    }
                    (im.w, im.h, bytes)
                });
                match rgba {
                    Some((w, h, bytes)) => {
                        if let Some(st) = self.gui_state.as_mut() {
                            if st.clipboard.is_none() { st.clipboard = arboard::Clipboard::new().ok(); }
                            if let Some(c) = st.clipboard.as_mut() {
                                let _ = c.set_image(arboard::ImageData { width: w, height: h, bytes: std::borrow::Cow::Owned(bytes) });
                            }
                        }
                        EvalResult::Value(self.null_ref)
                    }
                    None => {
                        self.rt_err_kind("GuiError", "Gui.clipboardSetImage: invalid image handle")
                    }
                }
            }

            "loadImage" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Gui.loadImage(path) requires 1 argument");
                }
                let path = match self.gui_str_arg(&dot_call.arguments[0]) {
                    Some(s) => s,
                    None => { return self.rt_err_kind("TypeError", "Gui.loadImage path must be a string"); }
                };
                let decoded = match image::open(&path) {
                    Ok(img) => img.to_rgba8(),
                    Err(e) => { return self.rt_err_kind("GuiError", format!("Gui.loadImage: {}", e)); }
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
                    None => { self.rt_err_kind("GuiError", "Gui.loadImage: no window open") }
                }
            }

            "loadImageBytes" => {
                // Igual que loadImage pero decodifica desde un arreglo de bytes en memoria
                // (0–255, como devuelve File binario / fetch / Binary.*), no desde una ruta.
                // Sirve para imágenes fetcheadas o generadas sin tocar el disco.
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Gui.loadImageBytes(bytes) requires 1 argument");
                }
                let r = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    other => return other,
                };
                let elems = match self.resolve(r) {
                    Some(ObjectData::Array { elements, .. }) => elements.clone(),
                    _ => { return self.rt_err_kind("TypeError", "Gui.loadImageBytes: argument must be a byte array"); }
                };
                let mut bytes = Vec::with_capacity(elems.len());
                for elem in elems {
                    match elem {
                        OwnedValue::Integer(b) => bytes.push(b as u8),
                        _ => { return self.rt_err_kind("TypeError", "Gui.loadImageBytes: all elements must be integers (0–255)"); }
                    }
                }
                let decoded = match image::load_from_memory(&bytes) {
                    Ok(img) => img.to_rgba8(),
                    Err(e) => { return self.rt_err_kind("GuiError", format!("Gui.loadImageBytes: {}", e)); }
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
                    None => { self.rt_err_kind("GuiError", "Gui.loadImageBytes: no window open") }
                }
            }

            "drawImage" => {
                // Aditivo: 3 args = (x,y,handle) tamaño nativo (como antes);
                //          5 args = (x,y,handle,w,h) escalado;
                //          6 args = (x,y,handle,w,h,alpha) escalado + alpha global 0–255.
                let n = dot_call.arguments.len();
                if n != 3 && n != 5 && n != 6 {
                    return self.rt_err_kind("TypeError", "Gui.drawImage requires (x,y,handle) | (x,y,handle,w,h) | (x,y,handle,w,h,alpha)");
                }
                let mut vals = [0i64; 6];
                vals[5] = 255; // alpha por defecto
                for i in 0..n {
                    match self.gui_int_arg(&dot_call.arguments[i]) {
                        Some(v) => vals[i] = v,
                        None => { return self.rt_err_kind("TypeError", "Gui.drawImage requires integers"); }
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
                    None => { self.rt_err_kind("GuiError", "Gui.drawImage: no window open") }
                }
            }

            "fillGradient" => {
                if dot_call.arguments.len() != 7 {
                    return self.rt_err_kind("TypeError", "Gui.fillGradient(x, y, w, h, color1, color2, vertical) requires 7 arguments");
                }
                let mut vals = [0i64; 6];
                for (i, slot) in vals.iter_mut().enumerate() {
                    match self.gui_int_arg(&dot_call.arguments[i]) {
                        Some(v) => *slot = v,
                        None => { return self.rt_err_kind("TypeError", "Gui.fillGradient requires 6 integers (x,y,w,h,color1,color2) + vertical(bool)"); }
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
                    None => { self.rt_err_kind("GuiError", "Gui.fillGradient: no window open") }
                }
            }

            "blur" => {
                if dot_call.arguments.len() != 5 {
                    return self.rt_err_kind("TypeError", "Gui.blur(x, y, w, h, radius) requires 5 arguments");
                }
                let mut vals = [0i64; 5];
                for (i, slot) in vals.iter_mut().enumerate() {
                    match self.gui_int_arg(&dot_call.arguments[i]) {
                        Some(v) => *slot = v,
                        None => { return self.rt_err_kind("TypeError", "Gui.blur requires 5 integers"); }
                    }
                }
                match self.gui_state.as_mut() {
                    Some(st) => { st.blur_region(vals[0] as i32, vals[1] as i32, vals[2] as i32, vals[3] as i32, vals[4] as i32); EvalResult::Value(self.null_ref) }
                    None => { self.rt_err_kind("GuiError", "Gui.blur: no window open") }
                }
            }

            "scaleFactor" => {
                let sf = self.gui_state.as_ref().map(|s| s.scale_factor).unwrap_or(1.0);
                EvalResult::Value(self.alloc(ObjectData::Decimal(sf)))
            }

            "textAdvances" => {
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind("TypeError", "Gui.textAdvances(text, scale) requires 2 arguments");
                }
                let text  = self.gui_str_arg(&dot_call.arguments[0]);
                let scale = self.gui_int_arg(&dot_call.arguments[1]);
                let (text, scale) = match (text, scale) {
                    (Some(t), Some(s)) => (t, s.max(1) as i32),
                    _ => { return self.rt_err_kind("TypeError", "Gui.textAdvances requires (string, int)"); }
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
                    return self.rt_err_kind("TypeError", "Gui.setWindowIcon(path) requires 1 argument");
                }
                let path = match self.gui_str_arg(&dot_call.arguments[0]) {
                    Some(s) => s,
                    None => { return self.rt_err_kind("TypeError", "Gui.setWindowIcon path must be a string"); }
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
                        Err(e) => { return self.rt_err_kind("GuiError", format!("Gui.setWindowIcon: {}", e)); }
                    }
                };
                if let Some(host) = host() {
                    host.inner.lock().unwrap().cmds.push_back(cmd);
                    host.cv.notify_all();
                }
                EvalResult::Value(self.null_ref)
            }
            // Cursor del mouse desde un archivo de imagen, con punto caliente (hotspot).
            // Gui.setCursorImage(path, hotspotX, hotspotY); "" = restaurar el cursor por defecto.
            "setCursorImage" => {
                if dot_call.arguments.len() != 3 {
                    return self.rt_err_kind("TypeError", "Gui.setCursorImage(path, hotspotX, hotspotY) requires 3 arguments");
                }
                let path = match self.gui_str_arg(&dot_call.arguments[0]) {
                    Some(s) => s,
                    None => { return self.rt_err_kind("TypeError", "Gui.setCursorImage path must be a string"); }
                };
                let hx = self.gui_int_arg(&dot_call.arguments[1]);
                let hy = self.gui_int_arg(&dot_call.arguments[2]);
                let (hx, hy) = match (hx, hy) {
                    (Some(a), Some(b)) => (a.max(0) as u32, b.max(0) as u32),
                    _ => { return self.rt_err_kind("TypeError", "Gui.setCursorImage hotspotX/hotspotY must be integers"); }
                };
                let cmd = if path.is_empty() {
                    GuiCmd::SetCustomCursor(Vec::new(), 0, 0, 0, 0)
                } else {
                    match image::open(&path) {
                        Ok(img) => {
                            let rgba = img.to_rgba8();
                            let (w, h) = (rgba.width(), rgba.height());
                            GuiCmd::SetCustomCursor(rgba.into_raw(), w, h, hx.min(w.saturating_sub(1)), hy.min(h.saturating_sub(1)))
                        }
                        Err(e) => { return self.rt_err_kind("GuiError", format!("Gui.setCursorImage: {}", e)); }
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
                    return self.rt_err_kind("TypeError", "Gui.idleWait(maxMs) requires 1 argument");
                }
                let ms = match self.gui_int_arg(&dot_call.arguments[0]) {
                    Some(v) => v.max(0) as u64,
                    None => { return self.rt_err_kind("TypeError", "Gui.idleWait maxMs must be an integer"); }
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
                    return self.rt_err_kind("TypeError", "Gui.imageSize(handle) requires 1 argument");
                }
                let hnd = match self.gui_int_arg(&dot_call.arguments[0]) {
                    Some(h) => h,
                    None => { return self.rt_err_kind("TypeError", "Gui.imageSize handle must be an integer"); }
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
                    return self.rt_err_kind("TypeError", "Gui.pushClip(x, y, w, h) requires 4 arguments");
                }
                let x = self.gui_int_arg(&dot_call.arguments[0]);
                let y = self.gui_int_arg(&dot_call.arguments[1]);
                let w = self.gui_int_arg(&dot_call.arguments[2]);
                let h = self.gui_int_arg(&dot_call.arguments[3]);
                let (x, y, w, h) = match (x, y, w, h) {
                    (Some(x), Some(y), Some(w), Some(h)) => (x as i32, y as i32, w as i32, h as i32),
                    _ => { return self.rt_err_kind("TypeError", "Gui.pushClip requires 4 integers"); }
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
                    return self.rt_err_kind("TypeError", "Gui.setTitle(text) requires 1 argument");
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
                    return self.rt_err_kind("TypeError", "Gui.setCursor(style) requires 1 argument");
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

            _ => { self.rt_err_kind("TypeError", format!("Unknown Gui method '{}'", dot_call.method)) }
        }
    }

    // ── arg helpers ──────────────────────────────────────────────────────────────

    pub(super) fn gui_int_arg(&mut self, expr: &ast::Expression) -> Option<i64> {
        match self.eval_expression(expr) {
            EvalResult::Value(v) => match self.resolve(v).cloned() {
                Some(ObjectData::Integer(n)) => Some(n),
                _ => None,
            },
            _ => None,
        }
    }

    pub(super) fn gui_str_arg(&mut self, expr: &ast::Expression) -> Option<String> {
        match self.eval_expression(expr) {
            EvalResult::Value(v) => match self.resolve(v).cloned() {
                Some(ObjectData::Str(s)) => Some(s),
                _ => None,
            },
            _ => None,
        }
    }

    /// Lee un argumento que es un arreglo plano de enteros → Vec<i64> (p.ej. puntos
    /// [x0,y0,x1,y1,…] para polilíneas/polígonos).
    fn gui_int_vec_arg(&mut self, expr: &ast::Expression) -> Option<Vec<i64>> {
        match self.eval_expression(expr) {
            EvalResult::Value(v) => match self.resolve(v) {
                Some(ObjectData::Array { elements, .. }) => {
                    let mut out = Vec::with_capacity(elements.len());
                    for e in elements {
                        match e {
                            OwnedValue::Integer(n) => out.push(*n),
                            _ => return None,
                        }
                    }
                    Some(out)
                }
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

    /// Lee el ctx de renderTree: un Array de pares [nombre, valor] → Vec<(String,String)>
    /// (para las condiciones reactivas del CSS: media queries / estado).
    fn gui_read_ctx(&mut self, expr: &ast::Expression) -> Vec<(String, String)> {
        let v = match self.eval_expression(expr) {
            EvalResult::Value(v) => v,
            _ => return Vec::new(),
        };
        match self.resolve(v) {
            Some(ObjectData::Array { elements, .. }) => {
                let mut out = Vec::new();
                for e in elements {
                    if let OwnedValue::Array { elements, .. } = e {
                        if elements.len() >= 2 {
                            if let OwnedValue::Str(name) = &elements[0] {
                                let val = match &elements[1] {
                                    OwnedValue::Str(s) => s.clone(),
                                    OwnedValue::Integer(k) => k.to_string(),
                                    OwnedValue::Boolean(b) => b.to_string(),
                                    OwnedValue::Decimal(d) => d.to_string(),
                                    _ => String::new(),
                                };
                                out.push((name.clone(), val));
                            }
                        }
                    }
                }
                out
            }
            _ => Vec::new(),
        }
    }

    /// Control de ventana (setMinSize/setResizable/setFullscreen/maximize/setPosition/
    /// setDecorations): valida args y encola el GuiCmd para el hilo main.
    fn gui_window_control(&mut self, method: &str, dot_call: &ast::DotCallExpression) -> EvalResult {
        let cmd = match method {
            "setMinSize" | "setMaxSize" => {
                if dot_call.arguments.len() != 2 { return self.rt_err_kind("TypeError", format!("Gui.{}(w, h) requires 2 arguments", method)); }
                let w = self.gui_int_arg(&dot_call.arguments[0]);
                let h = self.gui_int_arg(&dot_call.arguments[1]);
                match (w, h) {
                    (Some(w), Some(h)) => {
                        let (w, h) = (w.max(0) as u32, h.max(0) as u32);
                        if method == "setMinSize" { GuiCmd::SetMinSize(w, h) } else { GuiCmd::SetMaxSize(w, h) }
                    }
                    _ => { return self.rt_err_kind("TypeError", format!("Gui.{} requires 2 integers", method)); }
                }
            }
            "setPosition" => {
                if dot_call.arguments.len() != 2 { return self.rt_err_kind("TypeError", "Gui.setPosition(x, y) requires 2 arguments"); }
                let x = self.gui_int_arg(&dot_call.arguments[0]);
                let y = self.gui_int_arg(&dot_call.arguments[1]);
                match (x, y) {
                    (Some(x), Some(y)) => GuiCmd::SetPosition(x as i32, y as i32),
                    _ => { return self.rt_err_kind("TypeError", "Gui.setPosition requires 2 integers"); }
                }
            }
            _ => {
                if dot_call.arguments.len() != 1 { return self.rt_err_kind("TypeError", format!("Gui.{}(bool) requires 1 argument", method)); }
                let b = match self.gui_bool_arg(&dot_call.arguments[0]) {
                    Some(b) => b,
                    None => { return self.rt_err_kind("TypeError", format!("Gui.{} requires a boolean", method)); }
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
            return self.rt_err_kind("GuiError", "Gui file dialog: no window open");
        }
        let n = dot_call.arguments.len();
        let filter_name  = if n >= 1 { self.gui_str_arg(&dot_call.arguments[0]).unwrap_or_default() } else { String::new() };
        let exts_csv     = if n >= 2 { self.gui_str_arg(&dot_call.arguments[1]).unwrap_or_default() } else { String::new() };
        let default_name = if save && n >= 3 { self.gui_str_arg(&dot_call.arguments[2]).unwrap_or_default() } else { String::new() };
        let filter_exts: Vec<String> = exts_csv.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        let host = match host() { Some(h) => h.clone(), None => { return self.rt_err_kind("GuiError", "Gui file dialog: no GUI host"); } };
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
