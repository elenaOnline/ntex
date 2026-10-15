//! The trigger capture: what she types is the trigger, literally — every
//! printable key including space, backslash, punctuation, digits; no key is
//! a shortcut here except: Tab toggles "on Tab" (a ⇥ keycap appears after the
//! trigger), Backspace / ⌥⌫ / ⌘⌫ delete, ←/→ move, Return submits (focus moves
//! to the output), Esc cancels, ⌘-anything goes to the app. Big LM Mono text
//! on a fill-in rule, like `\verb|ff|` set large.
//!
//! Key dispatch: a keystroke interceptor (it runs before any key binding)
//! takes every key without ⌘/⌃ while the field is focused, so no app binding
//! can steal a plain key or Tab. ⌘-chords go through normal dispatch: the
//! field's context binds only the editing ones (⌘⌫ ⌘A ⌘C ⌘X ⌘V ⌘Z ⌘⇧Z ⌘←/→);
//! ⌘↵, ⌘1, ⌘N, … reach the app.
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, Keystroke, ParentElement, Render, SharedString,
    Subscription, Window, div, prelude::*, px,
};

use super::controls::Keycap;
use super::edit::{self, Buffer, EditArea, Editable, Look, Op, Outcome, TextLayout};
use super::theme::ActiveTheme as _;

pub enum TriggerEvent {
    /// The text or the Tab flag changed.
    Changed,
    Submit,
    Cancel,
}

pub struct TriggerField {
    pub focus: FocusHandle,
    pub trigger: String,
    /// Tab pressed in the field: the snippet expands on Tab.
    pub on_tab: bool,
    /// What the field is capturing (changes the placeholder and the hint):
    /// "trigger" | "pattern" | "key" (visual: one key).
    pub mode: &'static str,
    // ---- added by WIDGETS
    buffer: Buffer,
    synced: String,
    layout: Option<Rc<TextLayout>>,
    selecting: bool,
    focused_now: bool,
    _intercept: Option<Subscription>,
}

impl TriggerField {
    pub fn new(cx: &mut Context<Self>) -> TriggerField {
        let this = cx.weak_entity();
        let intercept = cx.intercept_keystrokes(move |e, window, cx| {
            let Some(this) = this.upgrade() else { return };
            let handled = this.update(cx, |f, cx| {
                if !f.focus.is_focused(window) {
                    return false;
                }
                f.key(&e.keystroke, window, cx)
            });
            if handled {
                cx.stop_propagation();
            }
        });
        TriggerField {
            focus: cx.focus_handle(),
            trigger: String::new(),
            on_tab: false,
            mode: "trigger",
            buffer: Buffer::default(),
            synced: String::new(),
            layout: None,
            selecting: false,
            focused_now: false,
            _intercept: Some(intercept),
        }
    }
    pub fn set(&mut self, trigger: impl Into<String>, on_tab: bool, cx: &mut Context<Self>) {
        self.trigger = trigger.into();
        self.on_tab = on_tab;
        self.sync_from_text();
        cx.notify();
    }
    pub fn text(&self) -> &str {
        &self.trigger
    }
    pub fn set_mode(&mut self, mode: &'static str, cx: &mut Context<Self>) {
        self.mode = mode;
        cx.notify();
    }
    pub fn selection(&self) -> Range<usize> {
        self.buffer.sel.clone()
    }
    pub fn can_undo(&self) -> bool {
        self.buffer.can_undo()
    }
    /// The footnote hint for the focused field.
    pub fn hint(&self) -> &'static str {
        match self.mode {
            "pattern" => "Type a Lua pattern; captures become %1, %2. Tab makes it expand on Tab.",
            "key" => "Press the key you will type over a selection.",
            _ => "Type the trigger exactly as you will in ntex. Tab makes it expand on Tab.",
        }
    }

    fn sync_from_text(&mut self) {
        if self.trigger != self.synced {
            self.buffer.reset(edit::items_from_str(&self.trigger));
            self.synced = self.trigger.clone();
        }
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.trigger = self.buffer.text();
        self.synced = self.trigger.clone();
        cx.emit(TriggerEvent::Changed);
        cx.notify();
    }

    fn insert_literal(&mut self, s: &str, cx: &mut Context<Self>) {
        let s: String = s.chars().filter(|c| !c.is_control()).collect();
        if s.is_empty() {
            return;
        }
        if self.mode == "key" {
            // one key: the latest replaces it
            let one: String = s.chars().last().into_iter().collect();
            self.buffer.set_items(edit::items_from_str(&one));
        } else {
            self.buffer.insert(edit::items_from_str(&s));
        }
        self.changed(cx);
    }

    /// The literal-capture key handling. Returns whether the key was taken.
    pub fn key(&mut self, ks: &Keystroke, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let m = ks.modifiers;
        if m.platform || m.control || m.function {
            return false; // ⌘-chords: normal dispatch (editing bindings or the app)
        }
        let op = |o| Some(o);
        let action = match (ks.key.as_str(), m.shift, m.alt) {
            ("tab", false, _) => {
                self.on_tab = !self.on_tab;
                cx.emit(TriggerEvent::Changed);
                cx.notify();
                return true;
            }
            ("tab", true, _) => return false, // ⇧Tab: the page's focus order
            ("enter", _, _) | ("down", false, false) => op(Op::Newline),
            ("escape", _, _) => op(Op::Cancel),
            ("backspace", _, false) => op(Op::Backspace),
            ("backspace", _, true) => op(Op::DeleteWordLeft),
            ("delete", _, false) => op(Op::Delete),
            ("delete", _, true) => op(Op::DeleteWordRight),
            ("left", s, false) => op(Op::Left(s)),
            ("right", s, false) => op(Op::Right(s)),
            ("left", s, true) => op(Op::WordLeft(s)),
            ("right", s, true) => op(Op::WordRight(s)),
            ("up", s, _) | ("home", s, _) => op(Op::LineStart(s)),
            ("down", true, _) | ("end", true, _) => op(Op::LineEnd(true)),
            ("end", false, _) => op(Op::LineEnd(false)),
            _ => None,
        };
        if let Some(o) = action {
            self.op(o, window, cx);
            return true;
        }
        // a printable key: literally what it types
        let typed = ks.key_char.clone().or_else(|| match ks.key.as_str() {
            "space" => Some(" ".into()),
            k if k.chars().count() == 1 => Some(if m.shift { k.to_uppercase() } else { k.to_string() }),
            _ => None,
        });
        match typed {
            Some(s) if !s.is_empty() && s.chars().all(|c| !c.is_control()) => {
                self.insert_literal(&s, cx);
                true
            }
            // dead keys and anything unknown: the IME / platform
            _ => false,
        }
    }
}

impl Editable for TriggerField {
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
        self.changed(cx);
    }
    fn input_text(&mut self, range_utf16: Option<Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        // IME commits (dead keys, compositions): still literal, one line
        let text: String = text.chars().filter(|c| !c.is_control()).collect();
        if self.mode == "key" {
            self.insert_literal(&text, cx);
            return;
        }
        let r = range_utf16.map(|r| self.buffer.range_from_utf16(&r));
        self.buffer.ime_replace(r, &text);
        self.changed(cx);
    }
    fn op(&mut self, op: Op, _window: &mut Window, cx: &mut Context<Self>) {
        match op {
            Op::Copy => {
                edit::copy_plain(&self.buffer, cx);
            }
            Op::Cut => {
                if edit::copy_plain(&self.buffer, cx) {
                    self.buffer.delete_to(self.buffer.caret());
                    self.changed(cx);
                }
            }
            Op::Paste => {
                if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    let line = text.lines().next().unwrap_or("").to_string();
                    self.insert_literal(&line, cx);
                }
            }
            // Return / ↓
            Op::Newline => cx.emit(TriggerEvent::Submit),
            // ⌘↵ is the page's (save): let the app's binding have it
            Op::Submit => cx.propagate(),
            Op::Cancel => cx.emit(TriggerEvent::Cancel),
            // ⌘⌫ deletes everything (one line)
            Op::DeleteLineLeft => {
                if !self.buffer.is_empty() {
                    self.buffer.select_all();
                    self.buffer.delete_to(0);
                    self.changed(cx);
                }
            }
            other => match edit::apply_op(&mut self.buffer, other) {
                Outcome::Edited => self.changed(cx),
                Outcome::Moved => cx.notify(),
                Outcome::None => {}
            },
        }
    }
    fn look(&self, cx: &App) -> Look {
        let t = cx.theme();
        let size = t.trigger_size;
        Look {
            font: t.mono(),
            size,
            line_height: size * 1.3,
            color: t.ink,
            placeholder: match self.mode {
                "pattern" => "(%a)hat".into(),
                "key" => "a key".into(),
                _ => "trigger".into(),
            },
            placeholder_font: t.mono_italic(),
            placeholder_color: gpui::Hsla { a: 0.55, ..t.slate },
            caret: t.ink,
            selection: t.selection,
            focused: self.focused_now,
            min_lines: 1,
            tab_spaces: 4,
            fit: Some(px(12.)),
        }
    }
}

crate::impl_editable_input_handler!(TriggerField);

impl EventEmitter<TriggerEvent> for TriggerField {}

impl Focusable for TriggerField {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TriggerField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_from_text();
        let t = cx.theme().clone();
        let focused = self.focus.is_focused(window);
        self.focused_now = focused;
        let rule = if focused { px(1.5) } else { px(1.) };
        let colour = if focused { t.blue } else { t.slate };
        let tab: Option<SharedString> = self.on_tab.then(|| "⇥".into());
        let root = div()
            .key_context("NtexEdit TriggerField")
            .track_focus(&self.focus)
            .cursor(gpui::CursorStyle::IBeam)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .min_w(px(280.))
            .pb(px(3.) - (rule - px(1.)))
            .border_b(rule)
            .border_color(colour);
        edit::listen(root, cx)
            .child(div().flex_none().child(EditArea::new(cx.entity())))
            .when_some(tab, |d, k| d.child(Keycap::new(k).size(px(18.))))
    }
}

/// The trigger's keys go through the interceptor; its ⌘-chords use the
/// shared "NtexEdit" bindings.
pub fn bind_keys(cx: &mut App) {
    let _ = cx;
}
