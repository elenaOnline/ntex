//! Canvas edits on the model (then `format` writes the source back).
//!
//! Edits keep the source close to what she wrote: new statements are
//! appended at the end, changes are made in place, and an edit that touches
//! an object of a `grid … end` block first turns that block into plain
//! statements (a comment marks where it was). After every edit the model is
//! normalised so that `parse(format(fig)) == fig` (spans aside).
use crate::format::{absorption, descendant_has};
use crate::model::*;
use crate::parse::{self, canon, is_token};
use crate::spelling;
use crate::unicode;
use std::collections::HashSet;

/// Apply an edit; returns the name / index of what it made or changed.
pub fn apply(fig: &mut Figure, edit: &Edit) -> Result<Option<String>, String> {
    let r = apply_inner(fig, edit)?;
    normalize(fig);
    Ok(r)
}

/// A fresh node name not used yet ("A", "B", … "A1").
pub fn fresh_name(fig: &Figure) -> String {
    let used = names(fig);
    for k in 0.. {
        for c in 'A'..='Z' {
            let n = if k == 0 { c.to_string() } else { format!("{c}{k}") };
            if !used.contains(&canon(&n)) {
                return n;
            }
        }
    }
    unreachable!()
}

// ---------------------------------------------------------------------------

fn apply_inner(fig: &mut Figure, edit: &Edit) -> Result<Option<String>, String> {
    match edit {
        // (FIG-MODEL, interaction pass) each works on a copy, so a refused
        // edit leaves the figure as it was
        Edit::SetItem { index, item } => atomically(fig, |g| {
            let gi = grid_of(g, *index);
            let r = set_item(g, *index, item)?;
            settle_or_convert(g, gi, "changed in the window");
            Ok(r)
        }),
        Edit::MoveObj { obj, pos } => atomically(fig, |g| move_obj(g, obj, *pos)),
        Edit::Duplicate { obj } => atomically(fig, |g| duplicate(g, obj)),
        Edit::AddNode { label: typed, pos } => {
            let (name, label) = name_from_label(fig, typed);
            spelling::record(fig, &format!("node:{name}"), &label, &clean(typed.trim()));
            fig.items.push(Item::Node(Node { name: name.clone(), label, pos: Some(*pos), color: None, span: Span::default() }));
            Ok(Some(name))
        }
        Edit::MoveNode { name, pos } => {
            if let Some(i) = node_index(fig, name) {
                touch_grid(fig, i, &format!("moved {name}"));
                if let Item::Node(n) = &mut fig.items[i] {
                    n.pos = Some(*pos);
                    return Ok(Some(n.name.clone()));
                }
            }
            let key = canon(name);
            for it in fig.items.iter_mut() {
                match it {
                    Item::Blob(b) => {
                        if canon(&b.name) == key {
                            b.pos = Some(*pos);
                            return Ok(Some(b.name.clone()));
                        }
                        if let Some(e) = b.elements.iter_mut().find(|e| canon(&e.name) == key) {
                            e.pos = Some(*pos);
                            return Ok(Some(e.name.clone()));
                        }
                    }
                    Item::NGon(g) if canon(&g.name) == key => {
                        g.pos = Some(*pos);
                        return Ok(Some(g.name.clone()));
                    }
                    Item::Cayley(c) if canon(&c.name) == key => {
                        c.pos = Some(*pos);
                        return Ok(Some(c.name.clone()));
                    }
                    _ => {}
                }
            }
            Err(format!("there is no node `{name}` to move"))
        }
        Edit::SetNodeLabel { name, label: typed } => {
            let raw = clean(typed.trim());
            let label = clean(&unicode::to_latex(typed));
            if let Some(i) = node_index(fig, name) {
                let gi = grid_of(fig, i);
                if let Item::Node(n) = &mut fig.items[i] {
                    n.label = label.clone();
                    let nm = n.name.clone();
                    spelling::record(fig, &format!("node:{nm}"), &label, &raw);
                    settle_or_convert(fig, gi, &format!("relabelled {name}"));
                    return Ok(Some(nm));
                }
            }
            let key = canon(name);
            let mut hit: Option<String> = None;
            for it in fig.items.iter_mut() {
                if let Item::Blob(b) = it {
                    if canon(&b.name) == key {
                        b.label = label.clone();
                        hit = Some(format!("blob:{}", b.name));
                        break;
                    }
                    if let Some(e) = b.elements.iter_mut().find(|e| canon(&e.name) == key) {
                        e.label = label.clone();
                        hit = Some(format!("elem:{}", e.name));
                        break;
                    }
                }
            }
            match hit {
                Some(k) => {
                    spelling::record(fig, &k, &label, &raw);
                    Ok(Some(k.split_once(':').map(|x| x.1.to_string()).unwrap_or_default()))
                }
                None => Err(format!("there is no node `{name}` to relabel")),
            }
        }
        Edit::AddArrow { from, to, label } => {
            let from = endpoint(fig, from)?;
            let to = endpoint(fig, to)?;
            let typed = label;
            let label = label.as_ref().map(|l| clean(&unicode::to_latex(l))).filter(|l| !l.is_empty());
            if let (Some(l), Some(t)) = (&label, typed) {
                spelling::record(fig, &spelling::arrow_key(&from, &to), l, &clean(t.trim()));
            }
            fig.items.push(Item::Arrow(Arrow {
                is_loop: from == to,
                from,
                to,
                label,
                head: Head::To,
                line: Line::Solid,
                bend: 0.0,
                side: LabelSide::Auto,
                label_pos: LabelPos::Middle,
                iso: false,
                thick: false,
                color: None,
                span: Span::default(),
            }));
            Ok(Some((fig.items.len() - 1).to_string()))
        }
        Edit::SetArrow { index, arrow } => {
            let Some(Item::Arrow(old)) = fig.items.get(*index) else {
                return Err(format!("item {index} isn't an arrow"));
            };
            let span = old.span;
            let mut a = arrow.clone();
            a.from = endpoint(fig, &a.from)?;
            a.to = endpoint(fig, &a.to)?;
            let typed = a.label.clone();
            a.label = a.label.map(|l| clean(&unicode::to_latex(&l))).filter(|l| !l.is_empty());
            // the label as given is her spelling (`\varphi` typed over `φ` is a choice too)
            if let (Some(l), Some(t)) = (&a.label, &typed) {
                spelling::record(fig, &spelling::arrow_key(&a.from, &a.to), l, &clean(t.trim()));
            }
            a.color = a.color.and_then(|c| parse::PALETTE.contains(&c.as_str()).then_some(c));
            a.span = span;
            let gi = grid_of(fig, *index);
            fig.items[*index] = Item::Arrow(a);
            settle_or_convert(fig, gi, "changed an arrow");
            Ok(Some(index.to_string()))
        }
        Edit::AddBlob { blob } => {
            let mut b = blob.clone();
            let used = names(fig);
            if !is_token(&b.name) || used.contains(&canon(&b.name)) {
                b.name = fresh_name(fig);
            }
            let typed_label = clean(b.label.trim());
            b.label = clean(&unicode::to_latex(&b.label));
            if b.label.is_empty() && blob.label.is_empty() {
                b.label = parse::default_label(&b.name);
            }
            if let Some(p) = &b.parent {
                if !fig.items.iter().any(|it| matches!(it, Item::Blob(x) if x.name == *p)) {
                    return Err(format!("there is no blob `{p}` to put `{}` in", b.name));
                }
            }
            let mut typed_elems: Vec<(String, String, String)> = Vec::new();
            let mut seen: HashSet<String> = used.clone();
            seen.insert(canon(&b.name));
            for e in b.elements.iter_mut() {
                if !is_token(&e.name) || seen.contains(&canon(&e.name)) {
                    return Err(format!("the element name `{}` is taken or unusable", e.name));
                }
                seen.insert(canon(&e.name));
                let t = clean(e.label.trim());
                e.label = clean(&unicode::to_latex(&e.label));
                typed_elems.push((format!("elem:{}", e.name), e.label.clone(), t));
            }
            b.color = b.color.and_then(|c| parse::PALETTE.contains(&c.as_str()).then_some(c));
            b.span = Span::default();
            let name = b.name.clone();
            spelling::record(fig, &format!("blob:{name}"), &b.label, &typed_label);
            for (k, l, t) in &typed_elems {
                spelling::record(fig, k, l, t);
            }
            fig.items.push(Item::Blob(b));
            Ok(Some(name))
        }
        Edit::MoveBlob { name, pos } => {
            for it in fig.items.iter_mut() {
                if let Item::Blob(b) = it {
                    if canon(&b.name) == canon(name) {
                        b.pos = Some(*pos);
                        return Ok(Some(b.name.clone()));
                    }
                }
            }
            Err(format!("there is no blob `{name}`"))
        }
        Edit::AddElement { blob, label: typed, pos } => {
            let (name, label) = name_from_label(fig, typed);
            let raw = clean(typed.trim());
            for it in fig.items.iter_mut() {
                if let Item::Blob(b) = it {
                    if canon(&b.name) == canon(blob) {
                        b.elements.push(Element { name: name.clone(), label: label.clone(), pos: *pos });
                        spelling::record(fig, &format!("elem:{name}"), &label, &raw);
                        return Ok(Some(name));
                    }
                }
            }
            Err(format!("there is no blob `{blob}`"))
        }
        Edit::AddNGon { n, pos, radius } => {
            if !(3..=64).contains(n) {
                return Err(format!("an n-gon needs 3 to 64 sides, not {n}"));
            }
            let used = names(fig);
            let mut cands: Vec<String> = ["P", "Q", "R", "S", "T", "U", "W"].iter().map(|s| s.to_string()).collect();
            cands.extend((1..1000).map(|k| format!("P{k}")));
            // `P` must not turn an existing node called `P.1` into a vertex
            let dotted: Vec<String> = fig.items.iter().filter_map(|it| if let Item::Node(n) = it { Some(n.name.clone()) } else { None }).collect();
            let name = cands.into_iter().find(|c| !used.contains(c) && !dotted.iter().any(|d| d.starts_with(&format!("{c}.")))).unwrap();
            fig.items.push(Item::NGon(NGon {
                angle: 0.0,
                show_labels: true,
                shaded: true,
                color: None,
                name: name.clone(),
                n: *n,
                labels: (1..=*n).map(|k| k.to_string()).collect(),
                all_axes: false,
                axes: vec![],
                rotation: 0,
                turn: 0,
                rotation_cw: false,
                rotation_edge: None,
                turn_neg: false,
                flip: None,
                radius: (*radius > 0.0 && radius.is_finite()).then_some(*radius),
                pos: Some(*pos),
                span: Span::default(),
                rotation_label: None,
                axis_labels: vec![],
            }));
            Ok(Some(name))
        }
        Edit::Delete { obj } => {
            delete(fig, obj)?;
            Ok(None)
        }
        Edit::AddText { text, pos } => {
            let t = clean(&unicode::to_latex_text(text));
            if t.trim().is_empty() {
                return Err("the text is empty".into());
            }
            let raw = clean(text);
            if raw != t && !fig.hints.spelling.iter().any(|s| s.key == "text" && s.latex == t) {
                fig.hints.spelling.push(Spelling { key: "text".into(), latex: t.clone(), raw });
            }
            fig.items.push(Item::Text(Text { text: t, pos: *pos, span: Span::default(), color: None, size: None }));
            Ok(Some((fig.items.len() - 1).to_string()))
        }
    }
}

fn atomically(fig: &mut Figure, f: impl FnOnce(&mut Figure) -> Result<Option<String>, String>) -> Result<Option<String>, String> {
    let mut g = fig.clone();
    let r = f(&mut g)?;
    *fig = g;
    Ok(r)
}

// ---------------------------------------------------------------------------
// (FIG-MODEL) SetItem, MoveObj, Duplicate

/// A label from the window: unchanged (as she typed it before, or as the
/// LaTeX) keeps her spelling; anything else is converted and its spelling
/// recorded. Returns the LaTeX.
fn typed_label(fig: &mut Figure, key: &str, old: Option<&str>, typed: &str) -> String {
    if let Some(old) = old {
        if typed == old || typed == spelling::raw(fig, key, old) {
            return old.to_string();
        }
    }
    let latex = clean(&unicode::to_latex(typed));
    spelling::record(fig, key, &latex, &clean(typed.trim()));
    latex
}

fn valid_color(c: &Option<String>) -> Result<Option<String>, String> {
    match c {
        None => Ok(None),
        Some(c) => {
            let v = c.trim().to_lowercase();
            let v = if v == "gray" { "grey".to_string() } else { v };
            if parse::PALETTE.contains(&v.as_str()) { Ok(Some(v)) } else { Err(format!("`{c}` isn't a colour; they are {}", parse::PALETTE.join(" "))) }
        }
    }
}

fn valid_pos(p: &Option<Pos>) -> Result<(), String> {
    match p {
        Some(Pos::Cell(a, b)) | Some(Pos::Abs(a, b)) if !(a.is_finite() && b.is_finite()) => Err("that position isn't a number".into()),
        _ => Ok(()),
    }
}

fn valid_angle(a: f32) -> Result<f32, String> {
    if a.is_finite() { Ok(if a == 0.0 { 0.0 } else { a }) } else { Err("that angle isn't a number".into()) }
}

/// Is `new` free for an object currently called `old` (a rename)?
fn check_rename(fig: &Figure, old: &str, new: &str) -> Result<(), String> {
    if canon(old) == canon(new) {
        return Ok(());
    }
    if !is_token(new) || new.contains('.') {
        return Err(format!("`{new}` can't be a name"));
    }
    if names(fig).contains(&canon(new)) {
        return Err(format!("the name `{new}` is taken"));
    }
    Ok(())
}

/// Rename an object everywhere it is referred to: arrow ends (`P.k`,
/// `C.word` too), blob parents, and the hints.
fn rename_refs(fig: &mut Figure, old: &str, new: &str) {
    if old == new {
        return;
    }
    let map = |s: &str| -> String {
        if s == old {
            return new.to_string();
        }
        match s.split_once('.') {
            Some((pre, rest)) if pre == old => format!("{new}.{rest}"),
            _ => s.to_string(),
        }
    };
    let refs: Vec<usize> = fig.items.iter().enumerate().filter(|(_, it)| matches!(it, Item::Arrow(a) if map(&a.from) != a.from || map(&a.to) != a.to)).map(|(k, _)| k).collect();
    for k in refs {
        touch_grid(fig, k, &format!("renamed {old}"));
    }
    for it in fig.items.iter_mut() {
        match it {
            Item::Arrow(a) => {
                a.from = map(&a.from);
                a.to = map(&a.to);
            }
            Item::Blob(b) => {
                if b.parent.as_deref() == Some(old) {
                    b.parent = Some(new.to_string());
                }
            }
            _ => {}
        }
    }
    for n in fig.hints.implicit.iter_mut() {
        if n == old {
            *n = new.to_string();
        }
    }
    for (b, names) in fig.hints.listed.iter_mut() {
        if b == old {
            *b = new.to_string();
        }
        for n in names.iter_mut() {
            if n == old {
                *n = new.to_string();
            }
        }
    }
    for sp in fig.hints.spelling.iter_mut() {
        let Some((kind, rest)) = sp.key.split_once(':') else { continue };
        let rest = match kind {
            "arrow" => match rest.split_once('>') {
                Some((f, t)) => format!("{}>{}", map(f), map(t)),
                None => rest.to_string(),
            },
            _ => map(rest),
        };
        sp.key = format!("{kind}:{rest}");
    }
}

/// Delete the arrows that end at any of these (by name, or `P.k` with k > n).
fn drop_arrows(fig: &mut Figure, dead: impl Fn(&str) -> bool, why: &str) {
    let deps: Vec<usize> = fig.items.iter().enumerate().filter(|(_, it)| matches!(it, Item::Arrow(a) if dead(&a.from) || dead(&a.to))).map(|(k, _)| k).collect();
    for &k in &deps {
        touch_grid(fig, k, why);
    }
    remove_items(fig, &deps);
}

fn set_item(fig: &mut Figure, index: usize, item: &Item) -> Result<Option<String>, String> {
    let Some(old) = fig.items.get(index).cloned() else {
        return Err(format!("there is no item {index}"));
    };
    let kind = |it: &Item| std::mem::discriminant(it);
    if kind(&old) != kind(item) {
        return Err("SetItem replaces a statement with one of the same kind".into());
    }
    match (&old, item) {
        (Item::Node(o), Item::Node(n)) => {
            let mut n = n.clone();
            check_rename(fig, &o.name, &n.name)?;
            valid_pos(&n.pos)?;
            n.color = valid_color(&n.color)?;
            if n.name != o.name {
                let nm = n.name.clone();
                rename_refs(fig, &o.name, &nm);
            }
            let key = format!("node:{}", n.name);
            n.label = if n.label.trim().is_empty() { parse::default_label(&n.name) } else { typed_label(fig, &key, Some(&o.label), &n.label) };
            n.span = o.span;
            let name = n.name.clone();
            fig.items[index] = Item::Node(n);
            Ok(Some(name))
        }
        (Item::Arrow(o), Item::Arrow(a)) => {
            let mut a = a.clone();
            a.from = endpoint(fig, &a.from)?;
            a.to = endpoint(fig, &a.to)?;
            if !a.bend.is_finite() {
                return Err("that bend isn't a number".into());
            }
            a.color = valid_color(&a.color)?;
            let key = spelling::arrow_key(&a.from, &a.to);
            // the same ends: her old spelling still applies
            let old_label = if o.from == a.from && o.to == a.to { o.label.clone() } else { None };
            a.label = match a.label.as_deref().map(str::trim) {
                None | Some("") => None,
                Some(t) => Some(typed_label(fig, &key, old_label.as_deref(), t)),
            };
            a.is_loop = a.from == a.to;
            a.span = o.span;
            fig.items[index] = Item::Arrow(a);
            Ok(Some(index.to_string()))
        }
        (Item::Text(o), Item::Text(t)) => {
            let mut t = t.clone();
            valid_pos(&Some(t.pos))?;
            t.color = valid_color(&t.color)?;
            if let Some(z) = &t.size {
                if !parse::TEXT_SIZES.contains(&z.as_str()) {
                    return Err(format!("`{z}` isn't a text size ({})", parse::TEXT_SIZES.join(", ")));
                }
            }
            if t.text.trim().is_empty() {
                return Err("the text is empty".into());
            }
            if t.text != o.text && t.text != spelling::raw(fig, "text", &o.text) {
                let latex = clean(&unicode::to_latex_text(&t.text));
                let raw = clean(&t.text);
                if raw != latex && !fig.hints.spelling.iter().any(|s| s.key == "text" && s.latex == latex) {
                    fig.hints.spelling.push(Spelling { key: "text".into(), latex: latex.clone(), raw });
                }
                t.text = latex;
            } else {
                t.text = o.text.clone();
            }
            t.span = o.span;
            fig.items[index] = Item::Text(t);
            Ok(Some(index.to_string()))
        }
        (Item::Blob(o), Item::Blob(b)) => set_blob(fig, index, o, b),
        (Item::NGon(o), Item::NGon(g)) => set_ngon(fig, index, o, g),
        (Item::Cayley(o), Item::Cayley(c)) => set_cayley(fig, index, o, c),
        (Item::Comment { span, .. }, Item::Comment { text, .. }) => {
            let t = text.trim();
            let t = if t.is_empty() || t.starts_with('%') { t.to_string() } else { format!("% {t}") };
            fig.items[index] = Item::Comment { text: t.replace(['\n', '\r'], " "), span: *span };
            Ok(None)
        }
        _ => Err("a grid block is changed through its nodes and arrows".into()),
    }
}

fn set_blob(fig: &mut Figure, index: usize, o: &Blob, b: &Blob) -> Result<Option<String>, String> {
    let mut b = b.clone();
    check_rename(fig, &o.name, &b.name)?;
    valid_pos(&b.pos)?;
    b.color = valid_color(&b.color)?;
    b.angle = valid_angle(b.angle)?;
    if let Some((w, h)) = b.size {
        if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
            return Err("a size is two positive lengths".into());
        }
    }
    // the parent exists and isn't inside this blob
    if let Some(p) = &b.parent {
        let mut cur = Some(p.clone());
        let mut guard = 0;
        while let Some(c) = cur {
            if canon(&c) == canon(&o.name) || canon(&c) == canon(&b.name) {
                return Err(format!("`{}` can't sit inside itself", b.name));
            }
            let parent = fig.items.iter().find_map(|it| match it {
                Item::Blob(x) if canon(&x.name) == canon(&c) => Some(x.parent.clone()),
                _ => None,
            });
            match parent {
                None => return Err(format!("there is no blob `{c}` to put `{}` in", b.name)),
                Some(pp) => cur = pp,
            }
            guard += 1;
            if guard > 1000 {
                break;
            }
        }
    }
    // element names: kept, or made from the label; unique in the figure
    let mut taken: HashSet<String> = names(fig);
    for e in &o.elements {
        taken.remove(&canon(&e.name));
    }
    taken.remove(&canon(&o.name));
    taken.insert(canon(&b.name));
    let old_elems: Vec<Element> = o.elements.clone();
    for e in b.elements.iter_mut() {
        valid_pos(&e.pos)?;
        if e.name.trim().is_empty() {
            let t = e.label.trim();
            e.name = if is_token(t) && !t.contains('.') && !taken.contains(&canon(t)) {
                t.to_string()
            } else {
                let mut k = 1;
                loop {
                    let c = format!("x{k}");
                    if !taken.contains(&canon(&c)) {
                        break c;
                    }
                    k += 1;
                }
            };
        }
        if !is_token(&e.name) || e.name.contains('.') || taken.contains(&canon(&e.name)) {
            return Err(format!("the element name `{}` is taken or unusable", e.name));
        }
        taken.insert(canon(&e.name));
    }
    // rename first, so the keys below are the new ones
    if b.name != o.name {
        rename_refs(fig, &o.name, &b.name.clone());
    }
    let bkey = format!("blob:{}", b.name);
    b.label = if b.label.trim().is_empty() { parse::default_label(&b.name) } else { typed_label(fig, &bkey, Some(&o.label), &b.label) };
    for e in b.elements.iter_mut() {
        let key = format!("elem:{}", e.name);
        let before = old_elems.iter().find(|x| x.name == e.name).map(|x| x.label.clone());
        e.label = if e.label.trim().is_empty() { parse::default_label(&e.name) } else { typed_label(fig, &key, before.as_deref(), &e.label) };
    }
    // elements that went: their arrows go too
    let gone: HashSet<String> = old_elems.iter().filter(|x| !b.elements.iter().any(|e| e.name == x.name)).map(|x| x.name.clone()).collect();
    b.span = o.span;
    let name = b.name.clone();
    fig.items[index] = Item::Blob(b);
    if !gone.is_empty() {
        drop_arrows(fig, |s| gone.contains(s), "deleted an arrow's end");
        for (_, names) in fig.hints.listed.iter_mut() {
            names.retain(|n| !gone.contains(n));
        }
    }
    Ok(Some(name))
}

fn set_ngon(fig: &mut Figure, index: usize, o: &NGon, g: &NGon) -> Result<Option<String>, String> {
    let mut g = g.clone();
    if !(3..=64).contains(&g.n) {
        return Err(format!("an n-gon needs 3 to 64 sides, not {}", g.n));
    }
    check_rename(fig, &o.name, &g.name)?;
    valid_pos(&g.pos)?;
    g.color = valid_color(&g.color)?;
    g.angle = valid_angle(g.angle)?;
    if let Some(r) = g.radius {
        if !(r.is_finite() && r > 0.0) {
            return Err("a radius is a positive length".into());
        }
    }
    let n = g.n;
    let ok_axis = |a: &u32| (1..=n).contains(&(a / 2)) && (a % 2 == 0 || n % 2 == 0);
    g.axes.retain(ok_axis);
    let mut seen = HashSet::new();
    g.axes.retain(|a| seen.insert(*a));
    g.flip = g.flip.filter(ok_axis);
    // (figure-qol D) the rotation arrow's edge is one of this polygon's; `turn=-0` is `turn=0`
    if g.rotation == 0 {
        g.rotation_cw = false;
        g.rotation_edge = None;
    }
    g.rotation_edge = g.rotation_edge.filter(|e| *e >= 1).map(|e| (e - 1) % n + 1);
    if g.turn % n == 0 {
        g.turn_neg = false;
    }

    if g.name != o.name {
        rename_refs(fig, &o.name, &g.name.clone());
    }
    // n labels; a missing one is numbered
    g.labels.truncate(n as usize);
    while g.labels.len() < n as usize {
        g.labels.push((g.labels.len() + 1).to_string());
    }
    for k in 0..n as usize {
        let key = format!("ngon:{}.{}", g.name, k + 1);
        let typed = g.labels[k].clone();
        g.labels[k] = if typed.trim().is_empty() {
            String::new()
        } else {
            typed_label(fig, &key, o.labels.get(k).map(|s| s.as_str()), &typed)
        };
    }
    // a label that's a number naming another vertex, or an empty list, is
    // fine; vertices that no longer exist lose their arrows
    let name = g.name.clone();
    g.span = o.span;
    fig.items[index] = Item::NGon(g);
    if n < o.n {
        let pre = format!("{name}.");
        drop_arrows(fig, |s| s.strip_prefix(&pre).and_then(|k| k.parse::<u32>().ok()).is_some_and(|k| k > n), "deleted an arrow's end");
    }
    Ok(Some(name))
}

fn set_cayley(fig: &mut Figure, index: usize, o: &Cayley, c: &Cayley) -> Result<Option<String>, String> {
    let mut c = c.clone();
    c.angle = valid_angle(c.angle)?;
    valid_pos(&c.pos)?;
    if let Some(r) = c.radius {
        if !(r.is_finite() && r > 0.0) {
            return Err("a radius is a positive length".into());
        }
    }
    // (figure-qol D) per-generator looks: palette colours only, nothing at the default
    c.gen_styles.retain(|s| !s.generator.trim().is_empty());
    for s in c.gen_styles.iter_mut() {
        s.color = valid_color(&s.color)?;
    }
    c.gen_styles.retain(|s| s.color.is_some() || s.inverse || s.hidden);
    // an automatic name follows a new group (`cayley D4` → `cayley D6`)
    if let (GroupSpec::Named(og), GroupSpec::Named(ng)) = (&o.group, &c.group) {
        let ng = parse::group_name(ng);
        if c.name == o.name && o.name == *og && ng != *og && !names(fig).contains(&canon(&ng)) && is_token(&ng) {
            c.name = ng;
        }
    }
    check_rename(fig, &o.name, &c.name)?;
    if c.name != o.name {
        rename_refs(fig, &o.name, &c.name.clone());
    }
    let name = c.name.clone();
    // generators as typed
    let og = o.gens.clone();
    for (k, gname) in c.gens.iter_mut().enumerate() {
        let key = format!("cayley:{name}.{}", k + 1);
        *gname = typed_label(fig, &key, og.get(k).map(|s| s.as_str()), gname.trim());
    }
    c.gens.retain(|x| !x.is_empty());
    match &mut c.group {
        GroupSpec::Named(g) => {
            let canon_g = parse::group_name(g);
            if !parse::known_group(&canon_g) {
                return Err(format!("I don't know the group `{g}`"));
            }
            let key = format!("group:{name}");
            let unchanged = matches!(&o.group, GroupSpec::Named(x) if *x == canon_g) && (*g == canon_g || *g == spelling::raw(fig, &key, &canon_g));
            if !unchanged {
                spelling::record(fig, &key, &canon_g, g.trim());
            }
            *g = canon_g;
        }
        GroupSpec::Presentation(gens, rels) => {
            if c.gens.is_empty() {
                return Err("a presentation needs generators".into());
            }
            *gens = c.gens.clone();
            let orels: Vec<String> = match &o.group {
                GroupSpec::Presentation(_, r) => r.clone(),
                _ => vec![],
            };
            let mut out = Vec::new();
            for (k, r) in rels.iter().enumerate() {
                let raw: String = r.chars().filter(|ch| !ch.is_whitespace()).collect();
                if raw.is_empty() {
                    continue;
                }
                let key = format!("rel:{name}.{}", out.len() + 1);
                let canon_r = parse::relator(&raw);
                let before = orels.get(k);
                let unchanged = before.is_some_and(|b| *b == canon_r) && (raw == canon_r || before.is_some_and(|b| raw == spelling::raw(fig, &key, b)));
                if !unchanged {
                    spelling::record(fig, &key, &canon_r, &raw);
                }
                out.push(canon_r);
            }
            *rels = out;
        }
    }
    c.span = o.span;
    fig.items[index] = Item::Cayley(c);
    Ok(Some(name))
}

fn move_obj(fig: &mut Figure, obj: &ObjRef, pos: Pos) -> Result<Option<String>, String> {
    valid_pos(&Some(pos))?;
    let key = |s: &str| canon(s);
    match obj {
        ObjRef::Node(name) => {
            let i = node_index(fig, name).ok_or_else(|| format!("there is no node `{name}` to move"))?;
            touch_grid(fig, i, &format!("moved {name}"));
            let Item::Node(n) = &mut fig.items[i] else { unreachable!() };
            n.pos = Some(pos);
            Ok(Some(n.name.clone()))
        }
        ObjRef::Text(i) => match fig.items.get_mut(*i) {
            Some(Item::Text(t)) => {
                t.pos = pos;
                Ok(Some(i.to_string()))
            }
            _ => Err(format!("item {i} isn't a text")),
        },
        ObjRef::Blob(name) | ObjRef::Element(name) | ObjRef::NGon(name) | ObjRef::Cayley(name) => {
            for it in fig.items.iter_mut() {
                match (it, obj) {
                    (Item::Blob(b), ObjRef::Blob(_)) if key(&b.name) == key(name) => {
                        b.pos = Some(pos);
                        return Ok(Some(b.name.clone()));
                    }
                    (Item::Blob(b), ObjRef::Element(_)) => {
                        if let Some(e) = b.elements.iter_mut().find(|e| key(&e.name) == key(name)) {
                            e.pos = Some(pos);
                            return Ok(Some(e.name.clone()));
                        }
                    }
                    (Item::NGon(g), ObjRef::NGon(_)) if key(&g.name) == key(name) => {
                        g.pos = Some(pos);
                        return Ok(Some(g.name.clone()));
                    }
                    (Item::Cayley(c), ObjRef::Cayley(_)) if key(&c.name) == key(name) => {
                        c.pos = Some(pos);
                        return Ok(Some(c.name.clone()));
                    }
                    _ => {}
                }
            }
            Err(format!("there is nothing called `{name}` to move"))
        }
        ObjRef::NGonVertex(..) | ObjRef::CayleyVertex(..) | ObjRef::CayleyEdge(..) => Err("a vertex moves with its shape; move the shape".into()),
        ObjRef::NGonRotation(_) => Err("the rotation arrow moves with its polygon; drag it to another edge instead".into()),
        ObjRef::NGonAxis(..) | ObjRef::NGonRotationLabel(..) => Err("a label moves with its shape; move the shape".into()),
        ObjRef::Arrow(_) => Err("an arrow follows its ends; move them".into()),
    }
}

/// A name not used yet: `base` + primes (`G'`, `G''`, …).
pub(crate) fn primed(taken: &HashSet<String>, base: &str) -> String {
    let mut s = format!("{base}'");
    while taken.contains(&canon(&s)) {
        s.push('\'');
    }
    s
}

/// A shape's copy goes 1 cm right and 1 cm down of a placed original; an
/// unplaced one joins the automatic row (at its end).
fn shape_offset(fig: &Figure, p: Option<Pos>) -> Option<Pos> {
    let sp = fig.globals.spacing;
    let sp = if sp.0 > 0.0 && sp.1 > 0.0 { sp } else { DEFAULT_SPACING };
    let r2 = |v: f32| (v * 100.0).round() / 100.0;
    p.map(|p| match p {
        Pos::Abs(x, y) => Pos::Abs(r2(x + 1.0), r2(y - 1.0)),
        Pos::Cell(c, r) => Pos::Abs(r2(c * sp.0 + 1.0), r2(-r * sp.1 - 1.0)),
    })
}

/// A point's copy (node, element, text) goes one cell to the right, to the
/// first free cell for a node.
fn point_offset(p: Pos) -> Pos {
    match p {
        Pos::Abs(x, y) => Pos::Abs(((x + 1.0) * 100.0).round() / 100.0, y),
        Pos::Cell(c, r) => Pos::Cell(c + 1.0, r),
    }
}

fn duplicate(fig: &mut Figure, obj: &ObjRef) -> Result<Option<String>, String> {
    let taken = names(fig);
    match obj {
        ObjRef::Node(name) => {
            let i = node_index(fig, name).ok_or_else(|| format!("there is no node `{name}`"))?;
            let Item::Node(o) = fig.items[i].clone() else { unreachable!() };
            let scene = crate::layout::layout(fig);
            let cell = if scene.cell.0 > 0.0 && scene.cell.1 > 0.0 { scene.cell } else { DEFAULT_SPACING };
            let to_cell = |q: Pt| ((q.x / cell.0).round() as i32, (-q.y / cell.1).round() as i32);
            let occupied: HashSet<(i32, i32)> = scene.anchors.iter().filter(|(o, _, _)| matches!(o, ObjRef::Node(_))).map(|(_, q, _)| to_cell(*q)).collect();
            let pos = match o.pos {
                Some(Pos::Abs(..)) => point_offset(o.pos.unwrap()),
                Some(Pos::Cell(c, r)) if c.fract() != 0.0 || r.fract() != 0.0 => point_offset(Pos::Cell(c, r)),
                _ => {
                    let at = scene.anchors.iter().find(|(x, _, _)| *x == ObjRef::Node(o.name.clone())).map(|x| x.1).unwrap_or_default();
                    let (mut c, r) = match o.pos {
                        Some(Pos::Cell(c, r)) => (c as i32, r as i32),
                        _ => to_cell(at),
                    };
                    c += 1;
                    while occupied.contains(&(c, r)) && c < 1000 {
                        c += 1;
                    }
                    Pos::Cell(c as f32, r as f32)
                }
            };
            let new = fresh_name(fig);
            let raw = spelling::raw(fig, &format!("node:{}", o.name), &o.label);
            spelling::record(fig, &format!("node:{new}"), &o.label, &raw);
            fig.items.push(Item::Node(Node { name: new.clone(), label: o.label.clone(), pos: Some(pos), color: o.color.clone(), span: Span::default() }));
            Ok(Some(new))
        }
        ObjRef::Element(name) => {
            for bi in 0..fig.items.len() {
                let Item::Blob(b) = &fig.items[bi] else { continue };
                let Some(e) = b.elements.iter().find(|e| canon(&e.name) == canon(name)).cloned() else { continue };
                let new = primed(&taken, &e.name);
                let label = if e.label == parse::default_label(&e.name) { parse::default_label(&new) } else { e.label.clone() };
                let raw = spelling::raw(fig, &format!("elem:{}", e.name), &e.label);
                if label == e.label {
                    spelling::record(fig, &format!("elem:{new}"), &label, &raw);
                }
                if let Item::Blob(b) = &mut fig.items[bi] {
                    b.elements.push(Element { name: new.clone(), label, pos: e.pos.map(point_offset) });
                }
                return Ok(Some(new));
            }
            Err(format!("there is no element `{name}`"))
        }
        ObjRef::Text(i) => match fig.items.get(*i).cloned() {
            Some(Item::Text(t)) => {
                fig.items.push(Item::Text(Text { text: t.text, pos: point_offset(t.pos), span: Span::default(), color: t.color, size: t.size }));
                Ok(Some((fig.items.len() - 1).to_string()))
            }
            _ => Err(format!("item {i} isn't a text")),
        },
        ObjRef::Arrow(i) => match fig.items.get(*i).cloned() {
            Some(Item::Arrow(mut a)) => {
                a.span = Span::default();
                fig.items.push(Item::Arrow(a));
                Ok(Some((fig.items.len() - 1).to_string()))
            }
            _ => Err(format!("item {i} isn't an arrow")),
        },
        ObjRef::Blob(name) => {
            let Some(Item::Blob(o)) = fig.items.iter().find(|it| matches!(it, Item::Blob(b) if canon(&b.name) == canon(name))).cloned() else {
                return Err(format!("there is no blob `{name}`"));
            };
            let mut taken = taken;
            let new = primed(&taken, &o.name);
            taken.insert(canon(&new));
            let mut b = o.clone();
            b.name = new.clone();
            b.label = if o.label == parse::default_label(&o.name) { parse::default_label(&new) } else { o.label.clone() };
            if b.label == o.label {
                let raw = spelling::raw(fig, &format!("blob:{}", o.name), &o.label);
                spelling::record(fig, &format!("blob:{new}"), &b.label, &raw);
            }
            for e in b.elements.iter_mut() {
                let en = primed(&taken, &e.name);
                taken.insert(canon(&en));
                let old_label = e.label.clone();
                e.label = if e.label == parse::default_label(&e.name) { parse::default_label(&en) } else { e.label.clone() };
                if e.label == old_label {
                    let raw = spelling::raw(fig, &format!("elem:{}", e.name), &old_label);
                    spelling::record(fig, &format!("elem:{en}"), &e.label, &raw);
                }
                e.name = en;
                // a placed point moves with its placed blob; otherwise it is spread inside
                e.pos = if o.pos.is_some() { shape_offset(fig, e.pos) } else { None };
            }
            b.pos = shape_offset(fig, o.pos);
            b.span = Span::default();
            fig.items.push(Item::Blob(b));
            Ok(Some(new))
        }
        ObjRef::NGon(name) | ObjRef::NGonVertex(name, _) | ObjRef::NGonRotation(name) | ObjRef::NGonAxis(name, _) | ObjRef::NGonRotationLabel(name) => {
            let Some(Item::NGon(o)) = fig.items.iter().find(|it| matches!(it, Item::NGon(g) if canon(&g.name) == canon(name))).cloned() else {
                return Err(format!("there is no n-gon `{name}`"));
            };
            // `P` → `P2`, `P3`, …; `P2` → `P3`
            let digits = o.name.trim_end_matches(|c: char| c.is_ascii_digit());
            let (base, start) = if digits.len() < o.name.len() && !digits.is_empty() { (digits.to_string(), o.name[digits.len()..].parse::<u32>().unwrap_or(1) + 1) } else { (o.name.clone(), 2) };
            let dotted: Vec<String> = fig.items.iter().filter_map(|it| if let Item::Node(n) = it { Some(n.name.clone()) } else { None }).collect();
            let new = (start..).map(|k| format!("{base}{k}")).find(|c| !taken.contains(&canon(c)) && !dotted.iter().any(|d| d.starts_with(&format!("{c}.")))).unwrap();
            let mut g = o.clone();
            g.name = new.clone();
            for (k, l) in o.labels.iter().enumerate() {
                let raw = spelling::raw(fig, &format!("ngon:{}.{}", o.name, k + 1), l);
                spelling::record(fig, &format!("ngon:{new}.{}", k + 1), l, &raw);
            }
            g.pos = shape_offset(fig, o.pos);
            g.span = Span::default();
            fig.items.push(Item::NGon(g));
            Ok(Some(new))
        }
        ObjRef::Cayley(name) | ObjRef::CayleyVertex(name, _) | ObjRef::CayleyEdge(name, _) => {
            let Some(Item::Cayley(o)) = fig.items.iter().find(|it| matches!(it, Item::Cayley(c) if canon(&c.name) == canon(name))).cloned() else {
                return Err(format!("there is no Cayley graph `{name}`"));
            };
            let new = primed(&taken, &o.name);
            let mut c = o.clone();
            c.name = new.clone();
            // her spelling of the group, generators and relators comes along
            let pairs: Vec<(String, String)> = std::iter::once((format!("group:{}", o.name), format!("group:{new}")))
                .chain((1..=o.gens.len()).map(|k| (format!("cayley:{}.{k}", o.name), format!("cayley:{new}.{k}"))))
                .chain((1..=match &o.group { GroupSpec::Presentation(_, r) => r.len(), _ => 0 }).map(|k| (format!("rel:{}.{k}", o.name), format!("rel:{new}.{k}"))))
                .collect();
            let copies: Vec<Spelling> = fig.hints.spelling.iter().filter_map(|s| pairs.iter().find(|p| p.0 == s.key).map(|p| Spelling { key: p.1.clone(), latex: s.latex.clone(), raw: s.raw.clone() })).collect();
            fig.hints.spelling.extend(copies);
            c.pos = shape_offset(fig, o.pos);
            c.span = Span::default();
            fig.items.push(Item::Cayley(c));
            Ok(Some(new))
        }
    }
}

fn delete(fig: &mut Figure, obj: &ObjRef) -> Result<(), String> {
    let refs = |a: &Arrow, names: &HashSet<String>, prefixes: &[String]| -> bool {
        let hit = |s: &str| names.contains(s) || prefixes.iter().any(|p| s.starts_with(&format!("{p}.")));
        hit(&a.from) || hit(&a.to)
    };
    let mut kill: Vec<usize> = Vec::new();
    // grids that lost an object: kept as art if they still can be
    let mut settle_after: Vec<(usize, String)> = Vec::new();
    let mut gone: HashSet<String> = HashSet::new();
    let mut prefixes: Vec<String> = Vec::new();
    match obj {
        // (figure-qol D) deleting the rotation arrow takes the arrow off, not the polygon
        ObjRef::NGonRotation(name) => {
            let g = fig
                .items
                .iter_mut()
                .find_map(|it| match it {
                    Item::NGon(g) if canon(&g.name) == canon(name) => Some(g),
                    _ => None,
                })
                .ok_or_else(|| format!("there is no n-gon `{name}`"))?;
            g.rotation = 0;
            g.rotation_cw = false;
            g.rotation_edge = None;
            return Ok(());
        }
        ObjRef::Node(name) => {
            let i = node_index(fig, name).ok_or_else(|| format!("there is no node `{name}`"))?;
            settle_after.extend(grid_of(fig, i).map(|g| (g, format!("deleted {name}"))));
            if let Item::Node(n) = &fig.items[i] {
                gone.insert(n.name.clone());
            }
            kill.push(i);
        }
        ObjRef::Arrow(i) => {
            if !matches!(fig.items.get(*i), Some(Item::Arrow(_))) {
                return Err(format!("item {i} isn't an arrow"));
            }
            settle_after.extend(grid_of(fig, *i).map(|g| (g, "deleted an arrow".to_string())));
            kill.push(*i);
        }
        ObjRef::Blob(name) => {
            let i = fig
                .items
                .iter()
                .position(|it| matches!(it, Item::Blob(b) if canon(&b.name) == canon(name)))
                .ok_or_else(|| format!("there is no blob `{name}`"))?;
            let (bname, parent, elems) = match &fig.items[i] {
                Item::Blob(b) => (b.name.clone(), b.parent.clone(), b.elements.iter().map(|e| e.name.clone()).collect::<Vec<_>>()),
                _ => unreachable!(),
            };
            gone.insert(bname.clone());
            gone.extend(elems);
            for it in fig.items.iter_mut() {
                if let Item::Blob(c) = it {
                    if c.parent.as_deref() == Some(bname.as_str()) {
                        c.parent = parent.clone();
                    }
                }
            }
            kill.push(i);
        }
        ObjRef::Element(name) => {
            let mut found = None;
            for it in fig.items.iter_mut() {
                if let Item::Blob(b) = it {
                    if let Some(k) = b.elements.iter().position(|e| canon(&e.name) == canon(name)) {
                        found = Some(b.elements.remove(k).name);
                        break;
                    }
                }
            }
            let n = found.ok_or_else(|| format!("there is no element `{name}`"))?;
            gone.insert(n);
        }
        ObjRef::NGon(name) | ObjRef::NGonVertex(name, _) | ObjRef::NGonAxis(name, _) | ObjRef::NGonRotationLabel(name) => {
            // an n-gon is one object: deleting a vertex deletes the n-gon
            let i = fig
                .items
                .iter()
                .position(|it| matches!(it, Item::NGon(g) if canon(&g.name) == canon(name)))
                .ok_or_else(|| format!("there is no n-gon `{name}`"))?;
            if let Item::NGon(g) = &fig.items[i] {
                gone.insert(g.name.clone());
                prefixes.push(g.name.clone());
            }
            kill.push(i);
        }
        ObjRef::Cayley(c) | ObjRef::CayleyVertex(c, _) | ObjRef::CayleyEdge(c, _) => {
            // a Cayley graph is one object: deleting any part of it deletes it
            let i = fig
                .items
                .iter()
                .position(|it| matches!(it, Item::Cayley(x) if canon(&x.name) == canon(c)))
                .ok_or_else(|| format!("there is no Cayley graph `{c}`"))?;
            if let Item::Cayley(x) = &fig.items[i] {
                gone.insert(x.name.clone());
                prefixes.push(x.name.clone());
            }
            kill.push(i);
        }
        ObjRef::Text(i) => {
            if !matches!(fig.items.get(*i), Some(Item::Text(_))) {
                return Err(format!("item {i} isn't a text"));
            }
            kill.push(*i);
        }
        #[allow(unreachable_patterns)]
        other => return Err(format!("can't delete {other:?}")),
    }
    // dependent arrows (grid arrows among them turn their grid into statements first)
    if !gone.is_empty() {
        let deps: Vec<usize> = fig
            .items
            .iter()
            .enumerate()
            .filter(|(_, it)| matches!(it, Item::Arrow(a) if refs(a, &gone, &prefixes)))
            .map(|(k, _)| k)
            .collect();
        for &k in &deps {
            settle_after.extend(grid_of(fig, k).map(|g| (g, "deleted an arrow's end".to_string())));
        }
        kill.extend(deps);
    }
    remove_items(fig, &kill);
    // (a Grid item sits before its objects, so only earlier kills shift it)
    let mut done: Vec<usize> = Vec::new();
    let mut empty: Vec<usize> = Vec::new();
    for (g, why) in settle_after {
        let g2 = g - kill.iter().filter(|&&k| k < g).count();
        if !done.contains(&g2) {
            done.push(g2);
            if !fig.grid_items.iter().any(|x| x.1 == g2) {
                // everything in it went: the block goes too (no orphan comment)
                empty.push(g2);
            } else {
                settle_or_convert(fig, Some(g2), &why);
            }
        }
    }
    remove_items(fig, &empty);
    for (_, names) in fig.hints.listed.iter_mut() {
        names.retain(|n| !gone.contains(n));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// helpers

/// Every object name (canonical form).
pub(crate) fn names(fig: &Figure) -> HashSet<String> {
    let mut s = HashSet::new();
    for it in &fig.items {
        match it {
            Item::Node(n) => {
                s.insert(canon(&n.name));
            }
            Item::Blob(b) => {
                s.insert(canon(&b.name));
                for e in &b.elements {
                    s.insert(canon(&e.name));
                }
            }
            Item::NGon(g) => {
                s.insert(canon(&g.name));
            }
            Item::Cayley(c) => {
                s.insert(canon(&c.name));
            }
            _ => {}
        }
    }
    s
}

fn clean(s: &str) -> String {
    s.replace('"', "''").replace(['\n', '\r'], " ")
}

/// A canvas label → (name, label): the label itself names the object when
/// it is a usable, unused token ("G" → node `G`); otherwise a fresh name.
fn name_from_label(fig: &Figure, label: &str) -> (String, String) {
    let t = label.trim();
    let used = names(fig);
    let vertexish = t.split_once('.').map(|(p, _)| {
        fig.items.iter().any(|it| matches!(it, Item::NGon(g) if canon(&g.name) == canon(p)) || matches!(it, Item::Cayley(c) if canon(&c.name) == canon(p)))
    });
    if is_token(t) && !used.contains(&canon(t)) && vertexish != Some(true) {
        return (t.to_string(), parse::default_label(t));
    }
    let name = fresh_name(fig);
    let l = clean(&unicode::to_latex(t));
    let l = if l.is_empty() { parse::default_label(&name) } else { l };
    (name, l)
}

fn node_index(fig: &Figure, name: &str) -> Option<usize> {
    let key = canon(name);
    fig.items.iter().position(|it| matches!(it, Item::Node(n) if canon(&n.name) == key))
}

/// The stored name of an arrow end: a node, blob, element, n-gon vertex
/// `P.k` or Cayley vertex `C.word`.
fn endpoint(fig: &Figure, name: &str) -> Result<String, String> {
    let key = canon(name);
    for it in &fig.items {
        match it {
            Item::Node(n) if canon(&n.name) == key => return Ok(n.name.clone()),
            Item::Blob(b) => {
                if canon(&b.name) == key {
                    return Ok(b.name.clone());
                }
                if let Some(e) = b.elements.iter().find(|e| canon(&e.name) == key) {
                    return Ok(e.name.clone());
                }
            }
            Item::NGon(g) if canon(&g.name) == key => return Ok(g.name.clone()),
            Item::Cayley(c) if canon(&c.name) == key => return Ok(c.name.clone()),
            _ => {}
        }
    }
    if let Some((p, s)) = name.split_once('.') {
        for it in &fig.items {
            match it {
                Item::NGon(g) if canon(&g.name) == canon(p) => {
                    if let Ok(k) = s.parse::<u32>() {
                        if (1..=g.n).contains(&k) {
                            return Ok(format!("{}.{k}", g.name));
                        }
                    }
                    if let Some(ix) = g.labels.iter().position(|l| *l == s || *l == unicode::to_latex(s)) {
                        return Ok(format!("{}.{}", g.name, ix + 1));
                    }
                    return Err(format!("`{name}`: {} has vertices 1..{}", g.name, g.n));
                }
                Item::Cayley(c) if canon(&c.name) == canon(p) && !s.is_empty() => return Ok(format!("{}.{}", c.name, s.trim())),
                _ => {}
            }
        }
    }
    Err(format!("there is nothing called `{name}` to draw an arrow to"))
}

/// If item `i` came from a grid block, turn that block into statements.
/// (FIG-INTERACT) The grid block item `i` came from.
fn grid_of(fig: &Figure, i: usize) -> Option<usize> {
    fig.grid_items.iter().find(|x| x.0 == i).map(|x| x.1)
}

/// (FIG-INTERACT) After a change to a grid's object: keep the art when it
/// can still say it (gridedit::settle), else turn the block into statements.
fn settle_or_convert(fig: &mut Figure, gi: Option<usize>, why: &str) {
    let Some(gi) = gi else { return };
    if !crate::gridedit::settle(fig, gi) {
        convert_grid(fig, gi, why);
    }
}

fn touch_grid(fig: &mut Figure, i: usize, why: &str) {
    let Some(&(_, gi)) = fig.grid_items.iter().find(|x| x.0 == i) else { return };
    convert_grid(fig, gi, why);
}

fn convert_grid(fig: &mut Figure, gi: usize, why: &str) {
    if !matches!(fig.items.get(gi), Some(Item::Grid { .. })) {
        return;
    }
    let line = match &fig.items[gi] {
        Item::Grid { span, .. } => *span,
        _ => Span::default(),
    };
    fig.items[gi] = Item::Comment { text: format!("% grid converted to statements ({why})"), span: line };
    let derived: Vec<usize> = fig.grid_items.iter().filter(|x| x.1 == gi).map(|x| x.0).collect();
    for k in derived {
        match &mut fig.items[k] {
            Item::Node(n) => n.span = Span::default(),
            Item::Arrow(a) => a.span = Span::default(),
            _ => {}
        }
    }
    fig.grid_items.retain(|x| x.1 != gi);
}

/// Remove items, keeping `grid_items` and the hints in step.
fn remove_items(fig: &mut Figure, kill: &[usize]) {
    let kill: HashSet<usize> = kill.iter().copied().collect();
    if kill.is_empty() {
        return;
    }
    let mut map: Vec<Option<usize>> = Vec::with_capacity(fig.items.len());
    let mut k = 0;
    for i in 0..fig.items.len() {
        if kill.contains(&i) {
            map.push(None);
        } else {
            map.push(Some(k));
            k += 1;
        }
    }
    let items = std::mem::take(&mut fig.items);
    fig.items = items.into_iter().enumerate().filter(|(i, _)| !kill.contains(i)).map(|(_, it)| it).collect();
    fig.grid_items = fig.grid_items.iter().filter_map(|&(i, g)| Some((map.get(i).copied().flatten()?, map.get(g).copied().flatten()?))).collect();
    // a global written before item k stays before what was after it
    for (_, at) in fig.hints.globals_at.iter_mut() {
        *at = (0..*at).filter(|i| !kill.contains(i)).count();
    }
}

/// Keep the model in the shape `parse` would give for `format`'s output.
pub(crate) fn normalize(fig: &mut Figure) {
    for it in fig.items.iter_mut() {
        if let Item::Arrow(a) = it {
            a.is_loop = a.from == a.to;
        }
    }
    // grid entries must point at a Grid
    let grids: HashSet<usize> = fig.items.iter().enumerate().filter(|(_, it)| matches!(it, Item::Grid { .. })).map(|(i, _)| i).collect();
    fig.grid_items.retain(|x| grids.contains(&x.1));
    // implicit nodes: only those still written inline
    let node_names: HashSet<String> = fig.items.iter().filter_map(|it| if let Item::Node(n) = it { Some(n.name.clone()) } else { None }).collect();
    fig.hints.implicit.retain(|n| node_names.contains(n));
    loop {
        let abs = absorption(fig);
        let keep: HashSet<String> = abs.keys().filter_map(|&k| if let Item::Node(n) = &fig.items[k] { Some(n.name.clone()) } else { None }).collect();
        let before = fig.hints.implicit.len();
        fig.hints.implicit.retain(|n| keep.contains(n));
        if fig.hints.implicit.len() == before {
            break;
        }
    }
    // listed: as format writes it, and only while it names a child's element
    let blobs: Vec<(String, Vec<String>)> = fig.items.iter().filter_map(|it| if let Item::Blob(b) = it { Some((b.name.clone(), b.elements.iter().map(|e| e.name.clone()).collect())) } else { None }).collect();
    let mut listed = std::mem::take(&mut fig.hints.listed);
    listed.retain(|(b, _)| blobs.iter().any(|x| x.0 == *b));
    for (b, names) in listed.iter_mut() {
        let own = &blobs.iter().find(|x| x.0 == *b).unwrap().1;
        let mut full: Vec<String> = names.iter().filter(|n| own.contains(n) || descendant_has(fig, b, n)).cloned().collect();
        for o in own {
            if !full.contains(o) {
                full.push(o.clone());
            }
        }
        *names = full;
    }
    listed.retain(|(b, names)| names.iter().any(|n| descendant_has(fig, b, n)));
    fig.hints.listed = listed;
    spelling::normalize(fig);
    let n = fig.items.len();
    for (_, at) in fig.hints.globals_at.iter_mut() {
        *at = (*at).min(n);
    }
}

// ---------------------------------------------------------------------------
// (figure-qol D) reversing an arrow

/// The arrow turned round: its ends swapped, drawn over the same line.
/// The label stays on the same visual side (`swap` flips, since "left of the
/// direction of travel" flips with it), at the same place along the line
/// (near start ↔ near end), and a bend keeps its curve (it flips sign). A
/// one-sided head (tail, hook, maps to) stays at its end of the line's
/// meaning: the tail is now at the new start. A loop has no direction to
/// turn: it is returned as it is.
pub fn reversed(a: &Arrow) -> Arrow {
    if a.is_loop || a.from == a.to {
        return a.clone();
    }
    let mut r = a.clone();
    std::mem::swap(&mut r.from, &mut r.to);
    r.side = match a.side {
        LabelSide::Auto => LabelSide::Swap,
        LabelSide::Swap => LabelSide::Auto,
    };
    r.label_pos = match a.label_pos {
        LabelPos::NearStart => LabelPos::NearEnd,
        LabelPos::NearEnd => LabelPos::NearStart,
        LabelPos::Middle => LabelPos::Middle,
    };
    r.bend = if a.bend == 0.0 { 0.0 } else { -a.bend };
    r
}

/// `reversed`, aware of the figure: an arrow between two blobs / n-gons is
/// drawn as an arc over the top whichever way it runs, and the layout already
/// keeps its label above, so its `swap` is left alone.
pub fn reversed_in(fig: &Figure, a: &Arrow) -> Arrow {
    let mut r = reversed(a);
    let shape = |name: &str| fig.items.iter().any(|it| matches!(it, Item::Blob(b) if b.name == name) || matches!(it, Item::NGon(g) if g.name == name));
    if shape(&a.from) && shape(&a.to) && a.from != a.to {
        r.side = a.side;
    }
    r
}
