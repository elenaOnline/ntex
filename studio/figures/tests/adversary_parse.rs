//! ADVERSARY: the parser against verification/expected.json (written from
//! FIGURES.md + model.rs before the implementation existed; see
//! verification/acceptance.md). Every case runs; every failure is printed as
//! input → expected → actual, and the full list is written to
//! verification/out/parse_results.json. The test fails if any `spec` or
//! `inferred` case fails (`gap` cases are reported only).
//!
//!     cargo test -p ntex-figures --test adversary_parse -- --nocapture

#[path = "adversary_common/mod.rs"]
mod common;

use common::*;
use ntex_figures::model::*;
use serde_json::{Value, json};

#[test]
fn adversary_parse() {
    let exp = load_expected();
    let cases = exp["cases"].as_array().unwrap();
    let mut results = Vec::new();
    let mut failures: Vec<Failure> = Vec::new();
    for c in cases {
        let id = c["id"].as_str().unwrap().to_string();
        let tier = c["tier"].as_str().unwrap().to_string();
        let src = c["src"].as_str().unwrap().to_string();
        let mut fails = Vec::new();
        let parsed = catch(|| ntex_figures::parse::parse(&src));
        match parsed {
            Err(msg) => fails.push(("panic".to_string(), json!("no panic"), json!(msg))),
            Ok((fig, diags)) => {
                check_case(c, &fig, &diags, &mut fails);
            }
        }
        let n = fails.len();
        for (check, e, a) in fails {
            failures.push(Failure { id: id.clone(), tier: tier.clone(), check, src: src.clone(), expected: e, actual: a });
        }
        results.push(json!({"id": id, "tier": tier, "failures": n}));
    }
    let hard: Vec<&Failure> = failures.iter().filter(|f| f.tier != "gap" && !f.check.starts_with("soft:")).collect();
    print_failures("PARSE", &failures);
    let passed = results.iter().filter(|r| r["failures"] == 0).count();
    println!("\nPARSE: {passed}/{} cases fully pass; {} hard failures, {} gap/soft notes",
             results.len(), hard.len(), failures.len() - hard.len());
    write_out("parse_results.json", &json!({"cases": results, "failures": failures_json(&failures)}));
    assert!(hard.is_empty(), "{} hard parse failures (see above / verification/out/parse_results.json)", hard.len());
}

fn check_case(c: &Value, fig: &Figure, diags: &[Diagnostic], fails: &mut Vec<(String, Value, Value)>) {
    let e = &c["expect"];
    let figv = serde_json::to_value(fig).unwrap();
    let items = figv["items"].as_array().cloned().unwrap_or_default();
    let get = |kind: &str| -> Vec<Value> {
        items.iter().filter_map(|it| it.get(kind).cloned()).collect()
    };
    let nodes_items = get("Node");
    let arrows = get("Arrow");
    let blobs = get("Blob");
    let ngons = get("NGon");
    let cayleys = get("Cayley");
    let texts = get("Text");

    // ---------------------------------------------------------- nodes
    let mut other_names: Vec<String> = Vec::new();
    for b in &blobs {
        other_names.push(b["name"].as_str().unwrap_or("").to_string());
        for el in b["elements"].as_array().cloned().unwrap_or_default() {
            other_names.push(el["name"].as_str().unwrap_or("").to_string());
        }
    }
    for g in &ngons { other_names.push(g["name"].as_str().unwrap_or("").to_string()); }
    for g in &cayleys { other_names.push(g["name"].as_str().unwrap_or("").to_string()); }
    let is_other = |n: &str| {
        other_names.iter().any(|o| tex_eq(o, n))
            || ngons.iter().chain(cayleys.iter()).any(|g| n.starts_with(&format!("{}.", g["name"].as_str().unwrap_or("\u{0}"))))
    };
    // name -> (label, pos)
    let mut nodes: Vec<(String, String, Value)> = Vec::new();
    for n in &nodes_items {
        let name = n["name"].as_str().unwrap_or("").to_string();
        if !nodes.iter().any(|(m, _, _)| tex_eq(m, &name)) {
            nodes.push((name, n["label"].as_str().unwrap_or("").to_string(), n["pos"].clone()));
        }
    }
    for a in &arrows {
        for end in ["from", "to"] {
            let name = a[end].as_str().unwrap_or("").to_string();
            if !is_other(&name) && !nodes.iter().any(|(m, _, _)| tex_eq(m, &name)) {
                nodes.push((name.clone(), name, Value::Null));
            }
        }
    }
    if let Some(want) = e.get("node_count") {
        let n = nodes.len();
        if n as i64 != want.as_i64().unwrap() {
            fails.push(("node_count".into(), want.clone(), json!({"count": n, "nodes": nodes.iter().map(|x| x.0.clone()).collect::<Vec<_>>()})));
        }
    }
    for want in e["nodes"].as_array().cloned().unwrap_or_default() {
        let wname = want["name"].as_str().unwrap();
        match nodes.iter().find(|(m, _, _)| tex_eq(m, wname)) {
            None => fails.push((format!("node {wname}"), want.clone(), json!({"nodes": nodes.iter().map(|x| x.0.clone()).collect::<Vec<_>>()}))),
            Some((name, label, pos)) => {
                if let Some(wl) = want.get("label") {
                    if !matches(wl, &json!(label)) {
                        fails.push((format!("node {wname} label"), wl.clone(), json!({"name": name, "label": label})));
                    }
                }
                if let Some(wp) = want.get("pos") {
                    if !matches(wp, pos) {
                        fails.push((format!("node {wname} pos"), wp.clone(), pos.clone()));
                    }
                }
            }
        }
    }
    if let Some(cells) = e.get("grid_cells").and_then(|v| v.as_object()) {
        let mut origin: Option<(f64, f64, f64, f64)> = None;
        for (name, want) in cells {
            let wc = (want[0].as_f64().unwrap(), want[1].as_f64().unwrap());
            let got = nodes.iter().find(|(m, _, _)| tex_eq(m, name)).map(|x| x.2.clone());
            let cell = got.as_ref().and_then(|p| p.get("Cell")).map(|c| (c[0].as_f64().unwrap(), c[1].as_f64().unwrap()));
            match cell {
                None => fails.push((format!("grid cell {name}"), want.clone(), got.unwrap_or(json!("no such node")))),
                Some(gc) => match origin {
                    None => origin = Some((wc.0, wc.1, gc.0, gc.1)),
                    Some((w0, w1, g0, g1)) => {
                        if ((gc.0 - g0) - (wc.0 - w0)).abs() > 1e-3 || ((gc.1 - g1) - (wc.1 - w1)).abs() > 1e-3 {
                            fails.push((format!("grid cell {name} (relative to first)"), want.clone(),
                                        json!({"cell": [gc.0, gc.1], "first_node_cell": [g0, g1], "all": nodes.iter().map(|x| json!([x.0, x.2])).collect::<Vec<_>>()})));
                        }
                    }
                },
            }
        }
    }
    if let Some(want) = e.get("node_cells_distinct") {
        let mut cells: Vec<(i64, i64)> = nodes.iter().filter_map(|(_, _, p)| p.get("Cell").map(|c| ((c[0].as_f64().unwrap() * 100.0) as i64, (c[1].as_f64().unwrap() * 100.0) as i64))).collect();
        cells.sort();
        cells.dedup();
        if cells.len() as i64 != want.as_i64().unwrap() {
            fails.push(("distinct node cells".into(), want.clone(), json!({"distinct": cells.len(), "nodes": nodes.iter().map(|x| json!([x.0, x.2])).collect::<Vec<_>>()})));
        }
    }
    if let Some(want) = e.get("grid_blocks") {
        let n = items.iter().filter(|it| it.get("Grid").is_some()).count();
        if n as i64 != want.as_i64().unwrap() { fails.push(("grid blocks".into(), want.clone(), json!(n))); }
    }

    // ---------------------------------------------------------- arrows
    if let Some(want) = e.get("arrows") {
        match_arrows(want.as_array().unwrap(), &arrows, true, fails);
    }
    if let Some(want) = e.get("arrows_include") {
        match_arrows(want.as_array().unwrap(), &arrows, false, fails);
    }
    if let Some(want) = e.get("arrows_count") {
        if arrows.len() as i64 != want.as_i64().unwrap() {
            fails.push(("arrow count".into(), want.clone(), json!({"count": arrows.len(), "arrows": arrows.iter().map(arrow_brief).collect::<Vec<_>>()})));
        }
    }

    // ---------------------------------------------------------- blobs
    for want in e["blobs"].as_array().cloned().unwrap_or_default() {
        let wname = want["name"].as_str().unwrap();
        let Some(b) = blobs.iter().find(|b| tex_eq(b["name"].as_str().unwrap_or(""), wname)) else {
            fails.push((format!("blob {wname}"), want.clone(), json!({"blobs": blobs.iter().map(|b| b["name"].clone()).collect::<Vec<_>>()})));
            continue;
        };
        let elems: Vec<Value> = b["elements"].as_array().cloned().unwrap_or_default();
        let names: Vec<Value> = elems.iter().map(|x| x["name"].clone()).collect();
        let labels: Vec<Value> = elems.iter().map(|x| x["label"].clone()).collect();
        for (k, w) in want.as_object().unwrap() {
            let got = match k.as_str() {
                "name" => continue,
                "elements" => json!(names),
                "element_labels" => json!(labels),
                "elements_include" => {
                    for x in w.as_array().unwrap() {
                        if !names.iter().any(|n| matches(x, n)) {
                            fails.push((format!("blob {wname} contains {x}"), w.clone(), json!(names)));
                        }
                    }
                    continue;
                }
                _ => b.get(k).cloned().unwrap_or(Value::Null),
            };
            if !matches(w, &got) {
                fails.push((format!("blob {wname} {k}"), w.clone(), got));
            }
        }
    }
    if e.get("unique_elements").is_some() {
        let mut all: Vec<String> = Vec::new();
        for b in &blobs {
            for el in b["elements"].as_array().cloned().unwrap_or_default() {
                all.push(norm_key(el["name"].as_str().unwrap_or("")));
            }
        }
        let mut s = all.clone();
        s.sort();
        s.dedup();
        if s.len() != all.len() {
            fails.push(("element names unique across blobs (FIGURES.md: 'must be unique in the figure')".into(),
                        json!("each element once"), json!(blobs.iter().map(|b| json!([b["name"], b["elements"].as_array().map(|v| v.iter().map(|x| x["name"].clone()).collect::<Vec<_>>())])).collect::<Vec<_>>())));
        }
    }

    // ---------------------------------------------------------- ngons
    for want in e["ngons"].as_array().cloned().unwrap_or_default() {
        let wname = want["name"].as_str().unwrap();
        let Some(g) = ngons.iter().find(|g| g["name"].as_str() == Some(wname)) else {
            fails.push((format!("ngon {wname}"), want.clone(), json!({"ngons": ngons.iter().map(|g| g["name"].clone()).collect::<Vec<_>>()})));
            continue;
        };
        for (k, w) in want.as_object().unwrap() {
            if ["name", "slots", "axis_dirs"].contains(&k.as_str()) { continue; }
            let got = g.get(k).cloned().unwrap_or(Value::Null);
            if !matches(w, &got) {
                fails.push((format!("ngon {wname} {k}"), w.clone(), got));
            }
        }
    }

    // ---------------------------------------------------------- cayley
    let wc = e["cayleys"].as_array().cloned().unwrap_or_default();
    if !wc.is_empty() && wc.len() != cayleys.len() {
        fails.push(("cayley count".into(), json!(wc.len()), json!(cayleys)));
    }
    for (want, got) in wc.iter().zip(cayleys.iter()) {
        for (k, w) in want.as_object().unwrap() {
            if k == "group_key" { continue; }
            let g = got.get(k).cloned().unwrap_or(Value::Null);
            if !matches(w, &g) {
                fails.push((format!("cayley {k}"), w.clone(), g));
            }
        }
    }

    // ---------------------------------------------------------- text, globals, comments
    for (i, want) in e["texts"].as_array().cloned().unwrap_or_default().iter().enumerate() {
        let Some(t) = texts.get(i) else {
            fails.push((format!("text #{i}"), want.clone(), json!(texts)));
            continue;
        };
        let s = t["text"].as_str().unwrap_or("");
        for frag in want["contains"].as_array().unwrap() {
            if !s.contains(frag.as_str().unwrap()) {
                fails.push((format!("text #{i} contains {frag}"), frag.clone(), json!(s)));
            }
        }
        if let Some(wp) = want.get("pos") {
            if !matches(wp, &t["pos"]) { fails.push((format!("text #{i} pos"), wp.clone(), t["pos"].clone())); }
        }
    }
    if let Some(g) = e.get("globals").and_then(|v| v.as_object()) {
        for (k, w) in g {
            let got = figv["globals"].get(k).cloned().unwrap_or(Value::Null);
            if !matches(w, &got) { fails.push((format!("globals.{k}"), w.clone(), got)); }
        }
    }
    if let Some(want) = e.get("min_comments") {
        let n = items.iter().filter(|it| it.get("Comment").is_some()).count();
        if (n as i64) < want.as_i64().unwrap() {
            fails.push(("comments kept as items".into(), want.clone(), json!(n)));
        }
    }

    if any_string(&figv["items"], &|s| s.contains('\r') || s.contains('\t')) {
        fails.push(("no CR/TAB inside names or labels".into(), json!("clean strings"), figv["items"].clone()));
    }

    // ---------------------------------------------------------- diagnostics
    check_diags(&c["diag"], diags, fails);
}

fn match_arrows(want: &[Value], got: &[Value], exact: bool, fails: &mut Vec<(String, Value, Value)>) {
    let mut used = vec![false; got.len()];
    for w in want {
        let hit = (0..got.len()).find(|&i| !used[i] && arrow_matches(w, &got[i]));
        match hit {
            Some(i) => used[i] = true,
            None => {
                // closest: same endpoints (either direction), else everything
                let close: Vec<Value> = got.iter().filter(|a| {
                    (tex_eq(a["from"].as_str().unwrap_or(""), w["from"].as_str().unwrap()) && tex_eq(a["to"].as_str().unwrap_or(""), w["to"].as_str().unwrap()))
                        || (tex_eq(a["to"].as_str().unwrap_or(""), w["from"].as_str().unwrap()) && tex_eq(a["from"].as_str().unwrap_or(""), w["to"].as_str().unwrap()))
                }).map(|a| arrow_diff(w, a)).collect();
                let actual = if close.is_empty() { json!({"no arrow between these; arrows are": got.iter().map(arrow_brief).collect::<Vec<_>>()}) } else { json!(close) };
                fails.push((format!("arrow {} → {}", w["from"].as_str().unwrap(), w["to"].as_str().unwrap()), w.clone(), actual));
            }
        }
    }
    if exact {
        let extra: Vec<Value> = got.iter().zip(used.iter()).filter(|(_, u)| !**u).map(|(a, _)| arrow_brief(a)).collect();
        if !extra.is_empty() {
            fails.push(("unexpected extra arrows".into(), json!(want.len()), json!(extra)));
        }
    }
}

fn arrow_matches(w: &Value, a: &Value) -> bool {
    w.as_object().unwrap().iter().all(|(k, v)| matches(v, &a.get(k).cloned().unwrap_or(Value::Null)))
}

fn arrow_diff(w: &Value, a: &Value) -> Value {
    let mut d = serde_json::Map::new();
    for (k, v) in w.as_object().unwrap() {
        let g = a.get(k).cloned().unwrap_or(Value::Null);
        if !matches(v, &g) { d.insert(k.clone(), g); }
    }
    json!({"differs": d})
}

fn arrow_brief(a: &Value) -> Value {
    json!(format!("{} -[{}|{}|{}]-> {}", a["from"].as_str().unwrap_or("?"), a["label"].as_str().unwrap_or(""),
                  a["head"].as_str().unwrap_or("?"), a["line"].as_str().unwrap_or("?"), a["to"].as_str().unwrap_or("?")))
}

/// FIGURES.md's Unicode table, straight at unicode::to_latex (TeX-token equality).
#[test]
fn adversary_unicode_table() {
    let cases: &[(&str, &str)] = &[
        ("φ(G) ≅ ℤ/nℤ", "\\varphi(G) \\cong \\mathbb{Z}/n\\mathbb{Z}"),
        ("ε", "\\varepsilon"), ("φx", "\\varphi x"), ("πr", "\\pi r"), ("Φ", "\\Phi"),
        ("ℚ ℝ ℂ ℕ", "\\mathbb{Q} \\mathbb{R} \\mathbb{C} \\mathbb{N}"),
        ("f∘g", "f\\circ g"), ("G×H", "G\\times H"), ("A⊗B", "A\\otimes B"), ("A⊕B", "A\\oplus B"),
        ("H⊂G", "H\\subset G"), ("H⊆G", "H\\subseteq G"), ("g∈G", "g\\in G"), ("g∉H", "g\\notin H"),
        ("A∩B", "A\\cap B"), ("A∪B", "A\\cup B"), ("∅", "\\emptyset"), ("∞", "\\infty"),
        ("a≤b", "a\\leq b"), ("a≥b", "a\\geq b"), ("a≠b", "a\\neq b"), ("x↦y", "x\\mapsto y"), ("A→B", "A\\to B"),
        ("x₁", "x_{1}"), ("g²", "g^{2}"), ("x₁₂", "x_{12}"), ("g³²", "g^{32}"), ("x₀", "x_{0}"), ("g⁻¹", "g^{-1}"),
        ("\\mathbb{Z}", "\\mathbb{Z}"), ("plain", "plain"),
    ];
    let mut bad = Vec::new();
    for (inp, want) in cases {
        match catch(|| ntex_figures::unicode::to_latex(inp)) {
            Ok(got) => if !tex_eq(want, &got) { bad.push(format!("{inp:?} → expected {want:?}, got {got:?}")); },
            Err(p) => bad.push(format!("{inp:?} panicked: {p}")),
        }
    }
    for b in &bad { println!("[UNICODE] {b}"); }
    // g⁻¹ (superscript minus) is beyond the table's 'digits': reported, not failed
    let hard: Vec<&String> = bad.iter().filter(|b| !b.contains("g⁻¹")).collect();
    assert!(hard.is_empty(), "{} unicode failures", hard.len());
}
