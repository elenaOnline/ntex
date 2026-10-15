//! The right-click menu (FIGURES.md "Right-click menu"), set in the LaTeX
//! look: an \fbox on the paper, entries in small caps with their keycaps,
//! groups split by a 0.4 pt rule, and a Colour ▸ list of the palette.
use super::interact::PALETTE;
use super::view::FigureView;
use crate::ui::widgets::controls::Keycap;
use crate::ui::widgets::theme::Theme;
use gpui::{AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Point, SharedString, StatefulInteractiveElement as _, Styled as _, deferred, div, prelude::FluentBuilder as _, px};
use ntex_figures::model::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    /// Where it opens (window px).
    pub at: Point<Pixels>,
    /// Where she clicked (cm): what Insert places goes there.
    pub at_cm: Option<Pt>,
    /// The open ▸ list.
    pub sub: Option<Sub>,
}

/// A menu's second column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sub {
    Colour,
    NGon,
    Cayley,
}

impl Menu {
    pub fn at(at: Point<Pixels>, at_cm: Pt) -> Menu {
        Menu { at, at_cm: Some(at_cm), sub: None }
    }
}

/// The groups Insert ▸ Cayley graph offers (besides Z_n): (shorthand, as shown).
const GROUPS: [(&str, &str); 5] = [("D3", "D₃"), ("D4", "D₄"), ("Q8", "Q₈"), ("S3", "S₃"), ("V4", "V₄")];
const NGONS: [(u32, &str); 6] = [(3, "triangle"), (4, "square"), (5, "pentagon"), (6, "hexagon"), (7, "heptagon"), (8, "octagon")];

/// What an entry does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    EditLabel,
    Duplicate,
    /// (figure-qol A) the clipboard, and spreading three or more evenly
    Copy,
    Cut,
    Paste,
    DistributeH,
    DistributeV,
    Rotate90,
    Labels,
    Colour,
    Forward,
    Back,
    Delete,
    SelectAll,
    /// an arrow: Hide label / Add label
    ArrowLabel,
    /// (figure-qol D) turn an arrow round (a rotation arrow: the other way)
    Reverse,
    InsertNode,
    InsertNGon,
    InsertBlob,
    InsertCayley,
    InsertText,
    /// (figure-qol E) clear the written position: layout places it again
    AutoPlace,
    /// (figure-qol E) a node: add an arrow from it to itself
    Loop,
}

pub const MENU_W: f32 = 236.0;

impl FigureView {
    /// The entries for the current selection: (command, label, keys, enabled), None = a rule.
    pub fn menu_entries(&self) -> Vec<Option<(Cmd, String, &'static str, bool)>> {
        let sel = &self.selection;
        if sel.is_empty() {
            // the empty canvas: what can go here
            return vec![
                Some((Cmd::InsertNode, "Node".into(), "N", true)),
                Some((Cmd::InsertNGon, "n-gon".into(), "▸", true)),
                Some((Cmd::InsertBlob, "Blob".into(), "", true)),
                Some((Cmd::InsertCayley, "Cayley graph".into(), "▸", true)),
                Some((Cmd::InsertText, "Text".into(), "", true)),
                None,
                Some((Cmd::Paste, "Paste".into(), "⌘V", true)),
                Some((Cmd::SelectAll, "Select all".into(), "⌘A", !self.doc.scene.draws.is_empty())),
            ];
        }
        let arrow_label = match sel.as_slice() {
            [ObjRef::Arrow(i)] => match self.doc.figure.items.get(*i) {
                Some(Item::Arrow(a)) => Some(a.label.is_some()),
                _ => None,
            },
            _ => None,
        };
        let one = sel.len() == 1;
        let has_label = one && matches!(sel[0], ObjRef::Node(_) | ObjRef::Arrow(_) | ObjRef::Blob(_) | ObjRef::Element(_) | ObjRef::Text(_) | ObjRef::NGonVertex(..) | ObjRef::NGon(_));
        let shape = sel.iter().any(|o| matches!(super::interact::owner(o), ObjRef::NGon(_) | ObjRef::Blob(_) | ObjRef::Cayley(_)));
        let labelled = sel.iter().any(|o| matches!(super::interact::owner(o), ObjRef::NGon(_) | ObjRef::Cayley(_)));
        let labels_on = sel.iter().any(|o| {
            let o = super::interact::owner(o);
            match super::handles::item_index(&self.doc.figure, &o).map(|i| &self.doc.figure.items[i]) {
                Some(Item::NGon(g)) => g.show_labels,
                Some(Item::Cayley(c)) => c.labels,
                _ => false,
            }
        });
        let colourable = sel.iter().any(|o| matches!(super::interact::owner(o), ObjRef::Node(_) | ObjRef::Arrow(_) | ObjRef::Blob(_) | ObjRef::NGon(_) | ObjRef::Text(_)));
        let dup = sel.iter().any(|o| !matches!(o, ObjRef::CayleyVertex(..) | ObjRef::CayleyEdge(..) | ObjRef::NGonVertex(..)));
        let placed = self.autoplace_targets() > 0;
        let mut v = vec![
            Some((Cmd::EditLabel, "Edit label".into(), "↵", has_label)),
            Some((Cmd::Duplicate, "Duplicate".into(), "⌘D", dup)),
            Some((Cmd::Copy, "Copy".into(), "⌘C", dup)),
            Some((Cmd::Cut, "Cut".into(), "⌘X", dup)),
            Some((Cmd::Paste, "Paste".into(), "⌘V", true)),
            None,
            Some((Cmd::Rotate90, "Rotate 90°".into(), "", shape)),
            Some((Cmd::Labels, if labels_on { "Hide labels".into() } else { "Show labels".into() }, "L", labelled)),
            Some((Cmd::Colour, "Colour".into(), "▸", colourable)),
            None,
            Some((Cmd::AutoPlace, "Auto-place".into(), "", placed)),
            Some((Cmd::Forward, "Bring forward".into(), "", one)),
            Some((Cmd::Back, "Send back".into(), "", one)),
            None,
            Some((Cmd::Delete, if sel.iter().all(|o| matches!(o, ObjRef::NGonVertex(..))) { "Hide label".into() } else { "Delete".into() }, "⌫", true)),
        ];
        let reversible = sel.iter().any(|o| match o {
            ObjRef::Arrow(i) => matches!(self.doc.figure.items.get(*i), Some(Item::Arrow(a)) if !a.is_loop && a.from != a.to),
            ObjRef::NGonRotation(_) => true,
            _ => false,
        });
        if sel.iter().any(|o| matches!(o, ObjRef::Arrow(_) | ObjRef::NGonRotation(_))) {
            let at = v.iter().position(|e| matches!(e, Some((Cmd::Duplicate, ..)))).map(|k| k + 1).unwrap_or(1);
            let label = if sel.iter().all(|o| matches!(o, ObjRef::NGonRotation(_))) { "Reverse direction" } else { "Reverse" };
            v.insert(at, Some((Cmd::Reverse, label.into(), "⌘R", reversible)));
        }
        // (figure-qol A) spread several objects evenly: three or more
        let spreadable = sel.iter().map(super::interact::owner).filter(|o| !matches!(o, ObjRef::Arrow(_))).collect::<std::collections::HashSet<_>>().len();
        if spreadable >= 2 {
            let at = v.iter().position(|e| matches!(e, Some((Cmd::Rotate90, ..)))).map(|k| k + 1).unwrap_or(v.len());
            v.insert(at, Some((Cmd::DistributeV, "Distribute vertically".into(), "", spreadable >= 3)));
            v.insert(at, Some((Cmd::DistributeH, "Distribute horizontally".into(), "", spreadable >= 3)));
        }
        if matches!(sel.as_slice(), [ObjRef::Node(_)]) {
            let at = v.iter().position(|e| matches!(e, Some((Cmd::Duplicate, ..)))).map(|k| k + 1).unwrap_or(1);
            v.insert(at, Some((Cmd::Loop, "Add loop arrow".into(), "", true)));
        }
        if let Some(has) = arrow_label {
            // an arrow's label: hide it (or add one) right here
            v.insert(1, Some((Cmd::ArrowLabel, if has { "Hide label".into() } else { "Add label".into() }, "", true)));
            // (Hide/Show labels is the shapes' vertex labels)
            v.retain(|e| !matches!(e, Some((Cmd::Labels, ..))));
        }
        v
    }

    pub fn run_menu(&mut self, cmd: Cmd, window: &mut gpui::Window, cx: &mut Context<Self>) {
        if !matches!(cmd, Cmd::Colour | Cmd::InsertNGon | Cmd::InsertCayley) {
            self.menu = None;
        }
        match cmd {
            Cmd::EditLabel => {
                if let Some(o) = self.selection.first().cloned() {
                    let o = if let ObjRef::NGon(n) = &o { ObjRef::NGonVertex(n.clone(), 1) } else { o };
                    self.start_label_edit(o, window, cx);
                }
            }
            Cmd::Duplicate => {
                self.duplicate_selection(cx);
            }
            Cmd::Copy => {
                self.copy_selection(false, cx);
            }
            Cmd::Cut => {
                self.copy_selection(true, cx);
            }
            Cmd::Paste => {
                self.paste(cx);
            }
            Cmd::DistributeH => {
                self.distribute(true, cx);
            }
            Cmd::DistributeV => {
                self.distribute(false, cx);
            }
            Cmd::Rotate90 => {
                self.rotate_by(90.0, cx);
            }
            Cmd::AutoPlace => {
                self.auto_place(cx);
            }
            Cmd::Loop => {
                if let Some(ObjRef::Node(n)) = self.selection.first().cloned() {
                    self.add_arrow(n.clone(), n, window, cx);
                    cx.notify();
                    return;
                }
            }
            Cmd::Reverse => {
                self.reverse_selection(cx);
            }
            Cmd::Labels => {
                self.toggle_labels(cx);
            }
            Cmd::Colour | Cmd::InsertNGon | Cmd::InsertCayley => {
                let want = match cmd {
                    Cmd::Colour => Sub::Colour,
                    Cmd::InsertNGon => Sub::NGon,
                    _ => Sub::Cayley,
                };
                if let Some(m) = self.menu.as_mut() {
                    m.sub = if m.sub == Some(want) { None } else { Some(want) };
                }
                if want == Sub::Cayley {
                    // the Z_n field takes the keys at once: type 6, Return
                    let f = self.zn_field.clone();
                    f.update(cx, |f, cx| {
                        f.set_text("6", cx);
                        f.select_all(cx);
                    });
                    f.read(cx).focus.clone().focus(window, cx);
                    cx.notify();
                    return;
                }
            }
            Cmd::ArrowLabel => {
                if let Some(ObjRef::Arrow(i)) = self.selection.first().cloned() {
                    let has = matches!(self.doc.figure.items.get(i), Some(Item::Arrow(a)) if a.label.is_some());
                    if has {
                        self.set_label(&ObjRef::Arrow(i), "", None, cx);
                    } else {
                        self.start_label_edit(ObjRef::Arrow(i), window, cx);
                        cx.notify();
                        return;
                    }
                }
            }
            Cmd::InsertNode => {
                self.insert_node(window, cx);
                cx.notify();
                return;
            }
            Cmd::InsertBlob => {
                self.insert_blob(window, cx);
                cx.notify();
                return;
            }
            Cmd::InsertText => {
                self.insert_text(window, cx);
                cx.notify();
                return;
            }
            Cmd::Forward => {
                self.reorder(true, cx);
            }
            Cmd::Back => {
                self.reorder(false, cx);
            }
            Cmd::Delete => self.delete_selected(window, cx),
            Cmd::SelectAll => self.select_all(cx),
        }
        self.canvas_focus.clone().focus(window, cx);
        cx.notify();
    }

    /// The menu, over everything, at the pointer (kept inside the window).
    pub fn render_menu(&self, t: &Theme, window: &gpui::Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let m = self.menu.as_ref()?;
        let entries = self.menu_entries();
        let mut list = div().flex().flex_col().w(px(MENU_W)).bg(t.paper).border_1().border_color(t.ink).py(px(4.));
        let insert = entries.iter().any(|e| matches!(e, Some((Cmd::InsertNode, ..))));
        if insert {
            list = list.child(div().px(px(12.)).pt(px(2.)).font(t.serif_italic()).text_size(t.small_size).text_color(t.slate).child("Insert here"));
        }
        // the open ▸ list sits level with its row
        let mut y = 4.0 + if insert { 20.0 } else { 0.0 };
        let mut sub_y = 0.0;
        for (i, e) in entries.into_iter().enumerate() {
            match e {
                None => {
                    y += 7.0;
                    list = list.child(div().mx(px(10.)).my(px(3.)).h(px(1.)).bg(Theme::alpha(t.ink, 0.6)))
                }
                Some((cmd, label, keys, enabled)) => {
                    let ink = if enabled { t.ink } else { Theme::alpha(t.ink, 0.4) };
                    let open = match (cmd, m.sub) {
                        (Cmd::Colour, Some(Sub::Colour)) | (Cmd::InsertNGon, Some(Sub::NGon)) | (Cmd::InsertCayley, Some(Sub::Cayley)) => true,
                        _ => false,
                    };
                    if open {
                        sub_y = y;
                    }
                    y += 25.0;
                    let mut row = div()
                        .id(SharedString::from(format!("menu.{i}")))
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .gap(px(12.))
                        .px(px(12.))
                        .min_h(px(25.))
                        .font(t.caps())
                        .text_size(t.body_size)
                        .text_color(ink)
                        .when(open, |d| d.bg(t.theorem.back))
                        .child(label)
                        .when(!keys.is_empty() && keys != "▸", |d| d.child(Keycap::new(keys).disabled(!enabled)))
                        .when(keys == "▸", |d| d.child(div().font(t.math_font()).text_color(ink).child("▸")));
                    if enabled {
                        let hover = t.theorem.back;
                        row = row.cursor_pointer().hover(move |s| s.bg(hover)).on_click(cx.listener(move |v, _, window, cx| v.run_menu(cmd, window, cx)));
                    }
                    let rec = self.ui_bounds.clone();
                    let key = format!("menu.{cmd:?}");
                    row = row.child(record_bounds(rec, key));
                    list = list.child(row.relative());
                }
            }
        }
        let colours = m.sub.map(|sub| {
            let col = match sub {
                Sub::Colour => self.render_colour_list(t, cx),
                Sub::NGon => self.render_ngon_list(t, cx),
                Sub::Cayley => self.render_cayley_list(t, cx),
            };
            div().mt(px(sub_y - 5.0)).child(col).into_any_element()
        });
        // keep it on the window
        let vw = f32::from(window.viewport_size().width);
        let vh = f32::from(window.viewport_size().height);
        let w = MENU_W + if m.sub.is_some() { 190.0 } else { 0.0 };
        let x = f32::from(m.at.x).min(vw - w - 8.0).max(8.0);
        let y = f32::from(m.at.y).min(vh - 300.0).max(8.0);
        let body = div().id("menu.body").occlude().absolute().left(px(x)).top(px(y)).flex().flex_row().items_start().child(list).children(colours);
        // a click anywhere else closes it
        let backdrop = div()
            .id("menu.backdrop")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|v, _, _, cx| {
                v.menu = None;
                cx.notify();
            }))
            .on_mouse_down(gpui::MouseButton::Right, cx.listener(|v, _, _, cx| {
                v.menu = None;
                cx.notify();
            }));
        Some(deferred(div().absolute().top_0().left_0().size_full().child(backdrop).child(body)).with_priority(3).into_any_element())
    }

    fn render_colour_list(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let mut col = div().flex().flex_col().w(px(150.)).bg(t.paper).border_1().border_color(t.ink).py(px(4.)).ml(px(-1.));
        let current = self.selection_colour();
        for (i, name) in std::iter::once("default").chain(PALETTE.iter().copied()).enumerate() {
            let on = match &current {
                Some(Some(c)) => c == name,
                Some(None) => name == "default",
                None => false,
            };
            let swatch_c = if name == "default" { t.ink } else { super::geom::color(name) };
            let hover = t.theorem.back;
            let pick: Option<String> = (name != "default").then(|| name.to_string());
            let rec = self.ui_bounds.clone();
            col = col.child(
                div()
                    .id(SharedString::from(format!("menu.colour.{i}")))
                    .relative()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .px(px(10.))
                    .min_h(px(24.))
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .child(div().size(px(11.)).bg(swatch_c).when(name == "default", |d| d.bg(t.paper).border_1().border_color(t.ink)))
                    .child(div().font(t.caps()).text_size(t.body_size).text_color(if on { t.ink } else { t.slate }).child(name))
                    .when(on, |d| d.child(div().font(t.math_font()).text_color(t.ink).child("✓")))
                    .child(record_bounds(rec, format!("menu.colour.{name}")))
                    .on_click(cx.listener(move |v, _, window, cx| {
                        v.set_colour(pick.as_deref(), cx);
                        v.menu = None;
                        v.canvas_focus.clone().focus(window, cx);
                        cx.notify();
                    })),
            );
        }
        col.into_any_element()
    }

    /// The selection's colour: Some(None) = the default, None = mixed / none.
    pub fn selection_colour(&self) -> Option<Option<String>> {
        let mut out: Option<Option<String>> = None;
        for o in &self.selection {
            let o = super::interact::owner(o);
            let c = match super::handles::item_index(&self.doc.figure, &o).map(|i| &self.doc.figure.items[i]) {
                Some(Item::Node(n)) => n.color.clone(),
                Some(Item::Arrow(a)) => a.color.clone(),
                Some(Item::Blob(b)) => b.color.clone(),
                Some(Item::NGon(g)) => g.color.clone(),
                Some(Item::Text(x)) => x.color.clone(),
                _ => continue,
            };
            match &out {
                None => out = Some(c),
                Some(prev) if *prev != c => return None,
                _ => {}
            }
        }
        out
    }
}

/// An invisible child that records its parent's bounds under `key` (the
/// offscreen tests click there; nothing else reads it).
pub fn record_bounds(rec: std::rc::Rc<std::cell::RefCell<std::collections::HashMap<String, gpui::Bounds<Pixels>>>>, key: String) -> impl IntoElement {
    gpui::canvas(move |b, _, _| {
        rec.borrow_mut().insert(key.clone(), b);
    }, |_, _, _, _| {})
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

impl FigureView {
    /// One row of a ▸ list.
    fn sub_row(&self, t: &Theme, key: String, cx: &mut Context<Self>, label: impl IntoElement, f: impl Fn(&mut FigureView, &mut gpui::Window, &mut Context<FigureView>) + 'static) -> gpui::Stateful<gpui::Div> {
        let hover = t.theorem.back;
        div()
            .id(SharedString::from(key.clone()))
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .px(px(10.))
            .min_h(px(25.))
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .child(label)
            .child(record_bounds(self.ui_bounds.clone(), key))
            .on_click(cx.listener(move |v, _, window, cx| {
                f(v, window, cx);
                cx.notify();
            }))
    }

    fn render_ngon_list(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let mut col = div().flex().flex_col().w(px(180.)).bg(t.paper).border_1().border_color(t.ink).py(px(4.)).ml(px(-1.));
        for (n, name) in NGONS {
            let label = div().flex().flex_row().gap(px(8.)).child(div().w(px(14.)).font(t.mono()).text_color(t.slate).child(n.to_string())).child(div().font(t.caps()).child(name));
            col = col.child(self.sub_row(t, format!("menu.ngon.{n}"), cx, label, move |v, _, cx| v.insert_ngon(n, cx)));
        }
        col.into_any_element()
    }

    fn render_cayley_list(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let mut col = div().flex().flex_col().w(px(180.)).bg(t.paper).border_1().border_color(t.ink).py(px(4.)).ml(px(-1.));
        // Z_n: n in a field, Return inserts
        let zn = div()
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .px(px(10.))
            .min_h(px(28.))
            .child(div().font(t.serif_italic()).child("ℤₙ,"))
            .child(div().font(t.serif_italic()).text_color(t.slate).child("n ="))
            .child(div().relative().child(self.zn_field.clone()).child(record_bounds(self.ui_bounds.clone(), "field.menu.zn".into())))
            .child(Keycap::new("↵"));
        col = col.child(zn);
        for (g, shown) in GROUPS {
            let label = div().font(t.serif_italic()).child(shown);
            col = col.child(self.sub_row(t, format!("menu.cayley.{g}"), cx, label, move |v, w, cx| v.insert_cayley(g, w, cx)));
        }
        col.into_any_element()
    }
}
