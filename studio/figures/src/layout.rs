//! Figure → Scene: auto-placement (cells, layered layout for cd, blob
//! packing, n-gon and Cayley geometry), arrow routing (shortened to node
//! boxes, bends, loops), label boxes (estimated from the label's length at
//! 10 pt Charter: good enough for hit-testing; TikZ does the exact clipping).
//!
//! Scene conventions (what `tikz` emits and the canvas draws):
//! - Coordinates are final centimetres, y up, `scale` already applied.
//! - Colours are palette names: `ink grey blue violet teal maroon green
//!   ochre`; a fill may also be `<name>-tint` (the clear Back tone).
//! - `Draw::Label::at` is where TikZ puts the node: for diagram nodes that is
//!   the maths axis (tikz-cd's cells), for every other label the box centre.
//! - Anchors: every node, element, n-gon vertex, Cayley vertex, plus each
//!   blob, n-gon, Cayley graph and text, with an approximate box.
//! - Arrow / Cayley edge paths start and end on their objects' boxes (dots:
//!   radius + a hair), exactly where TikZ's clipping puts them when the
//!   estimated box is right.
use crate::model::*;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

/// Centimetres per TeX point.
pub const PT: f32 = 0.035_146;
/// tikz-cd cell padding (`inner xsep=1ex`, `inner ysep=0.85ex`), in pt.
const CELL_XSEP: f32 = 4.6;
const CELL_YSEP: f32 = 3.9;
/// The maths axis above the baseline (newtxmath), pt.
const AXIS: f32 = 2.5;
/// tikz-cd labels: `inner sep=0.5ex`, pt.
const LABEL_SEP: f32 = 2.3;
/// Element dots: radius (cm) and the gap an arrow leaves before one.
pub const DOT_R: f32 = 1.6 * PT;
const DOT_GAP: f32 = 1.8 * PT;
/// Labelled Cayley vertices: `inner sep` in pt.
pub const VERTEX_SEP: f32 = 2.2;
pub const VERTEX_DOT_R: f32 = 1.5 * PT;

// ---------------------------------------------------------------------------
// label size estimation (calibrated against XCharter + newtxmath at 10 pt)

/// Width, height above the baseline and depth below it, in pt at 10 pt
/// text style, of a LaTeX maths label (or text when `math` is false).
pub fn label_metrics(latex: &str, math: bool) -> (f32, f32, f32) {
    let toks: Vec<char> = latex.chars().collect();
    let mut m = Metrics::default();
    if math {
        measure(&toks, &mut 0, 1.0, &mut m, false);
    } else {
        for c in &toks {
            m.w += match c {
                ' ' => 2.6,
                'A'..='Z' => 6.6,
                'm' | 'w' => 7.2,
                'i' | 'l' | 'j' | 't' | 'f' | 'r' => 3.2,
                _ if c.is_ascii_digit() => 5.5,
                '$' | '\\' | '{' | '}' => 0.0,
                _ => 4.7,
            };
            if "gjpqy".contains(*c) {
                m.dp = m.dp.max(2.0);
            }
        }
        m.ht = m.ht.max(6.9);
    }
    (m.w.max(1.0), m.ht.max(4.5), m.dp.max(0.0))
}

#[derive(Default)]
struct Metrics {
    w: f32,
    ht: f32,
    dp: f32,
}

const GREEK_LOWER: &[&str] = &[
    "alpha", "beta", "gamma", "delta", "epsilon", "varepsilon", "zeta", "eta", "theta", "vartheta", "iota", "kappa", "lambda", "mu", "nu", "xi", "pi", "varpi", "rho", "varrho",
    "sigma", "varsigma", "tau", "upsilon", "phi", "varphi", "chi", "psi", "omega", "ell",
];
const OPERATORS: &[&str] = &["ker", "im", "Im", "det", "dim", "deg", "gcd", "lcm", "sin", "cos", "tan", "log", "exp", "lim", "max", "min", "sup", "inf", "Hom", "Aut", "End", "Inn", "Out", "Gal", "Spec", "id", "Id", "mod", "coker", "rank", "tr", "sgn", "Sym"];
const BINS: &[&str] = &["times", "circ", "cdot", "oplus", "otimes", "ast", "star", "cap", "cup", "setminus", "wedge", "vee", "pm", "rtimes", "ltimes", "bullet"];
const RELS: &[&str] = &[
    "cong", "simeq", "sim", "to", "rightarrow", "leftarrow", "mapsto", "subset", "subseteq", "supset", "supseteq", "in", "notin", "le", "leq", "ge", "geq", "ne", "neq", "equiv", "approx",
    "trianglelefteq", "lhd", "unlhd", "hookrightarrow", "twoheadrightarrow", "iff", "implies",
];

fn read_group(t: &[char], i: &mut usize) -> Vec<char> {
    // at '{': return the balanced content; else one token (a char or a \command)
    if *i >= t.len() {
        return vec![];
    }
    if t[*i] == '{' {
        let mut depth = 0;
        let start = *i + 1;
        while *i < t.len() {
            match t[*i] {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let g = t[start..*i].to_vec();
                        *i += 1;
                        return g;
                    }
                }
                _ => {}
            }
            *i += 1;
        }
        return t[start.min(t.len())..].to_vec();
    }
    if t[*i] == '\\' {
        let start = *i;
        *i += 1;
        while *i < t.len() && t[*i].is_ascii_alphabetic() {
            *i += 1;
        }
        if *i == start + 1 && *i < t.len() {
            *i += 1;
        }
        return t[start..*i].to_vec();
    }
    *i += 1;
    vec![t[*i - 1]]
}

fn char_width(c: char, upright: bool) -> f32 {
    match c {
        'M' | 'W' => 8.7,
        'I' => 4.0,
        'J' => 4.8,
        'A'..='Z' => 6.6,
        'm' => 7.6,
        'w' => 6.6,
        'i' | 'j' | 'l' => 3.0,
        'f' => if upright { 3.3 } else { 4.6 },
        't' => 3.5,
        'r' => if upright { 3.6 } else { 4.0 },
        'a'..='z' => if upright { 4.6 } else { 4.7 },
        '0'..='9' => 5.56,
        '(' | ')' | '[' | ']' => 4.0,
        '/' => 5.0,
        '|' => 2.8,
        '\'' => 2.7,
        ',' | ';' => 2.8 + 1.7,
        '.' => 2.6,
        '+' | '-' => 7.8 + 4.4,
        '=' | '<' | '>' => 7.8 + 5.6,
        ':' => 2.8 + 5.6,
        '!' => 3.0,
        '*' => 5.0,
        ' ' | '~' | '&' => 0.0,
        _ => 6.0,
    }
}

thread_local! {
    static MEASURE_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Nesting deeper than this (braces, scripts) is measured flat: real labels
/// never come near it, and pasted text may.
const MEASURE_MAX_DEPTH: usize = 40;

fn measure(t: &[char], i: &mut usize, s: f32, m: &mut Metrics, upright: bool) {
    let d = MEASURE_DEPTH.with(|c| c.get());
    if d >= MEASURE_MAX_DEPTH {
        // too deep: count what is left as plain characters
        m.w += t[(*i).min(t.len())..].iter().filter(|c| !matches!(c, '{' | '}')).map(|&c| char_width(c, upright) * s).sum::<f32>();
        *i = t.len();
        return;
    }
    MEASURE_DEPTH.with(|c| c.set(d + 1));
    measure_inner(t, i, s, m, upright);
    MEASURE_DEPTH.with(|c| c.set(d));
}

fn measure_inner(t: &[char], i: &mut usize, s: f32, m: &mut Metrics, upright: bool) {
    while *i < t.len() {
        let c = t[*i];
        match c {
            '{' => {
                let g = read_group(t, i);
                measure(&g, &mut 0, s, m, upright);
            }
            '}' => *i += 1,
            '^' | '_' => {
                *i += 1;
                let g = read_group(t, i);
                let mut sub = Metrics::default();
                measure(&g, &mut 0, s * 0.72, &mut sub, upright);
                if c == '^' {
                    // a prime-only superscript barely raises anything
                    m.ht = m.ht.max((4.6 + sub.ht) * s.min(1.0));
                } else {
                    m.dp = m.dp.max((1.6 + sub.ht * 0.6) * s);
                }
                // a sub and sup stacked share the width: approximate by max
                let next_is_pair = *i < t.len() && (t[*i] == '^' || t[*i] == '_') && t[*i] != c;
                if next_is_pair {
                    *i += 1;
                    let g2 = read_group(t, i);
                    let mut sub2 = Metrics::default();
                    measure(&g2, &mut 0, s * 0.72, &mut sub2, upright);
                    m.w += sub.w.max(sub2.w) + 0.5 * s;
                    m.ht = m.ht.max(8.6 * s);
                    m.dp = m.dp.max(2.0 * s);
                } else {
                    m.w += sub.w + 0.5 * s;
                }
            }
            '\\' => {
                let cmd: String = read_group(t, i).into_iter().skip(1).collect();
                let cmd = cmd.as_str();
                match cmd {
                    "mathbb" | "mathbf" | "mathrm" | "mathcal" | "mathfrak" | "mathsf" | "operatorname" | "text" | "textrm" | "mathit" | "boldsymbol" => {
                        let g = read_group(t, i);
                        let before = m.w;
                        if cmd == "mathbb" {
                            m.w += 7.3 * s * g.iter().filter(|c| c.is_alphanumeric()).count().max(1) as f32;
                        } else if cmd == "mathcal" || cmd == "mathfrak" {
                            m.w += 7.4 * s * g.iter().filter(|c| c.is_alphanumeric()).count().max(1) as f32;
                        } else {
                            measure(&g, &mut 0, s, m, cmd != "mathit");
                        }
                        if cmd == "operatorname" {
                            m.w += 1.7 * s;
                        }
                        let _ = before;
                        m.ht = m.ht.max(6.9 * s);
                    }
                    "bar" | "overline" | "hat" | "widehat" | "tilde" | "widetilde" | "vec" | "dot" | "underline" => {
                        let g = read_group(t, i);
                        measure(&g, &mut 0, s, m, upright);
                        m.ht = m.ht.max(9.0 * s);
                    }
                    "frac" | "tfrac" | "dfrac" => {
                        let a = read_group(t, i);
                        let b = read_group(t, i);
                        let (mut ma, mut mb) = (Metrics::default(), Metrics::default());
                        measure(&a, &mut 0, s * 0.72, &mut ma, upright);
                        measure(&b, &mut 0, s * 0.72, &mut mb, upright);
                        m.w += ma.w.max(mb.w) + 2.0 * s;
                        m.ht = m.ht.max((AXIS + 2.0 + ma.ht + ma.dp) * s);
                        m.dp = m.dp.max((mb.ht + mb.dp) * s);
                    }
                    "left" | "right" | "big" | "Big" | "bigl" | "bigr" | "displaystyle" | "textstyle" | "scriptstyle" | "limits" | "nolimits" => {}
                    "," | "thinspace" => m.w += 1.7 * s,
                    ":" | ">" | "medspace" => m.w += 2.2 * s,
                    ";" | "thickspace" => m.w += 2.8 * s,
                    "quad" => m.w += 10.0 * s,
                    "qquad" => m.w += 20.0 * s,
                    "!" => m.w -= 1.7 * s,
                    "prime" => m.w += 2.7 * s,
                    "{" | "}" => m.w += 5.0 * s,
                    "langle" | "rangle" => m.w += 3.9 * s,
                    "ldots" | "cdots" | "dots" => m.w += 12.0 * s,
                    "infty" => m.w += 9.0 * s,
                    "emptyset" | "varnothing" => m.w += 5.6 * s,
                    "partial" | "nabla" => m.w += 5.6 * s,
                    "mid" => m.w += 2.8 * s + 5.6 * s,
                    "lvert" | "rvert" | "vert" => m.w += 2.8 * s,
                    "cong" => m.w += (6.96 + if s < 1.0 { 0.0 } else { 5.6 }) * s,
                    _ if GREEK_LOWER.contains(&cmd) => {
                        m.w += match cmd {
                            "iota" => 2.7,
                            "varphi" | "phi" | "psi" | "omega" => 7.5,
                            "pi" | "sigma" | "rho" | "mu" | "nu" | "kappa" | "theta" | "tau" | "alpha" => 6.0,
                            _ => 5.4,
                        } * s;
                        if ["varphi", "phi", "psi", "beta", "zeta", "xi", "eta", "mu", "rho", "chi", "gamma"].contains(&cmd) {
                            m.dp = m.dp.max(2.0 * s);
                        }
                    }
                    _ if cmd.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && cmd.len() > 1 && cmd.chars().skip(1).all(|c| c.is_ascii_lowercase()) && !OPERATORS.contains(&cmd) => {
                        // \Gamma, \Delta, \Sigma, …
                        m.w += 6.7 * s;
                        m.ht = m.ht.max(6.9 * s);
                    }
                    _ if OPERATORS.contains(&cmd) => {
                        for ch in cmd.chars() {
                            m.w += char_width(ch, true) * s;
                        }
                        m.w += 1.7 * s;
                        m.ht = m.ht.max(7.0 * s);
                    }
                    _ if BINS.contains(&cmd) => m.w += (6.4 + if s < 1.0 { 0.0 } else { 4.4 }) * s,
                    _ if RELS.contains(&cmd) => m.w += (7.8 + if s < 1.0 { 0.0 } else { 5.6 }) * s,
                    _ => m.w += 6.0 * s,
                }
            }
            _ => {
                *i += 1;
                let mut w = char_width(c, upright);
                // binary/relation spacing vanishes in script style
                if s < 1.0 && matches!(c, '+' | '-' | '=' | '<' | '>' | ':') {
                    w = 7.8;
                }
                m.w += w * s;
                if c.is_ascii_uppercase() || c.is_ascii_digit() || "bdfhklt()[]/|".contains(c) {
                    m.ht = m.ht.max(6.9 * s);
                } else {
                    m.ht = m.ht.max(4.5 * s);
                }
                if "gjpqy()[]/|,;".contains(c) || (c == 'f' && !upright) {
                    m.dp = m.dp.max(2.0 * s);
                }
            }
        }
    }
}

/// A diagram node's box relative to its anchor (the maths axis), in cm.
pub fn node_box(latex: &str, size_pt: f32) -> Rect {
    let (w, ht, dp) = label_metrics(latex, true);
    let k = size_pt / 10.0;
    let hw = (w * k / 2.0 + CELL_XSEP) * PT;
    Rect {
        min: Pt { x: -hw, y: (-(dp * k) - CELL_YSEP - AXIS * k) * PT },
        max: Pt { x: hw, y: (ht * k + CELL_YSEP - AXIS * k) * PT },
    }
}

/// A centred label's half-size (cm) with `sep` pt of padding.
pub fn label_half(latex: &str, size_pt: f32, math: bool, sep: f32) -> (f32, f32) {
    let (w, ht, dp) = label_metrics(latex, math);
    let k = size_pt / 10.0;
    ((w * k / 2.0 + sep) * PT, ((ht + dp) * k / 2.0 + sep) * PT)
}

// ---------------------------------------------------------------------------
// geometry helpers

fn p(x: f32, y: f32) -> Pt {
    Pt { x, y }
}
fn add(a: Pt, b: Pt) -> Pt {
    p(a.x + b.x, a.y + b.y)
}
fn sub(a: Pt, b: Pt) -> Pt {
    p(a.x - b.x, a.y - b.y)
}
fn mul(a: Pt, k: f32) -> Pt {
    p(a.x * k, a.y * k)
}
fn len(a: Pt) -> f32 {
    (a.x * a.x + a.y * a.y).sqrt()
}
fn norm(a: Pt) -> Pt {
    let l = len(a);
    if l < 1e-6 { p(1.0, 0.0) } else { mul(a, 1.0 / l) }
}
fn dir(deg: f32) -> Pt {
    let r = deg.to_radians();
    p(r.cos(), r.sin())
}
/// `a` turned `deg` degrees counter-clockwise about the origin.
pub fn rot(a: Pt, deg: f32) -> Pt {
    if deg == 0.0 {
        return a;
    }
    let (s, c) = deg.to_radians().sin_cos();
    p(a.x * c - a.y * s, a.x * s + a.y * c)
}
fn angle(a: Pt) -> f32 {
    a.y.atan2(a.x).to_degrees()
}
fn lerp(a: Pt, b: Pt, t: f32) -> Pt {
    add(a, mul(sub(b, a), t))
}

/// Cubic Bézier point.
pub fn bezier(p0: Pt, c1: Pt, c2: Pt, p1: Pt, t: f32) -> Pt {
    let u = 1.0 - t;
    let a = u * u * u;
    let b = 3.0 * u * u * t;
    let c = 3.0 * u * t * t;
    let d = t * t * t;
    p(a * p0.x + b * c1.x + c * c2.x + d * p1.x, a * p0.y + b * c1.y + c * c2.y + d * p1.y)
}
/// Cubic Bézier tangent.
pub fn bezier_d(p0: Pt, c1: Pt, c2: Pt, p1: Pt, t: f32) -> Pt {
    let u = 1.0 - t;
    let a = 3.0 * u * u;
    let b = 6.0 * u * t;
    let c = 3.0 * t * t;
    p(a * (c1.x - p0.x) + b * (c2.x - c1.x) + c * (p1.x - c2.x), a * (c1.y - p0.y) + b * (c2.y - c1.y) + c * (p1.y - c2.y))
}

/// Flatten a path into points (for bounds, hit-testing, containment).
pub fn flatten(start: Pt, segs: &[Seg]) -> Vec<Pt> {
    let mut out = vec![start];
    let mut cur = start;
    for s in segs {
        match *s {
            Seg::Line(e) => {
                out.push(e);
                cur = e;
            }
            Seg::Cubic(c1, c2, e) => {
                for k in 1..=16 {
                    out.push(bezier(cur, c1, c2, e, k as f32 / 16.0));
                }
                cur = e;
            }
        }
    }
    out
}

/// Point and tangent at parameter t ∈ [0,1] along a path (by segment).
pub fn path_at(start: Pt, segs: &[Seg], t: f32) -> (Pt, Pt) {
    if segs.is_empty() {
        return (start, p(1.0, 0.0));
    }
    let n = segs.len() as f32;
    let k = ((t * n).floor() as usize).min(segs.len() - 1);
    let local = t * n - k as f32;
    let mut cur = start;
    for s in &segs[..k] {
        cur = match *s {
            Seg::Line(e) | Seg::Cubic(_, _, e) => e,
        };
    }
    match segs[k] {
        Seg::Line(e) => (lerp(cur, e, local), sub(e, cur)),
        Seg::Cubic(c1, c2, e) => (bezier(cur, c1, c2, e, local), bezier_d(cur, c1, c2, e, local)),
    }
}

pub fn point_in_poly(pt: Pt, poly: &[Pt]) -> bool {
    let mut inside = false;
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > pt.y) != (b.y > pt.y) && pt.x < (b.x - a.x) * (pt.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn dist_to_seg(q: Pt, a: Pt, b: Pt) -> f32 {
    let ab = sub(b, a);
    let l2 = ab.x * ab.x + ab.y * ab.y;
    let t = if l2 < 1e-9 { 0.0 } else { (((q.x - a.x) * ab.x + (q.y - a.y) * ab.y) / l2).clamp(0.0, 1.0) };
    len(sub(q, lerp(a, b, t)))
}

pub fn dist_to_poly(q: Pt, poly: &[Pt], closed: bool) -> f32 {
    let mut d = f32::MAX;
    for i in 0..poly.len().saturating_sub(1) {
        d = d.min(dist_to_seg(q, poly[i], poly[i + 1]));
    }
    if closed && poly.len() > 2 {
        d = d.min(dist_to_seg(q, poly[poly.len() - 1], poly[0]));
    }
    d
}

fn rect_union(a: &mut Option<Rect>, b: &Rect) {
    match a {
        None => *a = Some(b.clone()),
        Some(r) => {
            r.min.x = r.min.x.min(b.min.x);
            r.min.y = r.min.y.min(b.min.y);
            r.max.x = r.max.x.max(b.max.x);
            r.max.y = r.max.y.max(b.max.y);
        }
    }
}
fn rect_of(pts: &[Pt]) -> Rect {
    let mut r = Rect { min: p(f32::MAX, f32::MAX), max: p(f32::MIN, f32::MIN) };
    for q in pts {
        r.min.x = r.min.x.min(q.x);
        r.min.y = r.min.y.min(q.y);
        r.max.x = r.max.x.max(q.x);
        r.max.y = r.max.y.max(q.y);
    }
    r
}
fn rect_at(c: Pt, hw: f32, hh: f32) -> Rect {
    Rect { min: p(c.x - hw, c.y - hh), max: p(c.x + hw, c.y + hh) }
}
fn rect_shift(r: &Rect, d: Pt) -> Rect {
    Rect { min: add(r.min, d), max: add(r.max, d) }
}

/// Arc as cubic segments (centre, radius, degrees from a0 to a1, either way).
fn arc_segs(c: Pt, r: f32, a0: f32, a1: f32) -> (Pt, Vec<Seg>) {
    let span = a1 - a0;
    let n = ((span.abs() / 90.0).ceil() as usize).max(1);
    let step = span / n as f32;
    let k = 4.0 / 3.0 * (step.to_radians() / 4.0).tan();
    let start = add(c, mul(dir(a0), r));
    let mut segs = vec![];
    for i in 0..n {
        let t0 = a0 + step * i as f32;
        let t1 = t0 + step;
        let (p0, p1) = (add(c, mul(dir(t0), r)), add(c, mul(dir(t1), r)));
        let c1 = add(p0, mul(dir(t0 + 90.0), k * r));
        let c2 = sub(p1, mul(dir(t1 + 90.0), k * r));
        segs.push(Seg::Cubic(c1, c2, p1));
    }
    (start, segs)
}

/// Closed Catmull–Rom spline through `pts` as cubic segments.
fn smooth_closed(pts: &[Pt]) -> (Pt, Vec<Seg>) {
    let n = pts.len();
    let mut segs = vec![];
    for i in 0..n {
        let p0 = pts[(i + n - 1) % n];
        let p1 = pts[i];
        let p2 = pts[(i + 1) % n];
        let p3 = pts[(i + 2) % n];
        let c1 = add(p1, mul(sub(p2, p0), 1.0 / 6.0));
        let c2 = sub(p2, mul(sub(p3, p1), 1.0 / 6.0));
        segs.push(Seg::Cubic(c1, c2, p2));
    }
    (pts[0], segs)
}

// deterministic noise, seeded by a name
fn seed_of(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        (z >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
}

// ---------------------------------------------------------------------------
// the builder

/// What an arrow can attach to.
#[derive(Clone, Debug)]
enum Shape {
    /// a box (relative to the anchor point)
    Box(Rect),
    /// a dot / point of this radius
    Dot(f32),
    /// a closed curve (absolute points)
    Curve(Vec<Pt>),
}

#[derive(Clone, Debug)]
struct Target {
    obj: ObjRef,
    at: Pt,
    shape: Shape,
}

fn clip(t: &Target, toward: Pt) -> Pt {
    let d = sub(toward, t.at);
    if len(d) < 1e-6 {
        return t.at;
    }
    let u = norm(d);
    match &t.shape {
        Shape::Dot(r) => add(t.at, mul(u, *r)),
        Shape::Box(r) => {
            // ray from the anchor to the box border
            let mut s = f32::MAX;
            if u.x > 1e-6 {
                s = s.min(r.max.x / u.x);
            } else if u.x < -1e-6 {
                s = s.min(r.min.x / u.x);
            }
            if u.y > 1e-6 {
                s = s.min(r.max.y / u.y);
            } else if u.y < -1e-6 {
                s = s.min(r.min.y / u.y);
            }
            if !s.is_finite() || s < 0.0 {
                s = 0.0;
            }
            add(t.at, mul(u, s))
        }
        Shape::Curve(poly) => {
            // farthest crossing of the ray with the curve
            let mut best = 0.0f32;
            let n = poly.len();
            for i in 0..n {
                let (a, b) = (poly[i], poly[(i + 1) % n]);
                if let Some(s) = ray_seg(t.at, u, a, b) {
                    best = best.max(s);
                }
            }
            add(t.at, mul(u, best))
        }
    }
}

fn ray_seg(o: Pt, u: Pt, a: Pt, b: Pt) -> Option<f32> {
    let e = sub(b, a);
    let den = u.x * e.y - u.y * e.x;
    if den.abs() < 1e-9 {
        return None;
    }
    let w = sub(a, o);
    let s = (w.x * e.y - w.y * e.x) / den;
    let v = (w.x * u.y - w.y * u.x) / den;
    if s >= 0.0 && (0.0..=1.0).contains(&v) { Some(s) } else { None }
}

pub(crate) fn palette(c: &Option<String>, default: &str) -> String {
    match c.as_deref().map(|s| s.trim().to_ascii_lowercase()) {
        Some(s) if ["ink", "grey", "gray", "blue", "violet", "teal", "maroon", "green", "ochre", "black"].contains(&s.as_str()) => match s.as_str() {
            "gray" => "grey".into(),
            "black" => "ink".into(),
            _ => s,
        },
        _ => default.into(),
    }
}

struct B<'a> {
    fig: &'a Figure,
    cell: (f32, f32),
    scale: f32,
    size: f32,
    draws: Vec<Draw>,
    anchors: Vec<(ObjRef, Pt, Rect)>,
    targets: HashMap<String, Target>,
    bounds: Option<Rect>,
    shapes: Vec<ShapeFrame>,
}

impl B<'_> {
    fn grow(&mut self, r: &Rect) {
        rect_union(&mut self.bounds, r);
    }
    fn label(&mut self, obj: ObjRef, at: Pt, latex: &str, color: &str, size: f32, math: bool) -> Rect {
        let (hw, hh) = label_half(latex, size, math, 1.0);
        let r = rect_at(at, hw, hh);
        self.grow(&r);
        self.draws.push(Draw::Label { obj, at, latex: latex.to_string(), color: color.to_string(), size, math });
        r
    }
    fn path(&mut self, obj: ObjRef, start: Pt, segs: Vec<Seg>, closed: bool, stroke: Option<Stroke>, fill: Option<String>, tips: (Option<Head>, Option<Head>)) {
        let pts = flatten(start, &segs);
        let r = rect_of(&pts);
        self.grow(&r);
        self.draws.push(Draw::Path { obj, start, segs, closed, stroke, fill, fill_opacity: 1.0, start_tip: tips.0, end_tip: tips.1 });
    }
    fn pos_to_pt(&self, pos: Pos) -> Pt {
        match pos {
            Pos::Cell(c, r) => p(c * self.cell.0, -r * self.cell.1),
            Pos::Abs(x, y) => p(x * self.scale, y * self.scale),
        }
    }
    fn current_right(&self) -> Option<f32> {
        self.bounds.as_ref().map(|r| r.max.x)
    }
}

// ---------------------------------------------------------------------------
// layout

pub fn layout(fig: &Figure) -> Scene {
    let g = &fig.globals;
    let scale = if g.scale > 0.0 { g.scale } else { 1.0 };
    let spacing = if g.spacing.0 > 0.0 && g.spacing.1 > 0.0 { g.spacing } else { DEFAULT_SPACING };
    let size = match g.size.as_str() {
        "small" => 9.0,
        "large" => 12.0,
        _ => 10.0,
    };
    let mut b = B { fig, cell: (spacing.0 * scale, spacing.1 * scale), scale, size, draws: vec![], anchors: vec![], targets: HashMap::new(), bounds: None, shapes: vec![] };

    // 1. diagram nodes (explicit and implicit)
    let mut nodes: Vec<Node> = vec![];
    let mut seen: HashSet<String> = HashSet::new();
    for it in &fig.items {
        if let Item::Node(n) = it {
            if seen.insert(n.name.clone()) {
                nodes.push(n.clone());
            } else if let Some(m) = nodes.iter_mut().find(|m| m.name == n.name) {
                if m.pos.is_none() {
                    m.pos = n.pos;
                }
                if m.color.is_none() {
                    m.color = n.color.clone();
                }
            }
        }
    }
    let other_names = other_object_names(fig);
    for it in &fig.items {
        if let Item::Arrow(a) = it {
            for nm in [&a.from, &a.to] {
                if !seen.contains(nm) && !other_names.contains(nm) && !nm.is_empty() {
                    seen.insert(nm.clone());
                    nodes.push(Node { name: nm.clone(), label: nm.clone(), pos: None, color: None, span: a.span });
                }
            }
        }
    }
    layout_nodes(&mut b, &nodes);

    // 2. blobs, 3. n-gons, 4. Cayley graphs, each after what's already placed
    layout_blobs(&mut b);
    layout_ngons(&mut b);
    layout_cayleys(&mut b);

    // 5. arrows (several straight arrows into one dot arrive spread around it)
    let arrivals = arrival_angles(&b, fig);
    let shifts = parallel_shifts(fig);
    for (i, it) in fig.items.iter().enumerate() {
        if let Item::Arrow(a) = it {
            route_arrow(&mut b, i, a, arrivals.get(&i).copied(), shifts.get(&i).copied().unwrap_or(0.0));
        }
    }

    // 6. free text
    for (i, it) in fig.items.iter().enumerate() {
        if let Item::Text(t) = it {
            let at = b.pos_to_pt(t.pos);
            let k = match t.size.as_deref() {
                Some("tiny") => 0.75,
                Some("small") => 0.85,
                Some("large") => 1.2,
                Some("huge") => 1.44,
                _ => 1.0,
            };
            let r = b.label(ObjRef::Text(i), at, &t.text, t.color.as_deref().unwrap_or("ink"), size * k, false);
            b.anchors.push((ObjRef::Text(i), at, r));
        }
    }

    // shape frames in item order
    let item_of = |o: &ObjRef| -> usize {
        fig.items
            .iter()
            .position(|it| match (it, o) {
                (Item::Blob(x), ObjRef::Blob(n)) => x.name == *n,
                (Item::NGon(x), ObjRef::NGon(n)) => x.name == *n,
                (Item::Cayley(x), ObjRef::Cayley(n)) => x.name == *n,
                _ => false,
            })
            .unwrap_or(usize::MAX)
    };
    b.shapes.sort_by_key(|f| item_of(&f.obj));
    let mut bounds = b.bounds.clone().unwrap_or(Rect { min: p(0.0, 0.0), max: p(1.0, 1.0) });
    let pad = 0.08;
    bounds.min.x -= pad;
    bounds.min.y -= pad;
    bounds.max.x += pad;
    bounds.max.y += pad;
    Scene { draws: b.draws, anchors: b.anchors, bounds, cell: b.cell, shapes: b.shapes, scale }
}

fn other_object_names(fig: &Figure) -> HashSet<String> {
    let mut s = HashSet::new();
    for it in &fig.items {
        match it {
            Item::Blob(bl) => {
                s.insert(bl.name.clone());
                for e in &bl.elements {
                    s.insert(e.name.clone());
                }
            }
            Item::NGon(n) => {
                s.insert(n.name.clone());
            }
            Item::Cayley(c) => {
                s.insert(c.name.clone());
            }
            _ => {}
        }
    }
    // dotted names (P.1, C.rs) refer to n-gon / Cayley vertices
    let prefixes: Vec<String> = s.iter().cloned().collect();
    for it in &fig.items {
        if let Item::Arrow(a) = it {
            for nm in [&a.from, &a.to] {
                if let Some((pre, _)) = nm.split_once('.') {
                    if prefixes.iter().any(|x| x == pre) {
                        s.insert(nm.clone());
                    }
                }
            }
        }
    }
    s
}

// ---------------------------------------------------------------------------
// commutative-diagram nodes

fn layout_nodes(b: &mut B, nodes: &[Node]) {
    if nodes.is_empty() {
        return;
    }
    let idx: HashMap<&str, usize> = nodes.iter().enumerate().map(|(i, n)| (n.name.as_str(), i)).collect();
    let mut edges: Vec<(usize, usize)> = vec![];
    for it in &b.fig.items {
        if let Item::Arrow(a) = it {
            if let (Some(&x), Some(&y)) = (idx.get(a.from.as_str()), idx.get(a.to.as_str())) {
                if x != y && !edges.contains(&(x, y)) {
                    edges.push((x, y));
                }
            }
        }
    }
    let mut pos: Vec<Option<Pt>> = nodes.iter().map(|n| n.pos.map(|q| b.pos_to_pt(q))).collect();
    // cells for collision checks: (col,row) of placed nodes on integer cells
    let unplaced: Vec<usize> = (0..nodes.len()).filter(|&i| pos[i].is_none()).collect();
    if !unplaced.is_empty() {
        let any_placed = pos.iter().any(|q| q.is_some());
        let cells = auto_cells(nodes.len(), &edges, &pos, b.cell, any_placed);
        // components with no placed node are shifted right of what's placed
        for &i in &unplaced {
            if let Some((c, r)) = cells[i] {
                pos[i] = Some(p(c as f32 * b.cell.0, -(r as f32) * b.cell.1));
            }
        }
    }
    for (i, n) in nodes.iter().enumerate() {
        let at = pos[i].unwrap_or(p(0.0, 0.0));
        let color = palette(&n.color, "ink");
        let bx = node_box(&n.label, b.size);
        let r = rect_shift(&bx, at);
        b.grow(&r);
        b.draws.push(Draw::Label { obj: ObjRef::Node(n.name.clone()), at, latex: n.label.clone(), color, size: b.size, math: true });
        b.anchors.push((ObjRef::Node(n.name.clone()), at, r));
        b.targets.insert(n.name.clone(), Target { obj: ObjRef::Node(n.name.clone()), at, shape: Shape::Box(bx) });
    }
}

/// Integer cells for unplaced nodes: near their placed neighbours when some
/// nodes are placed, otherwise a layered layout per connected component
/// (triangle, square, chain left to right, fan).
fn auto_cells(n: usize, edges: &[(usize, usize)], pos: &[Option<Pt>], cell: (f32, f32), any_placed: bool) -> Vec<Option<(i32, i32)>> {
    let mut out: Vec<Option<(i32, i32)>> = vec![None; n];
    let mut taken: HashSet<(i32, i32)> = HashSet::new();
    for (i, q) in pos.iter().enumerate() {
        if let Some(q) = q {
            let c = ((q.x / cell.0).round() as i32, (-q.y / cell.1).round() as i32);
            taken.insert(c);
            out[i] = Some(c);
        }
    }
    // undirected adjacency
    let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
    for &(x, y) in edges {
        adj[x].push(y);
        adj[y].push(x);
    }
    // components
    let mut comp = vec![usize::MAX; n];
    let mut comps: Vec<Vec<usize>> = vec![];
    for s in 0..n {
        if comp[s] != usize::MAX {
            continue;
        }
        let id = comps.len();
        let mut q = VecDeque::from([s]);
        comp[s] = id;
        let mut members = vec![];
        while let Some(v) = q.pop_front() {
            members.push(v);
            for &w in &adj[v] {
                if comp[w] == usize::MAX {
                    comp[w] = id;
                    q.push_back(w);
                }
            }
        }
        members.sort();
        comps.push(members);
    }
    let _ = any_placed;
    for members in &comps {
        let has_placed = members.iter().any(|&v| pos[v].is_some());
        if has_placed {
            // grow outwards from placed nodes along arrows
            let mut frontier: VecDeque<usize> = members.iter().copied().filter(|&v| out[v].is_some()).collect();
            while let Some(v) = frontier.pop_front() {
                let (c, r) = out[v].unwrap();
                for &(x, y) in edges {
                    let (w, forward) = if x == v { (y, true) } else if y == v { (x, false) } else { continue };
                    if out[w].is_some() {
                        continue;
                    }
                    let prefs: [(i32, i32); 8] = if forward { [(1, 0), (0, 1), (1, 1), (1, -1), (0, -1), (-1, 1), (-1, 0), (-1, -1)] } else { [(-1, 0), (0, -1), (-1, -1), (-1, 1), (0, 1), (1, -1), (1, 0), (1, 1)] };
                    let mut chosen = None;
                    for ring in 1..6 {
                        for &(dc, dr) in &prefs {
                            let cand = (c + dc * ring, r + dr * ring);
                            if !taken.contains(&cand) {
                                chosen = Some(cand);
                                break;
                            }
                        }
                        if chosen.is_some() {
                            break;
                        }
                    }
                    let cand = chosen.unwrap_or((c + 1, r));
                    taken.insert(cand);
                    out[w] = Some(cand);
                    frontier.push_back(w);
                }
            }
            continue;
        }
        // a free component: lay it out at the origin, then shift it right of
        // everything taken so far
        let local = layered(members, edges);
        let min_c = local.values().map(|c| c.0).min().unwrap_or(0);
        let min_r = local.values().map(|c| c.1).min().unwrap_or(0);
        let shift_c = if taken.is_empty() { 0 } else { taken.iter().map(|c| c.0).max().unwrap() + 1 - min_c };
        let shift_r = -min_r;
        for (&v, &(c, r)) in &local {
            let cc = (c + shift_c, r + shift_r);
            out[v] = Some(cc);
        }
        for &v in members {
            if let Some(cc) = out[v] {
                taken.insert(cc);
            }
        }
    }
    out
}

/// Layered layout of one component (local integer cells).
fn layered(members: &[usize], edges: &[(usize, usize)]) -> BTreeMap<usize, (i32, i32)> {
    let set: HashSet<usize> = members.iter().copied().collect();
    let es: Vec<(usize, usize)> = edges.iter().copied().filter(|(x, y)| set.contains(x) && set.contains(y)).collect();
    let mut out = BTreeMap::new();
    if members.len() == 1 {
        out.insert(members[0], (0, 0));
        return out;
    }
    let indeg = |v: usize| es.iter().filter(|e| e.1 == v).count();
    let outdeg = |v: usize| es.iter().filter(|e| e.0 == v).count();
    let mut undirected: HashSet<(usize, usize)> = HashSet::new();
    for &(x, y) in &es {
        undirected.insert((x.min(y), x.max(y)));
    }
    // a triangle: source top-left, middle top-right, sink below it
    if members.len() == 3 && undirected.len() == 3 {
        let src = members.iter().copied().max_by_key(|&v| (outdeg(v) as i32 - indeg(v) as i32, -(v as i32))).unwrap();
        let sink = members.iter().copied().filter(|&v| v != src).max_by_key(|&v| (indeg(v) as i32 - outdeg(v) as i32, v as i32)).unwrap();
        let mid = members.iter().copied().find(|&v| v != src && v != sink).unwrap();
        out.insert(src, (0, 0));
        out.insert(mid, (1, 0));
        out.insert(sink, (1, 1));
        return out;
    }
    // a square: a 4-cycle (ignoring direction)
    if members.len() == 4 && undirected.len() == 4 && members.iter().all(|&v| undirected.iter().filter(|e| e.0 == v || e.1 == v).count() == 2) {
        let src = members.iter().copied().max_by_key(|&v| (outdeg(v) as i32 - indeg(v) as i32, -(v as i32))).unwrap();
        // its two neighbours, in the order of the arrows in the file
        let mut nb: Vec<usize> = vec![];
        for &(x, y) in &es {
            let w = if x == src { y } else if y == src { x } else { continue };
            if !nb.contains(&w) {
                nb.push(w);
            }
        }
        let opp = members.iter().copied().find(|&v| v != src && !nb.contains(&v)).unwrap();
        out.insert(src, (0, 0));
        out.insert(nb[0], (1, 0));
        out.insert(nb[1], (0, 1));
        out.insert(opp, (1, 1));
        return out;
    }
    // general: longest-path layers (cycles broken by DFS order) → columns
    let mut order: Vec<usize> = vec![];
    let mut state: HashMap<usize, u8> = HashMap::new();
    let mut back: HashSet<(usize, usize)> = HashSet::new();
    fn dfs(v: usize, es: &[(usize, usize)], state: &mut HashMap<usize, u8>, order: &mut Vec<usize>, back: &mut HashSet<(usize, usize)>) {
        state.insert(v, 1);
        for &(x, y) in es {
            if x == v {
                match state.get(&y).copied().unwrap_or(0) {
                    0 => dfs(y, es, state, order, back),
                    1 => {
                        back.insert((x, y));
                    }
                    _ => {}
                }
            }
        }
        state.insert(v, 2);
        order.push(v);
    }
    let mut roots: Vec<usize> = members.iter().copied().filter(|&v| indeg(v) == 0).collect();
    roots.extend(members.iter().copied());
    for v in roots {
        if !state.contains_key(&v) {
            dfs(v, &es, &mut state, &mut order, &mut back);
        }
    }
    order.reverse(); // topological
    let dag: Vec<(usize, usize)> = es.iter().copied().filter(|e| !back.contains(e)).collect();
    let mut col: HashMap<usize, i32> = HashMap::new();
    for &v in &order {
        let c = dag.iter().filter(|e| e.1 == v).map(|e| col.get(&e.0).copied().unwrap_or(0) + 1).max().unwrap_or(0);
        col.insert(v, c);
    }
    // pull sources right next to their first child (so a fan's tip isn't far left)
    for &v in order.iter().rev() {
        if dag.iter().all(|e| e.1 != v) {
            if let Some(mc) = dag.iter().filter(|e| e.0 == v).map(|e| col[&e.1]).min() {
                col.insert(v, mc - 1);
            }
        }
    }
    let maxc = col.values().copied().max().unwrap_or(0);
    let mut row: HashMap<usize, f32> = HashMap::new();
    let mut taken: HashSet<(i32, i32)> = HashSet::new();
    for c in col.values().copied().min().unwrap_or(0)..=maxc {
        let mut in_col: Vec<usize> = order.iter().copied().filter(|v| col[v] == c).collect();
        // desired row: mean of placed predecessors (or successors' later)
        let want = |v: usize, row: &HashMap<usize, f32>| -> f32 {
            let preds: Vec<f32> = dag.iter().filter(|e| e.1 == v).filter_map(|e| row.get(&e.0).copied()).collect();
            if preds.is_empty() { 0.0 } else { preds.iter().sum::<f32>() / preds.len() as f32 }
        };
        in_col.sort_by(|&a, &b2| want(a, &row).partial_cmp(&want(b2, &row)).unwrap().then(a.cmp(&b2)));
        // a node with several children: fan the children around its row
        let k = in_col.len() as i32;
        let centre = if in_col.is_empty() { 0.0 } else { in_col.iter().map(|&v| want(v, &row)).sum::<f32>() / k as f32 };
        // the first stays on its parent's row, the rest fan out below (an odd
        // fan is centred)
        let first = (centre - (k - 1) as f32 / 2.0).ceil() as i32;
        for (j, &v) in in_col.iter().enumerate() {
            let mut r = first + j as i32;
            while taken.contains(&(c, r)) {
                r += 1;
            }
            taken.insert((c, r));
            row.insert(v, r as f32);
        }
    }
    // a source whose children were fanned sits at their median row
    for &v in &order {
        let kids: Vec<f32> = dag.iter().filter(|e| e.0 == v).map(|e| row[&e.1]).collect();
        let is_source = dag.iter().all(|e| e.1 != v);
        if is_source && kids.len() >= 2 {
            let mut ks = kids.clone();
            ks.sort_by(|a, b2| a.partial_cmp(b2).unwrap());
            let med = ks[(ks.len() - 1) / 2].round() as i32;
            let cur = row[&v] as i32;
            let c = col[&v];
            if med != cur && !taken.contains(&(c, med)) {
                taken.remove(&(c, cur));
                taken.insert((c, med));
                row.insert(v, med as f32);
            }
        }
    }
    for &v in members {
        out.insert(v, (col[&v], row[&v] as i32));
    }
    out
}

// ---------------------------------------------------------------------------
// blobs

struct BlobLocal {
    /// semi-axes (cm)
    a: f32,
    bb: f32,
    curve: Vec<Pt>, // jittered control points, relative to the centre
    /// element name → (dot position relative to the centre, label on the left?)
    elems: Vec<(Element, Pt, bool)>,
    /// child name → offset of its centre
    kids: Vec<(String, Pt)>,
    fixed: bool,
}

fn blob_points(name: &str, shape: BlobShape, a: f32, bb: f32) -> Vec<Pt> {
    let mut rng = Rng(seed_of(name));
    let n = 6 + (rng.next() * 3.0).floor().min(2.0) as usize;
    let off = rng.range(0.0, 360.0);
    let mut pts = vec![];
    for i in 0..n {
        let base = off + 360.0 * i as f32 / n as f32 + rng.range(-0.16, 0.16) * 360.0 / n as f32;
        let mut f = rng.range(0.955, 1.06);
        let d = dir(base);
        if shape == BlobShape::Bean {
            // a dent in the top middle
            let up = d.y.max(0.0) * (1.0 - d.x.abs());
            f *= 1.0 - 0.32 * up.powf(1.5);
        }
        pts.push(p(d.x * a * f, d.y * bb * f));
    }
    pts
}

fn blob_size_aspect(shape: BlobShape, a: f32, bb: f32) -> (f32, f32) {
    match shape {
        BlobShape::Round => {
            let m = a.max(bb / 1.1);
            (m.max(a), (m * 1.1).max(bb))
        }
        BlobShape::Tall => (a.max(bb / 1.55), bb.max(a * 1.55)),
        BlobShape::Wide | BlobShape::Bean => (a.max(bb * 1.5), bb.max(a / 1.5)),
    }
}

fn layout_blob_local(b: &B, name: &str, blobs: &[&Blob], out_labels_left: &HashSet<String>, memo: &mut HashMap<String, BlobLocal>) {
    let bl = *blobs.iter().find(|x| x.name == name).unwrap();
    let kids: Vec<&Blob> = blobs.iter().copied().filter(|x| x.parent.as_deref() == Some(name)).collect();
    for k in &kids {
        if !memo.contains_key(&k.name) {
            layout_blob_local(b, &k.name, blobs, out_labels_left, memo);
        }
    }
    let lsize = b.size;
    // the elements' cells
    let ne = bl.elements.len();
    let elem_w: f32 = bl.elements.iter().map(|e| label_half(&e.label, lsize, true, 0.0).0 * 2.0).fold(0.0, f32::max);
    let cell_w = (elem_w + 0.12 + DOT_R * 2.0 + 0.32).max(0.75);
    let cell_h = 0.58;
    let cols = if !kids.is_empty() {
        let kw: f32 = kids.iter().map(|k| memo[&k.name].a * 2.0).sum::<f32>() + 0.3 * (kids.len() as f32 - 1.0);
        ((kw / cell_w).floor() as usize).clamp(1, ne.max(1))
    } else if ne <= 4 {
        1
    } else if ne <= 8 {
        2
    } else {
        3
    };
    let rows = if ne == 0 { 0 } else { ne.div_ceil(cols) };
    let grid_w = cols as f32 * cell_w;
    let grid_h = rows as f32 * cell_h;
    // children in a row
    let kid_sizes: Vec<(f32, f32)> = kids.iter().map(|k| turned_half(&memo[&k.name], k.angle)).collect();
    let row_w: f32 = kid_sizes.iter().map(|s| s.0 * 2.0).sum::<f32>() + 0.3 * (kids.len().max(1) as f32 - 1.0);
    let row_h: f32 = kid_sizes.iter().map(|s| s.1 * 2.0).fold(0.0, f32::max);
    let gap = if rows > 0 && !kids.is_empty() { 0.18 } else { 0.0 };
    let content_w = grid_w.max(row_w);
    let content_h = grid_h + gap + row_h;
    // element positions (grid on top, children below), centred on (0,0)
    let top = content_h / 2.0;
    let mut elems = vec![];
    for (i, e) in bl.elements.iter().enumerate() {
        let (r, c) = (i / cols, i % cols);
        let in_row = if r == rows - 1 { ne - r * cols } else { cols };
        let x0 = -(in_row as f32) * cell_w / 2.0 + (c as f32 + 0.5) * cell_w;
        // a slight stagger in single columns, so a column of points looks drawn
        let stagger = if cols == 1 && ne > 2 { if i % 2 == 0 { -0.1 } else { 0.1 } } else { 0.0 };
        let y = top - (r as f32 + 0.5) * cell_h;
        let left = out_labels_left.contains(&e.name);
        let lw = label_half(&e.label, lsize, true, 0.0).0 * 2.0;
        // centre the dot + label pair in its cell
        let pair = DOT_R * 2.0 + 0.1 + lw;
        let dx = if left { pair / 2.0 - DOT_R } else { -pair / 2.0 + DOT_R };
        elems.push((e.clone(), p(x0 + dx + stagger, y), left));
    }
    let mut kid_offsets = vec![];
    let mut x = -row_w / 2.0;
    let ky = top - grid_h - gap - row_h / 2.0;
    for (k, s) in kids.iter().zip(&kid_sizes) {
        kid_offsets.push((k.name.clone(), p(x + s.0, ky)));
        x += s.0 * 2.0 + 0.3;
    }
    // the outline: an ellipse around the content, grown until all fits
    let (a, bb, fixed) = if let Some((w, h)) = bl.size {
        (w * b.scale / 2.0, h * b.scale / 2.0, true)
    } else {
        let a0 = (content_w / 2.0 + 0.32).max(0.75);
        let b0 = (content_h / 2.0 + 0.3).max(0.8);
        let (mut a, mut bb) = blob_size_aspect(bl.shape, a0, b0);
        // what must be inside
        let mut must: Vec<Pt> = vec![];
        for (e, at, left) in &elems {
            let (hw, hh) = label_half(&e.label, lsize, true, 0.0);
            let lc = if *left { p(at.x - DOT_R - 0.1 - hw, at.y) } else { p(at.x + DOT_R + 0.1 + hw, at.y) };
            for q in [p(lc.x - hw, lc.y - hh), p(lc.x + hw, lc.y + hh), p(lc.x - hw, lc.y + hh), p(lc.x + hw, lc.y - hh), *at] {
                must.push(q);
            }
        }
        for (kn, off) in &kid_offsets {
            let kl = &memo[kn];
            let ka = kids.iter().find(|k| &k.name == kn).map(|k| k.angle).unwrap_or(0.0);
            let curve: Vec<Pt> = kl.curve.iter().map(|q| rot(*q, ka)).collect();
            let (s, segs) = smooth_closed(&curve);
            for q in flatten(s, &segs).iter().step_by(4) {
                must.push(add(*q, *off));
            }
        }
        for _ in 0..40 {
            let (s, segs) = smooth_closed(&blob_points(name, bl.shape, a, bb));
            let poly = flatten(s, &segs);
            if must.iter().all(|q| point_in_poly(*q, &poly) && dist_to_poly(*q, &poly, true) > 0.09) {
                break;
            }
            a *= 1.05;
            bb *= 1.05;
        }
        (a, bb, false)
    };
    memo.insert(name.to_string(), BlobLocal { a, bb, curve: blob_points(name, bl.shape, a, bb), elems, kids: kid_offsets, fixed });
}

fn layout_blobs(b: &mut B) {
    let blobs: Vec<&Blob> = b.fig.items.iter().filter_map(|it| if let Item::Blob(x) = it { Some(x) } else { None }).collect();
    if blobs.is_empty() {
        return;
    }
    let names: HashSet<&str> = blobs.iter().map(|x| x.name.as_str()).collect();
    // element arrows decide which side an element's label goes on: a point
    // that only sends arrows has its label on the left (the domain side)
    let mut outs: HashSet<String> = HashSet::new();
    let mut ins: HashSet<String> = HashSet::new();
    let elem_names: HashSet<String> = blobs.iter().flat_map(|x| x.elements.iter().map(|e| e.name.clone())).collect();
    let mut blob_arrows = false;
    for it in &b.fig.items {
        if let Item::Arrow(a) = it {
            if elem_names.contains(&a.from) {
                outs.insert(a.from.clone());
            }
            if elem_names.contains(&a.to) {
                ins.insert(a.to.clone());
            }
            if names.contains(a.from.as_str()) || names.contains(a.to.as_str()) || elem_names.contains(&a.from) {
                blob_arrows = true;
            }
        }
    }
    let left: HashSet<String> = outs.difference(&ins).cloned().collect();
    let mut memo: HashMap<String, BlobLocal> = HashMap::new();
    let tops: Vec<&Blob> = blobs.iter().copied().filter(|x| x.parent.as_deref().is_none_or(|p| !names.contains(p))).collect();
    for t in &tops {
        layout_blob_local(b, &t.name, &blobs, &left, &mut memo);
    }
    // a codomain's points (only arrived at, never left) are ordered by the
    // height of what maps to them, so element arrows don't cross needlessly
    let mut height: HashMap<String, f32> = HashMap::new();
    fn heights(name: &str, cy: f32, memo: &HashMap<String, BlobLocal>, out: &mut HashMap<String, f32>) {
        let l = &memo[name];
        for (e, at, _) in &l.elems {
            out.insert(e.name.clone(), cy + at.y);
        }
        for (k, off) in &l.kids {
            heights(k, cy + off.y, memo, out);
        }
    }
    for t in &tops {
        heights(&t.name, 0.0, &memo, &mut height);
    }
    let mut owned: Vec<Blob> = blobs.iter().map(|x| (*x).clone()).collect();
    let mut changed = false;
    for bl in owned.iter_mut() {
        if bl.elements.len() < 2 || bl.elements.iter().any(|e| e.pos.is_some() || outs.contains(&e.name)) {
            continue;
        }
        let key = |e: &Element| -> f32 {
            let hs: Vec<f32> = b.fig.items.iter().filter_map(|it| match it {
                Item::Arrow(a) if a.to == e.name => height.get(&a.from).copied(),
                _ => None,
            }).collect();
            if hs.is_empty() { f32::NAN } else { hs.iter().sum::<f32>() / hs.len() as f32 }
        };
        if bl.elements.iter().any(|e| key(e).is_nan()) {
            continue;
        }
        let before: Vec<String> = bl.elements.iter().map(|e| e.name.clone()).collect();
        bl.elements.sort_by(|x, y| key(y).partial_cmp(&key(x)).unwrap_or(std::cmp::Ordering::Equal));
        if bl.elements.iter().map(|e| e.name.clone()).collect::<Vec<_>>() != before {
            changed = true;
        }
    }
    let blobs: Vec<&Blob> = owned.iter().collect();
    let tops: Vec<&Blob> = blobs.iter().copied().filter(|x| x.parent.as_deref().is_none_or(|p| !names.contains(p))).collect();
    if changed {
        memo.clear();
        for t in &tops {
            layout_blob_local(b, &t.name, &blobs, &left, &mut memo);
        }
    }
    // top-level blobs in a row
    let gap = if blob_arrows { 2.1 } else { 0.9 };
    let start_x = b.current_right().map(|r| r + 1.2);
    let mut x = start_x.unwrap_or(0.0);
    let mut first = true;
    for t in &tops {
        let l = &memo[&t.name];
        let la = turned_half(l, t.angle).0;
        let centre = if let Some(pos) = t.pos {
            b.pos_to_pt(pos)
        } else {
            if !first {
                x += gap;
            }
            let c = p(x + la, 0.0);
            x = c.x + la;
            c
        };
        if t.pos.is_some() {
            x = x.max(centre.x + la);
        }
        first = false;
        emit_blob(b, t, centre, &memo, &blobs, 0, 0.0);
    }
}

/// A blob's half extents once turned `deg` degrees: its semi-axes when
/// unturned, else the box of the turned outline.
fn turned_half(l: &BlobLocal, deg: f32) -> (f32, f32) {
    if deg == 0.0 || !deg.is_finite() {
        return (l.a, l.bb);
    }
    let pts: Vec<Pt> = l.curve.iter().map(|q| rot(*q, deg)).collect();
    let (s, segs) = smooth_closed(&pts);
    let poly = flatten(s, &segs);
    let hx = poly.iter().map(|q| q.x.abs()).fold(0.0, f32::max);
    let hy = poly.iter().map(|q| q.y.abs()).fold(0.0, f32::max);
    (hx, hy)
}

/// `inherited`: how far the enclosing blobs turn this one (a child turns
/// with its parent); the blob's own `angle` adds to it.
#[allow(clippy::too_many_arguments)]
fn emit_blob(b: &mut B, bl: &Blob, centre: Pt, memo: &HashMap<String, BlobLocal>, blobs: &[&Blob], depth: usize, inherited: f32) {
    let l = &memo[&bl.name];
    let own = if bl.angle.is_finite() { bl.angle } else { 0.0 };
    let turn = inherited + own;
    let pts: Vec<Pt> = l.curve.iter().map(|q| add(rot(*q, turn), centre)).collect();
    let (s, segs) = smooth_closed(&pts);
    let poly = flatten(s, &segs);
    let color = palette(&bl.color, "ink");
    let fill = if bl.shaded {
        let c = if bl.color.is_none() { "blue".to_string() } else { color.clone() };
        Some(format!("{c}-tint"))
    } else {
        None
    };
    let stroke = Stroke { color: color.clone(), width: if depth == 0 { 0.6 } else { 0.55 }, line: Line::Solid, double: false };
    b.path(ObjRef::Blob(bl.name.clone()), s, segs, true, Some(stroke), fill, (None, None));
    let r = rect_of(&poly);
    b.anchors.push((ObjRef::Blob(bl.name.clone()), centre, r.clone()));
    b.shapes.push(ShapeFrame { obj: ObjRef::Blob(bl.name.clone()), centre, radius: (l.a * l.a + l.bb * l.bb).sqrt(), half: (l.a, l.bb), angle: turn, inherited });
    b.targets.insert(bl.name.clone(), Target { obj: ObjRef::Blob(bl.name.clone()), at: centre, shape: Shape::Curve(poly.clone()) });
    // the blob's label
    if !bl.label.is_empty() {
        let (hw, hh) = label_half(&bl.label, b.size, true, 0.0);
        let lc = if turn != 0.0 {
            // a turned blob: the label sits off the turned outline in the
            // turned direction, pushed out until its (upright) box clears it
            let blob_names: HashSet<&str> = b.fig.items.iter().filter_map(|it| if let Item::Blob(x) = it { Some(x.name.as_str()) } else { None }).collect();
            let arc_in = b.fig.items.iter().any(|it| matches!(it, Item::Arrow(a) if a.to == bl.name && a.from != bl.name && blob_names.contains(a.from.as_str())));
            let (base, inward) = match bl.label_place {
                LabelPlace::Above => (if arc_in { 50.0 } else { 130.0 }, false),
                LabelPlace::Below => (270.0, false),
                LabelPlace::Left => (180.0, false),
                LabelPlace::Right => (0.0, false),
                LabelPlace::Inside => (90.0, true),
            };
            let u = dir(base + turn);
            let q = clip(&Target { obj: ObjRef::Blob(String::new()), at: centre, shape: Shape::Curve(poly.clone()) }, add(centre, u));
            let reach = hw * u.x.abs() + hh * u.y.abs();
            if inward { sub(q, mul(u, 0.12 + reach)) } else { add(q, mul(u, 0.08 + reach)) }
        } else {
            match bl.label_place {
            LabelPlace::Above => {
                // upper left, just outside the curve, as books do; upper right
                // when an arc from another blob lands at the upper left
                let blob_names: HashSet<&str> = b.fig.items.iter().filter_map(|it| if let Item::Blob(x) = it { Some(x.name.as_str()) } else { None }).collect();
                let arc_in = b.fig.items.iter().any(|it| matches!(it, Item::Arrow(a) if a.to == bl.name && a.from != bl.name && blob_names.contains(a.from.as_str())));
                let ang = if arc_in { 50.0 } else { 130.0 };
                let q = clip(&Target { obj: ObjRef::Blob(String::new()), at: centre, shape: Shape::Curve(poly.clone()) }, add(centre, dir(ang)));
                let sx = if arc_in { 1.0 } else { -1.0 };
                p(q.x + sx * hw * 0.7, q.y + hh + 0.06)
            }
            LabelPlace::Below => p(centre.x, r.min.y - hh - 0.1),
            LabelPlace::Left => p(r.min.x - hw - 0.12, centre.y),
            LabelPlace::Right => p(r.max.x + hw + 0.12, centre.y),
            LabelPlace::Inside => p(centre.x, r.max.y - hh - 0.12),
            }
        };
        let lcol = if depth > 0 && bl.color.is_none() && bl.shaded { "blue".to_string() } else { color.clone() };
        b.label(ObjRef::Blob(bl.name.clone()), lc, &bl.label, &lcol, b.size, true);
    }
    // children
    for (kn, off) in &l.kids {
        if let Some(k) = blobs.iter().find(|x| &x.name == kn) {
            emit_blob(b, k, add(centre, rot(*off, turn)), memo, blobs, depth + 1, turn);
        }
    }
    // elements
    for (e, at, left) in &l.elems {
        let at = match e.pos {
            Some(pos) => b.pos_to_pt(pos),
            None => add(centre, rot(*at, turn)),
        };
        b.draws.push(Draw::Dot { obj: ObjRef::Element(e.name.clone()), at, r: DOT_R, color: "ink".into() });
        let hw = label_half(&e.label, b.size, true, 0.0).0;
        let lc = if *left { p(at.x - DOT_R - 0.1 - hw, at.y) } else { p(at.x + DOT_R + 0.1 + hw, at.y) };
        let lr = b.label(ObjRef::Element(e.name.clone()), lc, &e.label, "ink", b.size, true);
        let mut r = lr.clone();
        r.min.x = r.min.x.min(at.x - DOT_R);
        r.max.x = r.max.x.max(at.x + DOT_R);
        b.anchors.push((ObjRef::Element(e.name.clone()), at, r));
        b.targets.insert(e.name.clone(), Target { obj: ObjRef::Element(e.name.clone()), at, shape: Shape::Dot(DOT_R + DOT_GAP) });
    }
    let _ = l.fixed;
}

// ---------------------------------------------------------------------------
// n-gons

/// Angle (degrees) of vertex position k (1-based) of an n-gon, numbered
/// counter-clockwise: k = 1 is the top vertex for odd n; for even n the
/// polygon sits on a flat bottom edge (as textbooks draw D4) and k = 1 is the
/// top-left vertex.
pub fn ngon_vertex_angle(n: u32, k: u32) -> f32 {
    ngon_base_angle(n) + (k as f32 - 1.0) * 360.0 / n.max(1) as f32
}

/// The angle of vertex 1: 90° for odd n, 90° + 180°/n for even n.
pub fn ngon_base_angle(n: u32) -> f32 {
    if n % 2 == 0 { 90.0 + 180.0 / n.max(1) as f32 } else { 90.0 }
}

/// An n-gon's radius when none is given (cm, before `scale`).
pub const NGON_RADIUS: f32 = 1.1;

/// The axis direction (degrees) for an encoded axis (2k = through vertex k,
/// 2k+1 = through the midpoint of edge k).
pub fn ngon_axis_angle(n: u32, code: u32) -> f32 {
    let k = code / 2;
    if code % 2 == 0 { ngon_vertex_angle(n, k) } else { ngon_vertex_angle(n, k) + 180.0 / n as f32 }
}

/// Which label sits at each vertex position after `turn` then `flip`.
pub fn ngon_labels_at(g: &NGon) -> Vec<String> {
    let n = g.n.max(1) as usize;
    let labels: Vec<String> = (0..n).map(|i| g.labels.get(i).cloned().unwrap_or_else(|| (i + 1).to_string())).collect();
    let at = ngon_positions(g);
    let mut out = vec![String::new(); n];
    for (li, &pi) in at.iter().enumerate() {
        out[pi] = labels[li].clone();
    }
    out
}

/// (FIG-MODEL) The index into `g.labels` of the label drawn at vertex
/// position `k` (1-based, as in `ObjRef::NGonVertex(_, k)`), after `turn`
/// and `flip`: what the window edits when she selects that vertex's label.
pub fn ngon_label_index(g: &NGon, k: u32) -> usize {
    let at = ngon_positions(g);
    let want = (k.max(1) as usize - 1) % at.len().max(1);
    at.iter().position(|&pi| pi == want).unwrap_or(want)
}

/// For each label index (0-based), the vertex position (0-based) it is
/// drawn at after `turn` then `flip`.
fn ngon_positions(g: &NGon) -> Vec<usize> {
    let n = g.n.max(1) as usize;
    let mut at: Vec<usize> = (0..n).collect();
    let t = g.turn as usize % n;
    for a in at.iter_mut() {
        *a = (*a + t) % n;
    }
    if let Some(code) = g.flip {
        let phi = ngon_axis_angle(g.n, code);
        for a in at.iter_mut() {
            let alpha = ngon_vertex_angle(g.n, *a as u32 + 1);
            let refl = 2.0 * phi - alpha;
            let k = ((refl - ngon_base_angle(g.n)) / (360.0 / n as f32)).round().rem_euclid(n as f32) as usize;
            *a = k % n;
        }
    }
    at
}


/// (figure-qol D) The rotation arrow's label when she hasn't typed one:
/// `r`, `r^{2}`; clockwise `r^{-1}`, `r^{-2}` (the inverse rotation).
pub fn rotation_default_label(k: u32, cw: bool) -> String {
    match (k, cw) {
        (1, false) => "r".to_string(),
        (k, false) => format!("r^{{{k}}}"),
        (k, true) => format!("r^{{-{k}}}"),
    }
}

/// (figure-qol D) The edge (1-based; edge k joins vertices k and k+1) the
/// rotation arrow sits on when `rotation edge=` isn't given: the one whose
/// midpoint, before `angle`, is nearest 50° among those in the upper right.
pub fn rotation_default_edge(n: u32) -> u32 {
    let n = n.max(3);
    (1..=n)
        .filter(|j| (ngon_vertex_angle(n, *j) + 180.0 / n as f32).rem_euclid(360.0) <= 90.0 + 1e-3)
        .min_by(|a, b| {
            let da = ((ngon_vertex_angle(n, *a) + 180.0 / n as f32).rem_euclid(360.0) - 50.0).abs();
            let db = ((ngon_vertex_angle(n, *b) + 180.0 / n as f32).rem_euclid(360.0) - 50.0).abs();
            da.total_cmp(&db)
        })
        .unwrap_or(1)
}

/// (figure-qol D) The direction (degrees, before `angle`) of the midpoint of
/// the edge the rotation arrow sits on.
pub fn rotation_edge_angle(n: u32, edge: Option<u32>) -> f32 {
    let n = n.max(3);
    let e = match edge {
        Some(e) => (e.max(1) - 1) % n + 1,
        None => rotation_default_edge(n),
    };
    ngon_vertex_angle(n, e) + 180.0 / n as f32
}

/// (figure-qol D) The edge whose midpoint is nearest direction `deg`
/// (degrees as drawn, `angle` included): where a dragged rotation arrow lands.
pub fn nearest_edge(n: u32, deg: f32, angle: f32) -> u32 {
    let n = n.max(3);
    (1..=n)
        .min_by(|a, b| {
            let d = |e: u32| {
                let x = (ngon_vertex_angle(n, e) + 180.0 / n as f32 + angle - deg).rem_euclid(360.0);
                x.min(360.0 - x)
            };
            d(*a).total_cmp(&d(*b))
        })
        .unwrap_or(1)
}

/// The rotation arrow's label when she hasn't named it: `r`, `r^{k}`, or
/// `r^{-k}` when it runs clockwise.
pub fn ngon_rotation_default(g: &NGon) -> String {
    let n = g.n.clamp(2, 64);
    let k = g.rotation % n;
    let k = if k == 0 { n } else { k };
    rotation_default_label(k, g.rotation_cw)
}

fn layout_ngons(b: &mut B) {
    let gons: Vec<NGon> = b.fig.items.iter().filter_map(|it| if let Item::NGon(x) = it { Some(x.clone()) } else { None }).collect();
    if gons.is_empty() {
        return;
    }
    let mut x = b.current_right().map(|r| r + 1.0);
    for g in &gons {
        let n = g.n.clamp(2, 64);
        let r = g.radius.unwrap_or(NGON_RADIUS) * b.scale;
        let has_axes = g.all_axes || !g.axes.is_empty();
        let ext = r + 0.45 + if has_axes { 0.3 } else { 0.0 };
        let c = match g.pos {
            Some(pos) => b.pos_to_pt(pos),
            None => {
                let cx = match x {
                    None => 0.0,
                    Some(x0) => x0 + ext,
                };
                p(cx, 0.0)
            }
        };
        x = Some(c.x + ext + 0.6);
        let name = &g.name;
        // `angle` turns the whole drawing (vertices, labels, axes, arc)
        let turn = if g.angle.is_finite() { g.angle } else { 0.0 };
        let va = |k: u32| ngon_vertex_angle(n, k) + turn;
        let verts: Vec<Pt> = (1..=n).map(|k| add(c, mul(dir(va(k)), r))).collect();
        // the polygon (tinted unless `plain`), then its axes over the tint
        let segs: Vec<Seg> = verts.iter().skip(1).map(|q| Seg::Line(*q)).collect();
        let stroke = palette(&g.color, "ink");
        let fill = g.shaded.then(|| format!("{}-tint", palette(&g.color, "blue")));
        b.path(ObjRef::NGon(name.clone()), verts[0], segs, true, Some(Stroke { color: stroke, width: 0.7, line: Line::Solid, double: false }), fill, (None, None));
        let mut codes: Vec<u32> = g.axes.clone();
        if g.all_axes {
            codes = (1..=n).map(|k| 2 * k).collect();
            if n % 2 == 0 {
                codes = (1..=n / 2).map(|k| 2 * k).chain((1..=n / 2).map(|k| 2 * k + 1)).collect();
            }
        }
        // axes reach a little past the polygon: just past a vertex (whose
        // label sits further out), further past an edge's midpoint
        let apothem = r * (std::f32::consts::PI / n as f32).cos();
        let axis_end = |through_vertex: bool| if through_vertex { r + 0.05 } else { apothem + 0.32 };
        for code in &codes {
            let phi = ngon_axis_angle(n, *code) + turn;
            let u = dir(phi);
            let near_v = code % 2 == 0;
            // the opposite end: for odd n an axis through a vertex leaves
            // through an edge's midpoint
            let far_v = if n % 2 == 1 { !near_v } else { near_v };
            let a0 = add(c, mul(u, axis_end(near_v)));
            let a1 = sub(c, mul(u, axis_end(far_v)));
            b.path(ObjRef::NGon(name.clone()), a0, vec![Seg::Line(a1)], false, Some(Stroke { color: "grey".into(), width: 0.45, line: Line::Dashed, double: false }), None, (None, None));
            // (figure-qol class C) the axis's label, past the end in its direction
            // (beyond a vertex's own label, or the edge's midpoint)
            if let Some((_, lab)) = g.axis_labels.iter().find(|(c, _)| c == code).filter(|(_, l)| !l.trim().is_empty()) {
                let (hw, hh) = label_half(lab, b.size, true, 0.0);
                let reach = if near_v { r + 0.55 } else { apothem + 0.44 };
                let lc = add(c, mul(u, reach + hw * u.x.abs() + hh * u.y.abs()));
                b.label(ObjRef::NGonAxis(name.clone(), *code), lc, lab, "grey", b.size, true);
            }
        }
        // vertices and labels
        let labels = ngon_labels_at(g);
        for k in 1..=n {
            let v = verts[k as usize - 1];
            b.draws.push(Draw::Dot { obj: ObjRef::NGonVertex(name.clone(), k), at: v, r: 1.5 * PT, color: "ink".into() });
            let lab = &labels[k as usize - 1];
            let mut ar = rect_at(v, 0.12, 0.12);
            // a hidden label (`labels=none`, or an empty entry) isn't drawn
            if g.show_labels && !lab.trim().is_empty() {
                let (hw, hh) = label_half(lab, b.size, true, 0.0);
                let u = dir(va(k));
                // push the label out along the radius until its box clears the vertex
                let off = 0.17 + (hw * u.x.abs() + hh * u.y.abs());
                let lc = add(v, mul(u, off));
                let lr = b.label(ObjRef::NGonVertex(name.clone(), k), lc, lab, "ink", b.size, true);
                ar.min.x = ar.min.x.min(lr.min.x);
                ar.min.y = ar.min.y.min(lr.min.y);
                ar.max.x = ar.max.x.max(lr.max.x);
                ar.max.y = ar.max.y.max(lr.max.y);
            }
            b.anchors.push((ObjRef::NGonVertex(name.clone(), k), v, ar));
            b.targets.insert(format!("{name}.{k}"), Target { obj: ObjRef::NGonVertex(name.clone(), k), at: v, shape: Shape::Dot(DOT_R + DOT_GAP) });
        }
        // the rotation arrow: an arc outside the polygon, upper right (or on
        // the edge she put it on), counter-clockwise or clockwise
        if g.rotation > 0 {
            let k = g.rotation % n;
            let k = if k == 0 { n } else { k };
            // a short arc just outside an edge (between two vertex labels)
            let span = (k as f32 * 360.0 / n as f32 * 0.55).clamp(36.0, 62.0);
            let ra = if has_axes { apothem + 0.32 + 0.16 } else { apothem + 0.34 };
            let mid = rotation_edge_angle(n, g.rotation_edge) + turn;
            let (a0, a1) = if g.rotation_cw { (mid + span / 2.0, mid - span / 2.0) } else { (mid - span / 2.0, mid + span / 2.0) };
            let (s0, segs) = arc_segs(c, ra, a0, a1);
            let rot = ObjRef::NGonRotation(name.clone());
            b.path(rot.clone(), s0, segs, false, Some(Stroke { color: "maroon".into(), width: 0.6, line: Line::Solid, double: false }), None, (None, Some(Head::To)));
            let lab = g.rotation_label.clone().filter(|l| !l.trim().is_empty()).unwrap_or_else(|| rotation_default_label(k, g.rotation_cw));
            let (hw, hh) = label_half(&lab, b.size, true, 0.0);
            let u = dir(mid);
            let lc = add(c, mul(u, ra + 0.12 + hw * u.x.abs() + hh * u.y.abs()));
            b.label(rot, lc, &lab, "maroon", b.size, true);
        }
        let rr = rect_at(c, r, r);
        b.anchors.push((ObjRef::NGon(name.clone()), c, rr));
        b.shapes.push(ShapeFrame { obj: ObjRef::NGon(name.clone()), centre: c, radius: r, half: (r, r), angle: turn, inherited: 0.0 });
        b.targets.insert(name.clone(), Target { obj: ObjRef::NGon(name.clone()), at: c, shape: Shape::Curve(verts.clone()) });
    }
}

// ---------------------------------------------------------------------------
// Cayley graphs

/// (figure-qol D) The look she gave generator `name` (None = the defaults).
pub fn gen_style_of<'a>(c: &'a Cayley, name: &str) -> Option<&'a GenStyle> {
    c.gen_styles.iter().find(|s| s.generator == name)
}

pub const GEN_COLORS: [&str; 6] = ["blue", "maroon", "teal", "green", "violet", "ochre"];

/// One edge of a Cayley graph: from, to (element indices), generator index,
/// directed?, bend (degrees).
#[derive(Clone, Debug, PartialEq)]
pub struct CayleyEdge {
    pub from: usize,
    pub to: usize,
    pub generator: usize,
    pub directed: bool,
    pub bend: f32,
}

/// Vertex positions (relative to the graph's centre, cm) and the edges.
pub fn cayley_geometry(c: &Cayley, g: &crate::cayley::Group) -> (Vec<Pt>, Vec<CayleyEdge>) {

    let n = g.elements.len();
    let order_of = |k: usize| -> usize {
        let t = &g.gens[k].1;
        let mut x = t[0];
        let mut m = 1;
        while x != 0 && m <= n {
            x = t[x];
            m += 1;
        }
        m
    };
    let ng = g.gens.len();
    let commute = |a: usize, b2: usize| -> bool { (0..n).all(|x| g.gens[b2].1[g.gens[a].1[x]] == g.gens[a].1[g.gens[b2].1[x]]) };
    let kind = crate::cayley::kind(&c.group);
    let r_scale = c.radius.unwrap_or(0.0);
    let mut layout = c.layout;
    if layout == CayleyLayout::Auto {
        layout = if ng >= 1 && order_of(0) == n {
            CayleyLayout::Circle
        } else if ng >= 2 && (n == 4 || n == 8) && (0..ng).all(|k| order_of(k) == 2) && (0..ng).all(|a| (0..ng).all(|b2| commute(a, b2))) {
            CayleyLayout::Grid
        } else if ng >= 2 && order_of(0) * 2 == n {
            CayleyLayout::Prism
        } else if ng >= 2 && order_of(1) * 2 == n {
            CayleyLayout::Prism
        } else if ng == 2 && commute(0, 1) && order_of(0) * order_of(1) == n {
            CayleyLayout::Grid
        } else {
            CayleyLayout::Circle
        };
        let _ = kind;
        // "a circle otherwise", refined: when the first generator has small
        // cycles, each cycle is its own little polygon and the polygons sit on
        // the circle (A4: four triangles, S4: six squares), so the first
        // generator's edges are short sides instead of long wrap-arounds
        if layout == CayleyLayout::Circle && ng >= 2 && order_of(0) >= 3 && order_of(0) < n && r_scale == 0.0 {
            if let Some(t) = truncated_tetrahedron(g) {
                return t;
            }
            return rings_geometry(g, order_of(0));
        }
    }
    // (FIG-POLISH) Q8-like: two non-commuting generators of order 4 in a
    // group of 8 (a² = b²). The graph is K4,4 (crossing number 4), drawn with
    // exactly 4 crossings: the a-cycle through e outside, the other a-coset
    // inside, each inner vertex x·b on the radius of x·a, so b's edges are
    // straight and no edge passes through the centre.
    if c.layout == CayleyLayout::Auto && n == 8 && ng == 2 && order_of(0) == 4 && order_of(1) == 4 && !commute(0, 1) {
        let (ta, tb) = (&g.gens[0].1, &g.gens[1].1);
        let outer = if r_scale > 0.0 { r_scale } else { 2.0 };
        let inner = outer * 0.42;
        let mut pos = vec![p(0.0, 0.0); n];
        let mut x = 0usize;
        let mut ok = vec![false; n];
        for k in 0..4 {
            pos[x] = mul(dir(90.0 + 90.0 * k as f32), outer);
            ok[x] = true;
            let y = tb[x];
            pos[y] = mul(dir(90.0 + 90.0 * (k + 1) as f32), inner);
            ok[y] = true;
            x = ta[x];
        }
        if ok.iter().all(|&v| v) {
            let edges = cayley_edges(g, &pos);
            return (pos, edges);
        }
    }
    let mut pos = vec![p(0.0, 0.0); n];
    let mut ring_gen = 0usize;
    match layout {
        CayleyLayout::Prism if ng >= 1 => {
            // the ring generator: the first one of order n/2
            ring_gen = (0..ng).find(|&k| order_of(k) * 2 == n).unwrap_or(0);
            let other = (0..ng).find(|&k| k != ring_gen).unwrap_or(ring_gen);
            let m = n / 2;
            let outer = if r_scale > 0.0 { r_scale } else { (m as f32 * 1.25 / std::f32::consts::TAU).max(1.35) };
            let inner = (outer * 0.52).max(m as f32 * 0.8 / std::f32::consts::TAU);
            let outer = outer.max(inner + 1.0);
            let t = &g.gens[ring_gen].1;
            let mut x = 0;
            let mut placed = vec![false; n];
            for k in 0..m {
                pos[x] = mul(dir(90.0 + 360.0 * k as f32 / m as f32), outer);
                placed[x] = true;
                let y = g.gens[other].1[x];
                if !placed[y] {
                    pos[y] = mul(dir(90.0 + 360.0 * k as f32 / m as f32), inner);
                    placed[y] = true;
                }
                x = t[x];
            }
            // anything left (if `other` didn't land in the second ring): circle
            let rest: Vec<usize> = (0..n).filter(|&i| !placed[i]).collect();
            for (j, &i) in rest.iter().enumerate() {
                pos[i] = mul(dir(90.0 + 360.0 * j as f32 / rest.len() as f32), outer + 1.0);
            }
        }
        CayleyLayout::Grid if ng >= 2 => {
            let d = if r_scale > 0.0 { r_scale } else { 1.75 };
            // coordinates by walking the generators from e
            let mut coord: Vec<Option<Vec<i32>>> = vec![None; n];
            coord[0] = Some(vec![0; ng]);
            let mut q = VecDeque::from([0usize]);
            while let Some(v) = q.pop_front() {
                for k in 0..ng {
                    let w = g.gens[k].1[v];
                    if coord[w].is_none() {
                        let mut cc = coord[v].clone().unwrap();
                        cc[k] += 1;
                        coord[w] = Some(cc);
                        q.push_back(w);
                    }
                }
            }
            let orders: Vec<i32> = (0..ng).map(|k| order_of(k) as i32).collect();
            for i in 0..n {
                let cc = coord[i].clone().unwrap_or(vec![0; ng]);
                let cc: Vec<i32> = cc.iter().zip(&orders).map(|(a, m)| a.rem_euclid(*m)).collect();
                let (mut xx, mut yy) = (cc[0] as f32 * d, cc[1] as f32 * d);
                if ng >= 3 {
                    // a cube: the third direction drawn obliquely
                    xx += cc[2] as f32 * d * 0.48;
                    yy += cc[2] as f32 * d * 0.36;
                }
                pos[i] = p(xx, yy);
            }
            // centre it
            let cx = pos.iter().map(|q| q.x).sum::<f32>() / n as f32;
            let cy = pos.iter().map(|q| q.y).sum::<f32>() / n as f32;
            for q in pos.iter_mut() {
                *q = p(q.x - cx, q.y - cy);
            }
        }
        _ => {
            // a circle, walking the first generator's cycles in turn so its
            // edges join neighbours
            let r = if r_scale > 0.0 { r_scale } else { (n as f32 * 0.95 / std::f32::consts::TAU).max(1.1) };
            let mut order: Vec<usize> = vec![];
            let mut seen = vec![false; n];
            for s in 0..n {
                if seen[s] {
                    continue;
                }
                let mut x = s;
                while !seen[x] {
                    seen[x] = true;
                    order.push(x);
                    x = if ng > 0 { g.gens[0].1[x] } else { x };
                }
            }
            for (j, &i) in order.iter().enumerate() {
                pos[i] = mul(dir(90.0 + 360.0 * j as f32 / n as f32), r);
            }
        }
    }
    // a forced layout that doesn't fit the group (a grid for a non-abelian
    // group puts two elements on one point): fall back to the circle
    let clash = (0..n).any(|i| (i + 1..n).any(|j| len(sub(pos[i], pos[j])) < 1e-3));
    if clash && layout != CayleyLayout::Circle {
        let c2 = Cayley { layout: CayleyLayout::Circle, ..c.clone() };
        return cayley_geometry(&c2, g);
    }
    let _ = ring_gen;
    let edges = cayley_edges(g, &pos);
    (pos, edges)
}

/// (FIG-POLISH) A4 = <a, b> with a of order 3 and b an involution: the
/// graph is the truncated tetrahedron (four a-triangles, one b-edge between
/// each pair). Drawn planar, with no crossings: e's triangle in the middle,
/// the other three around it, each turned so its b-edges point at the
/// triangles they join.
fn truncated_tetrahedron(g: &crate::cayley::Group) -> Option<(Vec<Pt>, Vec<CayleyEdge>)> {
    let n = g.elements.len();
    if n != 12 || g.gens.len() != 2 {
        return None;
    }
    let (ta, tb) = (&g.gens[0].1, &g.gens[1].1);
    if (0..n).any(|x| ta[ta[ta[x]]] != x || ta[x] == x || tb[tb[x]] != x || tb[x] == x) {
        return None;
    }
    // the a-triangles, e's first
    let mut tri = vec![usize::MAX; n];
    let mut tris: Vec<[usize; 3]> = vec![];
    for x in 0..n {
        if tri[x] == usize::MAX {
            let t = [x, ta[x], ta[ta[x]]];
            for &y in &t {
                tri[y] = tris.len();
            }
            tris.push(t);
        }
    }
    if tris.len() != 4 {
        return None;
    }
    // exactly one b-edge between each pair of triangles
    let mut pairs = HashSet::new();
    for x in 0..n {
        let (s, t) = (tri[x], tri[tb[x]]);
        if s == t {
            return None;
        }
        pairs.insert((s.min(t), s.max(t), x.min(tb[x])));
    }
    if pairs.len() != 6 || pairs.iter().map(|p| (p.0, p.1)).collect::<HashSet<_>>().len() != 6 {
        return None;
    }
    let (big, small) = (2.3f32, 0.62f32);
    let mut pos = vec![p(0.0, 0.0); n];
    let mut angle_of = vec![0.0f32; 4];
    // the middle triangle: its vertices at 90°, 210°, 330° in a's order
    for (j, &v) in tris[0].iter().enumerate() {
        let phi = 90.0 + 120.0 * j as f32;
        pos[v] = mul(dir(phi), small);
        angle_of[tri[tb[v]]] = phi;
    }
    for (t, cyc) in tris.iter().enumerate().skip(1) {
        let phi = angle_of[t];
        let c = mul(dir(phi), big);
        for &v in cyc {
            let other = tri[tb[v]];
            let d = if other == 0 {
                phi + 180.0
            } else if (angle_of[other] - phi - 120.0).rem_euclid(360.0) < 1.0 {
                phi + 60.0
            } else {
                phi - 60.0
            };
            pos[v] = add(c, mul(dir(d), small));
        }
    }
    Some((pos.clone(), cayley_edges(g, &pos)))
}

/// Cycles of the first generator as small m-gons around a big circle, in an
/// order (and with turns) that keeps the other generators' edges short.
fn rings_geometry(g: &crate::cayley::Group, m: usize) -> (Vec<Pt>, Vec<CayleyEdge>) {
    let n = g.elements.len();
    let t0 = &g.gens[0].1;
    // the cycles, from e on
    let mut coset = vec![usize::MAX; n];
    let mut cycles: Vec<Vec<usize>> = vec![];
    for s0 in 0..n {
        if coset[s0] != usize::MAX {
            continue;
        }
        let mut cyc = vec![];
        let mut x = s0;
        while coset[x] == usize::MAX {
            coset[x] = cycles.len();
            cyc.push(x);
            x = t0[x];
        }
        cycles.push(cyc);
    }
    let k = cycles.len();
    // links between cycles through the other generators
    let mut link = vec![vec![0usize; k]; k];
    for (_, t) in g.gens.iter().skip(1) {
        for x in 0..n {
            let (a, b2) = (coset[x], coset[t[x]]);
            if a != b2 {
                link[a][b2] += 1;
                link[b2][a] += 1;
            }
        }
    }
    // order around the circle: greedily the most-linked next
    let mut order = vec![0usize];
    let mut used = vec![false; k];
    used[0] = true;
    while order.len() < k {
        let last = *order.last().unwrap();
        let next = (0..k).filter(|&c| !used[c]).max_by_key(|&c| (link[last][c], usize::MAX - c)).unwrap();
        used[next] = true;
        order.push(next);
    }
    let rs = (m as f32 * 1.3 / std::f32::consts::TAU).max(0.9);
    let rb = ((2.0 * rs + 1.3) / (2.0 * (std::f32::consts::PI / k as f32).sin())).max(rs * 2.5);
    let mut pos = vec![p(0.0, 0.0); n];
    let mut placed = vec![false; n];
    for (slot, &c) in order.iter().enumerate() {
        let theta = 90.0 + 360.0 * slot as f32 / k as f32;
        let centre = mul(dir(theta), rb);
        let cyc = &cycles[c];
        // try every turn of the little polygon: shortest links to what's placed
        let mut best = (f32::MAX, 0usize);
        for rot in 0..m {
            let mut cost = 0.0;
            for (i, &x) in cyc.iter().enumerate() {
                let q = add(centre, mul(dir(theta + 180.0 + 360.0 * ((i + rot) % m) as f32 / m as f32), rs));
                for (_, t) in g.gens.iter().skip(1) {
                    for y in [t[x], (0..n).find(|&z| t[z] == x).unwrap_or(x)] {
                        if placed[y] {
                            cost += len(sub(q, pos[y]));
                        } else if coset[y] != c {
                            // unplaced: pull toward the outside of the ring
                            cost += len(sub(q, mul(dir(theta), rb * 1.4))) * 0.2;
                        }
                    }
                }
            }
            if cost < best.0 - 1e-4 {
                best = (cost, rot);
            }
        }
        for (i, &x) in cyc.iter().enumerate() {
            pos[x] = add(centre, mul(dir(theta + 180.0 + 360.0 * ((i + best.1) % m) as f32 / m as f32), rs));
            placed[x] = true;
        }
    }
    (pos.clone(), cayley_edges(g, &pos))
}

fn cayley_edges(g: &crate::cayley::Group, pos: &[Pt]) -> Vec<CayleyEdge> {
    let n = g.elements.len();
    let mut edges: Vec<CayleyEdge> = vec![];
    let mut pairs: HashMap<(usize, usize), usize> = HashMap::new();
    for (k, (_, t)) in g.gens.iter().enumerate() {
        let invol = (0..n).all(|x| t[t[x]] == x);
        for x in 0..n {
            let y = t[x];
            if y == x || (invol && y < x) {
                continue;
            }
            let count = pairs.entry((x.min(y), x.max(y))).or_insert(0);
            let mut bend = match *count {
                0 => 0.0,
                1 => 22.0,
                _ => -22.0,
            };
            *count += 1;
            let (pa, pb) = (pos[x], pos[y]);
            let through = (0..n).any(|z| z != x && z != y && dist_to_seg(pos[z], pa, pb) < 0.25);
            if bend == 0.0 && through {
                bend = 32.0;
            }
            edges.push(CayleyEdge { from: x, to: y, generator: k, directed: !invol, bend });
        }
    }
    edges
}

/// (FIG-MODEL) The `radius=` that puts a Cayley graph's farthest vertex
/// `outer` cm from its centre (what the window's resize handle sets), or
/// None when the group can't be drawn. With `radius=` set the graph is not
/// spread to fit its labels, and the auto layout's ring special cases
/// (A4, S4) give way to the plain circle, as when she types it.
pub fn cayley_radius_for(c: &Cayley, outer: f32) -> Option<f32> {
    let grp = crate::cayley::group(&c.group, &c.gens).ok()?;
    if !(outer.is_finite() && outer > 0.0) {
        return None;
    }
    let reach = |r: f32| -> f32 {
        let c2 = Cayley { radius: Some(r), ..c.clone() };
        cayley_geometry(&c2, &grp).0.iter().map(|q| len(*q)).fold(0.0, f32::max)
    };
    let (mut lo, mut hi) = (0.01f32, 200.0f32);
    if reach(lo) >= outer {
        return Some(lo);
    }
    if reach(hi) <= outer {
        return Some(hi);
    }
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        if reach(mid) < outer { lo = mid } else { hi = mid }
    }
    Some((hi * 100.0).round() / 100.0)
}

fn layout_cayleys(b: &mut B) {
    let cs: Vec<Cayley> = b.fig.items.iter().filter_map(|it| if let Item::Cayley(x) = it { Some(x.clone()) } else { None }).collect();
    for c in &cs {
        let grp = match crate::cayley::group(&c.group, &c.gens) {
            Ok(g) => g,
            // a group that can't be made (unknown, too big, infinite) is a
            // diagnostic (build::figure_diagnostics); nothing goes in the figure
            Err(_) => continue,
        };
        let (mut rel, edges) = cayley_geometry(c, &grp);
        // `angle` turns the layout about its centre (labels stay upright)
        let turn = if c.angle.is_finite() { c.angle } else { 0.0 };
        if turn != 0.0 {
            for q in rel.iter_mut() {
                *q = rot(*q, turn);
            }
        }
        let n = grp.elements.len();
        let vsize = if n > 24 { b.size * 0.8 } else if n > 12 { b.size * 0.9 } else { b.size };
        // spread the whole graph until no two vertex labels touch
        let half: Vec<(f32, f32)> = grp.elements.iter().map(|w| if c.labels {
            let shown = c.vertex_names.iter().find(|(x, _)| x == w).map(|(_, l)| l.as_str()).unwrap_or(w.as_str());
            label_half(shown, vsize, true, VERTEX_SEP)
        } else { (0.12, 0.12) }).collect();
        if c.radius.is_none() {
            let gap = 0.1;
            let mut k = 1.0f32;
            for i in 0..n {
                for j in i + 1..n {
                    let (dx, dy) = ((rel[i].x - rel[j].x).abs(), (rel[i].y - rel[j].y).abs());
                    let need_x = if dx > 1e-4 { (half[i].0 + half[j].0 + gap) / dx } else { f32::MAX };
                    let need_y = if dy > 1e-4 { (half[i].1 + half[j].1 + gap) / dy } else { f32::MAX };
                    let need = need_x.min(need_y);
                    if need.is_finite() {
                        k = k.max(need);
                    }
                }
            }
            if k > 1.0 && k.is_finite() {
                for q in rel.iter_mut() {
                    *q = mul(*q, k);
                }
            }
        }
        let minx = rel.iter().map(|q| q.x).fold(f32::MAX, f32::min);
        let maxx = rel.iter().map(|q| q.x).fold(f32::MIN, f32::max);
        let centre = match c.pos {
            Some(pos) => b.pos_to_pt(pos),
            None => match b.current_right() {
                None => p(0.0, 0.0),
                Some(r) => p(r + 1.2 - minx + 0.4, 0.0),
            },
        };
        let _ = maxx;
        let mut targets: Vec<Target> = vec![];
        for (i, w) in grp.elements.iter().enumerate() {
            let at = add(centre, rel[i]);
            let obj = ObjRef::CayleyVertex(c.name.clone(), w.clone());
            if c.labels {
                // (figure-qol class C) her name for this vertex, else its word
                let shown = c.vertex_names.iter().find(|(x, _)| x == w).map(|(_, l)| l.clone()).filter(|l| !l.trim().is_empty()).unwrap_or_else(|| w.clone());
                let (hw, hh) = label_half(&shown, vsize, true, VERTEX_SEP);
                let rr = rect_at(at, hw, hh);
                b.draws.push(Draw::Label { obj: obj.clone(), at, latex: shown, color: "ink".into(), size: vsize, math: true });
                b.grow(&rr);
                b.anchors.push((obj.clone(), at, rr.clone()));
                targets.push(Target { obj: obj.clone(), at, shape: Shape::Box(rect_at(p(0.0, 0.0), hw, hh)) });
            } else {
                b.draws.push(Draw::Dot { obj: obj.clone(), at, r: VERTEX_DOT_R, color: "ink".into() });
                let rr = rect_at(at, 0.12, 0.12);
                b.grow(&rr);
                b.anchors.push((obj.clone(), at, rr));
                targets.push(Target { obj: obj.clone(), at, shape: Shape::Dot(VERTEX_DOT_R + DOT_GAP) });
            }
            b.targets.insert(format!("{}.{}", c.name, w), targets[i].clone());
        }
        for (ei, e) in edges.iter().enumerate() {
            // (figure-qol D) per generator: hidden, a colour of its own, g⁻¹
            let st = gen_style_of(c, &grp.gens[e.generator].0);
            if st.is_some_and(|x| x.hidden) {
                continue;
            }
            let color = st.and_then(|x| x.color.as_deref()).unwrap_or(GEN_COLORS[e.generator % GEN_COLORS.len()]);
            let inv = e.directed && st.is_some_and(|x| x.inverse);
            let (from, to, bend) = if inv { (e.to, e.from, -e.bend) } else { (e.from, e.to, e.bend) };
            let (start, segs) = route(&targets[from], &targets[to], bend, false);
            // edges go under the labels: put them before the vertex draws
            let d = Draw::Path {
                obj: ObjRef::CayleyEdge(c.name.clone(), ei),
                start,
                segs: segs.clone(),
                closed: false,
                stroke: Some(Stroke { color: color.into(), width: 0.6, line: Line::Solid, double: false }),
                fill: None,
                fill_opacity: 1.0,
                start_tip: None,
                end_tip: if e.directed { Some(Head::To) } else { None },
            };
            b.grow(&rect_of(&flatten(start, &segs)));
            b.draws.push(d);
        }
        let mut gr: Option<Rect> = None;
        for t in &targets {
            rect_union(&mut gr, &rect_at(t.at, 0.2, 0.2));
        }
        let gr = gr.unwrap_or_default();
        b.anchors.push((ObjRef::Cayley(c.name.clone()), centre, gr.clone()));
        let radius = rel.iter().map(|q| len(*q)).fold(0.0, f32::max);
        let unturned: Vec<Pt> = rel.iter().map(|q| rot(*q, -turn)).collect();
        let half = (unturned.iter().map(|q| q.x.abs()).fold(0.0, f32::max), unturned.iter().map(|q| q.y.abs()).fold(0.0, f32::max));
        b.shapes.push(ShapeFrame { obj: ObjRef::Cayley(c.name.clone()), centre, radius, half, angle: turn, inherited: 0.0 });
        // the legend, under the graph: a short coloured stroke + the generator
        if c.legend {
            let mut x = gr.min.x;
            let y = gr.min.y - 0.55;
            for (k, (gname, t)) in grp.gens.iter().enumerate() {
                let invol = (0..n).all(|i| t[t[i]] == i);
                let st = gen_style_of(c, gname);
                if st.is_some_and(|x| x.hidden) {
                    continue;
                }
                let color = st.and_then(|x| x.color.as_deref()).unwrap_or(GEN_COLORS[k % GEN_COLORS.len()]);
                b.path(ObjRef::Cayley(c.name.clone()), p(x, y), vec![Seg::Line(p(x + 0.7, y))], false, Some(Stroke { color: color.into(), width: 0.6, line: Line::Solid, double: false }), None, (None, if invol { None } else { Some(Head::To) }));
                let lab = if !invol && st.is_some_and(|x| x.inverse) { format!("{{{gname}}}^{{-1}}") } else { gname.clone() };
                let (hw, _) = label_half(&lab, b.size, true, 0.0);
                b.label(ObjRef::Cayley(c.name.clone()), p(x + 0.85 + hw, y), &lab, color, b.size, true);
                x += 0.85 + hw * 2.0 + 0.55;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// arrows

/// Route a curve from `a` to `b`: straight when bend = 0, a TikZ-style bend
/// (out = direction − bend … looseness 1), or a loop. Endpoints are clipped
/// to the objects exactly as TikZ clips a path toward its control points.
fn route(a: &Target, bt: &Target, bend: f32, is_loop: bool) -> (Pt, Vec<Seg>) {
    if is_loop || (a.obj == bt.obj && len(sub(a.at, bt.at)) < 1e-4) {
        // a loop above the object
        let (o, i) = (dir(118.0), dir(62.0));
        let s0 = clip(a, add(a.at, o));
        let e0 = clip(a, add(a.at, i));
        let k = 0.85;
        let c1 = add(s0, mul(o, k));
        let c2 = add(e0, mul(i, k));
        let s = clip(a, c1);
        let e = clip(a, c2);
        return (s, vec![Seg::Cubic(c1, c2, e)]);
    }
    let d = sub(bt.at, a.at);
    if bend.abs() < 0.01 {
        let s = clip(a, bt.at);
        let e = clip(bt, a.at);
        return (s, vec![Seg::Line(e)]);
    }
    let alpha = angle(d);
    let (uo, ui) = (dir(alpha + bend), dir(alpha + 180.0 - bend));
    let s0 = clip(a, add(a.at, uo));
    let e0 = clip(bt, add(bt.at, ui));
    let l = 0.3915 * len(sub(e0, s0));
    let c1 = add(s0, mul(uo, l));
    let c2 = add(e0, mul(ui, l));
    let s = clip(a, c1);
    let e = clip(bt, c2);
    (s, vec![Seg::Cubic(c1, c2, e)])
}

/// The label's parameter along an arrow (tikz-cd: near start/end = 0.25/0.75).
pub fn label_t(lp: LabelPos) -> f32 {
    match lp {
        LabelPos::Middle => 0.5,
        LabelPos::NearStart => 0.25,
        LabelPos::NearEnd => 0.75,
    }
}

/// Where TikZ's `auto` puts a label of half-size (hw, hh) at point q with
/// tangent d, on the left (or right when `swap`): anchored by the opposite
/// compass point of its box.
pub fn auto_label_centre(q: Pt, d: Pt, hw: f32, hh: f32, swap: bool) -> Pt {
    let d = norm(d);
    let nrm = if swap { p(d.y, -d.x) } else { p(-d.y, d.x) };
    let eps = 0.05 * PT * 20.0; // TikZ treats tiny components as zero
    let sx = if nrm.x > eps / 10.0 && d.y.abs() > 0.003 { 1.0 } else if nrm.x < -eps / 10.0 && d.y.abs() > 0.003 { -1.0 } else { 0.0 };
    let sy = if nrm.y > 0.0 && d.x.abs() > 0.003 { 1.0 } else if nrm.y < 0.0 && d.x.abs() > 0.003 { -1.0 } else { 0.0 };
    p(q.x + sx * hw, q.y + sy * hh)
}

fn head_tips(h: Head) -> (Option<Head>, Option<Head>) {
    match h {
        Head::To | Head::Double => (None, Some(h)),
        Head::TwoHeads => (None, Some(Head::TwoHeads)),
        Head::Tail => (Some(Head::Tail), Some(Head::To)),
        Head::Hook => (Some(Head::Hook), Some(Head::To)),
        Head::MapsTo => (Some(Head::MapsTo), Some(Head::To)),
        Head::Both => (Some(Head::To), Some(Head::To)),
        Head::Equal => (None, None),
    }
}

/// (FIG-POLISH) Straight arrows that end on the same dot (an element, a
/// vertex) would stack their heads into what looks like a surjection's double
/// head. Give each its own arrival angle around the dot, at least 44° apart,
/// keeping their order and centred on where they come from.
fn arrival_angles(b: &B, fig: &Figure) -> HashMap<usize, f32> {
    let mut by_end: BTreeMap<String, Vec<(usize, f32)>> = BTreeMap::new();
    for (i, it) in fig.items.iter().enumerate() {
        let Item::Arrow(a) = it else { continue };
        if a.is_loop || a.bend != 0.0 || a.from == a.to {
            continue;
        }
        let (Some(ta), Some(tb)) = (b.targets.get(&a.from), b.targets.get(&a.to)) else { continue };
        if !matches!(tb.shape, Shape::Dot(_)) {
            continue;
        }
        let d = sub(ta.at, tb.at);
        if len(d) < 1e-4 {
            continue;
        }
        by_end.entry(a.to.clone()).or_default().push((i, angle(d)));
    }
    let mut out = HashMap::new();
    const GAP: f32 = 44.0;
    for (_, mut v) in by_end {
        if v.len() < 2 {
            continue;
        }
        // unwrap the angles around their mean direction
        let (sx, sy) = v.iter().fold((0.0f32, 0.0f32), |(x, y), (_, a)| (x + a.to_radians().cos(), y + a.to_radians().sin()));
        let mean = sy.atan2(sx).to_degrees();
        for (_, a) in v.iter_mut() {
            *a = mean + (*a - mean + 540.0).rem_euclid(360.0) - 180.0;
        }
        v.sort_by(|x, y| x.1.total_cmp(&y.1));
        let mut ang: Vec<f32> = v.iter().map(|x| x.1).collect();
        for k in 1..ang.len() {
            if ang[k] - ang[k - 1] < GAP {
                ang[k] = ang[k - 1] + GAP;
            }
        }
        // re-centre on the original spread
        let shift = (v.iter().map(|x| x.1).sum::<f32>() - ang.iter().sum::<f32>()) / ang.len() as f32;
        for (k, (i, _)) in v.iter().enumerate() {
            out.insert(*i, ang[k] + shift);
        }
    }
    out
}

/// (FIG-POLISH) Straight arrows between the same two objects (`A -> B` and
/// `B -> A`) would lie on one line and read as one two-headed arrow. Draw
/// them side by side, as tikz-cd's `shift left` does: a signed offset (cm)
/// to the left of the direction from the alphabetically first end.
fn parallel_shifts(fig: &Figure) -> HashMap<usize, f32> {
    let mut pairs: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
    for (i, it) in fig.items.iter().enumerate() {
        if let Item::Arrow(a) = it {
            if a.is_loop || a.from == a.to || a.bend != 0.0 {
                continue;
            }
            let key = if a.from <= a.to { (a.from.clone(), a.to.clone()) } else { (a.to.clone(), a.from.clone()) };
            pairs.entry(key).or_default().push(i);
        }
    }
    const GAP: f32 = 0.22;
    let mut out = HashMap::new();
    for (_, v) in pairs {
        if v.len() < 2 {
            continue;
        }
        let m = v.len() as f32;
        for (k, i) in v.into_iter().enumerate() {
            out.insert(i, ((m - 1.0) / 2.0 - k as f32) * GAP);
        }
    }
    out
}

fn route_arrow(b: &mut B, idx: usize, a: &Arrow, arrive: Option<f32>, shift: f32) {
    let (Some(ta), Some(tb)) = (b.targets.get(&a.from).cloned(), b.targets.get(&a.to).cloned()) else {
        return;
    };
    let blobish = |t: &Target| matches!(t.obj, ObjRef::Blob(_) | ObjRef::NGon(_));
    let (start, segs) = if blobish(&ta) && blobish(&tb) && ta.obj != tb.obj && !a.is_loop {
        // blob to blob: a big arc over the top, from upper right to upper left
        let (l, r) = (ta.at, tb.at);
        let rightwards = r.x >= l.x;
        let (oa, ob) = if rightwards { (dir(70.0), dir(110.0)) } else { (dir(110.0), dir(70.0)) };
        let s = clip(&ta, add(ta.at, oa));
        let e = clip(&tb, add(tb.at, ob));
        let lift = 0.22 * len(sub(e, s)) + 0.2 + a.bend.abs() * 0.01;
        let s2 = add(s, p(0.0, 0.08));
        let e2 = add(e, p(0.0, 0.08));
        let c1 = add(s2, p((e2.x - s2.x) * 0.18, lift));
        let c2 = add(e2, p((s2.x - e2.x) * 0.18, lift));
        (s2, vec![Seg::Cubic(c1, c2, e2)])
    } else if shift != 0.0 && len(sub(tb.at, ta.at)) > 1e-4 {
        // side by side with another arrow between the same two ends
        let (p0, p1) = if a.from <= a.to { (ta.at, tb.at) } else { (tb.at, ta.at) };
        let u = norm(sub(p1, p0));
        let nrm = mul(p(-u.y, u.x), shift);
        let (mut ta2, mut tb2) = (ta.clone(), tb.clone());
        ta2.at = add(ta.at, nrm);
        tb2.at = add(tb.at, nrm);
        if let Shape::Curve(pts) = &mut ta2.shape {
            pts.iter_mut().for_each(|q| *q = add(*q, nrm));
        }
        if let Shape::Curve(pts) = &mut tb2.shape {
            pts.iter_mut().for_each(|q| *q = add(*q, nrm));
        }
        route(&ta2, &tb2, 0.0, false)
    } else if let (Some(beta), Shape::Dot(r)) = (arrive, &tb.shape) {
        // a touch further out than a lone arrow, so the heads stand apart
        let e = add(tb.at, mul(dir(beta), *r + 0.04));
        (clip(&ta, e), vec![Seg::Line(e)])
    } else {
        route(&ta, &tb, a.bend, a.is_loop)
    };
    let color = palette(&a.color, "ink");
    let width = if a.thick { 0.8 } else { 0.4 };
    let double = matches!(a.head, Head::Equal | Head::Double);
    let tips = head_tips(a.head);
    b.path(ObjRef::Arrow(idx), start, segs.clone(), false, Some(Stroke { color: color.clone(), width, line: a.line, double }), None, tips);
    // labels
    let t = label_t(a.label_pos);
    let (q, d) = path_at(start, &segs, t);
    // an arc over blobs keeps its label above, whichever way it runs
    let mut swap = a.side == LabelSide::Swap;
    if blobish(&ta) && blobish(&tb) && tb.at.x < ta.at.x {
        swap = !swap;
    }
    let lsize = b.size * 0.72;
    let put = |latex: &str, swap: bool, b: &mut B| {
        let (hw, hh) = label_half(latex, lsize, true, LABEL_SEP);
        let c = auto_label_centre(q, d, hw, hh, swap);
        b.label(ObjRef::Arrow(idx), c, latex, &color, lsize, true);
    };
    match (&a.label, a.iso) {
        (Some(l), false) if !l.is_empty() => put(l, swap, b),
        (Some(l), true) if !l.is_empty() => {
            put(l, swap, b);
            put("\\cong", !swap, b);
        }
        (_, true) => put("\\cong", swap, b),
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// tidy

/// Deterministic tidy: snap nodes to cells, straighten near-axis arrows,
/// even out blob spacing; returns the edits it made.
pub fn tidy(fig: &mut Figure) -> Vec<Edit> {
    let mut edits = vec![];
    let sp = if fig.globals.spacing.0 > 0.0 { fig.globals.spacing } else { DEFAULT_SPACING };
    // 1. nodes onto integer cells (absolute positions too)
    let mut used: HashSet<(i32, i32)> = HashSet::new();
    for it in fig.items.iter_mut() {
        if let Item::Node(n) = it {
            let Some(pos) = n.pos else { continue };
            let (c, r) = match pos {
                Pos::Cell(c, r) => (c, r),
                Pos::Abs(x, y) => (x / sp.0, -y / sp.1),
            };
            let mut cell = (c.round() as i32, r.round() as i32);
            let mut k = 0;
            while used.contains(&cell) && k < 8 {
                cell.0 += 1;
                k += 1;
            }
            used.insert(cell);
            let new = Pos::Cell(cell.0 as f32, cell.1 as f32);
            if new != pos {
                n.pos = Some(new);
                edits.push(Edit::MoveNode { name: n.name.clone(), pos: new });
            }
        }
    }
    // 2. arrows with a slight bend are straightened (the edit carries the
    // label as she spelled it, so replaying it keeps her spelling)
    let spelled = fig.clone();
    for (i, it) in fig.items.iter_mut().enumerate() {
        if let Item::Arrow(a) = it {
            if a.bend != 0.0 && a.bend.abs() < 12.0 && !a.is_loop {
                a.bend = 0.0;
                let mut e = a.clone();
                e.label = e.label.map(|l| crate::spelling::raw(&spelled, &crate::spelling::arrow_key(&a.from, &a.to), &l));
                edits.push(Edit::SetArrow { index: i, arrow: e });
            }
        }
    }
    // 3. placed top-level blobs: same row, even gaps (in their order)
    let names: HashSet<String> = fig.items.iter().filter_map(|it| if let Item::Blob(b) = it { Some(b.name.clone()) } else { None }).collect();
    let mut tops: Vec<(usize, f32, f32)> = vec![]; // index, x, y (cm)
    for (i, it) in fig.items.iter().enumerate() {
        if let Item::Blob(bl) = it {
            if bl.parent.as_ref().is_some_and(|p| names.contains(p)) {
                continue;
            }
            if let Some(pos) = bl.pos {
                let (x, y) = match pos {
                    Pos::Cell(c, r) => (c * sp.0, -r * sp.1),
                    Pos::Abs(x, y) => (x, y),
                };
                tops.push((i, x, y));
            }
        }
    }
    if tops.len() >= 2 {
        tops.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let y = tops.iter().map(|t| t.2).sum::<f32>() / tops.len() as f32;
        let (x0, x1) = (tops[0].1, tops[tops.len() - 1].1);
        let step = (x1 - x0) / (tops.len() - 1) as f32;
        for (j, (i, _, _)) in tops.iter().enumerate() {
            let x = x0 + step * j as f32;
            let new = Pos::Abs((x * 100.0).round() / 100.0, (y * 100.0).round() / 100.0);
            if let Item::Blob(bl) = &mut fig.items[*i] {
                let old = bl.pos.unwrap();
                let same = match old {
                    Pos::Abs(ox, oy) => (ox - x).abs() < 0.005 && (oy - y).abs() < 0.005,
                    Pos::Cell(c, r) => (c * sp.0 - x).abs() < 0.005 && (-r * sp.1 - y).abs() < 0.005,
                };
                if !same {
                    bl.pos = Some(new);
                    edits.push(Edit::MoveBlob { name: bl.name.clone(), pos: new });
                }
            }
        }
    }
    edits
}
