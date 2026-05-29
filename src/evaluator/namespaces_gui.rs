#![allow(unused_imports)]
// namespaces_gui.rs — `Gui` namespace: backend nativo de ventana de píxeles (GUI real, no TUI)
//
// Abre una ventana del SO y dibuja sobre un framebuffer u32 (0x00RRGGBB). El
// compositor de alto nivel (serez-ui) recorre el árbol VNode y llama estas fns
// en un loop: clear → dibujar → present, leyendo entrada entre frames.
//
//   use permissions { Gui }
//   Gui.open("Mi App", 640, 480)
//   while (Gui.isOpen()) {
//       Gui.clear(0x101418)
//       Gui.fillRect(20, 20, 120, 40, 0x3b82f6)
//       Gui.drawText(28, 30, "Guardar", 2, 0xffffff)
//       Gui.present()
//       let chars = Gui.charsTyped()      // texto escrito este frame
//       let keys  = Gui.keysPressed()     // ["Enter","Backspace",...] (edge)
//       if (Gui.mousePressed()) { ... }   // clic en este frame
//   }
//
// Backend: minifb (CPU) + font8x8 (fuente bitmap pública). Texto vectorial
// (cosmic-text) y layout (taffy) son cortes posteriores; con esto ya se puede
// componer una UI completa: cajas, texto, clics, tecleo y scroll.

use crate::ast::{self};
use crate::region::{ObjectData, ObjectRef, OwnedValue};
use super::EvalResult;

use minifb::{Window, WindowOptions, Key, KeyRepeat, MouseMode, MouseButton};
use font8x8::{UnicodeFonts, BASIC_FONTS};

/// Estado persistente de la ventana GUI, almacenado en el Evaluator.
pub struct GuiState {
    pub window: Window,
    pub buffer: Vec<u32>,
    pub width:  usize,
    pub height: usize,
    /// Estado del botón izquierdo en el frame anterior (para detectar clic edge).
    pub prev_mouse_down: bool,
}

/// Mapea un nombre de tecla (serez-code) a `minifb::Key`.
fn map_key(name: &str) -> Option<Key> {
    let k = match name {
        "A" | "a" => Key::A, "B" | "b" => Key::B, "C" | "c" => Key::C,
        "D" | "d" => Key::D, "E" | "e" => Key::E, "F" | "f" => Key::F,
        "G" | "g" => Key::G, "H" | "h" => Key::H, "I" | "i" => Key::I,
        "J" | "j" => Key::J, "K" | "k" => Key::K, "L" | "l" => Key::L,
        "M" | "m" => Key::M, "N" | "n" => Key::N, "O" | "o" => Key::O,
        "P" | "p" => Key::P, "Q" | "q" => Key::Q, "R" | "r" => Key::R,
        "S" | "s" => Key::S, "T" | "t" => Key::T, "U" | "u" => Key::U,
        "V" | "v" => Key::V, "W" | "w" => Key::W, "X" | "x" => Key::X,
        "Y" | "y" => Key::Y, "Z" | "z" => Key::Z,
        "0" => Key::Key0, "1" => Key::Key1, "2" => Key::Key2, "3" => Key::Key3,
        "4" => Key::Key4, "5" => Key::Key5, "6" => Key::Key6, "7" => Key::Key7,
        "8" => Key::Key8, "9" => Key::Key9,
        "Enter" => Key::Enter, "Esc" => Key::Escape, "Space" => Key::Space,
        "Backspace" => Key::Backspace, "Tab" => Key::Tab, "Delete" => Key::Delete,
        "Left" => Key::Left, "Right" => Key::Right, "Up" => Key::Up, "Down" => Key::Down,
        "Home" => Key::Home, "End" => Key::End,
        _ => return None,
    };
    Some(k)
}

/// Nombre canónico de una tecla (para `Gui.keysPressed()`).
fn key_name(k: Key) -> Option<String> {
    let s = match k {
        Key::A => "a", Key::B => "b", Key::C => "c", Key::D => "d", Key::E => "e",
        Key::F => "f", Key::G => "g", Key::H => "h", Key::I => "i", Key::J => "j",
        Key::K => "k", Key::L => "l", Key::M => "m", Key::N => "n", Key::O => "o",
        Key::P => "p", Key::Q => "q", Key::R => "r", Key::S => "s", Key::T => "t",
        Key::U => "u", Key::V => "v", Key::W => "w", Key::X => "x", Key::Y => "y",
        Key::Z => "z",
        Key::Key0 => "0", Key::Key1 => "1", Key::Key2 => "2", Key::Key3 => "3",
        Key::Key4 => "4", Key::Key5 => "5", Key::Key6 => "6", Key::Key7 => "7",
        Key::Key8 => "8", Key::Key9 => "9",
        Key::Enter => "Enter", Key::Escape => "Esc", Key::Space => "Space",
        Key::Backspace => "Backspace", Key::Tab => "Tab", Key::Delete => "Delete",
        Key::Left => "Left", Key::Right => "Right", Key::Up => "Up", Key::Down => "Down",
        Key::Home => "Home", Key::End => "End",
        _ => return None,
    };
    Some(s.to_string())
}

/// Convierte una tecla a su carácter imprimible (layout US), respetando shift.
fn key_to_char(k: Key, shift: bool) -> Option<char> {
    let c = match k {
        Key::A => if shift {'A'} else {'a'}, Key::B => if shift {'B'} else {'b'},
        Key::C => if shift {'C'} else {'c'}, Key::D => if shift {'D'} else {'d'},
        Key::E => if shift {'E'} else {'e'}, Key::F => if shift {'F'} else {'f'},
        Key::G => if shift {'G'} else {'g'}, Key::H => if shift {'H'} else {'h'},
        Key::I => if shift {'I'} else {'i'}, Key::J => if shift {'J'} else {'j'},
        Key::K => if shift {'K'} else {'k'}, Key::L => if shift {'L'} else {'l'},
        Key::M => if shift {'M'} else {'m'}, Key::N => if shift {'N'} else {'n'},
        Key::O => if shift {'O'} else {'o'}, Key::P => if shift {'P'} else {'p'},
        Key::Q => if shift {'Q'} else {'q'}, Key::R => if shift {'R'} else {'r'},
        Key::S => if shift {'S'} else {'s'}, Key::T => if shift {'T'} else {'t'},
        Key::U => if shift {'U'} else {'u'}, Key::V => if shift {'V'} else {'v'},
        Key::W => if shift {'W'} else {'w'}, Key::X => if shift {'X'} else {'x'},
        Key::Y => if shift {'Y'} else {'y'}, Key::Z => if shift {'Z'} else {'z'},
        Key::Key0 => if shift {')'} else {'0'}, Key::Key1 => if shift {'!'} else {'1'},
        Key::Key2 => if shift {'@'} else {'2'}, Key::Key3 => if shift {'#'} else {'3'},
        Key::Key4 => if shift {'$'} else {'4'}, Key::Key5 => if shift {'%'} else {'5'},
        Key::Key6 => if shift {'^'} else {'6'}, Key::Key7 => if shift {'&'} else {'7'},
        Key::Key8 => if shift {'*'} else {'8'}, Key::Key9 => if shift {'('} else {'9'},
        Key::Space => ' ',
        Key::Minus => if shift {'_'} else {'-'}, Key::Equal => if shift {'+'} else {'='},
        Key::Comma => if shift {'<'} else {','}, Key::Period => if shift {'>'} else {'.'},
        Key::Slash => if shift {'?'} else {'/'}, Key::Semicolon => if shift {':'} else {';'},
        Key::Apostrophe => if shift {'"'} else {'\''},
        _ => return None,
    };
    Some(c)
}

/// Mezcla `src` sobre `dst` con alfa 0..=255 (255 = opaco).
fn blend(dst: u32, src: u32, a: u32) -> u32 {
    let a = a.min(255);
    let inv = 255 - a;
    let dr = (dst >> 16) & 0xff; let dg = (dst >> 8) & 0xff; let db = dst & 0xff;
    let sr = (src >> 16) & 0xff; let sg = (src >> 8) & 0xff; let sb = src & 0xff;
    let r = (sr * a + dr * inv) / 255;
    let g = (sg * a + dg * inv) / 255;
    let b = (sb * a + db * inv) / 255;
    (r << 16) | (g << 8) | b
}

impl super::Evaluator {

    // ── Gui ─────────────────────────────────────────────────────────────────────

    // `limit_update_rate` está deprecada en minifb pero sigue siendo la API estable
    // en 0.27; se silencia el warning sin cambiar de método.
    #[allow(deprecated)]
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
                    Some(v) if v > 0 => v as usize,
                    _ => { eprintln!("❌ ERROR: Gui.open width must be a positive integer"); return EvalResult::Error; }
                };
                let h = match self.gui_int_arg(&dot_call.arguments[2]) {
                    Some(v) if v > 0 => v as usize,
                    _ => { eprintln!("❌ ERROR: Gui.open height must be a positive integer"); return EvalResult::Error; }
                };
                let mut window = match Window::new(&title, w, h, WindowOptions {
                    resize: true,
                    ..WindowOptions::default()
                }) {
                    Ok(win) => win,
                    Err(e) => { eprintln!("❌ ERROR: Gui.open failed: {}", e); return EvalResult::Error; }
                };
                window.limit_update_rate(Some(std::time::Duration::from_micros(16_600)));
                self.gui_state = Some(GuiState {
                    window, buffer: vec![0u32; w * h], width: w, height: h, prev_mouse_down: false,
                });
                EvalResult::Value(self.null_ref)
            }

            "isOpen" => {
                let open = self.gui_state.as_ref().map(|s| s.window.is_open()).unwrap_or(false);
                EvalResult::Value(if open { self.true_ref } else { self.false_ref })
            }

            "size" => {
                let (w, h) = self.gui_state.as_ref().map(|s| (s.width as i64, s.height as i64)).unwrap_or((0, 0));
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: Some("int".to_string()),
                    elements: vec![OwnedValue::Integer(w), OwnedValue::Integer(h)],
                }))
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
                        // Reconciliar el framebuffer con el tamaño actual de la ventana (resize)
                        let (cw, ch) = st.window.get_size();
                        if cw != st.width || ch != st.height {
                            st.width = cw; st.height = ch;
                            st.buffer = vec![color; cw * ch];
                        } else {
                            for px in st.buffer.iter_mut() { *px = color; }
                        }
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
                    Some(st) => { fill_rect(st, x, y, w, h, color); EvalResult::Value(self.null_ref) }
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
                        (x, y, w, h, (c as u32) & 0x00FF_FFFF, a.clamp(0, 255) as u32),
                    _ => { eprintln!("❌ ERROR: Gui.fillRectAlpha requires 6 integers"); return EvalResult::Error; }
                };
                match self.gui_state.as_mut() {
                    Some(st) => {
                        let bw = st.width as i64; let bh = st.height as i64;
                        let (x0, y0) = (x.max(0), y.max(0));
                        let (x1, y1) = ((x + w).min(bw), (y + h).min(bh));
                        let mut yy = y0;
                        while yy < y1 {
                            let row = (yy as usize) * st.width;
                            let mut xx = x0;
                            while xx < x1 {
                                let idx = row + xx as usize;
                                st.buffer[idx] = blend(st.buffer[idx], color, alpha);
                                xx += 1;
                            }
                            yy += 1;
                        }
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
                    (Some(x), Some(y), Some(c)) => (x, y, (c as u32) & 0x00FF_FFFF),
                    _ => { eprintln!("❌ ERROR: Gui.setPixel requires 3 integers"); return EvalResult::Error; }
                };
                match self.gui_state.as_mut() {
                    Some(st) => {
                        if x >= 0 && y >= 0 && (x as usize) < st.width && (y as usize) < st.height {
                            st.buffer[(y as usize) * st.width + x as usize] = color;
                        }
                        EvalResult::Value(self.null_ref)
                    }
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
                let (mut x0, mut y0, x1, y1, color) = match (x0, y0, x1, y1, c) {
                    (Some(a), Some(b), Some(c), Some(d), Some(e)) => (a, b, c, d, (e as u32) & 0x00FF_FFFF),
                    _ => { eprintln!("❌ ERROR: Gui.drawLine requires 5 integers"); return EvalResult::Error; }
                };
                match self.gui_state.as_mut() {
                    Some(st) => {
                        // Bresenham
                        let dx = (x1 - x0).abs();
                        let dy = -(y1 - y0).abs();
                        let sx = if x0 < x1 { 1 } else { -1 };
                        let sy = if y0 < y1 { 1 } else { -1 };
                        let mut err = dx + dy;
                        loop {
                            if x0 >= 0 && y0 >= 0 && (x0 as usize) < st.width && (y0 as usize) < st.height {
                                st.buffer[(y0 as usize) * st.width + x0 as usize] = color;
                            }
                            if x0 == x1 && y0 == y1 { break; }
                            let e2 = 2 * err;
                            if e2 >= dy { err += dy; x0 += sx; }
                            if e2 <= dx { err += dx; y0 += sy; }
                        }
                        EvalResult::Value(self.null_ref)
                    }
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
                        (x, y, t, s.max(1), (c as u32) & 0x00FF_FFFF),
                    _ => { eprintln!("❌ ERROR: Gui.drawText requires (int, int, string, int, int)"); return EvalResult::Error; }
                };
                match self.gui_state.as_mut() {
                    Some(st) => { draw_text(st, x, y, &text, scale, color); EvalResult::Value(self.null_ref) }
                    None => { eprintln!("❌ ERROR: Gui.drawText: no window open"); EvalResult::Error }
                }
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
                let count = text.chars().count() as i64;
                let w = count * 8 * scale;
                let h = 8 * scale;
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: Some("int".to_string()),
                    elements: vec![OwnedValue::Integer(w), OwnedValue::Integer(h)],
                }))
            }

            "present" => {
                match self.gui_state.as_mut() {
                    Some(st) => {
                        let w = st.width; let h = st.height;
                        let res = st.window.update_with_buffer(&st.buffer, w, h);
                        // Frame boundary: registrar estado del mouse para el edge de mousePressed
                        st.prev_mouse_down = st.window.get_mouse_down(MouseButton::Left);
                        match res {
                            Ok(_) => EvalResult::Value(self.null_ref),
                            Err(e) => { eprintln!("❌ ERROR: Gui.present failed: {}", e); EvalResult::Error }
                        }
                    }
                    None => { eprintln!("❌ ERROR: Gui.present: no window open"); EvalResult::Error }
                }
            }

            "mouse" => {
                let (mx, my) = self.gui_state.as_ref()
                    .and_then(|s| s.window.get_mouse_pos(MouseMode::Clamp))
                    .unwrap_or((0.0, 0.0));
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: Some("int".to_string()),
                    elements: vec![OwnedValue::Integer(mx as i64), OwnedValue::Integer(my as i64)],
                }))
            }

            "mouseDown" => {
                let down = self.gui_state.as_ref().map(|s| s.window.get_mouse_down(MouseButton::Left)).unwrap_or(false);
                EvalResult::Value(if down { self.true_ref } else { self.false_ref })
            }

            "mousePressed" => {
                let pressed = self.gui_state.as_ref()
                    .map(|s| s.window.get_mouse_down(MouseButton::Left) && !s.prev_mouse_down)
                    .unwrap_or(false);
                EvalResult::Value(if pressed { self.true_ref } else { self.false_ref })
            }

            "scroll" => {
                let (dx, dy) = self.gui_state.as_ref()
                    .and_then(|s| s.window.get_scroll_wheel())
                    .unwrap_or((0.0, 0.0));
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: Some("int".to_string()),
                    elements: vec![OwnedValue::Integer(dx as i64), OwnedValue::Integer(dy as i64)],
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
                let down = match (map_key(&name), self.gui_state.as_ref()) {
                    (Some(k), Some(st)) => st.window.is_key_down(k),
                    _ => false,
                };
                EvalResult::Value(if down { self.true_ref } else { self.false_ref })
            }

            "keysPressed" => {
                let names: Vec<String> = match self.gui_state.as_ref() {
                    Some(st) => st.window.get_keys_pressed(KeyRepeat::No).into_iter()
                        .filter_map(key_name).collect(),
                    None => Vec::new(),
                };
                let mut elems: Vec<OwnedValue> = Vec::with_capacity(names.len());
                for n in names { elems.push(OwnedValue::Str(n)); }
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: Some("string".to_string()),
                    elements: elems,
                }))
            }

            "charsTyped" => {
                let s: String = match self.gui_state.as_ref() {
                    Some(st) => {
                        let shift = st.window.is_key_down(Key::LeftShift) || st.window.is_key_down(Key::RightShift);
                        st.window.get_keys_pressed(KeyRepeat::Yes).into_iter()
                            .filter_map(|k| key_to_char(k, shift)).collect()
                    }
                    None => String::new(),
                };
                EvalResult::Value(self.alloc(ObjectData::Str(s)))
            }

            "close" => {
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

    /// Evalúa los 5 args de fillRect: (x, y, w, h, color).
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

// ── Funciones de dibujo (operan sobre el framebuffer) ─────────────────────────

fn fill_rect(st: &mut GuiState, x: i64, y: i64, w: i64, h: i64, color: u32) {
    let bw = st.width as i64;
    let bh = st.height as i64;
    let (x0, y0) = (x.max(0), y.max(0));
    let (x1, y1) = ((x + w).min(bw), (y + h).min(bh));
    let mut yy = y0;
    while yy < y1 {
        let row = (yy as usize) * st.width;
        let mut xx = x0;
        while xx < x1 {
            st.buffer[row + xx as usize] = color;
            xx += 1;
        }
        yy += 1;
    }
}

fn draw_text(st: &mut GuiState, x: i64, y: i64, text: &str, scale: i64, color: u32) {
    let mut cx = x;
    for ch in text.chars() {
        if let Some(glyph) = BASIC_FONTS.get(ch) {
            // glyph: [u8; 8], una fila por byte, bit LSB = columna izquierda
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..8 {
                    if bits & (1 << col) != 0 {
                        let px = cx + col as i64 * scale;
                        let py = y + row as i64 * scale;
                        fill_rect(st, px, py, scale, scale, color);
                    }
                }
            }
        }
        cx += 8 * scale;
    }
}
