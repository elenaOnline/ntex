//! Direct manipulation on the view (FIGURES.md "Direct manipulation"):
//! picking what a click selects, handle drags, the marquee, and every
//! object command the keys, the right-click menu and the properties panel
//! share (set an item, nudge, rotate, resize, labels, duplicate, colour,
//! order, align).
//!
//! Every change goes through [`FigureView::set_item`] (`Edit::SetItem`), so
//! each is one undo step and the shorthand follows. While the figures crate
//! can't apply SetItem / MoveObj / Duplicate yet, the item is replaced here
//! and the formatter writes it back (what it doesn't know yet, it drops).
use super::doc::{self, core};
use super::geom::{self, Xf};
use super::handles::{self, DragStart, HKind, Handle};
use super::tools::{self, Gesture};
use super::view::FigureView;
use gpui::{Context, Window};
use ntex_figures::model::*;
use std::sync::Arc;

/// (figure-qol A) What the window remembers about copying: the offset of the
/// last ⌥-drag (⌘D repeats it), the shorthand it last copied or cut, and its
/// last paste (so a second paste steps on).
#[derive(Clone, Debug, Default)]
pub struct ClipState {
    pub last_offset: Option<Pt>,
    pub copied: Option<String>,
    pub cut: bool,
    pub pasted: Option<(String, u32)>,
}

/// The palette names (ntex-notes' tones), in the order the swatches show them.
pub const PALETTE: [&str; 8] = ["ink", "blue", "violet", "teal", "maroon", "green", "ochre", "grey"];

/// Is this edit error just "the figures crate doesn't do it yet"?
fn not_built(e: &str) -> bool {
    e.contains("not implemented") || e.contains("not built")
}

/// The object that moves, turns or resizes when `obj` is selected.
pub fn owner(obj: &ObjRef) -> ObjRef {
    match obj {
        ObjRef::NGonVertex(n, _) | ObjRef::NGonRotation(n) => ObjRef::NGon(n.clone()),
        ObjRef::CayleyVertex(c, _) | ObjRef::CayleyEdge(c, _) => ObjRef::Cayley(c.clone()),
        other => other.clone(),
    }
}

/// Objects that can be selected on their own (what ⌘A and the marquee take).
pub fn top_level(scene: &Scene) -> Vec<ObjRef> {
    let mut out: Vec<ObjRef> = Vec::new();
    let push = |o: &ObjRef, out: &mut Vec<ObjRef>| {
        if matches!(o, ObjRef::Node(_) | ObjRef::Arrow(_) | ObjRef::Blob(_) | ObjRef::NGon(_) | ObjRef::Cayley(_) | ObjRef::Text(_)) && !out.contains(o) {
            out.push(o.clone());
        }
    };
    for (o, _, _) in &scene.anchors {
        push(o, &mut out);
    }
    for d in &scene.draws {
        let o = match d {
            Draw::Label { obj, .. } | Draw::Path { obj, .. } | Draw::Dot { obj, .. } => obj,
        };
        push(o, &mut out);
    }
    out
}

pub(super) fn in_rect(p: Pt, r: &Rect, slack: f32) -> bool {
    p.x >= r.min.x - slack && p.x <= r.max.x + slack && p.y >= r.min.y - slack && p.y <= r.max.y + slack
}

impl FigureView {
    // -- what a click selects ---------------------------------------------------

    /// The object under `p` (cm) for selecting: labels by their own boxes
    /// (an n-gon's vertex labels one by one), then `hit`; a vertex dot is
    /// its n-gon; a Cayley graph is taken whole until it is selected, then a
    /// click goes into its vertex.
    pub fn pick(&self, p: Pt, xf: &Xf) -> Option<ObjRef> {
        let raw = self.pick_raw(p, xf)?;
        Some(self.promote(raw))
    }

    /// `pick` without the Cayley rule.
    pub fn pick_raw(&self, p: Pt, xf: &Xf) -> Option<ObjRef> {
        let scene = self.doc.scene.clone();
        let slack = 3.0 / xf.scale.max(1.0);
        let mut best: Option<(f32, ObjRef)> = None;
        for d in &scene.draws {
            if let Draw::Label { obj, at, latex, size, math, .. } = d {
                let r = handles::label_rect(*at, latex, *size, *math);
                if in_rect(p, &r, slack) {
                    let area = (r.max.x - r.min.x) * (r.max.y - r.min.y);
                    if best.as_ref().is_none_or(|(a, _)| area < *a) {
                        best = Some((area, obj.clone()));
                    }
                }
            }
        }
        if let Some((_, o)) = best {
            // (class C) an axis's label is part of its n-gon
            return Some(match o {
                ObjRef::NGonAxis(n, _) => ObjRef::NGon(n),
                o => o,
            });
        }
        match self.hit(p, xf)? {
            // the vertex itself (not its label) is part of the polygon
            ObjRef::NGonVertex(n, _) => Some(ObjRef::NGon(n)),
            o => Some(o),
        }
    }

    /// A Cayley graph's vertex or edge is the whole graph, until the graph
    /// (or one of its vertices) is selected: then a click on a vertex selects it.
    pub fn promote(&self, o: ObjRef) -> ObjRef {
        match o {
            ObjRef::CayleyVertex(c, w) => {
                let inside = self.selection.iter().any(|s| matches!(s, ObjRef::Cayley(n) | ObjRef::CayleyVertex(n, _) if *n == c));
                if inside { ObjRef::CayleyVertex(c, w) } else { ObjRef::Cayley(c) }
            }
            ObjRef::CayleyEdge(c, _) => ObjRef::Cayley(c),
            o => o,
        }
    }

    /// The handles on screen: the single selected object's (none while a
    /// move is under way). They follow a handle drag.
    pub fn handle_list(&self, px_per_cm: f32) -> Vec<Handle> {
        if self.selection.len() != 1 || self.gesture.as_ref().is_some_and(|g| g.is_moving()) || self.label_edit.is_some() {
            return Vec::new();
        }
        let fig = self.drag_figure.as_ref().unwrap_or(&self.doc.figure);
        let scene = self.drag_scene.clone().unwrap_or_else(|| self.doc.scene.clone());
        handles::handles(fig, &scene, &self.selection[0], px_per_cm)
    }

    // -- the one way an item changes ---------------------------------------------

    /// Replace item `index` (one undo step; `group` coalesces a run of
    /// changes into one, e.g. typing in a panel field). `spell` records how
    /// she typed changed labels: (site key, LaTeX, as typed).
    pub fn set_item(&mut self, index: usize, item: Item, what: &str, group: Option<String>, spell: Vec<(String, String, String)>, cx: &mut Context<Self>) -> bool {
        if self.doc.figure.items.get(index) == Some(&item) {
            return false;
        }
        let mut fig = self.doc.figure.clone();
        match doc::apply(&mut fig, &Edit::SetItem { index, item: item.clone() }) {
            Ok(_) => {}
            Err(e) if not_built(&e) => {
                fig = self.doc.figure.clone();
                if index >= fig.items.len() {
                    return false;
                }
                fig.items[index] = item;
            }
            Err(e) => {
                self.toast(format!("! Couldn't change the {what}: {e}"), cx);
                return false;
            }
        }
        for (key, latex, raw) in spell {
            core("spelling::record", || ntex_figures::spelling::record(&mut fig, &key, &latex, &raw));
        }
        core("spelling::normalize", || ntex_figures::spelling::normalize(&mut fig));
        self.commit_grouped(fig, what, group, cx)
    }

    /// Change the item `obj` lives in with `f` (false: nothing to change).
    pub fn edit_obj(&mut self, obj: &ObjRef, what: &str, group: Option<String>, cx: &mut Context<Self>, f: impl FnOnce(&mut Item) -> bool) -> bool {
        let Some(i) = handles::item_index(&self.doc.figure, obj) else { return false };
        let mut item = self.doc.figure.items[i].clone();
        if !f(&mut item) {
            return false;
        }
        self.set_item(i, item, what, group, Vec::new(), cx)
    }

    /// Set a label as typed (LaTeX or Unicode): the label field and the panel.
    pub fn set_label(&mut self, obj: &ObjRef, typed: &str, group: Option<String>, cx: &mut Context<Self>) -> bool {
        let typed = typed.trim();
        // (class C) the rotation arrow's, an axis's and a Cayley vertex's label
        if let Some(done) = self.set_label_part(obj, typed, group.clone(), cx) {
            return done;
        }
        // a placeholder from “+ element”: her first label is its name too (no `x "h'"`)
        if let ObjRef::Element(e) = obj
            && self.fresh_elements.contains(e)
        {
            self.fresh_elements.retain(|x| x != e);
            // Text mode changes the label's TeX wrapper, not the new
            // element's identity. A simple first label still names it.
            let name = if typed.starts_with(r"\text{") { ntex_figures::labeltext::field_from_label(typed) }
                else { typed.to_string() };
            let free = !name.is_empty() && ntex_figures::parse::is_token(&name) && !doc::names(&self.doc.figure).contains(&name);
            if free
                && let Some(i) = handles::item_index(&self.doc.figure, obj)
                && let Item::Blob(mut b) = self.doc.figure.items[i].clone()
                && let Some(el) = b.elements.iter_mut().find(|x| &x.name == e)
            {
                el.name = name.clone();
                el.label = doc::to_latex(typed);
                let spell = vec![(format!("elem:{name}"), el.label.clone(), typed.to_string())];
                if self.set_item(i, Item::Blob(b), "element", group.clone(), spell, cx) {
                    for o in self.selection.iter_mut() {
                        if o == obj {
                            *o = ObjRef::Element(name.clone());
                        }
                    }
                    return true;
                }
            }
        }
        if let ObjRef::NGonVertex(n, k) = obj {
            let Some(i) = handles::item_index(&self.doc.figure, obj) else { return false };
            let Item::NGon(mut g) = self.doc.figure.items[i].clone() else { return false };
            let j = handles::label_index_at(&g, *k);
            return self.set_ngon_label(i, &mut g, n, j, typed, group, cx);
        }
        let mut fig = self.doc.figure.clone();
        let edit = match obj {
            ObjRef::Node(n) if !typed.is_empty() => Some(Edit::SetNodeLabel { name: n.clone(), label: typed.to_string() }),
            ObjRef::Node(_) => return false,
            ObjRef::Blob(n) | ObjRef::Element(n) => Some(Edit::SetNodeLabel { name: n.clone(), label: typed.to_string() }),
            ObjRef::Arrow(i) => match fig.items.get(*i) {
                Some(Item::Arrow(a)) => {
                    let mut a = a.clone();
                    a.label = (!typed.is_empty()).then(|| typed.to_string());
                    Some(Edit::SetArrow { index: *i, arrow: a })
                }
                _ => None,
            },
            _ => None,
        };
        let ok = match edit {
            Some(e) => {
                doc::apply(&mut fig, &e).is_ok() || {
                    fig = self.doc.figure.clone();
                    super::view::set_label_direct(&mut fig, obj, typed)
                }
            }
            None => super::view::set_label_direct(&mut fig, obj, typed),
        };
        ok && self.commit_grouped(fig, "label", group, cx)
    }

    /// Label `j` of n-gon item `i` (the panel's per-vertex fields).
    pub fn set_ngon_label(&mut self, i: usize, g: &mut NGon, name: &str, j: usize, typed: &str, group: Option<String>, cx: &mut Context<Self>) -> bool {
        let n = g.n.max(1) as usize;
        if g.labels.len() < n {
            let have = g.labels.len();
            g.labels.extend((have..n).map(|k| (k + 1).to_string()));
        }
        if j >= g.labels.len() {
            return false;
        }
        let latex = if typed.is_empty() { String::new() } else { doc::to_latex(typed) };
        g.labels[j] = latex.clone();
        let spell = vec![(format!("ngon:{name}.{}", j + 1), latex, typed.to_string())];
        self.set_item(i, Item::NGon(g.clone()), "label", group, spell, cx)
    }

    // -- handle drags --------------------------------------------------------------

    pub fn start_handle(&mut self, h: Handle, grab: Pt, down_px: gpui::Point<gpui::Pixels>) -> bool {
        let fig = &self.doc.figure;
        let Some(index) = handles::item_index(fig, &h.obj) else { return false };
        let scene = self.doc.scene.clone();
        let frame = handles::frame(fig, &scene, &h.obj);
        let item = fig.items[index].clone();
        let top_level = match &item {
            Item::Blob(b) => b.parent.is_none(),
            _ => true,
        };
        let ends = match &item {
            Item::Arrow(a) => {
                let find = |name: &str| scene.anchors.iter().find(|(o, _, _)| geom::end_name(o).as_deref() == Some(name)).map(|(_, p, _)| *p);
                match (find(&a.from), find(&a.to)) {
                    (Some(p), Some(q)) => Some((p, q)),
                    _ => scene.draws.iter().find_map(|d| match d {
                        Draw::Path { obj, start, segs, .. } if *obj == h.obj => {
                            let pts = geom::flatten(*start, segs);
                            Some((*pts.first()?, *pts.last()?))
                        }
                        _ => None,
                    }),
                }
            }
            _ => None,
        };
        let s = DragStart { handle: h, index, item, frame, grab, scale: handles::scene_scale(&scene, fig), top_level, ends };
        self.gesture = Some(Gesture::Handle { start: Box::new(s), last: None, down_px, moved: false });
        true
    }

    /// The pointer moved during a handle drag: the draft follows.
    pub fn handle_moved(&mut self, p: Pt, free: bool, centred: bool) -> bool {
        let Some(Gesture::Handle { start, last, .. }) = self.gesture.as_mut() else { return false };
        let radius_for = |c: &Cayley, outer: f32| core("layout::cayley_radius_for", || ntex_figures::layout::cayley_radius_for(c, outer)).flatten();
        let Some((item, readout)) = handles::drag(start, p, free, centred, &radius_for) else { return false };
        self.readout = Some((p, readout));
        if last.as_ref() == Some(&item) {
            return true;
        }
        *last = Some(item.clone());
        let mut fig = self.doc.figure.clone();
        fig.items[start.index] = item;
        self.drag_scene = Some(Arc::new(doc::scene_of(&fig)));
        self.drag_figure = Some(fig);
        true
    }

    /// The button came up: one undo step.
    pub fn handle_done(&mut self, start: DragStart, last: Option<Item>, cx: &mut Context<Self>) {
        self.drag_scene = None;
        self.drag_figure = None;
        self.readout = None;
        let Some(item) = last else { return };
        let what = match start.handle.kind {
            HKind::Rotate => "angle",
            HKind::Radius | HKind::Scale | HKind::Corner(..) => "size",
            HKind::Bend => "bend",
            HKind::ArrowLabel => "label side",
            HKind::ArrowEnd(_) => "arrow end",
            HKind::RotationSlide => "rotation arrow",
        };
        let spell = match (&start.item, &item) {
            (Item::Arrow(o), Item::Arrow(n)) => self.moved_arrow_spelling(o, n),
            _ => Vec::new(),
        };
        self.set_item(start.index, item, what, None, spell, cx);
    }


    // -- (figure-qol D) direction and orientation ----------------------------------

    /// The spelling records that keep an arrow's label as she typed it when
    /// its ends change (the label's site is named after them).
    pub fn moved_arrow_spelling(&self, old: &Arrow, new: &Arrow) -> Vec<(String, String, String)> {
        let Some(l) = &old.label else { return Vec::new() };
        if old.from == new.from && old.to == new.to {
            return Vec::new();
        }
        let raw = ntex_figures::spelling::raw(&self.doc.figure, &ntex_figures::spelling::arrow_key(&old.from, &old.to), l);
        vec![(ntex_figures::spelling::arrow_key(&new.from, &new.to), l.clone(), raw)]
    }

    /// ⌘R / the ⇄ button / the menu: turn the selected arrows round (ends
    /// swapped, label kept on its visual side); a selected rotation arrow
    /// runs the other way. One undo step.
    pub fn reverse_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let mut fig = self.doc.figure.clone();
        let mut spell: Vec<(String, String, String)> = Vec::new();
        let mut done = 0;
        for o in self.selection.clone() {
            match &o {
                ObjRef::Arrow(i) => {
                    let Some(Item::Arrow(a)) = fig.items.get(*i).cloned() else { continue };
                    if a.is_loop || a.from == a.to {
                        continue;
                    }
                    let r = ntex_figures::edit::reversed_in(&fig, &a);
                    spell.extend(self.moved_arrow_spelling(&a, &r));
                    match doc::apply(&mut fig, &Edit::SetItem { index: *i, item: Item::Arrow(r.clone()) }) {
                        Ok(_) => {}
                        Err(e) if not_built(&e) => fig.items[*i] = Item::Arrow(r),
                        Err(e) => {
                            self.toast(format!("! Couldn't reverse: {e}"), cx);
                            return false;
                        }
                    }
                    done += 1;
                }
                ObjRef::NGonRotation(_) => {
                    let Some(i) = handles::item_index(&fig, &o) else { continue };
                    let Item::NGon(mut g) = fig.items[i].clone() else { continue };
                    g.rotation_cw = !g.rotation_cw;
                    if doc::apply(&mut fig, &Edit::SetItem { index: i, item: Item::NGon(g.clone()) }).is_err() {
                        fig.items[i] = Item::NGon(g);
                    }
                    done += 1;
                }
                _ => {}
            }
        }
        if done == 0 {
            return false;
        }
        for (key, latex, raw) in spell {
            core("spelling::record", || ntex_figures::spelling::record(&mut fig, &key, &latex, &raw));
        }
        core("spelling::normalize", || ntex_figures::spelling::normalize(&mut fig));
        self.commit_grouped(fig, "reverse", None, cx)
    }

    /// While an arrow end is dragged: the draft follows the object under the
    /// pointer (highlighted); over nothing the arrow is as it was.
    pub fn arrow_end_moved(&mut self, p: Pt, xf: &Xf) -> bool {
        let Some(Gesture::Handle { start, .. }) = self.gesture.as_ref() else { return false };
        let HKind::ArrowEnd(is_to) = start.handle.kind else { return false };
        let Item::Arrow(a) = start.item.clone() else { return false };
        let index = start.index;
        let target = self.hit(p, xf).and_then(|o| geom::end_name(&o).map(|n| (o, n)));
        let current = if is_to { &a.to } else { &a.from };
        let new = target.as_ref().filter(|(_, n)| n != current).map(|(_, n)| {
            let mut a2 = a.clone();
            if is_to { a2.to = n.clone() } else { a2.from = n.clone() }
            a2.is_loop = a2.from == a2.to;
            Item::Arrow(a2)
        });
        self.hover = target.as_ref().map(|(o, _)| o.clone());
        let text = match (&target, is_to) {
            (Some((_, n)), true) if n != current => format!("{} → {n}", a.from),
            (Some((_, n)), false) if n != current => format!("{n} → {}", a.to),
            (Some(_), _) => "same end".to_string(),
            (None, _) => "drop on an object".to_string(),
        };
        self.readout = Some((p, text));
        if let Some(Gesture::Handle { last, .. }) = self.gesture.as_mut() {
            *last = new.clone();
        }
        match new {
            Some(item) => {
                let mut fig = self.doc.figure.clone();
                fig.items[index] = item;
                self.drag_scene = Some(Arc::new(doc::scene_of(&fig)));
                self.drag_figure = Some(fig);
            }
            None => {
                self.drag_scene = None;
                self.drag_figure = None;
            }
        }
        true
    }

    /// Turn n-gon `obj`'s rotation arrow the other way, or on/off (the panel).
    pub fn set_rotation_dir(&mut self, obj: &ObjRef, cw: bool, cx: &mut Context<Self>) -> bool {
        self.edit_obj(obj, "rotation arrow", None, cx, |it| match it {
            Item::NGon(g) if g.rotation > 0 && g.rotation_cw != cw => {
                g.rotation_cw = cw;
                true
            }
            _ => false,
        })
    }

    // -- the marquee ------------------------------------------------------------------

    pub fn marquee_select(&mut self, a: Pt, b: Pt, base: &[ObjRef]) {
        let r = Rect { min: Pt { x: a.x.min(b.x), y: a.y.min(b.y) }, max: Pt { x: a.x.max(b.x), y: a.y.max(b.y) } };
        let scene = self.doc.scene.clone();
        let mut sel: Vec<ObjRef> = base.to_vec();
        for o in top_level(&scene) {
            let Some(ext) = handles::outline(&scene, &o) else { continue };
            let take = match o {
                // a container is taken when its middle is inside
                ObjRef::Blob(_) | ObjRef::NGon(_) | ObjRef::Cayley(_) => {
                    let c = Pt { x: (ext.min.x + ext.max.x) / 2.0, y: (ext.min.y + ext.max.y) / 2.0 };
                    in_rect(c, &r, 0.0)
                }
                _ => ext.max.x >= r.min.x && ext.min.x <= r.max.x && ext.max.y >= r.min.y && ext.min.y <= r.max.y,
            };
            if take && !sel.contains(&o) {
                sel.push(o);
            }
        }
        self.selection = sel;
    }

    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.selection = top_level(&self.doc.scene);
        cx.notify();
    }

    // -- object commands (keys, menu, panel) ---------------------------------------

    /// The distinct objects the selection moves / turns / resizes.
    /// How many objects (arrows apart) the selection holds.
    pub fn owners_count(&self) -> usize {
        self.owners().iter().filter(|o| !matches!(o, ObjRef::Arrow(_))).count()
    }

    fn owners(&self) -> Vec<ObjRef> {
        let mut v: Vec<ObjRef> = Vec::new();
        for o in &self.selection {
            let m = owner(o);
            if !v.contains(&m) {
                v.push(m);
            }
        }
        v
    }

    /// Where an object is, in the model's units: its `at`, else where layout put it.
    fn model_pos(&self, obj: &ObjRef) -> Option<Pos> {
        if let ObjRef::Element(e) = obj {
            let pos = self.doc.figure.items.iter().find_map(|it| match it {
                Item::Blob(b) => b.elements.iter().find(|x| &x.name == e).map(|x| x.pos),
                _ => None,
            })?;
            if pos.is_some() {
                return pos;
            }
        } else if let Some(p) = tools::item_pos(&self.doc.figure, obj) {
            return Some(p);
        }
        let scale = handles::scene_scale(&self.doc.scene, &self.doc.figure);
        let c = geom::center_of(&self.doc.scene, obj)?;
        Some(match obj {
            ObjRef::Node(_) => geom::snap(c, self.cell(), true),
            _ => Pos::Abs(c.x / scale, c.y / scale),
        })
    }

    /// Move several objects in one undo step (`group` coalesces a run).
    fn move_all(&mut self, moves: Vec<(ObjRef, Pos)>, what: &str, group: Option<String>, cx: &mut Context<Self>) -> bool {
        if moves.is_empty() {
            return false;
        }
        let mut fig = self.doc.figure.clone();
        // the nodes placed for her stay put while these move
        let scene = self.doc.scene.clone();
        tools::pin_auto_nodes(&mut fig, &scene, self.cell(), None);
        for (obj, pos) in moves {
            match doc::apply(&mut fig, &Edit::MoveObj { obj: obj.clone(), pos }) {
                Ok(_) => {}
                Err(_) => {
                    tools::move_obj(&mut fig, &obj, pos);
                    if let ObjRef::Element(e) = &obj {
                        for it in fig.items.iter_mut() {
                            if let Item::Blob(b) = it
                                && let Some(x) = b.elements.iter_mut().find(|x| &x.name == e)
                            {
                                x.pos = Some(pos);
                            }
                        }
                    }
                }
            }
        }
        self.commit_grouped(fig, what, group, cx)
    }

    /// Arrow keys: a node by a cell (⌥ half a cell), anything else 0.1 cm (⌥ 0.02 cm).
    pub fn nudge(&mut self, dx: f32, dy: f32, fine: bool, cx: &mut Context<Self>) -> bool {
        let mut moves = Vec::new();
        for obj in self.owners() {
            if matches!(obj, ObjRef::Arrow(_)) {
                continue;
            }
            let Some(pos) = self.model_pos(&obj) else { continue };
            let new = match (pos, &obj) {
                (Pos::Cell(c, r), ObjRef::Node(_)) => {
                    let s = if fine { 0.5 } else { 1.0 };
                    Pos::Cell(c + dx * s, r - dy * s)
                }
                (pos, _) => {
                    let s = if fine { 0.02 } else { 0.1 };
                    let p = match pos {
                        Pos::Abs(x, y) => Pt { x, y },
                        Pos::Cell(c, r) => {
                            let cell = self.cell();
                            Pt { x: c * cell.0, y: -r * cell.1 }
                        }
                    };
                    let r2 = |v: f32| (v * 100.0).round() / 100.0 + 0.0;
                    Pos::Abs(r2(p.x + dx * s), r2(p.y + dy * s))
                }
            };
            moves.push((obj, new));
        }
        let key = format!("nudge:{:?}", self.selection);
        self.move_all(moves, "nudge", Some(key), cx)
    }

    /// Turn the selected shapes by `by` degrees ([ and ], ⟲ ⟳, Rotate 90°).
    pub fn rotate_by(&mut self, by: f32, cx: &mut Context<Self>) -> bool {
        self.for_each_shape("angle", cx, |it| {
            let a = handles::angle_of(it);
            handles::set_angle(it, handles::snap_angle(a + by, true))
        })
    }

    /// Grow or shrink the selected shapes by `f` (− and =).
    pub fn scale_by(&mut self, f: f32, cx: &mut Context<Self>) -> bool {
        let scene = self.doc.scene.clone();
        let fig0 = self.doc.figure.clone();
        let scale = handles::scene_scale(&scene, &fig0);
        self.for_each_shape("size", cx, |it| match it {
            Item::NGon(g) => {
                let r = g.radius.unwrap_or(ntex_figures::layout::NGON_RADIUS);
                g.radius = Some(handles::snap_len(r * f).max(0.3));
                true
            }
            Item::Blob(b) => {
                let (w, h) = b.size.unwrap_or_else(|| {
                    handles::frame(&fig0, &scene, &ObjRef::Blob(b.name.clone())).map(|fr| (2.0 * fr.half.0 / scale, 2.0 * fr.half.1 / scale)).unwrap_or((2.0, 2.0))
                });
                b.size = Some((handles::snap_len(w * f).max(0.4), handles::snap_len(h * f).max(0.4)));
                true
            }
            Item::Cayley(c) if !handles::cayley_resizable(c) && c.radius.is_none() => false,
            Item::Cayley(c) => {
                let r = c.radius.or_else(|| {
                    let fr = handles::frame(&fig0, &scene, &ObjRef::Cayley(c.name.clone()))?;
                    core("layout::cayley_radius_for", || ntex_figures::layout::cayley_radius_for(c, fr.radius / scale)).flatten()
                });
                match r {
                    Some(r) => {
                        c.radius = Some(handles::snap_len(r * f).max(0.3));
                        true
                    }
                    None => false,
                }
            }
            _ => false,
        })
    }

    /// L: the labels of the selected n-gons / Cayley graphs on or off.
    pub fn toggle_labels(&mut self, cx: &mut Context<Self>) -> bool {
        // all on → all off; otherwise all on
        let any_on = self.owners().iter().any(|o| match handles::item_index(&self.doc.figure, o).map(|i| &self.doc.figure.items[i]) {
            Some(Item::NGon(g)) => g.show_labels,
            Some(Item::Cayley(c)) => c.labels,
            _ => false,
        });
        self.for_each_shape("labels", cx, |it| match it {
            Item::NGon(g) => {
                g.show_labels = !any_on;
                true
            }
            Item::Cayley(c) => {
                c.labels = !any_on;
                true
            }
            _ => false,
        })
    }

    /// Apply `f` to every selected shape's item, as one undo step.
    fn for_each_shape(&mut self, what: &str, cx: &mut Context<Self>, mut f: impl FnMut(&mut Item) -> bool) -> bool {
        let owners = self.owners();
        let mut fig = self.doc.figure.clone();
        let mut changed = false;
        for o in owners {
            let Some(i) = handles::item_index(&fig, &o) else { continue };
            let mut item = fig.items[i].clone();
            if !f(&mut item) || item == fig.items[i] {
                continue;
            }
            match doc::apply(&mut fig, &Edit::SetItem { index: i, item: item.clone() }) {
                Ok(_) => {}
                Err(e) if not_built(&e) => fig.items[i] = item,
                Err(e) => {
                    self.toast(format!("! Couldn't change the {what}: {e}"), cx);
                    return false;
                }
            }
            changed = true;
        }
        if !changed {
            return false;
        }
        self.commit_grouped(fig, what, None, cx)
    }

    /// ⌘D: copy the selection, one ⌥-drag's offset away (the last one; else
    /// one cell to the right, past anything in the way), with the arrows
    /// between the copied objects; the copies become the selection.
    pub fn duplicate_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let objs = self.copyable();
        if objs.is_empty() {
            return false;
        }
        let offset = self.clip.last_offset.unwrap_or_else(|| self.default_copy_offset(&objs));
        self.copy_with_offset(&objs, offset, "duplicate", cx)
    }

    /// The selection without a shape's vertices and edges (they copy with the shape).
    fn copyable(&self) -> Vec<ObjRef> {
        self.selection.iter().filter(|o| !matches!(o, ObjRef::CayleyVertex(..) | ObjRef::CayleyEdge(..) | ObjRef::NGonVertex(..) | ObjRef::NGonRotation(_))).cloned().collect()
    }

    /// One cell to the right; further right while a copied node would land on a node.
    fn default_copy_offset(&self, objs: &[ObjRef]) -> Pt {
        let cell = self.cell();
        let scene = &self.doc.scene;
        let nodes: Vec<Pt> = scene.anchors.iter().filter(|(o, _, _)| matches!(o, ObjRef::Node(_))).map(|(_, p, _)| *p).collect();
        let moving: Vec<Pt> = objs.iter().filter_map(|o| if matches!(o, ObjRef::Node(_)) { geom::center_of(scene, o) } else { None }).collect();
        for k in 1..=40 {
            let o = Pt { x: cell.0 * k as f32, y: 0.0 };
            let clash = moving.iter().any(|m| nodes.iter().any(|n| (n.x - m.x - o.x).abs() < 0.25 * cell.0 && (n.y - m.y - o.y).abs() < 0.25 * cell.1));
            if !clash {
                return o;
            }
        }
        Pt { x: cell.0, y: 0.0 }
    }

    /// Copy `objs` `offset` (cm, as drawn) away as one undo step; the copies become the selection.
    fn copy_with_offset(&mut self, objs: &[ObjRef], offset: Pt, what: &str, cx: &mut Context<Self>) -> bool {
        let src = self.doc.figure.clone();
        let mut fig = src.clone();
        match ntex_figures::clip::copy_objects(&mut fig, &src, objs, offset, false) {
            Ok(got) if !got.pairs.is_empty() => {
                if !self.commit_grouped(fig, what, None, cx) {
                    return false;
                }
                let mut sel: Vec<ObjRef> = Vec::new();
                for (_, n) in got.pairs {
                    if !sel.contains(&n) {
                        sel.push(n);
                    }
                }
                self.selection = sel;
                let n = self.selection.len();
                self.toast(format!("Duplicated ({n}) — ⌘Z undoes."), cx);
                true
            }
            Ok(_) => {
                self.toast("An arrow is copied with both of its ends; select them too.", cx);
                false
            }
            Err(e) => {
                self.toast(format!("! Couldn't duplicate: {e}"), cx);
                false
            }
        }
    }

    /// ⌘C / ⌘X: the selection as `.fig` shorthand on the clipboard (⌘X also deletes it).
    pub fn copy_selection(&mut self, cut: bool, cx: &mut Context<Self>) -> bool {
        let objs = self.copyable();
        let Some(text) = ntex_figures::clip::to_shorthand(&self.doc.figure, &objs) else {
            self.toast("Select an object to copy (an arrow only goes with its ends).", cx);
            return false;
        };
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()));
        self.clip.copied = Some(text);
        self.clip.cut = cut;
        self.clip.pasted = None;
        let n = objs.len();
        if cut {
            self.selection = objs;
            self.delete_selection(cx);
            self.toast(format!("Cut ({n}) — ⌘V pastes them back, ⌘Z undoes."), cx);
        } else {
            self.toast(format!("Copied ({n}) as shorthand — ⌘V pastes."), cx);
        }
        true
    }

    /// ⌘V: the clipboard's shorthand, as objects. What this window copied
    /// lands half a cell right and down, and each further paste another
    /// half cell; shorthand from elsewhere lands where it was written (a
    /// pasted-back cut too), then steps the same way. The pasted objects
    /// become the selection.
    pub fn paste(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) else {
            self.toast("Nothing to paste: the clipboard has no text.", cx);
            return false;
        };
        let own = self.clip.copied.as_deref() == Some(text.as_str()) && !self.clip.cut;
        let n = match &self.clip.pasted {
            Some((t, n)) if *t == text => n + 1,
            _ => u32::from(own),
        };
        let cell = self.cell();
        let offset = Pt { x: n as f32 * 0.5 * cell.0, y: -(n as f32) * 0.5 * cell.1 };
        let mut fig = self.doc.figure.clone();
        match ntex_figures::clip::paste_into(&mut fig, &text, offset) {
            Ok(got) => {
                if !self.commit_grouped(fig, "paste", None, cx) {
                    return false;
                }
                let mut sel: Vec<ObjRef> = got.pairs.into_iter().map(|(_, n)| n).collect();
                sel.extend(got.arrows);
                let count = sel.len();
                self.selection = sel;
                self.clip.pasted = Some((text, n));
                self.toast(format!("Pasted ({count}) — ⌘Z undoes."), cx);
                true
            }
            Err(e) => {
                self.toast(format!("Nothing to paste: {e}."), cx);
                false
            }
        }
    }

    /// The colour of the selection (`None`: the default).
    pub fn set_colour(&mut self, colour: Option<&str>, cx: &mut Context<Self>) -> bool {
        let c = colour.map(|s| s.to_string());
        let owners = self.owners();
        let mut fig = self.doc.figure.clone();
        let mut changed = false;
        for o in owners {
            let Some(i) = handles::item_index(&fig, &o) else { continue };
            let mut item = fig.items[i].clone();
            match &mut item {
                Item::Node(n) => n.color = c.clone(),
                Item::Arrow(a) => a.color = c.clone(),
                Item::Blob(b) => b.color = c.clone(),
                Item::NGon(g) => g.color = c.clone(),
                _ => continue,
            }
            if item == fig.items[i] {
                continue;
            }
            match doc::apply(&mut fig, &Edit::SetItem { index: i, item: item.clone() }) {
                Ok(_) => {}
                Err(_) => fig.items[i] = item,
            }
            changed = true;
        }
        changed && self.commit_grouped(fig, "colour", None, cx)
    }

    /// Bring forward / send back: swap the item with its neighbour of a
    /// drawn kind (later items draw on top).
    pub fn reorder(&mut self, forward: bool, cx: &mut Context<Self>) -> bool {
        let owners = self.owners();
        let [o] = owners.as_slice() else { return false };
        let fig0 = &self.doc.figure;
        let Some(i) = handles::item_index(fig0, o) else { return false };
        if fig0.grid_items.iter().any(|(k, _)| *k == i) {
            self.toast("! It's part of a grid block; its order is the block's.", cx);
            return false;
        }
        let drawn = |it: &Item| !matches!(it, Item::Comment { .. } | Item::Grid { .. });
        let j = if forward { (i + 1..fig0.items.len()).find(|&j| drawn(&fig0.items[j])) } else { (0..i).rev().find(|&j| drawn(&fig0.items[j])) };
        let Some(j) = j else {
            self.toast(if forward { "Already in front." } else { "Already at the back." }, cx);
            return false;
        };
        if fig0.grid_items.iter().any(|(k, _)| *k == j) {
            return false;
        }
        let mut fig = fig0.clone();
        fig.items.swap(i, j);
        // arrows and texts are named by index
        let fix = |o: &ObjRef| match o {
            ObjRef::Arrow(k) if *k == i => ObjRef::Arrow(j),
            ObjRef::Arrow(k) if *k == j => ObjRef::Arrow(i),
            ObjRef::Text(k) if *k == i => ObjRef::Text(j),
            ObjRef::Text(k) if *k == j => ObjRef::Text(i),
            o => o.clone(),
        };
        let sel: Vec<ObjRef> = self.selection.iter().map(fix).collect();
        if self.commit_grouped(fig, "order", None, cx) {
            self.selection = sel;
            return true;
        }
        false
    }

    /// Align the selection: left / centre (x) / top / middle (y).
    pub fn align(&mut self, how: Align, cx: &mut Context<Self>) -> bool {
        let scene = self.doc.scene.clone();
        let owners: Vec<ObjRef> = self.owners().into_iter().filter(|o| !matches!(o, ObjRef::Arrow(_))).collect();
        if owners.len() < 2 {
            return false;
        }
        let boxes: Vec<(ObjRef, Rect, Pt)> = owners
            .iter()
            .filter_map(|o| Some((o.clone(), handles::outline(&scene, o)?, geom::center_of(&scene, o)?)))
            .collect();
        let target = match how {
            Align::Left => boxes.iter().map(|b| b.1.min.x).fold(f32::MAX, f32::min),
            Align::Centre => boxes.iter().map(|b| b.2.x).sum::<f32>() / boxes.len() as f32,
            Align::Top => boxes.iter().map(|b| b.1.max.y).fold(f32::MIN, f32::max),
            Align::Middle => boxes.iter().map(|b| b.2.y).sum::<f32>() / boxes.len() as f32,
        };
        let scale = handles::scene_scale(&scene, &self.doc.figure);
        let cell = self.cell();
        let mut moves = Vec::new();
        for (o, r, c) in boxes {
            let d = match how {
                Align::Left => Pt { x: target - r.min.x, y: 0.0 },
                Align::Centre => Pt { x: target - c.x, y: 0.0 },
                Align::Top => Pt { x: 0.0, y: target - r.max.y },
                Align::Middle => Pt { x: 0.0, y: target - c.y },
            };
            let to = Pt { x: c.x + d.x, y: c.y + d.y };
            let pos = self.pos_at_centre(&o, to, scale, cell);
            moves.push((o, pos));
        }
        self.move_all(moves, "alignment", None, cx)
    }

    /// The position that puts `o`'s centre at `to` (cm, as drawn): a cell
    /// position stays in cells (to a hundredth), the rest goes absolute.
    fn pos_at_centre(&self, o: &ObjRef, to: Pt, scale: f32, cell: (f32, f32)) -> Pos {
        match (o, self.model_pos(o)) {
            (ObjRef::Node(_), Some(Pos::Cell(..))) => {
                let r2 = |v: f32| (v * 100.0).round() / 100.0 + 0.0;
                Pos::Cell(r2(to.x / cell.0), r2(-to.y / cell.1))
            }
            _ => geom::abs(Pt { x: to.x / scale, y: to.y / scale }),
        }
    }

    /// Spread three or more selected objects so the gaps between them are
    /// equal, the outermost two staying where they are (one undo step).
    pub fn distribute(&mut self, horizontal: bool, cx: &mut Context<Self>) -> bool {
        let scene = self.doc.scene.clone();
        let owners: Vec<ObjRef> = self.owners().into_iter().filter(|o| !matches!(o, ObjRef::Arrow(_))).collect();
        let mut boxes: Vec<(ObjRef, Rect, Pt)> = owners.iter().filter_map(|o| Some((o.clone(), handles::outline(&scene, o)?, geom::center_of(&scene, o)?))).collect();
        if boxes.len() < 3 {
            self.toast("Distribute needs three or more objects.", cx);
            return false;
        }
        // along the axis, first to last (down the page: top first)
        let (lo, hi): (fn(&Rect) -> f32, fn(&Rect) -> f32) = if horizontal { (|r| r.min.x, |r| r.max.x) } else { (|r| -r.max.y, |r| -r.min.y) };
        boxes.sort_by(|a, b| (lo(&a.1) + hi(&a.1)).total_cmp(&(lo(&b.1) + hi(&b.1))));
        let first = lo(&boxes[0].1);
        let last = hi(&boxes[boxes.len() - 1].1);
        let sizes: f32 = boxes.iter().map(|b| hi(&b.1) - lo(&b.1)).sum();
        let gap = (last - first - sizes) / (boxes.len() - 1) as f32;
        let scale = handles::scene_scale(&scene, &self.doc.figure);
        let cell = self.cell();
        let mut cursor = first;
        let mut moves = Vec::new();
        for (o, r, c) in boxes {
            let d = cursor - lo(&r);
            cursor += hi(&r) - lo(&r) + gap;
            if d.abs() < 1e-4 {
                continue;
            }
            let to = if horizontal { Pt { x: c.x + d, y: c.y } } else { Pt { x: c.x, y: c.y - d } };
            moves.push((o.clone(), self.pos_at_centre(&o, to, scale, cell)));
        }
        if moves.is_empty() {
            self.toast("Already evenly spaced.", cx);
            return false;
        }
        self.move_all(moves, "distribution", None, cx)
    }

    /// The figure's own settings (nothing selected): scale, spacing, label size.
    pub fn set_globals(&mut self, group: Option<String>, cx: &mut Context<Self>, f: impl FnOnce(&mut Globals) -> bool) -> bool {
        let mut fig = self.doc.figure.clone();
        if !f(&mut fig.globals) || fig.globals == self.doc.figure.globals {
            return false;
        }
        self.commit_grouped(fig, "settings", group, cx)
    }

    /// Delete the selection; an n-gon's vertex label is hidden instead (its
    /// entry in `labels` emptied), a Cayley graph's vertex can't go.
    pub fn delete_selected(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            return;
        }
        let labels: Vec<ObjRef> = self.selection.iter().filter(|o| matches!(o, ObjRef::NGonVertex(..))).cloned().collect();
        if !labels.is_empty() {
            let mut fig = self.doc.figure.clone();
            for o in &labels {
                let ObjRef::NGonVertex(_, k) = o else { continue };
                let Some(i) = handles::item_index(&fig, o) else { continue };
                let Item::NGon(mut g) = fig.items[i].clone() else { continue };
                let n = g.n.max(1) as usize;
                if g.labels.len() < n {
                    let have = g.labels.len();
                    g.labels.extend((have..n).map(|k| (k + 1).to_string()));
                }
                let j = handles::label_index_at(&g, *k);
                if j < g.labels.len() {
                    g.labels[j] = String::new();
                }
                match doc::apply(&mut fig, &Edit::SetItem { index: i, item: Item::NGon(g.clone()) }) {
                    Ok(_) => {}
                    Err(_) => fig.items[i] = Item::NGon(g),
                }
            }
            if self.commit_grouped(fig, "label", None, cx) {
                self.selection.retain(|o| !matches!(o, ObjRef::NGonVertex(..)));
                self.toast("Label hidden — L or the panel shows it again; ⌘Z undoes.", cx);
            }
            return;
        }
        if self.selection.iter().all(|o| matches!(o, ObjRef::CayleyVertex(..) | ObjRef::CayleyEdge(..))) {
            self.toast("A Cayley graph's vertices are its group's elements; select the graph to delete it.", cx);
            return;
        }
        let sel: Vec<ObjRef> = self.selection.iter().map(|o| match o {
            ObjRef::CayleyVertex(..) | ObjRef::CayleyEdge(..) => owner(o),
            o => o.clone(),
        }).collect();
        self.selection = sel;
        self.delete_selection(cx);
    }

    /// The figure after a gesture's draft: the selection keeps naming what it named.
    pub fn after_drag(&mut self) {
        self.drag_scene = None;
        self.drag_figure = None;
        self.readout = None;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Centre,
    Top,
    Middle,
}

fn obj_of_item(it: &Item, i: usize) -> Option<ObjRef> {
    Some(match it {
        Item::Node(n) => ObjRef::Node(n.name.clone()),
        Item::Arrow(_) => ObjRef::Arrow(i),
        Item::Blob(b) => ObjRef::Blob(b.name.clone()),
        Item::NGon(g) => ObjRef::NGon(g.name.clone()),
        Item::Cayley(c) => ObjRef::Cayley(c.name.clone()),
        Item::Text(_) => ObjRef::Text(i),
        _ => return None,
    })
}

impl FigureView {
    // -- Tab between labels ------------------------------------------------------------

    /// Tab / ⇧Tab in a label field: set this label and go on to the next one
    /// (in place on the canvas, or the panel's next label field).
    pub fn next_label(&mut self, dir: i32, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(le) = self.label_edit.clone()
            && self.label_field.read(cx).focus.is_focused(window)
        {
            // the next label, in reading order, before this edit changes the picture
            let next = self.neighbour_label(&le.obj, le.at, dir);
            let typed = self.label_field.read(cx).text().trim().to_string();
            let math = self.label_field.read(cx).math();
            let label = self.field_label(&le.obj, cx);
            self.label_edit = None;
            self.label_preview = None;
            if typed != le.before || math != le.before_math {
                self.set_label(&le.obj, &label, None, cx);
            }
            // (a placeholder element may have just taken its name)
            let next = match (next, &le.obj) {
                (Some(ObjRef::Element(e)), ObjRef::Element(me)) if e == *me => self.selection.iter().find(|o| matches!(o, ObjRef::Element(_))).cloned(),
                (n, _) => n,
            };
            match next {
                Some(next) => {
                    self.selection = vec![super::labels::selection_for(&next)];
                    self.start_label_edit(next, window, cx);
                }
                None => {
                    self.canvas_focus.clone().focus(window, cx);
                    cx.notify();
                }
            }
            return;
        }
        let order = self.tab_order.clone();
        let Some(at) = order.iter().position(|k| self.fields.get(k).is_some_and(|f| f.entity.read(cx).focus.is_focused(window))) else { return };
        let next = &order[(at as i32 + dir).rem_euclid(order.len() as i32) as usize];
        if let Some(f) = self.fields.get(next) {
            let e = f.entity.clone();
            self.undo_group = None;
            e.read(cx).focus.clone().focus(window, cx);
            e.update(cx, |f, cx| f.select_all(cx));
        }
        cx.notify();
    }

    // -- Insert (the empty canvas's right-click menu) -------------------------------------

    fn insert_point(&self) -> Pt {
        self.menu.as_ref().and_then(|m| m.at_cm).unwrap_or_default()
    }

    pub fn insert_node(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let p = self.insert_point();
        self.menu = None;
        let pos = geom::snap(p, self.cell(), false);
        let label = doc::fresh_name(&self.doc.figure);
        if let Some(name) = self.add_node_pinned(label, pos, cx) {
            let o = ObjRef::Node(name);
            self.selection = vec![o.clone()];
            self.start_label_edit(o, window, cx);
        }
    }

    pub fn insert_ngon(&mut self, n: u32, cx: &mut Context<Self>) {
        let p = self.insert_point();
        self.menu = None;
        let scale = handles::scene_scale(&self.doc.scene, &self.doc.figure);
        let pos = geom::abs(Pt { x: p.x / scale, y: p.y / scale });
        let before: Vec<String> = self.doc.figure.items.iter().filter_map(|it| if let Item::NGon(g) = it { Some(g.name.clone()) } else { None }).collect();
        let mut fig = self.doc.figure.clone();
        match doc::apply(&mut fig, &Edit::AddNGon { n, pos, radius: 0.0 }) {
            Ok(_) => {
                if self.commit_figure(fig, "a polygon", cx)
                    && let Some(name) = self.doc.figure.items.iter().find_map(|it| match it {
                        Item::NGon(g) if !before.contains(&g.name) => Some(g.name.clone()),
                        _ => None,
                    })
                {
                    self.last_ngon = Some(name.clone());
                    self.selection = vec![ObjRef::NGon(name)];
                }
            }
            Err(e) => self.toast(format!("! Couldn't add a polygon: {e}"), cx),
        }
        cx.notify();
    }

    pub fn insert_blob(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let p = self.insert_point();
        self.menu = None;
        self.add_blob(p, (2.6, 2.0), BlobShape::Round, Vec::new(), window, cx);
    }

    /// A Cayley graph of a named group (its standard generators) here.
    pub fn insert_cayley(&mut self, group: &str, window: &mut Window, cx: &mut Context<Self>) {
        let p = self.insert_point();
        self.menu = None;
        let scale = handles::scene_scale(&self.doc.scene, &self.doc.figure);
        let used = doc::names(&self.doc.figure);
        let name = if used.contains(group) { (1..).map(|k| format!("C{k}")).find(|n| !used.contains(n)).unwrap() } else { group.to_string() };
        let c = Cayley {
            name: name.clone(),
            group: GroupSpec::Named(group.to_string()),
            gens: Vec::new(),
            layout: CayleyLayout::Auto,
            labels: true,
            legend: false,
            radius: None,
            pos: Some(geom::abs(Pt { x: p.x / scale, y: p.y / scale })),
            span: Span::default(),
            angle: 0.0,
            gen_styles: Vec::new(),
            vertex_names: Vec::new(),
        };
        let mut fig = self.doc.figure.clone();
        fig.items.push(Item::Cayley(c));
        if self.commit_figure(fig, "a Cayley graph", cx) {
            // the graph as the parser named it
            let made = self.doc.figure.items.iter().rev().find_map(|it| if let Item::Cayley(c) = it { Some(c.name.clone()) } else { None });
            if let Some(n) = made {
                self.selection = vec![ObjRef::Cayley(n)];
            }
        }
        self.canvas_focus.clone().focus(window, cx);
        cx.notify();
    }

    pub fn insert_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let p = self.insert_point();
        self.menu = None;
        let scale = handles::scene_scale(&self.doc.scene, &self.doc.figure);
        let pos = geom::abs(Pt { x: p.x / scale, y: p.y / scale });
        if self.edit_and_commit(vec![Edit::AddText { text: "text".into(), pos }], "a text", window, cx).is_some()
            && let Some(i) = self.doc.figure.items.iter().rposition(|it| matches!(it, Item::Text(_)))
        {
            let o = ObjRef::Text(i);
            self.selection = vec![o.clone()];
            self.start_label_edit(o, window, cx);
        }
    }

    /// After the item at `index` changed: select it again by its (new) name
    /// (a Cayley graph is named after its group).
    pub fn reselect_item(&mut self, index: usize) {
        if let Some(o) = self.doc.figure.items.get(index).and_then(|it| obj_of_item(it, index)) {
            self.selection = vec![o];
        }
    }
}