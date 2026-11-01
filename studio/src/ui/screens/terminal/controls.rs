//! Everything around the preview: where changes go, saved looks, the palette
//! strip, the inspector for the selected option, the theme gallery and the
//! drawer with every look option.
use super::preview::hsla;
use super::{DragKind, GHOSTTY_DEFAULT, GalleryShow};
use crate::core::ghostty::catalog::{self, Kind};
use crate::core::ghostty::screen::Target;
use crate::core::ghostty::{self as g, AppliesTo, Colors, Defaults, TermLook};
use crate::core::look::{from_hsv, to_hsv};
use crate::ui::app::Studio;
use crate::ui::screens::shell::{self, GAP};
use crate::ui::widgets::boxes::{Section, aside};
use crate::ui::widgets::controls::{Button, ChannelSlider, Checkbox, Disclosure};
use crate::ui::widgets::theme::{ActiveTheme as _, Theme};
use gpui::{
    AnyElement, AnyView, App, AppContext as _, Context, InteractiveElement as _, IntoElement, MouseButton, MouseMoveEvent, ParentElement as _,
    Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

/// A docs tooltip (Ghostty's own words for the option).
struct DocTip(SharedString);

impl Render for DocTip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme().clone();
        div()
            .max_w(px(460.))
            .p(px(10.))
            .bg(t.paper)
            .border_1()
            .border_color(t.rule)
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(t.ink)
            .font(t.serif())
            .child(self.0.clone())
    }
}

fn doc_tip(text: String) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let lines: Vec<&str> = text.lines().collect();
    let mut short = lines.iter().take(14).cloned().collect::<Vec<_>>().join("\n");
    if lines.len() > 14 {
        short.push_str("\n…");
    }
    let s = SharedString::from(short);
    move |_, cx| cx.new(|_| DocTip(s.clone())).into()
}

/// A small chip-style choice (selected: blue double rule).
fn chip(t: &Theme, id: impl Into<SharedString>, label: impl Into<SharedString>, on: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(gpui::ElementId::Name(id.into()))
        .px(px(7.))
        .py(px(1.))
        .min_h(px(22.))
        .flex()
        .items_center()
        .border(px(if on { 1.5 } else { 1. }))
        .border_color(if on { t.blue } else { Theme::alpha(t.rule, 0.35) })
        .text_color(if on { t.blue } else { t.ink })
        .text_size(px(13.))
        .cursor_pointer()
        .child(label.into())
}

pub fn option_value_label(look: &TermLook, d: &Defaults, key: &str) -> String {
    let v = look.get(key, d);
    if v.is_empty() || v.iter().all(|x| x.is_empty()) { "unset".into() } else { v.join(", ") }
}

/// Which options sit next to the selected one in the inspector.
fn related(key: &str) -> &'static [&'static str] {
    match key {
        "background" => &["background-opacity", "background-blur", "background-opacity-cells", "window-padding-color", "minimum-contrast"],
        "foreground" | "bold-color" => &["bold-color", "faint-opacity", "minimum-contrast"],
        "cursor-color" | "cursor-text" | "cursor-style" => &["cursor-style", "cursor-color", "cursor-text", "cursor-style-blink", "cursor-opacity", "adjust-cursor-thickness", "adjust-cursor-height"],
        "selection-background" | "selection-foreground" => &["selection-background", "selection-foreground"],
        "macos-titlebar-style" => &["macos-titlebar-style", "window-theme", "macos-window-buttons", "macos-titlebar-proxy-icon", "window-title-font-family", "macos-window-shadow"],
        "unfocused-split-opacity" | "unfocused-split-fill" | "split-divider-color" => &["unfocused-split-opacity", "unfocused-split-fill", "split-divider-color"],
        "window-padding-x" | "window-padding-y" | "window-padding-balance" | "window-padding-color" => &["window-padding-x", "window-padding-y", "window-padding-balance", "window-padding-color"],
        "font-size" | "adjust-cell-height" | "adjust-cell-width" | "font-family" => &["font-family", "font-size", "adjust-cell-height", "adjust-cell-width", "adjust-font-baseline", "font-thicken", "font-feature"],
        "background-opacity" | "background-blur" => &["background-opacity", "background-blur", "background-opacity-cells", "macos-window-shadow"],
        k if k.starts_with("palette:") => &["minimum-contrast", "bold-color", "palette-generate", "palette-harmonious"],
        _ => &[],
    }
}

fn label_of(key: &str) -> String {
    if let Some(n) = key.strip_prefix("palette:") {
        let names = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"];
        let i: usize = n.parse().unwrap_or(0);
        return if i < 16 { format!("Palette {i} ({}{})", if i >= 8 { "bright " } else { "" }, names[i % 8]) } else { format!("Palette {i}") };
    }
    catalog::find(key).map(|o| o.label.to_string()).unwrap_or_else(|| key.to_string())
}

fn ratio_line(fg: u32, bg: u32) -> String {
    let r = g::contrast(fg, bg);
    let grade = if r >= 7.0 {
        "AAA"
    } else if r >= 4.5 {
        "AA"
    } else if r >= 3.0 {
        "AA large only"
    } else {
        "hard to read"
    };
    format!("{r:.1}:1 · {grade}")
}

impl Studio {
    pub fn render_terminal(&mut self, window: &mut Window, cx: &mut Context<Studio>) -> impl IntoElement {
        let t = cx.theme().clone();
        let preview = self.render_term_preview(window, cx);
        div()
            .id("terminal.page")
            .flex()
            .flex_col()
            .gap(px(GAP))
            .on_mouse_move(cx.listener(|s, e: &MouseMoveEvent, _, cx| {
                if s.terminal.drag.is_some() {
                    if e.pressed_button == Some(MouseButton::Left) {
                        s.term_drag_move(e.position, cx);
                    } else {
                        s.term_drag_end(cx);
                    }
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|s, _, _, cx| s.term_drag_end(cx)))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|s, _, _, cx| s.term_drag_end(cx)))
            .child(Section::new("7", "Terminal"))
            .child(aside(&t, "How Ghostty and cmux look. Click anything in the window below to change what draws it; drag its padding or the gap under a line; ⌘-scroll the text for the font size."))
            .child(self.render_term_where(cx))
            .child(self.render_term_saved(cx))
            .child(preview)
            .child(self.render_term_strip(cx))
            .child(self.render_term_inspector(window, cx))
            .child(self.render_term_gallery(cx))
            .child(self.render_term_drawer(cx))
    }

    /// Applies to …, the connection to Ghostty, the last write.
    fn render_term_where(&mut self, cx: &mut Context<Studio>) -> AnyElement {
        let t = cx.theme().clone();
        let page = &self.terminal;
        let applies = page.state.applies;
        let mut row = div().flex().flex_row().flex_wrap().items_center().gap(px(8.)).child(div().w(px(96.)).flex_none().font(t.caps()).child("Applies to"));
        for (i, (to, label)) in [(AppliesTo::Both, "Ghostty + cmux"), (AppliesTo::GhosttyOnly, "Ghostty only"), (AppliesTo::CmuxOnly, "cmux only")].into_iter().enumerate() {
            row = row.child(chip(&t, format!("term.applies.{i}"), label, applies == to).on_click(cx.listener(move |s, _, _, cx| s.term_applies(to, cx))));
        }
        let what = match applies {
            AppliesTo::Both => "Writes ~/.config/ghostty/ntex-look.ghostty, which Ghostty and cmux both read, and keeps the background and foreground lines in cmux's own config.ghostty in step.",
            AppliesTo::GhosttyOnly => "Writes ntex-look.ghostty for Ghostty; cmux keeps the look it had, pinned in a marked block at the end of its own config.ghostty.",
            AppliesTo::CmuxOnly => "Writes the look into a marked block at the end of cmux's own config.ghostty; Ghostty keeps the look it had (ntex-look.ghostty).",
        };
        let mut col = div().flex().flex_col().gap(px(6.)).child(row).child(aside(&t, what));
        if !page.connected && page.loaded {
            col = col.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .child(Button::new("term.connect", "Connect to Ghostty").primary().on_click(cx.listener(|s, _, _, cx| s.term_connect(cx))))
                    .child(aside(&t, "Ghostty doesn't load ntex-look.ghostty yet. This adds `config-file = ?ntex-look.ghostty` as the last line of ~/.config/ghostty/config (backed up first). Until then the file is written but nothing reloads.")),
            );
        }
        if let Some((ok, line)) = &page.status {
            col = col.child(div().text_size(px(13.)).text_color(if *ok { t.slate } else { t.error }).child(format!("{} {line}", if *ok { "✓" } else { "!" })));
        } else if !page.loaded {
            col = col.child(shell::slate("Asking Ghostty for its options and themes…", cx));
        }
        if let Some(e) = &page.capture_error {
            col = col.child(div().text_size(px(12.)).text_color(t.slate).child(format!("(ntex could not be captured, showing the stored screen: {})", e.lines().next().unwrap_or(""))));
        }
        col.into_any_element()
    }

    fn render_term_saved(&mut self, cx: &mut Context<Studio>) -> AnyElement {
        let t = cx.theme().clone();
        let all = self.term_all_saved();
        let mut buttons = div().flex().flex_row().flex_wrap().gap(px(8.));
        for (i, saved) in all.iter().enumerate() {
            let name = saved.name.clone();
            let label = if saved.protected { format!("{} ·", saved.name) } else { saved.name.clone() };
            let mut b = Button::new(SharedString::from(format!("term.saved.{i}")), label).on_click(cx.listener(move |s, _, _, cx| s.term_restore(&name, cx)));
            if saved.look == self.terminal.state.look {
                b = b.primary();
            }
            buttons = buttons.child(b);
        }
        let sel = self.terminal.saved_selected.clone();
        let editable = sel.as_ref().is_some_and(|n| self.terminal.saved.iter().any(|s| &s.name == n && !s.protected));
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(shell::label("Saved looks", cx))
            .child(buttons)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .child(self.terminal.name.clone())
                    .child(Button::new("term.save-as", "Save current as").on_click(cx.listener(|s, _, _, cx| s.term_save_as(cx))))
                    .child(Button::new("term.rename", "Rename").disabled(!editable).on_click(cx.listener(|s, _, _, cx| s.term_rename(cx))))
                    .child(Button::new("term.delete", "Delete").disabled(!editable).on_click(cx.listener(|s, _, _, cx| s.term_delete(cx)))),
            )
            .child(aside(&t, format!("Press a look to go back to it. Marked · are protected: “Ghostty today” is the look Ghostty had when this page first opened; “{GHOSTTY_DEFAULT}” is Ghostty with nothing set. Saved looks live in looks.json.")))
            .into_any_element()
    }

    /// The colors as chips: the six special ones and the 16 ANSI colors.
    /// Click to edit; drag one ANSI chip onto another to swap them.
    fn render_term_strip(&mut self, cx: &mut Context<Studio>) -> AnyElement {
        let t = cx.theme().clone();
        let page = &self.terminal;
        let d = page.defaults.clone();
        let shown = page.shown();
        let c = Colors::of(&shown, &d);
        let sel = page.selected.clone();
        let base = self.term_base();
        let page = &self.terminal;
        let swapping = match &page.drag {
            Some(dr) => match dr.kind {
                DragKind::Swap(i) if dr.moved => Some(i),
                _ => None,
            },
            None => None,
        };
        let swatch = |key: String, label: SharedString, color: u32, edited: bool, cx: &mut Context<Studio>| -> gpui::Stateful<gpui::Div> {
            let on = sel == Target::Key(key.clone());
            let k2 = key.clone();
            div()
                .id(SharedString::from(format!("term.strip.{key}")))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(2.))
                .cursor_pointer()
                .child(div().size(px(30.)).bg(hsla(color, 1.0)).border(px(if on { 2.5 } else { 1. })).border_color(if on { t.blue } else { Theme::alpha(t.rule, 0.4) }))
                .child(div().text_size(px(10.)).font(t.mono()).text_color(if on { t.blue } else { t.slate }).child(label))
                .child(div().h(px(6.)).text_size(px(10.)).line_height(px(6.)).text_color(t.slate).child(if edited { "•" } else { "" }))
                .on_hover(cx.listener(move |s, on: &bool, _, cx| {
                    s.terminal.hover = on.then(|| k2.clone());
                    cx.notify();
                }))
        };
        let mut special = div().flex().flex_row().gap(px(8.));
        for (key, label, color) in [
            ("background", "bg", c.bg),
            ("foreground", "fg", c.fg),
            ("cursor-color", "cursor", c.cursor.resolve(c.fg, c.bg)),
            ("cursor-text", "cur. text", c.cursor_text.resolve(c.fg, c.bg)),
            ("selection-background", "select", c.sel_bg.resolve(c.fg, c.bg)),
            ("selection-foreground", "sel. text", c.sel_fg.resolve(c.fg, c.bg)),
        ] {
            let k = key.to_string();
            let changed = self.term_changed_key(key, &base);
            special = special.child(swatch(k.clone(), label.into(), color, changed, cx).on_click(cx.listener(move |s, _, _, cx| s.term_select(Target::Key(k.clone()), cx))));
        }
        let mut ansi = div().flex().flex_row().gap(px(4.));
        for i in 0u8..16 {
            let key = format!("palette:{i}");
            let edited = self.term_changed_key(&key, &base);
            let mut sw = swatch(key.clone(), format!("{i}").into(), c.palette[i as usize], edited, cx)
                .on_mouse_down(MouseButton::Left, cx.listener(move |s, e: &gpui::MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    s.term_drag_start(DragKind::Swap(i), e.position, &format!("palette:{i}"), Target::Key(format!("palette:{i}")), cx);
                }))
                .on_mouse_up(MouseButton::Left, cx.listener(move |s, _, _, cx| {
                    if let Some(dr) = s.terminal.drag.take() {
                        cx.stop_propagation();
                        match dr.kind {
                            DragKind::Swap(from) if dr.moved && from != i => s.term_swap(from, i, cx),
                            _ => s.term_select(Target::Key(format!("palette:{i}")), cx),
                        }
                    }
                }));
            if swapping.is_some_and(|f| f != i) {
                sw = sw.border_b_2().border_color(t.blue);
            }
            ansi = ansi.child(sw);
        }
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(div().flex().flex_row().flex_wrap().items_start().gap(px(18.)).child(special).child(ansi))
            .child(aside(&t, "Drag a numbered color onto another to swap them. A dot marks a color changed from where the look started."))
            .into_any_element()
    }

    /// The selected option, with the most direct controls, and its neighbors.
    fn render_term_inspector(&mut self, _window: &mut Window, cx: &mut Context<Studio>) -> AnyElement {
        let t = cx.theme().clone();
        let page = &self.terminal;
        let d = page.defaults.clone();
        let shown = page.shown();
        let c = Colors::of(&shown, &d);
        let sel = page.selected.clone();
        let mut body = div().flex().flex_col().gap(px(10.));
        match &sel {
            Target::Fixed(col) => {
                let r = g::contrast(*col, c.bg);
                body = body
                    .child(div().flex().flex_row().gap(px(14.)).items_center().child(div().size(px(48.)).bg(hsla(*col, 1.0)).border_1().border_color(t.rule)).child(
                        div().flex().flex_col().child(div().font(t.serif_bold()).child("A color ntex chose")).child(shell::mono(g::hex(*col), 13., cx)),
                    ))
                    .child(aside(&t, format!(
                        "ntex draws its page in exact colors ({}), so Ghostty's palette and background don't reach it: change it on the Look page (⌘5). Ghostty still decides the cursor, the padding around ntex, the font and the cell size. Against the terminal background this reads {r:.1}:1.",
                        g::hex(*col)
                    )));
                for k in ["window-padding-color", "minimum-contrast", "font-size"] {
                    body = body.child(self.term_opt_row(k, cx));
                }
            }
            Target::Key(key) => {
                let key = key.clone();
                let kind = if key.starts_with("palette:") { Some(Kind::Color { optional: false, special: &[] }) } else { catalog::find(&key).map(|o| o.kind) };
                body = body.child(div().flex().flex_row().items_baseline().gap(px(10.)).child(div().font(t.serif_bold()).child(label_of(&key))).child(shell::mono(if key.starts_with("palette:") { format!("palette = {}", key.trim_start_matches("palette:")) } else { key.clone() }, 12., cx)));
                if let Some(Kind::Color { optional, special }) = kind {
                    body = body.child(self.term_color_editor(&key, optional, special, &c, cx));
                } else {
                    body = body.child(self.term_opt_row(&key, cx));
                }
                if let Some(doc) = d.docs.get(key.split(':').next().unwrap_or(&key)) {
                    let first: String = doc.split("\n\n").next().unwrap_or("").replace('\n', " ");
                    if !first.is_empty() {
                        body = body.child(aside(&t, first));
                    }
                }
                let rel: Vec<&str> = related(&key).iter().filter(|k| **k != key).cloned().collect();
                if !rel.is_empty() {
                    body = body.child(shell::label("Goes with it", cx));
                    for k in rel {
                        body = body.child(self.term_opt_row(k, cx));
                    }
                }
            }
        }
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(14.))
            .border_1()
            .border_color(Theme::alpha(t.rule, 0.3))
            .child(shell::label("Selected", cx))
            .child(body)
            .into_any_element()
    }

    /// Swatch, H/S/V, hex, the special words, reset, and how it reads.
    fn term_color_editor(&mut self, key: &str, optional: bool, special: &'static [&'static str], c: &Colors, cx: &mut Context<Studio>) -> AnyElement {
        let t = cx.theme().clone();
        let page = &self.terminal;
        let d = page.defaults.clone();
        let raw = page.state.look.one(key, &d);
        let col = self.term_selected_color().unwrap_or(c.fg);
        let (h, s, v) = to_hsv(col);
        let this = cx.weak_entity();
        let k = key.to_string();
        let set = move |f: Box<dyn Fn(f32) -> u32>| {
            let this = this.clone();
            let k = k.clone();
            move |x: f32, _: &mut Window, cx: &mut App| {
                let next = f(x);
                let k = k.clone();
                this.update(cx, |st, cx| st.term_set_color(&k, next, cx)).ok();
            }
        };
        let hue = ChannelSlider::new("term.h", "H", h, 360., move |x| hsla(from_hsv(x * 360., s.max(0.35), v.max(0.55)), 1.0)).unit("°").step(2.).on_change(set(Box::new(move |x| from_hsv(x, s, v))));
        let sat = ChannelSlider::new("term.s", "S", s * 100., 100., move |x| hsla(from_hsv(h, x, v), 1.0)).unit("%").on_change(set(Box::new(move |x| from_hsv(h, x / 100., v))));
        let val = ChannelSlider::new("term.v", "V", v * 100., 100., move |x| hsla(from_hsv(h, s, x), 1.0)).unit("%").on_change(set(Box::new(move |x| from_hsv(h, s, x / 100.))));
        let mut words = div().flex().flex_row().flex_wrap().gap(px(6.));
        if optional {
            let k = key.to_string();
            words = words.child(chip(&t, format!("term.word.unset.{key}"), "unset (Ghostty picks)", raw.is_empty()).on_click(cx.listener(move |s, _, _, cx| {
                let k = k.clone();
                s.set_term(|l, d| l.put1(&k, "", d), cx)
            })));
        }
        for w in special {
            let k = key.to_string();
            words = words.child(chip(&t, format!("term.word.{w}.{key}"), *w, raw == *w).on_click(cx.listener(move |s, _, _, cx| {
                let k = k.clone();
                s.set_term(|l, d| l.put1(&k, *w, d), cx)
            })));
        }
        let edited = page.state.look.edited(key);
        let k = key.to_string();
        let reset = Button::new("term.reset", "Ghostty's own").small().disabled(!edited).on_click(cx.listener(move |s, _, _, cx| {
            let k = k.clone();
            s.set_term(|l, _| l.reset(&k), cx)
        }));
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(18.))
                    .items_start()
                    .child(div().size(px(64.)).flex_none().bg(hsla(col, 1.0)).border_1().border_color(t.rule))
                    .child(div().flex().flex_col().child(hue).child(sat).child(val))
                    .child(div().flex().flex_col().gap(px(6.)).child(self.terminal.hex.clone()).child(reset)),
            )
            .child(words)
            .child(self.term_contrast(key, col, c, cx))
            .into_any_element()
    }

    /// How the selected color reads: against the background (text colors),
    /// or the text against it (backgrounds); and what minimum-contrast does.
    fn term_contrast(&self, key: &str, col: u32, c: &Colors, cx: &App) -> AnyElement {
        let t = cx.theme().clone();
        let sample = |fg: u32, bg: u32, text: String| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .child(div().px(px(6.)).bg(hsla(bg, 1.0)).border_1().border_color(Theme::alpha(t.rule, 0.3)).text_color(hsla(g::with_min_contrast(fg, bg, c.min_contrast), 1.0)).font(t.mono()).text_size(px(13.)).child("Orbit G·x"))
                .child(div().text_size(px(13.)).text_color(t.slate).child(text))
        };
        let mut col_ = div().flex().flex_col().gap(px(4.));
        let is_bg = matches!(key, "background" | "selection-background" | "cursor-color" | "unfocused-split-fill" | "search-background" | "search-selected-background");
        if is_bg {
            let fg = match key {
                "selection-background" => c.sel_fg.resolve(c.fg, col),
                "cursor-color" => c.cursor_text.resolve(c.fg, col),
                _ => c.fg,
            };
            col_ = col_.child(sample(fg, col, format!("text on it: {}", ratio_line(fg, col))));
            if key == "background" {
                let mut weakest: Vec<(u8, f64)> = (0u8..16).map(|i| (i, g::contrast(c.palette[i as usize], col))).collect();
                weakest.sort_by(|a, b| a.1.total_cmp(&b.1));
                let list = weakest.iter().take(3).map(|(i, r)| format!("{i}: {r:.1}")).collect::<Vec<_>>().join(", ");
                col_ = col_.child(div().text_size(px(13.)).text_color(t.slate).child(format!("The palette's faintest on it: {list}.")));
            }
        } else {
            col_ = col_.child(sample(col, c.bg, format!("on the background: {}", ratio_line(col, c.bg))));
        }
        if c.min_contrast > 1.0 && !is_bg && g::with_min_contrast(col, c.bg, c.min_contrast) != col {
            col_ = col_.child(div().text_size(px(13.)).text_color(t.error).child(format!("Below minimum-contrast {}: Ghostty draws it {} instead.", c.min_contrast, g::hex(g::with_min_contrast(col, c.bg, c.min_contrast)))));
        }
        col_.into_any_element()
    }

    /// One option: label (Ghostty's docs on hover), the control its kind
    /// allows, a dot if changed, a reset. Hovering it marks it in the preview.
    pub(super) fn term_opt_row(&mut self, key: &str, cx: &mut Context<Studio>) -> AnyElement {
        let t = cx.theme().clone();
        let page = &self.terminal;
        let d = page.defaults.clone();
        let look = page.state.look.clone();
        let kind = if key.starts_with("palette:") { Kind::Color { optional: false, special: &[] } } else { catalog::find(key).map(|o| o.kind).unwrap_or(Kind::Text) };
        let doc = d.docs.get(key.split(':').next().unwrap_or(key)).cloned().unwrap_or_default();
        let base = self.term_base();
        let edited = self.term_changed_key(key, &base);
        let resettable = look.edited(key);
        let k_hover = key.to_string();
        let label = div()
            .id(SharedString::from(format!("term.lbl.{key}")))
            .w(px(170.))
            .flex_none()
            .text_size(px(14.))
            .child(label_of(key))
            .when(!doc.is_empty(), |d| d.tooltip(doc_tip(format!("{key}\n\n{doc}"))));
        let control = self.term_control(key, kind, &look, &d, cx);
        let k = key.to_string();
        div()
            .id(SharedString::from(format!("term.row.{key}")))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .min_h(px(28.))
            .on_hover(cx.listener(move |s, on: &bool, _, cx| {
                let want = on.then(|| k_hover.clone());
                if s.terminal.hover != want && (*on || s.terminal.hover.as_deref() == Some(k_hover.as_str())) {
                    s.terminal.hover = want;
                    cx.notify();
                }
            }))
            .child(label)
            .child(div().flex_1().min_w_0().child(control))
            .child(div().w(px(10.)).text_color(t.slate).child(if edited { "•" } else { "" }))
            .child(
                div()
                    .id(SharedString::from(format!("term.reset.{key}")))
                    .w(px(16.))
                    .text_color(if resettable { t.slate } else { Theme::alpha(t.slate, 0.0) })
                    .cursor_pointer()
                    .child("×")
                    .when(resettable, |d| {
                        d.on_click(cx.listener(move |s, _, _, cx| {
                            let k = k.clone();
                            s.set_term(|l, _| l.reset(&k), cx)
                        }))
                    }),
            )
            .into_any_element()
    }

    fn term_control(&mut self, key: &str, kind: Kind, look: &TermLook, d: &Defaults, cx: &mut Context<Studio>) -> AnyElement {
        let t = cx.theme().clone();
        let value = look.one(key, d);
        let k = key.to_string();
        match kind {
            Kind::Color { .. } => {
                let shown = self.terminal.shown();
                let colors = Colors::of(&shown, d);
                let col = shown.color(key, d);
                let fallback = match key {
                    "split-divider-color" => Some(colors.divider),
                    "unfocused-split-fill" => Some(colors.unfocused_fill),
                    _ => None,
                };
                let shown_col = col.or(fallback);
                div()
                    .id(SharedString::from(format!("term.ctl.{key}")))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .cursor_pointer()
                    .child(div().size(px(18.)).border_1().border_color(Theme::alpha(t.rule, 0.4)).when_some(shown_col, |d, c| d.bg(hsla(c, 1.0))))
                    .child(shell::mono(if value.is_empty() { "unset".to_string() } else { value.clone() }, 13., cx))
                    .child(div().text_size(px(12.)).text_color(t.blue).child("edit"))
                    .on_click(cx.listener(move |s, _, _, cx| s.term_select(Target::Key(k.clone()), cx)))
                    .into_any_element()
            }
            Kind::Number { min, max, step, unit } => {
                let v = value.trim_end_matches('%').parse::<f32>().unwrap_or(min);
                let steps = ((max - min) / step).round().max(1.0);
                let at = ((v - min) / step).clamp(0.0, steps);
                let ink = t.ink;
                let this = cx.weak_entity();
                ChannelSlider::new(SharedString::from(format!("term.num.{key}")), "", at, steps, move |x| {
                    let mut c = ink;
                    c.a = 0.08 + 0.6 * x;
                    c
                })
                .format(move |x| format!("{}{unit}", g::norm_number(&format!("{}", min + x * step))))
                .on_change(move |x, _, cx| {
                    let nv = g::norm_number(&format!("{}", ((min + x.round() * step) * 1000.0).round() / 1000.0));
                    let k = k.clone();
                    this.update(cx, |st, cx| st.set_term(|l, d| l.put1(&k, nv, d), cx)).ok();
                })
                .into_any_element()
            }
            Kind::Metric => self.term_stepper(key, &value, if key.starts_with("adjust-cell") { "%" } else { "" }, cx),
            Kind::Padding => {
                let (a, b) = {
                    let mut it = value.split(',').map(|x| x.trim().parse::<f32>().unwrap_or(0.0));
                    let a = it.next().unwrap_or(0.0);
                    (a, it.next().unwrap_or(a))
                };
                let names = if key.ends_with("-x") { ("left", "right") } else { ("top", "bottom") };
                let mut row = div().flex().flex_row().items_center().gap(px(14.));
                for (half, name, cur) in [(0usize, names.0, a), (1, names.1, b)] {
                    let k1 = k.clone();
                    let k2 = k.clone();
                    let bump = move |s: &mut Studio, by: f32, cx: &mut Context<Studio>| {
                        let (k, mut p) = (k1.clone(), [a, b]);
                        p[half] = (p[half] + by).clamp(0.0, 200.0);
                        let v = if p[0] == p[1] { format!("{}", p[0]) } else { format!("{},{}", p[0], p[1]) };
                        s.set_term(|l, d| l.put1(&k, v, d), cx);
                    };
                    let bump2 = bump.clone();
                    let _ = k2;
                    row = row.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.))
                            .child(div().text_size(px(13.)).text_color(t.slate).child(name))
                            .child(Button::new(SharedString::from(format!("term.pad.{key}.{half}.-")), "−").small().on_click(cx.listener(move |s, _, _, cx| bump(s, -1.0, cx))))
                            .child(shell::mono(format!("{cur}"), 13., cx))
                            .child(Button::new(SharedString::from(format!("term.pad.{key}.{half}.+")), "+").small().on_click(cx.listener(move |s, _, _, cx| bump2(s, 1.0, cx)))),
                    );
                }
                row.into_any_element()
            }
            Kind::Bool => {
                let this = cx.weak_entity();
                Checkbox::new(SharedString::from(format!("term.bool.{key}")), "", value == "true")
                    .on_toggle(move |on, _, cx| {
                        let k = k.clone();
                        this.update(cx, |st, cx| st.set_term(|l, d| l.put1(&k, if on { "true" } else { "false" }, d), cx)).ok();
                    })
                    .into_any_element()
            }
            Kind::Choice(options) => {
                let mut row = div().flex().flex_row().flex_wrap().gap(px(5.));
                for opt in options {
                    let label: SharedString = match (key, *opt) {
                        ("cursor-style", "block") => "█ block".into(),
                        ("cursor-style", "bar") => "▏ bar".into(),
                        ("cursor-style", "underline") => "▁ underline".into(),
                        ("cursor-style", "block_hollow") => "▯ hollow".into(),
                        (_, "") => "program decides".into(),
                        (_, o) => SharedString::from(o.to_string()),
                    };
                    let k = k.clone();
                    row = row.child(chip(&t, format!("term.ch.{key}.{opt}"), label, value == *opt).on_click(cx.listener(move |s, _, _, cx| {
                        let k = k.clone();
                        s.set_term(|l, d| l.put1(&k, *opt, d), cx)
                    })));
                }
                if !options.contains(&value.as_str()) && !value.is_empty() {
                    row = row.child(chip(&t, format!("term.ch.{key}.other"), value.clone(), true));
                }
                row.into_any_element()
            }
            Kind::Font => {
                let fonts = self.terminal.fonts.clone();
                let current = look.get(key, d);
                let mut row = div().flex().flex_row().flex_wrap().items_center().gap(px(5.));
                for f in ["JuliaMono", "JetBrains Mono", "SF Mono", "Menlo", "Monaco", "Iosevka", "Fira Code", "IBM Plex Mono", "Source Code Pro", "Latin Modern Mono"] {
                    if !fonts.is_empty() && !fonts.iter().any(|x| x == f) {
                        continue;
                    }
                    let on = current.first().map(String::as_str) == Some(f);
                    let k = k.clone();
                    row = row.child(chip(&t, format!("term.font.{key}.{f}"), f, on).font_family(SharedString::from(f)).on_click(cx.listener(move |s, _, _, cx| {
                        let k = k.clone();
                        s.set_term(|l, d| l.put(&k, vec![f.to_string()], d), cx)
                    })));
                }
                row.child(self.term_raw_slot(key, cx)).into_any_element()
            }
            Kind::Text => self.term_raw_slot(key, cx),
        }
    }

    /// ± stepper for `adjust-*` (unset, px or %).
    fn term_stepper(&mut self, key: &str, value: &str, unit: &'static str, cx: &mut Context<Studio>) -> AnyElement {
        let t = cx.theme().clone();
        let v = value.to_string();
        let pct = v.ends_with('%') || (v.is_empty() && unit == "%");
        let n: f32 = v.trim_end_matches('%').parse().unwrap_or(0.0);
        let mk = move |by: f32| -> String {
            let x = n + by;
            if pct { format!("{x}%") } else { format!("{x}") }
        };
        let (minus, plus) = (mk(-1.0), mk(1.0));
        let flip = if pct { format!("{}", n.round()) } else { format!("{}%", n.round()) };
        let (k1, k2, k3) = (key.to_string(), key.to_string(), key.to_string());
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .child(Button::new(SharedString::from(format!("term.step.{key}.-")), "−").small().on_click(cx.listener(move |s, _, _, cx| {
                let (k, v) = (k1.clone(), minus.clone());
                s.set_term(|l, d| l.put1(&k, v, d), cx)
            })))
            .child(div().min_w(px(44.)).font(t.mono()).text_size(px(13.)).child(if v.is_empty() { "unset".to_string() } else { v.clone() }))
            .child(Button::new(SharedString::from(format!("term.step.{key}.+")), "+").small().on_click(cx.listener(move |s, _, _, cx| {
                let (k, v) = (k2.clone(), plus.clone());
                s.set_term(|l, d| l.put1(&k, v, d), cx)
            })))
            .child(chip(&t, format!("term.unit.{key}"), if pct { "%" } else { "px" }, false).on_click(cx.listener(move |s, _, _, cx| {
                let (k, v) = (k3.clone(), flip.clone());
                s.set_term(|l, d| l.put1(&k, v, d), cx)
            })))
            .into_any_element()
    }

    /// The value as Ghostty writes it; "edit" opens the raw field here.
    fn term_raw_slot(&mut self, key: &str, cx: &mut Context<Studio>) -> AnyElement {
        let t = cx.theme().clone();
        if self.terminal.raw_key.as_deref() == Some(key) {
            return div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .child(self.terminal.raw.clone())
                .child(div().text_size(px(12.)).text_color(t.slate).child(if catalog::is_repeatable(key) { "↵ to set (a list: commas)" } else { "↵ to set" }))
                .into_any_element();
        }
        let shown = option_value_label(&self.terminal.state.look, &self.terminal.defaults, key);
        let k = key.to_string();
        div()
            .id(SharedString::from(format!("term.raw.{key}")))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .cursor_pointer()
            .child(div().font(t.mono()).text_size(px(13.)).text_color(if shown == "unset" { t.slate } else { t.ink }).max_w(px(360.)).overflow_hidden().whitespace_nowrap().child(shown))
            .child(div().text_size(px(12.)).text_color(t.blue).child("edit"))
            .on_click(cx.listener(move |s, _, w, cx| s.term_raw_edit(&k, w, cx)))
            .into_any_element()
    }

    /// Ghostty's themes as little windows: click one to start from it.
    fn render_term_gallery(&mut self, cx: &mut Context<Studio>) -> AnyElement {
        let t = cx.theme().clone();
        let page = &self.terminal;
        let q = page.filter.read(cx).text.trim().to_lowercase();
        let show = page.gallery.show;
        let themes = page.themes.clone();
        let look = page.state.look.clone();
        let d = page.defaults.clone();
        let matching: Vec<&g::Theme> = themes
            .iter()
            .filter(|th| match show {
                GalleryShow::Light => !th.dark,
                GalleryShow::Dark => th.dark,
                GalleryShow::All => true,
            })
            .filter(|th| q.is_empty() || th.name.to_lowercase().contains(&q))
            .collect();
        let limit = if page.gallery.all { matching.len() } else { 36.min(matching.len()) };
        let mut grid = div().flex().flex_row().flex_wrap().gap(px(10.));
        for th in matching.iter().take(limit) {
            let name = th.name.clone();
            let current = th.look.set.iter().all(|(k, v)| look.get(k, &d) == *v) && !th.look.set.is_empty();
            let pair_on = page.gallery.pairing && (page.gallery.pair.0.as_deref() == Some(&th.name) || page.gallery.pair.1.as_deref() == Some(&th.name));
            let mut bars = div().flex().flex_col().gap(px(3.)).px(px(7.)).pt(px(5.));
            for (w1, c1, w2, c2) in [(18., 4u8, 34., 0xffu8), (12., 1, 26., 2), (24., 5, 14., 3), (10., 6, 30., 0xff)] {
                let colr = |i: u8| if i == 0xff { th.fg } else { th.ansi[i as usize] };
                bars = bars.child(div().flex().flex_row().gap(px(4.)).child(div().w(px(w1)).h(px(4.)).bg(hsla(colr(c1), 1.0))).child(div().w(px(w2)).h(px(4.)).bg(hsla(colr(c2), 1.0))));
            }
            grid = grid.child(
                div()
                    .id(SharedString::from(format!("term.theme.{name}")))
                    .w(px(124.))
                    .h(px(72.))
                    .flex()
                    .flex_col()
                    .bg(hsla(th.bg, 1.0))
                    .border(px(if current || pair_on { 2.5 } else { 1. }))
                    .border_color(if current || pair_on { t.blue } else { Theme::alpha(t.rule, 0.25) })
                    .rounded(px(5.))
                    .overflow_hidden()
                    .cursor_pointer()
                    .child(div().px(px(7.)).pt(px(4.)).text_size(px(11.)).font(t.mono()).text_color(hsla(th.fg, 1.0)).whitespace_nowrap().overflow_hidden().child(th.name.clone()))
                    .child(bars)
                    .on_click(cx.listener(move |s, _, _, cx| s.term_theme(&name, cx))),
            );
        }
        let mut filters = div().flex().flex_row().flex_wrap().items_center().gap(px(8.)).child(self.terminal.filter.clone());
        for (i, (sh, label)) in [(GalleryShow::Light, "Light"), (GalleryShow::Dark, "Dark"), (GalleryShow::All, "All")].into_iter().enumerate() {
            filters = filters.child(chip(&t, format!("term.gal.{i}"), label, show == sh).on_click(cx.listener(move |s, _, _, cx| {
                s.terminal.gallery.show = sh;
                cx.notify();
            })));
        }
        let pairing = page.gallery.pairing;
        filters = filters.child(chip(&t, "term.gal.pair", "Light/dark pair", pairing).on_click(cx.listener(|s, _, _, cx| {
            s.terminal.gallery.pairing = !s.terminal.gallery.pairing;
            cx.notify();
        })));
        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(shell::label(format!("Themes ({} of Ghostty's {})", matching.len(), themes.len()), cx))
            .child(filters);
        if pairing {
            let (l, dk) = page.gallery.pair.clone();
            col = col.child(aside(&t, format!(
                "Pairing: click a light theme and a dark one; Ghostty then follows macOS between them (theme = light:…,dark:…). Light: {}. Dark: {}. A pair only shows once your main config's own color lines are gone: explicit colors win over any theme.",
                l.unwrap_or_else(|| "—".into()),
                dk.unwrap_or_else(|| "—".into())
            )));
        }
        if let Some((l, dk)) = look.pair() {
            let show_dark = page.show_dark;
            col = col.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(div().text_size(px(13.)).child(format!("This look is the pair {l} / {dk}. Preview:")))
                    .child(chip(&t, "term.pairshow.l", "light", !show_dark).on_click(cx.listener(|s, _, _, cx| {
                        s.terminal.show_dark = false;
                        cx.notify();
                    })))
                    .child(chip(&t, "term.pairshow.d", "dark", show_dark).on_click(cx.listener(|s, _, _, cx| {
                        s.terminal.show_dark = true;
                        cx.notify();
                    })))
                    .child(Button::new("term.unpair", "Stop pairing").small().on_click(cx.listener(|s, _, _, cx| {
                        s.terminal.gallery.pair = (None, None);
                        s.set_term(|l, _| l.reset("theme"), cx)
                    }))),
            );
        }
        col = col.child(grid);
        if matching.len() > limit {
            col = col.child(div().flex().flex_row().child(Button::new("term.gal.all", format!("Show all {}", matching.len())).on_click(cx.listener(|s, _, _, cx| {
                s.terminal.gallery.all = true;
                cx.notify();
            }))));
        }
        col.child(aside(&t, "A theme is a starting point: its colors replace the look's, everything else stays, and every color can then be changed.")).into_any_element()
    }

    /// Every look option Ghostty has, grouped.
    fn render_term_drawer(&mut self, cx: &mut Context<Studio>) -> AnyElement {
        let t = cx.theme().clone();
        let open = self.terminal.drawer_open;
        let d = self.terminal.defaults.clone();
        let unknown = d.unknown_look_keys();
        let count = catalog::LOOK.len() + 256 + unknown.len();
        let mut col = div().flex().flex_col().gap(px(8.)).child(Disclosure::new("term.drawer", format!("All options ({count})"), open).on_toggle(cx.listener_toggle()));
        if !open {
            return col.into_any_element();
        }
        col = col.child(aside(&t, "Every option of Ghostty's that changes how it looks, from Ghostty's own list. Hover a name for Ghostty's docs; hover a row to see where it shows in the window."));
        for group in catalog::GROUPS {
            let keys: Vec<String> = match *group {
                "Palette" => Vec::new(),
                "Other" => unknown.clone(),
                gname => catalog::LOOK.iter().filter(|o| o.group == gname).map(|o| o.key.to_string()).collect(),
            };
            if keys.is_empty() && *group != "Palette" {
                continue;
            }
            let gopen = self.terminal.groups_open.contains(group);
            let gname: &'static str = group;
            let edited = keys.iter().filter(|k| self.terminal.state.look.edited(k)).count() + if *group == "Palette" { (0..256).filter(|i| self.terminal.state.look.edited(&format!("palette:{i}"))).count() } else { 0 };
            let title = if edited > 0 { format!("{group} ({edited} changed)") } else { group.to_string() };
            col = col.child(div().pl(px(14.)).child(Disclosure::new(SharedString::from(format!("term.group.{group}")), title, gopen).on_toggle({
                let this = cx.weak_entity();
                move |on, _, cx| {
                    this.update(cx, |s, cx| {
                        if on {
                            s.terminal.groups_open.insert(gname);
                        } else {
                            s.terminal.groups_open.remove(gname);
                        }
                        cx.notify();
                    })
                    .ok();
                }
            })));
            if !gopen {
                continue;
            }
            let mut rows = div().flex().flex_col().gap(px(2.)).pl(px(28.));
            if *group == "Palette" {
                rows = rows.child(self.term_palette_grid(cx));
            }
            for k in keys {
                rows = rows.child(self.term_opt_row(&k, cx));
            }
            col = col.child(rows);
        }
        col.into_any_element()
    }

    /// All 256 palette entries (16–255 are the extended colors).
    fn term_palette_grid(&mut self, cx: &mut Context<Studio>) -> AnyElement {
        let t = cx.theme().clone();
        let page = &self.terminal;
        let c = Colors::of(&page.shown(), &page.defaults);
        let sel = page.selected.clone();
        let mut grid = div().flex().flex_col().gap(px(2.));
        for row in 0..16 {
            let mut line = div().flex().flex_row().gap(px(2.));
            for colm in 0..16 {
                let i = row * 16 + colm;
                let key = format!("palette:{i}");
                let on = sel == Target::Key(key.clone());
                let edited = page.state.look.edited(&key);
                line = line.child(
                    div()
                        .id(SharedString::from(format!("term.pal256.{i}")))
                        .size(px(20.))
                        .bg(hsla(c.palette[i], 1.0))
                        .border(px(if on { 2. } else if edited { 1.5 } else { 0.5 }))
                        .border_color(if on { t.blue } else { Theme::alpha(t.rule, 0.4) })
                        .cursor_pointer()
                        .on_click(cx.listener(move |s, _, _, cx| s.term_select(Target::Key(key.clone()), cx))),
                );
            }
            grid = grid.child(line);
        }
        div().flex().flex_col().gap(px(6.)).child(grid).child(aside(&t, "Click a color to edit it above. 16–231 are the color cube, 232–255 the grays; palette-generate derives them from the first 16 instead.")).into_any_element()
    }
}

trait ListenerToggle {
    fn listener_toggle(&mut self) -> impl Fn(bool, &mut Window, &mut App) + 'static;
}

impl ListenerToggle for Context<'_, Studio> {
    fn listener_toggle(&mut self) -> impl Fn(bool, &mut Window, &mut App) + 'static {
        let this = self.weak_entity();
        move |on, _, cx| {
            this.update(cx, |s, cx| {
                s.terminal.drawer_open = on;
                cx.notify();
            })
            .ok();
        }
    }
}
