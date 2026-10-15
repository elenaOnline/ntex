//! (figure-qol B) The panel's Figure section: how big the figure is on the
//! page. Always there; open when nothing is selected, folded under the
//! selection's section otherwise (a click on its head toggles).
//!
//! Width on page (cm) is the figure's size on the PDF page: typing a width
//! solves for `scale` (labels don't scale, so it iterates; see
//! `ntex_figures::pagesize`). Scale, Spacing and Label size are the globals.
use super::{choice, fmt_num, note, parse_num, row, unit, FigureView, Slider};
use crate::ui::figure::menu::record_bounds;
use crate::ui::widgets::theme::Theme;
use gpui::{AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px};
use ntex_figures::pagesize;

impl FigureView {
    /// Is the Figure section open?
    pub fn figure_section_open(&self) -> bool {
        self.figure_section.unwrap_or(self.selection.is_empty())
    }

    /// The section: its head, and (when open) its fields.
    pub(super) fn panel_figure_section(&mut self, t: &Theme, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let open = self.figure_section_open();
        let size = self.page_readout().map(|r| r.replace(" on the page", "")).unwrap_or_default();
        let head = div()
            .id("panel.figure.toggle")
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .mt(px(8.))
            .pt(px(6.))
            .min_h(px(26.))
            .border_t_1()
            .border_color(Theme::alpha(t.slate, 0.35))
            .cursor_pointer()
            .child(div().flex_none().w(px(12.)).font(t.math_font()).text_color(t.slate).child(if open { "▾" } else { "▸" }))
            .child(div().font(t.caps()).text_size(t.body_size).text_color(t.ink).child("Figure"))
            .child(div().flex_1().text_right().font(t.serif_italic()).text_size(t.small_size).text_color(t.slate).child(size))
            .child(record_bounds(self.ui_bounds.clone(), "panel.figure.toggle".to_string()))
            .on_click(cx.listener(|v, _, _, cx| {
                v.figure_section = Some(!v.figure_section_open());
                cx.notify();
            }))
            .into_any_element();
        let mut out = vec![head];
        if open {
            out.extend(self.figure_rows(t, window, cx));
        }
        out
    }

    fn figure_rows(&mut self, t: &Theme, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let g = self.doc.figure.globals.clone();
        // mid-drag of the page corner the numbers follow the draft
        let fig = self.drag_figure.clone().unwrap_or_else(|| self.doc.figure.clone());
        let scene_w = {
            let scene = self.drag_scene.clone().unwrap_or_else(|| self.doc.scene.clone());
            pagesize::page_size(&scene).0
        };
        let shown_scale = fig.globals.scale;
        let has_size = !crate::ui::figure::geom::is_empty_rect(&self.doc.scene.bounds) && !self.doc.is_empty();
        let width_text = if has_size { format!("{scene_w:.1}") } else { String::new() };
        let width = self.field("fig.width", &width_text, true, Some(52.), "cm", window, cx, |v, s, _, cx| {
            let Some(w) = parse_num(s).filter(|w| *w >= 0.5) else { return };
            // a width that can't be had (the labels alone are wider) changes nothing
            if let Some(scale) = pagesize::scale_for_width(&v.doc.figure, w) {
                v.width_note = None;
                v.set_globals(Some("field:fig.scale".into()), cx, |g| {
                    g.scale = scale;
                    true
                });
            } else if !crate::ui::figure::geom::is_empty_rect(&v.doc.scene.bounds) && !v.doc.is_empty() {
                // (review U13) say why nothing moved
                let least = pagesize::width_at(&v.doc.figure, pagesize::SCALE_MIN);
                let most = pagesize::width_at(&v.doc.figure, pagesize::SCALE_MAX);
                v.width_note = Some(if w < least { format!("Can't be that narrow: the labels alone are {least:.1} cm wide.") } else { format!("Can't be that wide: the biggest scale ({}×) gives {most:.1} cm.", pagesize::SCALE_MAX) });
                cx.notify();
            }
        });
        let scale = self.field("fig.scale", &fmt_num(shown_scale), true, Some(52.), "1", window, cx, |v, s, _, cx| {
            if let Some(x) = parse_num(s).filter(|x| *x > 0.05) {
                v.width_note = None;
                v.set_globals(Some("field:fig.scale".into()), cx, |g| {
                    g.scale = x;
                    true
                });
            }
        });
        let start = self.on(cx, |v, _, _| v.undo_group = None);
        let change = self.on2f(cx, |v, x, cx| {
            let x = (x * 100.0).round() / 100.0;
            v.set_globals(Some("slider:fig.scale".into()), cx, |g| {
                g.scale = x;
                true
            });
        });
        let slider = Slider::new(SharedString::from("fig.scale.slider"), shown_scale.clamp(0.2, 3.0), 0.2, 3.0).on_start(start).on_change(change);
        let sx = self.field("fig.spacing.x", &fmt_num(g.spacing.0), true, Some(44.), "1.9", window, cx, |v, s, _, cx| {
            if let Some(x) = parse_num(s).filter(|x| *x > 0.1) {
                v.set_globals(Some("field:fig.spacing".into()), cx, |g| {
                    g.spacing.0 = x;
                    true
                });
            }
        });
        let sy = self.field("fig.spacing.y", &fmt_num(g.spacing.1), true, Some(44.), "1.35", window, cx, |v, s, _, cx| {
            if let Some(y) = parse_num(s).filter(|y| *y > 0.1) {
                v.set_globals(Some("field:fig.spacing".into()), cx, |g| {
                    g.spacing.1 = y;
                    true
                });
            }
        });
        let sizes = ["small", "normal", "large"];
        let cur = sizes.iter().position(|s| *s == g.size);
        let go = self.on2(cx, move |v, i, cx| {
            let s = sizes[i].to_string();
            v.set_globals(None, cx, |g| {
                g.size = s;
                true
            });
        });
        let width_note = self.width_note.clone().map(|n| note(t, n).id("fig.width.note").into_any_element());
        let flex = |c: AnyElement| div().w_full().flex().flex_row().items_center().gap(px(6.)).child(c);
        vec![
            row(t, "Width", flex(width).child(unit(t, "cm on the page"))).into_any_element(),
        ].into_iter().chain(width_note).chain(vec![
            row(
                t,
                "Scale",
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .child(scale)
                    .child(unit(t, "×"))
                    .child(div().relative().flex_1().child(slider).child(record_bounds(self.ui_bounds.clone(), "slider.fig.scale".to_string()))),
            )
            .into_any_element(),
            row(t, "Spacing", div().flex().flex_row().items_center().gap(px(4.)).child(sx).child(unit(t, "×")).child(sy).child(unit(t, "cm"))).into_any_element(),
            row(t, "Labels", choice(t, "panel.size", &sizes, cur, &self.ui_bounds, go)).into_any_element(),
            note(t, "Geometry scales; labels stay the notes' size. Hold ⌘ for the page frame and drag a corner; ⌘1 shows it at actual size.").into_any_element(),
        ]).collect()
    }
}
