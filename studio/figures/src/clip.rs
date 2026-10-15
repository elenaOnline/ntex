//! (figure-qol class A) Cloning objects: ⌥-drag, ⌘D and ⌘C/⌘V all come down
//! to `copy_objects`, which writes copies of some objects of `src` into
//! `dst` (the same figure, or another one) with fresh names, shifted by an
//! offset, with the arrows that run between the copied objects.
//!
//! The clipboard holds the copied objects as `.fig` shorthand (`to_shorthand`),
//! so it pastes into another window or into Neovim; `from_shorthand` reads
//! such text back (and refuses text that isn't shorthand).
use crate::edit::{names, normalize, primed};
use crate::format;
use crate::layout;
use crate::model::*;
use crate::parse::{self, canon};
use std::collections::{HashMap, HashSet};

/// What `copy_objects` made.
#[derive(Clone, Debug, Default)]
pub struct Cloned {
    /// Each object asked for → its copy in `dst` (arrows included: an arrow
    /// maps to its copy when both its ends were copied).
    pub pairs: Vec<(ObjRef, ObjRef)>,
    /// Every arrow copied because it runs between copied objects.
    pub arrows: Vec<ObjRef>,
    /// Arrows asked for whose ends were not both copied (not copied).
    pub skipped_arrows: usize,
}

/// The top-level objects of a figure: what ⌘A would take (arrows included).
pub fn all_objects(fig: &Figure) -> Vec<ObjRef> {
    let mut out = Vec::new();
    for (i, it) in fig.items.iter().enumerate() {
        match it {
            Item::Node(n) => out.push(ObjRef::Node(n.name.clone())),
            Item::Blob(b) => out.push(ObjRef::Blob(b.name.clone())),
            Item::NGon(g) => out.push(ObjRef::NGon(g.name.clone())),
            Item::Cayley(c) => out.push(ObjRef::Cayley(c.name.clone())),
            Item::Text(_) => out.push(ObjRef::Text(i)),
            Item::Arrow(_) => out.push(ObjRef::Arrow(i)),
            Item::Grid { .. } | Item::Comment { .. } => {}
        }
    }
    out
}

fn two(v: f32) -> f32 {
    (v * 100.0).round() / 100.0 + 0.0
}

/// A name for a copy: the old one when asked and free, else a fresh one.
fn fresh_node(taken: &HashSet<String>) -> String {
    for k in 0.. {
        for c in 'A'..='Z' {
            let n = if k == 0 { c.to_string() } else { format!("{c}{k}") };
            if !taken.contains(&canon(&n)) {
                return n;
            }
        }
    }
    unreachable!()
}

fn fresh_ngon(taken: &HashSet<String>, dotted: &[String], old: &str) -> String {
    // `P` → `P2`, `P3`, …; `P2` → `P3`
    let digits = old.trim_end_matches(|c: char| c.is_ascii_digit());
    let (base, start) = if digits.len() < old.len() && !digits.is_empty() { (digits.to_string(), old[digits.len()..].parse::<u64>().unwrap_or(1).saturating_add(1)) } else { (old.to_string(), 2) };
    (start..=u64::MAX)
        .map(|k| format!("{base}{k}"))
        .find(|c| !taken.contains(&canon(c)) && !dotted.iter().any(|d| d.starts_with(&format!("{c}."))))
        .unwrap_or_else(|| fresh_node(taken))
}

/// `key` of a spelling record with the names of `map` renamed (None when it
/// names something that wasn't copied).
fn rename_key(key: &str, map: &HashMap<String, String>) -> Option<String> {
    let (kind, rest) = key.split_once(':')?;
    let end = |s: &str| -> Option<String> {
        match s.split_once('.') {
            Some((a, b)) => map.get(a).map(|n| format!("{n}.{b}")),
            None => map.get(s).cloned(),
        }
    };
    match kind {
        "node" | "blob" | "elem" | "group" => Some(format!("{kind}:{}", map.get(rest)?)),
        "ngon" | "cayley" | "rel" => {
            let (name, tail) = rest.split_once('.')?;
            Some(format!("{kind}:{}.{tail}", map.get(name)?))
        }
        "arrow" => {
            let (a, b) = rest.split_once('>')?;
            Some(format!("arrow:{}>{}", end(a)?, end(b)?))
        }
        _ => None,
    }
}

/// Copy `objs` of `src` into `dst`, `offset` (cm, as drawn: the scene's
/// units) away, with fresh names (`keep_names`: the old name wherever it is
/// still free in `dst`, for a clipboard or a paste into another figure).
/// A blob brings its elements and the blobs inside it; the arrows between
/// the copied objects come along. Everything the copy needs of the
/// original's look, her label spellings included, is carried.
pub fn copy_objects(dst: &mut Figure, src: &Figure, objs: &[ObjRef], offset: Pt, keep_names: bool) -> Result<Cloned, String> {
    let scene = layout::layout(src);
    let src_scale = if src.globals.scale > 0.0 { src.globals.scale } else { 1.0 };
    let dst_scale = if dst.globals.scale > 0.0 { dst.globals.scale } else { 1.0 };
    let src_cell = if scene.cell.0 > 0.0 && scene.cell.1 > 0.0 { scene.cell } else { (DEFAULT_SPACING.0 * src_scale, DEFAULT_SPACING.1 * src_scale) };
    let dsp = if dst.globals.spacing.0 > 0.0 && dst.globals.spacing.1 > 0.0 { dst.globals.spacing } else { DEFAULT_SPACING };
    let dst_cell = (dsp.0 * dst_scale, dsp.1 * dst_scale);

    // -- what is copied
    // (sets of canonical names: copying thousands of objects stays linear)
    let mut want_nodes: HashSet<String> = HashSet::new();
    let mut want_blobs: HashSet<String> = HashSet::new();
    let mut want_ngons: HashSet<String> = HashSet::new();
    let mut want_cayleys: HashSet<String> = HashSet::new();
    let mut want_texts: Vec<usize> = Vec::new();
    let mut seen_texts: HashSet<usize> = HashSet::new();
    let mut want_elems: HashSet<String> = HashSet::new();
    let mut want_arrows: HashSet<usize> = HashSet::new();
    let push = |v: &mut HashSet<String>, n: &String| {
        v.insert(canon(n));
    };
    for o in objs {
        match o {
            ObjRef::Node(n) => push(&mut want_nodes, n),
            ObjRef::Blob(n) => push(&mut want_blobs, n),
            ObjRef::NGon(n) | ObjRef::NGonVertex(n, _) | ObjRef::NGonRotation(n) | ObjRef::NGonAxis(n, _) | ObjRef::NGonRotationLabel(n) => push(&mut want_ngons, n),
            ObjRef::Cayley(n) | ObjRef::CayleyVertex(n, _) | ObjRef::CayleyEdge(n, _) => push(&mut want_cayleys, n),
            ObjRef::Text(i) => {
                if seen_texts.insert(*i) {
                    want_texts.push(*i)
                }
            }
            ObjRef::Element(n) => push(&mut want_elems, n),
            ObjRef::Arrow(i) => {
                want_arrows.insert(*i);
            }
        }
    }
    // the blobs inside a copied blob go with it
    loop {
        let before = want_blobs.len();
        for it in &src.items {
            if let Item::Blob(b) = it
                && let Some(p) = &b.parent
                && want_blobs.contains(&canon(p))
            {
                want_blobs.insert(canon(&b.name));
            }
        }
        if want_blobs.len() == before {
            break;
        }
    }
    // an element of a copied blob comes with the blob
    let with_blob: HashSet<String> = src
        .items
        .iter()
        .filter_map(|it| match it {
            Item::Blob(b) if want_blobs.contains(&canon(&b.name)) => Some(b.elements.iter().map(|x| canon(&x.name))),
            _ => None,
        })
        .flatten()
        .collect();
    want_elems.retain(|e| !with_blob.contains(e));
    let has = |v: &HashSet<String>, n: &str| v.contains(&canon(n));
    let dst_blobs: HashSet<String> = dst.items.iter().filter_map(|it| if let Item::Blob(b) = it { Some(canon(&b.name)) } else { None }).collect();

    // -- names
    let mut taken = names(dst);
    let dotted: Vec<String> = dst.items.iter().filter_map(|it| if let Item::Node(n) = it { Some(n.name.clone()) } else { None }).collect();
    let mut map: HashMap<String, String> = HashMap::new();
    let alloc = |old: &str, kind: char, taken: &mut HashSet<String>| -> String {
        let new = if keep_names && !taken.contains(&canon(old)) {
            old.to_string()
        } else {
            match kind {
                'n' => fresh_node(taken),
                'g' => fresh_ngon(taken, &dotted, old),
                _ => primed(taken, old),
            }
        };
        taken.insert(canon(&new));
        new
    };
    for it in &src.items {
        match it {
            Item::Node(n) if has(&want_nodes, &n.name) => {
                let new = alloc(&n.name, 'n', &mut taken);
                map.insert(n.name.clone(), new);
            }
            Item::Blob(b) => {
                if has(&want_blobs, &b.name) {
                    let new = alloc(&b.name, 'b', &mut taken);
                    map.insert(b.name.clone(), new);
                    for e in &b.elements {
                        let en = alloc(&e.name, 'b', &mut taken);
                        map.insert(e.name.clone(), en);
                    }
                } else {
                    for e in b.elements.iter().filter(|e| has(&want_elems, &e.name)) {
                        let en = alloc(&e.name, 'b', &mut taken);
                        map.insert(e.name.clone(), en);
                    }
                }
            }
            Item::NGon(g) if has(&want_ngons, &g.name) => {
                let new = alloc(&g.name, 'g', &mut taken);
                map.insert(g.name.clone(), new);
            }
            Item::Cayley(c) if has(&want_cayleys, &c.name) => {
                let new = alloc(&c.name, 'c', &mut taken);
                map.insert(c.name.clone(), new);
            }
            _ => {}
        }
    }

    // -- positions
    let shift = |p: Option<Pos>, at: Option<Pt>| -> Option<Pos> {
        match p {
            Some(Pos::Cell(c, r)) => Some(Pos::Cell(two(c + offset.x / dst_cell.0), two(r - offset.y / dst_cell.1))),
            Some(Pos::Abs(x, y)) => Some(Pos::Abs(two(x + offset.x / dst_scale), two(y + offset.y / dst_scale))),
            None => at.map(|q| Pos::Abs(two(q.x / src_scale + offset.x / dst_scale), two(q.y / src_scale + offset.y / dst_scale))),
        }
    };
    let mut centres: HashMap<&ObjRef, Pt> = HashMap::new();
    for s in &scene.shapes {
        centres.entry(&s.obj).or_insert(s.centre);
    }
    let mut anchors: HashMap<&ObjRef, Pt> = HashMap::new();
    for (o, p, _) in &scene.anchors {
        anchors.entry(o).or_insert(*p);
    }
    let frame_centre = |o: ObjRef| centres.get(&o).copied();
    let anchor = |o: ObjRef| anchors.get(&o).copied();
    let root: HashSet<&ObjRef> = objs.iter().collect();
    let root_has = |o: &ObjRef| root.contains(o);
    let label_for = |old_label: &str, old: &str, new: &str| -> String {
        if old_label == parse::default_label(old) { parse::default_label(new) } else { old_label.to_string() }
    };

    // -- the copies
    let mut pairs: Vec<(ObjRef, ObjRef)> = Vec::new();
    let mut new_items: Vec<Item> = Vec::new();
    let mut elem_adds: Vec<(String, Element)> = Vec::new();
    for it in &src.items {
        match it {
            Item::Node(n) if has(&want_nodes, &n.name) => {
                let new = map[&n.name].clone();
                let pos = match n.pos {
                    Some(_) => shift(n.pos, None),
                    None => {
                        // auto-placed: pinned to the cell it is drawn in, then moved
                        let p = anchor(ObjRef::Node(n.name.clone())).unwrap_or_default();
                        let (c, r) = ((p.x / src_cell.0).round(), (-p.y / src_cell.1).round());
                        Some(Pos::Cell(two(c + offset.x / dst_cell.0), two(r - offset.y / dst_cell.1)))
                    }
                };
                new_items.push(Item::Node(Node { name: new.clone(), label: n.label.clone(), pos, color: n.color.clone(), span: Span::default() }));
                if root_has(&ObjRef::Node(n.name.clone())) {
                    pairs.push((ObjRef::Node(n.name.clone()), ObjRef::Node(new)));
                }
            }
            Item::Blob(b) if has(&want_blobs, &b.name) => {
                let new = map[&b.name].clone();
                let mut c = b.clone();
                c.name = new.clone();
                c.label = label_for(&b.label, &b.name, &new);
                // `in P` stays only where P exists in the copy's figure (copied
                // along, or already there): shorthand naming a missing blob
                // can't be pasted.
                c.parent = b.parent.as_ref().and_then(|p| map.get(p).cloned().or_else(|| dst_blobs.contains(&canon(p)).then(|| p.clone())));
                c.pos = shift(b.pos, frame_centre(ObjRef::Blob(b.name.clone())));
                c.span = Span::default();
                for e in c.elements.iter_mut() {
                    let en = map[&e.name].clone();
                    e.label = label_for(&e.label, &e.name, &en);
                    // a placed point goes with its blob; the others spread inside it
                    e.pos = shift(e.pos, None);
                    e.name = en;
                }
                if root_has(&ObjRef::Blob(b.name.clone())) {
                    pairs.push((ObjRef::Blob(b.name.clone()), ObjRef::Blob(new)));
                }
                new_items.push(Item::Blob(c));
            }
            Item::Blob(b) => {
                // elements asked for on their own: a new element in the same blob
                for e in b.elements.iter().filter(|e| has(&want_elems, &e.name)) {
                    let en = map[&e.name].clone();
                    let label = label_for(&e.label, &e.name, &en);
                    let pos = shift(e.pos, anchor(ObjRef::Element(e.name.clone())));
                    pairs.push((ObjRef::Element(e.name.clone()), ObjRef::Element(en.clone())));
                    elem_adds.push((b.name.clone(), Element { name: en, label, pos }));
                }
            }
            Item::NGon(g) if has(&want_ngons, &g.name) => {
                let new = map[&g.name].clone();
                let mut c = g.clone();
                c.name = new.clone();
                c.pos = shift(g.pos, frame_centre(ObjRef::NGon(g.name.clone())));
                c.span = Span::default();
                pairs.push((ObjRef::NGon(g.name.clone()), ObjRef::NGon(new)));
                new_items.push(Item::NGon(c));
            }
            Item::Cayley(cy) if has(&want_cayleys, &cy.name) => {
                let new = map[&cy.name].clone();
                let mut c = cy.clone();
                c.name = new.clone();
                c.pos = shift(cy.pos, frame_centre(ObjRef::Cayley(cy.name.clone())));
                c.span = Span::default();
                pairs.push((ObjRef::Cayley(cy.name.clone()), ObjRef::Cayley(new)));
                new_items.push(Item::Cayley(c));
            }
            _ => {}
        }
    }
    let mut text_pairs: Vec<(usize, Text)> = Vec::new();
    for &i in &want_texts {
        if let Some(Item::Text(t)) = src.items.get(i) {
            let pos = shift(Some(t.pos), None).unwrap_or(t.pos);
            text_pairs.push((i, Text { pos, span: Span::default(), ..t.clone() }));
        }
    }
    // arrows between copied objects
    let end = |s: &str| -> Option<String> {
        if let Some(n) = map.get(s) {
            return Some(n.clone());
        }
        let (a, b) = s.split_once('.')?;
        map.get(a).map(|n| format!("{n}.{b}"))
    };
    let mut arrow_items: Vec<(usize, Arrow, String)> = Vec::new();
    let mut skipped = 0;
    for (i, it) in src.items.iter().enumerate() {
        let Item::Arrow(a) = it else { continue };
        match (end(&a.from), end(&a.to)) {
            (Some(f), Some(t)) => {
                let mut c = a.clone();
                c.from = f;
                c.to = t;
                c.span = Span::default();
                arrow_items.push((i, c, crate::spelling::arrow_key(&a.from, &a.to)));
            }
            _ => {
                if want_arrows.contains(&i) {
                    skipped += 1;
                }
            }
        }
    }

    // -- write them
    let mut out = Cloned::default();
    let sp_id = |s: &Spelling| (s.key.clone(), s.latex.clone(), s.raw.clone());
    let mut have_sp: HashSet<(String, String, String)> = dst.hints.spelling.iter().map(sp_id).collect();
    let mut by_key: HashMap<&str, Vec<&Spelling>> = HashMap::new();
    for s in &src.hints.spelling {
        by_key.entry(s.key.as_str()).or_default().push(s);
    }
    dst.items.extend(new_items);
    for (owner, e) in elem_adds {
        match dst.items.iter_mut().find_map(|x| match x {
            Item::Blob(t) if canon(&t.name) == canon(&owner) => Some(t),
            _ => None,
        }) {
            Some(t) => t.elements.push(e),
            None => return Err(format!("there is no blob `{owner}` to put the element in")),
        }
    }
    out.pairs = pairs;
    for (i, t) in text_pairs {
        dst.items.push(Item::Text(t));
        out.pairs.push((ObjRef::Text(i), ObjRef::Text(dst.items.len() - 1)));
        // (free text is spelled by its LaTeX alone)
        for s in src.hints.spelling.iter().filter(|s| s.key == "text") {
            if matches!(src.items.get(i), Some(Item::Text(t)) if t.text == s.latex) && have_sp.insert(sp_id(s)) {
                dst.hints.spelling.push(s.clone());
            }
        }
    }
    for (i, a, old_key) in arrow_items {
        let new_key = crate::spelling::arrow_key(&a.from, &a.to);
        if let Some(l) = &a.label {
            for s in by_key.get(old_key.as_str()).into_iter().flatten().filter(|s| &s.latex == l) {
                let n = Spelling { key: new_key.clone(), latex: s.latex.clone(), raw: s.raw.clone() };
                if have_sp.insert(sp_id(&n)) {
                    dst.hints.spelling.push(n);
                }
            }
        }
        dst.items.push(Item::Arrow(a));
        let new = ObjRef::Arrow(dst.items.len() - 1);
        out.arrows.push(new.clone());
        if want_arrows.contains(&i) {
            out.pairs.push((ObjRef::Arrow(i), new));
        }
    }
    // her spelling of the copied labels
    for s in &src.hints.spelling {
        if let Some(k) = rename_key(&s.key, &map) {
            let n = Spelling { key: k, latex: s.latex.clone(), raw: s.raw.clone() };
            if have_sp.insert(sp_id(&n)) {
                dst.hints.spelling.push(n);
            }
        }
    }
    out.skipped_arrows = skipped;
    normalize(dst);
    Ok(out)
}

/// The shorthand for `objs` of `fig` (what ⌘C puts on the clipboard): the
/// objects with their names, positions and looks, and the arrows between
/// them. None when there is nothing to copy.
pub fn to_shorthand(fig: &Figure, objs: &[ObjRef]) -> Option<String> {
    let mut dst = Figure::default();
    copy_objects(&mut dst, fig, objs, Pt::default(), true).ok()?;
    if dst.items.is_empty() {
        return None;
    }
    let s = format::format(&dst);
    (!s.trim().is_empty()).then_some(s)
}

/// The deepest `{…}` nesting a paste accepts.
pub const MAX_NESTING: usize = 24;

/// Shorthand text (from the clipboard) as a figure, or why it isn't one.
/// Text that doesn't parse cleanly, or that holds no objects, is refused.
pub fn from_shorthand(text: &str) -> Result<Figure, String> {
    if text.len() > 400_000 {
        return Err("that text is too long to be a figure".into());
    }
    if text.trim().is_empty() {
        return Err("the clipboard is empty".into());
    }
    // braces nested deeper than any label needs: not a figure (and the
    // layout shouldn't be asked to measure it)
    let (mut depth, mut deepest) = (0usize, 0usize);
    for c in text.chars() {
        match c {
            '{' => {
                depth += 1;
                deepest = deepest.max(depth);
            }
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    if deepest > MAX_NESTING {
        return Err(format!("that text nests braces {deepest} deep; figure shorthand never needs more than {MAX_NESTING}"));
    }
    let (fig, diags) = parse::parse(text);
    if let Some(d) = diags.iter().find(|d| d.severity != Severity::Note) {
        return Err(format!("that text isn't figure shorthand ({})", d.message));
    }
    if all_objects(&fig).is_empty() {
        return Err("that text holds no figure objects".into());
    }
    Ok(fig)
}

/// Paste shorthand into `dst`: every object of the text, `offset` cm from
/// where it was written, with fresh names where its names are taken.
pub fn paste_into(dst: &mut Figure, text: &str, offset: Pt) -> Result<Cloned, String> {
    let src = from_shorthand(text)?;
    paste_parsed(dst, &src, offset)
}

/// `paste_into` for shorthand already read by `from_shorthand`.
pub fn paste_parsed(dst: &mut Figure, src: &Figure, offset: Pt) -> Result<Cloned, String> {
    let objs = all_objects(src);
    let mut got = copy_objects(dst, src, &objs, offset, true)?;
    got.pairs.retain(|(_, n)| !matches!(n, ObjRef::Arrow(_)));
    Ok(got)
}
