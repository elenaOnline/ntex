//! The preview: a Ghostty window as Ghostty would draw it with the look, in
//! her font at its real size and cell metrics. ntex (captured from a
//! headless nvim with her config) in the focused split above, a shell below
//! (unfocused). Every cell knows its role, so recoloring a role repaints
//! it at once, and a click selects the option a cell's color comes from.
use super::{Drag, DragKind};
use crate::core::ghostty::screen::{self, Screen, Target};
use crate::core::ghostty::{self as g, Colors, Defaults, TermLook};
use crate::ui::app::Studio;
use crate::ui::screens::shell::MARGIN;
use crate::ui::widgets::theme::ActiveTheme as _;
use gpui::{
    AnyElement, Context, Font, FontFeatures, FontStyle, FontWeight, Hsla, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, ParentElement as _, Pixels, ScrollWheelEvent, SharedString, Styled as _, Window, canvas,
    div, prelude::FluentBuilder as _, px, rgb,
};
use std::rc::Rc;
use std::sync::Arc;

pub fn hsla(c: u32, a: f32) -> Hsla {
    let mut h: Hsla = rgb(c).into();
    h.a = a;
    h
}

/// The window's height in the preview (the width follows the page).
const WINDOW_H: f32 = 640.;
/// The desk around the window (where opacity and the shadow show).
const DESK: f32 = 18.;

/// Cell metrics, as Ghostty derives them from the font.
#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub size: f32,
    pub cw: f32,
    pub ch: f32,
    pub base_ch: f32,
    pub baseline_shift: f32,
}

/// `adjust-*`: unset, `N` px or `N%` of `base`.
pub fn adjusted(base: f32, v: &str) -> f32 {
    let v = v.trim();
    if v.is_empty() {
        return base;
    }
    match v.strip_suffix('%') {
        Some(p) => base * (1.0 + p.trim().parse::<f32>().unwrap_or(0.0) / 100.0),
        None => base + v.parse::<f32>().unwrap_or(0.0),
    }
}

/// The four faces of the terminal font.
pub struct Faces {
    pub regular: Font,
    pub bold: Font,
    pub italic: Font,
    pub bold_italic: Font,
}

fn features(look: &TermLook, d: &Defaults) -> FontFeatures {
    let mut out: Vec<(String, u32)> = Vec::new();
    for v in look.get("font-feature", d) {
        for f in v.split(',') {
            let f = f.trim().trim_matches('"');
            if f.is_empty() {
                continue;
            }
            let (name, val) = if let Some(n) = f.strip_prefix('-') {
                (n.to_string(), 0)
            } else if let Some(n) = f.strip_prefix('+') {
                (n.to_string(), 1)
            } else if let Some((n, v)) = f.split_once(['=', ' ']) {
                (n.trim().to_string(), match v.trim() { "on" => 1, "off" => 0, x => x.parse().unwrap_or(1) })
            } else {
                (f.to_string(), 1)
            };
            if name.len() == 4 {
                out.push((name, val));
            }
        }
    }
    FontFeatures(Arc::new(out))
}

/// The family Ghostty uses for a style (the style's own, else the main
/// one, else its built-in JetBrains Mono), as a family GPUI has.
fn family(look: &TermLook, d: &Defaults, key: &str, fonts: &[String]) -> SharedString {
    let pick = look.get(key, d).into_iter().chain(look.get("font-family", d)).find(|f| fonts.is_empty() || fonts.iter().any(|x| x == f));
    SharedString::from(pick.unwrap_or_else(|| if fonts.iter().any(|f| f == "JetBrains Mono") { "JetBrains Mono".into() } else { "Menlo".into() }))
}

pub fn faces(look: &TermLook, d: &Defaults, fonts: &[String]) -> Faces {
    let feats = features(look, d);
    let thick = look.flag("font-thicken", d);
    let normal = if thick { FontWeight::MEDIUM } else { FontWeight::NORMAL };
    let f = |key: &str, weight, style| Font { family: family(look, d, key, fonts), features: feats.clone(), fallbacks: None, weight, style };
    Faces {
        regular: f("font-family", normal, FontStyle::Normal),
        bold: f("font-family-bold", FontWeight::BOLD, FontStyle::Normal),
        italic: f("font-family-italic", normal, FontStyle::Italic),
        bold_italic: f("font-family-bold-italic", FontWeight::BOLD, FontStyle::Italic),
    }
}

pub fn metrics(window: &Window, faces: &Faces, look: &TermLook, d: &Defaults) -> Metrics {
    let ts = window.text_system();
    let size = look.number("font-size", d, 13.0).clamp(4.0, 72.0);
    let id = ts.resolve_font(&faces.regular);
    let adv = ts.advance(id, px(size), 'M').map(|s| f32::from(s.width)).unwrap_or(size * 0.6);
    let asc = f32::from(ts.ascent(id, px(size)));
    let desc = f32::from(ts.descent(id, px(size))).abs();
    // Ghostty rounds cells to whole (device) pixels: half points on Retina
    let half = |x: f32| (x * 2.0).round() / 2.0;
    let base_ch = half(asc + desc).max(1.0);
    let base_cw = half(adv).max(1.0);
    let cw = half(adjusted(base_cw, &look.one("adjust-cell-width", d))).max(1.0);
    let ch = half(adjusted(base_ch, &look.one("adjust-cell-height", d))).max(1.0);
    let baseline_shift = adjusted(0.0, &look.one("adjust-font-baseline", d));
    Metrics { size, cw, ch, base_ch, baseline_shift }
}

/// `window-padding-*`: `n` or `a,b`.
pub fn padding(look: &TermLook, d: &Defaults, key: &str) -> (f32, f32) {
    let v = look.one(key, d);
    let mut it = v.split(',').map(|x| x.trim().parse::<f32>().unwrap_or(0.0).max(0.0));
    let a = it.next().unwrap_or(2.0);
    let b = it.next().unwrap_or(a);
    (a, b)
}

/// One pane's layout.
#[derive(Clone, Copy, Debug)]
pub struct PaneBox {
    pub w: f32,
    pub h: f32,
    pub cols: usize,
    pub rows: usize,
    /// The grid's offset in the pane.
    pub left: f32,
    pub top: f32,
}

pub fn pane_box(w: f32, h: f32, m: &Metrics, look: &TermLook, d: &Defaults) -> PaneBox {
    let (l, r) = padding(look, d, "window-padding-x");
    let (t, b) = padding(look, d, "window-padding-y");
    let cols = ((w - l - r) / m.cw).floor().max(1.0) as usize;
    let rows = ((h - t - b) / m.ch).floor().max(1.0) as usize;
    let (mut left, mut top) = (l, t);
    if look.flag("window-padding-balance", d) {
        let extra_x = w - l - r - cols as f32 * m.cw;
        let extra_y = h - t - b - rows as f32 * m.ch;
        left += (extra_x / 2.0).max(0.0);
        top += (extra_y / 2.0).max(0.0);
    }
    PaneBox { w, h, cols, rows, left, top }
}

/// The rows of a screen to show in `rows`: ntex keeps its last two lines
/// (statusline, command line) at the bottom until it is re-captured at this
/// size; the shell shows its last rows, as a terminal scrolled to the end.
fn visible_rows(s: &Screen, rows: usize, keep_bottom: usize) -> Vec<Option<usize>> {
    if s.rows <= rows {
        let mut v: Vec<Option<usize>> = (0..s.rows).map(Some).collect();
        if keep_bottom > 0 {
            let tail: Vec<Option<usize>> = v.split_off(s.rows.saturating_sub(keep_bottom));
            v.resize(rows - tail.len(), None);
            v.extend(tail);
        } else {
            v.resize(rows, None);
        }
        return v;
    }
    if keep_bottom > 0 {
        let head = rows.saturating_sub(keep_bottom);
        (0..head).map(Some).chain((s.rows - keep_bottom.min(rows)..s.rows).map(Some)).collect()
    } else {
        (s.rows - rows..s.rows).map(Some).collect()
    }
}

struct PaneArgs<'a> {
    id: &'static str,
    screen: &'a Screen,
    focused: bool,
    keep_bottom: usize,
    pb: PaneBox,
    which: Which,
}

#[derive(Clone, Copy, PartialEq)]
enum Which {
    Ntex,
    Shell,
}

impl Studio {
    /// The desk with the Ghostty window on it.
    pub(super) fn render_term_preview(&mut self, window: &mut Window, cx: &mut Context<Studio>) -> AnyElement {
        let t = cx.theme().clone();
        let page = &self.terminal;
        let d = page.defaults.clone();
        let look = page.shown();
        let colors = Colors::of(&look, &d);
        let faces = faces(&look, &d, &page.fonts);
        let m = metrics(window, &faces, &look, &d);
        let vw = f32::from(window.viewport_size().width);
        let win_w = (vw - 2.0 * MARGIN - 2.0 * DESK).clamp(420.0, 1600.0);
        let style = look.one("macos-titlebar-style", &d);
        let title_h = match style.as_str() {
            "hidden" => 0.0,
            "tabs" => 38.0,
            _ => 28.0,
        };
        let content_h = WINDOW_H - title_h;
        let top_h = (content_h * 0.56).round();
        let bottom_h = content_h - top_h - 1.0;
        let top = pane_box(win_w, top_h, &m, &look, &d);
        let bottom = pane_box(win_w, bottom_h, &m, &look, &d);
        self.terminal.last_base_ch.set(m.base_ch);
        self.term_want_capture((top.cols, top.rows));
        let page = &self.terminal;
        let ntex = page.ntex.clone();
        let shell = Rc::new(screen::shell(bottom.cols.max(20)));
        let hover = page.hover.clone();
        let opacity = colors.opacity;
        let shadow = look.flag("macos-window-shadow", &d);
        let blur = !matches!(look.one("background-blur", &d).as_str(), "false" | "0" | "");

        let top_pane = self.term_pane(PaneArgs { id: "term.ntex", screen: &ntex, focused: true, keep_bottom: 2, pb: top, which: Which::Ntex }, &look, &d, &colors, &faces, &m, hover.as_deref(), cx);
        let bottom_pane =
            self.term_pane(PaneArgs { id: "term.shell", screen: &shell, focused: false, keep_bottom: 0, pb: bottom, which: Which::Shell }, &look, &d, &colors, &faces, &m, hover.as_deref(), cx);

        // the split divider: 1 px, a wider hit zone
        let mark = |k: &str| hover.as_deref() == Some(k);
        let divider = div()
            .id("term.divider")
            .relative()
            .h(px(1.))
            .w_full()
            .bg(hsla(colors.divider, 1.0))
            .child(
                div()
                    .id("term.divider.hit")
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(-4.))
                    .h(px(9.))
                    .cursor_pointer()
                    .when(mark("split-divider-color"), |d| d.border_1().border_color(t.blue))
                    .on_mouse_down(MouseButton::Left, cx.listener(|s, _, _, cx| {
                        cx.stop_propagation();
                        s.term_select(Target::key("split-divider-color"), cx);
                    })),
            );

        let titlebar = self.term_titlebar(&look, &d, &colors, win_w, title_h, cx);
        let mut win = div()
            .id("term.window")
            .relative()
            .w(px(win_w))
            .h(px(WINDOW_H))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(if style == "hidden" && look.one("window-decoration", &d) == "none" { 0. } else { 10. }))
            .bg(hsla(colors.bg, opacity))
            .border_1()
            .border_color(hsla(0x000000, if g::is_dark(colors.bg) { 0.6 } else { 0.18 }))
            .child(titlebar)
            .child(top_pane)
            .child(divider)
            .child(bottom_pane);
        if shadow {
            win = win.shadow(vec![gpui::BoxShadow {
                color: hsla(0x000000, 0.28),
                offset: gpui::point(px(0.), px(8.)),
                blur_radius: px(22.),
                spread_radius: px(0.),
                inset: false,
            }]);
        }
        if mark("background-opacity") || mark("background-blur") {
            win = win.border_2().border_color(t.blue);
        }

        // the desk: stripes that show through a see-through window (softened by blur)
        let mut desk = div()
            .id("term.desk")
            .relative()
            .p(px(DESK))
            .bg(hsla(0xdfe3ea, 1.0))
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, cx.listener(|s, _, _, cx| s.term_select(Target::key("background-opacity"), cx)));
        if opacity < 1.0 {
            let stripe = if blur { 0.10 } else { 0.35 };
            for i in 0..40 {
                desk = desk.child(div().absolute().top(px(0.)).bottom(px(0.)).left(px(i as f32 * 48.0 - 200.0)).w(px(16.)).bg(hsla(0x5b6b8c, stripe)));
            }
        }
        desk.child(win).into_any_element()
    }

    /// The title bar, as `macos-titlebar-style` draws it.
    fn term_titlebar(&mut self, look: &TermLook, d: &Defaults, c: &Colors, w: f32, h: f32, cx: &mut Context<Studio>) -> AnyElement {
        let t = cx.theme().clone();
        if h == 0.0 {
            return div().into_any_element();
        }
        let style = look.one("macos-titlebar-style", d);
        let theme = look.one("window-theme", d);
        let dark = match theme.as_str() {
            "dark" => true,
            "light" | "system" => false,
            _ => g::is_dark(c.bg),
        };
        let (bar_bg, bar_fg) = match style.as_str() {
            "native" => {
                if dark {
                    (0x2b2b2b, 0xdddddd)
                } else {
                    (0xe8e8e8, 0x3c3c3c)
                }
            }
            _ => (c.bg, if g::is_dark(c.bg) { 0xdddddd } else { 0x3c3c3c }),
        };
        let buttons = look.one("macos-window-buttons", d) != "hidden";
        let mut lights = div().flex().flex_row().gap(px(8.)).pl(px(12.)).flex_none().w(px(76.));
        if buttons {
            for col in [0xff5f57, 0xfebc2e, 0x28c840] {
                lights = lights.child(div().size(px(12.)).rounded_full().bg(hsla(col, 1.0)).border_1().border_color(hsla(0x000000, 0.12)));
            }
        }
        let title_font = look.one("window-title-font-family", d);
        let proxy = look.one("macos-titlebar-proxy-icon", d) != "hidden";
        let title = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(5.))
            .text_size(px(13.))
            .text_color(hsla(bar_fg, 1.0))
            .when(!title_font.is_empty(), |d| d.font_family(SharedString::from(title_font.clone())))
            .when(proxy, |d| d.child(div().w(px(11.)).h(px(13.)).border_1().border_color(hsla(bar_fg, 0.6)).bg(hsla(0xffffff, 0.7))))
            .child("group-actions.tex — ntex");
        let mut bar = div()
            .id("term.titlebar")
            .flex()
            .flex_row()
            .items_center()
            .h(px(h))
            .w(px(w))
            .flex_none()
            .bg(hsla(bar_bg, if style == "native" { 1.0 } else { c.opacity }))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(|s, _, _, cx| {
                cx.stop_propagation();
                s.term_select(Target::key("macos-titlebar-style"), cx);
            }))
            .child(lights);
        if style == "tabs" {
            let tab = |label: &'static str, on: bool| {
                div()
                    .flex_1()
                    .h(px(26.))
                    .mx(px(3.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .text_size(px(12.))
                    .text_color(hsla(bar_fg, if on { 1.0 } else { 0.55 }))
                    .when(on, |d| d.bg(hsla(if g::is_dark(c.bg) { 0xffffff } else { 0x000000 }, 0.07)))
                    .child(label)
            };
            bar = bar.child(div().flex_1().flex().flex_row().pr(px(10.)).child(tab("group-actions.tex — ntex", true)).child(tab("~/notes", false)));
        } else {
            bar = bar.child(div().flex_1().flex().justify_center().pr(px(76.)).child(title));
        }
        if style == "native" {
            bar = bar.border_b_1().border_color(hsla(0x000000, 0.12));
        }
        let hovered = self.terminal.hover.as_deref().is_some_and(|k| matches!(k, "macos-titlebar-style" | "window-theme" | "macos-window-buttons" | "macos-titlebar-proxy-icon" | "window-title-font-family"));
        bar.when(hovered, |d| d.border_2().border_color(t.blue)).into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn term_pane(&mut self, a: PaneArgs, look: &TermLook, d: &Defaults, c: &Colors, f: &Faces, m: &Metrics, hover: Option<&str>, cx: &mut Context<Studio>) -> AnyElement {
        let t = cx.theme().clone();
        let pb = a.pb;
        let rows = visible_rows(a.screen, pb.rows, a.keep_bottom);
        let cols = pb.cols.min(a.screen.cols);
        let style = look.one("cursor-style", d);
        let thickness = adjusted(if style == "underline" { 2.0 } else { 1.5 }, &look.one("adjust-cursor-thickness", d)).max(0.5);
        let cursor_h = adjusted(m.ch, &look.one("adjust-cursor-height", d)).clamp(1.0, m.ch * 2.0);
        let bold_font = |flags: u8| match (flags & screen::BOLD != 0, flags & screen::ITALIC != 0) {
            (true, true) => f.bold_italic.clone(),
            (true, false) => f.bold.clone(),
            (false, true) => f.italic.clone(),
            _ => f.regular.clone(),
        };
        let opacity_cells = look.flag("background-opacity-cells", d);
        let cell_alpha = if opacity_cells { c.opacity } else { 1.0 };
        let underline_y = adjusted(m.ch * 0.5 + m.size * 0.5, &look.one("adjust-underline-position", d)).min(m.ch - 1.0);
        let underline_th = adjusted(1.0, &look.one("adjust-underline-thickness", d)).max(0.5);
        let strike_y = adjusted(m.ch * 0.5 + m.baseline_shift * 0.0, &look.one("adjust-strikethrough-position", d));
        let strike_th = adjusted(1.0, &look.one("adjust-strikethrough-thickness", d)).max(0.5);

        let mut grid = div().relative().w(px(cols as f32 * m.cw)).h(px(pb.rows as f32 * m.ch));
        let mut edge_bg: (Option<u32>, Option<u32>) = (None, None);
        // rows ntex hasn't been re-captured for yet: its page color, not a hole
        let filler = a.screen.cells.first().and_then(|l| l.last()).map(|cell| screen::draw(cell, c, false)).filter(|d| !d.plain_bg).map(|d| d.bg);
        for (vr, src) in rows.iter().enumerate() {
            let Some(r) = *src else {
                if let Some(bg) = filler {
                    grid = grid.child(div().absolute().left_0().top(px(vr as f32 * m.ch)).w(px(cols as f32 * m.cw)).h(px(m.ch)).bg(hsla(bg, cell_alpha)));
                }
                continue;
            };
            let line = &a.screen.cells[r];
            let y = vr as f32 * m.ch;
            let mut run: Option<(usize, u32, bool)> = None;
            // cell backgrounds under the glyphs
            let mut back = div().absolute().left_0().top_0().size_full();
            let mut row = div().absolute().left_0().top_0().size_full();
            let flush = |row: gpui::Div, run: Option<(usize, u32, bool)>, end: usize| -> gpui::Div {
                match run {
                    Some((start, bg, marked)) => row.child(
                        div()
                            .absolute()
                            .left(px(start as f32 * m.cw))
                            .top_0()
                            .w(px((end - start) as f32 * m.cw))
                            .h(px(m.ch))
                            .bg(hsla(bg, cell_alpha))
                            .when(marked, |d| d.border_1().border_color(t.blue)),
                    ),
                    None => row,
                }
            };
            for (col, cell) in line.iter().take(cols).enumerate() {
                let is_cursor = a.focused && a.screen.cursor == Some((r, col)) && (style == "block");
                let dr = screen::draw(cell, c, is_cursor);
                if col == 0 && vr == 0 {
                    edge_bg.0 = (!dr.plain_bg).then_some(dr.bg);
                }
                if col == 0 {
                    edge_bg.1 = (!dr.plain_bg).then_some(dr.bg);
                }
                let marked_bg = hover.is_some_and(|k| !dr.plain_bg && matches!(&dr.bg_from, Target::Key(x) if x == k));
                let want = (!dr.plain_bg).then_some((dr.bg, marked_bg));
                match (run, want) {
                    (Some((s, bg, mk)), Some((b, m2))) if bg == b && mk == m2 => run = Some((s, bg, mk)),
                    (prev, w) => {
                        back = flush(back, prev, col);
                        run = w.map(|(b, mk)| (col, b, mk));
                    }
                }
                let ch = cell.ch.as_str();
                if !ch.is_empty() && ch != " " {
                    let faint = cell.flags & screen::FAINT != 0;
                    let alpha = if faint { c.faint } else { 1.0 };
                    let marked = hover.is_some_and(|k| matches!(&dr.fg_from, Target::Key(x) if x == k));
                    row = row.child(
                        div()
                            .absolute()
                            .left(px(col as f32 * m.cw))
                            .top(px(-m.baseline_shift))
                            .h(px(m.ch))
                            .line_height(px(m.ch))
                            .text_size(px(m.size))
                            .font(bold_font(cell.flags))
                            .whitespace_nowrap()
                            .text_color(hsla(dr.fg, alpha))
                            .when(marked, |d| d.bg(hsla(0x1e47b8, 0.18)))
                            .child(SharedString::from(ch.to_string())),
                    );
                    if cell.flags & screen::UNDERLINE != 0 {
                        row = row.child(div().absolute().left(px(col as f32 * m.cw)).top(px(underline_y)).w(px(m.cw)).h(px(underline_th)).bg(hsla(dr.fg, alpha)));
                    }
                    if cell.flags & screen::STRIKE != 0 {
                        row = row.child(div().absolute().left(px(col as f32 * m.cw)).top(px(strike_y)).w(px(m.cw)).h(px(strike_th)).bg(hsla(dr.fg, alpha)));
                    }
                }
                // the cursor's other shapes, and the unfocused split's hollow block
                if a.screen.cursor == Some((r, col)) && !(a.focused && style == "block") {
                    let cc = screen::cursor_color(cell, c);
                    let al = c.cursor_opacity;
                    let x = col as f32 * m.cw;
                    let top_y = (m.ch - cursor_h) / 2.0;
                    let cur = match (a.focused, style.as_str()) {
                        (true, "bar") => div().absolute().left(px(x)).top(px(top_y)).w(px(thickness)).h(px(cursor_h)).bg(hsla(cc, al)),
                        (true, "underline") => div().absolute().left(px(x)).top(px(m.ch - thickness)).w(px(m.cw)).h(px(thickness)).bg(hsla(cc, al)),
                        _ => div().absolute().left(px(x)).top(px(top_y)).w(px(m.cw)).h(px(cursor_h)).border(px(thickness.min(2.0))).border_color(hsla(cc, al)),
                    };
                    let marked = hover.is_some_and(|k| k.starts_with("cursor-") || k == "adjust-cursor-thickness" || k == "adjust-cursor-height");
                    row = row.child(cur.when(marked, |d| d.shadow(vec![gpui::BoxShadow { color: hsla(0x1e47b8, 0.9), offset: gpui::point(px(0.), px(0.)), blur_radius: px(0.), spread_radius: px(2.), inset: false }])));
                }
            }
            back = flush(back, run, cols);
            grid = grid.child(div().absolute().left_0().top(px(y)).w(px(cols as f32 * m.cw)).h(px(m.ch)).child(back).child(row));
        }
        // where the grid is (clicks → cells)
        let geo = self.terminal.geometry.clone();
        let which = a.which;
        grid = grid.child(
            canvas(
                move |b, _, _| match which {
                    Which::Ntex => geo.ntex.set(Some(b)),
                    Which::Shell => geo.shell.set(Some(b)),
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        );
        let (cw, chh) = (m.cw, m.ch);
        let geo = self.terminal.geometry.clone();
        let s1 = Rc::new(a.screen.clone());
        let r1 = Rc::new(rows.clone());
        let colors = c.clone();
        let focused = a.focused;
        let grid = div()
            .id(SharedString::from(format!("{}.grid", a.id)))
            .absolute()
            .left(px(pb.left))
            .top(px(pb.top))
            .cursor_text()
            .child(grid)
            .on_mouse_down(MouseButton::Left, cx.listener(move |s, e: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                let b = match which {
                    Which::Ntex => geo.ntex.get(),
                    Which::Shell => geo.shell.get(),
                };
                let Some(b) = b else { return };
                let x = f32::from(e.position.x - b.origin.x);
                let y = f32::from(e.position.y - b.origin.y);
                let col = (x / cw).floor().max(0.0) as usize;
                let vr = (y / chh).floor().max(0.0) as usize;
                let within = y - vr as f32 * chh;
                let target = cell_target(&s1, &r1, vr, col, x - col as f32 * cw, cw, &colors, focused);
                // near the line under a row: a cell-height drag (a click still selects)
                if within > chh - 3.0 && vr + 1 < r1.len() {
                    s.term_drag_start(DragKind::CellHeight { row: vr }, e.position, "adjust-cell-height", Target::key("adjust-cell-height"), cx);
                    return;
                }
                s.term_select(target, cx);
            }))
            .on_scroll_wheel(cx.listener(|s, e: &ScrollWheelEvent, _, cx| {
                if !e.modifiers.platform {
                    return;
                }
                cx.stop_propagation();
                let dy = f32::from(e.delta.pixel_delta(px(16.)).y);
                if dy.abs() < 0.5 {
                    return;
                }
                let step = if dy > 0.0 { 0.5 } else { -0.5 };
                s.set_term(
                    |l, d| {
                        let v = (l.number("font-size", d, 13.0) + step).clamp(6.0, 36.0);
                        l.put1("font-size", g::norm_number(&format!("{v}")), d);
                    },
                    cx,
                );
                s.terminal.selected = Target::key("font-size");
            }));

        // the pane: padding around the grid (drag its edges), the unfocused veil
        let pad_bg = if look.one("window-padding-color", d).starts_with("extend") { edge_bg.0.unwrap_or(c.bg) } else { c.bg };
        let mut pane = div()
            .id(a.id)
            .relative()
            .w(px(pb.w))
            .h(px(pb.h))
            .flex_none()
            .overflow_hidden()
            .bg(hsla(pad_bg, c.opacity))
            .on_mouse_down(MouseButton::Left, cx.listener(move |s, e: &MouseDownEvent, _, cx| {
                // which padding edge: the nearest side of the grid
                let geo = s.terminal.geometry.clone();
                let b = match which {
                    Which::Ntex => geo.ntex.get(),
                    Which::Shell => geo.shell.get(),
                };
                let Some(b) = b else { return };
                let (x, y) = (e.position.x, e.position.y);
                let kind = if x < b.origin.x {
                    DragKind::Padding { key: "window-padding-x", half: 0, sign: 1.0 }
                } else if x > b.origin.x + b.size.width {
                    DragKind::Padding { key: "window-padding-x", half: 1, sign: -1.0 }
                } else if y < b.origin.y {
                    DragKind::Padding { key: "window-padding-y", half: 0, sign: 1.0 }
                } else {
                    DragKind::Padding { key: "window-padding-y", half: 1, sign: -1.0 }
                };
                let key = match &kind {
                    DragKind::Padding { key, .. } => *key,
                    _ => "window-padding-x",
                };
                s.term_drag_start(kind, e.position, key, Target::key(key), cx);
            }))
            .child(grid);
        if let Some(k) = hover.filter(|k| k.starts_with("window-padding")) {
            let (x, y) = (!k.ends_with("-y"), !k.ends_with("-x"));
            let right = (pb.w - pb.left - cols as f32 * m.cw).max(0.0);
            let bottom = (pb.h - pb.top - pb.rows as f32 * m.ch).max(0.0);
            pane = pane.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(px(pb.w))
                    .h(px(pb.h))
                    .border_color(hsla(0x1e47b8, 0.45))
                    .border_l(px(if x { pb.left } else { 0. }))
                    .border_r(px(if x { right } else { 0. }))
                    .border_t(px(if y { pb.top } else { 0. }))
                    .border_b(px(if y { bottom } else { 0. })),
            );
        }
        if !a.focused {
            let veil = 1.0 - c.unfocused_opacity;
            if veil > 0.001 {
                pane = pane.child(div().absolute().top_0().left_0().w(px(pb.w)).h(px(pb.h)).bg(hsla(c.unfocused_fill, veil)));
            }
            let marked = hover.is_some_and(|k| k.starts_with("unfocused-split"));
            pane = pane.child(
                div()
                    .id("term.unfocused")
                    .absolute()
                    .top(px(4.))
                    .right(px(6.))
                    .px(px(5.))
                    .rounded(px(3.))
                    .text_size(px(10.))
                    .font(t.mono())
                    .bg(hsla(0x1e47b8, if marked { 0.9 } else { 0.12 }))
                    .text_color(if marked { hsla(0xffffff, 1.0) } else { t.blue })
                    .cursor_pointer()
                    .child("unfocused split")
                    .on_mouse_down(MouseButton::Left, cx.listener(|s, _, _, cx| {
                        cx.stop_propagation();
                        s.term_select(Target::key("unfocused-split-opacity"), cx);
                    })),
            );
        }
        if hover.is_some_and(|k| k.starts_with("font-") || k.starts_with("adjust-cell") || k == "adjust-font-baseline") {
            pane = pane.child(div().absolute().left(px(pb.left)).top(px(pb.top)).w(px(cols as f32 * m.cw)).h(px(pb.rows as f32 * m.ch)).border_1().border_color(t.blue));
        }
        pane.into_any_element()
    }

    pub fn term_drag_start(&mut self, kind: DragKind, at: gpui::Point<Pixels>, key: &str, click: Target, cx: &mut Context<Studio>) {
        let from = self.terminal.state.look.one(key, self.terminal.d());
        self.terminal.drag = Some(Drag { kind, start: at, from, moved: false, click });
        cx.notify();
    }

    /// The mouse moved while a drag is on (the page's root listens).
    pub fn term_drag_move(&mut self, at: gpui::Point<Pixels>, cx: &mut Context<Studio>) {
        let Some(drag) = self.terminal.drag.clone() else { return };
        let dx = f32::from(at.x - drag.start.x);
        let dy = f32::from(at.y - drag.start.y);
        if !drag.moved && dx.abs() < 3.0 && dy.abs() < 3.0 {
            return;
        }
        if let Some(dr) = &mut self.terminal.drag {
            dr.moved = true;
        }
        match drag.kind {
            DragKind::Padding { key, half, sign } => {
                let delta = if key == "window-padding-x" { dx } else { dy } * sign;
                let parts: Vec<f32> = drag.from.split(',').map(|x| x.trim().parse().unwrap_or(0.0)).collect();
                // one edge moves: `n` becomes `left,right` (and back to `n` when equal)
                let a = parts.first().copied().unwrap_or(0.0);
                let b = parts.get(1).copied().unwrap_or(a);
                let mut p = [a, b];
                p[half.min(1)] = (p[half.min(1)] + delta).round().clamp(0.0, 200.0);
                let v = if p[0] == p[1] { format!("{}", p[0]) } else { format!("{},{}", p[0], p[1]) };
                self.terminal.selected = Target::key(key);
                self.set_term(|l, d| l.put1(key, v, d), cx);
            }
            DragKind::CellHeight { row } => {
                // the line under `row` moves by (row + 1) × the change per row
                let per_row = dy / (row as f32 + 1.0);
                let base = self.terminal.last_base_ch.get();
                let from = drag.from.trim();
                let v = if from.is_empty() || from.ends_with('%') {
                    let pct: f32 = from.trim_end_matches('%').parse().unwrap_or(0.0);
                    format!("{}%", (pct + per_row / base.max(1.0) * 100.0).round().clamp(-60.0, 300.0))
                } else {
                    let px_: f32 = from.parse().unwrap_or(0.0);
                    format!("{}", (px_ + per_row).round().clamp(-10.0, 60.0))
                };
                self.terminal.selected = Target::key("adjust-cell-height");
                self.set_term(|l, d| l.put1("adjust-cell-height", v, d), cx);
            }
            DragKind::Swap(_) => cx.notify(),
        }
    }

    /// The button came up: a drag that never moved is a click.
    pub fn term_drag_end(&mut self, cx: &mut Context<Studio>) {
        if let Some(drag) = self.terminal.drag.take() {
            if !drag.moved && !matches!(drag.kind, DragKind::Swap(_)) {
                self.term_select(drag.click, cx);
            }
            cx.notify();
        }
    }
}

/// What clicking cell (`vr`, `col`) selects: its glyph's color if the click
/// is on a glyph, else its background (the cursor: its colors).
#[allow(clippy::too_many_arguments)]
fn cell_target(s: &Screen, rows: &[Option<usize>], vr: usize, col: usize, _x: f32, _cw: f32, c: &Colors, focused: bool) -> Target {
    let Some(Some(r)) = rows.get(vr) else { return Target::key("background") };
    let Some(cell) = s.cells.get(*r).and_then(|l| l.get(col)) else { return Target::key("background") };
    let cursor = focused && s.cursor == Some((*r, col));
    let d = screen::draw(cell, c, cursor);
    let glyph = !cell.ch.trim().is_empty();
    if cursor {
        return Target::key("cursor-color");
    }
    if s.cursor == Some((*r, col)) {
        return Target::key("cursor-color");
    }
    if glyph { d.fg_from } else { d.bg_from }
}
