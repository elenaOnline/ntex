//! The canvas tools (V N A B P R) and what the mouse does with each.
//! Every gesture ends in canvas edits (`Edit`), applied to the Figure and
//! written back as source by the view ([`FigureView::commit_figure`]).
use super::doc::{self, core};
use super::geom::{self, Xf};
use super::view::FigureView;
use super::handles::{self, DragStart};
use gpui::{Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Window};
use ntex_figures::model::*;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Node,
    Arrow,
    Blob,
    Polygon,
    Rough,
}

impl Tool {
    pub const ALL: [Tool; 6] = [Tool::Select, Tool::Node, Tool::Arrow, Tool::Blob, Tool::Polygon, Tool::Rough];

    pub fn key(self) -> &'static str {
        match self {
            Tool::Select => "V",
            Tool::Node => "N",
            Tool::Arrow => "A",
            Tool::Blob => "B",
            Tool::Polygon => "P",
            Tool::Rough => "R",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Tool::Select => "select",
            Tool::Node => "node",
            Tool::Arrow => "arrow",
            Tool::Blob => "blob",
            Tool::Polygon => "polygon",
            Tool::Rough => "rough",
        }
    }

    /// The footnote for the tool.
    pub fn hint(self) -> &'static str {
        match self {
            Tool::Select => "V click, ⇧-click or drag a box to select · drag the handles to resize and turn · drag moves the selection (⌥ copies, ⇧ keeps a line, ⌘ goes free) · drag an arrow's end dots onto another object · ⌘R reverses an arrow · ⌘C ⌘X ⌘V · double-click a label or ↵ to edit it · ⌫ deletes · right-click for more",
            Tool::Node => "N click: a node on the nearest cell (⌘ anywhere), then type its label",
            Tool::Arrow => "A drag from one object to another: an arrow, then type its label",
            Tool::Blob => "B drag a rectangle: a blob of that size",
            Tool::Polygon => "P click: a square · 3–9 set the number of sides",
            Tool::Rough => "R draw roughly: a loop becomes a blob, corners a polygon, a stroke between objects an arrow",
        }
    }

    /// Cell dots under the select and node tools.
    pub fn shows_grid(self) -> bool {
        matches!(self, Tool::Select | Tool::Node | Tool::Polygon)
    }
}

/// A mouse gesture in progress.
#[derive(Clone, Debug)]
pub enum Gesture {
    /// Select: a press on an object; it (and the rest of the selection) moves
    /// once the mouse has travelled.
    Move(Box<MoveG>),
    /// Arrow: from an object to the cursor.
    Arrow { from: String, from_obj: ObjRef, a: Pt, b: Pt },
    /// Blob: a rectangle.
    Rect { a: Pt, b: Pt },
    /// Rough: the stroke so far.
    Rough { pts: Vec<Pt> },
    /// Select: a handle of the selected object (resize, rotate, bend, label side).
    /// `down_px`/`moved`: nothing happens until the pointer has travelled
    /// (a click on a label's handle stays a click; a second one edits it).
    Handle { start: Box<DragStart>, last: Option<Item>, down_px: Point<Pixels>, moved: bool },
    /// Select: a box dragged on empty canvas; `base` was selected before (⇧).
    Marquee { a: Pt, b: Pt, base: Vec<ObjRef> },
}

/// (figure-qol A) A drag of the selection: every object moves together, each
/// snapping as it would alone; ⇧ locks the axis, ⌘ turns snapping off, ⌥
/// (held as the drag starts) leaves the originals and drags copies.
#[derive(Clone, Debug)]
pub struct MoveG {
    /// What moves (the selection's movable objects), and the one grabbed.
    pub objs: Vec<ObjRef>,
    pub primary: ObjRef,
    pub grab: Pt,
    pub down_px: Point<Pixels>,
    /// The figure the draft is made from: the figure itself, or (⌥) with the copies added.
    pub base: Figure,
    pub moved: bool,
    /// ⌥: the copies are what moves.
    pub copied: bool,
    /// What was selected before the copies took over (Esc gives it back).
    pub sel0: Vec<ObjRef>,
    /// How each object sits at the start (set when the drag starts).
    pub plan: Vec<MoveItem>,
    pub scene: Option<Arc<Scene>>,
    /// The draft's last positions (to skip a repeat), and how far the grabbed object has gone (cm).
    pub last: Option<Vec<(ObjRef, Pos)>>,
    pub shift: Pt,
}

/// One object of a drag: where it starts (cm, as drawn) and the placed parts
/// that go with it (a blob's placed elements and inner blobs).
#[derive(Clone, Debug)]
pub struct MoveItem {
    pub obj: ObjRef,
    pub origin: Pt,
    pub deps: Vec<(ObjRef, Pos)>,
}

impl Gesture {
    /// A selection drag that has really started.
    pub fn is_moving(&self) -> bool {
        matches!(self, Gesture::Move(m) if m.moved)
    }
}

/// How long the ghost of a recognised stroke shows.
pub const GHOST: Duration = Duration::from_millis(150);
/// Mouse travel (px) before a press becomes a drag.
const DRAG_START: f32 = 3.0;

fn movable(obj: &ObjRef) -> bool {
    matches!(obj, ObjRef::Node(_) | ObjRef::Blob(_) | ObjRef::Element(_) | ObjRef::NGon(_) | ObjRef::NGonVertex(..) | ObjRef::Cayley(_) | ObjRef::CayleyVertex(..) | ObjRef::CayleyEdge(..) | ObjRef::Text(_))
}

/// The object that moves when `obj` is dragged (an n-gon's vertex moves the n-gon).
fn mover(obj: &ObjRef) -> ObjRef {
    match obj {
        ObjRef::NGonVertex(n, _) | ObjRef::NGonRotation(n) => ObjRef::NGon(n.clone()),
        ObjRef::CayleyVertex(c, _) | ObjRef::CayleyEdge(c, _) => ObjRef::Cayley(c.clone()),
        other => other.clone(),
    }
}

/// Where `pos` is, as drawn (cm).
pub fn pos_scene(pos: Pos, cell: (f32, f32), scale: f32) -> Pt {
    match pos {
        Pos::Cell(..) => geom::pos_pt(pos, cell),
        Pos::Abs(x, y) => Pt { x: x * scale, y: y * scale },
    }
}

/// `pos` moved by `by` (cm, as drawn).
pub fn shifted(pos: Pos, by: Pt, cell: (f32, f32), scale: f32) -> Pos {
    let r2 = |v: f32| (v * 100.0).round() / 100.0 + 0.0;
    match pos {
        Pos::Cell(c, r) => Pos::Cell(r2(c + by.x / cell.0), r2(r - by.y / cell.1)),
        Pos::Abs(x, y) => Pos::Abs(r2(x + by.x / scale), r2(y + by.y / scale)),
    }
}

/// Where a dragged object lands for the pointer at `at` (cm, as drawn): nodes
/// on cells, shapes on half cells, blobs / texts / elements to a millimetre;
/// `free` (⌘) turns the snapping off.
pub fn landing(obj: &ObjRef, at: Pt, cell: (f32, f32), scale: f32, free: bool) -> Pos {
    let abs = || geom::abs(Pt { x: at.x / scale, y: at.y / scale });
    match obj {
        ObjRef::Blob(_) | ObjRef::Text(_) | ObjRef::Element(_) => abs(),
        _ if free => abs(),
        ObjRef::NGon(_) | ObjRef::Cayley(_) => geom::snap(at, cell, true),
        _ => geom::snap(at, cell, false),
    }
}

/// How each dragged object starts: its place as drawn, and the placed parts
/// that go with it (a blob's placed elements and the blobs inside it, which
/// the layout would otherwise leave behind).
pub fn plan_for(fig: &Figure, scene: &Scene, cell: (f32, f32), objs: &[ObjRef]) -> Vec<MoveItem> {
    let scale = handles::scene_scale(scene, fig);
    objs.iter()
        .map(|o| {
            let origin = match item_pos(fig, o) {
                Some(pos) => pos_scene(pos, cell, scale),
                None => geom::center_of(scene, o).unwrap_or_default(),
            };
            let mut deps: Vec<(ObjRef, Pos)> = Vec::new();
            if let ObjRef::Blob(name) = o {
                let mut family = vec![name.clone()];
                loop {
                    let before = family.len();
                    for it in &fig.items {
                        if let Item::Blob(b) = it
                            && b.parent.as_ref().is_some_and(|p| family.contains(p))
                            && !family.contains(&b.name)
                        {
                            family.push(b.name.clone());
                        }
                    }
                    if family.len() == before {
                        break;
                    }
                }
                for it in &fig.items {
                    let Item::Blob(b) = it else { continue };
                    if !family.contains(&b.name) {
                        continue;
                    }
                    let own = ObjRef::Blob(b.name.clone());
                    if &own != o && !objs.contains(&own) && let Some(pos) = b.pos {
                        deps.push((own, pos));
                    }
                    for e in &b.elements {
                        let eo = ObjRef::Element(e.name.clone());
                        if !objs.contains(&eo) && let Some(pos) = e.pos {
                            deps.push((eo, pos));
                        }
                    }
                }
            }
            MoveItem { obj: o.clone(), origin, deps }
        })
        .collect()
}

/// The written position of an object, if it has one.
pub fn item_pos(fig: &Figure, obj: &ObjRef) -> Option<Pos> {
    for (i, it) in fig.items.iter().enumerate() {
        match (it, obj) {
            (Item::Node(n), ObjRef::Node(m)) if &n.name == m => return n.pos,
            (Item::Blob(b), ObjRef::Blob(m)) if &b.name == m => return b.pos,
            (Item::NGon(g), ObjRef::NGon(m)) if &g.name == m => return g.pos,
            (Item::Cayley(c), ObjRef::Cayley(m) | ObjRef::CayleyVertex(m, _) | ObjRef::CayleyEdge(m, _)) if &c.name == m => return c.pos,
            (Item::Text(t), ObjRef::Text(k)) if i == *k => return Some(t.pos),
            _ => {}
        }
    }
    None
}

/// Move an object (canvas edit when there is one, else directly in the model).
pub fn move_obj(fig: &mut Figure, obj: &ObjRef, pos: Pos) {
    let edit = match obj {
        ObjRef::Node(n) => Some(Edit::MoveNode { name: n.clone(), pos }),
        ObjRef::Blob(n) => Some(Edit::MoveBlob { name: n.clone(), pos }),
        _ => Some(Edit::MoveObj { obj: obj.clone(), pos }),
    };
    if let Some(e) = edit
        && doc::apply(fig, &e).is_ok()
    {
        return;
    }
    for (i, it) in fig.items.iter_mut().enumerate() {
        match (it, obj) {
            (Item::Node(n), ObjRef::Node(m)) if &n.name == m => n.pos = Some(pos),
            (Item::Blob(b), ObjRef::Blob(m)) if &b.name == m => b.pos = Some(pos),
            (Item::NGon(g), ObjRef::NGon(m)) if &g.name == m => g.pos = Some(pos),
            (Item::Cayley(c), ObjRef::Cayley(m) | ObjRef::CayleyVertex(m, _) | ObjRef::CayleyEdge(m, _)) if &c.name == m => c.pos = Some(pos),
            (Item::Text(t), ObjRef::Text(k)) if i == *k => t.pos = pos,
            (Item::Blob(b), ObjRef::Element(e)) => {
                if let Some(x) = b.elements.iter_mut().find(|x| &x.name == e) {
                    x.pos = Some(pos);
                }
            }
            _ => {}
        }
    }
}

impl FigureView {
    /// The object under a point (cm): `recognize::hit`, else the local test.
    pub fn hit(&self, p: Pt, xf: &Xf) -> Option<ObjRef> {
        let scene = self.doc.scene.clone();
        if let Some(Some(o)) = core("recognize::hit", || ntex_figures::recognize::hit(&scene, p)) {
            return Some(o);
        }
        geom::hit_local(&scene, p, 6.0 / xf.scale.max(1.0))
    }

    /// A point of the node / polygon tools: on a cell, or (⌘) where it is.
    fn placed(&self, p: Pt, free: bool) -> Pos {
        if free {
            let scale = handles::scene_scale(&self.doc.scene, &self.doc.figure);
            geom::abs(Pt { x: p.x / scale, y: p.y / scale })
        } else {
            geom::snap(p, self.cell(), false)
        }
    }

    /// The pointer moved during a drag of the selection.
    fn drag_move(&mut self, ev: &MouseMoveEvent, p: Pt, cx: &mut Context<Self>) {
        let Some(Gesture::Move(mut m)) = self.gesture.take() else { return };
        let travel = (f32::from(ev.position.x - m.down_px.x)).hypot(f32::from(ev.position.y - m.down_px.y));
        if !m.moved && travel < DRAG_START {
            self.gesture = Some(Gesture::Move(m));
            return;
        }
        if !m.moved {
            m.moved = true;
            self.begin_move(&mut m, ev.modifiers.alt);
        }
        let scene = m.scene.clone().unwrap_or_else(|| self.doc.scene.clone());
        let scale = handles::scene_scale(&scene, &m.base);
        let cell = geom::cell(&scene, &m.base);
        let mut d = Pt { x: p.x - m.grab.x, y: p.y - m.grab.y };
        // ⇧: along whichever axis has gone further
        if ev.modifiers.shift {
            if d.x.abs() >= d.y.abs() {
                d.y = 0.0;
            } else {
                d.x = 0.0;
            }
        }
        let mut moves: Vec<(ObjRef, Pos)> = Vec::new();
        for it in &m.plan {
            let at = Pt { x: it.origin.x + d.x, y: it.origin.y + d.y };
            let pos = landing(&it.obj, at, cell, scale, ev.modifiers.platform);
            let landed = pos_scene(pos, cell, scale);
            let went = Pt { x: landed.x - it.origin.x, y: landed.y - it.origin.y };
            if it.obj == m.primary {
                m.shift = went;
            }
            moves.push((it.obj.clone(), pos));
            for (o, q) in &it.deps {
                moves.push((o.clone(), shifted(*q, went, cell, scale)));
            }
        }
        if m.last.as_ref() == Some(&moves) {
            self.gesture = Some(Gesture::Move(m));
            return;
        }
        let mut fig = m.base.clone();
        for (o, pos) in &moves {
            move_obj(&mut fig, o, *pos);
        }
        m.last = Some(moves);
        if m.shift.x.abs() < 0.005 && m.shift.y.abs() < 0.005 {
            m.shift = Pt::default();
        }
        self.drag_scene = Some(Arc::new(doc::scene_of(&fig)));
        self.drag_figure = Some(fig);
        self.readout = m.copied.then(|| (p, "copy".to_string()));
        self.gesture = Some(Gesture::Move(m));
        cx.notify();
    }

    /// The drag has travelled: with ⌥ the selection is copied (the copies are
    /// what moves, one undo step with the drop); otherwise the objects the
    /// layout placed for her stay where they are drawn.
    fn begin_move(&mut self, m: &mut MoveG, alt: bool) {
        let cell = self.cell();
        if alt {
            let src = self.doc.figure.clone();
            let mut fig = src.clone();
            let sel: Vec<ObjRef> = self.selection.clone();
            match ntex_figures::clip::copy_objects(&mut fig, &src, &sel, Pt::default(), false) {
                Ok(got) if !got.pairs.is_empty() => {
                    let to_copy = |o: &ObjRef| got.pairs.iter().find(|(a, _)| a == o).map(|(_, b)| b.clone());
                    let objs: Vec<ObjRef> = m.objs.iter().filter_map(to_copy).collect();
                    if !objs.is_empty() {
                        m.primary = to_copy(&m.primary).unwrap_or_else(|| objs[0].clone());
                        m.objs = objs;
                        let mut new_sel: Vec<ObjRef> = Vec::new();
                        for (_, n) in &got.pairs {
                            if !new_sel.contains(n) {
                                new_sel.push(n.clone());
                            }
                        }
                        m.sel0 = std::mem::replace(&mut self.selection, new_sel);
                        m.copied = true;
                        m.scene = Some(Arc::new(doc::scene_of(&fig)));
                        m.base = fig;
                    }
                }
                _ => {}
            }
        }
        if !m.copied {
            // the others placed for her stay where they are drawn (part of this drag's one undo step)
            let scene = self.doc.scene.clone();
            pin_auto_nodes_except(&mut m.base, &scene, cell, &m.objs);
        }
        let scene = m.scene.clone().unwrap_or_else(|| self.doc.scene.clone());
        m.plan = plan_for(&m.base, &scene, cell, &m.objs);
    }

    pub fn cell(&self) -> (f32, f32) {
        geom::cell(&self.doc.scene, &self.doc.figure)
    }

    pub fn set_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        self.tool = tool;
        self.gesture = None;
        self.drag_scene = None;
        self.hover = None;
        cx.notify();
    }

    pub fn canvas_down(&mut self, ev: &MouseDownEvent, xf: Xf, window: &mut Window, cx: &mut Context<Self>) {
        let p = xf.to_cm(ev.position);
        self.canvas_focus.clone().focus(window, cx);
        self.ask.menu_open = false;
        if self.label_edit.is_some() {
            self.commit_label(window, cx);
        }
        let tool = self.tool;
        // right-click (or ⌃-click): the menu, for what is under the pointer
        if ev.button == MouseButton::Right || (ev.button == MouseButton::Left && ev.modifiers.control) {
            if tool == Tool::Select || tool == Tool::Node || tool == Tool::Polygon {
                match self.pick(p, &xf) {
                    Some(o) if !self.selection.contains(&o) => self.selection = vec![o],
                    Some(_) => {}
                    None => self.selection.clear(),
                }
            }
            self.menu = Some(super::menu::Menu::at(ev.position, p));
            cx.notify();
            return;
        }
        if ev.button != MouseButton::Left {
            return;
        }
        if self.menu.take().is_some() {
            cx.notify();
        }
        // (class C) the labels of an n-gon's axes and rotation arrow: a double-click,
        // or a click once the n-gon is selected, edits them in place
        if tool == Tool::Select
            && !ev.modifiers.shift
            && let Some(part) = self.part_label_at(p, &xf)
        {
            let owner = super::labels::selection_for(&part);
            let was_selected = self.selection == vec![owner.clone()];
            self.gesture = None;
            self.selection = vec![owner];
            if ev.click_count >= 2 || was_selected {
                self.start_label_edit(part, window, cx);
                window.prevent_default();
            }
            cx.notify();
            return;
        }
        // a double-click edits the label under it, before any handle takes the press
        if tool == Tool::Select
            && ev.click_count >= 2
            && let Some(raw) = self.pick_raw(p, &xf)
        {
            self.gesture = None;
            if let ObjRef::CayleyVertex(c, w) = &raw
                && !self.selection.iter().any(|s| matches!(s, ObjRef::CayleyVertex(..)))
            {
                self.selection = vec![ObjRef::CayleyVertex(c.clone(), w.clone())];
                cx.notify();
                return;
            }
            let obj = self.promote(raw);
            self.selection = vec![obj.clone()];
            self.start_label_edit(obj, window, cx);
            window.prevent_default();
            return;
        }
        // a handle of the selected object
        self.click_edit = None;
        if tool == Tool::Select
            && let Some(h) = handles::hit(&self.handle_list(xf.scale), ev.position, &xf)
            && let (kind, hobj) = (h.kind, h.obj.clone())
            && self.start_handle(h, p, ev.position)
        {
            // (class C) a click on the label of an already-selected arrow edits it
            // (a drag across still swaps its side; a click alone moves nothing)
            if kind == handles::HKind::ArrowLabel && !ev.modifiers.shift {
                self.click_edit = Some(hobj);
            }
            cx.notify();
            return;
        }
        // an arrow's label, selected or not: a drag across swaps its side
        if tool == Tool::Select
            && !ev.modifiers.shift
            && let Some(i) = self.arrow_label_at(p, &xf)
        {
            // (class C) a click on the label of an arrow that is already selected edits it;
            // on one that is not, it selects the arrow (a drag across swaps its side either way)
            if self.selection == vec![ObjRef::Arrow(i)] {
                self.click_edit = Some(ObjRef::Arrow(i));
            }
            self.selection = vec![ObjRef::Arrow(i)];
            let at = self.doc.scene.draws.iter().find_map(|d| match d {
                Draw::Label { obj: ObjRef::Arrow(k), at, .. } if *k == i => Some(*at),
                _ => None,
            });
            if let Some(at) = at {
                let h = handles::Handle { kind: handles::HKind::ArrowLabel, at, obj: ObjRef::Arrow(i) };
                self.start_handle(h, p, ev.position);
            }
            cx.notify();
            return;
        }
        // (figure-qol D) the rotation arrow is a part of its own: a press
        // selects it, and a drag slides it round to another edge
        if tool == Tool::Select
            && !ev.modifiers.shift
            && ev.click_count < 2
            && let Some(rot @ ObjRef::NGonRotation(_)) = self.pick_raw(p, &xf)
        {
            self.selection = vec![rot.clone()];
            let scene = self.doc.scene.clone();
            if let Some(h) = handles::handles(&self.doc.figure, &scene, &rot, xf.scale).into_iter().next() {
                self.start_handle(h, p, ev.position);
            }
            cx.notify();
            return;
        }
        let raw = if tool == Tool::Select { self.pick_raw(p, &xf) } else { self.hit(p, &xf) };
        let hit = raw.clone().map(|o| self.promote(o));
        if let Some(obj) = hit.clone()
            && matches!(tool, Tool::Select | Tool::Node | Tool::Polygon)
        {
            if ev.click_count >= 2 {
                // a quick second click into a Cayley graph selects the vertex
                if let Some(ObjRef::CayleyVertex(c, w)) = raw.clone()
                    && !matches!(obj, ObjRef::CayleyVertex(..))
                {
                    self.selection = vec![ObjRef::CayleyVertex(c, w)];
                    cx.notify();
                    return;
                }
                self.start_label_edit(obj, window, cx);
                // (the canvas div would take the focus back on this press)
                window.prevent_default();
                return;
            }
            // (class C) Keynote's rule: a click on the label of an object that is
            // already the selection edits it (on mouse-up, if nothing was dragged)
            let on_label = self.label_at(p, &xf).is_some_and(|t| super::labels::selection_for(&t) == obj) || matches!(obj, ObjRef::Node(_) | ObjRef::Element(_));
            if !ev.modifiers.shift && on_label && self.selection == vec![obj.clone()] && !matches!(obj, ObjRef::Cayley(_) | ObjRef::NGon(_)) {
                self.click_edit = Some(obj.clone());
            }
            let mut toggled_off = false;
            if ev.modifiers.shift {
                if let Some(i) = self.selection.iter().position(|o| o == &obj) {
                    self.selection.remove(i);
                    toggled_off = true;
                } else {
                    self.selection.push(obj.clone());
                }
            } else if !self.selection.contains(&obj) {
                self.selection = vec![obj.clone()];
            }
            // (figure-qol A) the whole selection moves with the object grabbed
            if movable(&obj) && !toggled_off {
                let primary = mover(&obj);
                let mut objs: Vec<ObjRef> = Vec::new();
                for o in self.selection.iter().map(mover) {
                    if movable(&o) && !objs.contains(&o) {
                        objs.push(o);
                    }
                }
                if !objs.contains(&primary) {
                    objs.push(primary.clone());
                }
                self.gesture = Some(Gesture::Move(Box::new(MoveG {
                    objs,
                    primary,
                    grab: p,
                    down_px: ev.position,
                    base: self.doc.figure.clone(),
                    moved: false,
                    copied: false,
                    sel0: self.selection.clone(),
                    plan: Vec::new(),
                    scene: None,
                    last: None,
                    shift: Pt::default(),
                })));
            }
            cx.notify();
            return;
        }
        match tool {
            Tool::Select => {
                if !ev.modifiers.shift {
                    self.selection.clear();
                }
                // drag a box to select what it takes in
                self.gesture = Some(Gesture::Marquee { a: p, b: p, base: self.selection.clone() });
            }
            Tool::Node => {
                let pos = self.placed(p, ev.modifiers.platform);
                let label = doc::fresh_name(&self.doc.figure);
                if let Some(name) = self.add_node_pinned(label, pos, cx) {
                    let obj = ObjRef::Node(name);
                    self.selection = vec![obj.clone()];
                    self.start_label_edit(obj, window, cx);
                }
            }
            Tool::Polygon => {
                let pos = self.placed(p, ev.modifiers.platform);
                if let Some(names) = self.edit_and_commit(vec![Edit::AddNGon { n: 4, pos, radius: 0.0 }], "a polygon", window, cx) {
                    let name = names.into_iter().next().flatten().or_else(|| self.last_of_kind_ngon());
                    if let Some(n) = name {
                        self.last_ngon = Some(n.clone());
                        self.selection = vec![ObjRef::NGon(n)];
                    }
                }
            }
            Tool::Arrow => {
                if let Some(obj) = hit
                    && let Some(from) = geom::end_name(&obj)
                {
                    let a = geom::center_of(&self.doc.scene, &obj).unwrap_or(p);
                    self.gesture = Some(Gesture::Arrow { from, from_obj: obj, a, b: p });
                }
            }
            Tool::Blob => self.gesture = Some(Gesture::Rect { a: p, b: p }),
            Tool::Rough => self.gesture = Some(Gesture::Rough { pts: vec![p] }),
        }
        cx.notify();
    }

    pub fn canvas_moved(&mut self, ev: &MouseMoveEvent, xf: Xf, _window: &mut Window, cx: &mut Context<Self>) {
        let p = xf.to_cm(ev.position);
        if matches!(self.gesture, Some(Gesture::Move(_))) {
            if ev.pressed_button.is_some() {
                self.drag_move(ev, p, cx);
            }
            return;
        }
        let Some(g) = self.gesture.as_mut() else {
            let inside = self.canvas_bounds.get().is_some_and(|b| b.contains(&ev.position));
            // hover feedback for the arrow tool's possible ends
            if self.tool == Tool::Arrow && inside {
                let h = self.hit(p, &xf).filter(|o| geom::end_name(o).is_some());
                if h != self.hover {
                    self.hover = h;
                    cx.notify();
                }
            }
            // select: a faint outline on what a click would take, the handle under the pointer
            if self.tool == Tool::Select && self.menu.is_none() && self.label_edit.is_none() {
                let (h, kind) = if inside {
                    match handles::hit(&self.handle_list(xf.scale), ev.position, &xf) {
                        Some(hd) => (None, Some(hd.kind)),
                        None => (self.pick(p, &xf), None),
                    }
                } else {
                    (None, None)
                };
                if h != self.hover || kind != self.hover_handle {
                    self.hover = h;
                    self.hover_handle = kind;
                    cx.notify();
                }
            }
            return;
        };
        if ev.pressed_button.is_none() {
            // the button came up outside the window
            return;
        }
        match g {
            Gesture::Move(_) => unreachable!("handled above"),
            Gesture::Arrow { from_obj, b, .. } => {
                *b = p;
                let from_obj = from_obj.clone();
                let h = self.hit(p, &xf).filter(|o| geom::end_name(o).is_some() && *o != from_obj);
                self.hover = h;
                cx.notify();
            }
            Gesture::Rect { b, .. } => {
                *b = p;
                cx.notify();
            }
            Gesture::Rough { pts } => {
                if pts.last().is_none_or(|q| geom::dist(*q, p) * xf.scale >= 1.5) {
                    pts.push(p);
                    cx.notify();
                }
            }
            Gesture::Handle { down_px, moved, .. } => {
                let travel = (f32::from(ev.position.x - down_px.x)).hypot(f32::from(ev.position.y - down_px.y));
                if !*moved && travel < DRAG_START {
                    return;
                }
                *moved = true;
                if self.arrow_end_moved(p, &xf) || self.handle_moved(p, ev.modifiers.platform, ev.modifiers.alt) {
                    cx.notify();
                }
            }
            Gesture::Marquee { a, b, base } => {
                *b = p;
                let (a, b, base) = (*a, *b, base.clone());
                self.marquee_select(a, b, &base);
                cx.notify();
            }
        }
    }

    pub fn canvas_up(&mut self, ev: &MouseUpEvent, xf: Xf, window: &mut Window, cx: &mut Context<Self>) {
        let click_edit = self.click_edit.take();
        let Some(g) = self.gesture.take() else { return };
        let p = xf.to_cm(ev.position);
        self.hover = None;
        // (class C) pressed on the label of the selected object and released
        // without dragging: edit the label
        if let Some(obj) = click_edit
            && ev.click_count == 1
            && (matches!(&g, Gesture::Handle { moved: false, .. }) || matches!(&g, Gesture::Move(m) if !m.moved))
            && self.selection == vec![obj.clone()]
        {
            self.drag_scene = None;
            self.drag_figure = None;
            self.start_label_edit(obj, window, cx);
            cx.notify();
            return;
        }
        match g {
            Gesture::Move(m) => {
                self.drag_scene = None;
                self.readout = None;
                if m.moved && let Some(fig) = self.drag_figure.take() {
                    if m.copied {
                        // a copy dropped where it started is no copy
                        if m.shift == Pt::default() {
                            self.selection = m.sel0.clone();
                        } else if self.commit_figure(fig, "copy", cx) {
                            self.clip.last_offset = Some(m.shift);
                        } else {
                            self.selection = m.sel0.clone();
                        }
                    } else {
                        self.commit_figure(fig, "move", cx);
                    }
                }
                self.drag_figure = None;
            }
            Gesture::Arrow { from, .. } => {
                let to = self.hit(p, &xf).and_then(|o| geom::end_name(&o));
                if let Some(to) = to {
                    self.add_arrow(from, to, window, cx);
                }
            }
            Gesture::Rect { a, b } => {
                let (w, h) = ((a.x - b.x).abs(), (a.y - b.y).abs());
                if w >= 0.4 && h >= 0.4 {
                    let center = Pt { x: (a.x + b.x) / 2.0, y: (a.y + b.y) / 2.0 };
                    self.add_blob(center, (w, h), BlobShape::Round, Vec::new(), window, cx);
                }
            }
            Gesture::Rough { mut pts } => {
                pts.push(p);
                self.recognise(&pts, window, cx);
            }
            Gesture::Handle { start, last, .. } => self.handle_done(*start, last, cx),
            Gesture::Marquee { a, b, base } => self.marquee_select(a, b, &base),
        }
        cx.notify();
    }

    fn last_of_kind_ngon(&self) -> Option<String> {
        self.doc.figure.items.iter().rev().find_map(|it| if let Item::NGon(g) = it { Some(g.name.clone()) } else { None })
    }

    pub fn add_arrow(&mut self, from: String, to: String, window: &mut Window, cx: &mut Context<Self>) {
        // element to element is a maps-to (her kernel pictures): ↦
        let is_elem = |n: &str| self.doc.figure.items.iter().any(|it| matches!(it, Item::Blob(b) if b.elements.iter().any(|e| e.name == n)));
        let maps_to = is_elem(&from) && is_elem(&to);
        let mut fig = self.doc.figure.clone();
        let made = match doc::apply(&mut fig, &Edit::AddArrow { from: from.clone(), to: to.clone(), label: None }) {
            Ok(r) => r,
            Err(e) => {
                self.toast(format!("! Couldn't add an arrow: {e}"), cx);
                return;
            }
        };
        if maps_to
            && let Some(i) = made.and_then(|s| s.parse::<usize>().ok())
            && let Some(Item::Arrow(a)) = fig.items.get_mut(i)
        {
            a.head = Head::MapsTo;
        }
        let _ = window;
        if self.commit_figure(fig, "an arrow", cx) {
            // the new arrow: the last one between these two
            let idx = self.doc.figure.items.iter().enumerate().rev().find_map(|(i, it)| match it {
                Item::Arrow(a) if a.from == from && a.to == to => Some(i),
                _ => None,
            });
            if let Some(i) = idx {
                let obj = ObjRef::Arrow(i);
                self.selection = vec![obj.clone()];
                self.start_label_edit(obj, window, cx);
            }
        }
    }

    pub fn add_blob(&mut self, center: Pt, size: (f32, f32), shape: BlobShape, encloses: Vec<String>, window: &mut Window, cx: &mut Context<Self>) {
        let name = doc::fresh_name(&self.doc.figure);
        let r = |v: f32| (v * 10.0).round() / 10.0;
        let blob = Blob {
            name: name.clone(),
            label: name.clone(),
            elements: Vec::new(),
            parent: None,
            shaded: false,
            color: None,
            shape,
            size: Some((r(size.0), r(size.1))),
            pos: Some(geom::abs(center)),
            label_place: LabelPlace::Above,
            span: Span::default(),
            angle: 0.0,
        };
        let (mut fig, names) = match self.doc.applied(&[Edit::AddBlob { blob }]) {
            Ok(v) => v,
            Err(e) => {
                self.toast(format!("! Couldn't add a blob: {e}"), cx);
                return;
            }
        };
        let made = names.into_iter().next().flatten().unwrap_or(name);
        // the blobs it was drawn around now sit inside it
        for it in fig.items.iter_mut() {
            if let Item::Blob(b) = it
                && encloses.contains(&b.name)
                && b.name != made
                && b.parent.is_none()
            {
                b.parent = Some(made.clone());
            }
        }
        if self.commit_figure(fig, "blob", cx) {
            let obj = ObjRef::Blob(made);
            self.selection = vec![obj.clone()];
            self.start_label_edit(obj, window, cx);
        }
    }

    /// Rough: a stroke → an object, with a 150 ms ghost of what it became.
    pub fn recognise(&mut self, pts: &[Pt], window: &mut Window, cx: &mut Context<Self>) {
        let scene = self.doc.scene.clone();
        let Some(r) = core("recognize::recognize", || ntex_figures::recognize::recognize(pts, &scene)) else {
            self.toast("! The recogniser is not built yet.", cx);
            return;
        };
        self.ghost = Some((geom::ghost_outline(&r, &scene), self.now(cx)));
        match r {
            Recognized::Node(pos) => {
                let label = doc::fresh_name(&self.doc.figure);
                if let Some(name) = self.edit_and_commit(vec![Edit::AddNode { label, pos }], "a node", window, cx).and_then(|n| n.into_iter().next().flatten()) {
                    let obj = ObjRef::Node(name);
                    self.selection = vec![obj.clone()];
                    self.start_label_edit(obj, window, cx);
                }
            }
            Recognized::Arrow { from, to } => self.add_arrow(from, to, window, cx),
            Recognized::Blob { center, size, shape, encloses } => self.add_blob(center, size, shape, encloses, window, cx),
            Recognized::NGon { n, center, radius } => {
                let r = (radius * 10.0).round() / 10.0;
                if let Some(names) = self.edit_and_commit(vec![Edit::AddNGon { n, pos: geom::abs(center), radius: r.max(0.3) }], "a polygon", window, cx) {
                    if let Some(name) = names.into_iter().next().flatten().or_else(|| self.last_of_kind_ngon()) {
                        self.last_ngon = Some(name.clone());
                        self.selection = vec![ObjRef::NGon(name)];
                    }
                }
            }
            Recognized::Nothing => {
                self.ghost = None;
                self.toast("That stroke didn't read as a node, an arrow, a loop or a polygon.", cx);
            }
        }
    }

    /// Delete the selection (one undo step). Arrows and texts are named by
    /// index, so they go first (highest first), each found again by what it
    /// is; then the rest, by name, skipping what an earlier deletion took
    /// with it (a node takes its arrows).
    pub fn delete_selection(&mut self, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            return;
        }
        let mut fig = self.doc.figure.clone();
        let mut by_index: Vec<(usize, Item)> = self
            .selection
            .iter()
            .filter_map(|o| match o {
                ObjRef::Arrow(i) | ObjRef::Text(i) => fig.items.get(*i).map(|it| (*i, it.clone())),
                _ => None,
            })
            .collect();
        by_index.sort_by(|a, b| b.0.cmp(&a.0));
        // a grid block whose every node goes: the block goes whole (no
        // "converted to statements" comment left behind)
        let gone_nodes: std::collections::HashSet<String> = self.selection.iter().filter_map(|o| if let ObjRef::Node(n) = o { Some(n.clone()) } else { None }).collect();
        remove_whole_grids(&mut fig, &gone_nodes);
        for (i, it) in by_index {
            let same = |x: &Item| match (x, &it) {
                (Item::Arrow(a), Item::Arrow(b)) => a.from == b.from && a.to == b.to && a.label == b.label && a.head == b.head && a.bend == b.bend,
                (Item::Text(a), Item::Text(b)) => a.text == b.text && a.pos == b.pos,
                _ => false,
            };
            let at = if fig.items.get(i).is_some_and(same) { Some(i) } else { fig.items.iter().position(same) };
            let Some(j) = at else { continue };
            let obj = if matches!(it, Item::Arrow(_)) { ObjRef::Arrow(j) } else { ObjRef::Text(j) };
            if let Err(e) = doc::apply(&mut fig, &Edit::Delete { obj }) {
                self.toast(format!("! Couldn't delete: {e}"), cx);
                return;
            }
        }
        for o in self.selection.clone() {
            if matches!(o, ObjRef::Arrow(_) | ObjRef::Text(_)) {
                continue;
            }
            // already gone with something else?
            if handles::item_index(&fig, &o).is_none() {
                continue;
            }
            if let Err(e) = doc::apply(&mut fig, &Edit::Delete { obj: o }) {
                self.toast(format!("! Couldn't delete: {e}"), cx);
                return;
            }
        }
        self.selection.clear();
        self.commit_figure(fig, "delete", cx);
    }

    /// A node from the node tool or the Insert menu: the auto-placed nodes
    /// are pinned where they are drawn first, so nothing else moves (one step).
    pub fn add_node_pinned(&mut self, label: String, pos: Pos, cx: &mut Context<Self>) -> Option<String> {
        let mut fig = self.doc.figure.clone();
        let scene = self.doc.scene.clone();
        pin_auto_nodes(&mut fig, &scene, self.cell(), None);
        match doc::apply(&mut fig, &Edit::AddNode { label, pos }) {
            Ok(name) => {
                if self.commit_figure(fig, "a node", cx) {
                    name
                } else {
                    None
                }
            }
            Err(e) => {
                self.toast(format!("! Couldn't add a node: {e}"), cx);
                None
            }
        }
    }

    /// The arrow whose label is under `p` (cm), if any.
    pub fn arrow_label_at(&self, p: Pt, xf: &Xf) -> Option<usize> {
        let slack = 3.0 / xf.scale.max(1.0);
        self.doc.scene.draws.iter().find_map(|d| match d {
            Draw::Label { obj: ObjRef::Arrow(i), at, latex, size, math, .. } => {
                let r = handles::label_rect(*at, latex, *size, *math);
                (p.x >= r.min.x - slack && p.x <= r.max.x + slack && p.y >= r.min.y - slack && p.y <= r.max.y + slack).then_some(*i)
            }
            _ => None,
        })
    }

    /// 3–9: the selected (or last placed) polygon's number of sides.
    pub fn set_sides(&mut self, n: u32, cx: &mut Context<Self>) -> bool {
        let name = self
            .selection
            .iter()
            .find_map(|o| match o {
                ObjRef::NGon(n) | ObjRef::NGonVertex(n, _) => Some(n.clone()),
                _ => None,
            })
            .or_else(|| self.last_ngon.clone());
        let Some(name) = name else { return false };
        let mut fig = self.doc.figure.clone();
        let mut found = false;
        for it in fig.items.iter_mut() {
            if let Item::NGon(g) = it
                && g.name == name
            {
                let default_labels = g.labels.is_empty() || g.labels.iter().enumerate().all(|(i, l)| *l == (i + 1).to_string());
                g.n = n;
                if default_labels {
                    g.labels = (1..=n).map(|i| i.to_string()).collect();
                } else {
                    g.labels.resize(n as usize, String::new());
                    for (i, l) in g.labels.iter_mut().enumerate() {
                        if l.is_empty() {
                            *l = (i + 1).to_string();
                        }
                    }
                }
                found = true;
            }
        }
        if found {
            self.commit_figure(fig, "sides", cx);
        }
        found
    }
}

/// Pin every auto-placed node (no `at`) at the cell it is drawn in, so
/// moving or adding one doesn't re-flow the rest. `except`: the one being moved.
pub fn pin_auto_nodes(fig: &mut Figure, scene: &Scene, cell: (f32, f32), except: Option<&ObjRef>) {
    pin_auto_nodes_except(fig, scene, cell, except.map(std::slice::from_ref).unwrap_or(&[]));
}

/// `pin_auto_nodes`, leaving several free (the ones being moved together).
pub fn pin_auto_nodes_except(fig: &mut Figure, scene: &Scene, cell: (f32, f32), except: &[ObjRef]) {
    for it in fig.items.iter_mut() {
        if let Item::Node(n) = it
            && n.pos.is_none()
            && !except.contains(&ObjRef::Node(n.name.clone()))
            && let Some((_, p, _)) = scene.anchors.iter().find(|(o, _, _)| *o == ObjRef::Node(n.name.clone()))
        {
            n.pos = Some(geom::snap(*p, cell, false));
        }
    }
}

/// Remove every `grid … end` block all of whose nodes are in `gone`, with
/// its objects and any arrow that ends at one of them (indices kept in step).
pub fn remove_whole_grids(fig: &mut Figure, gone: &std::collections::HashSet<String>) {
    let grids: Vec<usize> = fig.items.iter().enumerate().filter(|(_, it)| matches!(it, Item::Grid { .. })).map(|(i, _)| i).collect();
    let mut kill: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for gi in grids {
        let derived: Vec<usize> = fig.grid_items.iter().filter(|x| x.1 == gi).map(|x| x.0).collect();
        let nodes: Vec<String> = derived.iter().filter_map(|&k| if let Some(Item::Node(n)) = fig.items.get(k) { Some(n.name.clone()) } else { None }).collect();
        if !nodes.is_empty() && nodes.iter().all(|n| gone.contains(n)) {
            kill.insert(gi);
            kill.extend(derived);
            names.extend(nodes);
        }
    }
    if kill.is_empty() {
        return;
    }
    for (i, it) in fig.items.iter().enumerate() {
        if let Item::Arrow(a) = it
            && (names.contains(&a.from) || names.contains(&a.to))
        {
            kill.insert(i);
        }
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
    for (_, at) in fig.hints.globals_at.iter_mut() {
        *at = (0..*at).filter(|i| !kill.contains(i)).count();
    }
    fig.hints.implicit.retain(|n| !names.contains(n));
}
