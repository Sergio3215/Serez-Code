// namespaces_gui/render.rs — motor de primitivos: layout (bloque/flex/scroll) +
// resolución de CSS + emisión de nodos de escena, en UNA pasada nativa. Reemplaza el
// walk interpretado de serez-ui (renderer_gui.sz). El árbol es un Array anidado
// [tag, [[prop,val]…], [hijo|texto…]]; `render_tree` es la única entrada pública.
//
// Accede a los internos de GuiState/GuiFonts/SceneNode del módulo padre por ser un
// submódulo descendiente (no necesitan `pub`). Ver PROPUESTA_RENDER_PRIMITIVOS_CORE.md.

use crate::region::{ObjectData, OwnedValue};
use super::{GuiState, GuiFonts, SceneNode, SceneNodeKind, ImageData, NativeStylesheet};
use super::css::css_color;
use super::svg;

/// Punto de entrada del motor: baja el árbol de primitivos a la escena de `st`,
/// resolviendo el CSS de `sheet` con el `ctx` reactivo, y llena `regions` (hit-test).
/// Encapsula el marco raíz y el contexto para que el dispatch de Gui no toque los
/// internos del motor (Prim* quedan privados de este módulo).
pub(crate) fn render_tree(
    tag: &str,
    style: &[OwnedValue],
    kids: &[OwnedValue],
    w: i32,
    h: i32,
    sheet: Option<&NativeStylesheet>,
    svgs: &[svg::ParsedSvg],
    ctx: &[(String, String)],
    fonts: &mut Option<GuiFonts>,
    st: &mut GuiState,
    regions: &mut Vec<PrimRegion>,
) {
    let root = PrimFrame {
        x: 0, y: 0, avail_w: w,
        depth: 0, z_off: 0,
        cb_x: 0, cb_y: 0, cb_w: w, cb_h: h,
        inh_color: 0xffffff, // color raíz por defecto (blanco); se hereda hacia abajo
    };
    let mut pcx = PrimCtx { sheet, svgs, ctx, fonts, st, regions };
    prim_render(tag, style, kids, root, &mut pcx);
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
/// Región de hit-testing que devuelve el motor al dispatch de Gui (`renderTree`):
/// caja + el `onClick` embebido (VNode/función) para enrutar el clic en `.sz`.
pub(crate) struct PrimRegion {
    pub(crate) tag: String,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) w: i32,
    pub(crate) h: i32,
    pub(crate) onclick: Option<OwnedValue>,
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
    inh_color: u32, // `color` heredado del ancestro (como en CSS): un hijo sin
                    // `color` propio lo toma del padre en vez de blanco por defecto.
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
               sheet: Option<&NativeStylesheet>, ctx: &[(String, String)], inh_color: u32) -> PrimStyle {
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
            // `font-size` en px (web-like) tiene prioridad: la rejilla usa glifo base
            // de 8px, así font-size:16px → escala 2. Si no, `font-scale` (nativo) o el
            // default por tag.
            scale: match sget(&props, "font-size") {
                Some(fs) => {
                    let px = fs.trim().trim_end_matches("px").trim().parse::<f32>().unwrap_or(8.0);
                    ((px / 8.0).round() as i32).max(1)
                }
                None => snum(&props, "font-scale", prim_default_scale(tag)),
            },
            text_col: scol(&props, "color").unwrap_or(inh_color),
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
    let sty = PrimStyle::resolve(tag, style_inline, f.avail_w, win_h, cx.sheet, cx.ctx, f.inh_color);
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
    // El borde se pinta inset (dentro de la caja); el contenido arranca al menos
    // después del borde para no pintarse encima cuando el padding es menor que el
    // grosor del borde. Con padding >= borde (el caso normal) esto es idéntico al
    // padding puro, así que no cambia el layout existente.
    let bw = sty.border_w.max(0);
    let (ipl, ipr, ipt) = (sty.pad.3.max(bw), sty.pad.1.max(bw), sty.pad.0.max(bw));
    let b = PrimBox {
        x,
        y,
        w: box_w,
        content_x: x + ipl,
        content_w: (box_w - ipl - ipr).max(1),
        z: z_off,
        bg_z: z_off + f.depth - 100,
    };
    // Marco de los hijos: si este nodo está posicionado, pasa a ser el
    // containing block de sus descendientes absolute.
    let positioned = sty.absolute || sty.relative;
    let kf = PrimFrame {
        x: b.content_x,
        y: b.y + ipt,
        avail_w: b.content_w,
        depth: f.depth + 1,
        z_off,
        cb_x: if positioned { x } else { f.cb_x },
        cb_y: if positioned { y } else { f.cb_y },
        cb_w: if positioned { box_w } else { f.cb_w },
        cb_h: if positioned { sty.hgt } else { f.cb_h },
        inh_color: sty.text_col, // los hijos heredan el color resuelto de este nodo
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
    let mut shrunk = false; // ¿se aplicó flex-shrink? (afecta el ancho pasado al hijo)
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
        // Todos fijos: si NO caben, flex-shrink (encoge proporcionalmente a su base,
        // como en la web con el shrink:1 por defecto) para que no desborden; si caben,
        // justify-content reparte el espacio libre.
        for k in &plan { widths.push(if k.abs { 0 } else { k.base.max(1) }); }
        let sum_w: i32 = widths.iter().sum();
        let avail = (b.content_w - total_gap).max(0);
        if sum_w > avail && sum_w > 0 {
            for w in widths.iter_mut() { *w = (*w * avail / sum_w).max(1); }
            shrunk = true;
        }
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
            // CONTENEDOR: así su `width: 50%` se resuelve otra vez contra lo mismo
            // que usó el plan (y no contra el slot, que lo achicaría). PERO si hubo
            // flex-shrink, el slot encogido (`each`) ES el límite → se pasa ese para
            // que el hijo (p.ej. width:150 en un slot de 100) no se salga.
            let from_width = plan.get(i).map(|p| p.from_width).unwrap_or(false);
            let child_avail = if from_width && !shrunk { b.content_w } else { each };
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

