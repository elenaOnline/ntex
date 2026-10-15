//! The properties panel (FIGURES.md "The properties panel"): a margin
//! column on the canvas's right, set as a `Definition (Properties).` box,
//! with only the fields that apply to the selection. Every change applies
//! live through `Edit::SetItem` (built from the current item) as one undo
//! step; typing in one field is one step however many keys it takes.
//!
//! The text fields are kept in a pool keyed by what they edit, so a field
//! keeps its caret while the figure changes under it; a field she isn't in
//! follows the model.
mod page_panel;

use super::doc;
use super::handles;
use super::interact::{Align, PALETTE};
use super::menu::record_bounds;
use super::view::FigureView;
use crate::ui::widgets::boxes::{BoxKind, TBox};
use crate::ui::widgets::controls::{Button, Checkbox};
use crate::ui::widgets::snippet_field::{SnippetField, SnippetFieldEvent};
use ntex_figures::labeltext::{edit_form, label_from_edit_spelling};
use crate::ui::widgets::theme::{ActiveTheme as _, Theme};
use gpui::{
    AnyElement, App, AppContext as _, Bounds, Context, ElementId, Entity, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, ParentElement as _, Pixels, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, canvas, div, prelude::FluentBuilder as _, px,
};
use ntex_figures::model::*;
use std::cell::Cell;
use std::rc::Rc;

mod multi;

/// The column's width, px.
pub const PANEL_W: f32 = 268.0;
/// Below this window width the panel folds away (⌘I brings it back).
pub const PANEL_MIN_WINDOW: f32 = 1000.0;
/// Field labels' column, px.
const LABEL_W: f32 = 58.0;

type Apply = Rc<dyn Fn(&mut FigureView, &str, &mut Window, &mut Context<FigureView>)>;

pub struct PanelField {
    pub entity: Entity<SnippetField>,
    pub is_label: bool,
    /// (text, maths?) the field had before she started typing in it
    before: Option<(String, bool)>,
    idle_text: String,
    idle_math: bool,
    /// A label stored inside TikZ's maths (as opposed to free text / numbers)
    converts: bool,
    apply: Apply,
    used: bool,
    _sub: Subscription,
}

pub(super) fn fmt_num(v: f32) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() || s == "-0" { "0".into() } else { s }
}

fn parse_num(s: &str) -> Option<f32> {
    let s = s.trim().trim_end_matches("cm").trim_end_matches('°').trim().replace(',', ".").replace('−', "-");
    s.parse::<f32>().ok().filter(|v| v.is_finite())
}

fn head_glyph(h: Head) -> &'static str {
    match h {
        Head::To => "→",
        Head::TwoHeads => "↠",
        Head::Tail => "↣",
        Head::Hook => "↪",
        Head::MapsTo => "↦",
        Head::Double => "⇒",
        Head::Equal => "=",
        Head::Both => "↔",
    }
}

const HEADS: [Head; 8] = [Head::To, Head::TwoHeads, Head::Tail, Head::Hook, Head::MapsTo, Head::Double, Head::Equal, Head::Both];

/// The axis codes an n-gon offers, with their names ("1", "1.5").
fn axis_codes(n: u32) -> Vec<(u32, String)> {
    if n % 2 == 1 {
        (1..=n).map(|k| (2 * k, k.to_string())).collect()
    } else {
        (1..=n / 2).map(|k| (2 * k, k.to_string())).chain((1..=n / 2).map(|k| (2 * k + 1, format!("{k}.5")))).collect()
    }
}

/// What an axis passes through, in vertex numbers: "1–3" (two vertices),
/// "1–3.5" (a vertex and the middle of edge 3–4), "1.5–3.5" (two edges' middles).
pub fn axis_says(n: u32, code: u32) -> String {
    let n = n.max(3);
    let k = code / 2;
    let name = |x2: u32| -> String {
        // x2: twice a position, 2 = vertex 1, 3 = edge 1–2's middle
        let x2 = (x2 - 2) % (2 * n) + 2;
        if x2 % 2 == 0 { (x2 / 2).to_string() } else { format!("{}.5", x2 / 2) }
    };
    let a = if code % 2 == 0 { 2 * k } else { 2 * k + 1 };
    // the far end: half way round
    let b = a + n;
    format!("{}–{}", name(a), name(b))
}

/// An n-gon with `n` sides, keeping her labels (new vertices get numbers).
pub fn with_sides(g: &NGon, n: u32) -> NGon {
    let mut g = g.clone();
    let default_labels = g.labels.is_empty() || g.labels.iter().enumerate().all(|(i, l)| *l == (i + 1).to_string());
    g.n = n;
    if default_labels {
        g.labels = (1..=n).map(|i| i.to_string()).collect();
    } else {
        let have = g.labels.len();
        g.labels.resize(n as usize, String::new());
        for (i, l) in g.labels.iter_mut().enumerate().skip(have) {
            *l = (i + 1).to_string();
        }
    }
    g.turn %= n.max(1);
    g.axes.retain(|c| axis_codes(n).iter().any(|(k, _)| k == c));
    if g.flip.is_some_and(|f| !axis_codes(n).iter().any(|(k, _)| *k == f)) {
        g.flip = None;
    }
    g
}

/// A fresh element name (`x`, `y`, … `x1`).
fn fresh_element(fig: &Figure) -> String {
    let used = doc::names(fig);
    for k in 0.. {
        for c in ["x", "y", "z", "u", "v", "w"] {
            let n = if k == 0 { c.to_string() } else { format!("{c}{k}") };
            if !used.contains(&n) {
                return n;
            }
        }
    }
    unreachable!()
}

/// `<a, b | a^4, b^2>` → generators and relators.
fn parse_presentation(s: &str) -> Option<(Vec<String>, Vec<String>)> {
    let s = s.trim().strip_prefix('<')?.trim_end().strip_suffix('>')?;
    let (g, r) = s.split_once('|').unwrap_or((s, ""));
    let split = |x: &str| x.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect::<Vec<_>>();
    let gens = split(g);
    (!gens.is_empty()).then(|| (gens, split(r)))
}

fn group_text(g: &GroupSpec) -> String {
    match g {
        GroupSpec::Named(n) => n.clone(),
        GroupSpec::Presentation(gens, rels) => {
            if rels.is_empty() { format!("<{}>", gens.join(", ")) } else { format!("<{} | {}>", gens.join(", "), rels.join(", ")) }
        }
    }
}

// ---------------------------------------------------------------------------
// small controls

/// (review U9) A free text item's stored text → the field's text and whether it
/// is maths: `$x^2$` all in one island is maths mode; a `\text{…}` wrapper (older
/// panel edits) is dropped; anything else is text.
fn text_item_form(v: &str) -> (String, bool) {
    let v = v.trim();
    if let Some(inner) = v.strip_prefix(r"\text{").and_then(|r| r.strip_suffix('}')).filter(|i| i.matches('{').count() == i.matches('}').count()) {
        return (inner.to_string(), false);
    }
    if v.len() >= 2 && v.starts_with('$') && v.ends_with('$') && v[1..v.len() - 1].find('$').is_none() {
        return (v[1..v.len() - 1].to_string(), true);
    }
    (v.to_string(), false)
}

/// The inverse: the field's text and mode → what the text item stores.
fn text_item_model(text: &str, math: bool) -> String {
    if math && !text.is_empty() { format!("${text}$") } else { text.to_string() }
}

fn row(t: &Theme, label: &str, content: impl IntoElement) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.))
        .min_h(px(26.))
        .child(div().flex_none().w(px(LABEL_W)).font(t.caps()).text_size(t.body_size).text_color(t.ink).child(label.to_string()))
        .child(div().flex_1().min_w_0().flex().flex_row().flex_wrap().items_center().gap(px(6.)).child(content))
}

/// A label on its own line, the control under it (for wide controls).
fn stack(t: &Theme, label: &str, content: impl IntoElement) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .pt(px(2.))
        .child(div().font(t.caps()).text_size(t.body_size).text_color(t.ink).child(label.to_string()))
        .child(content)
}

fn unit(t: &Theme, s: &str) -> gpui::Div {
    div().flex_none().font(t.serif()).text_color(t.slate).child(s.to_string())
}

fn note(t: &Theme, s: impl Into<SharedString>) -> gpui::Div {
    div().font(t.serif_italic()).text_size(t.small_size).line_height(t.line(t.small_size)).text_color(t.slate).child(s.into())
}

/// A small toggle chip (an \fbox; filled when on).
pub(super) fn chip(t: &Theme, id: impl Into<ElementId>, label: &str, on: bool, mono: bool) -> gpui::Stateful<gpui::Div> {
    let hover = t.theorem.back;
    div()
        .id(id)
        .relative()
        .flex_none()
        .px(px(5.))
        .min_w(px(24.))
        .h(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(if on { t.ink } else { Theme::alpha(t.slate, 0.7) })
        .when(on, |d| d.bg(t.theorem.back))
        .when(!on, move |d| d.hover(move |s| s.bg(hover)))
        .cursor_pointer()
        .font(if mono { t.mono() } else { t.serif() })
        .text_size(if mono { t.mono_size } else { t.body_size })
        .text_color(if on { t.ink } else { t.slate })
        .child(label.to_string())
}

/// ● a ○ b ○ c, wrapping (her radio, as an inline enumerate).
fn choice<F>(t: &Theme, id: &str, options: &[&str], selected: Option<usize>, rec: &BoundsRec, on: F) -> gpui::Div
where
    F: Fn(usize, &mut Window, &mut App) + 'static,
{
    let on = Rc::new(on);
    let mut d = div().flex().flex_row().flex_wrap().items_center().gap_x(px(10.)).gap_y(px(2.));
    for (i, o) in options.iter().enumerate() {
        let sel = selected == Some(i);
        let f = on.clone();
        d = d.child(
            div()
                .id(SharedString::from(format!("{id}.{i}")))
                .relative()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .min_h(px(24.))
                .cursor_pointer()
                .child(div().font(t.math_font()).text_color(if sel { t.ink } else { t.slate }).child(if sel { "●" } else { "○" }))
                .child(div().text_color(if sel { t.ink } else { t.slate }).child(o.to_string()))
                .child(record_bounds(rec.clone(), format!("{id}.{o}")))
                .on_click(move |_, w, cx| f(i, w, cx)),
        );
    }
    d
}

type BoundsRec = Rc<std::cell::RefCell<std::collections::HashMap<String, Bounds<Pixels>>>>;

#[derive(Default)]
struct SliderState {
    bounds: Cell<Option<Bounds<Pixels>>>,
    dragging: Cell<bool>,
}

/// A plain slider: a slate rule with an ink ● (as the hue slider, without the hues).
#[derive(IntoElement)]
pub struct Slider {
    id: ElementId,
    value: f32,
    min: f32,
    max: f32,
    on_start: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_change: Option<Rc<dyn Fn(f32, &mut Window, &mut App)>>,
}

impl Slider {
    pub fn new(id: impl Into<ElementId>, value: f32, min: f32, max: f32) -> Slider {
        Slider { id: id.into(), value, min, max, on_start: None, on_change: None }
    }
    pub fn on_start(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Slider {
        self.on_start = Some(Rc::new(f));
        self
    }
    pub fn on_change(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Slider {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Slider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme().clone();
        let state_id = ElementId::NamedChild(std::sync::Arc::new(self.id.clone()), "state".into());
        let state = window.use_keyed_state(state_id, cx, |_, _| Rc::new(SliderState::default())).read(cx).clone();
        let (min, max) = (self.min, self.max);
        let frac = ((self.value - min) / (max - min)).clamp(0.0, 1.0);
        let at = move |b: Bounds<Pixels>, x: Pixels| -> f32 {
            let w = f32::from(b.size.width).max(1.0);
            min + (f32::from(x - b.origin.x) / w).clamp(0.0, 1.0) * (max - min)
        };
        let st = state.clone();
        let track = div()
            .relative()
            .w_full()
            .h(px(1.))
            .bg(t.slate)
            .child(canvas(move |b, _, _| st.bounds.set(Some(b)), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
            .child(
                div()
                    .absolute()
                    .top(px(-5.5))
                    .left(gpui::relative(frac))
                    .ml(px(-6.))
                    .size(px(12.))
                    .rounded_full()
                    .bg(t.paper)
                    .border(px(1.5))
                    .border_color(t.ink),
            );
        let (s1, s2) = (state.clone(), state.clone());
        let (c1, c2) = (self.on_change.clone(), self.on_change);
        let start = self.on_start;
        div()
            .id(self.id)
            .flex_1()
            .min_w(px(60.))
            .py(px(11.))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, window, cx| {
                if let Some(b) = s1.bounds.get() {
                    s1.dragging.set(true);
                    if let Some(f) = start.as_ref() {
                        f(window, cx);
                    }
                    if let Some(f) = c1.as_ref() {
                        f(at(b, e.position.x), window, cx);
                    }
                }
            })
            .on_mouse_move(move |e: &MouseMoveEvent, window, cx| {
                if !s2.dragging.get() {
                    return;
                }
                if e.pressed_button != Some(MouseButton::Left) {
                    s2.dragging.set(false);
                    return;
                }
                if let (Some(b), Some(f)) = (s2.bounds.get(), c2.as_ref()) {
                    f(at(b, e.position.x), window, cx);
                }
            })
            .on_mouse_up(MouseButton::Left, {
                let s = state.clone();
                move |_, _, _| s.dragging.set(false)
            })
            .on_mouse_up_out(MouseButton::Left, move |_, _, _| state.dragging.set(false))
            .child(track)
    }
}

// ---------------------------------------------------------------------------

impl FigureView {
    /// Is the panel showing? (auto: when the window is wide enough; ⌘I sets it)
    pub fn panel_shown(&self, window: &Window) -> bool {
        self.panel_open.unwrap_or(f32::from(window.viewport_size().width) >= PANEL_MIN_WINDOW)
    }

    /// A text field from the pool: `value` is the model's (shown unless she
    /// is typing in it); `apply` runs on every change.
    #[allow(clippy::too_many_arguments)]
    fn field(
        &mut self,
        key: &str,
        value: &str,
        mono: bool,
        width: Option<f32>,
        placeholder: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
        apply: impl Fn(&mut FigureView, &str, &mut Window, &mut Context<FigureView>) + 'static,
    ) -> AnyElement {
        let is_label = key.starts_with("ngonv.") || key.starts_with("cayleyv.") || key.starts_with("elem.") || key.starts_with("text.")
            || key.contains(".label.") || key.ends_with(".label") || key.contains(".elem.");
        // Free text items already store text-mode TeX, including $math$
        // islands. Other labels are maths by default (no `$` shown) with a
        // `text` chip; they are stored inside TikZ's math delimiters.
        // (review U9) a text item is shown as its text, with the same chip
        let text_item = key.starts_with("text.");
        let converts = is_label;
        let (value, value_math) = if text_item { text_item_form(value) } else if converts { edit_form(value) } else { (value.to_string(), false) };
        let apply: Apply = Rc::new(move |v, text, w, cx| apply(v, text, w, cx));
        if !self.fields.contains_key(key) {
            let ph = placeholder.to_string();
            let entity = cx.new(|cx| {
                let mut f = SnippetField::new(cx).placeholder(ph);
                if is_label { f = f.with_session(self.label_session.clone()); }
                if converts && value_math { f = f.math_mode(); }
                if mono {
                    f = f.mono();
                }
                if let Some(w) = width {
                    f = f.width(px(w));
                }
                f
            });
            let k = key.to_string();
            let sub = cx.subscribe_in(&entity, window, move |v: &mut FigureView, _, ev: &SnippetFieldEvent, window, cx| match ev {
                SnippetFieldEvent::Changed => v.panel_field_changed(&k, window, cx),
                SnippetFieldEvent::Cancel => {
                    let restore = v.fields.get_mut(&k).and_then(|f| f.before.take().filter(|_| f.is_label).map(|b| (f.apply.clone(), f.converts, b)));
                    if let Some((apply, converts, (text, math))) = restore {
                        let model = if k.starts_with("text.") { text_item_model(text.trim(), math) } else if converts { label_from_edit_spelling(text.trim(), math) } else { text };
                        apply(v, &model, window, cx);
                    }
                    v.undo_group = None;
                    v.canvas_focus.clone().focus(window, cx);
                    cx.notify();
                }
                SnippetFieldEvent::Submit => {
                    if let Some(f) = v.fields.get_mut(&k) { f.before = None; }
                    v.undo_group = None;
                    v.canvas_focus.clone().focus(window, cx);
                    cx.notify();
                }
                SnippetFieldEvent::Walk(dir) => v.next_label(*dir, window, cx),
                SnippetFieldEvent::Blur => {
                    if let Some(f) = v.fields.get_mut(&k) { f.before = None; }
                    v.undo_group = None;
                    cx.notify();
                }
            });
            self.fields.insert(key.to_string(), PanelField { entity, is_label, before: None, idle_text: value.clone(), idle_math: value_math, converts, apply: apply.clone(), used: true, _sub: sub });
        }
        let pf = self.fields.get_mut(key).unwrap();
        pf.apply = apply;
        pf.used = true;
        let entity = pf.entity.clone();
        // the placeholder follows the model (r → r⁻¹ when the arrow turns round)
        if entity.read(cx).placeholder.as_ref() != placeholder {
            let ph = placeholder.to_string();
            entity.update(cx, |f, cx| {
                f.placeholder = ph.into();
                cx.notify();
            });
        }
        let focused = entity.read(cx).focus.is_focused(window);
        if focused && pf.before.is_none() { pf.before = Some((pf.idle_text.clone(), pf.idle_math)); }
        if !focused { pf.idle_text = value.clone(); pf.idle_math = value_math; }
        if !focused && converts && entity.read(cx).math() != value_math {
            entity.update(cx, |f, cx| f.set_math(value_math, cx));
        }
        if !focused && entity.read(cx).text() != value.as_str() {
            let v = value.to_string();
            entity.update(cx, |f, cx| f.set_text(v, cx));
        }
        let chip = (converts && width.is_none()).then(|| {
            let t = cx.theme().clone();
            let math = entity.read(cx).math();
            let k = key.to_string();
            chip(&t, SharedString::from(format!("chip.{key}")), "text", !math, false)
                .on_click(cx.listener(move |v, _, window, cx| v.panel_toggle_mode(&k, window, cx)))
                .child(record_bounds(self.ui_bounds.clone(), format!("chip.{key}")))
                .into_any_element()
        });
        let tab = self.tab_order.iter().any(|k| k == key);
        div()
            .relative()
            .when(tab, |d| d.key_context(super::view::LABEL_CONTEXT))
            .when(width.is_none(), |d| d.flex_1().min_w(px(40.)).flex().flex_row().items_center().gap(px(4.)))
            .when_some(width, |d, w| d.flex_none().w(px(w)))
            .child(div().flex_1().min_w_0().when_some(width, |d, w| d.w(px(w)).flex_none()).child(entity))
            .children(chip)
            .child(record_bounds(self.ui_bounds.clone(), format!("field.{key}")))
            .into_any_element()
    }

    /// A label field's text (or mode) changed: apply it live.
    fn panel_field_changed(&mut self, k: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pf) = self.fields.get_mut(k) else { return };
        if pf.before.is_none() { pf.before = Some((pf.idle_text.clone(), pf.idle_math)); }
        let (text, math) = { let e = pf.entity.read(cx); (e.text().to_string(), e.math()) };
        let model = if k.starts_with("text.") { text_item_model(text.trim(), math) } else if pf.converts { label_from_edit_spelling(text.trim(), math) } else { text };
        let f = pf.apply.clone();
        f(self, &model, window, cx);
    }

    /// The panel's `text` chip: this label becomes text (or maths again).
    fn panel_toggle_mode(&mut self, k: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pf) = self.fields.get(k) else { return };
        let e = pf.entity.clone();
        let m = e.read(cx).math();
        e.update(cx, |f, cx| f.set_math(!m, cx));
        e.read(cx).focus.clone().focus(window, cx);
        self.panel_field_changed(k, window, cx);
    }

    /// Run `f` on the view from a control's callback.
    fn on<F>(&self, cx: &mut Context<Self>, f: F) -> impl Fn(&mut Window, &mut App) + 'static
    where
        F: Fn(&mut FigureView, &mut Window, &mut Context<FigureView>) + 'static,
    {
        let me = cx.entity().downgrade();
        let f = Rc::new(f);
        move |window, cx| {
            let f = f.clone();
            let _ = me.update(cx, |v, cx| f(v, window, cx));
        }
    }

    fn button(&self, id: &str, label: &str, cx: &mut Context<Self>, f: impl Fn(&mut FigureView, &mut Window, &mut Context<FigureView>) + 'static) -> AnyElement {
        let go = self.on(cx, f);
        div()
            .relative()
            .flex_none()
            .child(Button::new(SharedString::from(format!("panel.{id}")), label.to_string()).small().on_click(move |_, w, cx| go(w, cx)))
            .child(record_bounds(self.ui_bounds.clone(), format!("button.{id}")))
            .into_any_element()
    }

    /// A glyph button (⟲ ⟳): an \fbox chip with the glyph in the maths face.
    fn glyph(&self, t: &Theme, id: &str, glyph: &str, cx: &mut Context<Self>, f: impl Fn(&mut FigureView, &mut Window, &mut Context<FigureView>) + 'static) -> AnyElement {
        let go = self.on(cx, f);
        div()
            .relative()
            .flex_none()
            .child(
                chip(t, SharedString::from(format!("panel.{id}")), glyph, false, false)
                    .font(t.math_font())
                    .text_size(px(17.))
                    .text_color(t.ink)
                    .border_color(t.ink)
                    .w(px(30.))
                    .h(px(26.))
                    .on_click(move |_, w, cx| go(w, cx)),
            )
            .child(record_bounds(self.ui_bounds.clone(), format!("button.{id}")))
            .into_any_element()
    }

    fn check(&self, id: &str, label: &str, on: bool, cx: &mut Context<Self>, f: impl Fn(&mut FigureView, bool, &mut Context<FigureView>) + 'static) -> AnyElement {
        let me = cx.entity().downgrade();
        let f = Rc::new(f);
        div()
            .relative()
            .flex_none()
            .child(Checkbox::new(SharedString::from(format!("panel.{id}")), label.to_string(), on).on_toggle(move |on, _, cx| {
                let f = f.clone();
                let _ = me.update(cx, |v, cx| f(v, on, cx));
            }))
            .child(record_bounds(self.ui_bounds.clone(), format!("check.{id}")))
            .into_any_element()
    }

    fn colours(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let current = self.selection_colour();
        let mut d = div().flex().flex_row().flex_wrap().items_center().gap(px(1.));
        for name in std::iter::once("default").chain(PALETTE.iter().copied()) {
            let on = match &current {
                Some(Some(c)) => c == name,
                Some(None) => name == "default",
                None => false,
            };
            let c = if name == "default" { t.paper } else { super::geom::color(name) };
            let pick: Option<String> = (name != "default").then(|| name.to_string());
            let go = self.on(cx, move |v, _, cx| {
                v.set_colour(pick.as_deref(), cx);
            });
            d = d.child(
                div()
                    .relative()
                    .flex_none()
                    .child(
                        div()
                            .id(SharedString::from(format!("panel.colour.{name}")))
                            .size(px(24.))
                            .p(px(3.5))
                            .border(px(1.5))
                            .border_color(if on { t.ink } else { gpui::transparent_black() })
                            .cursor_pointer()
                            .child(div().size_full().bg(c).when(name == "default", |d| d.border_1().border_color(t.slate)))
                            .on_click(move |_, w, cx| go(w, cx)),
                    )
                    .child(record_bounds(self.ui_bounds.clone(), format!("colour.{name}"))),
            );
        }
        let name = match &current {
            Some(Some(c)) => c.clone(),
            Some(None) => "default".into(),
            None => "mixed".into(),
        };
        div().flex().flex_col().gap(px(1.)).child(d).child(note(t, name)).into_any_element()
    }

    /// Angle: field °, ⟲ ⟳ 15°.
    fn angle_rows(&mut self, t: &Theme, key: &str, obj: &ObjRef, angle: f32, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let o = obj.clone();
        let fkey = format!("{key}.angle");
        let g = format!("field:{fkey}");
        let field = self.field(&fkey, &fmt_num(angle), true, Some(52.), "0", window, cx, move |v, s, _, cx| {
            if let Some(a) = parse_num(s) {
                let a = handles::snap_angle(a, true);
                v.edit_obj(&o, "angle", Some(g.clone()), cx, |it| handles::set_angle(it, a));
            }
        });
        let ccw = self.glyph(t, &format!("{key}.ccw"), "⟲", cx, |v, _, cx| {
            v.rotate_by(15.0, cx);
        });
        let cw = self.glyph(t, &format!("{key}.cw"), "⟳", cx, |v, _, cx| {
            v.rotate_by(-15.0, cx);
        });
        vec![row(t, "Angle", div().flex().flex_row().items_center().gap(px(6.)).child(field).child(unit(t, "°")).child(ccw).child(cw)).into_any_element()]
    }

    /// The panel's body for the selection.
    pub fn render_panel(&mut self, t: &Theme, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        for f in self.fields.values_mut() {
            f.used = false;
        }
        self.tab_order.clear();
        let sel = self.selection.clone();
        let (what, mut body): (String, Vec<AnyElement>) = match sel.as_slice() {
            [] => ("the figure".into(), vec![note(t, "Click an object to see its properties; drag on empty canvas to select several.").into_any_element()]),
            [one] => self.panel_one(t, one, window, cx),
            many => (format!("{} objects", many.len()), self.panel_many(t, cx)),
        };
        // (figure-qol B) the figure's own section is always there, under the selection's
        // (review U4) folded, its head is pinned under the scrolling part so it
        // stays reachable however long the selection's section is; open, the
        // whole section scrolls (its fields are the point)
        let mut section = self.panel_figure_section(t, window, cx);
        let pinned = (!self.figure_section_open()).then(|| section.remove(0));
        body.extend(section);
        // a field she is still in stays; the rest of the unused go
        self.fields.retain(|_, f| f.used || f.entity.read(cx).focus.is_focused(window));
        let head = div().font(t.serif_italic()).text_color(t.slate).child(what);
        let boxed = TBox::new(BoxKind::Definition, "Definition").name("Properties").own_line().child(head).children(body);
        let more = {
            let m = self.panel_scroll.max_offset();
            let off = self.panel_scroll.offset();
            f32::from(m.y) > 1.0 && f32::from(off.y) > -f32::from(m.y) + 1.0
        };
        let tones = t.tones(BoxKind::Definition);
        div()
            .relative()
            .flex_none()
            .w(px(PANEL_W))
            .flex()
            .flex_col()
            .min_h_0()
            .overflow_hidden()
            .child(super::menu::record_bounds(self.ui_bounds.clone(), "panel.box".to_string()))
            .child(
                div()
                    .id("panel")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.panel_scroll)
                    .child(boxed),
            )
            // a rule where the panel is cut: there is more below
            .when(more, |d| d.child(div().flex_none().h(px(1.)).bg(Theme::alpha(t.slate, 0.55))))
            .children(pinned.map(|p| {
                div()
                    .flex_none()
                    .bg(tones.back)
                    .border_l(px(3.))
                    .border_color(tones.rule)
                    .pl(px(11.))
                    .pr(px(10.))
                    .pb(px(7.))
                    .child(p)
            }))
            .into_any_element()
    }

    /// `on` for a callback that also takes an index.
    fn on2<F>(&self, cx: &mut Context<Self>, f: F) -> impl Fn(usize, &mut Window, &mut App) + 'static
    where
        F: Fn(&mut FigureView, usize, &mut Context<FigureView>) + 'static,
    {
        let me = cx.entity().downgrade();
        let f = Rc::new(f);
        move |i, _, cx| {
            let f = f.clone();
            let _ = me.update(cx, |v, cx| f(v, i, cx));
        }
    }

    fn panel_many(&mut self, t: &Theme, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let colours = self.colours(t, cx);
        let del = self.button("many.delete", "Delete", cx, |v, w, cx| v.delete_selected(w, cx));
        let al = |v: &FigureView, id: &str, label: &str, how: Align, cx: &mut Context<FigureView>| v.button(id, label, cx, move |v, _, cx| {
            v.align(how, cx);
        });
        let a1 = al(self, "align.left", "left", Align::Left, cx);
        let a2 = al(self, "align.centre", "centre", Align::Centre, cx);
        let a3 = al(self, "align.top", "top", Align::Top, cx);
        let a4 = al(self, "align.middle", "middle", Align::Middle, cx);
        // (figure-qol A) three or more can be spread evenly
        let spread = (self.owners_count() >= 3).then(|| {
            let h = self.button("distribute.h", "across", cx, |v, _, cx| {
                v.distribute(true, cx);
            });
            let d = self.button("distribute.v", "down", cx, |v, _, cx| {
                v.distribute(false, cx);
            });
            row(t, "Distribute", div().flex().flex_row().flex_wrap().gap(px(5.)).child(h).child(d)).into_any_element()
        });
        let mut out = vec![
            stack(t, "Colour", colours).into_any_element(),
            row(t, "Align", div().flex().flex_row().flex_wrap().gap(px(5.)).child(a1).child(a2).child(a3).child(a4)).into_any_element(),
        ];
        out.extend(spread);
        out.push(row(t, "", del).into_any_element());
        out.extend(self.shared_sections(t, cx));
        out
    }

    fn panel_one(&mut self, t: &Theme, obj: &ObjRef, window: &mut Window, cx: &mut Context<Self>) -> (String, Vec<AnyElement>) {
        // mid-drag the panel follows the draft (the numbers move with the pointer)
        let fig = match &self.gesture {
            Some(super::tools::Gesture::Handle { .. }) => self.drag_figure.clone().unwrap_or_else(|| self.doc.figure.clone()),
            _ => self.doc.figure.clone(),
        };
        let Some(i) = handles::item_index(&fig, obj) else { return ("—".into(), vec![]) };
        let item = fig.items[i].clone();
        match (obj, item) {
            (ObjRef::NGon(_), Item::NGon(g)) => (format!("n-gon {}", g.name), self.panel_ngon(t, obj, i, g, window, cx)),
            (ObjRef::NGonRotation(_), Item::NGon(g)) => (format!("rotation arrow of {}", g.name), self.panel_rotation(t, obj, i, g, &fig, window, cx)),
            (ObjRef::NGonVertex(_, k), Item::NGon(g)) => {
                let j = handles::label_index_at(&g, *k);
                let typed = ntex_figures::spelling::raw(&fig, &format!("ngon:{}.{}", g.name, j + 1), g.labels.get(j).map(|s| s.as_str()).unwrap_or(""));
                let o = obj.clone();
                let f = self.field(&format!("ngonv.{}.{}", g.name, k), &typed, true, None, "hidden", window, cx, move |v, s, _, cx| {
                    v.set_label(&o, s, Some(format!("field:vlabel:{o:?}")), cx);
                });
                let name = g.name.clone();
                let up = self.button("vertex.parent", &format!("Polygon {name}"), cx, move |v, _, cx| {
                    v.selection = vec![ObjRef::NGon(name.clone())];
                    cx.notify();
                });
                let hide = self.button("vertex.hide", "Hide", cx, |v, w, cx| v.delete_selected(w, cx));
                (
                    format!("vertex {k} of {}", g.name),
                    vec![
                        row(t, "Label", f).into_any_element(),
                        row(t, "", div().flex().flex_row().gap(px(6.)).child(hide).child(up)).into_any_element(),
                        note(t, "Empty hides the label; ↵ or a double-click edits it on the canvas.").into_any_element(),
                    ],
                )
            }
            (ObjRef::Node(_), Item::Node(n)) => (format!("node {}", n.name), self.panel_node(t, obj, n, window, cx)),
            (ObjRef::Arrow(_), Item::Arrow(a)) => (format!("arrow {} → {}", a.from, a.to), self.panel_arrow(t, obj, i, a, window, cx)),
            (ObjRef::Blob(_), Item::Blob(b)) => (format!("blob {}", b.name), self.panel_blob(t, obj, i, b, window, cx)),
            (ObjRef::Element(e), Item::Blob(b)) => {
                let typed = ntex_figures::spelling::as_typed(&fig, obj).unwrap_or_else(|| e.clone());
                let o = obj.clone();
                let f = self.field(&format!("elem.{e}"), &typed, true, None, e, window, cx, move |v, s, _, cx| {
                    if !s.trim().is_empty() {
                        v.set_label(&o, s, Some(format!("field:elabel:{o:?}")), cx);
                    }
                });
                let bn = b.name.clone();
                let up = self.button("element.parent", &format!("Blob {bn}"), cx, move |v, _, cx| {
                    v.selection = vec![ObjRef::Blob(bn.clone())];
                    cx.notify();
                });
                let mut rows = vec![row(t, "Label", f).into_any_element(), row(t, "In", up).into_any_element()];
                rows.extend(self.pos_rows(t, &format!("epos.{e}"), obj, window, cx));
                (format!("element {e} of {}", b.name), rows)
            }
            (ObjRef::Cayley(_), Item::Cayley(c)) => (format!("Cayley graph {}", c.name), self.panel_cayley(t, obj, i, c, window, cx)),
            (ObjRef::CayleyVertex(_, w), Item::Cayley(c)) => {
                let cn = c.name.clone();
                let up = self.button("cayley.parent", &format!("Graph {cn}"), cx, move |v, _, cx| {
                    v.selection = vec![ObjRef::Cayley(cn.clone())];
                    cx.notify();
                });
                let typed = ntex_figures::spelling::as_typed(&fig, obj).unwrap_or_else(|| w.clone());
                let o = obj.clone();
                let f = self.field(&format!("cayleyv.{}.{}", c.name, w), &typed, true, None, w, window, cx, move |v, s, _, cx| {
                    v.set_label(&o, s, Some(format!("field:cvlabel:{o:?}")), cx);
                });
                (
                    format!("vertex {} of {}", doc::preview(w), c.name),
                    vec![
                        row(t, "Name", f).into_any_element(),
                        note(t, "The element's word by default; empty puts it back. ↵ or a double-click on the vertex edits it on the canvas.").into_any_element(),
                        row(t, "", up).into_any_element(),
                    ],
                )
            }
            (ObjRef::Text(_), Item::Text(x)) => {
                let typed = ntex_figures::spelling::raw(&fig, "text", &x.text);
                let o = obj.clone();
                let f = self.field(&format!("text.{i}"), &typed, false, None, "text", window, cx, move |v, s, _, cx| {
                    v.set_label(&o, s, Some(format!("field:text:{o:?}")), cx);
                });
                let mut rows = vec![row(t, "Text", f).into_any_element()];
                rows.extend(self.text_rows(t, i, &x, cx));
                rows.extend(self.pos_rows(t, &format!("txtpos.{i}"), obj, window, cx));
                ("text".into(), rows)
            }
            _ => ("—".into(), vec![]),
        }
    }

    fn panel_ngon(&mut self, t: &Theme, obj: &ObjRef, i: usize, g: NGon, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut out: Vec<AnyElement> = Vec::new();
        let name = g.name.clone();
        let key = format!("ngon.{name}");
        // sides
        let n = g.n;
        let (g1, g2) = (g.clone(), g.clone());
        let minus = self.button(&format!("{key}.sides-"), "−", cx, move |v, _, cx| {
            if g1.n > 3 {
                v.set_item(i, Item::NGon(with_sides(&g1, g1.n - 1)), "sides", None, vec![], cx);
            }
        });
        let plus = self.button(&format!("{key}.sides+"), "+", cx, move |v, _, cx| {
            if g2.n < 12 {
                v.set_item(i, Item::NGon(with_sides(&g2, g2.n + 1)), "sides", None, vec![], cx);
            }
        });
        out.push(row(t, "Sides", div().flex().flex_row().items_center().gap(px(8.)).child(minus).child(div().font(t.mono()).child(n.to_string())).child(plus)).into_any_element());
        // size
        let r = g.radius.unwrap_or(ntex_figures::layout::NGON_RADIUS);
        let o = obj.clone();
        let gk = format!("field:{key}.radius");
        let field = self.field(&format!("{key}.radius"), &fmt_num(r), true, Some(44.), "1.1", window, cx, move |v, s, _, cx| {
            if let Some(x) = parse_num(s).filter(|x| *x >= 0.2) {
                v.edit_obj(&o, "size", Some(gk.clone()), cx, |it| {
                    if let Item::NGon(g) = it {
                        g.radius = Some(x);
                    }
                    true
                });
            }
        });
        let o2 = obj.clone();
        let sk = format!("slider:{key}.radius");
        let start = self.on(cx, |v, _, _| v.undo_group = None);
        let change = self.on2f(cx, move |v, x, cx| {
            let x = handles::snap_len(x);
            v.edit_obj(&o2, "size", Some(sk.clone()), cx, |it| {
                if let Item::NGon(g) = it {
                    g.radius = Some(x);
                }
                true
            });
        });
        let slider = Slider::new(SharedString::from(format!("{key}.radius.slider")), r, 0.4, 4.0).on_start(start).on_change(change);
        out.push(row(t, "Size", div().w_full().flex().flex_row().items_center().gap(px(6.)).child(field).child(unit(t, "cm")).child(div().relative().flex_1().child(slider).child(record_bounds(self.ui_bounds.clone(), format!("slider.{key}.radius"))))).into_any_element());
        // angle
        out.extend(self.angle_rows(t, &key, obj, g.angle, window, cx));
        let (gu, gf) = (g.clone(), g.clone());
        let up = self.button(&format!("{key}.pointup"), "point up", cx, move |v, _, cx| {
            // a vertex at the top: vertex 1 sits at base + angle
            let n = gu.n.max(3);
            let base = ntex_figures::layout::ngon_base_angle(n);
            let step = 360.0 / n as f32;
            let a = handles::snap_angle((90.0 - base).rem_euclid(step), true);
            let mut g = gu.clone();
            g.angle = a;
            v.set_item(i, Item::NGon(g), "angle", None, vec![], cx);
        });
        let flat = self.button(&format!("{key}.flat"), "flat bottom", cx, move |v, _, cx| {
            let n = gf.n.max(3);
            let base = ntex_figures::layout::ngon_base_angle(n);
            let step = 360.0 / n as f32;
            // an edge's midpoint straight down
            let a = handles::snap_angle((270.0 - base - step / 2.0).rem_euclid(step), true);
            let mut g = gf.clone();
            g.angle = a;
            v.set_item(i, Item::NGon(g), "angle", None, vec![], cx);
        });
        out.push(row(t, "", div().flex().flex_row().gap(px(6.)).child(up).child(flat)).into_any_element());
        // labels
        let gl = g.clone();
        let labels_on = self.check(&format!("{key}.labels"), "Labels", g.show_labels, cx, move |v, on, cx| {
            let mut g = gl.clone();
            g.show_labels = on;
            v.set_item(i, Item::NGon(g), "labels", None, vec![], cx);
        });
        out.push(row(t, "", labels_on).into_any_element());
        let fig = self.doc.figure.clone();
        let mut grid = div().flex().flex_row().flex_wrap().gap_x(px(8.)).gap_y(px(2.)).pl(px(LABEL_W + 6.));
        for j in 0..g.n as usize {
            self.tab_order.push(format!("{key}.label.{j}"));
        }
        for j in 0..g.n as usize {
            let latex = g.labels.get(j).cloned().unwrap_or_else(|| (j + 1).to_string());
            let typed = ntex_figures::spelling::raw(&fig, &format!("ngon:{name}.{}", j + 1), &latex);
            let nm = name.clone();
            let f = self.field(&format!("{key}.label.{j}"), &typed, true, Some(64.), "hidden", window, cx, move |v, s, _, cx| {
                let Some(i) = handles::item_index(&v.doc.figure, &ObjRef::NGon(nm.clone())) else { return };
                let Item::NGon(mut g) = v.doc.figure.items[i].clone() else { return };
                v.set_ngon_label(i, &mut g, &nm, j, s, Some(format!("field:ngon.{nm}.label.{j}")), cx);
            });
            grid = grid.child(div().flex().flex_row().items_center().gap(px(3.)).child(div().font(t.mono()).text_size(t.small_size).text_color(t.slate).child(format!("{}", j + 1))).child(f));
        }
        out.push(grid.into_any_element());
        // axes
        let ga = g.clone();
        let all = self.check(&format!("{key}.axes"), "All axes", g.all_axes, cx, move |v, on, cx| {
            let mut g = ga.clone();
            g.all_axes = on;
            if on {
                g.axes.clear();
            }
            v.set_item(i, Item::NGon(g), "axes", None, vec![], cx);
        });
        let mut chips = div().flex().flex_row().flex_wrap().gap(px(4.));
        for (code, label) in axis_codes(g.n) {
            let on = g.all_axes || g.axes.contains(&code);
            let gc = g.clone();
            let go = self.on(cx, move |v, _, cx| {
                let mut g = gc.clone();
                if g.all_axes {
                    // from "all": every axis but this one
                    g.all_axes = false;
                    g.axes = axis_codes(g.n).into_iter().map(|(c, _)| c).filter(|c| *c != code).collect();
                } else if let Some(p) = g.axes.iter().position(|c| *c == code) {
                    g.axes.remove(p);
                } else {
                    g.axes.push(code);
                    g.axes.sort();
                }
                v.set_item(i, Item::NGon(g), "axes", None, vec![], cx);
            });
            let says = axis_says(g.n, code);
            let gp = g.clone();
            let me = cx.entity().downgrade();
            chips = chips.child(div().relative().flex_none().child(
                chip(t, SharedString::from(format!("{key}.axis.{code}")), &says, on, true)
                    .on_hover(move |hovered, _, cx| {
                        let hovered = *hovered;
                        let gp = gp.clone();
                        let _ = me.update(cx, |v, cx| {
                            v.preview_axis(i, &gp, code, hovered);
                            cx.notify();
                        });
                    })
                    .on_click(move |_, w, cx| go(w, cx)),
            ).child(record_bounds(self.ui_bounds.clone(), format!("axis.{label}"))).child(record_bounds(self.ui_bounds.clone(), format!("axis.{says}"))));
        }
        out.push(row(t, "Axes", div().flex().flex_col().gap(px(3.)).child(all).child(chips)).into_any_element());
        // (class C) a name at the end of each drawn axis (s, sr, …)
        let drawn: Vec<(u32, String)> = axis_codes(g.n).into_iter().filter(|(c, _)| g.all_axes || g.axes.contains(c)).collect();
        if !drawn.is_empty() {
            let mut names = div().flex().flex_row().flex_wrap().gap_x(px(8.)).gap_y(px(2.)).pl(px(LABEL_W + 6.));
            for (code, says) in drawn {
                let o = ObjRef::NGonAxis(name.clone(), code);
                let typed = ntex_figures::spelling::as_typed(&fig, &o).unwrap_or_default();
                let f = self.field(&format!("{key}.label.axis{code}"), &typed, true, Some(64.), "none", window, cx, move |v, s, _, cx| {
                    v.set_label(&o, s, Some(format!("field:axislabel:{o:?}")), cx);
                });
                names = names.child(div().flex().flex_row().items_center().gap(px(3.)).child(div().font(t.mono()).text_size(t.small_size).text_color(t.slate).child(says)).child(f));
            }
            out.push(row(t, "Names", note(t, "a label at each axis's end")).into_any_element());
            out.push(names.into_any_element());
        }
        // rotation arrow
        let gr = g.clone();
        let rot_on = self.check(&format!("{key}.rotation"), "Rotation arrow", g.rotation > 0, cx, move |v, on, cx| {
            let mut g = gr.clone();
            g.rotation = if on { 1 } else { 0 };
            v.set_item(i, Item::NGon(g), "rotation arrow", None, vec![], cx);
        });
        let mut rot_row = div().flex().flex_row().flex_wrap().items_center().gap(px(6.)).child(rot_on);
        if g.rotation > 0 {
            rot_row = rot_row.child(self.rotation_controls(t, &key, i, &g, cx));
        }
        out.push(row(t, "", rot_row).into_any_element());
        if g.rotation > 0 {
            // (class C) the arrow's own name (r, r², ρ, …)
            let o = ObjRef::NGonRotationLabel(name.clone());
            // (review U12) the default is a placeholder (r⁻¹), not text to edit
            let typed = if g.rotation_label.is_none() { String::new() } else { ntex_figures::spelling::as_typed(&fig, &o).unwrap_or_default() };
            let ph = doc::preview(&ntex_figures::layout::ngon_rotation_default(&g));
            let f = self.field(&format!("{key}.label.rot"), &typed, true, None, &ph, window, cx, move |v, s, _, cx| {
                v.set_label(&o, s, Some(format!("field:rotlabel:{o:?}")), cx);
            });
            out.push(row(t, "Name", f).into_any_element());
        }
        // the group action: turn k (either way), flip k
        let (gtm, gtp) = (g.clone(), g.clone());
        let tm = self.glyph(t, &format!("{key}.turn-"), "⟳", cx, move |v, _, cx| {
            v.set_item(i, Item::NGon(step_turn(&gtm, -1)), "turn", None, vec![], cx);
        });
        let tp = self.glyph(t, &format!("{key}.turn+"), "⟲", cx, move |v, _, cx| {
            v.set_item(i, Item::NGon(step_turn(&gtp, 1)), "turn", None, vec![], cx);
        });
        out.push(row(t, "Turn", div().flex().flex_row().items_center().gap(px(8.))// (review U10) counter-clockwise left, clockwise right, like Angle and Rotation
            .child(tp).child(div().font(t.mono()).child(signed_turn(&g).to_string())).child(tm).child(note(t, "relabels"))).into_any_element());
        let mut flips = div().flex().flex_row().flex_wrap().gap(px(4.));
        let gn = g.clone();
        let go_none = self.on(cx, move |v, _, cx| {
            let mut g = gn.clone();
            g.flip = None;
            v.set_item(i, Item::NGon(g), "flip", None, vec![], cx);
        });
        flips = flips.child(chip(t, SharedString::from(format!("{key}.flip.none")), "none", g.flip.is_none(), false).on_click(move |_, w, cx| go_none(w, cx)));
        for (code, _) in axis_codes(g.n) {
            let label = axis_says(g.n, code);
            let gf = g.clone();
            let go = self.on(cx, move |v, _, cx| {
                let mut g = gf.clone();
                g.flip = if g.flip == Some(code) { None } else { Some(code) };
                v.set_item(i, Item::NGon(g), "flip", None, vec![], cx);
            });
            flips = flips.child(chip(t, SharedString::from(format!("{key}.flip.{code}")), &label, g.flip == Some(code), true).on_click(move |_, w, cx| go(w, cx)));
        }
        out.push(row(t, "Flip", flips).into_any_element());
        // fill, colour
        let gs = g.clone();
        let fill = self.check(&format!("{key}.fill"), "Fill", g.shaded, cx, move |v, on, cx| {
            let mut g = gs.clone();
            g.shaded = on;
            v.set_item(i, Item::NGon(g), "fill", None, vec![], cx);
        });
        out.push(row(t, "", fill).into_any_element());
        let colours = self.colours(t, cx);
        out.push(stack(t, "Colour", colours).into_any_element());
        out.extend(self.pos_rows(t, &key, obj, window, cx));
        out
    }

    /// (figure-qol D) One row per generator: its colour, g or g⁻¹, shown or hidden.
    fn gen_rows(&mut self, t: &Theme, i: usize, c: &Cayley, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Ok(grp) = ntex_figures::cayley::group(&c.group, &c.gens) else { return vec![] };
        let mut out: Vec<AnyElement> = Vec::new();
        for (k, (gname, table)) in grp.gens.iter().enumerate() {
            let invol = (0..table.len()).all(|x| table[table[x]] == x);
            let st = ntex_figures::layout::gen_style_of(c, gname).cloned().unwrap_or(GenStyle { generator: gname.clone(), ..Default::default() });
            let auto = ntex_figures::layout::GEN_COLORS[k % ntex_figures::layout::GEN_COLORS.len()];
            let shown = self.check(&format!("cayley.{}.gen{k}.shown", c.name), "shown", !st.hidden, cx, {
                let (c0, g0) = (c.clone(), gname.clone());
                move |v, on, cx| {
                    v.set_item(i, Item::Cayley(with_gen_style(&c0, &g0, |s| s.hidden = !on)), "generator", None, vec![], cx);
                }
            });
            let mut line = div().flex().flex_row().flex_wrap().items_center().gap(px(6.)).child(div().min_w(px(22.)).font(t.serif_italic()).text_color(super::geom::color(st.color.as_deref().unwrap_or(auto))).child(doc::preview(gname)));
            if !invol {
                for (inv, label) in [(false, "g"), (true, "g⁻¹")] {
                    let (c0, g0) = (c.clone(), gname.clone());
                    let go = self.on(cx, move |v, _, cx| {
                        v.set_item(i, Item::Cayley(with_gen_style(&c0, &g0, |s| s.inverse = inv)), "generator", None, vec![], cx);
                    });
                    line = line.child(div().relative().child(chip(t, SharedString::from(format!("panel.cayley.{}.gen{k}.{}", c.name, if inv { "inv" } else { "fwd" })), label, st.inverse == inv, false).on_click(move |_, w, cx| go(w, cx))).child(record_bounds(self.ui_bounds.clone(), format!("cayley.{}.gen{k}.{}", c.name, if inv { "inv" } else { "fwd" }))));
                }
            }
            line = line.child(shown);
            let mut sw = div().flex().flex_row().flex_wrap().items_center().gap(px(1.));
            for name in std::iter::once("auto").chain(PALETTE.iter().copied()) {
                let on = match (&st.color, name) {
                    (None, "auto") => true,
                    (Some(cn), n) => cn == n,
                    _ => false,
                };
                let col = if name == "auto" { super::geom::color(auto) } else { super::geom::color(name) };
                let (c0, g0) = (c.clone(), gname.clone());
                let pick: Option<String> = (name != "auto").then(|| name.to_string());
                let go = self.on(cx, move |v, _, cx| {
                    v.set_item(i, Item::Cayley(with_gen_style(&c0, &g0, |s| s.color = pick.clone())), "generator colour", None, vec![], cx);
                });
                sw = sw.child(
                    div().relative().flex_none().child(
                        div()
                            .id(SharedString::from(format!("panel.cayley.{}.gen{k}.colour.{name}", c.name)))
                            .size(px(24.))
                            .p(px(3.5))
                            .border(px(1.5))
                            .border_color(if on { t.ink } else { gpui::transparent_black() })
                            .cursor_pointer()
                            .child(div().size_full().bg(col).when(name == "auto", |d| d.opacity(0.55)))
                            .on_click(move |_, w, cx| go(w, cx)),
                    ).child(record_bounds(self.ui_bounds.clone(), format!("cayley.{}.gen{k}.colour.{name}", c.name))),
                );
            }
            out.push(row(t, if k == 0 { "Edges" } else { "" }, div().flex().flex_col().gap(px(2.)).child(line).child(sw)).into_any_element());
        }
        out
    }

    /// ↺ ↻ and the step count of a polygon's rotation arrow (the panel and the arrow's own panel).
    fn rotation_controls(&mut self, t: &Theme, key: &str, i: usize, g: &NGon, cx: &mut Context<Self>) -> AnyElement {
        let (g1, g2, gm, gp) = (g.clone(), g.clone(), g.clone(), g.clone());
        let ccw = chip(t, SharedString::from(format!("{key}.rot.ccw")), "↺", !g.rotation_cw, false).font(t.math_font()).text_size(px(17.));
        let go = self.on(cx, move |v, _, cx| {
            let mut g = g1.clone();
            g.rotation_cw = false;
            v.set_item(i, Item::NGon(g), "rotation arrow", None, vec![], cx);
        });
        let cw = chip(t, SharedString::from(format!("{key}.rot.cw")), "↻", g.rotation_cw, false).font(t.math_font()).text_size(px(17.));
        let go2 = self.on(cx, move |v, _, cx| {
            let mut g = g2.clone();
            g.rotation_cw = true;
            v.set_item(i, Item::NGon(g), "rotation arrow", None, vec![], cx);
        });
        let m = self.button(&format!("{key}.rot-"), "−", cx, move |v, _, cx| {
            let mut g = gm.clone();
            g.rotation = (g.rotation.max(2)) - 1;
            v.set_item(i, Item::NGon(g), "rotation arrow", None, vec![], cx);
        });
        let p = self.button(&format!("{key}.rot+"), "+", cx, move |v, _, cx| {
            let mut g = gp.clone();
            g.rotation = (g.rotation + 1).min(g.n.max(2) - 1);
            v.set_item(i, Item::NGon(g), "rotation arrow", None, vec![], cx);
        });
        let k = ntex_figures::layout::rotation_default_label(g.rotation % g.n.max(1).max(1), g.rotation_cw);
        let k = if g.rotation % g.n.max(1) == 0 { ntex_figures::layout::rotation_default_label(g.n.max(1), g.rotation_cw) } else { k };
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(6.))
            .child(div().relative().child(ccw.on_click(move |_, w, cx| go(w, cx))).child(record_bounds(self.ui_bounds.clone(), format!("{key}.rot.ccw"))))
            .child(div().relative().child(cw.on_click(move |_, w, cx| go2(w, cx))).child(record_bounds(self.ui_bounds.clone(), format!("{key}.rot.cw"))))
            .child(m)
            .child(div().font(t.serif_italic()).child(doc::preview(&k)))
            .child(p)
            .into_any_element()
    }

    /// The rotation arrow selected as a part of its own.
    fn panel_rotation(&mut self, t: &Theme, obj: &ObjRef, i: usize, g: NGon, fig: &Figure, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let key = format!("ngon.{}", g.name);
        let controls = self.rotation_controls(t, &key, i, &g, cx);
        let n = g.n.max(3);
        let edge = g.rotation_edge.unwrap_or_else(|| ntex_figures::layout::rotation_default_edge(n));
        let (ga, gb) = (g.clone(), g.clone());
        let prev = self.glyph(t, &format!("{key}.edge-"), "◂", cx, move |v, _, cx| {
            let mut g = ga.clone();
            let cur = g.rotation_edge.unwrap_or_else(|| ntex_figures::layout::rotation_default_edge(n));
            g.rotation_edge = Some((cur + n - 2) % n + 1);
            v.set_item(i, Item::NGon(g), "rotation edge", None, vec![], cx);
        });
        let next = self.glyph(t, &format!("{key}.edge+"), "▸", cx, move |v, _, cx| {
            let mut g = gb.clone();
            let cur = g.rotation_edge.unwrap_or_else(|| ntex_figures::layout::rotation_default_edge(n));
            g.rotation_edge = Some(cur % n + 1);
            v.set_item(i, Item::NGon(g), "rotation edge", None, vec![], cx);
        });
        let name = g.name.clone();
        let up = self.button(&format!("{key}.rot.parent"), &format!("Polygon {name}"), cx, move |v, _, cx| {
            v.selection = vec![ObjRef::NGon(name.clone())];
            cx.notify();
        });
        let o = obj.clone();
        let remove = self.button(&format!("{key}.rot.remove"), "Remove", cx, move |v, _, cx| {
            v.selection = vec![o.clone()];
            v.edit_obj(&o, "rotation arrow", None, cx, |it| match it {
                Item::NGon(g) => {
                    g.rotation = 0;
                    true
                }
                _ => false,
            });
            v.selection = vec![ObjRef::NGon(g_name(&o))];
        });
        // (class C) the arrow's own name (r, r⁻¹, ρ, …)
        let ol = ObjRef::NGonRotationLabel(g.name.clone());
        let typed = if g.rotation_label.is_none() { String::new() } else { ntex_figures::spelling::as_typed(fig, &ol).unwrap_or_default() };
        let ph = doc::preview(&ntex_figures::layout::ngon_rotation_default(&g));
        let name_field = self.field(&format!("{key}.rot.label"), &typed, true, None, &ph, window, cx, move |v, s, _, cx| {
            v.set_label(&ol, s, Some(format!("field:rotlabel:{ol:?}")), cx);
        });
        vec![
            row(t, "Name", name_field).into_any_element(),
            row(t, "Turns", controls).into_any_element(),
            row(t, "Edge", div().flex().flex_row().items_center().gap(px(8.)).child(prev).child(div().font(t.mono()).child(edge.to_string())).child(next).child(note(t, "or drag it"))).into_any_element(),
            row(t, "", div().flex().flex_row().gap(px(6.)).child(remove).child(up)).into_any_element(),
            note(t, "⌘R runs it the other way; ↺ is r, ↻ is r⁻¹.").into_any_element(),
        ]
    }

    /// `on` for a callback that takes an f32.
    fn on2f<F>(&self, cx: &mut Context<Self>, f: F) -> impl Fn(f32, &mut Window, &mut App) + 'static
    where
        F: Fn(&mut FigureView, f32, &mut Context<FigureView>) + 'static,
    {
        let me = cx.entity().downgrade();
        let f = Rc::new(f);
        move |x, _, cx| {
            let f = f.clone();
            let _ = me.update(cx, |v, cx| f(v, x, cx));
        }
    }

    fn panel_node(&mut self, t: &Theme, obj: &ObjRef, n: Node, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let fig = self.doc.figure.clone();
        let typed = ntex_figures::spelling::as_typed(&fig, obj).unwrap_or_else(|| n.name.clone());
        let o = obj.clone();
        let f = self.field(&format!("node.{}.label", n.name), &typed, true, None, &n.name, window, cx, move |v, s, _, cx| {
            if !s.trim().is_empty() {
                v.set_label(&o, s, Some(format!("field:nlabel:{o:?}")), cx);
            }
        });
        let preview = doc::preview(&n.label);
        let colours = self.colours(t, cx);
        let at = self.pos_rows(t, &format!("node.{}", n.name), obj, window, cx);
        let lp = self.loop_row(t, &n.name, cx);
        vec![
            row(t, "Label", div().w_full().flex().flex_row().items_center().gap(px(8.)).child(f).child(div().flex_none().font(super::canvas::charter(true)).text_size(px(17.)).child(preview))).into_any_element(),
            stack(t, "Colour", colours).into_any_element(),
        ]
        .into_iter()
        .chain(at)
        .chain(std::iter::once(lp))
        .collect()
    }

    fn panel_arrow(&mut self, t: &Theme, obj: &ObjRef, i: usize, a: Arrow, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let fig = self.doc.figure.clone();
        let typed = ntex_figures::spelling::as_typed(&fig, obj).unwrap_or_default();
        let o = obj.clone();
        let f = self.field(&format!("arrow.{i}.label"), &typed, true, None, "no label", window, cx, move |v, s, _, cx| {
            v.set_label(&o, s, Some(format!("field:alabel:{o:?}")), cx);
        });
        let mut heads = div().flex().flex_row().flex_wrap().gap(px(3.));
        for h in HEADS {
            let ah = a.clone();
            let go = self.on(cx, move |v, _, cx| {
                let mut a = ah.clone();
                a.head = h;
                v.set_item(i, Item::Arrow(a), "head", None, vec![], cx);
            });
            heads = heads.child(
                div()
                    .relative()
                    .flex_none()
                    .child(chip(t, SharedString::from(format!("arrow.head.{h:?}")), head_glyph(h), a.head == h, false).font(t.math_font()).min_w(px(28.)).on_click(move |_, w, cx| go(w, cx)))
                    .child(record_bounds(self.ui_bounds.clone(), format!("head.{h:?}"))),
            );
        }
        let lines = ["solid", "dashed", "dotted"];
        let cur = match a.line {
            Line::Solid => 0,
            Line::Dashed => 1,
            Line::Dotted => 2,
        };
        let al = a.clone();
        let go_line = self.on2(cx, move |v, k, cx| {
            let mut a = al.clone();
            a.line = [Line::Solid, Line::Dashed, Line::Dotted][k];
            v.set_item(i, Item::Arrow(a), "line", None, vec![], cx);
        });
        let ab = a.clone();
        let start = self.on(cx, |v, _, _| v.undo_group = None);
        let change = self.on2f(cx, move |v, x, cx| {
            let mut a = ab.clone();
            let mut b = x.round();
            for s in [0.0f32, 30.0, -30.0, 45.0, -45.0] {
                if (b - s).abs() <= 4.0 {
                    b = s;
                }
            }
            a.bend = b;
            v.set_item(i, Item::Arrow(a), "bend", Some(format!("slider:arrow.{i}.bend")), vec![], cx);
        });
        let slider = Slider::new(SharedString::from(format!("arrow.{i}.bend")), a.bend, -90.0, 90.0).on_start(start).on_change(change);
        let bend_s = if a.bend == 0.0 { "none".to_string() } else { format!("{}°", a.bend as i32) };
        let (a1, a2, a3) = (a.clone(), a.clone(), a.clone());
        let swap = self.check(&format!("arrow.{i}.swap"), "Swap side", a.side == LabelSide::Swap, cx, move |v, on, cx| {
            let mut a = a1.clone();
            a.side = if on { LabelSide::Swap } else { LabelSide::Auto };
            v.set_item(i, Item::Arrow(a), "label side", None, vec![], cx);
        });
        let iso = self.check(&format!("arrow.{i}.iso"), "≅", a.iso, cx, move |v, on, cx| {
            let mut a = a2.clone();
            a.iso = on;
            v.set_item(i, Item::Arrow(a), "≅", None, vec![], cx);
        });
        let thick = self.check(&format!("arrow.{i}.thick"), "Thick", a.thick, cx, move |v, on, cx| {
            let mut a = a3.clone();
            a.thick = on;
            v.set_item(i, Item::Arrow(a), "thickness", None, vec![], cx);
        });
        let colours = self.colours(t, cx);
        let can_reverse = !a.is_loop && a.from != a.to;
        let rev = self.glyph(t, &format!("arrow.{i}.reverse"), "⇄", cx, |v, _, cx| {
            v.reverse_selection(cx);
        });
        let ends = row(t, "Ends", div().flex().flex_row().items_center().gap(px(8.)).child(div().font(t.serif_italic()).child(format!("{} → {}", a.from, a.to))).when(can_reverse, |d| d.child(rev)).child(note(t, if can_reverse { "⌘R" } else { "a loop: no reverse" })));
        let lpos = self.label_pos_row(t, i, &a, cx);
        let iso_note = a.iso.then(|| note(t, if a.label.is_some() { "≅ sits across the line from the label; Swap side moves both." } else { "≅ sits on the label's side; Swap side moves it." }).into_any_element());
        vec![
            row(t, "Label", f).into_any_element(),
            ends.into_any_element(),
            row(t, "Head", heads).into_any_element(),
            row(t, "Line", choice(t, "panel.line", &lines, Some(cur), &self.ui_bounds, go_line)).into_any_element(),
            row(t, "Bend", div().w_full().flex().flex_row().items_center().gap(px(8.)).child(div().relative().flex_1().child(slider).child(record_bounds(self.ui_bounds.clone(), format!("slider.arrow.{i}.bend")))).child(div().flex_none().w(px(38.)).font(t.mono()).text_size(t.small_size).child(bend_s))).into_any_element(),
            lpos,
            row(t, "", div().flex().flex_row().flex_wrap().gap_x(px(12.)).child(swap).child(iso).child(thick)).into_any_element(),
            stack(t, "Colour", colours).into_any_element(),
        ]
        .into_iter()
        .chain(iso_note)
        .collect()
    }

    fn panel_blob(&mut self, t: &Theme, obj: &ObjRef, i: usize, b: Blob, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let fig = self.doc.figure.clone();
        let mut out: Vec<AnyElement> = Vec::new();
        let typed = ntex_figures::spelling::as_typed(&fig, obj).unwrap_or_else(|| b.name.clone());
        let o = obj.clone();
        let f = self.field(&format!("blob.{}.label", b.name), &typed, true, None, &b.name, window, cx, move |v, s, _, cx| {
            v.set_label(&o, s, Some(format!("field:blabel:{o:?}")), cx);
        });
        out.push(row(t, "Label", f).into_any_element());
        let places = ["above", "below", "left", "right", "inside"];
        let cur = [LabelPlace::Above, LabelPlace::Below, LabelPlace::Left, LabelPlace::Right, LabelPlace::Inside].iter().position(|p| *p == b.label_place);
        let bp = b.clone();
        let go = self.on2(cx, move |v, k, cx| {
            let mut b = bp.clone();
            b.label_place = [LabelPlace::Above, LabelPlace::Below, LabelPlace::Left, LabelPlace::Right, LabelPlace::Inside][k];
            v.set_item(i, Item::Blob(b), "label place", None, vec![], cx);
        });
        out.push(row(t, "Place", choice(t, "panel.place", &places, cur, &self.ui_bounds, go)).into_any_element());
        let bs = b.clone();
        let shaded = self.check(&format!("blob.{}.shaded", b.name), "Shaded", b.shaded, cx, move |v, on, cx| {
            let mut b = bs.clone();
            b.shaded = on;
            v.set_item(i, Item::Blob(b), "shading", None, vec![], cx);
        });
        out.push(row(t, "", shaded).into_any_element());
        let colours = self.colours(t, cx);
        out.push(stack(t, "Colour", colours).into_any_element());
        let shapes = ["round", "tall", "wide", "bean"];
        let curs = [BlobShape::Round, BlobShape::Tall, BlobShape::Wide, BlobShape::Bean].iter().position(|s| *s == b.shape);
        let bsh = b.clone();
        let go = self.on2(cx, move |v, k, cx| {
            let mut b = bsh.clone();
            b.shape = [BlobShape::Round, BlobShape::Tall, BlobShape::Wide, BlobShape::Bean][k];
            v.set_item(i, Item::Blob(b), "shape", None, vec![], cx);
        });
        out.push(row(t, "Shape", choice(t, "panel.shape", &shapes, curs, &self.ui_bounds, go)).into_any_element());
        let (w, h) = match b.size {
            Some((w, h)) => (fmt_num(w), fmt_num(h)),
            None => (String::new(), String::new()),
        };
        let size_edit = |v: &mut FigureView, which: usize, s: &str, name: &str, cx: &mut Context<FigureView>| {
            let Some(x) = parse_num(s).filter(|x| *x >= 0.3) else { return };
            let o = ObjRef::Blob(name.to_string());
            let scene = v.doc.scene.clone();
            let fig = v.doc.figure.clone();
            let scale = handles::scene_scale(&scene, &fig);
            let cur = handles::frame(&fig, &scene, &o).map(|f| (2.0 * f.half.0 / scale, 2.0 * f.half.1 / scale)).unwrap_or((2.0, 2.0));
            v.edit_obj(&o, "size", Some(format!("field:bsize:{name}")), cx, |it| {
                if let Item::Blob(b) = it {
                    let (w, h) = b.size.unwrap_or((handles::snap_len(cur.0), handles::snap_len(cur.1)));
                    b.size = Some(if which == 0 { (x, h) } else { (w, x) });
                }
                true
            });
        };
        let (n1, n2) = (b.name.clone(), b.name.clone());
        let fw = self.field(&format!("blob.{}.w", b.name), &w, true, Some(40.), "auto", window, cx, move |v, s, _, cx| size_edit(v, 0, s, &n1, cx));
        let fh = self.field(&format!("blob.{}.h", b.name), &h, true, Some(40.), "auto", window, cx, move |v, s, _, cx| size_edit(v, 1, s, &n2, cx));
        out.push(row(t, "Size", div().flex().flex_row().items_center().gap(px(4.)).child(fw).child(unit(t, "×")).child(fh).child(unit(t, "cm"))).into_any_element());
        out.extend(self.angle_rows(t, &format!("blob.{}", b.name), obj, b.angle, window, cx));
        // elements: rename, remove, add
        let mut list = div().flex().flex_col().gap(px(2.));
        for e in &b.elements {
            self.tab_order.push(format!("blob.{}.elem.{}", b.name, e.name));
        }
        for (k, e) in b.elements.iter().enumerate() {
            let typed = ntex_figures::spelling::raw(&fig, &format!("elem:{}", e.name), &e.label);
            let typed = if e.label == ntex_figures::parse::default_label(&e.name) { e.name.clone() } else { typed };
            let eo = ObjRef::Element(e.name.clone());
            let fe = self.field(&format!("blob.{}.elem.{}", b.name, e.name), &typed, true, None, &e.name, window, cx, move |v, s, _, cx| {
                if !s.trim().is_empty() {
                    v.set_label(&eo, s, Some(format!("field:elabel:{eo:?}")), cx);
                }
            });
            let bn = b.name.clone();
            let rm = self.button(&format!("blob.{}.rm.{k}", b.name), "×", cx, move |v, _, cx| {
                let o = ObjRef::Blob(bn.clone());
                v.edit_obj(&o, "elements", None, cx, |it| {
                    if let Item::Blob(b) = it
                        && k < b.elements.len()
                    {
                        b.elements.remove(k);
                        return true;
                    }
                    false
                });
            });
            list = list.child(div().flex().flex_row().items_center().gap(px(6.)).child(fe).child(rm));
        }
        let bn = b.name.clone();
        let add = self.button(&format!("blob.{}.add", b.name), "+ element", cx, move |v, window, cx| {
            let name = fresh_element(&v.doc.figure);
            let o = ObjRef::Blob(bn.clone());
            let n2 = name.clone();
            if v.edit_obj(&o, "elements", None, cx, |it| {
                if let Item::Blob(b) = it {
                    b.elements.push(Element { name: n2.clone(), label: n2.clone(), pos: None });
                    return true;
                }
                false
            }) {
                // her label for it, straight away (typing replaces the placeholder,
                // which then takes that name too); Tab goes on to the next element
                v.fresh_elements.push(name.clone());
                let e = ObjRef::Element(name);
                v.selection = vec![e.clone()];
                v.start_label_edit(e, window, cx);
            }
        });
        list = list.child(div().pt(px(2.)).child(add));
        out.push(stack(t, "Elements", list).into_any_element());
        out.extend(self.parent_row(t, i, &b, cx));
        out.extend(self.pos_rows(t, &format!("blob.{}", b.name), obj, window, cx));
        out
    }

    fn panel_cayley(&mut self, t: &Theme, obj: &ObjRef, i: usize, c: Cayley, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let fig = self.doc.figure.clone();
        let mut out: Vec<AnyElement> = Vec::new();
        let gtext = match &c.group {
            GroupSpec::Named(n) => ntex_figures::spelling::raw(&fig, &format!("group:{}", c.name), n),
            g => group_text(g),
        };
        let o = obj.clone();
        let name = c.name.clone();
        let fg = self.field(&format!("cayley.{}.group", c.name), &gtext, true, None, "D4", window, cx, move |v, s, _, cx| {
            let s = s.trim();
            if s.is_empty() {
                return;
            }
            let spec = match parse_presentation(s) {
                Some((g, r)) => GroupSpec::Presentation(g, r),
                None if s.starts_with('<') => return,
                None => GroupSpec::Named(doc::to_latex(s)),
            };
            let mut spell = Vec::new();
            if let GroupSpec::Named(n) = &spec {
                spell.push((format!("group:{name}"), n.clone(), s.to_string()));
            }
            let Some(i) = handles::item_index(&v.doc.figure, &o) else { return };
            let Item::Cayley(mut c) = v.doc.figure.items[i].clone() else { return };
            if let GroupSpec::Presentation(g, _) = &spec {
                c.gens = g.clone();
            }
            c.group = spec;
            if v.set_item(i, Item::Cayley(c), "group", Some(format!("field:cgroup:{name}")), spell, cx) {
                // (a graph is named after its group: keep it selected under its new name)
                v.reselect_item(i);
            }
        });
        out.push(row(t, "Group", fg).into_any_element());
        let o = obj.clone();
        let name = c.name.clone();
        let fgen = self.field(&format!("cayley.{}.gens", c.name), &c.gens.join(", "), true, None, "standard", window, cx, move |v, s, _, cx| {
            let gens: Vec<String> = s.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
            let Some(i) = handles::item_index(&v.doc.figure, &o) else { return };
            let Item::Cayley(mut c) = v.doc.figure.items[i].clone() else { return };
            c.gens = gens;
            if v.set_item(i, Item::Cayley(c), "generators", Some(format!("field:cgens:{name}")), vec![], cx) {
                v.reselect_item(i);
            }
        });
        out.push(row(t, "Gens", fgen).into_any_element());
        out.extend(self.gen_rows(t, i, &c, cx));
        let layouts = ["auto", "circle", "prism", "grid"];
        let cur = [CayleyLayout::Auto, CayleyLayout::Circle, CayleyLayout::Prism, CayleyLayout::Grid].iter().position(|l| *l == c.layout);
        let cl = c.clone();
        let go = self.on2(cx, move |v, k, cx| {
            let mut c = cl.clone();
            c.layout = [CayleyLayout::Auto, CayleyLayout::Circle, CayleyLayout::Prism, CayleyLayout::Grid][k];
            v.set_item(i, Item::Cayley(c), "layout", None, vec![], cx);
        });
        out.push(row(t, "Layout", choice(t, "panel.layout", &layouts, cur, &self.ui_bounds, go)).into_any_element());
        let (c1, c2) = (c.clone(), c.clone());
        let labels = self.check(&format!("cayley.{}.labels", c.name), "Labels", c.labels, cx, move |v, on, cx| {
            let mut c = c1.clone();
            c.labels = on;
            v.set_item(i, Item::Cayley(c), "labels", None, vec![], cx);
        });
        let legend = self.check(&format!("cayley.{}.legend", c.name), "Legend", c.legend, cx, move |v, on, cx| {
            let mut c = c2.clone();
            c.legend = on;
            v.set_item(i, Item::Cayley(c), "legend", None, vec![], cx);
        });
        out.push(row(t, "", div().flex().flex_row().gap_x(px(12.)).child(labels).child(legend)).into_any_element());
        let r = c.radius.map(fmt_num).unwrap_or_default();
        let o = obj.clone();
        let name = c.name.clone();
        let fr = self.field(&format!("cayley.{}.radius", c.name), &r, true, Some(44.), "auto", window, cx, move |v, s, _, cx| {
            let x = if s.trim().is_empty() { None } else { parse_num(s).filter(|x| *x >= 0.3) };
            if x.is_none() && !s.trim().is_empty() {
                return;
            }
            v.edit_obj(&o, "size", Some(format!("field:cradius:{name}")), cx, |it| {
                if let Item::Cayley(c) = it {
                    c.radius = x;
                }
                true
            });
        });
        out.push(row(t, "Size", div().flex().flex_row().items_center().gap(px(6.)).child(fr).child(unit(t, "cm")).child(note(t, "empty: auto"))).into_any_element());
        if !handles::cayley_resizable(&c) {
            out.push(note(t, format!("A set size draws {} as a plain circle; leave it empty to keep this layout.", group_text(&c.group))).into_any_element());
        }
        out.extend(self.angle_rows(t, &format!("cayley.{}", c.name), obj, c.angle, window, cx));
        out.extend(self.pos_rows(t, &format!("cayley.{}", c.name), obj, window, cx));
        out
    }
}

/// `c` with generator `gen`'s look changed by `f` (an entry at the defaults is dropped).
fn with_gen_style(c: &Cayley, generator: &str, f: impl FnOnce(&mut GenStyle)) -> Cayley {
    let mut c = c.clone();
    let mut st = c.gen_styles.iter().find(|s| s.generator == generator).cloned().unwrap_or(GenStyle { generator: generator.to_string(), ..Default::default() });
    f(&mut st);
    c.gen_styles.retain(|s| s.generator != generator);
    if st.color.is_some() || st.inverse || st.hidden {
        c.gen_styles.push(st);
    }
    c
}

fn g_name(o: &ObjRef) -> String {
    match o {
        ObjRef::NGonRotation(n) | ObjRef::NGon(n) => n.clone(),
        _ => String::new(),
    }
}

/// A polygon's turn as written: −k when it was given as `turn=-k`.
fn signed_turn(g: &NGon) -> i32 {
    let n = g.n.max(1) as i32;
    if g.turn_neg && g.turn > 0 { g.turn as i32 % n - n } else { g.turn as i32 }
}

/// The turn one step (+1 counter-clockwise ⟲, −1 ⟳) on, keeping its sign as shown.
fn step_turn(g: &NGon, by: i32) -> NGon {
    let n = g.n.max(1) as i32;
    // (review C7) a turn written ≥ n (`turn=7`) or ≤ -n counts mod n first
    let cur = signed_turn(g);
    let cur = cur % n;
    let mut s = cur + by;
    if s >= n || s <= -n {
        s = 0;
    }
    let mut g = g.clone();
    if s < 0 {
        g.turn = (s + n) as u32;
        g.turn_neg = true;
    } else {
        g.turn = s as u32;
        g.turn_neg = false;
    }
    g
}

#[allow(dead_code)]
fn superscript(k: u32) -> String {
    k.to_string().chars().map(|c| match c {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        c => c,
    }).collect()
}

impl FigureView {
    /// Hovering an axis chip: the canvas shows the polygon with that axis.
    pub fn preview_axis(&mut self, index: usize, g: &NGon, code: u32, on: bool) {
        if self.gesture.is_some() {
            return;
        }
        if !on {
            self.drag_scene = None;
            return;
        }
        let mut g2 = g.clone();
        if !g2.all_axes && !g2.axes.contains(&code) {
            g2.axes.push(code);
        }
        let mut fig = self.doc.figure.clone();
        if index < fig.items.len() {
            fig.items[index] = Item::NGon(g2);
            self.drag_scene = Some(std::sync::Arc::new(doc::scene_of(&fig)));
        }
    }
}
