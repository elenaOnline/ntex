//! The page frame: the running head (fancyhdr), the scrolling page body,
//! the pinned footnotes (keys for the focused control) and the toast (a
//! margin note in the footer line). Owner: the SCREENS builder.
use crate::ui::app::{Page, Studio, TOAST_FADE, TOAST_FOR};
use crate::ui::widgets::boxes::{Footnotes, MarginNote, RunningHead, aside, caps_label};
use crate::ui::widgets::theme::ActiveTheme as _;
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

/// Left/right page margin (UX.md: 44 px).
pub const MARGIN: f32 = 44.0;
/// Room for the traffic lights on the paper (transparent title bar).
pub const TITLEBAR: f32 = 30.0;

/// The running head: *Ntex Studio* left, the pages in small caps right.
pub fn running_head(studio: &Studio, cx: &mut Context<Studio>) -> impl IntoElement {
    let count = studio.inbox.len();
    let current = match studio.page {
        Page::Snippet => 0,
        Page::Block => 1,
        Page::Inbox => 2,
    };
    let this = cx.weak_entity();
    div().pt(px(TITLEBAR)).px(px(MARGIN)).child(
        RunningHead::new("running-head", "Ntex Studio")
            .page("Snippet", None)
            .page("Block", None)
            .page("Inbox", (count > 0).then_some(count))
            .current(current)
            .on_select(move |i, window, cx| {
                let page = [Page::Snippet, Page::Block, Page::Inbox][i.min(2)];
                this.update(cx, |s, cx| s.set_page(page, window, cx)).ok();
            }),
    )
}

/// The pinned footer: the toast (a margin note) and the footnotes (hints).
pub fn footer(studio: &Studio, window: &Window, cx: &mut Context<Studio>) -> impl IntoElement {
    let t = cx.theme().clone();
    let notes = studio.hints(window, cx);
    let now = studio.now(cx);
    let toast = studio.toast.as_ref().filter(|toast| toast.page == studio.page).map(|toast| {
        MarginNote::new(SharedString::from(format!("toast.{}", toast.id)), toast.text.clone())
            .duration(TOAST_FOR + TOAST_FADE)
            .opacity(toast.opacity(now))
    });
    div()
        .flex()
        .flex_col()
        .px(px(MARGIN))
        .pt(px(6.))
        .pb(px(14.))
        .bg(t.paper)
        .when_some(toast, |d, note| {
            d.child(div().id("toast").w_full().flex().flex_row().justify_end().pb(px(6.)).child(div().max_w(px(560.)).child(note)))
        })
        .child(Footnotes::new(notes))
}

/// Running head, the scrolling body, the footer.
pub fn frame(studio: &mut Studio, body: AnyElement, window: &mut Window, cx: &mut Context<Studio>) -> AnyElement {
    let t = cx.theme().clone();
    let head = running_head(studio, cx).into_any_element();
    let foot = footer(studio, window, cx).into_any_element();
    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(t.paper)
        .text_color(t.ink)
        .font(t.serif())
        .text_size(t.body_size)
        .line_height(t.line(t.body_size))
        .child(head)
        .child(
            div()
                .id("page")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&studio.scroll)
                .child(div().px(px(MARGIN)).pt(px(28.)).pb(px(28.)).child(body)),
        )
        .child(foot)
        .into_any_element()
}

/// A field label in small caps (`\\textsc`).
pub fn label(text: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    caps_label(cx.theme(), text).flex_none()
}

/// Italic slate remark text (log lines, placeholders, "(guessed)").
pub fn slate(text: impl Into<SharedString>, cx: &App) -> gpui::Div {
    aside(cx.theme(), text)
}

/// LM Mono text at the given size.
pub fn mono(text: impl Into<SharedString>, size: f32, cx: &App) -> gpui::Div {
    div().font(cx.theme().mono()).text_size(px(size)).child(text.into())
}

/// The vertical rhythm between boxes.
pub const GAP: f32 = 14.0;
