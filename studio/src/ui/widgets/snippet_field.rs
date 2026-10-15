//! Text-mode LaTeX label field: the shared native editing core, backed by
//! one scratch tex buffer in the real hidden ntex. Every receiver is polled;
//! printable input echoes immediately, including while the bridge starts.
use ntex_studio_core::model::Paths;
use ntex_studio_io::bridge::{Bridge, FieldState};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::{
    Arc,
    mpsc::{Receiver, TryRecvError},
};
use std::time::{Duration, Instant};

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, ParentElement, Pixels, Render,
    SharedString, Subscription, Window, div, prelude::*, px,
};

use super::edit::{self, Buffer, EditArea, Editable, Look, Op, Outcome, TextLayout};
use super::theme::ActiveTheme as _;

pub enum SnippetFieldEvent {
    Changed,
    /// Return (single-line) / ⌘↵.
    Submit,
    /// Esc.
    Cancel,
    Blur,
    /// Tab outside a live snippet: walk the page's fields.
    Walk(i32),
}

pub struct SnippetField {
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
    session: Option<Session>,
    id: Option<u64>,
    opening: Option<Receiver<Result<(u64, FieldState), String>>>,
    pending: Option<Receiver<Result<FieldState, String>>>,
    queue: VecDeque<Command>,
    projected_selection: Range<usize>,
    native_selection: Option<Range<usize>>,
    pub in_snippet: bool,
    pub error: Option<String>,
    retry_at: Instant,
    poll: Option<gpui::Task<()>>,
    /// (figure-qol C) Maths mode: the field shows what is between the `$`s;
    /// the scratch buffer in ntex holds `$…$` (so maths snippets fire), and
    /// the cursor is mapped across the hidden delimiters.
    math: bool,
    keep_face: bool,
}

impl SnippetField {
    pub fn new(cx: &mut Context<Self>) -> SnippetField {
        SnippetField {
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
            session: None,
            id: None,
            opening: None,
            pending: None,
            queue: VecDeque::new(),
            projected_selection: 0..0,
            native_selection: None,
            in_snippet: false,
            error: None,
            retry_at: Instant::now(),
            poll: None,
            math: false,
            keep_face: false,
        }
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.text = text.into();
        self.sync_from_text();
        self.projected_selection = self.buffer.sel.clone();
        self.in_snippet = false;
        self.native_selection = None;
        if self.id.is_some() || self.opening.is_some() {
            self.queue
                .push_back(Command::Set(self.text.clone(), self.cursor_byte()));
        }
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
        self.native_selection = Some(self.buffer.sel.clone());
        cx.notify();
    }
    /// Can ⌘Z undo something in this field?
    pub fn can_undo(&self) -> bool {
        self.buffer.can_undo()
    }

    /// Maths mode (the delimiters hidden; ntex still sees them).
    pub fn math(&self) -> bool {
        self.math
    }
    pub fn math_mode(mut self) -> Self {
        self.math = true;
        self
    }
    /// Switch between maths and text without touching what is typed.
    pub fn set_math(&mut self, math: bool, cx: &mut Context<Self>) {
        if self.math == math {
            return;
        }
        self.math = math;
        self.in_snippet = false;
        self.native_selection = None;
        if self.id.is_some() || self.opening.is_some() {
            self.queue
                .push_back(Command::Set(self.buffer.text(), self.cursor_byte()));
        }
        cx.notify();
    }
    /// Show a proportional face (the in-place label field) instead of mono.
    pub fn proportional(mut self) -> Self {
        self.mono = false;
        self.keep_face = true;
        self
    }
    /// What the scratch buffer holds for the visible `text`.
    fn wrap(&self, text: &str) -> String {
        if self.math { format!("${text}$") } else { text.to_string() }
    }
    /// Bytes before the visible text in the scratch buffer.
    fn lead(&self) -> usize {
        usize::from(self.math)
    }

    fn sync_from_text(&mut self) {
        if self.text != self.synced {
            self.buffer.reset(edit::items_from_str(&self.text));
            self.synced = self.text.clone();
        }
    }

    fn clean(&self, s: &str) -> String {
        if self.multiline {
            s.to_string()
        } else {
            s.replace(['\n', '\r', '\t'], " ")
        }
    }
}

impl Editable for SnippetField {
    fn buffer(&self) -> &Buffer {
        &self.buffer
    }
    fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }
    fn focus_handle_ref(&self) -> &FocusHandle {
        &self.focus
    }
    fn underlines(&self) -> Vec<Range<usize>> {
        if self.session.is_some() && !self.math {
            math_ranges(&self.text)
        } else {
            Vec::new()
        }
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
        self.projected_selection = self.buffer.sel.clone();
        self.native_selection = None;
        cx.emit(SnippetFieldEvent::Changed);
        cx.notify();
    }
    fn composition_changed(&mut self, cx: &mut Context<Self>) {
        if self.live() {
            self.queue
                .push_back(Command::Set(self.buffer.text(), self.cursor_byte()));
        }
        self.after_edit(cx);
        self.drive(cx);
    }
    fn input_text(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut text = self.clean(text);
        if self.math {
            // the delimiters are implicit; a pasted `$x$` is just `x`
            text = text.replace('$', "");
        }
        let programmatic = range_utf16.is_some() || self.buffer.marked.is_some();
        if let Some(r) = range_utf16 {
            self.buffer.select(self.buffer.range_from_utf16(&r));
        }
        self.prepare_selection();
        self.buffer.ime_replace(None, &text);
        if self.live() {
            if programmatic {
                self.queue
                    .push_back(Command::Set(self.buffer.text(), self.cursor_byte()));
            } else {
                self.queue
                    .push_back(Command::Keys(text.replace('<', "<LT>"), Echo::Insert(text)));
            }
        }
        self.after_edit(cx);
        self.drive(cx);
    }
    fn op(&mut self, op: Op, _window: &mut Window, cx: &mut Context<Self>) {
        if matches!(op, Op::Newline | Op::Submit | Op::Cancel) {
            self.finish(
                if op == Op::Cancel {
                    Finish::Cancel
                } else {
                    Finish::Submit
                },
                cx,
            );
            return;
        }
        if op == Op::Copy {
            edit::copy_plain(&self.buffer, cx);
            return;
        }
        // maths: the hidden `$`s can't be deleted or stepped onto
        if self.math && self.buffer.sel.is_empty() {
            let (caret, len) = (self.buffer.caret(), self.buffer.text().chars().count());
            if (caret == 0 && matches!(op, Op::Backspace | Op::DeleteWordLeft | Op::Left(false)))
                || (caret >= len && matches!(op, Op::Delete | Op::DeleteWordRight | Op::Right(false)))
            {
                return;
            }
        }
        // Native selection, clipboard, undo and IME use the shared editing
        // core. Paste/programmatic replacement deliberately does not fire snippets.
        let motion = matches!(
            op,
            Op::Left(_)
                | Op::Right(_)
                | Op::Up(_)
                | Op::Down(_)
                | Op::WordLeft(_)
                | Op::WordRight(_)
                | Op::LineStart(_)
                | Op::LineEnd(_)
                | Op::DocStart(_)
                | Op::DocEnd(_)
        );
        if (motion
            && (self.native_selection.is_some() || self.buffer.sel != self.projected_selection))
            || matches!(
                op,
                Op::SelectAll
                    | Op::Left(true)
                    | Op::Right(true)
                    | Op::Up(true)
                    | Op::Down(true)
                    | Op::WordLeft(_)
                    | Op::WordRight(_)
                    | Op::LineStart(true)
                    | Op::LineEnd(true)
                    | Op::DocStart(true)
                    | Op::DocEnd(true)
            )
        {
            edit::apply_op(&mut self.buffer, op);
            self.native_selection = Some(self.buffer.sel.clone());
            cx.notify();
            return;
        }
        let native_override =
            self.native_selection.is_some() || self.buffer.sel != self.projected_selection;
        let before = self.buffer.text();
        let outcome = match op {
            Op::Cut => {
                if edit::copy_plain(&self.buffer, cx) {
                    self.buffer.delete_to(self.buffer.caret());
                }
                Outcome::Edited
            }
            Op::Paste => {
                if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    let text = if self.math { text.replace('$', "") } else { text };
                    self.buffer.insert(edit::items_from_str(&self.clean(&text)));
                }
                Outcome::Edited
            }
            other => edit::apply_op(&mut self.buffer, other),
        };
        if self.live() {
            // (maths: Home/End would land outside the hidden `$`s: set the cursor)
            let line_edge = matches!(op, Op::LineStart(_) | Op::LineEnd(_) | Op::DocStart(_) | Op::DocEnd(_));
            if let Some(keys) = notation(op).filter(|_| !native_override && !(self.math && line_edge)) {
                self.queue
                    .push_back(Command::Keys(keys.into(), Echo::Op(op)));
            } else {
                self.queue
                    .push_back(Command::Set(self.buffer.text(), self.cursor_byte()));
            }
        }
        if self.buffer.text() != before {
            self.after_edit(cx);
        } else if outcome != Outcome::None {
            self.projected_selection = self.buffer.sel.clone();
            cx.notify();
        }
        self.drive(cx);
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
            placeholder_font: if self.mono {
                t.mono_italic()
            } else {
                t.serif_italic()
            },
            placeholder_color: t.slate,
            caret: t.blue,
            selection: t.selection,
            focused: self.focused_now,
            min_lines: if self.multiline { 2 } else { 1 },
            tab_spaces: 4,
            fit: None,
        }
    }
}

crate::impl_editable_input_handler!(SnippetField);

impl EventEmitter<SnippetFieldEvent> for SnippetField {}

impl Focusable for SnippetField {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for SnippetField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_from_text();
        if self.session.is_some() && self.poll.is_none() {
            self.poll = Some(cx.spawn_in(window, async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(2))
                        .await;
                    if this
                        .update_in(cx, |v, window, cx| v.pump(window, cx))
                        .is_err()
                    {
                        break;
                    }
                }
            }));
        }
        self.pump(window, cx);
        if self.blur_sub.is_none() {
            self.blur_sub =
                Some(cx.on_blur(&self.focus, window, |v, _, cx| v.finish(Finish::Blur, cx)));
        }
        let t = cx.theme().clone();
        let focused = self.focus.is_focused(window);
        self.focused_now = focused;
        let rule = if focused { px(1.5) } else { px(1.) };
        let colour = if focused { t.blue } else { t.slate };
        let root = div()
            .key_context(if self.session.is_some() {
                "NtexEdit SnippetField"
            } else {
                "NtexEdit TextField"
            })
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
        let root = edit::listen(root, cx)
            .on_action(cx.listener(|v, _: &SnippetTab, _, cx| v.finish(Finish::Walk(1), cx)))
            .on_action(cx.listener(|v, _: &SnippetBackTab, _, cx| v.finish(Finish::Walk(-1), cx)));
        root.child(EditArea::new(cx.entity()))
    }
}

/// The shared editing keys ("NtexEdit"); the text field has no keys of its own.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        gpui::KeyBinding::new("tab", SnippetTab, Some("SnippetField")),
        gpui::KeyBinding::new("shift-tab", SnippetBackTab, Some("SnippetField")),
    ]);
}

/// One lazy child for all label fields in a figure window. Its final owner
/// drops Bridge, which reaps both nvim processes and removes scratch files.
#[derive(Clone)]
pub struct Session(Rc<RefCell<SessionState>>, Paths);
#[derive(Default)]
struct SessionState {
    bridge: Option<Arc<Bridge>>,
    starting: Option<Receiver<std::io::Result<Bridge>>>,
    retry_after: Option<Instant>,
}
impl Drop for SessionState {
    fn drop(&mut self) {
        // Bridge::drop reaps child processes. Keep process waits off the UI
        // thread, including a startup result already queued in its receiver.
        let bridge = self.bridge.take();
        let starting = self.starting.take();
        if bridge.is_some() || starting.is_some() {
            std::thread::spawn(move || {
                drop(starting);
                drop(bridge);
            });
        }
    }
}
impl Session {
    pub fn new(paths: Paths) -> Self {
        Self(Rc::new(RefCell::new(SessionState::default())), paths)
    }
    /// Child ids for lifecycle checks; does not launch or wait for the bridge.
    pub fn pids(&self) -> Vec<u32> {
        self.bridge().map(|b| b.pids()).unwrap_or_default()
    }
    fn bridge(&self) -> Option<Arc<Bridge>> {
        let mut state = self.0.borrow_mut();
        if let Some(rx) = &state.starting {
            match rx.try_recv() {
                Ok(result) => {
                    state.starting = None;
                    state.bridge = result.ok().map(Arc::new);
                    if state.bridge.is_none() {
                        state.retry_after = Some(Instant::now() + Duration::from_secs(1));
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    state.starting = None;
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        state.bridge.clone()
    }
    fn start(&self) {
        if self.bridge().is_none()
            && self.0.borrow().starting.is_none()
            && self
                .0
                .borrow()
                .retry_after
                .is_none_or(|at| Instant::now() >= at)
        {
            let paths = self.1.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            self.0.borrow_mut().starting = Some(rx);
            std::thread::spawn(move || {
                let _ = tx.send(Bridge::start(&paths));
            });
        }
    }
}
#[derive(Clone, Copy)]
enum Finish {
    Submit,
    Cancel,
    Blur,
    Walk(i32),
}
#[derive(Clone)]
enum Echo {
    Insert(String),
    Op(Op),
}
#[derive(Clone)]
enum Command {
    Keys(String, Echo),
    Set(String, usize),
    Finish(Finish),
}
gpui::actions!(snippet_field, [SnippetTab, SnippetBackTab]);

impl SnippetField {
    pub fn with_session(mut self, session: Session) -> Self {
        self.session = Some(session);
        self.mono = !self.keep_face;
        self
    }
    pub fn ready(&self) -> bool {
        self.id.is_some() && self.error.is_none()
    }
    pub fn settled(&self) -> bool {
        self.queue.is_empty() && self.pending.is_none() && self.opening.is_none()
    }
    pub fn hint(&self) -> &'static str {
        match (self.ready(), self.error.is_some(), self.math) {
            (true, _, true) => "maths · your snippets work here · ↵ sets the label · tab goes to the next label · esc cancels",
            (true, _, false) => "text · mk for maths inside it · ↵ sets the label · tab goes to the next label · esc cancels",
            (false, true, _) => "Plain typing; ntex is unavailable · retrying snippets · ↵ sets the label",
            (false, false, true) => "maths · snippets are starting, plain typing for now · ↵ sets the label",
            (false, false, false) => "Plain typing for now; snippets are starting · mk for maths when ntex is ready · ↵ sets the label",
        }
    }
    fn live(&self) -> bool {
        self.id.is_some()
    }
    fn cursor_byte(&self) -> usize {
        self.buffer
            .text()
            .chars()
            .take(self.buffer.caret())
            .map(char::len_utf8)
            .sum()
    }
    // Mouse/native selection belongs to the editing core. Synchronize it
    // before forwarding the next key; ntex's own selected tab stop stays live.
    fn prepare_selection(&mut self) {
        if self.buffer.sel != self.projected_selection && self.live() {
            let mut b = self.buffer.clone();
            if !b.sel.is_empty() {
                b.delete_to(b.caret());
            }
            let cursor = b.text().chars().take(b.caret()).map(char::len_utf8).sum();
            self.queue.push_back(Command::Set(b.text(), cursor));
        }
    }
    fn finish(&mut self, finish: Finish, cx: &mut Context<Self>) {
        self.queue.push_back(Command::Finish(finish));
        self.drive(cx);
    }
    fn emit_finish(&mut self, finish: Finish, cx: &mut Context<Self>) {
        cx.emit(match finish {
            Finish::Submit => SnippetFieldEvent::Submit,
            Finish::Cancel => SnippetFieldEvent::Cancel,
            Finish::Blur => SnippetFieldEvent::Blur,
            Finish::Walk(dir) => SnippetFieldEvent::Walk(dir),
        });
    }
    fn drive(&mut self, cx: &mut Context<Self>) {
        if self.pending.is_some() || self.opening.is_some() {
            return;
        }
        let bridge = self.session.as_ref().and_then(Session::bridge);
        while let Some(cmd) = self.queue.pop_front() {
            match cmd {
                Command::Finish(Finish::Walk(dir)) if self.in_snippet && self.id.is_some() => {
                    self.pending =
                        Some(bridge.as_ref().unwrap().field_keys(
                            self.id.unwrap(),
                            if dir < 0 { "<S-Tab>" } else { "<Tab>" },
                        ));
                    break;
                }
                Command::Finish(Finish::Cancel) if self.in_snippet && self.id.is_some() => {
                    self.pending = Some(
                        bridge
                            .as_ref()
                            .unwrap()
                            .field_keys(self.id.unwrap(), "<Esc>"),
                    );
                    break;
                }
                Command::Finish(finish) => {
                    self.emit_finish(finish, cx);
                    break;
                }
                Command::Keys(keys, _) if self.id.is_some() => {
                    self.pending =
                        Some(bridge.as_ref().unwrap().field_keys(self.id.unwrap(), &keys));
                    break;
                }
                Command::Set(text, cursor) if self.id.is_some() => {
                    self.pending = Some(bridge.as_ref().unwrap().field_set(
                        self.id.unwrap(),
                        &self.wrap(&text),
                        cursor + self.lead(),
                    ));
                    break;
                }
                _ => {}
            }
        }
    }
    pub fn pump(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.session.is_some()
            && self.focus.is_focused(window)
            && self.id.is_none()
            && self.opening.is_none()
            && Instant::now() >= self.retry_at
        {
            let session = self.session.as_ref().unwrap();
            session.start();
            if let Some(b) = session.bridge().filter(|b| b.is_ready()) {
                self.opening = Some(b.field_open(&self.wrap(&self.text), self.cursor_byte() + self.lead()));
            }
        }
        if let Some(rx) = &self.opening {
            match rx.try_recv() {
                Ok(Ok((id, state))) => {
                    self.opening = None;
                    self.id = Some(id);
                    self.error = None;
                    // Fallback typing/IME may have continued while opening.
                    if self.wrap(&self.text) != state.text || self.cursor_byte() + self.lead() != state.cursor {
                        self.queue
                            .push_front(Command::Set(self.text.clone(), self.cursor_byte()));
                    }
                    cx.notify();
                }
                Ok(Err(e)) => {
                    self.opening = None;
                    self.failed(e, cx);
                }
                Err(TryRecvError::Disconnected) => {
                    self.opening = None;
                    self.failed("ntex disconnected".into(), cx);
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok(Ok(state)) => {
                    self.pending = None;
                    self.reconcile(state, cx);
                }
                Ok(Err(e)) => {
                    self.pending = None;
                    self.failed(e, cx);
                }
                Err(TryRecvError::Disconnected) => {
                    self.pending = None;
                    self.failed("ntex disconnected".into(), cx);
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        self.drive(cx);
    }
    fn failed(&mut self, error: String, cx: &mut Context<Self>) {
        if let Some(id) = self.id.take() {
            if let Some(b) = self.session.as_ref().and_then(Session::bridge) {
                b.field_close(id);
            }
        }
        self.in_snippet = false;
        self.error = Some(error);
        self.retry_at = Instant::now() + Duration::from_secs(1);
        self.queue.retain(|cmd| matches!(cmd, Command::Finish(_)));
        cx.notify();
    }
    fn reconcile(&mut self, mut state: FieldState, cx: &mut Context<Self>) {
        let marked = self.buffer.marked.clone();
        if self.math {
            // back to what is visible: the text between the `$`s, the cursor
            // and the selected tab stop shifted past the hidden opening one
            let had_open = state.text.starts_with('$');
            let had_close = state.text.len() > usize::from(had_open) && state.text.ends_with('$');
            let end = state.text.len() - usize::from(had_close);
            let inner = state.text[usize::from(had_open)..end].to_string();
            let shift = |b: usize| b.saturating_sub(usize::from(had_open)).min(inner.len());
            state.cursor = shift(state.cursor);
            state.selected = state.selected.map(|(a, b)| (shift(a), shift(b)));
            if !(had_open && had_close) {
                // something removed a delimiter: put it back
                self.queue.push_front(Command::Set(inner.clone(), state.cursor));
            }
            state.text = inner;
        }
        let char_at = |byte: usize| {
            state
                .text
                .char_indices()
                .take_while(|(i, _)| *i < byte)
                .count()
        };
        self.buffer.set_items(edit::items_from_str(&state.text));
        self.buffer.move_to(char_at(state.cursor));
        if let Some((a, b)) = state.selected {
            self.buffer.select(char_at(a)..char_at(b));
        }
        self.in_snippet = state.in_snippet;
        // Replay keys still in flight over authoritative snippet expansion;
        // an older answer must never erase the latest optimistic characters.
        for cmd in &self.queue {
            match cmd {
                Command::Keys(_, Echo::Insert(text)) => {
                    self.buffer.insert(edit::items_from_str(text));
                }
                Command::Keys(_, Echo::Op(op)) => {
                    edit::apply_op(&mut self.buffer, *op);
                }
                Command::Set(text, cursor) => {
                    self.buffer.set_items(edit::items_from_str(text));
                    self.buffer
                        .move_to(text.char_indices().take_while(|(i, _)| i < cursor).count());
                }
                _ => {}
            }
        }
        let native = self.native_selection.clone();
        if let Some(range) = &native {
            self.buffer.select(range.clone());
        }
        self.after_edit(cx);
        self.buffer.marked = marked;
        // A native selection is an overlay until the next edit consumes it.
        if let Some(range) = native {
            self.native_selection = Some(range);
            self.projected_selection = usize::MAX..usize::MAX;
        }
    }
}
impl Drop for SnippetField {
    fn drop(&mut self) {
        let bridge = self.session.as_ref().and_then(Session::bridge);
        if let (Some(b), Some(id)) = (&bridge, self.id) {
            b.field_close(id);
        }
        if let (Some(b), Some(rx)) = (bridge, self.opening.take()) {
            // An open must be closed even if its widget disappears before
            // the answer. Bridge is kept alive only until this reply arrives.
            std::thread::spawn(move || {
                if let Ok(Ok((id, _))) = rx.recv() {
                    b.field_close(id);
                }
            });
        }
    }
}
fn notation(op: Op) -> Option<&'static str> {
    Some(match op {
        Op::Left(false) => "<Left>",
        Op::Right(false) => "<Right>",
        Op::Backspace => "<BS>",
        Op::Delete => "<Del>",
        Op::DeleteWordLeft => "<M-BS>",
        Op::DeleteWordRight => "<C-Del>",
        Op::LineStart(false) | Op::DocStart(false) => "<Home>",
        Op::LineEnd(false) | Op::DocEnd(false) => "<End>",
        _ => return None,
    })
}

/// Character offsets, matching the shared editor's layout. Escaped dollars
/// are literal text; a not-yet-closed maths span is also marked while typing.
fn math_ranges(text: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = None;
    let mut escaped = false;
    for (i, c) in text.chars().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
        } else if c == '$' {
            if let Some(a) = start.take() {
                ranges.push(a..i + 1);
            } else {
                start = Some(i);
            }
        }
    }
    if let Some(a) = start {
        ranges.push(a..text.chars().count());
    }
    ranges
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maths_underlines_use_character_offsets_and_ignore_escaped_dollars() {
        assert_eq!(math_ranges("φ $ℤ$"), vec![2..5]);
        assert_eq!(math_ranges(r"\$5 $x$ and $y"), vec![4..7, 12..14]);
    }
}
