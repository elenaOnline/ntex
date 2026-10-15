//! The Ask-box evaluation: every task in tasks.txt, through the real
//! `figure_ai::AskJob` (same prompt, flags, retry and timeout as the
//! window), checked against the parsed answer.
//!
//!   cargo run --release -- check                      the tasks and reference/*.fig (no model calls)
//!   cargo run --release -- run <config> [runs] [--tasks a,b] [--jobs 3]
//!   cargo run --release -- report                     results/*.jsonl → results/table.md
//!   cargo run --release -- rescore                    re-check stored answers with the current checks
//!   (EVAL_RESULTS=<dir> points any of these at another results folder)
//!   cargo run --release -- configs                    the configurations
//!
//! Results append to results/<config>.jsonl; a (task, run) already there is
//! skipped, so an interrupted run resumes and a finished one costs nothing.
use ntex_figures::model::*;
use ntex_studio_io::figure_ai::{self, AskEvent, AskJob, AskModel};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// configurations

struct Config {
    name: &'static str,
    backend: &'static str,
    model: &'static str,
    effort: &'static str,
    /// codex service tier: "priority" | "default" (claude: ignored)
    tier: &'static str,
}

const CONFIGS: &[Config] = &[
    Config { name: "luna-low", backend: "codex", model: "gpt-6-luna", effort: "low", tier: "default" },
    Config { name: "luna-low-priority", backend: "codex", model: "gpt-6-luna", effort: "low", tier: "priority" },
    Config { name: "luna-medium", backend: "codex", model: "gpt-6-luna", effort: "medium", tier: "default" },
    Config { name: "luna-medium-priority", backend: "codex", model: "gpt-6-luna", effort: "medium", tier: "priority" },
    Config { name: "haiku-low", backend: "claude", model: "claude-haiku-5-5", effort: "low", tier: "" },
    Config { name: "haiku-medium", backend: "claude", model: "claude-haiku-5-5", effort: "medium", tier: "" },
    Config { name: "sonnet-low", backend: "claude", model: "claude-sonnet-5-5", effort: "low", tier: "" },
    Config { name: "sonnet-medium", backend: "claude", model: "claude-sonnet-5-5", effort: "medium", tier: "" },
    Config { name: "sol-low", backend: "codex", model: "gpt-6.1-sol", effort: "low", tier: "default" },
];

fn eval_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

// ---------------------------------------------------------------------------
// tasks

#[derive(Clone, Debug)]
struct Task {
    id: String,
    kind: String,
    ask: String,
    source: String,
    expect: Vec<String>,
    touch: Vec<String>,
}

/// Which version of a task an answer was given to: a hash of its ask and
/// starting source (FNV-1a, stable across Rust versions). `rescore` never
/// re-judges an answer to an older version, since the checks no longer apply.
fn task_fp(t: &Task) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in t.ask.trim().bytes().chain([0]).chain(t.source.trim().bytes()) {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

fn load_tasks() -> Vec<Task> {
    let text = std::fs::read_to_string(eval_dir().join("tasks.txt")).expect("tasks.txt");
    let mut tasks = Vec::new();
    let mut cur: Option<Task> = None;
    let mut section = String::new();
    for line in text.lines() {
        if let Some(h) = line.strip_prefix("=== ") {
            if let Some(t) = cur.take() {
                tasks.push(t);
            }
            let mut w = h.split_whitespace();
            cur = Some(Task { id: w.next().unwrap().into(), kind: w.next().unwrap_or("?").into(), ask: String::new(), source: String::new(), expect: vec![], touch: vec![] });
            section.clear();
            continue;
        }
        let Some(t) = cur.as_mut() else { continue };
        if let Some(s) = line.strip_prefix("--- ") {
            section = s.trim().to_string();
            continue;
        }
        match section.as_str() {
            "ask" => {
                if !line.trim().is_empty() {
                    if !t.ask.is_empty() {
                        t.ask.push(' ');
                    }
                    t.ask.push_str(line.trim());
                }
            }
            "source" => {
                t.source.push_str(line);
                t.source.push('\n');
            }
            "expect" => {
                if !line.trim().is_empty() {
                    t.expect.push(line.trim().to_string());
                }
            }
            "touch" => {
                t.touch.extend(line.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()));
            }
            _ => {}
        }
    }
    if let Some(t) = cur.take() {
        tasks.push(t);
    }
    // a source ends at the blank line before the next task: trim trailing blank lines
    for t in &mut tasks {
        while t.source.ends_with("\n\n") {
            t.source.pop();
        }
        if t.source.trim().is_empty() {
            t.source.clear();
        }
    }
    tasks
}

// ---------------------------------------------------------------------------
// checks against the parsed figure

fn uni(s: &str) -> String {
    let table = [
        ("φ", "\\varphi"), ("ψ", "\\psi"), ("π", "\\pi"), ("ι", "\\iota"), ("η", "\\eta"), ("α", "\\alpha"), ("β", "\\beta"), ("ε", "\\varepsilon"),
        ("∘", "\\circ"), ("×", "\\times"), ("²", "^2"), ("ℤ", "\\mathbb{Z}"), ("≅", "\\cong"),
    ];
    let mut o = s.to_string();
    for (a, b) in table {
        o = o.replace(a, b);
    }
    o
}

/// Labels and names compare without spaces, $, braces, and after Unicode → LaTeX.
fn norm(s: &str) -> String {
    uni(s).chars().filter(|c| !c.is_whitespace() && !matches!(c, '$' | '{' | '}')).collect()
}

fn alts(v: &str) -> Vec<String> {
    v.split('|').map(norm).collect()
}

struct View<'a> {
    fig: &'a Figure,
}

impl<'a> View<'a> {
    fn arrows(&self) -> Vec<&'a Arrow> {
        self.fig.items.iter().filter_map(|i| if let Item::Arrow(a) = i { Some(a) } else { None }).collect()
    }
    fn blobs(&self) -> Vec<&'a Blob> {
        self.fig.items.iter().filter_map(|i| if let Item::Blob(b) = i { Some(b) } else { None }).collect()
    }
    fn ngons(&self) -> Vec<&'a NGon> {
        self.fig.items.iter().filter_map(|i| if let Item::NGon(b) = i { Some(b) } else { None }).collect()
    }
    fn cayleys(&self) -> Vec<&'a Cayley> {
        self.fig.items.iter().filter_map(|i| if let Item::Cayley(b) = i { Some(b) } else { None }).collect()
    }
    fn element_names(&self) -> Vec<String> {
        self.blobs().iter().flat_map(|b| b.elements.iter().map(|e| norm(&e.name))).collect()
    }
    fn non_node_names(&self) -> HashSet<String> {
        let mut s: HashSet<String> = self.element_names().into_iter().collect();
        s.extend(self.blobs().iter().map(|b| norm(&b.name)));
        s
    }
    /// Nodes: Node items, plus arrow endpoints that aren't blobs, elements or vertices.
    fn nodes(&self) -> BTreeMap<String, Option<&'a Node>> {
        let mut m: BTreeMap<String, Option<&'a Node>> = BTreeMap::new();
        for i in &self.fig.items {
            if let Item::Node(n) = i {
                let e = m.entry(norm(&n.name)).or_insert(None);
                // the placed mention wins over an unplaced one
                if e.is_none() || (e.unwrap().pos.is_none() && n.pos.is_some()) {
                    *e = Some(n);
                }
            }
        }
        let skip = self.non_node_names();
        for a in self.arrows() {
            for end in [&a.from, &a.to] {
                let n = norm(end);
                if !skip.contains(&n) && !n.contains('.') {
                    m.entry(n).or_insert(None);
                }
            }
        }
        m
    }
    fn node_pos(&self, name: &str) -> Option<Pos> {
        self.nodes().get(&norm(name)).copied().flatten().and_then(|n| n.pos)
    }
}

fn opts(words: &[&str]) -> Vec<(String, Option<String>)> {
    words.iter().map(|w| match w.split_once('=') {
        Some((k, v)) => (k.to_string(), Some(v.to_string())),
        None => match w.split_once('~') {
            Some((k, v)) => (format!("{k}~"), Some(v.to_string())),
            None => (w.to_string(), None),
        },
    }).collect()
}

fn num_pair(v: &str) -> (Option<f32>, Option<f32>) {
    let mut it = v.split(',');
    let a = it.next().and_then(|x| x.trim().parse().ok());
    let b = it.next().and_then(|x| x.trim().parse().ok());
    (a, b)
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

fn head_name(h: Head) -> &'static str {
    match h {
        Head::To => "To",
        Head::TwoHeads => "TwoHeads",
        Head::Tail => "Tail",
        Head::Hook => "Hook",
        Head::MapsTo => "MapsTo",
        Head::Both => "Both",
        Head::Equal => "Equal",
        Head::Double => "Double",
    }
}

fn arrow_ok(a: &Arrow, o: &[(String, Option<String>)]) -> bool {
    o.iter().all(|(k, v)| {
        let v = v.as_deref();
        match (k.as_str(), v) {
            ("label", Some("none")) => a.label.as_deref().map(str::trim).unwrap_or("").is_empty(),
            ("label", Some(l)) => a.label.as_deref().map(|x| alts(l).contains(&norm(x))).unwrap_or(false),
            ("head", Some(h)) => h.split('|').any(|h| h == head_name(a.head)),
            ("line", Some(l)) => l.split('|').any(|l| l == format!("{:?}", a.line)),
            // bare `bend`: bent to either side; `bend=left|right|0`: the side
            // matters (left is positive, as the parser reads `bend-left`)
            ("bend", None) => a.bend.abs() > 0.5,
            ("bend", Some(b)) => b.split('|').any(|b| match b {
                "left" => a.bend > 0.5,
                "right" => a.bend < -0.5,
                "0" => a.bend.abs() < 0.5,
                _ => panic!("unknown bend {b}"),
            }),
            ("loop", None) => a.is_loop,
            ("swap", None) => a.side == LabelSide::Swap,
            ("iso", None) => a.iso,
            ("thick", None) => a.thick,
            ("color", Some(c)) => a.color.as_deref() == Some(c),
            ("pos", Some(p)) => format!("{:?}", a.label_pos) == p,
            _ => panic!("unknown arrow option {k}"),
        }
    })
}

/// How many distinct reflection lines an n-gon's `axes` list draws (axis k
/// and axis k + n/2 are the same line when n is even).
fn axis_lines(g: &NGon) -> usize {
    let n = g.n as f64;
    let mut lines: Vec<i64> = g
        .axes
        .iter()
        .map(|&a| {
            let k = a as f64 / 2.0; // vertex k, or edge k.5
            let deg = 90.0 + 360.0 * (k - 1.0) / n;
            ((deg.rem_euclid(180.0)) * 1000.0).round() as i64
        })
        .collect();
    lines.sort();
    lines.dedup();
    lines.len()
}

fn group_norm(s: &str) -> String {
    s.to_lowercase().replace(['_', ' ', '{', '}'], "").replace("\\times", "x").replace('×', "x").replace("c", "z")
}

/// One check: Ok, or what was wrong.
fn check(v: &View, line: &str) -> Result<(), String> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let fail = |why: String| Err(format!("{line}  ← {why}"));
    let (neg, words) = if words[0] == "no" { (true, &words[1..]) } else { (false, &words[..]) };
    match words[0] {
        "node" => {
            let name = words[1];
            let nodes = v.nodes();
            let found = nodes.get(&norm(name));
            if neg {
                return if found.is_some() { fail("still there".into()) } else { Ok(()) };
            }
            let Some(node) = found else { return fail("no such node".into()) };
            for (k, val) in opts(&words[2..]) {
                let val = val.unwrap_or_default();
                let pos = node.and_then(|n| n.pos);
                match k.as_str() {
                    "label" => {
                        let l = node.map(|n| n.label.clone()).unwrap_or_else(|| name.to_string());
                        if !alts(&val).contains(&norm(&l)) {
                            return fail(format!("label is {l}"));
                        }
                    }
                    "cell" | "cell~" => {
                        let (x, y) = num_pair(&val);
                        match pos {
                            Some(Pos::Cell(a, b)) if close(a, x.unwrap()) && close(b, y.unwrap()) => {}
                            None if k == "cell~" => {}
                            other => return fail(format!("position is {other:?}")),
                        }
                    }
                    "color" => {
                        if node.and_then(|n| n.color.as_deref()) != Some(val.as_str()) {
                            return fail("colour".into());
                        }
                    }
                    _ => panic!("unknown node option {k}"),
                }
            }
            Ok(())
        }
        "arrow" => {
            let spec = words[1];
            let (from, to, either) = if let Some((a, b)) = spec.split_once("->") { (a, b, false) } else if let Some((a, b)) = spec.split_once("--") { (a, b, true) } else { panic!("bad arrow spec {spec}") };
            let m = |x: &str, want: &str| want == "*" || norm(x) == norm(want);
            let cands: Vec<&Arrow> = v.arrows().into_iter().filter(|a| (m(&a.from, from) && m(&a.to, to)) || (either && m(&a.from, to) && m(&a.to, from))).collect();
            if neg {
                return if cands.is_empty() { Ok(()) } else { fail("still there".into()) };
            }
            if cands.is_empty() {
                return fail(format!("no arrow (arrows: {})", v.arrows().iter().map(|a| format!("{}->{}", a.from, a.to)).collect::<Vec<_>>().join(" ")));
            }
            let o = opts(&words[2..]);
            if cands.iter().any(|a| arrow_ok(a, &o)) {
                Ok(())
            } else {
                fail(format!("found {:?}", cands.iter().map(|a| (a.label.clone(), head_name(a.head), a.line, a.bend, a.is_loop, a.side, a.iso, a.thick, a.color.clone())).collect::<Vec<_>>()))
            }
        }
        "count" => {
            let spec = words[1];
            let (what, op, n) = if let Some((w, n)) = spec.split_once(">=") { (w, ">=", n) } else { let (w, n) = spec.split_once('=').unwrap(); (w, "=", n) };
            let n: usize = n.parse().unwrap();
            let got = match what {
                "arrows" => v.arrows().len(),
                "nodes" => v.nodes().len(),
                "blobs" => v.blobs().len(),
                "ngons" => v.ngons().len(),
                "cayleys" => v.cayleys().len(),
                "elements" => v.element_names().len(),
                "texts" => v.fig.items.iter().filter(|i| matches!(i, Item::Text(_))).count(),
                "grids" => v.fig.items.iter().filter(|i| matches!(i, Item::Grid { .. })).count(),
                _ => panic!("unknown count {what}"),
            };
            let ok = if op == "=" { got == n } else { got >= n };
            if ok { Ok(()) } else { fail(format!("got {got}")) }
        }
        "blob" => {
            let name = words[1];
            let cands: Vec<&Blob> = v.blobs().into_iter().filter(|b| name == "*" || norm(&b.name) == norm(name)).collect();
            if neg {
                return if cands.is_empty() { Ok(()) } else { fail("still there".into()) };
            }
            let o = opts(&words[2..]);
            let ok = |b: &Blob| {
                let els: Vec<String> = b.elements.iter().map(|e| norm(&e.name)).collect();
                o.iter().all(|(k, val)| {
                    let val = val.as_deref().unwrap_or("");
                    match k.as_str() {
                        "label" => alts(val).contains(&norm(&b.label)),
                        "parent" => b.parent.as_deref().map(norm) == Some(norm(val)),
                        "shaded" => b.shaded,
                        "color" => b.color.as_deref() == Some(val),
                        "shape" => format!("{:?}", b.shape) == val,
                        "place" => format!("{:?}", b.label_place) == val,
                        "has" => val.split(',').all(|e| els.contains(&norm(e))),
                        "lacks" => val.split(',').all(|e| !els.contains(&norm(e))),
                        _ => panic!("unknown blob option {k}"),
                    }
                })
            };
            if cands.iter().any(|b| ok(b)) {
                Ok(())
            } else {
                fail(format!("blobs: {:?}", cands.iter().map(|b| (b.name.clone(), b.label.clone(), b.parent.clone(), b.shaded, b.color.clone(), b.shape, b.label_place, b.elements.iter().map(|e| e.name.clone()).collect::<Vec<_>>())).collect::<Vec<_>>()))
            }
        }
        "ngon" => {
            let name = words[1];
            let cands: Vec<&NGon> = v.ngons().into_iter().filter(|g| name == "*" || g.name == name).collect();
            let o = opts(&words[2..]);
            let enc = |s: &str| -> u32 {
                if let Some(k) = s.strip_suffix(".5") { 2 * k.parse::<u32>().unwrap() + 1 } else { 2 * s.parse::<u32>().unwrap() }
            };
            let ok = |g: &NGon| {
                o.iter().all(|(k, val)| {
                    let val = val.as_deref().unwrap_or("");
                    match k.as_str() {
                        "n" => g.n.to_string() == val,
                        "axes" => g.all_axes || axis_lines(g) >= g.n as usize,
                        "noaxes" => !g.all_axes,
                        "axis" => val.split(',').all(|a| g.axes.contains(&enc(a))),
                        "rotation" => g.rotation.to_string() == val,
                        "turn" => g.turn.to_string() == val,
                        "flip" => g.flip == Some(enc(val)),
                        "labels" => g.labels.iter().map(|l| norm(l)).collect::<Vec<_>>() == val.split(',').map(norm).collect::<Vec<_>>(),
                        _ => panic!("unknown ngon option {k}"),
                    }
                })
            };
            if cands.iter().any(|g| ok(g)) { Ok(()) } else { fail(format!("ngons: {:?}", cands.iter().map(|g| (g.name.clone(), g.n, g.all_axes, g.axes.clone(), g.rotation, g.turn, g.flip, g.labels.clone())).collect::<Vec<_>>())) }
        }
        "cayley" => {
            let name = words[1];
            let cands: Vec<&Cayley> = v.cayleys().into_iter().filter(|c| name == "*" || c.name == name).collect();
            let o = opts(&words[2..]);
            let ok = |c: &Cayley| {
                o.iter().all(|(k, val)| {
                    let val = val.as_deref().unwrap_or("");
                    match k.as_str() {
                        "group" => matches!(&c.group, GroupSpec::Named(g) if group_norm(g) == group_norm(val)),
                        "pres" => matches!(&c.group, GroupSpec::Presentation(..)),
                        "gens" => {
                            let want: Vec<String> = val.split(',').map(norm).collect();
                            let got: Vec<String> = c.gens.iter().map(|g| norm(g)).collect();
                            let pres: Vec<String> = if let GroupSpec::Presentation(g, _) = &c.group { g.iter().map(|g| norm(g)).collect() } else { vec![] };
                            got == want || (got.is_empty() && pres == want)
                        }
                        "ngens" => {
                            let n = if c.gens.is_empty() { if let GroupSpec::Presentation(g, _) = &c.group { g.len() } else { 0 } } else { c.gens.len() };
                            n.to_string() == val
                        }
                        "rels" => matches!(&c.group, GroupSpec::Presentation(_, r) if r.len().to_string() == val),
                        "layout" => format!("{:?}", c.layout) == val,
                        "legend" => c.legend,
                        "nolabels" => !c.labels,
                        _ => panic!("unknown cayley option {k}"),
                    }
                })
            };
            if cands.iter().any(|c| ok(c)) { Ok(()) } else { fail(format!("cayleys: {:?}", cands.iter().map(|c| (c.name.clone(), c.group.clone(), c.gens.clone(), c.layout, c.legend, c.labels)).collect::<Vec<_>>())) }
        }
        "comment" => {
            let want = words[1..].join(" ");
            let found = v.fig.items.iter().any(|i| matches!(i, Item::Comment { text, .. } if text.contains(&want)));
            if found { Ok(()) } else { fail("comment gone".into()) }
        }
        "text" => {
            let want = words[1..].join(" ");
            let found = v.fig.items.iter().any(|i| matches!(i, Item::Text(t) if t.text.contains(&want)));
            if found { Ok(()) } else { fail("no such text".into()) }
        }
        "globals" => {
            let g = &v.fig.globals;
            for (k, val) in opts(&words[1..]) {
                let val = val.unwrap_or_default();
                let ok = match k.as_str() {
                    "spacing" => {
                        let mut it = val.split(',');
                        let (x, y) = (it.next().unwrap(), it.next().unwrap_or("*"));
                        (x == "*" || close(g.spacing.0, x.parse().unwrap())) && (y == "*" || close(g.spacing.1, y.parse().unwrap()))
                    }
                    "size" => g.size == val,
                    "scale" => close(g.scale, val.parse().unwrap()),
                    _ => panic!("unknown globals option {k}"),
                };
                if !ok {
                    return fail(format!("globals are {g:?}"));
                }
            }
            Ok(())
        }
        "apart" => {
            // the arrows between A and B (either direction) curve to different
            // sides of the line A→B, so none sits on top of another. A bend is
            // relative to the arrow's own direction, so B→A bend-left is on the
            // same side as A→B bend-right.
            let (a, b) = (norm(words[1]), norm(words[2]));
            let between: Vec<&Arrow> = v.arrows().into_iter().filter(|x| !x.is_loop && ((norm(&x.from) == a && norm(&x.to) == b) || (norm(&x.from) == b && norm(&x.to) == a))).collect();
            let side = |x: &Arrow| {
                let s = if x.bend > 0.5 { 1 } else if x.bend < -0.5 { -1 } else { 0 };
                if norm(&x.from) == a { s } else { -s }
            };
            let sides: Vec<i32> = between.iter().map(|x| side(x)).collect();
            let distinct: HashSet<i32> = sides.iter().copied().collect();
            if between.len() >= 2 && distinct.len() == sides.len() {
                Ok(())
            } else {
                fail(format!("sides relative to {}->{}: {sides:?}", words[1], words[2]))
            }
        }
        "below" | "right" => {
            let (a, b) = (words[1], words[2]);
            match (v.node_pos(a), v.node_pos(b)) {
                (Some(Pos::Cell(ax, ay)), Some(Pos::Cell(bx, by))) => {
                    let ok = if words[0] == "below" { close(ax, bx) && ay > by } else { close(ay, by) && ax > bx };
                    if ok { Ok(()) } else { fail(format!("{a} at {ax},{ay}, {b} at {bx},{by}")) }
                }
                (pa, pb) => fail(format!("positions {pa:?} {pb:?}")),
            }
        }
        other => panic!("unknown check {other}"),
    }
}

// ---------------------------------------------------------------------------
// preservation: the statements the instruction doesn't touch come back
// unchanged and in the same order

fn strip_spans(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.remove("span");
            for (_, x) in m.iter_mut() {
                strip_spans(x);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(strip_spans),
        _ => {}
    }
}

fn norm_labels(v: &mut Value) {
    match v {
        Value::Object(m) => {
            for (k, x) in m.iter_mut() {
                if k == "label" || k == "labels" {
                    match x {
                        Value::String(s) => *s = norm(s),
                        Value::Array(a) => a.iter_mut().for_each(|l| if let Value::String(s) = l { *s = norm(s) }),
                        _ => {}
                    }
                } else {
                    norm_labels(x);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(norm_labels),
        _ => {}
    }
}

fn item_key(i: &Item) -> Option<(String, Value)> {
    let tag = match i {
        Item::Node(n) => format!("node {}", n.name),
        Item::Arrow(a) => format!("arrow {}->{}", a.from, a.to),
        Item::Blob(b) => format!("blob {}", b.name),
        Item::NGon(g) => format!("ngon {}", g.name),
        Item::Cayley(c) => format!("cayley {}", c.name),
        Item::Text(t) => format!("text {}", t.text),
        Item::Comment { text, .. } => {
            if text.trim().trim_start_matches('%').trim().is_empty() {
                return None;
            }
            format!("comment {}", text.trim())
        }
        Item::Grid { .. } => return None,
    };
    let mut v = serde_json::to_value(i).unwrap();
    strip_spans(&mut v);
    // comments compare by their text only (trailing spaces, % spacing aside)
    if let Item::Comment { text, .. } = i {
        v = json!(text.trim().trim_start_matches('%').trim());
    }
    // labels compare like the checks do: LaTeX spacing and braces aside
    norm_labels(&mut v);
    Some((tag, v))
}

fn touched(i: &Item, touch: &[String]) -> bool {
    if touch.iter().any(|t| t == "*") {
        return true;
    }
    let has = |n: &str| touch.iter().any(|t| norm(t) == norm(n));
    match i {
        Item::Node(n) => has(&n.name),
        Item::Arrow(a) => touch.iter().any(|t| {
            t.split_once("->").map(|(x, y)| (norm(x) == norm(&a.from) && norm(y) == norm(&a.to)) || (norm(x) == norm(&a.to) && norm(y) == norm(&a.from))).unwrap_or(false)
        }),
        Item::Blob(b) => has(&b.name),
        Item::NGon(g) => has(&g.name),
        Item::Cayley(c) => has(&c.name),
        Item::Text(_) => has("text"),
        Item::Comment { .. } => false,
        Item::Grid { .. } => false,
    }
}

/// (all preserved, what wasn't)
fn preserved(before: &Figure, after: &Figure, touch: &[String]) -> (bool, Vec<String>) {
    let after_keys: Vec<Option<(String, Value)>> = after.items.iter().map(item_key).collect();
    let mut used = vec![false; after.items.len()];
    let mut last = 0usize;
    let mut problems = Vec::new();
    let mut in_order = true;
    for i in &before.items {
        if touched(i, touch) {
            continue;
        }
        let Some((tag, val)) = item_key(i) else { continue };
        // the first unused equal item; prefer one after the previous match
        let pos = (last..after_keys.len()).chain(0..last).find(|&j| !used[j] && after_keys[j].as_ref().map(|(_, v)| v == &val).unwrap_or(false));
        match pos {
            Some(j) => {
                used[j] = true;
                if j < last {
                    in_order = false;
                }
                last = j;
            }
            None => problems.push(format!("changed or gone: {tag}")),
        }
    }
    if !in_order {
        problems.push("order changed".into());
    }
    (problems.is_empty(), problems)
}

// ---------------------------------------------------------------------------
// running

fn parse_safe(src: &str) -> Option<(Figure, Vec<Diagnostic>)> {
    std::panic::catch_unwind(|| ntex_figures::parse::parse(src)).ok()
}

fn errors_of(d: &[Diagnostic]) -> Vec<String> {
    d.iter().filter(|d| d.severity == Severity::Error).map(|d| d.message.clone()).collect()
}

fn evaluate(task: &Task, answer: &str) -> Value {
    let Some((after, diags)) = parse_safe(answer) else { return json!({"parse_clean": Value::Null, "note": "parser unavailable"}) };
    let errs = errors_of(&diags);
    let view = View { fig: &after };
    let failed: Vec<String> = task.expect.iter().filter_map(|e| check(&view, e).err()).collect();
    let (before, _) = parse_safe(&task.source).unwrap();
    let (pres, problems) = preserved(&before, &after, &task.touch);
    json!({
        "parse_clean": errs.is_empty(),
        "parse_errors": errs,
        "expect_ok": failed.is_empty(),
        "expect_failed": failed,
        "expect_total": task.expect.len(),
        "preserved": pres,
        "preserve_problems": problems,
    })
}

fn run_one(task: &Task, cfg: &Config) -> Value {
    let model = AskModel { backend: cfg.backend.into(), model: cfg.model.into(), effort: cfg.effort.into() };
    let t0 = Instant::now();
    let (job, rx) = match AskJob::start(&task.source, &task.ask, &model) {
        Ok(x) => x,
        Err(e) => return json!({"outcome": "failed", "kind": "process", "reason": e.to_string(), "seconds": 0.0, "calls": 0}),
    };
    let mut retried = false;
    let mut first_errors = String::new();
    let result = loop {
        match rx.recv_timeout(figure_ai::TIMEOUT + Duration::from_secs(15)) {
            Ok(AskEvent::Progress(p)) => {
                if p.starts_with(figure_ai::RETRY_PREFIX) {
                    retried = true;
                    first_errors = p;
                }
            }
            Ok(AskEvent::Done { source, summary, seconds }) => break json!({"outcome": "done", "source": source, "summary": summary, "seconds": seconds}),
            Ok(AskEvent::Failed(why)) => {
                let kind = if why.starts_with("No answer in") { "timeout" } else if why.starts_with("The answer still had errors") { "parse" } else { "process" };
                break json!({"outcome": "failed", "kind": kind, "reason": why, "seconds": t0.elapsed().as_secs_f32()});
            }
            Err(_) => break json!({"outcome": "failed", "kind": "harness", "reason": "no event", "seconds": t0.elapsed().as_secs_f32()}),
        }
    };
    drop(job);
    let mut r = result;
    r["retried"] = json!(retried);
    r["first_errors"] = json!(first_errors);
    r["calls"] = json!(if retried { 2 } else { 1 });
    if r["outcome"] == "done" {
        let ev = evaluate(task, r["source"].as_str().unwrap());
        for (k, v) in ev.as_object().unwrap() {
            r[k] = v.clone();
        }
    }
    r
}

fn results_dir() -> PathBuf {
    std::env::var("EVAL_RESULTS").map(PathBuf::from).unwrap_or_else(|_| eval_dir().join("results"))
}

fn results_path(cfg: &str) -> PathBuf {
    results_dir().join(format!("{cfg}.jsonl"))
}

fn read_results(cfg: &str) -> Vec<Value> {
    std::fs::read_to_string(results_path(cfg)).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// Re-evaluate every stored answer with the current checks (no model calls).
fn cmd_rescore() {
    let tasks = load_tasks();
    for cfg in CONFIGS {
        let rs = read_results(cfg.name);
        if rs.is_empty() {
            continue;
        }
        let mut out = String::new();
        let mut changed = 0;
        let mut stale = 0;
        for mut r in rs {
            let t = tasks.iter().find(|t| t.id == r["task"].as_str().unwrap_or(""));
            // answers to a task that has since changed (or gone) keep their verdict;
            // an answer with no fingerprint is taken to be for the current version
            let current = t.is_some_and(|t| r["task_fp"].as_str().is_none_or(|fp| fp == task_fp(t)));
            if !current {
                stale += 1;
            }
            if let Some(t) = t.filter(|_| current && r["outcome"] == "done") {
                r["task_fp"] = json!(task_fp(t));
                let before = (r["expect_ok"].clone(), r["preserved"].clone());
                let ev = evaluate(t, r["source"].as_str().unwrap());
                for (k, v) in ev.as_object().unwrap() {
                    r[k] = v.clone();
                }
                if before != (r["expect_ok"].clone(), r["preserved"].clone()) {
                    changed += 1;
                }
            }
            out.push_str(&serde_json::to_string(&r).unwrap());
            out.push('\n');
        }
        std::fs::write(results_path(cfg.name), out).unwrap();
        println!("{}: {} verdicts changed{}", cfg.name, changed, if stale > 0 { format!(", {stale} answers to an older version of a task kept as scored") } else { String::new() });
    }
}

fn cmd_run(args: &[String]) {
    let cfg_name = args.first().expect("run <config> [runs]");
    let cfg = CONFIGS.iter().find(|c| c.name == cfg_name).unwrap_or_else(|| panic!("unknown config {cfg_name}"));
    let runs: usize = args.get(1).and_then(|r| r.parse().ok()).unwrap_or(3);
    let mut only: Option<Vec<String>> = None;
    let mut jobs = 3usize;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--tasks" => {
                only = Some(args[i + 1].split(',').map(str::to_string).collect());
                i += 2;
            }
            "--jobs" => {
                jobs = args[i + 1].parse().unwrap();
                i += 2;
            }
            _ => i += 1,
        }
    }
    if cfg.backend == "codex" {
        // SAFETY: set once, before any thread starts
        unsafe { std::env::set_var("NTEX_STUDIO_CODEX_TIER", cfg.tier) };
    }
    let tasks: Vec<Task> = load_tasks().into_iter().filter(|t| only.as_ref().map(|o| o.contains(&t.id)).unwrap_or(true)).collect();
    let done: HashSet<(String, u64)> = read_results(cfg.name).iter().map(|r| (r["task"].as_str().unwrap_or("").to_string(), r["run"].as_u64().unwrap_or(0))).collect();
    // run-major order: every task once, then again, so a stopped eval is balanced
    let mut queue: Vec<(Task, usize)> = Vec::new();
    for run in 1..=runs {
        for t in &tasks {
            if !done.contains(&(t.id.clone(), run as u64)) {
                queue.push((t.clone(), run));
            }
        }
    }
    let total = queue.len();
    eprintln!("{}: {} asks to run ({} tasks × {} runs, {} already done)", cfg.name, total, tasks.len(), runs, done.len());
    let queue = Arc::new(Mutex::new(queue.into_iter().rev().collect::<Vec<_>>()));
    let out = Arc::new(Mutex::new(std::fs::OpenOptions::new().create(true).append(true).open(results_path(cfg.name)).unwrap()));
    let counter = Arc::new(Mutex::new(0usize));
    std::thread::scope(|s| {
        for _ in 0..jobs {
            let (queue, out, counter) = (queue.clone(), out.clone(), counter.clone());
            s.spawn(move || {
                loop {
                    let Some((task, run)) = queue.lock().unwrap().pop() else { break };
                    let mut r = run_one(&task, cfg);
                    r["config"] = json!(cfg.name);
                    r["task"] = json!(task.id);
                    r["kind_of_task"] = json!(task.kind);
                    r["task_fp"] = json!(task_fp(&task));
                    r["run"] = json!(run);
                    r["prompt_version"] = json!(figure_ai::PROMPT_VERSION);
                    let line = serde_json::to_string(&r).unwrap();
                    let mut f = out.lock().unwrap();
                    writeln!(f, "{line}").unwrap();
                    f.flush().unwrap();
                    drop(f);
                    let mut c = counter.lock().unwrap();
                    *c += 1;
                    let status = match (r["outcome"].as_str(), r["expect_ok"].as_bool(), r["preserved"].as_bool()) {
                        (Some("done"), Some(true), Some(true)) => "ok".to_string(),
                        (Some("done"), e, p) => format!("done expect={e:?} preserved={p:?}"),
                        _ => format!("FAILED {}", r["reason"].as_str().unwrap_or("")),
                    };
                    eprintln!("[{}/{}] {} run {} {:.1}s{} {}", *c, total, task.id, run, r["seconds"].as_f64().unwrap_or(0.0), if r["retried"] == true { " (retried)" } else { "" }, status);
                }
            });
        }
    });
}

fn pct(n: usize, d: usize) -> String {
    if d == 0 { "–".into() } else { format!("{:.0}%", 100.0 * n as f64 / d as f64) }
}

fn quantile(v: &mut [f64], q: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let i = ((v.len() as f64 - 1.0) * q).round() as usize;
    v[i]
}

fn cmd_report() {
    let mut table = String::new();
    table.push_str("| config | asks | model calls | parse-clean | first-try clean | expectations met | checks passed | preserved | all good | tasks good in every run | median s | p90 s | failures |\n");
    table.push_str("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|\n");
    let mut by_kind = String::from("\n| config | cd | blob | ngon | cayley | tidy |\n|---|---:|---:|---:|---:|---:|\n");
    let mut misses = String::new();
    for cfg in CONFIGS {
        let rs = read_results(cfg.name);
        if rs.is_empty() {
            continue;
        }
        let n = rs.len();
        let calls: u64 = rs.iter().map(|r| r["calls"].as_u64().unwrap_or(1)).sum();
        let clean = rs.iter().filter(|r| r["parse_clean"] == true).count();
        let first = rs.iter().filter(|r| r["parse_clean"] == true && r["retried"] != true).count();
        let exp = rs.iter().filter(|r| r["expect_ok"] == true).count();
        let (checks_ok, checks_all) = rs.iter().fold((0usize, 0usize), |(a, b), r| {
            let total = r["expect_total"].as_u64().unwrap_or(0) as usize;
            let failed = r["expect_failed"].as_array().map(|x| x.len()).unwrap_or(total);
            (a + total.saturating_sub(failed), b + total)
        });
        let pres = rs.iter().filter(|r| r["preserved"] == true).count();
        let good = |r: &Value| r["parse_clean"] == true && r["expect_ok"] == true && r["preserved"] == true;
        let all = rs.iter().filter(|r| good(r)).count();
        let mut per_task: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        for r in &rs {
            let e = per_task.entry(r["task"].as_str().unwrap().to_string()).or_default();
            e.1 += 1;
            if good(r) {
                e.0 += 1;
            }
        }
        let tasks_all = per_task.values().filter(|(g, t)| g == t).count();
        let mut secs: Vec<f64> = rs.iter().filter_map(|r| r["seconds"].as_f64()).collect();
        let (med, p90) = (quantile(&mut secs.clone(), 0.5), quantile(&mut secs, 0.9));
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        for r in &rs {
            let k = if r["outcome"] != "done" {
                r["kind"].as_str().unwrap_or("?").to_string()
            } else if r["parse_clean"] != true {
                "parse".into()
            } else if r["expect_ok"] != true && r["preserved"] != true {
                "wrong+unpreserved".into()
            } else if r["expect_ok"] != true {
                "wrong".into()
            } else if r["preserved"] != true {
                "unpreserved".into()
            } else {
                continue;
            };
            *kinds.entry(k).or_default() += 1;
        }
        let fk = kinds.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ");
        table.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {}/{} | {:.1} | {:.1} | {} |\n",
            cfg.name, n, calls, pct(clean, n), pct(first, n), pct(exp, n), pct(checks_ok, checks_all), pct(pres, n), pct(all, n), tasks_all, per_task.len(), med, p90, if fk.is_empty() { "–".into() } else { fk }
        ));
        let kind_rate = |k: &str| {
            let sel: Vec<&Value> = rs.iter().filter(|r| r["kind_of_task"] == k).collect();
            pct(sel.iter().filter(|r| good(r)).count(), sel.len())
        };
        by_kind.push_str(&format!("| {} | {} | {} | {} | {} | {} |\n", cfg.name, kind_rate("cd"), kind_rate("blob"), kind_rate("ngon"), kind_rate("cayley"), kind_rate("tidy")));
        let bad: Vec<String> = per_task.iter().filter(|(_, (g, t))| g < t).map(|(k, (g, t))| format!("{k} {g}/{t}")).collect();
        misses.push_str(&format!("- **{}**: {}\n", cfg.name, if bad.is_empty() { "none".into() } else { bad.join(", ") }));
    }
    let doc = format!("{table}\nAll good, by task kind:\n{by_kind}\nTasks not good in every run (good/runs):\n\n{misses}");
    std::fs::write(results_dir().join("table.md"), &doc).unwrap();
    println!("{doc}");
}

/// The tasks themselves: every starting source parses, every check runs,
/// and the starting source does NOT already pass all its checks.
fn cmd_check() {
    let tasks = load_tasks();
    let mut bad = 0;
    let mut ids = HashSet::new();
    for t in &tasks {
        assert!(ids.insert(t.id.clone()), "duplicate id {}", t.id);
        let Some((fig, diags)) = parse_safe(&t.source) else {
            println!("{}: parser unavailable", t.id);
            bad += 1;
            continue;
        };
        let errs = errors_of(&diags);
        if !errs.is_empty() {
            println!("{}: the source has errors: {errs:?}", t.id);
            bad += 1;
        }
        let view = View { fig: &fig };
        let failing: Vec<String> = t.expect.iter().filter_map(|e| check(&view, e).err()).collect();
        let (pres, problems) = preserved(&fig, &fig, &t.touch);
        // (a tidy-up may already mean the right thing: its test is that it still does)
        if failing.is_empty() && t.kind != "tidy" {
            println!("{}: the source already meets every expectation", t.id);
            bad += 1;
        }
        // the hand-written reference answer must pass everything
        match std::fs::read_to_string(eval_dir().join("reference").join(format!("{}.fig", t.id))) {
            Ok(r) => {
                let e = evaluate(t, &r);
                if !(e["parse_clean"] == true && e["expect_ok"] == true && e["preserved"] == true) {
                    println!("{}: the reference answer fails: {e}", t.id);
                    bad += 1;
                }
            }
            Err(_) => println!("{}: no reference answer", t.id),
        }
        if !pres {
            println!("{}: the source doesn't preserve itself: {problems:?}", t.id);
            bad += 1;
        }
        println!("{:<20} {:<7} {} checks, {} fail on the source", t.id, t.kind, t.expect.len(), failing.len());
    }
    println!("{} tasks, {} problems", tasks.len(), bad);
}

/// Check a hand-written answer for a task (to test the checks): `try <task> <file>`.
fn cmd_try(args: &[String]) {
    let tasks = load_tasks();
    let t = tasks.iter().find(|t| t.id == args[0]).expect("task");
    let answer = std::fs::read_to_string(&args[1]).unwrap();
    println!("{}", serde_json::to_string_pretty(&evaluate(t, &answer)).unwrap());
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => cmd_run(&args[1..]),
        Some("report") => cmd_report(),
        Some("rescore") => cmd_rescore(),
        Some("check") => cmd_check(),
        Some("try") => cmd_try(&args[1..]),
        Some("prompt") => println!("{}", figure_ai::system_prompt()),
        Some("configs") => {
            for c in CONFIGS {
                println!("{:<22} {} {} {} {}", c.name, c.backend, c.model, c.effort, c.tier);
            }
        }
        _ => eprintln!("usage: ntex-figures-eval check | run <config> [runs] [--tasks a,b] [--jobs n] | report | try <task> <answer.fig> | prompt | configs"),
    }
}
