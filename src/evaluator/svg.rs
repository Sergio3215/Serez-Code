//! Subset de SVG rasterizado con tiny-skia, para el primitivo `svg` del motor de
//! render (Fase 2b). Parser propio (sin dependencia de XML) de un subconjunto
//! práctico: `<path d>` (comandos M L H V C S Q T A Z, absolutos y relativos),
//! shapes (rect/circle/ellipse/line/polyline/polygon), `<g transform>`,
//! fill/stroke/stroke-width/opacity con herencia, y `viewBox`.
//! FUERA DE ALCANCE: filtros, máscaras, texto, gradientes, clipPath.
//!
//! `parse(src)` produce un `ParsedSvg` retenido (handle de `Gui.loadSvg`); el motor
//! llama `rasterize(svg, w, h)` para blitear el vector nítido a la caja (con caché
//! por tamaño en el lado del motor). La salida es ARGB straight (no premultiplicado)
//! para el `blend` del canvas.

use tiny_skia::{FillRule, Paint, Path, PathBuilder, Pixmap, Stroke, Transform};

/// Un SVG parseado: viewBox + lista de shapes ya aplanadas a `tiny_skia::Path`
/// (en coords del viewBox) con su transform acumulada y su pintura.
pub struct ParsedSvg {
    view: (f32, f32, f32, f32), // min_x, min_y, width, height
    shapes: Vec<Shape>,
}

struct Shape {
    path: Path,
    transform: Transform, // acumulada de los <g> ancestros + la propia
    fill: Option<[u8; 4]>,
    stroke: Option<[u8; 4]>,
    stroke_width: f32,
    fill_rule: FillRule,
}

/// Contexto de pintura heredable (de `<svg>`/`<g>` a los hijos).
#[derive(Clone)]
struct Ctx {
    fill: Option<[u8; 4]>,
    stroke: Option<[u8; 4]>,
    stroke_width: f32,
    opacity: f32,
    fill_opacity: f32,
    stroke_opacity: f32,
    fill_rule: FillRule,
    transform: Transform,
}

impl Ctx {
    fn root() -> Ctx {
        Ctx {
            fill: Some([0, 0, 0, 255]), // SVG: fill por defecto = black
            stroke: None,
            stroke_width: 1.0,
            opacity: 1.0,
            fill_opacity: 1.0,
            stroke_opacity: 1.0,
            fill_rule: FillRule::Winding,
            transform: Transform::identity(),
        }
    }

    /// Deriva un contexto hijo aplicando los atributos de presentación del elemento.
    fn merge(&self, attrs: &[(String, String)]) -> Ctx {
        let mut c = self.clone();
        // `style="fill:..;stroke:.."` primero, luego atributos de presentación (que ganan).
        if let Some(style) = get(attrs, "style") {
            for decl in style.split(';') {
                let mut it = decl.splitn(2, ':');
                if let (Some(k), Some(v)) = (it.next(), it.next()) {
                    apply_paint(&mut c, k.trim(), v.trim());
                }
            }
        }
        for (k, v) in attrs {
            apply_paint(&mut c, k.as_str(), v.trim());
        }
        if let Some(t) = get(attrs, "transform") {
            c.transform = compose(c.transform, parse_transform(&t));
        }
        c
    }

    /// Color de relleno final (con opacidad aplicada), o None si `fill:none`.
    fn eff_fill(&self) -> Option<[u8; 4]> {
        self.fill.map(|mut c| {
            c[3] = (c[3] as f32 * self.fill_opacity * self.opacity).round().clamp(0.0, 255.0) as u8;
            c
        }).filter(|c| c[3] > 0)
    }
    fn eff_stroke(&self) -> Option<[u8; 4]> {
        self.stroke.map(|mut c| {
            c[3] = (c[3] as f32 * self.stroke_opacity * self.opacity).round().clamp(0.0, 255.0) as u8;
            c
        }).filter(|c| c[3] > 0)
    }
}

fn apply_paint(c: &mut Ctx, k: &str, v: &str) {
    match k {
        "fill" => c.fill = parse_color(v),
        "stroke" => c.stroke = parse_color(v),
        "stroke-width" => { if let Some(n) = parse_num(v) { c.stroke_width = n; } }
        "opacity" => { if let Some(n) = parse_num(v) { c.opacity = n.clamp(0.0, 1.0); } }
        "fill-opacity" => { if let Some(n) = parse_num(v) { c.fill_opacity = n.clamp(0.0, 1.0); } }
        "stroke-opacity" => { if let Some(n) = parse_num(v) { c.stroke_opacity = n.clamp(0.0, 1.0); } }
        "fill-rule" => { c.fill_rule = if v == "evenodd" { FillRule::EvenOdd } else { FillRule::Winding }; }
        _ => {}
    }
}

fn get<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}
fn getf(attrs: &[(String, String)], name: &str, d: f32) -> f32 {
    get(attrs, name).and_then(parse_num).unwrap_or(d)
}

// ─────────────────────────────────────────────────────────────── parsing público
pub fn parse(src: &str) -> Option<ParsedSvg> {
    let elems = scan_elements(src);
    let mut view = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let mut svg_w = 0.0f32;
    let mut svg_h = 0.0f32;
    let mut have_view = false;
    let mut stack: Vec<Ctx> = vec![Ctx::root()];
    let mut shapes: Vec<Shape> = Vec::new();

    for e in &elems {
        if e.closing {
            if e.name == "g" && stack.len() > 1 {
                stack.pop();
            }
            continue;
        }
        match e.name.as_str() {
            "svg" => {
                if let Some(vb) = get(&e.attrs, "viewBox") {
                    let n: Vec<f32> = vb.split(|c: char| c == ',' || c.is_whitespace())
                        .filter(|s| !s.is_empty())
                        .filter_map(parse_num)
                        .collect();
                    if n.len() == 4 { view = (n[0], n[1], n[2], n[3]); have_view = true; }
                }
                svg_w = getf(&e.attrs, "width", 0.0);
                svg_h = getf(&e.attrs, "height", 0.0);
                // fill/stroke a nivel <svg> heredan a los hijos.
                let merged = stack.last().unwrap().merge(&e.attrs);
                *stack.last_mut().unwrap() = merged;
            }
            "g" => {
                let merged = stack.last().unwrap().merge(&e.attrs);
                if !e.self_close {
                    stack.push(merged);
                }
            }
            "path" | "rect" | "circle" | "ellipse" | "line" | "polyline" | "polygon" => {
                let ctx = stack.last().unwrap().merge(&e.attrs);
                if let Some(path) = build_path(&e.name, &e.attrs) {
                    // `line`/`polyline` no se rellenan aunque haya fill heredado.
                    let fill = if e.name == "line" || e.name == "polyline" { None } else { ctx.eff_fill() };
                    let stroke = ctx.eff_stroke();
                    if fill.is_some() || stroke.is_some() {
                        shapes.push(Shape {
                            path,
                            transform: ctx.transform,
                            fill,
                            stroke,
                            stroke_width: ctx.stroke_width,
                            fill_rule: ctx.fill_rule,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    if !have_view {
        // Sin viewBox: usar width/height (o un fallback) como sistema de coords.
        let w = if svg_w > 0.0 { svg_w } else { 100.0 };
        let h = if svg_h > 0.0 { svg_h } else { 100.0 };
        view = (0.0, 0.0, w, h);
    }
    if shapes.is_empty() {
        return None;
    }
    Some(ParsedSvg { view, shapes })
}

/// Rasteriza a una caja de `w`×`h` px (aspecto preservado, centrado). Salida ARGB
/// straight, longitud `w*h`. Usa AA de tiny-skia.
pub fn rasterize(svg: &ParsedSvg, w: u32, h: u32) -> Vec<u32> {
    let w = w.max(1);
    let h = h.max(1);
    let mut pixmap = match Pixmap::new(w, h) {
        Some(p) => p,
        None => return vec![0u32; (w * h) as usize],
    };
    let (vx, vy, vw0, vh0) = svg.view;
    let vw = if vw0 > 0.0 { vw0 } else { w as f32 };
    let vh = if vh0 > 0.0 { vh0 } else { h as f32 };
    // "meet": escala uniforme que hace caber el viewBox en la caja, centrado.
    let s = (w as f32 / vw).min(h as f32 / vh);
    let tx = (w as f32 - vw * s) / 2.0 - vx * s;
    let ty = (h as f32 - vh * s) / 2.0 - vy * s;
    let box_t = Transform::from_row(s, 0.0, 0.0, s, tx, ty);

    for shape in &svg.shapes {
        let t = compose(box_t, shape.transform);
        if let Some(f) = shape.fill {
            let mut paint = Paint::default();
            paint.anti_alias = true;
            paint.set_color_rgba8(f[0], f[1], f[2], f[3]);
            pixmap.fill_path(&shape.path, &paint, shape.fill_rule, t, None);
        }
        if let Some(sc) = shape.stroke {
            if shape.stroke_width > 0.0 {
                let mut paint = Paint::default();
                paint.anti_alias = true;
                paint.set_color_rgba8(sc[0], sc[1], sc[2], sc[3]);
                let stroke = Stroke { width: shape.stroke_width, ..Stroke::default() };
                pixmap.stroke_path(&shape.path, &paint, &stroke, t, None);
            }
        }
    }

    let mut out = vec![0u32; (w * h) as usize];
    for (i, px) in pixmap.pixels().iter().enumerate() {
        let c = px.demultiply();
        out[i] = ((c.alpha() as u32) << 24)
            | ((c.red() as u32) << 16)
            | ((c.green() as u32) << 8)
            | (c.blue() as u32);
    }
    out
}

// ─────────────────────────────────────────────────────────────── transform
/// Compone A∘B (aplica B y luego A). Transform de tiny-skia: x'=sx·x+kx·y+tx, y'=ky·x+sy·y+ty.
fn compose(a: Transform, b: Transform) -> Transform {
    Transform::from_row(
        a.sx * b.sx + a.kx * b.ky,          // sx
        a.ky * b.sx + a.sy * b.ky,          // ky
        a.sx * b.kx + a.kx * b.sy,          // kx
        a.ky * b.kx + a.sy * b.sy,          // sy
        a.sx * b.tx + a.kx * b.ty + a.tx,   // tx
        a.ky * b.tx + a.sy * b.ty + a.ty,   // ty
    )
}

fn parse_transform(s: &str) -> Transform {
    let mut t = Transform::identity();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // nombre de la función
        while i < bytes.len() && !bytes[i].is_ascii_alphabetic() { i += 1; }
        let start = i;
        while i < bytes.len() && (bytes[i].is_ascii_alphabetic()) { i += 1; }
        if start == i { break; }
        let name = &s[start..i];
        // argumentos entre paréntesis
        while i < bytes.len() && bytes[i] != b'(' { i += 1; }
        if i >= bytes.len() { break; }
        i += 1; // saltar '('
        let arg_start = i;
        while i < bytes.len() && bytes[i] != b')' { i += 1; }
        let args_str = &s[arg_start..i.min(s.len())];
        if i < bytes.len() { i += 1; } // saltar ')'
        let a: Vec<f32> = args_str.split(|c: char| c == ',' || c.is_whitespace())
            .filter(|x| !x.is_empty())
            .filter_map(parse_num)
            .collect();
        let f = match name {
            "translate" => {
                let tx = a.first().copied().unwrap_or(0.0);
                let ty = a.get(1).copied().unwrap_or(0.0);
                Transform::from_row(1.0, 0.0, 0.0, 1.0, tx, ty)
            }
            "scale" => {
                let sx = a.first().copied().unwrap_or(1.0);
                let sy = a.get(1).copied().unwrap_or(sx);
                Transform::from_row(sx, 0.0, 0.0, sy, 0.0, 0.0)
            }
            "matrix" if a.len() == 6 => Transform::from_row(a[0], a[1], a[2], a[3], a[4], a[5]),
            "rotate" => {
                let deg = a.first().copied().unwrap_or(0.0);
                let rad = deg.to_radians();
                let (sn, cs) = (rad.sin(), rad.cos());
                let rot = Transform::from_row(cs, sn, -sn, cs, 0.0, 0.0);
                if a.len() == 3 {
                    // rotate(deg, cx, cy) = translate(c) · rotate · translate(-c)
                    let (cx, cy) = (a[1], a[2]);
                    let pre = Transform::from_row(1.0, 0.0, 0.0, 1.0, cx, cy);
                    let post = Transform::from_row(1.0, 0.0, 0.0, 1.0, -cx, -cy);
                    compose(compose(pre, rot), post)
                } else {
                    rot
                }
            }
            _ => Transform::identity(),
        };
        t = compose(t, f);
    }
    t
}

// ─────────────────────────────────────────────────────────────── shapes
fn build_path(name: &str, attrs: &[(String, String)]) -> Option<Path> {
    match name {
        "path" => parse_path_data(get(attrs, "d")?),
        "rect" => {
            let x = getf(attrs, "x", 0.0);
            let y = getf(attrs, "y", 0.0);
            let w = getf(attrs, "width", 0.0);
            let h = getf(attrs, "height", 0.0);
            if w <= 0.0 || h <= 0.0 { return None; }
            let mut pb = PathBuilder::new();
            pb.move_to(x, y);
            pb.line_to(x + w, y);
            pb.line_to(x + w, y + h);
            pb.line_to(x, y + h);
            pb.close();
            pb.finish()
        }
        "circle" => {
            let cx = getf(attrs, "cx", 0.0);
            let cy = getf(attrs, "cy", 0.0);
            let r = getf(attrs, "r", 0.0);
            if r <= 0.0 { return None; }
            ellipse_path(cx, cy, r, r)
        }
        "ellipse" => {
            let cx = getf(attrs, "cx", 0.0);
            let cy = getf(attrs, "cy", 0.0);
            let rx = getf(attrs, "rx", 0.0);
            let ry = getf(attrs, "ry", 0.0);
            if rx <= 0.0 || ry <= 0.0 { return None; }
            ellipse_path(cx, cy, rx, ry)
        }
        "line" => {
            let mut pb = PathBuilder::new();
            pb.move_to(getf(attrs, "x1", 0.0), getf(attrs, "y1", 0.0));
            pb.line_to(getf(attrs, "x2", 0.0), getf(attrs, "y2", 0.0));
            pb.finish()
        }
        "polyline" | "polygon" => {
            let pts = parse_points(get(attrs, "points")?);
            if pts.len() < 2 { return None; }
            let mut pb = PathBuilder::new();
            pb.move_to(pts[0].0, pts[0].1);
            for p in &pts[1..] { pb.line_to(p.0, p.1); }
            if name == "polygon" { pb.close(); }
            pb.finish()
        }
        _ => None,
    }
}

/// Elipse como 4 cubics (aproximación de Bézier con kappa).
fn ellipse_path(cx: f32, cy: f32, rx: f32, ry: f32) -> Option<Path> {
    const K: f32 = 0.5522847498307936;
    let (ox, oy) = (rx * K, ry * K);
    let mut pb = PathBuilder::new();
    pb.move_to(cx + rx, cy);
    pb.cubic_to(cx + rx, cy + oy, cx + ox, cy + ry, cx, cy + ry);
    pb.cubic_to(cx - ox, cy + ry, cx - rx, cy + oy, cx - rx, cy);
    pb.cubic_to(cx - rx, cy - oy, cx - ox, cy - ry, cx, cy - ry);
    pb.cubic_to(cx + ox, cy - ry, cx + rx, cy - oy, cx + rx, cy);
    pb.close();
    pb.finish()
}

fn parse_points(s: &str) -> Vec<(f32, f32)> {
    let n: Vec<f32> = s.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|x| !x.is_empty())
        .filter_map(parse_num)
        .collect();
    n.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0], c[1])).collect()
}

// ─────────────────────────────────────────────────────────────── path data (d)
fn parse_path_data(d: &str) -> Option<Path> {
    let mut pb = PathBuilder::new();
    let mut nums = NumScanner::new(d);
    let (mut cx, mut cy) = (0.0f32, 0.0f32); // punto actual
    let (mut sx, mut sy) = (0.0f32, 0.0f32); // inicio del subpath (para Z)
    let mut prev_cmd = ' ';
    let mut last_c2: Option<(f32, f32)> = None; // 2º control del cubic previo (para S)
    let mut last_q: Option<(f32, f32)> = None; // control del quad previo (para T)
    let mut started = false;

    loop {
        let cmd = match nums.next_cmd() {
            Some(c) => c,
            None => break,
        };
        let rel = cmd.is_ascii_lowercase();
        let up = cmd.to_ascii_uppercase();
        match up {
            'M' => {
                let (mut x, mut y) = (nums.num()?, nums.num()?);
                if rel { x += cx; y += cy; }
                pb.move_to(x, y);
                cx = x; cy = y; sx = x; sy = y; started = true;
                // Pares extra tras M/m son lineto implícitos.
                while nums.has_num() {
                    let (mut nx, mut ny) = (nums.num()?, nums.num()?);
                    if rel { nx += cx; ny += cy; }
                    pb.line_to(nx, ny);
                    cx = nx; cy = ny;
                }
                last_c2 = None; last_q = None;
            }
            'L' => {
                while nums.has_num() {
                    let (mut x, mut y) = (nums.num()?, nums.num()?);
                    if rel { x += cx; y += cy; }
                    pb.line_to(x, y);
                    cx = x; cy = y;
                }
                last_c2 = None; last_q = None;
            }
            'H' => {
                while nums.has_num() {
                    let mut x = nums.num()?;
                    if rel { x += cx; }
                    pb.line_to(x, cy);
                    cx = x;
                }
                last_c2 = None; last_q = None;
            }
            'V' => {
                while nums.has_num() {
                    let mut y = nums.num()?;
                    if rel { y += cy; }
                    pb.line_to(cx, y);
                    cy = y;
                }
                last_c2 = None; last_q = None;
            }
            'C' => {
                while nums.has_num() {
                    let (mut x1, mut y1) = (nums.num()?, nums.num()?);
                    let (mut x2, mut y2) = (nums.num()?, nums.num()?);
                    let (mut x, mut y) = (nums.num()?, nums.num()?);
                    if rel { x1 += cx; y1 += cy; x2 += cx; y2 += cy; x += cx; y += cy; }
                    pb.cubic_to(x1, y1, x2, y2, x, y);
                    cx = x; cy = y; last_c2 = Some((x2, y2)); last_q = None;
                }
            }
            'S' => {
                while nums.has_num() {
                    let (mut x2, mut y2) = (nums.num()?, nums.num()?);
                    let (mut x, mut y) = (nums.num()?, nums.num()?);
                    if rel { x2 += cx; y2 += cy; x += cx; y += cy; }
                    // Primer control = reflejo del 2º control previo si el previo fue C/S.
                    let (x1, y1) = if matches!(prev_cmd.to_ascii_uppercase(), 'C' | 'S') {
                        let (px, py) = last_c2.unwrap_or((cx, cy));
                        (2.0 * cx - px, 2.0 * cy - py)
                    } else { (cx, cy) };
                    pb.cubic_to(x1, y1, x2, y2, x, y);
                    cx = x; cy = y; last_c2 = Some((x2, y2)); last_q = None;
                }
            }
            'Q' => {
                while nums.has_num() {
                    let (mut x1, mut y1) = (nums.num()?, nums.num()?);
                    let (mut x, mut y) = (nums.num()?, nums.num()?);
                    if rel { x1 += cx; y1 += cy; x += cx; y += cy; }
                    pb.quad_to(x1, y1, x, y);
                    cx = x; cy = y; last_q = Some((x1, y1)); last_c2 = None;
                }
            }
            'T' => {
                while nums.has_num() {
                    let (mut x, mut y) = (nums.num()?, nums.num()?);
                    if rel { x += cx; y += cy; }
                    let (x1, y1) = if matches!(prev_cmd.to_ascii_uppercase(), 'Q' | 'T') {
                        let (px, py) = last_q.unwrap_or((cx, cy));
                        (2.0 * cx - px, 2.0 * cy - py)
                    } else { (cx, cy) };
                    pb.quad_to(x1, y1, x, y);
                    cx = x; cy = y; last_q = Some((x1, y1)); last_c2 = None;
                }
            }
            'A' => {
                while nums.has_num() {
                    let rx = nums.num()?;
                    let ry = nums.num()?;
                    let rot = nums.num()?;
                    let large = nums.flag()?;
                    let sweep = nums.flag()?;
                    let (mut x, mut y) = (nums.num()?, nums.num()?);
                    if rel { x += cx; y += cy; }
                    arc_to(&mut pb, cx, cy, rx, ry, rot, large != 0.0, sweep != 0.0, x, y);
                    cx = x; cy = y;
                }
                last_c2 = None; last_q = None;
            }
            'Z' => {
                if started { pb.close(); cx = sx; cy = sy; }
                last_c2 = None; last_q = None;
            }
            _ => break,
        }
        prev_cmd = cmd;
    }
    pb.finish()
}

/// Aproxima un arco elíptico SVG a cubics (algoritmo endpoint→center + segmentos).
#[allow(clippy::too_many_arguments)]
fn arc_to(pb: &mut PathBuilder, x0: f32, y0: f32, mut rx: f32, mut ry: f32, x_rot: f32, large: bool, sweep: bool, x: f32, y: f32) {
    if (x0 - x).abs() < 1e-6 && (y0 - y).abs() < 1e-6 { return; }
    if rx.abs() < 1e-6 || ry.abs() < 1e-6 {
        pb.line_to(x, y);
        return;
    }
    rx = rx.abs(); ry = ry.abs();
    let phi = x_rot.to_radians();
    let (sinp, cosp) = (phi.sin(), phi.cos());
    let dx = (x0 - x) / 2.0;
    let dy = (y0 - y) / 2.0;
    let x1p = cosp * dx + sinp * dy;
    let y1p = -sinp * dx + cosp * dy;
    // Corrección de radios fuera de rango.
    let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
    if lambda > 1.0 {
        let s = lambda.sqrt();
        rx *= s; ry *= s;
    }
    let sign = if large != sweep { 1.0 } else { -1.0 };
    let num = (rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p).max(0.0);
    let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
    let co = if den == 0.0 { 0.0 } else { sign * (num / den).sqrt() };
    let cxp = co * (rx * y1p / ry);
    let cyp = co * -(ry * x1p / rx);
    let cx = cosp * cxp - sinp * cyp + (x0 + x) / 2.0;
    let cy = sinp * cxp + cosp * cyp + (y0 + y) / 2.0;

    let ang = |ux: f32, uy: f32, vx: f32, vy: f32| -> f32 {
        let dot = ux * vx + uy * vy;
        let len = ((ux * ux + uy * uy) * (vx * vx + vy * vy)).sqrt();
        let mut a = (dot / len).clamp(-1.0, 1.0).acos();
        if ux * vy - uy * vx < 0.0 { a = -a; }
        a
    };
    let theta1 = ang(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry);
    let mut dtheta = ang((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry);
    if !sweep && dtheta > 0.0 { dtheta -= 2.0 * std::f32::consts::PI; }
    if sweep && dtheta < 0.0 { dtheta += 2.0 * std::f32::consts::PI; }

    let segs = (dtheta.abs() / (std::f32::consts::PI / 2.0)).ceil().max(1.0) as usize;
    let delta = dtheta / segs as f32;
    let t = (delta / 2.0).tan();
    let alpha = delta.sin() * ((4.0 + 3.0 * t * t).sqrt() - 1.0) / 3.0;
    let mut th = theta1;
    let (mut px, mut py) = {
        let e1x = cosp * rx * th.cos() - sinp * ry * th.sin() + cx;
        let e1y = sinp * rx * th.cos() + cosp * ry * th.sin() + cy;
        (e1x, e1y)
    };
    for _ in 0..segs {
        let th2 = th + delta;
        let (ex, ey) = {
            let e2x = cosp * rx * th2.cos() - sinp * ry * th2.sin() + cx;
            let e2y = sinp * rx * th2.cos() + cosp * ry * th2.sin() + cy;
            (e2x, e2y)
        };
        // Derivadas para los puntos de control.
        let d1x = -cosp * rx * th.sin() - sinp * ry * th.cos();
        let d1y = -sinp * rx * th.sin() + cosp * ry * th.cos();
        let d2x = -cosp * rx * th2.sin() - sinp * ry * th2.cos();
        let d2y = -sinp * rx * th2.sin() + cosp * ry * th2.cos();
        pb.cubic_to(px + alpha * d1x, py + alpha * d1y, ex - alpha * d2x, ey - alpha * d2y, ex, ey);
        px = ex; py = ey; th = th2;
    }
}

// ─────────────────────────────────────────────────────────────── colores/números
fn parse_color(s: &str) -> Option<[u8; 4]> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("none") || s.eq_ignore_ascii_case("transparent") {
        return None;
    }
    if s.eq_ignore_ascii_case("currentColor") {
        return Some([0, 0, 0, 255]); // sin sistema de "color actual": negro
    }
    if let Some(hex) = s.strip_prefix('#') {
        let h = hex.trim();
        if h.len() == 3 {
            let b = h.as_bytes();
            let r = hexval(b[0])? ; let g = hexval(b[1])?; let bl = hexval(b[2])?;
            return Some([r << 4 | r, g << 4 | g, bl << 4 | bl, 255]);
        }
        if h.len() == 6 {
            let r = u8::from_str_radix(&h[0..2], 16).ok()?;
            let g = u8::from_str_radix(&h[2..4], 16).ok()?;
            let b = u8::from_str_radix(&h[4..6], 16).ok()?;
            return Some([r, g, b, 255]);
        }
        return None;
    }
    if let Some(inner) = s.strip_prefix("rgb(").and_then(|x| x.strip_suffix(')')) {
        let v: Vec<f32> = inner.split(',').filter_map(parse_num).collect();
        if v.len() == 3 {
            return Some([v[0] as u8, v[1] as u8, v[2] as u8, 255]);
        }
        return None;
    }
    // colores nombrados frecuentes
    let named = match s.to_ascii_lowercase().as_str() {
        "black" => [0, 0, 0], "white" => [255, 255, 255], "red" => [255, 0, 0],
        "green" => [0, 128, 0], "blue" => [0, 0, 255], "gray" | "grey" => [128, 128, 128],
        "silver" => [192, 192, 192], "orange" => [255, 165, 0], "yellow" => [255, 255, 0],
        "purple" => [128, 0, 128], "cyan" | "aqua" => [0, 255, 255], "magenta" | "fuchsia" => [255, 0, 255],
        "navy" => [0, 0, 128], "teal" => [0, 128, 128], "lime" => [0, 255, 0], "maroon" => [128, 0, 0],
        "darkgray" | "darkgrey" => [169, 169, 169], "lightgray" | "lightgrey" => [211, 211, 211],
        _ => return None,
    };
    Some([named[0], named[1], named[2], 255])
}

fn hexval(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Parsea un número tolerando sufijos de unidad (px) y espacios.
fn parse_num(s: &str) -> Option<f32> {
    let t = s.trim();
    if t.is_empty() { return None; }
    let end = t.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E')).unwrap_or(t.len());
    t[..end].parse::<f32>().ok()
}

/// Escáner de números para path data: separa por espacios, comas y signos/decimales
/// implícitos ("10-5" = 10 y -5; "0.5.5" = 0.5 y .5).
struct NumScanner<'a> {
    b: &'a [u8],
    i: usize,
}
impl<'a> NumScanner<'a> {
    fn new(s: &'a str) -> Self { NumScanner { b: s.as_bytes(), i: 0 } }
    fn skip_sep(&mut self) {
        while self.i < self.b.len() {
            let c = self.b[self.i];
            if c == b' ' || c == b',' || c == b'\t' || c == b'\n' || c == b'\r' { self.i += 1; } else { break; }
        }
    }
    fn next_cmd(&mut self) -> Option<char> {
        self.skip_sep();
        while self.i < self.b.len() {
            let c = self.b[self.i];
            if c.is_ascii_alphabetic() {
                self.i += 1;
                return Some(c as char);
            }
            // no debería llegar aquí con números sueltos; los consume num()
            return None;
        }
        None
    }
    /// ¿Hay otro número antes del próximo comando?
    fn has_num(&mut self) -> bool {
        self.skip_sep();
        if self.i >= self.b.len() { return false; }
        let c = self.b[self.i];
        c.is_ascii_digit() || c == b'.' || c == b'-' || c == b'+'
    }
    fn num(&mut self) -> Option<f32> {
        self.skip_sep();
        let start = self.i;
        if self.i < self.b.len() && (self.b[self.i] == b'-' || self.b[self.i] == b'+') { self.i += 1; }
        let mut seen_dot = false;
        while self.i < self.b.len() {
            let c = self.b[self.i];
            if c.is_ascii_digit() { self.i += 1; }
            else if c == b'.' && !seen_dot { seen_dot = true; self.i += 1; }
            else if (c == b'e' || c == b'E') {
                self.i += 1;
                if self.i < self.b.len() && (self.b[self.i] == b'-' || self.b[self.i] == b'+') { self.i += 1; }
            } else { break; }
        }
        if self.i == start { return None; }
        std::str::from_utf8(&self.b[start..self.i]).ok()?.parse::<f32>().ok()
    }
    /// Un flag de arco (0 o 1), que puede venir pegado ("...110 5" → 1,1,0).
    fn flag(&mut self) -> Option<f32> {
        self.skip_sep();
        if self.i < self.b.len() {
            let c = self.b[self.i];
            if c == b'0' || c == b'1' {
                self.i += 1;
                return Some((c - b'0') as f32);
            }
        }
        None
    }
}

// ─────────────────────────────────────────────────────────────── scanner XML
struct Elem {
    name: String,
    attrs: Vec<(String, String)>,
    closing: bool,
    self_close: bool,
}

fn scan_elements(src: &str) -> Vec<Elem> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        // avanzar a '<'
        while i < b.len() && b[i] != b'<' { i += 1; }
        if i >= b.len() { break; }
        // comentarios / PI / DOCTYPE / CDATA
        if src[i..].starts_with("<!--") {
            if let Some(end) = src[i..].find("-->") { i += end + 3; } else { break; }
            continue;
        }
        if src[i..].starts_with("<?") {
            if let Some(end) = src[i..].find("?>") { i += end + 2; } else { break; }
            continue;
        }
        if src[i..].starts_with("<!") {
            if let Some(end) = src[i..].find('>') { i += end + 1; } else { break; }
            continue;
        }
        i += 1; // saltar '<'
        let closing = i < b.len() && b[i] == b'/';
        if closing { i += 1; }
        // nombre
        let ns = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' && b[i] != b'/' { i += 1; }
        let name = src[ns..i].to_string();
        // cuerpo hasta '>'
        let bs = i;
        while i < b.len() && b[i] != b'>' { i += 1; }
        let body = &src[bs..i.min(src.len())];
        if i < b.len() { i += 1; } // saltar '>'
        let self_close = body.trim_end().ends_with('/');
        let attrs = if closing { Vec::new() } else { parse_attrs(body) };
        out.push(Elem { name, attrs, closing, self_close });
    }
    out
}

fn parse_attrs(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') { i += 1; }
        let ns = i;
        while i < b.len() && b[i] != b'=' && !b[i].is_ascii_whitespace() && b[i] != b'/' && b[i] != b'>' { i += 1; }
        if ns == i { break; }
        let name = s[ns..i].to_string();
        while i < b.len() && b[i].is_ascii_whitespace() { i += 1; }
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() { i += 1; }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                i += 1;
                let vs = i;
                while i < b.len() && b[i] != q { i += 1; }
                let val = s[vs..i.min(s.len())].to_string();
                if i < b.len() { i += 1; }
                out.push((name, val));
            } else {
                let vs = i;
                while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' { i += 1; }
                out.push((name, s[vs..i].to_string()));
            }
        } else {
            out.push((name, String::new()));
        }
    }
    out
}
