// namespaces_gui/css.rs — motor CSS nativo del engine de primitivos (port de css.sz).
//
// Autocontenido (solo `std`): parsea la hoja `.szs` a reglas y resuelve las props
// aplicables a un nodo. Selectores simples por tag / `.clase`(s) / `#id` / `:pseudo` /
// universal `*`, encadenables por ESPACIO (combinador descendiente; `>` se trata como
// descendiente), con condición reactiva `(var op val)` evaluada contra un ctx
// [(nombre, valor)]; la última regla que matchea gana. Lo consume el motor de
// primitivos (render.rs) vía `NativeStylesheet::props_for_node` y `Gui.loadStylesheet`
// vía `parse_css`.

/// Identidad CSS de un nodo para el matching: tag, clases, id y pseudo-estados
/// activos. Los estados salen de los ATTRS del nodo (focused/hover/active/disabled
/// = "true") — el motor es stateless por frame, así que `:focus` matchea lo que el
/// framework marque en el árbol, igual que la convención `.focused` previa.
pub(crate) struct NodeKey {
    pub(crate) tag: String,
    pub(crate) classes: Vec<String>,
    pub(crate) id: Option<String>,
    pub(crate) states: Vec<String>,
}

/// Selector simple: tag y/o `.clase`(s) y/o `#id` y/o `:pseudo`(s), o universal `*`.
/// Un nodo casa si TODAS las partes presentes casan (habilita el lowering
/// widget→div/span y compuestos `.a.b` / `tag.a:focus`).
struct SimpleSel {
    universal: bool,
    tag: Option<String>,
    classes: Vec<String>,
    id: Option<String>,
    pseudos: Vec<String>,
}

/// Selector completo: cadena de selectores simples (descendiente). El ÚLTIMO es el
/// sujeto (el nodo que recibe las props); los anteriores deben casar ancestros en
/// orden (de afuera hacia adentro), como en la web.
struct Selector {
    parts: Vec<SimpleSel>,
}

/// Una regla CSS: selector + condición opcional (var op val) + decls.
struct CssRule {
    sel: Selector,
    cond: Option<(String, String, String)>,
    decls: Vec<(String, String)>,
}

/// Hoja de estilo nativa (port de css.sz). Match por tag/clase/id/pseudo/`*` con
/// combinador descendiente + condición reactiva.
pub(crate) struct NativeStylesheet {
    rules: Vec<CssRule>,
}

fn parse_simple(s: &str) -> SimpleSel {
    let mut sel = SimpleSel { universal: false, tag: None, classes: Vec::new(), id: None, pseudos: Vec::new() };
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    if i < chars.len() && chars[i] == '*' {
        sel.universal = true;
        i += 1;
    }
    if !sel.universal && i < chars.len() && chars[i] != '.' && chars[i] != '#' && chars[i] != ':' {
        let mut t = String::new();
        while i < chars.len() && chars[i] != '.' && chars[i] != '#' && chars[i] != ':' { t.push(chars[i]); i += 1; }
        if !t.is_empty() { sel.tag = Some(t); }
    }
    while i < chars.len() {
        let kind = chars[i];
        i += 1;
        let mut name = String::new();
        while i < chars.len() && chars[i] != '.' && chars[i] != '#' && chars[i] != ':' { name.push(chars[i]); i += 1; }
        if name.is_empty() { continue; }
        match kind {
            '.' => sel.classes.push(name),
            '#' => sel.id = Some(name),
            ':' => sel.pseudos.push(name),
            _ => {}
        }
    }
    sel
}

fn parse_selector(s: &str) -> Selector {
    Selector {
        parts: s.split_whitespace()
            .filter(|t| *t != ">") // combinador hijo: se degrada a descendiente
            .map(parse_simple)
            .collect(),
    }
}

/// ¿El selector simple casa este nodo? Todas las partes presentes deben casar,
/// y debe haber al menos una parte (o `*`) para que la regla tenga sujeto.
fn simple_matches(sel: &SimpleSel, k: &NodeKey) -> bool {
    if let Some(t) = &sel.tag { if t != &k.tag { return false; } }
    for c in &sel.classes { if !k.classes.iter().any(|x| x == c) { return false; } }
    if let Some(i) = &sel.id { if k.id.as_deref() != Some(i.as_str()) { return false; } }
    for p in &sel.pseudos { if !k.states.iter().any(|x| x == p) { return false; } }
    sel.universal || sel.tag.is_some() || !sel.classes.is_empty() || sel.id.is_some() || !sel.pseudos.is_empty()
}

/// Match del selector completo: el último simple casa el SUJETO; los anteriores
/// deben casar ancestros en orden (semántica descendiente: se busca de la hoja
/// hacia la raíz, cada parte consume el primer ancestro que la satisface).
fn selector_matches(sel: &Selector, subject: &NodeKey, ancestors: &[NodeKey]) -> bool {
    let parts = &sel.parts;
    let Some(last) = parts.last() else { return false; };
    if !simple_matches(last, subject) { return false; }
    let mut pi = parts.len() as i32 - 2;
    let mut ai = ancestors.len() as i32 - 1; // el ancestro más cercano va al final
    while pi >= 0 {
        let mut found = false;
        while ai >= 0 {
            let a = &ancestors[ai as usize];
            ai -= 1;
            if simple_matches(&parts[pi as usize], a) { found = true; break; }
        }
        if !found { return false; }
        pi -= 1;
    }
    true
}

impl NativeStylesheet {
    /// Props aplicables al nodo (sujeto + cadena de ancestros), última gana,
    /// dado el ctx [(nombre,valor)]. `ancestors` va de la raíz al padre directo.
    pub(crate) fn props_for_node(&self, subject: &NodeKey, ancestors: &[NodeKey], ctx: &[(String, String)]) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        for r in &self.rules {
            if !selector_matches(&r.sel, subject, ancestors) {
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

pub(crate) fn css_color(raw: &str) -> Option<u32> {
    let s = raw.trim();
    // #rgb / #rrggbb
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
    // rgb(r, g, b) / rgba(r, g, b, a) — el alpha se ignora (el color es 0xRRGGBB;
    // la translucidez de un box va por `opacity`, no por el canal alpha del color).
    let lower = s.to_ascii_lowercase();
    if let Some(inner) = lower.strip_prefix("rgb(").or_else(|| lower.strip_prefix("rgba(")) {
        let inner = inner.trim_end_matches(')');
        let n: Vec<u32> = inner.split(',').take(3)
            .filter_map(|p| p.trim().parse::<f32>().ok().map(|v| v.round().clamp(0.0, 255.0) as u32))
            .collect();
        if n.len() == 3 { return Some((n[0] << 16) | (n[1] << 8) | n[2]); }
        return None;
    }
    // hsl(h, s%, l%) / hsla(...) — h en grados, s/l en porcentaje.
    if let Some(inner) = lower.strip_prefix("hsl(").or_else(|| lower.strip_prefix("hsla(")) {
        let inner = inner.trim_end_matches(')');
        let parts: Vec<f32> = inner.split(',').take(3)
            .filter_map(|p| p.trim().trim_end_matches('%').trim().parse::<f32>().ok())
            .collect();
        if parts.len() == 3 { return Some(hsl_to_rgb(parts[0], parts[1] / 100.0, parts[2] / 100.0)); }
        return None;
    }
    // Nombres CSS comunes.
    Some(match lower.as_str() {
        "white" => 0xffffff, "black" => 0x000000, "red" => 0xff0000, "green" => 0x008000,
        "lime" => 0x00ff00, "blue" => 0x0000ff, "yellow" => 0xffff00, "cyan" | "aqua" => 0x00ffff,
        "magenta" | "fuchsia" => 0xff00ff, "gray" | "grey" => 0x808080, "silver" => 0xc0c0c0,
        "maroon" => 0x800000, "olive" => 0x808000, "navy" => 0x000080, "teal" => 0x008080,
        "purple" => 0x800080, "orange" => 0xffa500, "pink" => 0xffc0cb, "brown" => 0xa52a2a,
        "gold" => 0xffd700, "transparent" => return None,
        _ => return None,
    })
}

/// hsl → rgb (0xRRGGBB). `h` en grados [0,360), `s`/`l` en [0,1].
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> u32 {
    let h = h.rem_euclid(360.0) / 360.0;
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    let (r, g, b) = if s == 0.0 {
        (l, l, l)
    } else {
        let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
        let p = 2.0 * l - q;
        (hue_to_rgb(p, q, h + 1.0 / 3.0), hue_to_rgb(p, q, h), hue_to_rgb(p, q, h - 1.0 / 3.0))
    };
    let to = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    (to(r) << 16) | (to(g) << 8) | to(b)
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 { t += 1.0; }
    if t > 1.0 { t -= 1.0; }
    if t < 1.0 / 6.0 { return p + (q - p) * 6.0 * t; }
    if t < 1.0 / 2.0 { return q; }
    if t < 2.0 / 3.0 { return p + (q - p) * (2.0 / 3.0 - t) * 6.0; }
    p
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

/// Parser CSS (port de parseCss): selectores (con descendientes/pseudos) + (cond) +
/// { decls }. Salta comentarios y bloques :import/:font.
pub(crate) fn parse_css(src: &str) -> NativeStylesheet {
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
        // Selector: todo hasta `(` (condición) o `{` (decls). Admite espacios
        // (descendientes), `.` `#` `:` y `>` — parse_selector lo trocea.
        let mut sel = String::new();
        while i < n && s[i] != '{' && s[i] != '(' && s[i] != '}' { sel.push(s[i]); i += 1; }
        let sel = sel.trim().to_string();
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
        // Grupo `h1, h2 { … }` → una regla por selector (mismas decls/cond).
        for part in sel.split(',') {
            let part = part.trim();
            if part.is_empty() { continue; }
            rules.push(CssRule { sel: parse_selector(part), cond: cond.clone(), decls: decls.clone() });
        }
    }
    NativeStylesheet { rules }
}
