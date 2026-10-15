//! Scene (+ Figure, for node names and arrow styles) → TikZ. Uses tikz-cd's
//! arrow styles (`\usetikzlibrary{cd}`) and her palette colours (ntex-base.sty).
//!
//! Everything is emitted at the Scene's coordinates, so the PDF and the
//! canvas agree by construction: diagram nodes and labelled Cayley vertices
//! are named TikZ nodes (tikz-cd cells) at their Scene points, and arrows run
//! between those names along the Scene's own curve (TikZ clips them to the
//! real text boxes); everything else is explicit coordinates. The bounding
//! box is the Scene's `bounds`, so PNG pixel ↔ Scene cm is a fixed map.
use crate::layout::label_t;
use crate::model::*;
use std::collections::HashMap;
use std::fmt::Write;

/// The palette, as in ntex-base.sty: (figure colour name, Rule hex, Back hex).
/// `grey` is ntexSlate (with the proof grey as its tint); `ink` is ntexInk.
pub const PALETTE: [(&str, &str, &str); 8] = [
    ("ink", "1E1E28", "F3F3F3"),
    ("grey", "5A6378", "F3F3F3"),
    ("blue", "1E47B8", "E0E7FB"),
    ("violet", "661EB8", "ECE0FB"),
    ("teal", "1C829C", "E2F4F8"),
    ("maroon", "A92D4A", "F8EAED"),
    ("green", "238B45", "E5F5EA"),
    ("ochre", "B07D12", "FAF3E2"),
];

fn cap(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_ascii_uppercase().to_string() + c.as_str(),
        None => String::new(),
    }
}

/// A palette name (or `<name>-tint`) → the xcolor name from ntex-figure.sty.
pub fn color_name(c: &str) -> String {
    let (base, tint) = match c.strip_suffix("-tint") {
        Some(b) => (b, true),
        None => (c, false),
    };
    let base = if PALETTE.iter().any(|p| p.0 == base) { base } else { "ink" };
    format!("ntexFig{}{}", cap(base), if tint { "Tint" } else { "" })
}

fn num(v: f32) -> String {
    let v = if v.abs() < 0.0005 { 0.0 } else { v };
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" { "0".into() } else { s }
}
fn pt(q: Pt) -> String {
    format!("({},{})", num(q.x), num(q.y))
}

fn font(size: f32, base: f32) -> &'static str {
    let r = size / base;
    if r < 0.8 {
        "\\footnotesize"
    } else if r < 0.95 {
        "\\small"
    } else if r > 1.35 {
        "\\Large"
    } else if r > 1.15 {
        "\\large"
    } else {
        ""
    }
}

fn base_size(fig: &Figure) -> f32 {
    match fig.globals.size.as_str() {
        "small" => 9.0,
        "large" => 12.0,
        _ => 10.0,
    }
}

fn tip_keys(start: Option<Head>, end: Option<Head>, double: bool) -> &'static str {
    match (start, end, double) {
        (None, None, true) => "equal",
        (None, None, false) => "dash",
        (None, Some(Head::Double), _) | (None, Some(Head::To), true) => "Rightarrow",
        (None, Some(Head::TwoHeads), _) => "two heads",
        (Some(Head::Tail), _, _) => "tail",
        (Some(Head::Hook), _, _) => "hook",
        (Some(Head::MapsTo), _, _) => "maps to",
        (Some(_), Some(_), _) => "leftrightarrow",
        (Some(_), None, _) => "leftarrow",
        _ => "rightarrow",
    }
}

fn line_key(l: Line) -> &'static str {
    match l {
        Line::Solid => "",
        Line::Dashed => ", dashed",
        Line::Dotted => ", dotted",
    }
}

fn seg_text(start_ref: &str, segs: &[Seg], end_ref: Option<&str>, closed: bool, label: &str) -> String {
    seg_text_at(start_ref, None, segs, end_ref, closed, label)
}

fn seg_text_at(start_ref: &str, start_pt: Option<Pt>, segs: &[Seg], end_ref: Option<&str>, closed: bool, label: &str) -> String {
    let mut s = start_ref.to_string();
    let n = segs.len();
    // a closed path whose last segment comes back to the start ends in `cycle`;
    // a closed polyline that stops at its last vertex gets ` -- cycle` added
    let returns = match segs.last() {
        Some(Seg::Line(e)) | Some(Seg::Cubic(_, _, e)) => start_pt.is_some_and(|q| (q.x - e.x).abs() < 1e-4 && (q.y - e.y).abs() < 1e-4),
        None => false,
    };
    for (i, g) in segs.iter().enumerate() {
        let last = i + 1 == n;
        let target = if last && closed && returns {
            "cycle".to_string()
        } else if last {
            end_ref.map(|r| r.to_string()).unwrap_or_else(|| match g {
                Seg::Line(e) | Seg::Cubic(_, _, e) => pt(*e),
            })
        } else {
            match g {
                Seg::Line(e) | Seg::Cubic(_, _, e) => pt(*e),
            }
        };
        let lab = if last { label } else { "" };
        match g {
            Seg::Line(_) => {
                let _ = write!(s, " --{lab} {target}");
            }
            Seg::Cubic(c1, c2, _) => {
                let _ = write!(s, " .. controls {} and {} ..{lab} {target}", pt(*c1), pt(*c2));
            }
        }
    }
    if closed && !returns {
        s.push_str(" -- cycle");
    }
    s
}

struct Named {
    /// obj → tikz node name
    names: HashMap<ObjRef, String>,
    /// (tikz name, centre, box) for clipping lookups
    boxes: Vec<(String, Pt, Rect)>,
}

fn named_nodes(scene: &Scene) -> Named {
    let labelled: std::collections::HashSet<&ObjRef> = scene
        .draws
        .iter()
        .filter_map(|d| match d {
            Draw::Label { obj, .. } if matches!(obj, ObjRef::Node(_) | ObjRef::CayleyVertex(..)) => Some(obj),
            _ => None,
        })
        .collect();
    let mut names = HashMap::new();
    let mut boxes = vec![];
    let (mut ni, mut vi) = (0, 0);
    for (obj, at, r) in &scene.anchors {
        if !labelled.contains(obj) || names.contains_key(obj) {
            continue;
        }
        let nm = match obj {
            ObjRef::Node(_) => {
                ni += 1;
                format!("n{ni}")
            }
            _ => {
                vi += 1;
                format!("v{vi}")
            }
        };
        names.insert(obj.clone(), nm.clone());
        boxes.push((nm, *at, r.clone()));
    }
    Named { names, boxes }
}

fn attach(named: &Named, q: Pt) -> Option<String> {
    let e = 0.03;
    named
        .boxes
        .iter()
        .filter(|(_, _, r)| q.x >= r.min.x - e && q.x <= r.max.x + e && q.y >= r.min.y - e && q.y <= r.max.y + e)
        .min_by(|a, b| {
            let da = (a.1.x - q.x).powi(2) + (a.1.y - q.y).powi(2);
            let db = (b.1.x - q.x).powi(2) + (b.1.y - q.y).powi(2);
            da.partial_cmp(&db).unwrap()
        })
        .map(|x| format!("({})", x.0))
}

/// `(n3)` → ("n3", its centre), for a reference that names a node.
fn start_ref_name(named: &Named, r: &str) -> Option<(String, Pt)> {
    let n = r.strip_prefix('(')?.strip_suffix(')')?;
    named.boxes.iter().find(|b| b.0 == n).map(|b| (b.0.clone(), b.1))
}

fn esc_comment(s: &str) -> String {
    s.replace('\n', " ")
}

/// The tikzpicture only.
pub fn picture(fig: &Figure, scene: &Scene) -> String {
    let named = named_nodes(scene);
    let base = base_size(fig);
    let mut o = String::new();
    o.push_str("\\begin{tikzpicture}[ntex figure]\n");
    let bb = &scene.bounds;
    let _ = writeln!(o, "  \\useasboundingbox {} rectangle {};", pt(bb.min), pt(bb.max));

    // arrow labels belong to their paths
    let mut arrow_labels: HashMap<usize, Vec<(Pt, String)>> = HashMap::new();
    for d in &scene.draws {
        if let Draw::Label { obj: ObjRef::Arrow(i), at, latex, .. } = d {
            arrow_labels.entry(*i).or_default().push((*at, latex.clone()));
        }
    }
    // 1. fills and outlines that sit under everything (blobs, n-gons, axes)
    // 2. named nodes   3. the rest in scene order
    let under = |d: &Draw| matches!(d, Draw::Path { obj: ObjRef::Blob(_) | ObjRef::NGon(_), start_tip: None, end_tip: None, .. });
    let mut ordered: Vec<&Draw> = scene.draws.iter().filter(|d| under(d)).collect();
    ordered.extend(scene.draws.iter().filter(|d| !under(d)));

    // named nodes first among the rest, so paths can refer to them
    o.push_str("  % objects\n");
    for d in &ordered {
        if let Draw::Path { obj, start, segs, closed, stroke, fill, .. } = d {
            if !under(d) {
                continue;
            }
            let mut opts: Vec<String> = vec![];
            if let Some(s) = stroke {
                opts.push(format!("draw={}", color_name(&s.color)));
                opts.push(format!("line width={}pt", num(s.width)));
                match s.line {
                    Line::Dashed => opts.push("dash pattern=on 2.4pt off 1.8pt".into()),
                    Line::Dotted => opts.push("dash pattern=on 0.6pt off 1.6pt".into()),
                    Line::Solid => {}
                }
            }
            if let Some(f) = fill {
                opts.push(format!("fill={}", color_name(f)));
            }
            let what = match obj {
                ObjRef::Blob(n) => format!("blob {n}"),
                ObjRef::NGon(n) => format!("n-gon {n}"),
                _ => String::new(),
            };
            let _ = writeln!(o, "  \\path[{}] {}; % {}", opts.join(", "), seg_text_at(&pt(*start), Some(*start), segs, None, *closed, ""), esc_comment(&what));
        }
    }
    for d in &ordered {
        if let Draw::Label { obj, at, latex, color, size, math } = d {
            let Some(nm) = named.names.get(obj) else { continue };
            let f = font(*size, base);
            let content = if *math { format!("${latex}$") } else { latex.clone() };
            let cell = match obj {
                ObjRef::Node(_) => "commutative diagrams/every cell".to_string(),
                _ => format!("inner sep={}pt", num(crate::layout::VERTEX_SEP)),
            };
            let mut opts = vec![cell];
            if color != "ink" {
                opts.push(format!("ntex {color}"));
            }
            if !f.is_empty() {
                opts.push(format!("font={f}"));
            }
            let what = match obj {
                ObjRef::Node(n) => n.clone(),
                ObjRef::CayleyVertex(c, w) => format!("{c}.{w}"),
                _ => String::new(),
            };
            let _ = writeln!(o, "  \\node[{}] ({nm}) at {} {{{content}}}; % {}", opts.join(", "), pt(*at), esc_comment(&what));
        }
    }
    for d in &ordered {
        match d {
            Draw::Path { obj, start, segs, closed, stroke, fill, start_tip, end_tip, .. } => {
                if under(d) {
                    continue;
                }
                let Some(s) = stroke else {
                    if let Some(f) = fill {
                        let _ = writeln!(o, "  \\fill[{}] {};", color_name(f), seg_text_at(&pt(*start), Some(*start), segs, None, *closed, ""));
                    }
                    continue;
                };
                let is_arrowish = matches!(obj, ObjRef::Arrow(_) | ObjRef::CayleyEdge(..)) || start_tip.is_some() || end_tip.is_some();
                if !is_arrowish {
                    let _ = writeln!(o, "  \\draw[draw={}, line width={}pt{}] {};", color_name(&s.color), num(s.width), line_key(s.line), seg_text_at(&pt(*start), Some(*start), segs, None, *closed, ""));
                    continue;
                }
                let keys = tip_keys(*start_tip, *end_tip, s.double);
                let mut opts = format!("commutative diagrams/.cd, every arrow, every label, {keys}, /tikz/.cd");
                if s.color != "ink" {
                    let _ = write!(opts, ", ntex {}", s.color);
                }
                if (s.width - 0.4).abs() > 0.05 {
                    let _ = write!(opts, ", line width={}pt", num(s.width));
                }
                opts.push_str(line_key(s.line));
                let start_ref = if matches!(obj, ObjRef::Arrow(_) | ObjRef::CayleyEdge(..)) { attach(&named, *start).unwrap_or_else(|| pt(*start)) } else { pt(*start) };
                let end = match segs.last() {
                    Some(Seg::Line(e)) | Some(Seg::Cubic(_, _, e)) => *e,
                    None => *start,
                };
                let end_ref = if matches!(obj, ObjRef::Arrow(_) | ObjRef::CayleyEdge(..)) { attach(&named, end) } else { None };
                // (FIG-POLISH) a straight arrow drawn beside another one
                // between the same nodes: tikz-cd's way, a canvas shift
                if let (Some(sr), Some(er), [Seg::Line(_)]) = (&start_ref_name(&named, &start_ref), &start_ref_name(&named, &end_ref.clone().unwrap_or_default()), segs.as_slice()) {
                    let (c0, c1) = (sr.1, er.1);
                    let (dx, dy) = (c1.x - c0.x, c1.y - c0.y);
                    let l = (dx * dx + dy * dy).sqrt();
                    if l > 1e-4 {
                        let (ux, uy) = (dx / l, dy / l);
                        let off = ux * (start.y - c0.y) - uy * (start.x - c0.x);
                        if off.abs() > 0.02 {
                            let _ = write!(opts, ", transform canvas={{shift={{({},{})}}}}", num(-uy * off), num(ux * off));
                        }
                    }
                }
                // labels: as tikz-cd places them (auto / swap, pos)
                let mut lab = String::new();
                let mut what = String::new();
                if let ObjRef::Arrow(i) = obj {
                    if let Some(Item::Arrow(a)) = fig.items.get(*i) {
                        what = format!("{} -> {}", a.from, a.to);
                        let t = label_t(a.label_pos);
                        let (q, tan) = crate::layout::path_at(*start, segs, t);
                        for (at, latex) in arrow_labels.get(i).into_iter().flatten() {
                            // which side the Scene put it on decides auto/swap
                            let cross = tan.x * (at.y - q.y) - tan.y * (at.x - q.x);
                            let mut lo: Vec<String> = vec![];
                            if cross < 0.0 {
                                lo.push("swap".into());
                            }
                            if (t - 0.5).abs() > 1e-3 {
                                lo.push(format!("pos={}", num(t)));
                            }
                            let lo = if lo.is_empty() { String::new() } else { format!("[{}]", lo.join(", ")) };
                            let _ = write!(lab, " node{lo} {{${latex}$}}");
                        }
                    }
                } else if let ObjRef::CayleyEdge(c, i) = obj {
                    what = format!("{c} edge {i}");
                }
                let _ = writeln!(o, "  \\draw[{opts}] {};{}", seg_text(&start_ref, segs, end_ref.as_deref(), false, &lab), if what.is_empty() { String::new() } else { format!(" % {}", esc_comment(&what)) });
            }
            Draw::Dot { at, r, color, .. } => {
                let _ = writeln!(o, "  \\fill[{}] {} circle[radius={}pt];", color_name(color), pt(*at), num(r / crate::layout::PT));
            }
            Draw::Label { obj, at, latex, color, size, math } => {
                if named.names.contains_key(obj) || matches!(obj, ObjRef::Arrow(_)) {
                    continue;
                }
                let f = font(*size, base);
                let content = if *math { format!("${latex}$") } else { latex.clone() };
                let mut opts = vec!["inner sep=0pt".to_string()];
                if color != "ink" {
                    opts.push(format!("ntex {color}"));
                }
                if !f.is_empty() {
                    opts.push(format!("font={f}"));
                }
                let _ = writeln!(o, "  \\node[{}] at {} {{{content}}};", opts.join(", "), pt(*at));
            }
        }
    }
    o.push_str("\\end{tikzpicture}\n");
    o
}

/// A complete standalone document using ntex-figure.sty.
pub fn document(fig: &Figure, scene: &Scene) -> String {
    format!("% Ntex Figures — built by ntex-fig; edit the .fig, not this.\n\\documentclass[border=3pt]{{standalone}}\n\\usepackage{{ntex-figure}}\n\\begin{{document}}\n{}\\end{{document}}\n", picture(fig, scene))
}

/// The styles ntex-figure.sty defines, for `portable`.
fn inline_styles() -> String {
    let mut s = String::new();
    s.push_str("% colours (ntex-base.sty) and styles (ntex-figure.sty), inlined\n");
    for (name, rule, back) in PALETTE {
        let _ = writeln!(s, "\\definecolor{{ntexFig{}}}{{HTML}}{{{rule}}}\\definecolor{{ntexFig{}Tint}}{{HTML}}{{{back}}}%", cap(name), cap(name));
    }
    s.push_str("\\tikzset{ntex figure/.style={color=ntexFigInk, line join=round},\n");
    for (name, _, _) in PALETTE {
        let _ = writeln!(s, "  ntex {name}/.style={{color=ntexFig{}}}, ntex {name} shade/.style={{fill=ntexFig{}Tint}},", cap(name), cap(name));
    }
    s.push_str("}%\n");
    s
}

/// The same picture with the palette and styles inlined (no ntex-figure.sty
/// needed): for pasting into someone else's .tex (`ntex-fig tikz --portable`).
pub fn portable(fig: &Figure, scene: &Scene) -> String {
    format!(
        "% Ntex Figures (portable): needs \\usepackage{{tikz-cd}} (or tikz with\n% \\usetikzlibrary{{cd}}) in the preamble. The fonts are whatever your document uses.\n{{%\n{}{}}}%\n",
        inline_styles(),
        picture(fig, scene)
    )
}
