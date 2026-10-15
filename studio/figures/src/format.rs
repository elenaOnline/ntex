//! Figure → canonical .fig text. Round-trip: parse(format(parse(s).0)).0 ==
//! parse(s).0 (spans aside). Comments, blank lines, statement order and grid
//! blocks are kept; a grid block whose objects were edited by the canvas is
//! rewritten as plain statements.
//!
//! Fidelity rules: statements that shared a line (`;`) share it again, a
//! trailing `% comment` stays on its line, nodes first named inside an arrow
//! (`Hints::implicit`) are written inline there, global settings go at the
//! top (after any leading comment lines), and a grid block is written back
//! verbatim, followed by any node labels/colours set outside it.
use crate::model::*;
use crate::parse::{self, default_label};
use std::collections::{HashMap, HashSet};

pub fn format(fig: &Figure) -> String {
    let plan = Plan::new(fig);
    let mut lines: Vec<Out> = Vec::new();
    let placed = placed_globals(fig);
    let global_out = |k: usize, lines: &mut Vec<Out>| {
        for (key, at, text) in &placed {
            let _ = key;
            if *at == k {
                lines.push(Out { text: text.clone(), line: 0, joinable: false, is_comment: false });
            }
        }
    };
    for (i, it) in fig.items.iter().enumerate() {
        global_out(i, &mut lines);
        plan.emit(fig, i, it, &mut lines);
    }
    global_out(fig.items.len(), &mut lines);
    // join statements that shared a source line
    let mut joined: Vec<(String, bool)> = Vec::new(); // (text, is a % comment line start)
    let mut prev: Option<&Out> = None;
    for o in &lines {
        if let Some(p) = prev {
            if o.line > 0 && p.line == o.line && p.joinable && o.joinable && !p.is_comment {
                let last = joined.last_mut().unwrap();
                if o.is_comment {
                    last.0.push(' ');
                } else {
                    last.0.push_str(" ; ");
                }
                last.0.push_str(&o.text);
                prev = Some(o);
                continue;
            }
        }
        joined.push((o.text.clone(), o.is_comment));
        prev = Some(o);
    }
    // globals she didn't place go after the leading comment lines
    let g: Vec<String> = globals_lines(&fig.globals)
        .into_iter()
        .filter(|l| !placed.iter().any(|(k, _, _)| l.split(' ').next() == Some(k.as_str())))
        .collect();
    if !g.is_empty() {
        let k = joined.iter().take_while(|(t, c)| *c && t.starts_with('%')).count();
        for (j, l) in g.into_iter().enumerate() {
            joined.insert(k + j, (l, false));
        }
    }
    if joined.is_empty() {
        return String::new();
    }
    let mut s = joined.into_iter().map(|x| x.0).collect::<Vec<_>>().join("\n");
    s.push('\n');
    s
}

/// One statement's text (for the window's inspector and the AI's diffs).
pub fn item(fig: &Figure, index: usize) -> String {
    let Some(it) = fig.items.get(index) else { return String::new() };
    match it {
        Item::Node(n) => node_stmt(fig, n),
        Item::Arrow(a) => arrow_stmt(fig, a, None, None),
        Item::Blob(b) => blob_stmt(fig, b),
        Item::NGon(g) => ngon_stmt(fig, g),
        Item::Cayley(c) => cayley_stmt(fig, c),
        Item::Text(t) => text_stmt(fig, t),
        Item::Grid { source, .. } => source.clone(),
        Item::Comment { text, .. } => text.clone(),
    }
}

/// A copy with every span zeroed: what the round trip compares.
pub fn without_spans(fig: &Figure) -> Figure {
    let z = Span::default();
    let mut f = fig.clone();
    for it in f.items.iter_mut() {
        match it {
            Item::Node(n) => n.span = z,
            Item::Arrow(a) => a.span = z,
            Item::Blob(b) => b.span = z,
            Item::NGon(g) => g.span = z,
            Item::Cayley(c) => c.span = z,
            Item::Text(t) => t.span = z,
            Item::Grid { span, .. } => *span = z,
            Item::Comment { span, .. } => *span = z,
        }
    }
    f
}

// ---------------------------------------------------------------------------

struct Out {
    text: String,
    line: usize,
    joinable: bool,
    is_comment: bool,
}

enum Verdict {
    Verbatim(Vec<String>),
    Convert,
}

struct Plan {
    grid_of: HashMap<usize, usize>,
    verdicts: HashMap<usize, Verdict>,
    absorbed: HashMap<usize, usize>,
    /// arrow item → its absorbed implicit nodes
    inline: HashMap<usize, Vec<usize>>,
}

impl Plan {
    fn new(fig: &Figure) -> Plan {
        let grid_of: HashMap<usize, usize> = fig.grid_items.iter().map(|&(i, g)| (i, g)).collect();
        let mut verdicts = HashMap::new();
        for (gi, it) in fig.items.iter().enumerate() {
            if let Item::Grid { source, .. } = it {
                let derived: Vec<usize> = {
                    let mut v: Vec<usize> = fig.grid_items.iter().filter(|x| x.1 == gi).map(|x| x.0).collect();
                    v.sort();
                    v
                };
                verdicts.insert(gi, grid_verdict(fig, source, &derived));
            }
        }
        let absorbed = absorption(fig);
        let mut inline: HashMap<usize, Vec<usize>> = HashMap::new();
        for (&n, &a) in &absorbed {
            inline.entry(a).or_default().push(n);
        }
        Plan { grid_of, verdicts, absorbed, inline }
    }

    fn emit(&self, fig: &Figure, i: usize, it: &Item, out: &mut Vec<Out>) {
        if let Some(g) = self.grid_of.get(&i) {
            if let Some(Verdict::Convert) = self.verdicts.get(g) {
                let text = match it {
                    Item::Node(n) => node_stmt(fig, n),
                    Item::Arrow(a) => arrow_stmt(fig, a, None, None),
                    _ => item(fig, i),
                };
                out.push(Out { text, line: 0, joinable: false, is_comment: false });
            }
            return;
        }
        let stmt = |text: String, line: usize| Out { text, line, joinable: true, is_comment: false };
        match it {
            Item::Node(n) => {
                if self.absorbed.contains_key(&i) {
                    return;
                }
                out.push(stmt(node_stmt(fig, n), n.span.line));
            }
            Item::Arrow(a) => {
                let nodes: Vec<&Node> = self
                    .inline
                    .get(&i)
                    .map(|v| v.iter().filter_map(|&k| if let Item::Node(n) = &fig.items[k] { Some(n) } else { None }).collect())
                    .unwrap_or_default();
                let from = nodes.iter().find(|n| n.name == a.from).copied();
                let to = nodes.iter().find(|n| n.name == a.to && n.name != a.from).copied();
                out.push(stmt(arrow_stmt(fig, a, from, to), a.span.line));
            }
            Item::Blob(b) => out.push(stmt(blob_stmt(fig, b), b.span.line)),
            Item::NGon(g) => out.push(stmt(ngon_stmt(fig, g), g.span.line)),
            Item::Cayley(c) => out.push(stmt(cayley_stmt(fig, c), c.span.line)),
            Item::Text(t) => out.push(stmt(text_stmt(fig, t), t.span.line)),
            Item::Comment { text, span } => {
                if text.starts_with('%') {
                    out.push(Out { text: text.clone(), line: span.line, joinable: true, is_comment: true });
                } else if text.trim().is_empty() {
                    out.push(Out { text: String::new(), line: 0, joinable: false, is_comment: true });
                } else {
                    // a statement that didn't parse, kept as written
                    out.push(stmt(text.clone(), span.line));
                }
            }
            Item::Grid { source, span } => match self.verdicts.get(&i) {
                Some(Verdict::Verbatim(sup)) => {
                    out.push(Out { text: source.clone(), line: span.line, joinable: false, is_comment: false });
                    for s in sup {
                        out.push(Out { text: s.clone(), line: 0, joinable: false, is_comment: false });
                    }
                }
                _ => out.push(Out {
                    text: format!("% grid (line {}) rewritten as statements: an edit changed it", span.line),
                    line: 0,
                    joinable: false,
                    is_comment: true,
                }),
            },
        }
    }
}

/// Which implicit Node items are written inline in an arrow: node item →
/// arrow item. A run of implicit nodes is absorbed by the arrow right after
/// it when they are exactly that arrow's new endpoints (in from, to order),
/// carry nothing an arrow can't say (a colour), and nothing earlier names them.
pub(crate) fn absorption(fig: &Figure) -> HashMap<usize, usize> {
    let implicit: HashSet<&str> = fig.hints.implicit.iter().map(|s| s.as_str()).collect();
    let grid: HashSet<usize> = fig.grid_items.iter().map(|x| x.0).collect();
    let is_imp = |k: usize| -> bool {
        !grid.contains(&k) && matches!(&fig.items[k], Item::Node(n) if implicit.contains(n.name.as_str()) && n.color.is_none())
    };
    let mut res = HashMap::new();
    let mut i = 0;
    while i < fig.items.len() {
        if !is_imp(i) {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < fig.items.len() && is_imp(j) {
            j += 1;
        }
        let run: Vec<&str> = (i..j).map(|k| if let Item::Node(n) = &fig.items[k] { n.name.as_str() } else { "" }).collect();
        if let Some(Item::Arrow(a)) = fig.items.get(j) {
            if !grid.contains(&j) {
                let mut ends: Vec<&str> = vec![a.from.as_str()];
                if a.to != a.from {
                    ends.push(a.to.as_str());
                }
                // the longest tail of the run that is exactly this arrow's new ends
                for k in (1..=run.len().min(2)).rev() {
                    let tail = &run[run.len() - k..];
                    let want: Vec<&str> = ends.iter().copied().filter(|e| tail.contains(e)).collect();
                    let start = j - k;
                    let earlier = fig.items[..start].iter().any(|it| match it {
                        Item::Arrow(b) => tail.contains(&b.from.as_str()) || tail.contains(&b.to.as_str()),
                        Item::Node(n) => tail.contains(&n.name.as_str()),
                        _ => false,
                    });
                    if want == tail && !earlier {
                        for m in start..j {
                            res.insert(m, j);
                        }
                        break;
                    }
                }
            }
        }
        i = j;
    }
    res
}

/// (FIG-INTERACT) Would `format` write grid item `gi` back verbatim (its
/// art still says what its objects are)?
pub(crate) fn grid_keeps(fig: &Figure, gi: usize) -> bool {
    let Some(Item::Grid { source, .. }) = fig.items.get(gi) else { return false };
    let mut derived: Vec<usize> = fig.grid_items.iter().filter(|x| x.1 == gi).map(|x| x.0).collect();
    derived.sort();
    matches!(grid_verdict(fig, source, &derived), Verdict::Verbatim(_))
}

fn grid_verdict(fig: &Figure, source: &str, derived: &[usize]) -> Verdict {
    let g = parse::grid_from_source(source, 1);
    let canon = |s: &str| crate::unicode::to_latex(s.trim());
    let mut sup = Vec::new();
    let d_nodes: Vec<&Node> = derived.iter().filter_map(|&k| if let Item::Node(n) = &fig.items[k] { Some(n) } else { None }).collect();
    let d_arrows: Vec<&Arrow> = derived.iter().filter_map(|&k| if let Item::Arrow(a) = &fig.items[k] { Some(a) } else { None }).collect();
    if d_arrows.len() != g.arrows.len() {
        return Verdict::Convert;
    }
    for (a, e) in d_arrows.iter().zip(g.arrows.iter()) {
        let el = e.label.as_ref().map(|l| crate::unicode::to_latex(l));
        let same = canon(&a.from) == canon(&e.from)
            && canon(&a.to) == canon(&e.to)
            && a.label == el
            && a.head == e.head
            && a.line == e.line_style
            && a.side == e.side
            && a.bend == 0.0
            && a.label_pos == LabelPos::Middle
            && !a.iso
            && !a.thick
            && a.color.is_none();
        if !same {
            return Verdict::Convert;
        }
    }
    let names: HashSet<String> = fig
        .items
        .iter()
        .enumerate()
        .filter(|(k, _)| !derived.contains(k))
        .flat_map(|(_, it)| match it {
            Item::Node(n) => vec![canon(&n.name)],
            Item::Blob(b) => {
                let mut v = vec![canon(&b.name)];
                v.extend(b.elements.iter().map(|e| canon(&e.name)));
                v
            }
            Item::NGon(p) => vec![canon(&p.name)],
            Item::Cayley(c) => vec![canon(&c.name)],
            _ => vec![],
        })
        .collect();
    let mut matched = HashSet::new();
    for n in &d_nodes {
        let Some(e) = g.nodes.iter().find(|e| canon(&e.name) == canon(&n.name)) else { return Verdict::Convert };
        matched.insert(canon(&e.name));
        if n.pos != Some(Pos::Cell(e.cell.0, e.cell.1)) {
            return Verdict::Convert;
        }
        let expected = e.label.as_ref().map(|l| crate::unicode::to_latex(l)).unwrap_or_else(|| default_label(&n.name));
        let mut s = String::new();
        if n.label != expected {
            if e.label.is_some() {
                return Verdict::Convert; // the grid's own label would win
            }
            s = format!("{} {}", n.name, quote(&sp(fig, &format!("node:{}", n.name), &n.label)));
        }
        if let Some(c) = &n.color {
            if s.is_empty() {
                s = n.name.clone();
            }
            s.push_str(&format!(" color={c}"));
        }
        if !s.is_empty() {
            sup.push(s);
        }
    }
    for e in &g.nodes {
        if !matched.contains(&canon(&e.name)) && !names.contains(&canon(&e.name)) {
            return Verdict::Convert;
        }
    }
    Verdict::Verbatim(sup)
}

// ---------------------------------------------------------------------------
// statements

pub(crate) fn num(x: f32) -> String {
    if x == 0.0 { "0".into() } else { format!("{x}") }
}

pub(crate) fn pos(p: &Pos) -> String {
    match p {
        Pos::Cell(x, y) => format!("at {},{}", num(*x), num(*y)),
        Pos::Abs(x, y) => format!("at ({},{})", num(*x), num(*y)),
    }
}

fn quote(l: &str) -> String {
    format!("\"{l}\"")
}

/// Does a label need quotes inside an arrow (`-"g \circ f"->`)?
fn needs_quote(l: &str) -> bool {
    l.is_empty() || l.chars().any(|c| c.is_whitespace() || "-<>.=|,;\"%".contains(c) || c == '\u{E000}') || {
        let mut d = 0i32;
        let mut esc = false;
        let mut bad = false;
        for c in l.chars() {
            if esc {
                esc = false;
                continue;
            }
            match c {
                '\\' => esc = true,
                '{' => d += 1,
                '}' => {
                    d -= 1;
                    if d < 0 {
                        bad = true;
                    }
                }
                _ => {}
            }
        }
        bad || d != 0
    }
}

fn sp(fig: &Figure, key: &str, latex: &str) -> String {
    crate::spelling::raw(fig, key, latex)
}

fn term(fig: &Figure, n: &Node, inline: bool) -> String {
    let mut s = n.name.clone();
    if n.label != default_label(&n.name) {
        s.push(' ');
        s.push_str(&quote(&sp(fig, &format!("node:{}", n.name), &n.label)));
    }
    if let Some(p) = &n.pos {
        s.push(' ');
        s.push_str(&pos(p));
    }
    if !inline {
        if let Some(c) = &n.color {
            s.push_str(&format!(" color={c}"));
        }
    }
    s
}

fn node_stmt(fig: &Figure, n: &Node) -> String {
    term(fig, n, false)
}

pub(crate) fn arrow_token(head: Head, line: Line, label: Option<&str>) -> (String, Option<&'static str>) {
    let l = label.map(|l| if needs_quote(l) { quote(l) } else { l.to_string() });
    let shaft = |line: Line| -> (&'static str, &'static str) {
        match line {
            Line::Solid => ("-", "-"),
            Line::Dashed => ("--", "--"),
            Line::Dotted => (".", "."),
        }
    };
    let bare = |line: Line| -> &'static str {
        match line {
            Line::Solid => "-",
            Line::Dashed => "--",
            Line::Dotted => "..",
        }
    };
    let opt = match line {
        Line::Solid => None,
        Line::Dashed => Some("dashed"),
        Line::Dotted => Some("dotted"),
    };
    let body = |pre: &str, line: Line, post: &str| -> String {
        match &l {
            None => format!("{pre}{}{post}", bare(line)),
            Some(l) => {
                let (a, b) = shaft(line);
                format!("{pre}{a}{l}{b}{post}")
            }
        }
    };
    match head {
        Head::To => (body("", line, ">"), None),
        Head::TwoHeads => (body("", line, ">>"), None),
        Head::Tail => (body(">", line, ">"), None),
        Head::MapsTo => (body("|", line, ">"), None),
        Head::Both => (body("<", line, ">"), None),
        Head::Hook => (
            match &l {
                None => "hook->".to_string(),
                Some(l) => format!("-{l}-hook->"),
            },
            opt,
        ),
        Head::Double => (
            match &l {
                None => "=>".to_string(),
                Some(l) => format!("={l}=>"),
            },
            opt,
        ),
        Head::Equal => (
            match &l {
                None => "=".to_string(),
                Some(l) => format!("={l}="),
            },
            opt,
        ),
    }
}

fn arrow_stmt(fig: &Figure, a: &Arrow, from: Option<&Node>, to: Option<&Node>) -> String {
    let f = from.map(|n| term(fig, n, true)).unwrap_or_else(|| a.from.clone());
    let t = to.map(|n| term(fig, n, true)).unwrap_or_else(|| a.to.clone());
    let label = a.label.as_ref().map(|l| sp(fig, &crate::spelling::arrow_key(&a.from, &a.to), l));
    let (tok, line_opt) = arrow_token(a.head, a.line, label.as_deref());
    let mut s = format!("{f} {tok} {t}");
    if a.is_loop && a.from != a.to {
        s.push_str(" loop");
    }
    if a.bend != 0.0 {
        if a.bend == 30.0 {
            s.push_str(" bend-left");
        } else if a.bend == -30.0 {
            s.push_str(" bend-right");
        } else {
            s.push_str(&format!(" bend={}", num(a.bend)));
        }
    }
    if a.side == LabelSide::Swap {
        s.push_str(" swap");
    }
    match a.label_pos {
        LabelPos::Middle => {}
        LabelPos::NearStart => s.push_str(" near-start"),
        LabelPos::NearEnd => s.push_str(" near-end"),
    }
    if a.iso {
        s.push_str(" iso");
    }
    if a.thick {
        s.push_str(" thick");
    }
    if let Some(c) = &a.color {
        s.push_str(&format!(" color={c}"));
    }
    if let Some(o) = line_opt {
        s.push(' ');
        s.push_str(o);
    }
    s
}

fn elem(fig: &Figure, e: &Element) -> String {
    let mut s = e.name.clone();
    if e.label != default_label(&e.name) {
        s.push(' ');
        s.push_str(&quote(&sp(fig, &format!("elem:{}", e.name), &e.label)));
    }
    if let Some(p) = &e.pos {
        s.push(' ');
        s.push_str(&pos(p));
    }
    s
}

fn blob_stmt(fig: &Figure, b: &Blob) -> String {
    let mut s = format!("blob {}", b.name);
    if b.label != default_label(&b.name) {
        s.push(' ');
        s.push_str(&quote(&sp(fig, &format!("blob:{}", b.name), &b.label)));
    }
    let mut entries: Vec<String> = Vec::new();
    let mut written: HashSet<&str> = HashSet::new();
    if let Some((_, listed)) = fig.hints.listed.iter().find(|(n, _)| *n == b.name) {
        for name in listed {
            if let Some(e) = b.elements.iter().find(|e| e.name == *name) {
                entries.push(elem(fig, e));
                written.insert(e.name.as_str());
            } else if descendant_has(fig, &b.name, name) {
                entries.push(name.clone());
            }
        }
    }
    for e in &b.elements {
        if !written.contains(e.name.as_str()) {
            entries.push(elem(fig, e));
        }
    }
    if !entries.is_empty() {
        s.push_str(&format!(" {{{}}}", entries.join(", ")));
    }
    if let Some(p) = &b.parent {
        s.push_str(&format!(" in {p}"));
    }
    if b.shaded {
        s.push_str(" shaded");
    }
    if let Some(c) = &b.color {
        s.push_str(&format!(" color={c}"));
    }
    match b.shape {
        BlobShape::Round => {}
        BlobShape::Tall => s.push_str(" shape=tall"),
        BlobShape::Wide => s.push_str(" shape=wide"),
        BlobShape::Bean => s.push_str(" shape=bean"),
    }
    if let Some((w, h)) = b.size {
        s.push_str(&format!(" size={},{}", num(w), num(h)));
    }
    if b.angle != 0.0 {
        s.push_str(&format!(" angle={}", num(b.angle)));
    }
    if let Some(p) = &b.pos {
        s.push(' ');
        s.push_str(&pos(p));
    }
    match b.label_place {
        LabelPlace::Above => {}
        LabelPlace::Below => s.push_str(" label=below"),
        LabelPlace::Left => s.push_str(" label=left"),
        LabelPlace::Right => s.push_str(" label=right"),
        LabelPlace::Inside => s.push_str(" label=inside"),
    }
    s
}

/// Is `elem` an element of a blob strictly inside `blob`?
pub(crate) fn descendant_has(fig: &Figure, blob: &str, elem: &str) -> bool {
    let blobs: Vec<&Blob> = fig.items.iter().filter_map(|it| if let Item::Blob(b) = it { Some(b) } else { None }).collect();
    let Some(owner) = blobs.iter().find(|b| b.elements.iter().any(|e| e.name == elem)) else { return false };
    let mut cur = owner.parent.clone();
    let mut guard = 0;
    while let Some(p) = cur {
        if p == blob {
            return true;
        }
        cur = blobs.iter().find(|b| b.name == p).and_then(|b| b.parent.clone());
        guard += 1;
        if guard > 1000 {
            break;
        }
    }
    false
}

fn list_item(l: &str) -> String {
    if l.is_empty() || l.chars().any(|c| c.is_whitespace() || ",;\"%".contains(c)) { quote(l) } else { l.to_string() }
}

fn axis(v: u32) -> String {
    if v % 2 == 0 { format!("{}", v / 2) } else { format!("{}.5", (v - 1) / 2) }
}

fn ngon_stmt(fig: &Figure, g: &NGon) -> String {
    let mut s = format!("ngon {} {}", g.name, g.n);
    let default: Vec<String> = (1..=g.n).map(|k| k.to_string()).collect();
    if g.labels != default {
        let last = g.labels.len().saturating_sub(1);
        let ls: Vec<String> = g
            .labels
            .iter()
            .enumerate()
            .map(|(k, l)| match l.is_empty() {
                // a hidden label is an empty entry (`labels 1,,3,4`); the
                // last one is written `""`, so a trailing comma can't join
                // the next option onto the list
                true if k == last => "\"\"".to_string(),
                true => String::new(),
                false => list_item(&sp(fig, &format!("ngon:{}.{}", g.name, k + 1), l)),
            })
            .collect();
        s.push_str(&format!(" labels {}", ls.join(",")));
    }
    if !g.show_labels {
        s.push_str(" labels=none");
    }
    if g.all_axes {
        s.push_str(" axes");
    }
    for a in &g.axes {
        s.push_str(&format!(" axis {}", axis(*a)));
        if let Some((_, l)) = g.axis_labels.iter().find(|(c, _)| c == a) {
            s.push_str(&format!(" {}", quote(&sp(fig, &format!("ngon:{}.axis.{}", g.name, a), l))));
        }
    }
    // labels of axes not listed (`axes` draws them all)
    for (a, l) in &g.axis_labels {
        if !g.axes.contains(a) {
            s.push_str(&format!(" axis {} {}", axis(*a), quote(&sp(fig, &format!("ngon:{}.axis.{}", g.name, a), l))));
        }
    }
    if g.rotation > 0 {
        // (figure-qol D) clockwise: `rotation cw` for one step, else `rotation=-k`
        match (g.rotation, g.rotation_cw) {
            (1, false) => s.push_str(" rotation"),
            (1, true) => s.push_str(" rotation cw"),
            (k, false) => s.push_str(&format!(" rotation={k}")),
            (k, true) => s.push_str(&format!(" rotation=-{k}")),
        }
        if let Some(e) = g.rotation_edge {
            s.push_str(&format!(" edge={e}"));
        }
    }
    if g.rotation > 0
        && let Some(l) = &g.rotation_label
    {
        s.push_str(&format!(" {}", quote(&sp(fig, &format!("ngon:{}.rot", g.name), l))));
    }
    if g.turn != 0 {
        if g.turn_neg {
            s.push_str(&format!(" turn=-{}", g.n.max(1).saturating_sub(g.turn % g.n.max(1)).max(1)));
        } else {
            s.push_str(&format!(" turn={}", g.turn));
        }
    }
    if let Some(f) = g.flip {
        s.push_str(&format!(" flip={}", axis(f)));
    }
    if let Some(r) = g.radius {
        s.push_str(&format!(" radius={}", num(r)));
    }
    if g.angle != 0.0 {
        s.push_str(&format!(" angle={}", num(g.angle)));
    }
    if !g.shaded {
        s.push_str(" plain");
    }
    if let Some(c) = &g.color {
        s.push_str(&format!(" color={c}"));
    }
    if let Some(p) = &g.pos {
        s.push(' ');
        s.push_str(&pos(p));
    }
    s
}

fn cayley_stmt(fig: &Figure, c: &Cayley) -> String {
    let mut s = String::from("cayley");
    let spelled = |v: &[String], kind: &str| -> Vec<String> {
        v.iter().enumerate().map(|(k, l)| sp(fig, &format!("{kind}:{}.{}", c.name, k + 1), l)).collect()
    };
    match &c.group {
        GroupSpec::Named(g) => {
            s.push_str(&format!(" {}", sp(fig, &format!("group:{}", c.name), g)));
            if !c.gens.is_empty() {
                s.push_str(&format!(" <{}>", spelled(&c.gens, "cayley").join(", ")));
            }
            if c.name != *g {
                s.push_str(&format!(" name={}", c.name));
            }
        }
        GroupSpec::Presentation(gens, rels) => {
            if c.name != "C" {
                s.push_str(&format!(" {}", c.name));
            }
            let gens = if gens.is_empty() || *gens == c.gens { spelled(&c.gens, "cayley") } else { gens.clone() };
            s.push_str(&format!(" <{} | {}>", gens.join(", "), spelled(rels, "rel").join(", ")));
        }
    }
    match c.layout {
        CayleyLayout::Auto => {}
        CayleyLayout::Circle => s.push_str(" layout=circle"),
        CayleyLayout::Prism => s.push_str(" layout=prism"),
        CayleyLayout::Grid => s.push_str(" layout=grid"),
    }
    if !c.labels {
        s.push_str(" labels=none");
    }
    if !c.gen_styles.is_empty() {
        // (figure-qol D) `gens r:maroon,s^-1,t:hidden`, the generators as she spelled them
        let names = spelled(&c.gens, "cayley");
        let parts: Vec<String> = c
            .gen_styles
            .iter()
            .map(|st| {
                let mut p = match c.gens.iter().position(|g| *g == st.generator) {
                    Some(k) => names[k].clone(),
                    None => st.generator.clone(),
                };
                if st.inverse {
                    p.push_str("^-1");
                }
                if let Some(col) = &st.color {
                    p.push_str(&format!(":{col}"));
                }
                if st.hidden {
                    p.push_str(":hidden");
                }
                p
            })
            .collect();
        s.push_str(&format!(" gens {}", parts.join(",")));
    }
    if c.legend {
        s.push_str(" legend");
    }
    if let Some(r) = c.radius {
        s.push_str(&format!(" radius={}", num(r)));
    }
    if c.angle != 0.0 {
        s.push_str(&format!(" angle={}", num(c.angle)));
    }
    for (w, l) in &c.vertex_names {
        s.push_str(&format!(" vertex {w} {}", quote(&sp(fig, &format!("cayley:{}.v.{w}", c.name), l))));
    }
    if let Some(p) = &c.pos {
        s.push(' ');
        s.push_str(&pos(p));
    }
    s
}

fn text_stmt(fig: &Figure, t: &Text) -> String {
    format!("text {} {}", quote(&sp(fig, "text", &t.text)), pos(&t.pos))
}

/// (FIG-POLISH) The globals with a remembered place: (key, item index, line).
fn placed_globals(fig: &Figure) -> Vec<(String, usize, String)> {
    let lines = globals_lines(&fig.globals);
    let n = fig.items.len();
    fig.hints
        .globals_at
        .iter()
        .filter_map(|(k, at)| {
            let l = lines.iter().find(|l| l.split(' ').next() == Some(k.as_str()))?;
            Some((k.clone(), (*at).min(n), l.clone()))
        })
        .collect()
}

fn globals_lines(g: &Globals) -> Vec<String> {
    let d = Globals::default();
    let mut v = Vec::new();
    if g.scale != d.scale {
        v.push(format!("scale {}", num(g.scale)));
    }
    if g.spacing != d.spacing {
        v.push(format!("spacing {},{}", num(g.spacing.0), num(g.spacing.1)));
    }
    if g.size != d.size {
        v.push(format!("size {}", g.size));
    }
    v
}
