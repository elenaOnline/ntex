//! \fbox buttons, radio groups (○ ●), checkboxes (□ ⊠), \keys keycaps,
//! colour swatches, a hue slider, a collapsible \paragraph.
//!
//! Every control is focusable (its focus handle is kept per element id),
//! shows a 1.5 px blue focus ring 2 px outside when focused from the
//! keyboard, and is at least 24 px tall to click.
use gpui::{
    App, Bounds, ClickEvent, ElementId, FocusHandle, Hsla, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, Pixels, RenderOnce, SharedString, Stateful, Window, canvas, div, prelude::*, px,
};
use std::cell::Cell;
use std::rc::Rc;

use super::boxes::{aside, pmb};
use super::theme::{ActiveTheme as _, Theme};

type Handler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// Forces a look for previews and the gallery (hover / press can't be held
/// still otherwise).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visual {
    Hover,
    Press,
    Focus,
}

/// The focus handle a stateless control keeps under its element id.
pub fn control_focus(id: &ElementId, window: &mut Window, cx: &mut App) -> FocusHandle {
    window.use_keyed_state(id.clone(), cx, |_, cx| cx.focus_handle()).read(cx).clone()
}

/// The focus ring: 1.5 px blue, 2 px outside the control.
fn focus_ring(t: &Theme) -> gpui::Div {
    div().absolute().top(px(-3.5)).bottom(px(-3.5)).left(px(-3.5)).right(px(-3.5)).border(px(1.5)).border_color(t.blue)
}

fn ring_visible(focus: &FocusHandle, window: &Window) -> bool {
    focus.is_focused(window) && window.last_input_was_keyboard()
}

/// 6 % darker (the pressed fill).
fn darken(c: Hsla, by: f32) -> Hsla {
    Hsla { l: (c.l - by).max(0.0), ..c }
}

// ---------------------------------------------------------------------------
// \fbox / \doublebox buttons

/// \fbox{Label} with an optional keycap: [ Save to inbox  ⌘↵ ]. Primary =
/// double-ruled box (fancybox \doublebox). Focusable (Tab order), Space/Return
/// presses it.
#[derive(IntoElement)]
pub struct Button {
    pub id: ElementId,
    pub label: SharedString,
    pub keys: Option<SharedString>,
    pub primary: bool,
    pub disabled: bool,
    pub on_click: Option<Handler>,
    // ---- added by WIDGETS
    /// Force a look (gallery / previews).
    pub visual: Option<Visual>,
    /// Use this focus handle instead of the button's own (so a screen can
    /// focus it, e.g. after a save error).
    pub focus: Option<FocusHandle>,
    /// Smaller, for fix buttons inside a log line.
    pub small: bool,
}

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
        Button {
            id: id.into(),
            label: label.into(),
            keys: None,
            primary: false,
            disabled: false,
            on_click: None,
            visual: None,
            focus: None,
            small: false,
        }
    }
    pub fn keys(mut self, keys: impl Into<SharedString>) -> Button {
        self.keys = Some(keys.into());
        self
    }
    pub fn primary(mut self) -> Button {
        self.primary = true;
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Button {
        self.disabled = disabled;
        self
    }
    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Button {
        self.on_click = Some(Rc::new(f));
        self
    }
    pub fn visual(mut self, v: Visual) -> Button {
        self.visual = Some(v);
        self
    }
    pub fn focus_handle(mut self, focus: FocusHandle) -> Button {
        self.focus = Some(focus);
        self
    }
    pub fn small(mut self) -> Button {
        self.small = true;
        self
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme().clone();
        let focus = self.focus.clone().unwrap_or_else(|| control_focus(&self.id, window, cx));
        let ring = self.visual == Some(Visual::Focus) || (!self.disabled && ring_visible(&focus, window));
        let ink = if self.disabled { Theme::alpha(t.ink, 0.4) } else { t.ink };
        let hover_fill = t.theorem.back;
        let press_fill = darken(hover_fill, 0.06);
        let size = if self.small { t.small_size } else { t.body_size };
        let forced_fill = match self.visual {
            Some(Visual::Hover) if !self.disabled => Some(hover_fill),
            Some(Visual::Press) if !self.disabled => Some(press_fill),
            _ => None,
        };
        let enabled = !self.disabled;
        // the \fbox itself
        let inner = div()
            .id("fbox")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .px(px(if self.small { 6. } else { 9. }))
            .py(px(if self.small { 1. } else { 3. }))
            .border_1()
            .border_color(ink)
            .when_some(forced_fill, |d, f| d.bg(f))
            .when(enabled && forced_fill.is_none(), |d| {
                d.group_hover("ntex-button", move |s| s.bg(hover_fill))
                    .group_active("ntex-button", move |s| s.bg(press_fill))
            })
            .child(div().font(t.caps()).text_size(size).text_color(ink).child(self.label.clone()))
            .when_some(self.keys.clone(), |d, k| d.child(Keycap::new(k).disabled(self.disabled)));
        let body = if self.primary {
            // \doublebox: 1 px + 2 px gap + 1 px
            div().p(px(2.)).border_1().border_color(ink).child(inner)
        } else {
            div().child(inner)
        };
        let mut root = div()
            .id(self.id.clone())
            .group("ntex-button")
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .min_h(px(24.))
            .line_height(t.line(size))
            .track_focus(&focus.clone().tab_stop(enabled))
            .when(enabled, |d| d.cursor_pointer())
            .when(!enabled, |d| d.on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default()))
            .child(body)
            .when(ring, |d| d.child(focus_ring(&t)));
        if enabled && let Some(handler) = self.on_click.clone() {
            root = root.on_click(move |e, window, cx| handler(e, window, cx));
        }
        root
    }
}

// ---------------------------------------------------------------------------
// radio: an inline enumerate

/// One line of choices: "Fires  ● by itself  ○ on Tab   (guessed)".
/// `guessed` shows the italic slate "(guessed)" after the choices.
/// Focusable as one control: ←/→ choose (and call `on_select`).
#[derive(IntoElement)]
pub struct Radio {
    pub id: ElementId,
    pub label: SharedString,
    pub options: Vec<SharedString>,
    pub selected: usize,
    pub guessed: bool,
    pub on_select: Option<Rc<dyn Fn(usize, &mut Window, &mut App)>>,
    // ---- added by WIDGETS
    pub visual: Option<Visual>,
    pub focus: Option<FocusHandle>,
    /// Options that can't be chosen right now (drawn at 40 %).
    pub disabled: Vec<usize>,
    /// Width of the label column, so stacked radios line up ("Kind", "Maths", …).
    pub label_width: Option<Pixels>,
}

impl Radio {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, options: Vec<SharedString>, selected: usize) -> Radio {
        Radio {
            id: id.into(),
            label: label.into(),
            options,
            selected,
            guessed: false,
            on_select: None,
            visual: None,
            focus: None,
            disabled: Vec::new(),
            label_width: None,
        }
    }
    pub fn guessed(mut self, guessed: bool) -> Radio {
        self.guessed = guessed;
        self
    }
    pub fn on_select(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Radio {
        self.on_select = Some(Rc::new(f));
        self
    }
    pub fn visual(mut self, v: Visual) -> Radio {
        self.visual = Some(v);
        self
    }
    pub fn focus_handle(mut self, focus: FocusHandle) -> Radio {
        self.focus = Some(focus);
        self
    }
    pub fn disabled_options(mut self, disabled: Vec<usize>) -> Radio {
        self.disabled = disabled;
        self
    }
    pub fn label_width(mut self, w: Pixels) -> Radio {
        self.label_width = Some(w);
        self
    }
}

/// The next enabled option from `from` in direction `dir` (no wrap).
fn step_option(from: usize, dir: isize, n: usize, disabled: &[usize]) -> Option<usize> {
    let mut i = from as isize;
    loop {
        i += dir;
        if i < 0 || i >= n as isize {
            return None;
        }
        if !disabled.contains(&(i as usize)) {
            return Some(i as usize);
        }
    }
}

impl RenderOnce for Radio {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme().clone();
        let focus = self.focus.clone().unwrap_or_else(|| control_focus(&self.id, window, cx));
        let ring = self.visual == Some(Visual::Focus) || ring_visible(&focus, window);
        let n = self.options.len();
        let selected = self.selected;
        let x_mid = x_height_middle(&t, t.body_size, window);
        let mut choices = div().relative().flex().flex_row().flex_wrap().items_center().gap_x(px(12.));
        for (i, opt) in self.options.iter().enumerate() {
            let on = i == selected;
            let off = self.disabled.contains(&i);
            let colour = if off { Theme::alpha(t.ink, 0.4) } else { t.ink };
            let mark = radio_mark(&t, on, x_mid).when(off, |d| d.opacity(0.4));
            let mut item = div()
                .id(ElementId::NamedInteger("radio-option".into(), i as u64))
                .flex()
                .flex_row()
                .items_center()
                .min_h(px(24.))
                .text_color(colour)
                .child(mark)
                .child(opt.clone());
            if !off {
                item = item.cursor_pointer();
                if let Some(f) = self.on_select.clone() {
                    item = item.on_click(move |_, window, cx| f(i, window, cx));
                }
            }
            choices = choices.child(item);
        }
        if self.guessed {
            choices = choices.child(aside(&t, "(guessed)").flex_none());
        }
        if ring {
            choices = choices.child(focus_ring(&t));
        }
        let disabled = self.disabled.clone();
        let on_select = self.on_select.clone();
        div()
            .id(self.id)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .font(t.serif())
            .text_size(t.body_size)
            .line_height(t.line(t.body_size))
            .text_color(t.ink)
            .track_focus(&focus.tab_stop(true))
            .on_key_down(move |e: &KeyDownEvent, window, cx| {
                let m = e.keystroke.modifiers;
                if m.platform || m.control || m.alt || m.shift {
                    return;
                }
                let dir = match e.keystroke.key.as_str() {
                    "left" | "up" => -1,
                    "right" | "down" => 1,
                    _ => return,
                };
                if let Some(next) = step_option(selected, dir, n, &disabled) {
                    if let Some(f) = on_select.as_ref() {
                        f(next, window, cx);
                    }
                }
                cx.stop_propagation();
            })
            .when(!self.label.is_empty(), |d| {
                d.child(
                    div()
                        .font(t.caps())
                        .when_some(self.label_width, |d, w| d.w(w).flex_none())
                        .child(self.label.clone()),
                )
            })
            .child(choices)
    }
}

/// Where the middle of the x-height sits in a line box of the body text:
/// GPUI centres the font's ascent + descent in the line box, so this follows
/// the face (LM Roman and Pagella differ by about 2 px).
pub fn x_height_middle(t: &Theme, size: Pixels, window: &Window) -> Pixels {
    let ts = window.text_system();
    let id = ts.resolve_font(&t.serif());
    let (a, d, xh) = (ts.ascent(id, size), ts.descent(id, size), ts.x_height(id, size));
    // a face without an OS/2 x-height reports 0: LM and Pagella are ≈0.45 em
    let xh = if xh > size * 0.3 && xh < size * 0.7 { xh } else { size * 0.45 };
    // font metrics give the descent as a negative number; the line layout
    // centres ascent + |descent|
    let d = if d < px(0.) { -d } else { d };
    let line = t.line(size);
    (line - a - d) / 2. + a - xh / 2.
}

/// The radio's ● / ○, drawn (not a glyph) so it sits on the x-height middle
/// in either typeface: a disc 0.5 em across, ink when on, a slate ring off.
fn radio_mark(t: &Theme, on: bool, x_mid: Pixels) -> gpui::Div {
    let d = (t.body_size * 0.5).round();
    let line = t.line(t.body_size);
    let dot = div().absolute().left_0().top(x_mid - d / 2.).size(d).rounded_full();
    let dot = if on { dot.bg(t.ink) } else { dot.border(px(1.1)).border_color(t.slate) };
    div().relative().flex_none().w(d).h(line).mr(px(6.)).child(dot)
}

// ---------------------------------------------------------------------------
// checkbox: $\square$ / $\boxtimes$

/// □ / ⊠ Label
#[derive(IntoElement)]
pub struct Checkbox {
    pub id: ElementId,
    pub label: SharedString,
    pub checked: bool,
    pub on_toggle: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
    // ---- added by WIDGETS
    pub visual: Option<Visual>,
    pub disabled: bool,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, checked: bool) -> Checkbox {
        Checkbox { id: id.into(), label: label.into(), checked, on_toggle: None, visual: None, disabled: false }
    }
    pub fn on_toggle(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Checkbox {
        self.on_toggle = Some(Rc::new(f));
        self
    }
    pub fn visual(mut self, v: Visual) -> Checkbox {
        self.visual = Some(v);
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Checkbox {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme().clone();
        let focus = control_focus(&self.id, window, cx);
        let ring = self.visual == Some(Visual::Focus) || ring_visible(&focus, window);
        let checked = self.checked;
        let colour = if self.disabled { Theme::alpha(t.ink, 0.4) } else { t.ink };
        let mut root = div()
            .id(self.id)
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .min_h(px(24.))
            .gap(px(5.))
            .font(t.serif())
            .text_size(t.body_size)
            .line_height(t.line(t.body_size))
            .text_color(colour)
            .track_focus(&focus.tab_stop(!self.disabled))
            .child(div().font(t.math_font()).child(if checked { "⊠" } else { "□" }))
            .child(self.label)
            .when(ring, |d| d.child(focus_ring(&t)));
        if !self.disabled {
            root = root.cursor_pointer();
            if let Some(f) = self.on_toggle {
                root = root.on_click(move |_, window, cx| f(!checked, window, cx));
            }
        }
        root
    }
}

// ---------------------------------------------------------------------------
// \keys keycaps

/// A \keys{⌘ ↵} keycap: LM Mono 12 px, 1 px slate border, radius 2, 0×4.
#[derive(IntoElement)]
pub struct Keycap {
    pub keys: SharedString,
    pub disabled: bool,
    /// Larger, for the trigger field's ⇥.
    pub size: Option<Pixels>,
}

impl Keycap {
    pub fn new(keys: impl Into<SharedString>) -> Keycap {
        Keycap { keys: keys.into(), disabled: false, size: None }
    }
    pub fn disabled(mut self, d: bool) -> Keycap {
        self.disabled = d;
        self
    }
    pub fn size(mut self, s: Pixels) -> Keycap {
        self.size = Some(s);
        self
    }
}

impl RenderOnce for Keycap {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme().clone();
        let size = self.size.unwrap_or(t.keycap_size);
        let colour = if self.disabled { Theme::alpha(t.ink, 0.4) } else { t.ink };
        let border = if self.disabled { Theme::alpha(t.slate, 0.4) } else { t.slate };
        div()
            .flex_none()
            .px(size / 3.)
            .border_1()
            .border_color(border)
            .rounded(px(2.))
            .font(t.mono())
            .text_size(size)
            .line_height(size * 1.2)
            .text_color(colour)
            .child(super::boxes::prose(&t, &self.keys, t.mono(), colour))
    }
}

/// A \keys{⌘ ↵} keycap.
pub fn keycap(keys: impl Into<SharedString>) -> impl IntoElement {
    Keycap::new(keys)
}

// ---------------------------------------------------------------------------
// swatches

/// A row of colour swatches (name + three tones); the selected one framed.
#[derive(IntoElement)]
pub struct Swatches {
    pub id: ElementId,
    pub items: Vec<(SharedString, crate::model::Tones)>,
    pub selected: Option<usize>,
    pub on_select: Option<Rc<dyn Fn(usize, &mut Window, &mut App)>>,
}

impl Swatches {
    pub fn new(id: impl Into<ElementId>, items: Vec<(SharedString, crate::model::Tones)>) -> Swatches {
        Swatches { id: id.into(), items, selected: None, on_select: None }
    }
    pub fn selected(mut self, s: Option<usize>) -> Swatches {
        self.selected = s;
        self
    }
    pub fn on_select(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Swatches {
        self.on_select = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Swatches {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme().clone();
        let focus = control_focus(&self.id, window, cx);
        let ring = ring_visible(&focus, window);
        let n = self.items.len();
        let selected = self.selected;
        let on_select = self.on_select.clone();
        let mut row = div().relative().flex().flex_row().flex_wrap().gap(px(10.));
        for (i, (name, tones)) in self.items.into_iter().enumerate() {
            let on = Some(i) == selected;
            let chip = div()
                .flex()
                .flex_col()
                .w(px(34.))
                .child(div().h(px(6.)).bg(Theme::rgb_of(tones.rule)))
                .child(div().h(px(12.)).bg(Theme::rgb_of(tones.back)))
                .child(div().h(px(6.)).bg(Theme::rgb_of(tones.head)));
            let mut item = div()
                .id(ElementId::NamedInteger("swatch".into(), i as u64))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(2.))
                .p(px(2.))
                .border_1()
                .border_color(if on { t.ink } else { Theme::alpha(t.ink, 0.0) })
                .cursor_pointer()
                .child(chip)
                .child(
                    div()
                        .font(t.caps())
                        .text_size(t.footnote_size)
                        .line_height(t.line(t.footnote_size))
                        .text_color(if on { t.ink } else { t.slate })
                        .child(name),
                );
            if let Some(f) = self.on_select.clone() {
                item = item.on_click(move |_, window, cx| f(i, window, cx));
            }
            row = row.child(item);
        }
        if ring {
            row = row.child(focus_ring(&t));
        }
        div()
            .id(self.id)
            .track_focus(&focus.tab_stop(true))
            .on_key_down(move |e: &KeyDownEvent, window, cx| {
                let dir = match e.keystroke.key.as_str() {
                    "left" => -1isize,
                    "right" => 1,
                    _ => return,
                };
                let from = selected.unwrap_or(0) as isize;
                let next = (from + dir).clamp(0, n.saturating_sub(1) as isize) as usize;
                if let Some(f) = on_select.as_ref() {
                    f(next, window, cx);
                }
                cx.stop_propagation();
            })
            .child(row)
    }
}

// ---------------------------------------------------------------------------
// hue slider

/// The palette's rule tone at a hue (`core::colour::tones_for_hue`), for
/// painting the track.
pub fn hue_rule(hue: f32) -> Hsla {
    Theme::rgb_of(crate::core::colour::tones_for_hue(hue.rem_euclid(360.)).rule)
}

#[derive(Default)]
struct SliderState {
    bounds: Cell<Option<Bounds<Pixels>>>,
    dragging: Cell<bool>,
}

/// A hue slider 0–360°, the track painted with the hue's Rule tones.
/// Drag or click the track; ←/→ step 5° (⇧ 15°) when focused.
#[derive(IntoElement)]
pub struct HueSlider {
    pub id: ElementId,
    pub hue: f32,
    pub on_change: Option<Rc<dyn Fn(f32, &mut Window, &mut App)>>,
}

impl HueSlider {
    pub fn new(id: impl Into<ElementId>, hue: f32) -> HueSlider {
        HueSlider { id: id.into(), hue, on_change: None }
    }
    pub fn on_change(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> HueSlider {
        self.on_change = Some(Rc::new(f));
        self
    }
    /// Track x → hue.
    pub fn hue_at(bounds: Bounds<Pixels>, x: Pixels) -> f32 {
        let w = f32::from(bounds.size.width).max(1.0);
        ((f32::from(x - bounds.origin.x) / w).clamp(0.0, 1.0) * 360.0).round()
    }
}

impl RenderOnce for HueSlider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme().clone();
        let focus = control_focus(&self.id, window, cx);
        let ring = ring_visible(&focus, window);
        let state_id = ElementId::NamedChild(std::sync::Arc::new(self.id.clone()), "state".into());
        let state = window.use_keyed_state(state_id, cx, |_, _| Rc::new(SliderState::default())).read(cx).clone();
        let hue = self.hue.rem_euclid(360.);
        const TRACK_W: f32 = 240.;
        let segments = 48;
        let mut track = div().relative().flex().flex_row().w(px(TRACK_W)).h(px(4.));
        for i in 0..segments {
            let h = (i as f32 + 0.5) / segments as f32 * 360.;
            track = track.child(div().flex_1().h_full().bg(hue_rule(h)));
        }
        let st = state.clone();
        track = track.child(
            canvas(move |b, _, _| st.bounds.set(Some(b)), |_, _, _, _| {}).absolute().top_0().left_0().size_full(),
        );
        // the knob: ● in ink, centred on the hue
        let knob_x = px(hue / 360. * TRACK_W - 6.);
        let knob = div()
            .absolute()
            .top(px(-5.))
            .left(knob_x)
            .size(px(12.))
            .rounded_full()
            .bg(t.paper)
            .border(px(1.5))
            .border_color(t.ink);
        let track_box = div().relative().py(px(10.)).child(track.child(knob));

        let change = self.on_change.clone();
        let (s1, s2) = (state.clone(), state.clone());
        let (c1, c2, c3) = (change.clone(), change.clone(), change);
        div()
            .id(self.id)
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .min_h(px(24.))
            .track_focus(&focus.tab_stop(true))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, window, cx| {
                if let Some(b) = s1.bounds.get() {
                    s1.dragging.set(true);
                    if let Some(f) = c1.as_ref() {
                        f(HueSlider::hue_at(b, e.position.x), window, cx);
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
                    f(HueSlider::hue_at(b, e.position.x), window, cx);
                }
            })
            .on_mouse_up(MouseButton::Left, {
                let s = state.clone();
                move |_, _, _| s.dragging.set(false)
            })
            .on_mouse_up_out(MouseButton::Left, move |_, _, _| state.dragging.set(false))
            .on_key_down(move |e: &KeyDownEvent, window, cx| {
                let step = if e.keystroke.modifiers.shift { 15. } else { 5. };
                let next = match e.keystroke.key.as_str() {
                    "left" => hue - step,
                    "right" => hue + step,
                    _ => return,
                };
                if let Some(f) = c3.as_ref() {
                    f(next.rem_euclid(360.), window, cx);
                }
                cx.stop_propagation();
            })
            .child(div().font(t.caps()).text_size(t.body_size).text_color(t.ink).child("Hue"))
            .child(track_box)
            .child(
                div()
                    .font(t.serif())
                    .text_size(t.body_size)
                    .text_color(t.ink)
                    .child(format!("{}°", hue.round() as i32)),
            )
            .when(ring, |d| d.child(focus_ring(&t)))
    }
}

// ---------------------------------------------------------------------------
// \paragraph disclosure

/// \paragraph{More options.} — a run-in bold head with ▸/▾ that opens a body.
/// Renders the head only; the screen renders the body when `open`.
#[derive(IntoElement)]
pub struct Disclosure {
    pub id: ElementId,
    pub title: SharedString,
    pub open: bool,
    pub on_toggle: Option<Rc<dyn Fn(bool, &mut Window, &mut App)>>,
}

impl Disclosure {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>, open: bool) -> Disclosure {
        Disclosure { id: id.into(), title: title.into(), open, on_toggle: None }
    }
    pub fn on_toggle(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Disclosure {
        self.on_toggle = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Disclosure {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme().clone();
        let focus = control_focus(&self.id, window, cx);
        let ring = ring_visible(&focus, window);
        let open = self.open;
        let mut root: Stateful<gpui::Div> = div()
            .id(self.id)
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .min_h(px(24.))
            .cursor_pointer()
            .font(t.serif_bold())
            .text_size(t.body_size)
            .line_height(t.line(t.body_size))
            .text_color(t.ink)
            .track_focus(&focus.tab_stop(true))
            .child(self.title)
            .child(div().font(t.math_font()).text_color(t.blue).child(if open { "▾" } else { "▸" }))
            .when(ring, |d| d.child(focus_ring(&t)));
        if let Some(f) = self.on_toggle {
            root = root.on_click(move |_, window, cx| f(!open, window, cx));
        }
        root
    }
}

/// Faux-bold re-export for screens (\pmb).
pub fn bold_caps(text: impl Into<SharedString>) -> gpui::Div {
    pmb(text)
}
