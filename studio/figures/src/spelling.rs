//! (FIG-POLISH) Her spelling. The model stores labels as LaTeX (`φ` →
//! `\varphi`), so a rewrite of the source would otherwise trade her `φ` for
//! `\varphi`. `Hints::spelling` remembers every label typed in another
//! spelling; `format` writes that spelling back while the label is unchanged.
//!
//! A label's *site* is a (key, latex) pair: `node:A`, `arrow:A>B`, `blob:G`,
//! `elem:g`, `ngon:P.2`, `cayley:C.1` (generator 1), `rel:C.1` (relator 1),
//! `group:C` (a named group as written: `ℤ2 × ℤ4`), `text`. For each (key, latex) the first site decides, so the hints are a
//! function of the figure and the source: parse(format(fig)) gives them back.
use crate::model::*;
use crate::parse::default_label;
use std::collections::HashSet;

/// Every label site, in item order: (item index, key, latex).
pub fn sites(fig: &Figure) -> Vec<(usize, String, String)> {
    let mut v = Vec::new();
    for (i, it) in fig.items.iter().enumerate() {
        match it {
            Item::Node(n) => {
                if n.label != default_label(&n.name) {
                    v.push((i, format!("node:{}", n.name), n.label.clone()));
                }
            }
            Item::Arrow(a) => {
                if let Some(l) = &a.label {
                    v.push((i, arrow_key(&a.from, &a.to), l.clone()));
                }
            }
            Item::Blob(b) => {
                if b.label != default_label(&b.name) {
                    v.push((i, format!("blob:{}", b.name), b.label.clone()));
                }
                for e in &b.elements {
                    if e.label != default_label(&e.name) {
                        v.push((i, format!("elem:{}", e.name), e.label.clone()));
                    }
                }
            }
            Item::NGon(g) => {
                for (k, l) in g.labels.iter().enumerate() {
                    v.push((i, format!("ngon:{}.{}", g.name, k + 1), l.clone()));
                }
                // (figure-qol class C) axis labels, then the rotation arrow's
                for a in &g.axes {
                    if let Some((_, l)) = g.axis_labels.iter().find(|(c, _)| c == a) {
                        v.push((i, format!("ngon:{}.axis.{}", g.name, a), l.clone()));
                    }
                }
                for (a, l) in &g.axis_labels {
                    if !g.axes.contains(a) {
                        v.push((i, format!("ngon:{}.axis.{}", g.name, a), l.clone()));
                    }
                }
                if g.rotation > 0
                    && let Some(l) = &g.rotation_label
                {
                    v.push((i, format!("ngon:{}.rot", g.name), l.clone()));
                }
            }
            Item::Cayley(c) => {
                if let GroupSpec::Named(g) = &c.group {
                    v.push((i, format!("group:{}", c.name), g.clone()));
                }
                for (k, l) in c.gens.iter().enumerate() {
                    v.push((i, format!("cayley:{}.{}", c.name, k + 1), l.clone()));
                }
                for (w, l) in &c.vertex_names {
                    v.push((i, format!("cayley:{}.v.{w}", c.name), l.clone()));
                }
                if let GroupSpec::Presentation(_, rels) = &c.group {
                    for (k, r) in rels.iter().enumerate() {
                        v.push((i, format!("rel:{}.{}", c.name, k + 1), r.clone()));
                    }
                }
            }
            Item::Text(t) => v.push((i, "text".into(), t.text.clone())),
            Item::Grid { .. } | Item::Comment { .. } => {}
        }
    }
    v
}

pub fn arrow_key(from: &str, to: &str) -> String {
    format!("arrow:{from}>{to}")
}

/// The label as she typed it at this site, or the LaTeX.
pub fn raw(fig: &Figure, key: &str, latex: &str) -> String {
    fig.hints
        .spelling
        .iter()
        .find(|s| s.key == key && s.latex == latex)
        .map(|s| s.raw.clone())
        .unwrap_or_else(|| latex.to_string())
}

/// The label of a canvas object as she typed it (what the window's label
/// field shows for editing).
pub fn as_typed(fig: &Figure, obj: &ObjRef) -> Option<String> {
    for (i, it) in fig.items.iter().enumerate() {
        match (it, obj) {
            (Item::Node(n), ObjRef::Node(m)) if &n.name == m => {
                if n.label == default_label(&n.name) {
                    return Some(n.name.clone());
                }
                return Some(raw(fig, &format!("node:{}", n.name), &n.label));
            }
            (Item::Arrow(a), ObjRef::Arrow(k)) if i == *k => {
                return Some(a.label.as_ref().map(|l| raw(fig, &arrow_key(&a.from, &a.to), l)).unwrap_or_default());
            }
            (Item::Blob(b), ObjRef::Blob(m)) if &b.name == m => {
                if b.label == default_label(&b.name) {
                    return Some(b.name.clone());
                }
                return Some(raw(fig, &format!("blob:{}", b.name), &b.label));
            }
            (Item::Blob(b), ObjRef::Element(m)) => {
                if let Some(e) = b.elements.iter().find(|e| &e.name == m) {
                    if e.label == default_label(&e.name) {
                        return Some(e.name.clone());
                    }
                    return Some(raw(fig, &format!("elem:{}", e.name), &e.label));
                }
            }
            (Item::Text(t), ObjRef::Text(k)) if i == *k => return Some(raw(fig, "text", &t.text)),
            // (FIG-MODEL) the label drawn at vertex position k (after turn/flip)
            (Item::NGon(g), ObjRef::NGonVertex(m, k)) if &g.name == m => {
                let li = crate::layout::ngon_label_index(g, *k);
                let l = g.labels.get(li)?;
                return Some(raw(fig, &format!("ngon:{}.{}", g.name, li + 1), l));
            }
            // (figure-qol class C) the label of a part that had none to set
            (Item::NGon(g), ObjRef::NGonRotationLabel(m)) if &g.name == m && g.rotation > 0 => {
                let l = g.rotation_label.clone().unwrap_or_else(|| crate::layout::ngon_rotation_default(g));
                return Some(raw(fig, &format!("ngon:{}.rot", g.name), &l));
            }
            (Item::NGon(g), ObjRef::NGonAxis(m, code)) if &g.name == m => {
                let l = g.axis_labels.iter().find(|(c, _)| c == code).map(|(_, l)| l.clone()).unwrap_or_default();
                return Some(raw(fig, &format!("ngon:{}.axis.{}", g.name, code), &l));
            }
            (Item::Cayley(c), ObjRef::CayleyVertex(m, w)) if &c.name == m => {
                let l = c.vertex_names.iter().find(|(x, _)| x == w).map(|(_, l)| l.clone()).unwrap_or_else(|| w.clone());
                return Some(raw(fig, &format!("cayley:{}.v.{w}", c.name), &l));
            }
            _ => {}
        }
    }
    None
}

/// Remember that the label at `key` was typed as `raw` (an edit from the
/// window). `normalize` then puts the hints in canonical order.
pub fn record(fig: &mut Figure, key: &str, latex: &str, raw: &str) {
    fig.hints.spelling.retain(|s| s.key != key);
    if raw != latex && !raw.is_empty() {
        fig.hints.spelling.insert(0, Spelling { key: key.into(), latex: latex.into(), raw: raw.into() });
    }
}

/// Keep only spellings that still match a site, one per (key, latex), in
/// site order (what `parse` gives for `format`'s output).
pub fn normalize(fig: &mut Figure) {
    let old = std::mem::take(&mut fig.hints.spelling);
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut out = Vec::new();
    for (_, key, latex) in sites(fig) {
        if !seen.insert((key.clone(), latex.clone())) {
            continue;
        }
        if let Some(s) = old.iter().find(|s| s.key == key && s.latex == latex) {
            out.push(s.clone());
        }
    }
    fig.hints.spelling = out;
}

/// From the parser: each item's statement (`origin`) and each statement's
/// conversions (latex, raw) in the order they were read.
pub(crate) fn assign(fig: &Figure, pools: &[Vec<(String, String)>], origin: &[usize]) -> Vec<Spelling> {
    let mut used: Vec<Vec<bool>> = pools.iter().map(|p| vec![false; p.len()]).collect();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut out = Vec::new();
    for (i, key, latex) in sites(fig) {
        let first = seen.insert((key.clone(), latex.clone()));
        let s = origin.get(i).copied().unwrap_or(usize::MAX);
        let mut hit: Option<(usize, usize)> = None;
        if let Some(p) = pools.get(s) {
            hit = p.iter().enumerate().position(|(k, (l, _))| !used[s][k] && *l == latex).map(|k| (s, k));
        }
        if hit.is_none() {
            'outer: for (si, p) in pools.iter().enumerate() {
                for (k, (l, _)) in p.iter().enumerate() {
                    if !used[si][k] && *l == latex {
                        hit = Some((si, k));
                        break 'outer;
                    }
                }
            }
        }
        let raw = match hit {
            Some((si, k)) => {
                used[si][k] = true;
                pools[si][k].1.clone()
            }
            None => pools.iter().flatten().find(|(l, _)| *l == latex).map(|x| x.1.clone()).unwrap_or_else(|| latex.clone()),
        };
        if first && raw != latex {
            out.push(Spelling { key, latex, raw });
        }
    }
    out
}
