//! A single-line (or multi-line) text field drawn as a fill-in blank: a thin
//! baseline rule, blue when focused. IME, selection, clipboard, ⌥-word and
//! ⌘-line motions, undo. Model: GPUI's examples/input.rs; the editing core is `edit.rs`.
//!
//! Keys (context "NtexEdit TextField"): ←/→/↑/↓ (⇧ selects), ⌥←/→ words,
//! ⌘←/→ line, ⌘↑/↓ document, ⌫ ⌦ ⌥⌫ ⌘⌫, ⌘A ⌘C ⌘X ⌘V, ⌘Z ⌘⇧Z, Return
//! (submit; a newline when multi-line), ⌘↵ submit, Esc cancel. Tab is left to
//! the page (focus moves on).
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, ParentElement, Pixels, Render, SharedString,
    Subscription, Window, div, prelude::*, px,
};

use super::edit::{self, Buffer, EditArea, Editable, Item, Look, Op, Outcome, TextLayout};
use super::theme::ActiveTheme as _;

pub enum TextFieldEvent {
    Changed,
    /// Return (single-line) / ⌘↵.
    Submit,
    /// Esc.
    Cancel,
    Blur,
}

pub struct TextField {
    pub focus: FocusHandle,
    pub text: String,
    pub placeholder: SharedString,
    pub mono: bool,
    pub multiline: bool,
    // ---- added by WIDGETS
    /// Fixed width of the blank; None = the container's width.
    pub width: Option<Pixels>,
    buffer: Buffer,
    /// `text` as last synced with the buffer (so a screen may still assign
    /// `field.text` directly).
    synced: String,
    layout: Option<Rc<TextLayout>>,
    selecting: bool,
    _typing: Option<gpui::Subscription>,
    blur_sub: Option<Subscription>,
    focused_now: bool,
}

impl TextField {
    pub fn new(cx: &mut Context<Self>) -> TextField {
        TextField {
            focus: cx.focus_handle(),
            text: String::new(),
            placeholder: SharedString::default(),
            mono: false,
            multiline: false,
            width: None,
            buffer: Buffer::default(),
            synced: String::new(),
            layout: None,
            selecting: false,
            _typing: Some(edit::intercept_typing(cx)),
            blur_sub: None,
            focused_now: false,
        }
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.text = text.into();
        self.sync_from_text();
        cx.notify();
    }
    pub fn placeholder(mut self, p: impl Into<SharedString>) -> Self {
        self.placeholder = p.into();
        self
    }
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }
    pub fn multiline(mut self) -> Self {
        self.multiline = true;
        self
    }
    pub fn width(mut self, w: Pixels) -> Self {
        self.width = Some(w);
        self
    }
    /// The selection, in chars.
    pub fn selection(&self) -> Range<usize> {
        self.buffer.sel.clone()
    }
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.buffer.select_all();
        cx.notify();
    }
    /// Can ⌘Z undo something in this field?
    pub fn can_undo(&self) -> bool {
        self.buffer.can_undo()
    }

    fn sync_from_text(&mut self) {
        if self.text != self.synced {
            self.buffer.reset(edit::items_from_str(&self.text));
            self.synced = self.text.clone();
        }
    }

    fn clean(&self, s: &str) -> String {
        if self.multiline { s.to_string() } else { s.replace(['\n', '\r', '\t'], " ") }
    }
}

impl Editable for TextField {
    fn buffer(&self) -> &Buffer {
        &self.buffer
    }
    fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }
    fn focus_handle_ref(&self) -> &FocusHandle {
        &self.focus
    }
    fn layout_cache(&mut self) -> &mut Option<Rc<TextLayout>> {
        &mut self.layout
    }
    fn selecting(&mut self) -> &mut bool {
        &mut self.selecting
    }
    fn after_edit(&mut self, cx: &mut Context<Self>) {
        self.text = self.buffer.text();
        self.synced = self.text.clone();
        cx.emit(TextFieldEvent::Changed);
        cx.notify();
    }
    fn input_text(&mut self, range_utf16: Option<Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        let text = self.clean(text);
        let r = range_utf16.map(|r| self.buffer.range_from_utf16(&r));
        self.buffer.ime_replace(r, &text);
        self.after_edit(cx);
    }
    fn op(&mut self, op: Op, _window: &mut Window, cx: &mut Context<Self>) {
        match op {
            Op::Copy => {
                edit::copy_plain(&self.buffer, cx);
            }
            Op::Cut => {
                if edit::copy_plain(&self.buffer, cx) {
                    self.buffer.delete_to(self.buffer.caret());
                    self.after_edit(cx);
                }
            }
            Op::Paste => {
                if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    let text = self.clean(&text);
                    self.buffer.insert(edit::items_from_str(&text));
                    self.buffer.break_undo();
                    self.after_edit(cx);
                }
            }
            Op::Newline if self.multiline => {
                // auto-indent: the previous line's leading tabs and spaces
                let c = self.buffer.sel.start;
                let start = self.buffer.line_start(c);
                let mut new = vec![Item::Ch('\n')];
                new.extend(
                    self.buffer.items[start..c].iter().take_while(|it| matches!(it, Item::Ch(' ') | Item::Ch('\t'))).cloned(),
                );
                self.buffer.insert(new);
                self.after_edit(cx);
            }
            Op::Newline | Op::Submit => cx.emit(TextFieldEvent::Submit),
            Op::Cancel => cx.emit(TextFieldEvent::Cancel),
            Op::Up(_) | Op::Down(_) if !self.multiline => {
                // single line: ↑/↓ go to the ends, as in a macOS field
                let to = if op == Op::Up(false) || op == Op::Up(true) { 0 } else { self.buffer.len() };
                let sel = matches!(op, Op::Up(true) | Op::Down(true));
                if sel { self.buffer.select_to(to) } else { self.buffer.move_to(to) }
                cx.notify();
            }
            other => match edit::apply_op(&mut self.buffer, other) {
                Outcome::Edited => self.after_edit(cx),
                Outcome::Moved => cx.notify(),
                Outcome::None => {}
            },
        }
    }
    fn look(&self, cx: &App) -> Look {
        let t = cx.theme();
        let size = if self.mono { t.mono_size } else { t.body_size };
        let font = if self.mono { t.mono() } else { t.serif() };
        Look {
            font,
            size,
            line_height: t.line(size).max(px(20.)),
            color: t.ink,
            placeholder: self.placeholder.clone(),
            placeholder_font: if self.mono { t.mono_italic() } else { t.serif_italic() },
            placeholder_color: t.slate,
            caret: t.ink,
            selection: t.selection,
            focused: self.focused_now,
            min_lines: if self.multiline { 2 } else { 1 },
            tab_spaces: 4,
            fit: None,
        }
    }
}

crate::impl_editable_input_handler!(TextField);

impl EventEmitter<TextFieldEvent> for TextField {}

impl Focusable for TextField {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TextField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_from_text();
        if self.blur_sub.is_none() {
            self.blur_sub = Some(cx.on_blur(&self.focus, window, |_, _, cx| cx.emit(TextFieldEvent::Blur)));
        }
        let t = cx.theme().clone();
        let focused = self.focus.is_focused(window);
        self.focused_now = focused;
        let rule = if focused { px(1.5) } else { px(1.) };
        let colour = if focused { t.blue } else { t.slate };
        let root = div()
            .key_context("NtexEdit TextField")
            .track_focus(&self.focus)
            .cursor(gpui::CursorStyle::IBeam)
            .flex()
            .flex_col()
            .min_h(px(24.))
            .when_some(self.width, |d, w| d.w(w).flex_none())
            .when(self.width.is_none(), |d| d.w_full())
            .pb(px(2.) - (rule - px(1.)))
            .border_b(rule)
            .border_color(colour);
        let root = edit::listen(root, cx);
        root.child(EditArea::new(cx.entity()))
    }
}

/// The shared editing keys ("NtexEdit"); the text field has no keys of its own.
pub fn bind_keys(cx: &mut App) {
    edit::bind_keys(cx);
}
