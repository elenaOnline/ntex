//! ADVERSARY: layout / TikZ / Cayley / build against verification/expected.json
//! and acceptance.md's invariants. Temp copies only: templates/tex is copied
//! into a temp dir, PDFs are built in temp dirs, rasters (CoreGraphics, no
//! window) land in verification/out/.
//!
//!     cargo test -p ntex-figures --test adversary_build -- --nocapture --test-threads=1

#[path = "adversary_common/mod.rs"]
mod common;

use common::*;
use ntex_figures::model::*;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::PathBuf;
use std::process::Command;

// ---------------------------------------------------------------- helpers

fn cases() -> Vec<Value> { load_expected()["cases"].as_array().unwrap().clone() }

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ntex-fig-adversary-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A temp copy of her real templates/tex (never written).
fn templates_copy() -> PathBuf {
    let real = PathBuf::from(std::env::var("HOME").unwrap()).join(".config/texnvim/templates/tex");
    let d = tmp("templates");
    for e in std::fs::read_dir(&real).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_file() {
            std::fs::copy(e.path(), d.join(e.file_name())).unwrap();
        }
    }
    d
}

fn parse_layout(src: &str) -> Result<(Figure, Vec<Diagnostic>, Scene), String> {
    let (fig, d) = catch(|| ntex_figures::parse::parse(src)).map_err(|e| format!("parse panicked: {e}"))?;
    let scene = catch(|| ntex_figures::layout::layout(&fig)).map_err(|e| format!("layout panicked: {e}"))?;
    Ok((fig, d, scene))
}

fn report(name: &str, fails: &[Failure]) {
    print_failures(name, fails);
    write_out(&format!("{}.json", name.to_lowercase()), &failures_json(fails));
}

fn finish(name: &str, fails: Vec<Failure>, total: usize) {
    let hard = fails.iter().filter(|f| f.tier != "gap" && !f.check.starts_with("soft:")).count();
    report(name, &fails);
    println!("\n{name}: {} findings ({hard} hard) over {total} cases", fails.len());
    assert!(hard == 0, "{name}: {hard} hard failures");
}

fn fail(fs: &mut Vec<Failure>, c: &Value, check: impl Into<String>, e: Value, a: Value) {
    fs.push(Failure { id: c["id"].as_str().unwrap().into(), tier: c["tier"].as_str().unwrap().into(),
                      check: check.into(), src: c["src"].as_str().unwrap().into(), expected: e, actual: a });
}

fn pt(p: Pt) -> Value { json!([round(p.x), round(p.y)]) }
fn round(x: f32) -> f64 { (x as f64 * 1000.0).round() / 1000.0 }

fn dist_to_rect(p: Pt, r: &Rect) -> f32 {
    let dx = (r.min.x - p.x).max(0.0).max(p.x - r.max.x);
    let dy = (r.min.y - p.y).max(0.0).max(p.y - r.max.y);
    (dx * dx + dy * dy).sqrt()
}
fn inside_inset(p: Pt, r: &Rect, inset: f32) -> bool {
    p.x > r.min.x + inset && p.x < r.max.x - inset && p.y > r.min.y + inset && p.y < r.max.y - inset
}

fn path_points(start: Pt, segs: &[Seg]) -> Vec<Pt> {
    let mut v = vec![start];
    let mut cur = start;
    for s in segs {
        match *s {
            Seg::Line(p) => { v.push(p); cur = p; }
            Seg::Cubic(c1, c2, e) => {
                for t in [0.25f32, 0.5, 0.75] {
                    let u = 1.0 - t;
                    v.push(Pt { x: u*u*u*cur.x + 3.0*u*u*t*c1.x + 3.0*u*t*t*c2.x + t*t*t*e.x,
                                y: u*u*u*cur.y + 3.0*u*u*t*c1.y + 3.0*u*t*t*c2.y + t*t*t*e.y });
                }
                v.push(e);
                cur = e;
            }
        }
    }
    v
}
fn path_end(start: Pt, segs: &[Seg]) -> Pt {
    match segs.last() { Some(Seg::Line(p)) => *p, Some(Seg::Cubic(_, _, e)) => *e, None => start }
}
fn bbox(ps: &[Pt]) -> Rect {
    let mut r = Rect { min: Pt { x: f32::MAX, y: f32::MAX }, max: Pt { x: f32::MIN, y: f32::MIN } };
    for p in ps { r.min.x = r.min.x.min(p.x); r.min.y = r.min.y.min(p.y); r.max.x = r.max.x.max(p.x); r.max.y = r.max.y.max(p.y); }
    r
}

/// The box an arrow end should meet.
fn end_box(name: &str, fig: &Figure, scene: &Scene) -> Option<Rect> {
    let refs: Vec<ObjRef> = {
        let mut v = vec![ObjRef::Node(name.into()), ObjRef::Element(name.into())];
        if let Some((a, b)) = name.split_once('.') {
            if let Ok(k) = b.parse::<u32>() { v.push(ObjRef::NGonVertex(a.into(), k)); }
            v.push(ObjRef::CayleyVertex(a.into(), b.into()));
        }
        v
    };
    for (o, _, r) in &scene.anchors {
        if refs.contains(o) { return Some(r.clone()); }
        if let ObjRef::Node(n) | ObjRef::Element(n) = o { if tex_eq(n, name) { return Some(r.clone()); } }
    }
    // a blob: its outline's bounding box
    let is_blob = fig.items.iter().any(|it| matches!(it, Item::Blob(b) if b.name == name));
    if is_blob {
        for d in &scene.draws {
            if let Draw::Path { obj: ObjRef::Blob(n), start, segs, closed: true, .. } = d {
                if n == name { return Some(bbox(&path_points(*start, segs))); }
            }
        }
    }
    None
}

/// a blob's outline, densely sampled (for blob-to-blob arrows)
fn blob_outline(name: &str, fig: &Figure, scene: &Scene) -> Option<Vec<Pt>> {
    if !fig.items.iter().any(|it| matches!(it, Item::Blob(b) if b.name == name)) { return None; }
    scene.draws.iter().find_map(|d| match d {
        Draw::Path { obj: ObjRef::Blob(n), start, segs, closed: true, .. } if n == name => {
            let mut v = vec![*start];
            let mut cur = *start;
            for sg in segs {
                match *sg {
                    Seg::Line(q) => { for k in 1..=20 { let t = k as f32 / 20.0; v.push(Pt { x: cur.x + (q.x - cur.x) * t, y: cur.y + (q.y - cur.y) * t }); } cur = q; }
                    Seg::Cubic(c1, c2, e) => { for k in 1..=20 { let t = k as f32 / 20.0; let u = 1.0 - t;
                        v.push(Pt { x: u*u*u*cur.x + 3.0*u*u*t*c1.x + 3.0*u*t*t*c2.x + t*t*t*e.x, y: u*u*u*cur.y + 3.0*u*u*t*c1.y + 3.0*u*t*t*c2.y + t*t*t*e.y }); } cur = e; }
                }
            }
            Some(v)
        }
        _ => None })
}

/// my estimate of a label's box (deliberately small)
fn glyphs(latex: &str) -> f32 {
    let toks = tex_tokens(latex);
    let mut n: f32 = 0.0;
    let mut script = false;
    for t in &toks {
        match t.as_str() {
            "{" | "}" => continue,
            "^" | "_" => { script = true; continue; }
            "\\mathbb" | "\\mathrm" | "\\operatorname" | "\\bar" | "\\tilde" | "\\hat" | "\\left" | "\\right" | "\\," | "\\;" => continue,
            _ => {}
        }
        n += if script { 0.7 } else { 1.0 };
        script = false;
    }
    n.max(1.0)
}
fn label_box(at: Pt, latex: &str, size: f32) -> Rect {
    let f = if size > 3.0 { size / 10.0 } else { 1.0 };
    let w = 0.17 * glyphs(latex) * f;
    let h = 0.30 * f;
    Rect { min: Pt { x: at.x - w / 2.0, y: at.y - h / 2.0 }, max: Pt { x: at.x + w / 2.0, y: at.y + h / 2.0 } }
}
fn overlap(a: &Rect, b: &Rect) -> f32 {
    let w = (a.max.x.min(b.max.x) - a.min.x.max(b.min.x)).max(0.0);
    let h = (a.max.y.min(b.max.y) - a.min.y.max(b.min.y)).max(0.0);
    w * h
}

// ---------------------------------------------------------------- 1. Scene invariants

#[test]
fn scene_invariants() {
    let cs = cases();
    let mut fs = Vec::new();
    for c in &cs {
        let src = c["src"].as_str().unwrap();
        let (fig, _d, scene) = match parse_layout(src) {
            Ok(x) => x,
            Err(e) => { fail(&mut fs, c, "parse+layout", json!("no panic"), json!(e)); continue; }
        };
        // arrows meet their ends' boxes
        for d in &scene.draws {
            let Draw::Path { obj: ObjRef::Arrow(i), start, segs, .. } = d else { continue };
            let Some(Item::Arrow(a)) = fig.items.get(*i) else {
                fail(&mut fs, c, "arrow path refers to an arrow item", json!(i), json!(format!("{:?}", fig.items.get(*i))));
                continue;
            };
            let end = path_end(*start, segs);
            for (which, name, p) in [("start", &a.from, *start), ("end", &a.to, end)] {
                if let Some(outl) = blob_outline(name, &fig, &scene) {
                    let dd = outl.iter().map(|q| ((q.x - p.x).powi(2) + (q.y - p.y).powi(2)).sqrt()).fold(f32::MAX, f32::min);
                    if dd > 0.2 { fail(&mut fs, c, format!("arrow {} → {} {which} on blob `{name}`'s outline", a.from, a.to), json!({"within_cm": 0.2}), json!({"point": pt(p), "distance_to_outline": round(dd)})); }
                    continue;
                }
                match end_box(name, &fig, &scene) {
                    None => fail(&mut fs, c, format!("soft: arrow {} → {}: no anchor for `{name}`", a.from, a.to), json!("an anchor"), json!(scene.anchors.iter().map(|x| format!("{:?}", x.0)).collect::<Vec<_>>())),
                    Some(r) => {
                        let dd = dist_to_rect(p, &r);
                        if dd > 0.15 || (!a.is_loop && inside_inset(p, &r, 0.08)) {
                            fail(&mut fs, c, format!("arrow {} → {} {which} at `{name}`'s boundary", a.from, a.to),
                                 json!({"box": [pt(r.min), pt(r.max)], "within_cm": 0.15}), json!({"point": pt(p), "distance": round(dd), "inside": inside_inset(p, &r, 0.08)}));
                        }
                    }
                }
            }
        }
        // nothing outside bounds
        let b = &scene.bounds;
        let tol = 0.05;
        let out = |p: Pt| p.x < b.min.x - tol || p.x > b.max.x + tol || p.y < b.min.y - tol || p.y > b.max.y + tol;
        let mut outside = Vec::new();
        for d in &scene.draws {
            match d {
                Draw::Path { obj, start, segs, .. } => {
                    for p in path_points(*start, segs) { if out(p) { outside.push(json!([format!("{obj:?}"), pt(p)])); break; } }
                }
                Draw::Label { obj, at, latex, .. } => { if out(*at) { outside.push(json!([format!("{obj:?} {latex}"), pt(*at)])); } }
                Draw::Dot { obj, at, r, .. } => {
                    for p in [Pt { x: at.x - r, y: at.y - r }, Pt { x: at.x + r, y: at.y + r }] {
                        if out(p) { outside.push(json!([format!("{obj:?}"), pt(*at)])); break; }
                    }
                }
            }
        }
        if !outside.is_empty() {
            fail(&mut fs, c, "everything inside scene.bounds", json!([pt(b.min), pt(b.max)]), json!(outside));
        }
        // labels don't overlap
        let labels: Vec<(&ObjRef, Pt, &String, f32)> = scene.draws.iter().filter_map(|d| match d {
            Draw::Label { obj, at, latex, size, .. } => Some((obj, *at, latex, *size)), _ => None }).collect();
        let mut hits = Vec::new();
        for i in 0..labels.len() {
            for j in i + 1..labels.len() {
                if labels[i].0 == labels[j].0 { continue; }
                let (a, b) = (label_box(labels[i].1, labels[i].2, labels[i].3), label_box(labels[j].1, labels[j].2, labels[j].3));
                if overlap(&a, &b) > 0.004 {
                    hits.push(json!([labels[i].2, pt(labels[i].1), labels[j].2, pt(labels[j].1)]));
                }
            }
        }
        if !hits.is_empty() {
            fail(&mut fs, c, "labels don't overlap", json!("no overlapping label boxes"), json!(hits));
        }
        // diagnostics never become figure content
        let leaked: Vec<String> = scene.draws.iter().filter_map(|d| match d { Draw::Label { latex, .. } if latex.contains("coset enumeration") || latex.contains("Cayley graph:") || latex.contains("didn't") || latex.contains("line ") && latex.contains(':') => Some(latex.clone()), _ => None }).collect();
        if !leaked.is_empty() { fail(&mut fs, c, "an error message is reported, not typeset into the figure", json!("no message text in the Scene"), json!(leaked)); }
        // palette colours only, in the Scene
        let pal: Vec<&str> = "blue violet teal maroon green ochre grey ink".split(' ').collect();
        let mut bad = Vec::new();
        for d in &scene.draws {
            let cols: Vec<&String> = match d {
                Draw::Label { color, .. } | Draw::Dot { color, .. } => vec![color],
                Draw::Path { stroke, fill, .. } => stroke.iter().map(|s| &s.color).chain(fill.iter()).collect(),
            };
            for col in cols {
                let base = col.split('!').next().unwrap();
                let base = base.strip_suffix("-tint").unwrap_or(base);
                if !pal.contains(&base) && base != "white" && base != "paper" { bad.push(col.clone()); }
            }
        }
        bad.sort(); bad.dedup();
        if !bad.is_empty() { fail(&mut fs, c, "scene colours are palette names", json!(pal), json!(bad)); }
    }
    finish("SCENE", fs, cs.len());
}

// ---------------------------------------------------------------- 2. format round trip

fn strip_spans(v: &mut Value) {
    match v {
        Value::Object(m) => { m.remove("span"); for (_, x) in m.iter_mut() { strip_spans(x); } }
        Value::Array(a) => for x in a { strip_spans(x) },
        _ => {}
    }
}

#[test]
fn format_round_trip() {
    let cs = cases();
    let mut fs = Vec::new();
    for c in &cs {
        let src = c["src"].as_str().unwrap();
        let r = catch(|| {
            let f1 = ntex_figures::parse::parse(src).0;
            let s2 = ntex_figures::format::format(&f1);
            let f2 = ntex_figures::parse::parse(&s2).0;
            let s3 = ntex_figures::format::format(&f2);
            (f1, s2, f2, s3)
        });
        let Ok((f1, s2, f2, s3)) = r else { fail(&mut fs, c, "format round trip", json!("no panic"), json!(r.err())); continue };
        let (mut a, mut b) = (serde_json::to_value(&f1).unwrap(), serde_json::to_value(&f2).unwrap());
        strip_spans(&mut a);
        strip_spans(&mut b);
        if a != b {
            fail(&mut fs, c, "parse(format(parse(s))) == parse(s)", json!({"formatted": s2}), json!({"first_difference": first_diff(&a, &b, "")}));
        }
        if s2 != s3 {
            fail(&mut fs, c, "format is idempotent", json!(s2), json!(s3));
        }
    }
    finish("ROUNDTRIP", fs, cs.len());
}

fn first_diff(a: &Value, b: &Value, path: &str) -> Value {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, v) in x { if y.get(k) != Some(v) { return first_diff(v, y.get(k).unwrap_or(&Value::Null), &format!("{path}.{k}")); } }
            for k in y.keys() { if !x.contains_key(k) { return json!({"at": format!("{path}.{k}"), "before": null, "after": y[k]}); } }
            json!(null)
        }
        (Value::Array(x), Value::Array(y)) => {
            for i in 0..x.len().max(y.len()) {
                let (p, q) = (x.get(i).unwrap_or(&Value::Null), y.get(i).unwrap_or(&Value::Null));
                if p != q { return first_diff(p, q, &format!("{path}[{i}]")); }
            }
            json!(null)
        }
        _ => json!({"at": path, "before": a, "after": b}),
    }
}

// ---------------------------------------------------------------- 3. Cayley groups

/// my word evaluator: right multiplication in the implementation's tables
fn eval_word(label: &str, gens: &[(String, Vec<usize>)]) -> Option<(usize, usize)> {
    let s: Vec<char> = label.chars().filter(|c| !c.is_whitespace()).collect();
    let mut pos = 0usize;
    fn exponent(s: &[char], pos: &mut usize) -> Option<i64> {
        if *pos < s.len() && s[*pos] == '^' {
            *pos += 1;
            let brace = *pos < s.len() && s[*pos] == '{';
            if brace { *pos += 1; }
            let st = *pos;
            if *pos < s.len() && (s[*pos] == '-' || s[*pos] == '+') { *pos += 1; }
            while *pos < s.len() && s[*pos].is_ascii_digit() { *pos += 1; }
            let k: i64 = s[st..*pos].iter().collect::<String>().parse().ok()?;
            if brace { if *pos < s.len() && s[*pos] == '}' { *pos += 1; } else { return None; } }
            Some(k)
        } else { Some(1) }
    }
    // returns the word as (gen, sign) letters
    fn seq(s: &[char], pos: &mut usize, gens: &[(String, Vec<usize>)], stop: Option<char>) -> Option<Vec<(usize, i64)>> {
        let mut out = Vec::new();
        let mut order: Vec<usize> = (0..gens.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(gens[i].0.chars().count()));
        while *pos < s.len() && Some(s[*pos]) != stop {
            if s[*pos] == '(' {
                *pos += 1;
                let inner = seq(s, pos, gens, Some(')'))?;
                if *pos >= s.len() { return None; }
                *pos += 1;
                let k = exponent(s, pos)?;
                out.extend(power(&inner, k));
                continue;
            }
            if s[*pos] == '{' || s[*pos] == '}' { *pos += 1; continue; }
            let rest: String = s[*pos..].iter().collect();
            if rest.starts_with('e') && !gens.iter().any(|g| rest.starts_with(g.0.as_str())) { *pos += 1; continue; }
            let i = order.iter().copied().find(|&i| rest.starts_with(gens[i].0.as_str()))?;
            *pos += gens[i].0.chars().count();
            let k = exponent(s, pos)?;
            out.extend(power(&[(i, 1)], k));
        }
        Some(out)
    }
    fn power(w: &[(usize, i64)], k: i64) -> Vec<(usize, i64)> {
        let base: Vec<(usize, i64)> = if k >= 0 { w.to_vec() } else { w.iter().rev().map(|&(g, s)| (g, -s)).collect() };
        let mut v = Vec::new();
        for _ in 0..k.unsigned_abs() { v.extend(base.iter().copied()); }
        v
    }
    let word = seq(&s, &mut pos, gens, None)?;
    if pos != s.len() { return None; }
    let mut cur = 0usize;
    for &(g, sg) in &word {
        let t = &gens[g].1;
        cur = if sg > 0 { t[cur] } else { t.iter().position(|&x| x == cur)? };
    }
    Some((cur, word.len()))
}

fn perm_order(p: &[usize]) -> usize {
    let mut cur: Vec<usize> = p.to_vec();
    let id: Vec<usize> = (0..p.len()).collect();
    let mut k = 1;
    while cur != id { cur = cur.iter().map(|&c| p[c]).collect(); k += 1; if k > 1000 { break; } }
    k
}

/// right-regular images of all elements, by BFS (as groups.py does)
fn element_orders(gens: &[Vec<usize>]) -> BTreeMap<String, usize> {
    let n = gens[0].len();
    let mut reg: HashMap<usize, Vec<usize>> = HashMap::new();
    reg.insert(0, (0..n).collect());
    let mut q = VecDeque::from([0usize]);
    while let Some(i) = q.pop_front() {
        for g in gens {
            let j = g[i];
            if !reg.contains_key(&j) {
                let ri = reg[&i].clone();
                reg.insert(j, (0..n).map(|x| g[ri[x]]).collect());
                q.push_back(j);
            }
        }
    }
    let mut m = BTreeMap::new();
    for (_, p) in reg { *m.entry(perm_order(&p).to_string()).or_insert(0) += 1; }
    m
}

#[test]
fn cayley_groups() {
    let exp = load_expected();
    let groups = exp["groups"].clone();
    let colours: Vec<String> = exp["generator_colours"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
    let mut fs = Vec::new();
    let mut n = 0;
    for c in exp["cases"].as_array().unwrap() {
        let Some(wc) = c["expect"].get("cayleys").and_then(|v| v.as_array()).and_then(|v| v.first()) else { continue };
        let Some(key) = wc.get("group_key").and_then(|k| k.as_str()) else { continue };
        n += 1;
        let mine = &groups[key];
        let src = c["src"].as_str().unwrap();
        let (fig, _d, scene) = match parse_layout(src) { Ok(x) => x, Err(e) => { fail(&mut fs, c, "parse+layout", json!("ok"), json!(e)); continue } };
        let Some(cay) = fig.items.iter().find_map(|it| if let Item::Cayley(k) = it { Some(k.clone()) } else { None }) else {
            fail(&mut fs, c, "a Cayley item", json!(wc), json!("none")); continue
        };
        let g = match catch(|| ntex_figures::cayley::group(&cay.group, &cay.gens)) {
            Ok(Ok(g)) => g,
            Ok(Err(e)) => { fail(&mut fs, c, "cayley::group", json!(mine["order"]), json!(format!("Err: {e}"))); continue }
            Err(p) => { fail(&mut fs, c, "cayley::group", json!("no panic"), json!(p)); continue }
        };
        let order = mine["order"].as_u64().unwrap() as usize;
        if g.elements.len() != order {
            fail(&mut fs, c, "|G|", json!(order), json!(g.elements.len()));
            continue;
        }
        if g.elements.first().map(|s| s.as_str()) != Some("e") && !cay.gens.iter().all(|x| x.chars().all(|c| c.is_ascii_digit())) {
            fail(&mut fs, c, "soft: identity first, labelled e", json!("e"), json!(g.elements.first()));
        }
        let tables: Vec<Vec<usize>> = g.gens.iter().map(|x| x.1.clone()).collect();
        // tables are permutations
        for (name, t) in &g.gens {
            let mut s = t.clone(); s.sort();
            if s != (0..order).collect::<Vec<_>>() { fail(&mut fs, c, format!("generator {name}'s table is a permutation"), json!(order), json!(t)); }
        }
        let eo = element_orders(&tables);
        let my_eo: BTreeMap<String, usize> = mine["element_orders"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_u64().unwrap() as usize)).collect();
        if eo != my_eo { fail(&mut fs, c, "element orders (isomorphism class)", json!(my_eo), json!(eo)); }
        // rooted, generator-labelled isomorphism with my tables (skip S3/S4/A4: generators are the implementation's choice)
        let named = cay.group.clone();
        let free_choice = matches!(&named, GroupSpec::Named(s) if ["S3", "S4", "A4"].contains(&s.as_str()));
        let my_tables: Vec<Vec<usize>> = mine["gens"].as_array().unwrap().iter().map(|x| x["perm"].as_array().unwrap().iter().map(|y| y.as_u64().unwrap() as usize).collect()).collect();
        if !free_choice && my_tables.len() == tables.len() {
            let mut map = vec![usize::MAX; order];
            map[0] = 0;
            let mut q = VecDeque::from([0usize]);
            let mut ok = true;
            while let Some(u) = q.pop_front() {
                for k in 0..tables.len() {
                    let (iu, mu) = (tables[k][u], my_tables[k][map[u]]);
                    if map[iu] == usize::MAX { map[iu] = mu; q.push_back(iu); } else if map[iu] != mu { ok = false; }
                }
            }
            if !ok || map.contains(&usize::MAX) {
                fail(&mut fs, c, "same group with the same generators (rooted labelled Cayley graph isomorphism)", json!(mine["source"]), json!({"impl_tables": tables}));
            }
        }
        // relations hold (presentations)
        if let GroupSpec::Presentation(_, rels) = &cay.group {
            for r in rels {
                for start in 0..order {
                    // evaluate from `start`: shift by composing
                    let gens_named: Vec<(String, Vec<usize>)> = g.gens.clone();
                    let Some((end0, _)) = eval_word(r, &gens_named) else { fail(&mut fs, c, format!("relator {r} readable"), json!(r), json!("unparsed")); break };
                    if start == 0 && end0 != 0 { fail(&mut fs, c, format!("relator {r} = e"), json!(0), json!(end0)); }
                    break;
                }
            }
        }
        // generator orders → expected edges (mine for fixed groups, the impl's for S3/S4/A4)
        let gen_orders: Vec<usize> = tables.iter().map(|t| perm_order(t)).collect();
        if !free_choice {
            let mine_o: Vec<usize> = mine["gens"].as_array().unwrap().iter().map(|x| x["order"].as_u64().unwrap() as usize).collect();
            if mine_o != gen_orders { fail(&mut fs, c, "generator orders", json!(mine_o), json!(gen_orders)); }
        }
        let want_edges: Vec<usize> = gen_orders.iter().map(|&o| if o == 2 { order / 2 } else if o == 1 { 0 } else { order }).collect();
        // Scene: vertices, edges per colour, tips
        let verts: Vec<(String, Pt)> = scene.anchors.iter().filter_map(|(o, p, _)| match o { ObjRef::CayleyVertex(cn, w) if *cn == cay.name => Some((w.clone(), *p)), _ => None }).collect();
        if verts.len() != order { fail(&mut fs, c, "vertex anchors = |G|", json!(order), json!(verts.len())); }
        let mut by_edge: BTreeMap<usize, (String, bool)> = BTreeMap::new();
        for d in &scene.draws {
            if let Draw::Path { obj: ObjRef::CayleyEdge(cn, i), stroke, start_tip, end_tip, .. } = d {
                if *cn != cay.name { continue; }
                let col = stroke.as_ref().map(|s| s.color.clone()).unwrap_or_default();
                let tipped = start_tip.is_some() || end_tip.is_some();
                let e = by_edge.entry(*i).or_insert((col, false));
                e.1 |= tipped;
            }
        }
        let total: usize = want_edges.iter().sum();
        if by_edge.len() != total { fail(&mut fs, c, "edge count = Σ (|G| or |G|/2 for involutions)", json!({"per_generator": want_edges, "total": total}), json!(by_edge.len())); }
        for (k, &we) in want_edges.iter().enumerate() {
            let col = colours.get(k).cloned().unwrap_or_default();
            let got: Vec<&(String, bool)> = by_edge.values().filter(|(c2, _)| c2.split('!').next() == Some(col.as_str())).collect();
            if got.len() != we { fail(&mut fs, c, format!("generator {} ({col}) edges", cay.gens[k]), json!(we), json!(got.len())); }
            let tipped = got.iter().filter(|x| x.1).count();
            if gen_orders[k] == 2 && tipped > 0 { fail(&mut fs, c, format!("involution {} drawn without heads", cay.gens[k]), json!(0), json!(tipped)); }
            if gen_orders[k] > 2 && tipped != got.len() { fail(&mut fs, c, format!("generator {} arcs have heads", cay.gens[k]), json!(got.len()), json!(tipped)); }
        }
        // labels name what they label, shortlex = shortest
        // (FIG-POLISH, spec change: named Dn uses the normal form r^k, r^k s)
        let dihedral = matches!(&cay.group, GroupSpec::Named(s) if s.starts_with('D'));
        let additive = cay.gens.iter().all(|x| x.chars().all(|c| c.is_ascii_digit()));
        let vlabels: Vec<(String, String)> = scene.draws.iter().filter_map(|d| match d { Draw::Label { obj: ObjRef::CayleyVertex(cn, w), latex, .. } if *cn == cay.name => Some((w.clone(), latex.clone())), _ => None }).collect();
        if !cay.labels && !vlabels.is_empty() { fail(&mut fs, c, "labels=none draws no vertex labels", json!(0), json!(vlabels.len())); }
        if cay.labels && vlabels.len() != order { fail(&mut fs, c, "one label per vertex", json!(order), json!(vlabels.len())); }
        if cay.labels && !additive {
            // BFS distances over positive generators
            let mut dist = vec![usize::MAX; order]; dist[0] = 0;
            let mut q = VecDeque::from([0usize]);
            while let Some(u) = q.pop_front() { for t in &tables { if dist[t[u]] == usize::MAX { dist[t[u]] = dist[u] + 1; q.push_back(t[u]); } } }
            let mut bad = Vec::new();
            for (w, latex) in &vlabels {
                let Some(idx) = g.elements.iter().position(|x| x == w) else { bad.push(json!([w, latex, "vertex word not in elements"])); continue };
                match eval_word(latex, &g.gens) {
                    None => bad.push(json!([w, latex, "unreadable as a word"])),
                    Some((v, len)) => {
                        if v != idx { bad.push(json!([w, latex, format!("evaluates to element {} ({}), not {}", v, g.elements[v], idx)])); }
                        else if len != dist[idx] && !dihedral { bad.push(json!([w, latex, format!("length {len}, shortest is {}", dist[idx])])); }
                    }
                }
            }
            if !bad.is_empty() { fail(&mut fs, c, "vertex labels are shortest words naming their vertex", json!("all"), json!(bad)); }
        }
        if cay.legend && !scene.draws.iter().any(|d| matches!(d, Draw::Label { obj: ObjRef::Cayley(_), .. } | Draw::Path { obj: ObjRef::Cayley(_), .. })) {
            fail(&mut fs, c, "legend drawn", json!("a legend"), json!("none"));
        }
        // layout shape
        if verts.len() == order && order > 2 {
            let cx = verts.iter().map(|v| v.1.x).sum::<f32>() / order as f32;
            let cy = verts.iter().map(|v| v.1.y).sum::<f32>() / order as f32;
            let mut rs: Vec<f32> = verts.iter().map(|v| ((v.1.x - cx).powi(2) + (v.1.y - cy).powi(2)).sqrt()).collect();
            rs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let rings = { let mut k = 1; for i in 1..rs.len() { if rs[i] - rs[i - 1] > 0.03 * rs[rs.len() - 1].max(0.1) { k += 1; } } k };
            let spec_name = match &cay.group { GroupSpec::Named(s) => s.replace(' ', ""), _ => String::new() };
            let cyclic = (spec_name.starts_with('C') || (spec_name.starts_with('Z') && !spec_name.contains('x'))) || matches!(&cay.group, GroupSpec::Presentation(gs, _) if gs.len() == 1);
            let want_rings = match cay.layout {
                CayleyLayout::Circle => Some(1),
                CayleyLayout::Auto if cyclic && cay.gens.len() == 1 => Some(1),
                CayleyLayout::Auto | CayleyLayout::Prism if spec_name.starts_with('D') => Some(2),
                _ => None,
            };
            if let Some(w) = want_rings { if rings != w { fail(&mut fs, c, format!("layout: {w} concentric ring(s)"), json!(w), json!({"rings": rings, "radii": rs})); } }
            if spec_name.contains('x') && spec_name != "Z2xZ2" && cay.layout == CayleyLayout::Auto {
                let mut xs: Vec<i64> = verts.iter().map(|v| (v.1.x * 20.0).round() as i64).collect(); xs.sort(); xs.dedup();
                let mut ys: Vec<i64> = verts.iter().map(|v| (v.1.y * 20.0).round() as i64).collect(); ys.sort(); ys.dedup();
                let dims = { let mut d = vec![xs.len(), ys.len()]; d.sort(); d };
                if dims != vec![3, 4] { fail(&mut fs, c, "layout: Z3 x Z4 as a 3 × 4 grid", json!([3, 4]), json!(dims)); }
            }
        }
    }
    finish("CAYLEY", fs, n);
}

// ---------------------------------------------------------------- 4. n-gons

#[test]
fn ngon_geometry() {
    let cs = cases();
    let mut fs = Vec::new();
    let mut n_cases = 0;
    for c in &cs {
        let ngs = c["expect"]["ngons"].as_array().cloned().unwrap_or_default();
        if ngs.is_empty() { continue; }
        n_cases += 1;
        let (_fig, _d, scene) = match parse_layout(c["src"].as_str().unwrap()) { Ok(x) => x, Err(e) => { fail(&mut fs, c, "parse+layout", json!("ok"), json!(e)); continue } };
        check_ngons(c, &ngs, &scene, &mut fs);
    }
    finish("NGON", fs, n_cases);
}

/// The same checks on figures built straight from expected.json's model facts
/// (independent of the parser).
#[test]
fn ngon_geometry_direct() {
    let cs = cases();
    let mut fs = Vec::new();
    let mut n_cases = 0;
    for c in &cs {
        let ngs = c["expect"]["ngons"].as_array().cloned().unwrap_or_default();
        if ngs.is_empty() || c["tier"] != json!("spec") { continue; }
        n_cases += 1;
        let mut fig = Figure::default();
        for w in &ngs {
            let pick = |v: &Value| if let Some(a) = v.get("any_of") { a[0].clone() } else { v.clone() };
            let ng = NGon {
                name: w["name"].as_str().unwrap().into(), n: w["n"].as_u64().unwrap() as u32,
                labels: w["labels"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().into()).collect(),
                all_axes: w["all_axes"].as_bool().unwrap_or(false),
                axes: pick(&w["axes"]).as_array().map(|a| a.iter().map(|x| x.as_u64().unwrap() as u32).collect()).unwrap_or_default(),
                rotation: w["rotation"].as_u64().unwrap_or(0) as u32, turn: pick(&w["turn"]).as_u64().unwrap_or(0) as u32,
                flip: w["flip"].as_u64().map(|x| x as u32), radius: w.get("radius").and_then(|x| x.as_f64()).map(|x| x as f32),
                pos: w.get("pos").and_then(|p| p.get("Abs")).map(|p| Pos::Abs(p[0].as_f64().unwrap() as f32, p[1].as_f64().unwrap() as f32)),
                span: Span { line: 1, col: 1 },
                // spec change (interaction pass): NGon gained angle/show_labels/shaded/color
                angle: 0.0, show_labels: true, shaded: true, color: None,
                // (figure-qol D, C)
                rotation_cw: false, rotation_edge: None, turn_neg: false,
                rotation_label: None, axis_labels: vec![],
            };
            fig.items.push(Item::NGon(ng));
        }
        let scene = match catch(|| ntex_figures::layout::layout(&fig)) { Ok(s) => s, Err(e) => { fail(&mut fs, c, "layout", json!("no panic"), json!(e)); continue } };
        check_ngons(c, &ngs, &scene, &mut fs);
        // the closed outline visits all n vertices
        for w in &ngs {
            let name = w["name"].as_str().unwrap();
            let n = w["n"].as_u64().unwrap() as usize;
            let outline = scene.draws.iter().find_map(|d| match d { Draw::Path { obj: ObjRef::NGon(m), start, segs, closed: true, .. } if m == name => Some(path_points(*start, segs)), _ => None });
            let verts: Vec<Pt> = scene.anchors.iter().filter_map(|(o, p, _)| match o { ObjRef::NGonVertex(m, _) if m == name => Some(*p), _ => None }).collect();
            match outline {
                None => fail(&mut fs, c, format!("ngon {name}: closed outline"), json!("a closed path"), json!("none")),
                Some(pts) => {
                    let missing: Vec<Value> = verts.iter().filter(|v| !pts.iter().any(|p| (p.x - v.x).abs() < 0.02 && (p.y - v.y).abs() < 0.02)).map(|v| pt(*v)).collect();
                    if !missing.is_empty() || pts.len() < n { fail(&mut fs, c, format!("ngon {name}: outline passes through all {n} vertices"), json!(n), json!({"outline_points": pts.iter().map(|p| pt(*p)).collect::<Vec<_>>(), "vertices_not_on_outline": missing})); }
                }
            }
        }
    }
    // render them (my tectonic run of tikz::document, temp templates) for the look pass
    let tpl = templates_copy();
    let dir = tmp("direct");
    let mut pdfs = Vec::new();
    for c in &cs {
        let ngs = c["expect"]["ngons"].as_array().cloned().unwrap_or_default();
        if ngs.is_empty() || c["tier"] != json!("spec") { continue; }
        let id = c["id"].as_str().unwrap();
        let mut fig = Figure::default();
        for w in &ngs {
            let pick = |v: &Value| if let Some(a) = v.get("any_of") { a[0].clone() } else { v.clone() };
            fig.items.push(Item::NGon(NGon { name: w["name"].as_str().unwrap().into(), n: w["n"].as_u64().unwrap() as u32,
                labels: w["labels"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().into()).collect(),
                all_axes: w["all_axes"].as_bool().unwrap_or(false),
                axes: pick(&w["axes"]).as_array().map(|a| a.iter().map(|x| x.as_u64().unwrap() as u32).collect()).unwrap_or_default(),
                rotation: w["rotation"].as_u64().unwrap_or(0) as u32, turn: pick(&w["turn"]).as_u64().unwrap_or(0) as u32,
                flip: w["flip"].as_u64().map(|x| x as u32), radius: w.get("radius").and_then(|x| x.as_f64()).map(|x| x as f32),
                pos: w.get("pos").and_then(|p| p.get("Abs")).map(|p| Pos::Abs(p[0].as_f64().unwrap() as f32, p[1].as_f64().unwrap() as f32)),
                span: Span { line: 1, col: 1 },
                /* spec change (interaction pass) */ angle: 0.0, show_labels: true, shaded: true, color: None, rotation_cw: false, rotation_edge: None, turn_neg: false, rotation_label: None, axis_labels: vec![] }));
        }
        if let Some(p) = render_doc(&format!("direct-{id}"), &fig, &tpl, &dir) { pdfs.push(p); }
    }
    raster(&pdfs, "direct");
    finish("NGON_DIRECT", fs, n_cases);
}

/// my tectonic run of tikz::document (temp dir, temp templates); the PDF is
/// kept in verification/out/pdf.
fn render_doc(id: &str, fig: &Figure, tpl: &std::path::Path, dir: &std::path::Path) -> Option<PathBuf> {
    let scene = catch(|| ntex_figures::layout::layout(fig)).ok()?;
    let doc = catch(|| ntex_figures::tikz::document(fig, &scene)).ok()?;
    let tex = dir.join(format!("{id}.tex"));
    std::fs::write(&tex, &doc).ok()?;
    let o = Command::new("tectonic").args(["-X", "compile", "--keep-logs", "-Z"]).arg(format!("search-path={}", tpl.display())).arg(&tex).current_dir(dir).output().ok()?;
    if !o.status.success() { println!("[RENDER {id}] tectonic failed: {}", String::from_utf8_lossy(&o.stderr).lines().filter(|l| l.contains("error")).take(3).collect::<Vec<_>>().join(" | ")); return None; }
    let keep_dir = crate_dir().join("verification/out/pdf");
    std::fs::create_dir_all(&keep_dir).ok()?;
    let keep = keep_dir.join(format!("{id}.pdf"));
    std::fs::copy(dir.join(format!("{id}.pdf")), &keep).ok()?;
    let _ = std::fs::copy(&tex, keep_dir.join(format!("{id}.tex")));
    Some(keep)
}

fn raster(pdfs: &[PathBuf], sub: &str) -> Vec<Value> {
    if !raster_tool().exists() || pdfs.is_empty() { return vec![]; }
    let dir = crate_dir().join("verification/out/raster").join(sub);
    let o = Command::new(raster_tool()).arg("4").arg(&dir).args(pdfs).output().unwrap();
    let v: Vec<Value> = String::from_utf8_lossy(&o.stdout).lines().map(|l| serde_json::from_str(l).unwrap_or(json!({"bad": l}))).collect();
    write_out(&format!("look-{sub}.json"), &json!(v));
    v
}

fn check_ngons(c: &Value, ngs: &[Value], scene: &Scene, fs: &mut Vec<Failure>) {
    let mut fs = fs;
    {
        for w in ngs.iter().cloned() {
            let name = w["name"].as_str().unwrap();
            let n = w["n"].as_u64().unwrap() as u32;
            let labels: Vec<String> = w["labels"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
            let verts: BTreeMap<u32, Pt> = scene.anchors.iter().filter_map(|(o, p, _)| match o { ObjRef::NGonVertex(m, k) if m == name => Some((*k, *p)), _ => None }).collect();
            if verts.len() != n as usize { fail(&mut fs, c, format!("ngon {name}: n vertex anchors"), json!(n), json!(verts.len())); continue; }
            let cx = verts.values().map(|p| p.x).sum::<f32>() / n as f32;
            let cy = verts.values().map(|p| p.y).sum::<f32>() / n as f32;
            let ang = |p: Pt| -> f64 { (((p.y - cy) as f64).atan2((p.x - cx) as f64).to_degrees() + 360.0) % 360.0 };
            let angdiff = |a: f64, b: f64| -> f64 { let d = (a - b).rem_euclid(360.0); d.min(360.0 - d) };
            // vertex k at slot k-1: base + 360(k-1)/n, base = 90 (odd n) or
            // 90 + 180/n (even n: a flat bottom edge, vertex 1 top-left) —
            // FIG-POLISH, spec change
            let base = if n % 2 == 0 { 90.0 + 180.0 / n as f64 } else { 90.0 };
            let mut vbad = Vec::new();
            for (k, p) in &verts {
                let want = (base + 360.0 * (*k as f64 - 1.0) / n as f64) % 360.0;
                if angdiff(ang(*p), want) > 3.0 { vbad.push(json!({"vertex": k, "want_deg": want, "got_deg": ang(*p)})); }
            }
            if !vbad.is_empty() { fail(&mut fs, c, format!("ngon {name}: vertex k counter-clockwise from the top"), json!("base+360°(k−1)/n"), json!(vbad)); }
            if let Some(r) = w.get("radius").and_then(|v| v.as_f64()) {
                let got = ((verts[&1].x - cx).powi(2) + (verts[&1].y - cy).powi(2)).sqrt() as f64;
                if (got - r).abs() > 0.02 { fail(&mut fs, c, format!("ngon {name}: radius"), json!(r), json!(got)); }
            }
            if let Some(p) = w.get("pos").and_then(|v| v.get("Abs")) {
                let (px, py) = (p[0].as_f64().unwrap(), p[1].as_f64().unwrap());
                if (cx as f64 - px).abs() > 0.05 || (cy as f64 - py).abs() > 0.05 { fail(&mut fs, c, format!("ngon {name}: centre at"), json!([px, py]), json!([cx, cy])); }
            }
            // labels → slots
            if let Some(slots) = w.get("slots").and_then(|v| v.as_object()) {
                let mut bad = Vec::new();
                for (j, slot) in slots {
                    let j: usize = j.parse().unwrap();
                    let lab = &labels[j - 1];
                    let want_slot = slot.as_u64().unwrap() as u32;
                    let found: Vec<Pt> = scene.draws.iter().filter_map(|d| match d {
                        Draw::Label { obj: ObjRef::NGon(m) | ObjRef::NGonVertex(m, _), at, latex, .. } if m == name && tex_eq(latex, lab) => Some(*at),
                        _ => None }).collect();
                    let Some(at) = found.first() else { bad.push(json!({"label": lab, "problem": "not drawn"})); continue };
                    let got_slot = verts.iter().min_by(|a, b| {
                        let da = (a.1.x - at.x).powi(2) + (a.1.y - at.y).powi(2);
                        let db = (b.1.x - at.x).powi(2) + (b.1.y - at.y).powi(2);
                        da.partial_cmp(&db).unwrap() }).map(|(k, _)| k - 1).unwrap();
                    if got_slot != want_slot { bad.push(json!({"label": lab, "want_slot": want_slot, "got_slot": got_slot})); }
                }
                if !bad.is_empty() { fail(&mut fs, c, format!("ngon {name}: labels after turn/flip"), json!(slots), json!(bad)); }
            }
            // axes
            if let Some(dirs) = w.get("axis_dirs").and_then(|v| v.as_array()) {
                let want: Vec<f64> = dirs.iter().map(|x| x.as_f64().unwrap()).collect();
                let mut got: Vec<f64> = Vec::new();
                let mut off_centre = Vec::new();
                for d in &scene.draws {
                    if let Draw::Path { obj: ObjRef::NGon(m), start, segs, closed: false, stroke: Some(s), .. } = d {
                        if m != name || s.line != Line::Dashed || segs.len() != 1 { continue; }
                        let e = path_end(*start, segs);
                        let a = (((e.y - start.y) as f64).atan2((e.x - start.x) as f64).to_degrees() + 360.0) % 180.0;
                        got.push(a);
                        // distance from centre to the line
                        let (dx, dy) = (e.x - start.x, e.y - start.y);
                        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
                        let dist = ((cx - start.x) * dy - (cy - start.y) * dx).abs() / len;
                        if dist > 0.05 { off_centre.push(json!({"dir": a, "dist": dist})); }
                    }
                }
                got.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let ok = got.len() == want.len() && want.iter().all(|wa| got.iter().any(|ga| { let d = (wa - ga).rem_euclid(180.0); d.min(180.0 - d) < 2.0 }));
                if !ok { fail(&mut fs, c, format!("ngon {name}: reflection axes (directions mod 180°)"), json!(want), json!(got)); }
                if !off_centre.is_empty() { fail(&mut fs, c, format!("ngon {name}: axes pass through the centre"), json!(0), json!(off_centre)); }
            }
            let rot = w.get("rotation").and_then(|v| v.as_u64()).unwrap_or(0);
            if rot > 0 {
                let want = if rot == 1 { "r".to_string() } else { format!("r^{{{rot}}}") };
                let has = scene.draws.iter().any(|d| matches!(d, Draw::Label { obj: ObjRef::NGonRotation(m), latex, .. } if m == name && tex_eq(latex, &want)));
                if !has { fail(&mut fs, c, format!("ngon {name}: rotation arrow labelled {want}"), json!(want), json!(scene.draws.iter().filter_map(|d| match d { Draw::Label { obj: ObjRef::NGonRotation(m), latex, .. } if m == name => Some(latex.clone()), _ => None }).collect::<Vec<_>>())); }
            }
        }
    }
}

// ---------------------------------------------------------------- 5. layout shapes, grid geometry, blobs

#[test]
fn layout_shapes() {
    let cs = cases();
    let mut fs = Vec::new();
    let mut n = 0;
    for c in &cs {
        let e = &c["expect"];
        let interesting = ["auto_chain", "auto_shape", "cell_vector", "grid_cells", "blobs", "blob_row"].iter().any(|k| e.get(*k).is_some());
        if !interesting { continue; }
        n += 1;
        let (fig, _d, scene) = match parse_layout(c["src"].as_str().unwrap()) { Ok(x) => x, Err(err) => { fail(&mut fs, c, "parse+layout", json!("ok"), json!(err)); continue } };
        let cell = if scene.cell.0 > 0.0 { scene.cell } else { (fig.globals.spacing.0 * fig.globals.scale, fig.globals.spacing.1 * fig.globals.scale) };
        let pos = |name: &str| -> Option<Pt> { scene.anchors.iter().find_map(|(o, p, _)| match o { ObjRef::Node(m) if tex_eq(m, name) => Some(*p), _ => None }) };
        let on_cells = |names: &[String]| -> Option<Vec<(f32, f32)>> {
            let p0 = pos(&names[0])?;
            names.iter().map(|m| pos(m).map(|p| ((p.x - p0.x) / cell.0, (p0.y - p.y) / cell.1))).collect()
        };
        let integral = |v: &[(f32, f32)]| v.iter().all(|(a, b)| (a - a.round()).abs() < 0.02 && (b - b.round()).abs() < 0.02);
        if let Some(chain) = e.get("auto_chain") {
            let names: Vec<String> = chain.as_array().unwrap().iter().map(|x| x.as_str().unwrap().into()).collect();
            match on_cells(&names) {
                None => fail(&mut fs, c, "chain: nodes placed", json!(names), json!("missing anchors")),
                Some(v) => {
                    let ok = integral(&v) && v.iter().all(|p| p.1.abs() < 0.02) && v.windows(2).all(|w| w[1].0 > w[0].0 + 0.5);
                    if !ok { fail(&mut fs, c, "chain goes left to right on one row, integer cells", json!("(0,0),(1,0),(2,0),…"), json!(v)); }
                }
            }
        }
        if let Some(sh) = e.get("auto_shape") {
            let names: Vec<String> = sh["nodes"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().into()).collect();
            let kind = sh["kind"].as_str().unwrap();
            match on_cells(&names) {
                None => fail(&mut fs, c, format!("{kind}: nodes placed"), json!(names), json!("missing anchors")),
                Some(v) => {
                    let mut ok = integral(&v);
                    match kind {
                        "triangle" => { let a = (v[1].0 - v[0].0) * (v[2].1 - v[0].1) - (v[2].0 - v[0].0) * (v[1].1 - v[0].1); ok &= a.abs() > 0.4; }
                        "square" => {
                            let mut xs: Vec<i32> = v.iter().map(|p| p.0.round() as i32).collect(); xs.sort(); xs.dedup();
                            let mut ys: Vec<i32> = v.iter().map(|p| p.1.round() as i32).collect(); ys.sort(); ys.dedup();
                            ok &= xs.len() == 2 && ys.len() == 2 && (v[0].0 - v[3].0).abs() > 0.5 && (v[0].1 - v[3].1).abs() > 0.5;
                        }
                        "fan" => {
                            let t = &v[1..];
                            let col = t.iter().all(|p| (p.0 - t[0].0).abs() < 0.02) && (t[0].0 - v[0].0).abs() > 0.5;
                            let row = t.iter().all(|p| (p.1 - t[0].1).abs() < 0.02) && (t[0].1 - v[0].1).abs() > 0.5;
                            let mut uniq: Vec<(i32, i32)> = v.iter().map(|p| (p.0.round() as i32, p.1.round() as i32)).collect(); uniq.sort(); uniq.dedup();
                            ok &= (col || row) && uniq.len() == v.len();
                        }
                        _ => { let mut uniq: Vec<(i32, i32)> = v.iter().map(|p| (p.0.round() as i32, p.1.round() as i32)).collect(); uniq.sort(); uniq.dedup(); ok &= uniq.len() == v.len(); }
                    }
                    if !ok { fail(&mut fs, c, format!("auto-placement: {kind} on integer cells"), json!(kind), json!(names.iter().zip(v.iter()).map(|(m, p)| json!([m, p.0, p.1])).collect::<Vec<_>>())); }
                }
            }
        }
        if let Some(cv) = e.get("cell_vector") {
            let (a, b) = (pos(cv["from"].as_str().unwrap()), pos(cv["to"].as_str().unwrap()));
            if let (Some(a), Some(b)) = (a, b) {
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                if (dx as f64 - cv["dx"].as_f64().unwrap()).abs() > 0.02 || (dy as f64 - cv["dy"].as_f64().unwrap()).abs() > 0.02 {
                    fail(&mut fs, c, "cell size in cm", json!([cv["dx"], cv["dy"]]), json!([dx, dy]));
                }
            } else { fail(&mut fs, c, "cell size: anchors", json!("A, B"), json!("missing")); }
        }
        if let Some(gc) = e.get("grid_cells").and_then(|v| v.as_object()) {
            let names: Vec<String> = gc.keys().cloned().collect();
            let want: Vec<(f64, f64)> = gc.values().map(|v| (v[0].as_f64().unwrap(), v[1].as_f64().unwrap())).collect();
            if let Some(v) = on_cells(&names) {
                let bad: Vec<Value> = names.iter().zip(v.iter()).zip(want.iter()).filter(|((_, g), w)| ((g.0 as f64) - (w.0 - want[0].0)).abs() > 0.02 || ((g.1 as f64) - (w.1 - want[0].1)).abs() > 0.02)
                    .map(|((m, g), w)| json!({"node": m, "want": [w.0 - want[0].0, w.1 - want[0].1], "got": [g.0, g.1]})).collect();
                if !bad.is_empty() { fail(&mut fs, c, "grid nodes at (col·cell.0, −row·cell.1)", json!(gc), json!(bad)); }
            } else { fail(&mut fs, c, "grid nodes have anchors", json!(names), json!(scene.anchors.iter().map(|x| format!("{:?}", x.0)).collect::<Vec<_>>())); }
        }
        // blobs: containment
        let outline = |name: &str| -> Option<Rect> { scene.draws.iter().find_map(|d| match d { Draw::Path { obj: ObjRef::Blob(m), start, segs, closed: true, .. } if m == name => Some(bbox(&path_points(*start, segs))), _ => None }) };
        let contains = |outer: &Rect, inner: &Rect| inner.min.x >= outer.min.x - 0.02 && inner.max.x <= outer.max.x + 0.02 && inner.min.y >= outer.min.y - 0.02 && inner.max.y <= outer.max.y + 0.02;
        for it in &fig.items {
            let Item::Blob(b) = it else { continue };
            let Some(r) = outline(&b.name) else { fail(&mut fs, c, format!("blob {} outline drawn", b.name), json!("closed path"), json!("none")); continue };
            if let Some(p) = &b.parent { if let Some(pr) = outline(p) { if !contains(&pr, &r) { fail(&mut fs, c, format!("blob {} inside parent {p}", b.name), json!([pt(pr.min), pt(pr.max)]), json!([pt(r.min), pt(r.max)])); } } }
            let children: Vec<Rect> = fig.items.iter().filter_map(|x| match x { Item::Blob(k) if k.parent.as_deref() == Some(&b.name) => outline(&k.name), _ => None }).collect();
            for el in &b.elements {
                let at = scene.anchors.iter().find_map(|(o, p, _)| match o { ObjRef::Element(m) if *m == el.name => Some(*p), _ => None });
                match at {
                    None => fail(&mut fs, c, format!("element {} placed", el.name), json!("an anchor"), json!("none")),
                    Some(p) => {
                        let pr = Rect { min: p, max: p };
                        if !contains(&r, &pr) { fail(&mut fs, c, format!("element {} inside blob {}", el.name, b.name), json!([pt(r.min), pt(r.max)]), json!(pt(p))); }
                        for ch in &children { if contains(ch, &pr) { fail(&mut fs, c, format!("element {} avoids child blobs of {}", el.name, b.name), json!("outside children"), json!(pt(p))); } }
                    }
                }
            }
        }
        if let Some(row) = e.get("blob_row") {
            let rs: Vec<Option<Rect>> = row.as_array().unwrap().iter().map(|x| outline(x.as_str().unwrap())).collect();
            if rs.iter().all(|r| r.is_some()) {
                let cs2: Vec<(f32, f32)> = rs.iter().map(|r| { let r = r.as_ref().unwrap(); ((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0) }).collect();
                let ok = cs2.windows(2).all(|w| w[1].0 > w[0].0 + 0.5 && (w[1].1 - w[0].1).abs() < 0.3)
                    && rs.windows(2).all(|w| w[0].as_ref().unwrap().max.x < w[1].as_ref().unwrap().min.x);
                if !ok { fail(&mut fs, c, "top-level blobs in a row, left to right, not overlapping", json!(row), json!(cs2)); }
            }
        }
    }
    finish("LAYOUT", fs, n);
}

// ---------------------------------------------------------------- 6. TikZ text

#[test]
fn tikz_styles() {
    let cs = cases();
    let mut fs = Vec::new();
    let mut n = 0;
    for c in &cs {
        let src = c["src"].as_str().unwrap();
        let Ok((fig, _d, scene)) = parse_layout(src) else { continue };
        n += 1;
        let Ok((pic, doc)) = catch(|| (ntex_figures::tikz::picture(&fig, &scene), ntex_figures::tikz::document(&fig, &scene))) else {
            fail(&mut fs, c, "tikz", json!("no panic"), json!("panicked")); continue
        };
        if !doc.contains("\\documentclass[border=3pt]{standalone}") || !doc.contains("\\usepackage{ntex-figure}") {
            fail(&mut fs, c, "document: standalone border=3pt + ntex-figure", json!("both"), json!(doc.lines().take(6).collect::<Vec<_>>()));
        }
        for it in &fig.items {
            let Item::Arrow(a) = it else { continue };
            let key = match a.head { Head::TwoHeads => Some("two heads"), Head::Tail => Some("tail"), Head::Hook => Some("hook"),
                Head::Double => Some("Rightarrow"), Head::Equal => Some("equal"), Head::MapsTo => Some("maps to"), _ => None };
            if let Some(k) = key {
                let alt = if a.head == Head::MapsTo { pic.contains("|->") } else { false };
                if !pic.contains(k) && !alt { fail(&mut fs, c, format!("tikz-cd style `{k}` for {:?}", a.head), json!(k), json!(pic.chars().take(600).collect::<String>())); }
            }
            let lk = match a.line { Line::Dashed => Some("dashed"), Line::Dotted => Some("dotted"), _ => None };
            if let Some(k) = lk { if !pic.contains(k) { fail(&mut fs, c, format!("line style `{k}`"), json!(k), json!(pic.chars().take(600).collect::<String>())); } }
        }
        // colours named in the TikZ
        let pal = ["blue", "violet", "teal", "maroon", "green", "ochre", "grey", "ink", "white", "none"];
        let mut bad = Vec::new();
        let bytes: Vec<char> = pic.chars().collect();
        let s: String = bytes.iter().collect();
        for key in ["color=", "draw=", "fill=", "text=", "\\color{", "\\textcolor{"] {
            let mut from = 0;
            while let Some(i) = s[from..].find(key) {
                let st = from + i + key.len();
                let val: String = s[st..].chars().take_while(|ch| ch.is_alphanumeric() || *ch == '!' || *ch == '.' || *ch == '-').collect();
                let base = val.split('!').next().unwrap_or("").to_string();
                let base_l = base.to_lowercase();
                if !base.is_empty() && !pal.contains(&base.as_str()) && !base_l.starts_with("ntex") && !base_l.starts_with("fig") { bad.push(val); }
                from = st;
            }
        }
        bad.sort(); bad.dedup();
        if !bad.is_empty() { fail(&mut fs, c, "TikZ names palette colours only", json!(pal), json!(bad)); }
    }
    finish("TIKZ", fs, n);
}

// ---------------------------------------------------------------- 7. build with tectonic (+ look rasters)

fn raster_tool() -> PathBuf { crate_dir().join("verification/tools/raster") }

#[test]
fn build_all_and_look() {
    let cs = cases();
    let tpl = templates_copy();
    let work = tmp("build");
    let out_pdf_dir = crate_dir().join("verification/out/pdf");
    let _ = std::fs::remove_dir_all(&out_pdf_dir);
    std::fs::create_dir_all(&out_pdf_dir).unwrap();
    let results: std::sync::Mutex<Vec<(Value, Result<Vec<String>, String>, Vec<Diagnostic>)>> = std::sync::Mutex::new(Vec::new());
    let chunks: Vec<Vec<Value>> = cs.chunks(cs.len().div_ceil(4)).map(|x| x.to_vec()).collect();
    std::thread::scope(|sc| {
        for ch in &chunks {
            let (tpl, work, results) = (&tpl, &work, &results);
            sc.spawn(move || {
                for c in ch {
                    let id = c["id"].as_str().unwrap();
                    let src = c["src"].as_str().unwrap();
                    let diags = catch(|| ntex_figures::parse::parse(src).1).unwrap_or_default();
                    let pdf = work.join(format!("{id}.pdf"));
                    let r = match catch(|| ntex_figures::build::build_source(src, &pdf, tpl)) {
                        Ok(Ok(b)) => Ok(b.warnings),
                        Ok(Err(e)) => Err(e),
                        Err(p) => Err(format!("PANIC: {p}")),
                    };
                    results.lock().unwrap().push((c.clone(), r, diags));
                }
            });
        }
    });
    let mut fs = Vec::new();
    let mut ok = 0;
    for (c, r, diags) in results.into_inner().unwrap() {
        let id = c["id"].as_str().unwrap();
        let want = &c["build"]["compile"];
        let has_err = diags.iter().any(|d| d.severity == Severity::Error);
        let pdf = work.join(format!("{id}.pdf"));
        match &r {
            Ok(_) => { ok += 1; let _ = std::fs::copy(&pdf, out_pdf_dir.join(format!("{id}.pdf"))); }
            Err(e) => {
                if e.starts_with("PANIC") || (want == &json!(true)) {
                    fail(&mut fs, &c, if has_err { "builds despite a bad line" } else { "builds" }, json!("a PDF"), json!(e));
                }
            }
        }
        if let Some(l) = c["build"].get("report_line").and_then(|v| v.as_i64()) {
            let in_diags = diags.iter().any(|d| d.span.line as i64 == l);
            let in_warn = r.as_ref().map(|w| w.iter().any(|s| s.contains(&format!("line {l}")))).unwrap_or(false);
            if !in_diags && !in_warn { fail(&mut fs, &c, format!("problem on line {l} reported (parse or build)"), json!(l), json!({"diags": diag_brief(&diags), "build": r})); }
        }
    }
    println!("built {ok}/{} cases", cs.len());

    // atomic: a TeX error keeps the old PDF (build(), with a .fig in a temp figures/ dir)
    let figdir = tmp("atomic").join("figures");
    std::fs::create_dir_all(&figdir).unwrap();
    let fig = figdir.join("x.fig");
    std::fs::write(&fig, "A -f-> B\n").unwrap();
    match catch(|| ntex_figures::build::build(&fig, &tpl)) {
        Ok(Ok(b)) => {
            let before = std::fs::read(&b.pdf).unwrap_or_default();
            std::fs::write(&fig, "A -\\undefinedcontrolsequence-> B\n").unwrap();
            let r2 = catch(|| ntex_figures::build::build(&fig, &tpl));
            let after = std::fs::read(figdir.join("x.pdf")).unwrap_or_default();
            let c = json!({"id": "ATOMIC", "tier": "spec", "src": "A -\\undefinedcontrolsequence-> B"});
            if !matches!(r2, Ok(Err(_))) { fail(&mut fs, &c, "a TeX error is an Err", json!("Err(first TeX error line)"), json!(format!("{:?}", r2.map(|x| x.map(|b| b.warnings))))); }
            if before.is_empty() || before != after { fail(&mut fs, &c, "the old PDF is kept", json!(before.len()), json!(after.len())); }
        }
        other => { let c = json!({"id": "ATOMIC", "tier": "spec", "src": "A -f-> B"}); fail(&mut fs, &c, "build() of a good .fig", json!("Ok"), json!(format!("{:?}", other.map(|x| x.map(|b| b.pdf))))); }
    }

    // the look: my own tectonic run (keep logs) + rasters for the look cases and a sample
    let look_ids: Vec<String> = cs.iter().filter(|c| c["look"] == json!(true)).map(|c| c["id"].as_str().unwrap().to_string())
        .chain(["A04", "A06", "A08", "A10", "A12", "A17", "A19", "A20", "A26", "A27", "G01", "G02", "G05", "G08", "G13", "G15", "B01", "B02", "B03", "B04", "P02", "P03", "P07", "P15", "P16", "C01", "C05", "C07", "C11", "C12", "C13", "C18", "C19", "C22", "N09", "S05", "L02", "L04", "R05"].iter().map(|s| s.to_string())).collect();
    let mut pdfs = Vec::new();
    let logdir = tmp("logs");
    for id in &look_ids {
        let Some(c) = cs.iter().find(|c| c["id"] == json!(id)) else { continue };
        let Ok((f, _d, sc)) = parse_layout(c["src"].as_str().unwrap()) else { continue };
        let Ok(doc) = catch(|| ntex_figures::tikz::document(&f, &sc)) else { continue };
        let tex = logdir.join(format!("{id}.tex"));
        std::fs::write(&tex, &doc).unwrap();
        let o = Command::new("tectonic").args(["-X", "compile", "--keep-logs", "-Z"]).arg(format!("search-path={}", tpl.display())).arg(&tex).current_dir(&logdir).output();
        let log = std::fs::read_to_string(logdir.join(format!("{id}.log"))).unwrap_or_default();
        let missing: Vec<&str> = log.lines().filter(|l| l.contains("Missing character")).collect();
        if !missing.is_empty() { fail(&mut fs, c, "no Missing character in the log", json!(0), json!(missing)); }
        if o.map(|o| !o.status.success()).unwrap_or(true) { fail(&mut fs, c, "my tectonic run of tikz::document", json!("compiles"), json!(log.lines().filter(|l| l.starts_with('!')).take(3).collect::<Vec<_>>())); continue; }
        let p = logdir.join(format!("{id}.pdf"));
        let keep = out_pdf_dir.join(format!("look-{id}.pdf"));
        let _ = std::fs::copy(&p, &keep);
        pdfs.push(keep);
    }
    let raster_dir = crate_dir().join("verification/out/raster");
    let _ = std::fs::remove_dir_all(&raster_dir);
    if raster_tool().exists() && !pdfs.is_empty() {
        let o = Command::new(raster_tool()).arg("4").arg(&raster_dir).args(&pdfs).output().unwrap();
        let text = String::from_utf8_lossy(&o.stdout).to_string();
        let mut looks = Vec::new();
        for line in text.lines() {
            let v: Value = serde_json::from_str(line).unwrap_or(json!({"bad": line}));
            let id = v["pdf"].as_str().unwrap_or("").rsplit('/').next().unwrap_or("").trim_start_matches("look-").trim_end_matches(".pdf").to_string();
            let c = cs.iter().find(|c| c["id"] == json!(id)).cloned().unwrap_or(json!({"id": id, "tier": "spec", "src": ""}));
            let ink = v["ink"].as_f64().unwrap_or(1.0).max(1.0);
            let off = v["off_palette"].as_f64().unwrap_or(0.0);
            if off / ink > 0.005 { fail(&mut fs, &c, "pixels are palette colours (blends with white)", json!("≤ 0.5 % off-palette"), json!({"off": off, "ink": ink, "samples": v["off_samples"]})); }
            let fonts: Vec<String> = v["fonts"].as_array().cloned().unwrap_or_default().iter().map(|x| x.as_str().unwrap_or("").to_string()).collect();
            let has_charter = fonts.iter().any(|f| f.contains("XCharter") || f.contains("Charter"));
            let has_cm = fonts.iter().any(|f| { let f = f.split('+').next_back().unwrap_or(""); f.starts_with("CM") || f.starts_with("LM") || f.contains("lmroman") || f.contains("LMRoman") || f.starts_with("cmr") || f.starts_with("cmmi") });
            if !fonts.is_empty() && (!has_charter || has_cm) { fail(&mut fs, &c, "labels in Charter (no Computer/Latin Modern)", json!("XCharter / XCharterMath"), json!(fonts)); }
            looks.push(v);
        }
        write_out("look.json", &json!(looks));
    }
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&logdir);
    finish("BUILD", fs, cs.len());
}

// ---------------------------------------------------------------- 3b. cayley::group alone (no parser)

#[test]
fn cayley_group_direct() {
    let exp = load_expected();
    let groups = exp["groups"].clone();
    let mut fs = Vec::new();
    let mut n = 0;
    for c in exp["cases"].as_array().unwrap() {
        let Some(wc) = c["expect"].get("cayleys").and_then(|v| v.as_array()).and_then(|v| v.first()) else { continue };
        let gens: Vec<String> = wc["gens"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().into()).collect();
        let spec = if let Some(nm) = wc["group"].get("Named") {
            let s = nm.as_str().map(|s| s.to_string()).unwrap_or_else(|| nm["any_of"][0].as_str().unwrap().to_string());
            GroupSpec::Named(s)
        } else {
            let p = &wc["group"]["Presentation"];
            GroupSpec::Presentation(p[0].as_array().unwrap().iter().map(|x| x.as_str().unwrap().into()).collect(),
                                    p[1].as_array().unwrap().iter().map(|x| x.as_str().unwrap().into()).collect())
        };
        n += 1;
        let key = wc.get("group_key").and_then(|k| k.as_str());
        let r = catch(|| ntex_figures::cayley::group(&spec, &gens));
        let Some(key) = key else {
            // C15 (infinite): must be an Err, quickly, not a hang or panic
            match r { Ok(Err(_)) => {}, other => fail(&mut fs, c, "an infinite group is refused with Err", json!("Err"), json!(format!("{:?}", other.map(|x| x.map(|g| g.elements.len()))))) }
            continue;
        };
        let mine = &groups[key];
        let g = match r { Ok(Ok(g)) => g, Ok(Err(e)) => { fail(&mut fs, c, "cayley::group", json!(mine["order"]), json!(format!("Err: {e}"))); continue } Err(p) => { fail(&mut fs, c, "cayley::group", json!("no panic"), json!(p)); continue } };
        let order = mine["order"].as_u64().unwrap() as usize;
        if g.elements.len() != order { fail(&mut fs, c, "|G|", json!(order), json!(g.elements.len())); continue; }
        let tables: Vec<Vec<usize>> = g.gens.iter().map(|x| x.1.clone()).collect();
        let eo = element_orders(&tables);
        let my_eo: BTreeMap<String, usize> = mine["element_orders"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_u64().unwrap() as usize)).collect();
        if eo != my_eo { fail(&mut fs, c, "element orders", json!(my_eo), json!(eo)); }
        let free = matches!(&spec, GroupSpec::Named(s) if ["S3", "S4", "A4"].contains(&s.as_str()));
        let my_tables: Vec<Vec<usize>> = mine["gens"].as_array().unwrap().iter().map(|x| x["perm"].as_array().unwrap().iter().map(|y| y.as_u64().unwrap() as usize).collect()).collect();
        if !free {
            let mut map = vec![usize::MAX; order]; map[0] = 0;
            let mut q = VecDeque::from([0usize]); let mut ok = my_tables.len() == tables.len();
            while ok && let Some(u) = q.pop_front() {
                for k in 0..tables.len() { let (iu, mu) = (tables[k][u], my_tables[k][map[u]]); if map[iu] == usize::MAX { map[iu] = mu; q.push_back(iu); } else if map[iu] != mu { ok = false; } }
            }
            if !ok || map.contains(&usize::MAX) { fail(&mut fs, c, "same group, same generators (rooted labelled isomorphism)", json!(mine["source"]), json!({"impl_gens": g.gens.iter().map(|x| x.0.clone()).collect::<Vec<_>>()})); }
        }
        // every element word names its element
        let numeric = gens.iter().all(|x| x.chars().all(|ch| ch.is_ascii_digit()));
        // (FIG-POLISH, spec change: named Dn uses the normal form r^k, r^k s)
        let dihedral = matches!(&spec, GroupSpec::Named(s) if s.starts_with('D'));
        if !numeric {
            let mut dist = vec![usize::MAX; order]; dist[0] = 0;
            let mut q = VecDeque::from([0usize]);
            while let Some(u) = q.pop_front() { for t in &tables { if dist[t[u]] == usize::MAX { dist[t[u]] = dist[u] + 1; q.push_back(t[u]); } } }
            let mut bad = Vec::new();
            for (i, w) in g.elements.iter().enumerate() {
                match eval_word(w, &g.gens) {
                    None => bad.push(json!([w, "unreadable"])),
                    Some((v, len)) => { if v != i { bad.push(json!([w, format!("is element {v}, listed as {i}")])); } else if len != dist[i] && !dihedral { bad.push(json!([w, format!("length {len} > shortest {}", dist[i])])); } }
                }
            }
            if !bad.is_empty() { fail(&mut fs, c, "element words name their elements, shortest", json!("all"), json!(bad)); }
            // shortlex order: by length, then lexicographic in generator order
            let keyed: Vec<usize> = g.elements.iter().map(|w| eval_word(w, &g.gens).map(|x| x.1).unwrap_or(0)).collect();
            if !dihedral && keyed.windows(2).any(|w| w[1] < w[0]) { fail(&mut fs, c, "soft: elements listed in shortlex (length) order", json!("non-decreasing lengths"), json!(g.elements)); }
        }
        if let GroupSpec::Presentation(_, rels) = &spec {
            for rel in rels {
                match eval_word(rel, &g.gens) { Some((0, _)) => {}, other => fail(&mut fs, c, format!("relator {rel} = e"), json!(0), json!(format!("{other:?}"))) }
            }
        }
    }
    finish("GROUPS", fs, n);
}
