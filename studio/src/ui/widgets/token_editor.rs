//! The output editor: multi-line LaTeX (LM Mono) with tokens drawn as chips.
//! The text model is a Vec<Piece>; tokens are atomic (the caret steps over a
//! chip; Backspace deletes a whole chip). UX.md "Output editor" has the keys:
//! Tab = new tab stop (selected text becomes its default), ⇧Tab = mirror
//! (again: cycle which tab stop), ⌘E = exit point, ⌘⇧V = the selection,
//! ⌥1…⌥9 = captures, ⌘↵ submit, Esc leaves the field. Pasting helper /
//! UltiSnips / VS Code snippet text turns into tokens (codegen::parse_body).
//! Undo/redo, IME, ⌥/⌘ motions, clipboard (copies helper syntax).
//!
//! Double-click a tab stop to edit its default in place (type, ⌫; Return,
//! Esc, Tab or a click elsewhere ends it). Hovering a chip explains it.
//! Tab stops are renumbered 1, 2, 3 in reading order after every edit
//! (`codegen::renumber`).
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    App, Bounds, ClickEvent, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, KeyBinding,
    MouseMoveEvent, Pixels, Point, Render, RenderOnce, SharedString, Window, actions, anchored, deferred, div,
    point, prelude::*, px,
};

use super::controls::Button;
use super::edit::{self, Buffer, ChipLook, EditArea, Editable, Item, Look, Op, Outcome, TextLayout};
use super::theme::{ActiveTheme as _, Theme};
use crate::core::codegen;
use crate::model::{Kind, Piece};

pub enum TokenEditorEvent {
    Changed,
    Submit,
    Cancel,
}

/// The editor offers something the current kind doesn't allow (⌘⇧V outside
/// a visual snippet, ⌥1 outside post/pattern): the screen may switch kind.
/// A separate event type, so `TokenEditorEvent` matches stay exhaustive.
#[derive(Clone, Debug, PartialEq)]
pub enum TokenEditorOffer {
    SwitchKind(Kind),
}

/// Which token kinds the editor offers (the toolbar and the keys): depends on
/// the snippet kind (captures only for post/pattern, selection only for visual).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenSet {
    pub captures: u8,
    pub selection: bool,
}

impl TokenSet {
    pub const NONE: TokenSet = TokenSet { captures: 0, selection: false };
    /// What a kind allows: post → %1, pattern → %1…%9, visual → %V.
    pub fn for_kind(kind: Kind) -> TokenSet {
        match kind {
            Kind::Post => TokenSet { captures: 1, selection: false },
            Kind::Pattern => TokenSet { captures: 9, selection: false },
            Kind::Visual => TokenSet { captures: 0, selection: true },
            _ => TokenSet::NONE,
        }
    }
}

actions!(
    ntex_token_editor,
    [
        InsertTabStop,
        InsertMirror,
        InsertExit,
        InsertSelection,
        Capture1,
        Capture2,
        Capture3,
        Capture4,
        Capture5,
        Capture6,
        Capture7,
        Capture8,
        Capture9
    ]
);

/// A tab stop's default being edited in place.
#[derive(Clone, Debug)]
struct ChipEdit {
    /// Item index of the tab stop's first occurrence.
    index: usize,
    text: String,
    /// The whole default is selected (typing replaces it).
    all: bool,
}

#[derive(Clone, Copy, Debug)]
struct MirrorCycle {
    index: usize,
    version: u64,
}

pub struct TokenEditor {
    pub focus: FocusHandle,
    pub pieces: Vec<Piece>,
    pub allowed: TokenSet,
    // ---- added by WIDGETS
    /// Capture chips read `letter` (post) rather than `%1` (pattern).
    pub post_capture: bool,
    /// Placeholder when empty.
    pub placeholder: SharedString,
    buffer: Buffer,
    synced: Vec<Piece>,
    layout: Option<Rc<TextLayout>>,
    selecting: bool,
    _typing: Option<gpui::Subscription>,
    focused_now: bool,
    cycle: Option<MirrorCycle>,
    chip_edit: Option<ChipEdit>,
    /// (item index, chip bounds, shown yet)
    hover: Option<(usize, Bounds<Pixels>, bool)>,
}

impl TokenEditor {
    pub fn new(cx: &mut Context<Self>) -> TokenEditor {
        TokenEditor {
            focus: cx.focus_handle(),
            pieces: Vec::new(),
            allowed: TokenSet { captures: 0, selection: false },
            post_capture: false,
            placeholder: "\\frac{…}{…}  (Tab for a tab stop)".into(),
            buffer: Buffer::default(),
            synced: Vec::new(),
            layout: None,
            selecting: false,
            _typing: Some(edit::intercept_typing(cx)),
            focused_now: false,
            cycle: None,
            chip_edit: None,
            hover: None,
        }
    }
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }
    pub fn set_pieces(&mut self, pieces: Vec<Piece>, cx: &mut Context<Self>) {
        self.pieces = pieces;
        self.sync_from_pieces();
        cx.notify();
    }
    pub fn set_allowed(&mut self, allowed: TokenSet, cx: &mut Context<Self>) {
        self.allowed = allowed;
        cx.notify();
    }
    /// `set_allowed(TokenSet::for_kind(kind))` plus the capture chips' label.
    pub fn set_kind(&mut self, kind: Kind, cx: &mut Context<Self>) {
        self.allowed = TokenSet::for_kind(kind);
        self.post_capture = kind == Kind::Post;
        cx.notify();
    }
    /// The toolbar's buttons call these (same as the keys).
    pub fn insert_tab_stop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_chip_edit(cx);
        let default = self.buffer.text_of(self.buffer.sel.clone());
        let n = self.max_tab() + 1;
        self.buffer.insert(vec![Item::Tok(Piece::Tab { n, default })]);
        self.buffer.break_undo();
        self.after_edit(cx);
        let _ = window;
    }
    pub fn insert_mirror(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = window;
        self.commit_chip_edit(cx);
        // pressed again right away: cycle the mirror through the tab stops
        if let Some(c) = self.cycle.filter(|c| c.version == self.buffer.version)
            && let Some(Item::Tok(Piece::Tab { n, .. })) = self.buffer.items.get(c.index).cloned()
        {
            let mut ns: Vec<u32> = self.tab_numbers();
            ns.sort_unstable();
            ns.dedup();
            if ns.len() > 1 {
                let pos = ns.iter().position(|x| *x == n).unwrap_or(0);
                let prev = if pos == 0 { ns[ns.len() - 1] } else { ns[pos - 1] };
                self.buffer.checkpoint();
                self.buffer.items[c.index] = Item::Tok(Piece::Tab { n: prev, default: String::new() });
                self.buffer.version += 1;
                self.after_edit(cx);
                self.cycle = Some(MirrorCycle { index: c.index, version: self.buffer.version });
            }
            return;
        }
        let caret = self.buffer.sel.start;
        let before = self.buffer.items[..caret].iter().rev().find_map(tab_n);
        let target = before.or_else(|| self.buffer.items[caret..].iter().find_map(tab_n));
        let Some(n) = target else { return };
        let at = self.buffer.insert(vec![Item::Tok(Piece::Tab { n, default: String::new() })]);
        self.buffer.break_undo();
        self.after_edit(cx);
        self.cycle = Some(MirrorCycle { index: at.start, version: self.buffer.version });
    }
    pub fn insert_exit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = window;
        self.commit_chip_edit(cx);
        // at most one: inserting moves it
        self.buffer.checkpoint();
        let mut sel = self.buffer.sel.clone();
        let mut i = 0;
        while i < self.buffer.items.len() {
            if matches!(self.buffer.items[i], Item::Tok(Piece::Exit)) && !(sel.start <= i && i < sel.end) {
                self.buffer.items.remove(i);
                if i < sel.start {
                    sel.start -= 1;
                    sel.end -= 1;
                }
            } else {
                i += 1;
            }
        }
        self.buffer.sel = sel.clone();
        self.buffer.items.splice(sel.clone(), [Item::Tok(Piece::Exit)]);
        self.buffer.move_to(sel.start + 1);
        self.buffer.version += 1;
        self.buffer.break_undo();
        self.after_edit(cx);
    }
    pub fn insert_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = window;
        if !self.allowed.selection {
            cx.emit(TokenEditorOffer::SwitchKind(Kind::Visual));
            return;
        }
        self.commit_chip_edit(cx);
        self.buffer.insert(vec![Item::Tok(Piece::Selection)]);
        self.buffer.break_undo();
        self.after_edit(cx);
    }
    pub fn insert_capture(&mut self, n: u8, window: &mut Window, cx: &mut Context<Self>) {
        let _ = window;
        if n == 0 || n > self.allowed.captures {
            cx.emit(TokenEditorOffer::SwitchKind(if n <= 1 { Kind::Post } else { Kind::Pattern }));
            return;
        }
        self.commit_chip_edit(cx);
        self.buffer.insert(vec![Item::Tok(Piece::Capture(n))]);
        self.buffer.break_undo();
        self.after_edit(cx);
    }

    // ---- added by WIDGETS
    /// The caret / selection, in positions (a chip is one).
    pub fn selection(&self) -> Range<usize> {
        self.buffer.sel.clone()
    }
    pub fn select(&mut self, r: Range<usize>, cx: &mut Context<Self>) {
        self.buffer.select(r);
        cx.notify();
    }
    pub fn can_undo(&self) -> bool {
        self.buffer.can_undo()
    }
    /// Is a tab stop's default being edited in place?
    pub fn editing_default(&self) -> bool {
        self.chip_edit.is_some()
    }
    /// The output as helper syntax (what ⌘C writes).
    pub fn helper_text(&self) -> String {
        codegen::body(&self.pieces)
    }
    /// The footnote hint while focused (only the allowed keys).
    pub fn hint(&self) -> String {
        let mut s = String::from("Tab inserts a tab stop · ⇧Tab a mirror · ⌘E the exit point");
        if self.allowed.selection {
            s.push_str(" · ⌘⇧V the selection");
        }
        if self.allowed.captures == 1 {
            s.push_str(" · ⌥1 the letter");
        } else if self.allowed.captures > 1 {
            s.push_str(" · ⌥1…⌥9 captures");
        }
        s.push('.');
        s
    }
    /// Where the chips were laid out last frame: (item index, window bounds).
    pub fn chip_rects(&self) -> Vec<(usize, Bounds<Pixels>)> {
        self.layout.as_ref().map(|l| l.chips.clone()).unwrap_or_default()
    }
    /// The tooltip on screen, if any.
    pub fn hovered_tooltip(&self) -> Option<String> {
        self.hover.filter(|h| h.2).and_then(|(i, _, _)| self.chip_tooltip(i))
    }
    /// What the tooltip says about the chip at item `i`.
    pub fn chip_tooltip(&self, i: usize) -> Option<String> {
        let Item::Tok(p) = self.buffer.items.get(i)? else { return None };
        Some(match p {
            Piece::Tab { n, .. } => {
                let count = self.buffer.items.iter().filter(|it| tab_n(it) == Some(*n)).count();
                let first = self.first_of(*n) == Some(i);
                let places = if count == 1 { "typed in 1 place".to_string() } else { format!("typed in {count} places") };
                if first {
                    format!("Tab stop {n} — {places}")
                } else {
                    format!("Mirror of tab stop {n} — {places}")
                }
            }
            Piece::Exit => "Exit point — where the last Tab leaves the cursor".into(),
            Piece::Selection => "The selection (%V) — the text selected before the key".into(),
            Piece::Capture(k) if self.post_capture => format!("Capture %{k} — the letter before the trigger"),
            Piece::Capture(k) => format!("Capture %{k} — what the pattern's group {k} matched"),
            Piece::Text(_) => return None,
        })
    }

    fn tab_numbers(&self) -> Vec<u32> {
        self.buffer.items.iter().filter_map(tab_n).collect()
    }
    fn max_tab(&self) -> u32 {
        self.tab_numbers().into_iter().max().unwrap_or(0)
    }
    fn first_of(&self, n: u32) -> Option<usize> {
        self.buffer.items.iter().position(|it| tab_n(it) == Some(n))
    }

    fn sync_from_pieces(&mut self) {
        if self.pieces != self.synced {
            self.buffer.reset(edit::items_from_pieces(&self.pieces));
            self.synced = self.pieces.clone();
            self.chip_edit = None;
            self.cycle = None;
        }
    }

    /// Renumber in reading order (same positions: renumber only relabels).
    fn renumber(&mut self) {
        let mut pieces = edit::pieces_from_items(&self.buffer.items);
        let before = pieces.clone();
        codegen::renumber(&mut pieces);
        if pieces != before {
            let items = edit::items_from_pieces(&pieces);
            debug_assert_eq!(items.len(), self.buffer.items.len());
            if items.len() == self.buffer.items.len() {
                self.buffer.items = items;
            }
        }
        self.pieces = pieces;
    }

    fn commit_chip_edit(&mut self, cx: &mut Context<Self>) {
        let Some(ce) = self.chip_edit.take() else { return };
        if let Some(Item::Tok(Piece::Tab { n, default })) = self.buffer.items.get(ce.index).cloned()
            && default != ce.text
        {
            self.buffer.checkpoint();
            self.buffer.items[ce.index] = Item::Tok(Piece::Tab { n, default: ce.text });
            self.buffer.version += 1;
            self.buffer.break_undo();
            self.after_edit(cx);
        }
        cx.notify();
    }

    fn chip_edit_op(&mut self, op: Op, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(ce) = self.chip_edit.as_mut() else { return false };
        match op {
            Op::Backspace | Op::Delete | Op::DeleteWordLeft | Op::DeleteLineLeft => {
                if ce.all || op == Op::DeleteLineLeft {
                    ce.text.clear();
                } else if op == Op::DeleteWordLeft {
                    let t = ce.text.trim_end_matches(|c: char| c.is_alphanumeric()).len();
                    ce.text.truncate(if t == ce.text.len() { ce.text.len().saturating_sub(1) } else { t });
                } else {
                    ce.text.pop();
                }
                ce.all = false;
                cx.notify();
                true
            }
            Op::SelectAll => {
                ce.all = true;
                cx.notify();
                true
            }
            Op::Undo | Op::Redo | Op::Copy | Op::Cut | Op::Paste => {
                self.commit_chip_edit(cx);
                false
            }
            Op::Newline | Op::Cancel | Op::Submit => {
                self.commit_chip_edit(cx);
                if op == Op::Submit {
                    cx.emit(TokenEditorEvent::Submit);
                }
                true
            }
            _ => {
                // a motion leaves the chip, caret after it
                let after = ce.index + 1;
                self.commit_chip_edit(cx);
                self.buffer.move_to(after);
                let _ = window;
                cx.notify();
                true
            }
        }
    }

    fn on_hover_move(&mut self, e: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(layout) = self.layout.clone() else { return };
        let hit = layout.chip_at(e.position).and_then(|i| layout.chips.iter().find(|(j, _)| *j == i).copied());
        match (hit, self.hover) {
            (Some((i, _)), Some((j, _, _))) if i == j => {}
            (Some((i, b)), _) => {
                self.hover = Some((i, b, false));
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_millis(450)).await;
                    this.update(cx, |ed, cx| {
                        if let Some((j, b, false)) = ed.hover
                            && j == i
                        {
                            ed.hover = Some((j, b, true));
                            cx.notify();
                        }
                    })
                    .ok();
                })
                .detach();
                cx.notify();
            }
            (None, Some(_)) => {
                self.hover = None;
                cx.notify();
            }
            (None, None) => {}
        }
    }

    /// Chip colours for a token.
    fn chip_look_for(&self, t: &Theme, i: usize, p: &Piece) -> ChipLook {
        let mono = t.mono();
        let base = |tones: super::theme::BoxTones, label: String| ChipLook {
            label: label.into(),
            default: None,
            border: tones.rule,
            fill: tones.back,
            text: tones.head,
            default_color: t.ink,
            dashed: false,
            label_font: Some(mono.clone()),
            label_size: t.chip_size,
            default_font: Some(mono.clone()),
            default_size: px(12.),
            editing: None,
        };
        match p {
            Piece::Tab { n, default } => {
                let first = self.first_of(*n) == Some(i);
                let mut cl = base(t.theorem, n.to_string());
                cl.dashed = !first;
                if first {
                    match self.chip_edit.as_ref().filter(|ce| ce.index == i) {
                        Some(ce) => {
                            cl.default = Some(ce.text.clone().into());
                            cl.editing = Some(ce.all);
                        }
                        None if !default.is_empty() => cl.default = Some(default.clone().into()),
                        None => {}
                    }
                }
                cl
            }
            Piece::Exit => base(t.remark, "end".into()),
            Piece::Selection => base(t.lemma, "sel".into()),
            Piece::Capture(k) => base(t.example, if self.post_capture { "letter".into() } else { format!("%{k}") }),
            Piece::Text(_) => base(t.proof, String::new()),
        }
    }
}

fn tab_n(it: &Item) -> Option<u32> {
    match it {
        Item::Tok(Piece::Tab { n, .. }) => Some(*n),
        _ => None,
    }
}

impl Buffer {
    /// Plain text of a range (tokens dropped).
    pub fn text_of(&self, r: Range<usize>) -> String {
        edit::plain_text(&self.items[r.start.min(self.len())..r.end.min(self.len())])
    }
}

impl Editable for TokenEditor {
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
        self.renumber();
        self.synced = self.pieces.clone();
        self.cycle = None;
        cx.emit(TokenEditorEvent::Changed);
        cx.notify();
    }
    fn input_text(&mut self, range_utf16: Option<Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(ce) = self.chip_edit.as_mut() {
            let clean: String = text.chars().filter(|c| !c.is_control()).collect();
            if ce.all {
                ce.text = clean;
            } else {
                ce.text.push_str(&clean);
            }
            ce.all = false;
            cx.notify();
            return;
        }
        let r = range_utf16.map(|r| self.buffer.range_from_utf16(&r));
        self.buffer.ime_replace(r, text);
        self.after_edit(cx);
    }
    fn double_click(&mut self, at: usize, p: Point<Pixels>, _: &mut Window, cx: &mut Context<Self>) {
        self.commit_chip_edit(cx);
        let chip = self.layout.as_ref().and_then(|l| l.chip_at(p));
        match chip.and_then(|i| self.buffer.items.get(i).cloned().map(|it| (i, it))) {
            Some((i, Item::Tok(Piece::Tab { n, .. }))) => {
                let first = self.first_of(n).unwrap_or(i);
                let text = match &self.buffer.items[first] {
                    Item::Tok(Piece::Tab { default, .. }) => default.clone(),
                    _ => String::new(),
                };
                self.chip_edit = Some(ChipEdit { index: first, text, all: true });
                self.buffer.move_to(first + 1);
            }
            Some((i, _)) => self.buffer.select(i..i + 1),
            None => {
                let r = self.buffer.word_at(at);
                self.buffer.select(r);
            }
        }
        cx.notify();
    }
    fn op(&mut self, op: Op, window: &mut Window, cx: &mut Context<Self>) {
        if self.chip_edit.is_some() && self.chip_edit_op(op, window, cx) {
            return;
        }
        match op {
            Op::Copy | Op::Cut => {
                let sel = self.buffer.sel.clone();
                if sel.is_empty() {
                    return;
                }
                let pieces = edit::pieces_from_items(&self.buffer.items[sel]);
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(codegen::body(&pieces)));
                if op == Op::Cut {
                    self.buffer.delete_to(self.buffer.caret());
                    self.after_edit(cx);
                }
            }
            Op::Paste => {
                let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) else { return };
                let mut pasted = codegen::parse_body(&text);
                // pasted tab stops are new ones, after the existing numbers
                let offset = self.max_tab();
                let has_exit = pasted.iter().any(|p| matches!(p, Piece::Exit));
                for p in &mut pasted {
                    if let Piece::Tab { n, .. } = p {
                        *n += offset;
                    }
                }
                let items = edit::items_from_pieces(&pasted);
                self.buffer.insert(items);
                if has_exit {
                    // keep only the pasted exit point
                    let keep = self.buffer.caret();
                    let sel_end = keep;
                    let start = keep - edit::items_from_pieces(&pasted).len();
                    let mut i = 0;
                    let mut caret = keep;
                    while i < self.buffer.items.len() {
                        let inside = i >= start && i < sel_end;
                        if !inside && matches!(self.buffer.items[i], Item::Tok(Piece::Exit)) {
                            self.buffer.items.remove(i);
                            if i < caret {
                                caret -= 1;
                            }
                        } else {
                            i += 1;
                        }
                    }
                    self.buffer.move_to(caret);
                }
                self.buffer.break_undo();
                self.after_edit(cx);
            }
            Op::Newline => {
                // auto-indent: copy the line's leading tabs and spaces
                let c = self.buffer.sel.start;
                let start = self.buffer.line_start(c);
                let mut new = vec![Item::Ch('\n')];
                new.extend(
                    self.buffer.items[start..c].iter().take_while(|it| matches!(it, Item::Ch(' ') | Item::Ch('\t'))).cloned(),
                );
                self.buffer.insert(new);
                self.after_edit(cx);
            }
            Op::Submit => cx.emit(TokenEditorEvent::Submit),
            Op::Cancel => cx.emit(TokenEditorEvent::Cancel),
            other => match edit::apply_op(&mut self.buffer, other) {
                Outcome::Edited => {
                    if matches!(other, Op::Undo | Op::Redo) {
                        self.pieces = edit::pieces_from_items(&self.buffer.items);
                        self.synced = self.pieces.clone();
                        self.cycle = None;
                        cx.emit(TokenEditorEvent::Changed);
                        cx.notify();
                    } else {
                        self.after_edit(cx);
                    }
                }
                Outcome::Moved => {
                    self.cycle = None;
                    cx.notify();
                }
                Outcome::None => {}
            },
        }
    }
    fn look(&self, cx: &App) -> Look {
        let t = cx.theme();
        let size = t.mono_size;
        Look {
            font: t.mono(),
            size,
            line_height: t.line(size).max(px(22.)),
            color: t.ink,
            placeholder: self.placeholder.clone(),
            placeholder_font: t.mono_italic(),
            placeholder_color: Theme::alpha(t.slate, 0.8),
            caret: t.ink,
            selection: t.selection,
            focused: self.focused_now && self.chip_edit.is_none(),
            min_lines: 2,
            tab_spaces: 4,
            fit: None,
        }
    }
    fn chip(&self, index: usize, piece: &Piece, cx: &App) -> ChipLook {
        self.chip_look_for(cx.theme(), index, piece)
    }
}

crate::impl_editable_input_handler!(TokenEditor);

impl EventEmitter<TokenEditorEvent> for TokenEditor {}
impl EventEmitter<TokenEditorOffer> for TokenEditor {}

impl Focusable for TokenEditor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TokenEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_from_pieces();
        let t = cx.theme().clone();
        let focused = self.focus.is_focused(window);
        if !focused && self.chip_edit.is_some() {
            self.commit_chip_edit(cx);
        }
        self.focused_now = focused;
        let rule = if focused { px(1.5) } else { px(1.) };
        let colour = if focused { t.blue } else { t.slate };
        let tooltip = self.hover.filter(|h| h.2).and_then(|(i, b, _)| self.chip_tooltip(i).map(|s| (s, b)));
        let root = div()
            .id("ntex-token-editor")
            .key_context("NtexEdit TokenEditor")
            .track_focus(&self.focus)
            .cursor(gpui::CursorStyle::IBeam)
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .pb(px(3.) - (rule - px(1.)))
            .border_b(rule)
            .border_color(colour)
            .on_action(cx.listener(|this, _: &InsertTabStop, w, cx| this.insert_tab_stop(w, cx)))
            .on_action(cx.listener(|this, _: &InsertMirror, w, cx| this.insert_mirror(w, cx)))
            .on_action(cx.listener(|this, _: &InsertExit, w, cx| this.insert_exit(w, cx)))
            .on_action(cx.listener(|this, _: &InsertSelection, w, cx| this.insert_selection(w, cx)))
            .on_action(cx.listener(|this, _: &Capture1, w, cx| this.insert_capture(1, w, cx)))
            .on_action(cx.listener(|this, _: &Capture2, w, cx| this.insert_capture(2, w, cx)))
            .on_action(cx.listener(|this, _: &Capture3, w, cx| this.insert_capture(3, w, cx)))
            .on_action(cx.listener(|this, _: &Capture4, w, cx| this.insert_capture(4, w, cx)))
            .on_action(cx.listener(|this, _: &Capture5, w, cx| this.insert_capture(5, w, cx)))
            .on_action(cx.listener(|this, _: &Capture6, w, cx| this.insert_capture(6, w, cx)))
            .on_action(cx.listener(|this, _: &Capture7, w, cx| this.insert_capture(7, w, cx)))
            .on_action(cx.listener(|this, _: &Capture8, w, cx| this.insert_capture(8, w, cx)))
            .on_action(cx.listener(|this, _: &Capture9, w, cx| this.insert_capture(9, w, cx)))
            .on_mouse_move(cx.listener(Self::on_hover_move))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !hovered && this.hover.is_some() {
                    this.hover = None;
                    cx.notify();
                }
            }));
        edit::listen(root, cx).child(EditArea::new(cx.entity())).when_some(tooltip, |d, (text, b)| {
            d.child(deferred(
                // above the chip, so it never covers the line below
                anchored().anchor(gpui::Anchor::BottomLeft).position(point(b.left(), b.top() - px(4.))).snap_to_window().child(
                    div()
                        .px(px(5.))
                        .py(px(1.))
                        .bg(t.paper)
                        .border_1()
                        .border_color(t.slate)
                        .font(t.serif_italic())
                        .text_size(t.footnote_size)
                        .line_height(t.line(t.footnote_size))
                        .text_color(t.ink)
                        .child(text),
                ),
            ))
        })
    }
}

/// The token keys, in the "TokenEditor" context (deeper than any page
/// binding, so Tab here is a tab stop, not focus-next).
pub fn bind_keys(cx: &mut App) {
    let c = Some("TokenEditor");
    cx.bind_keys([
        KeyBinding::new("tab", InsertTabStop, c),
        KeyBinding::new("shift-tab", InsertMirror, c),
        KeyBinding::new("cmd-e", InsertExit, c),
        KeyBinding::new("cmd-shift-v", InsertSelection, c),
        KeyBinding::new("alt-1", Capture1, c),
        KeyBinding::new("alt-2", Capture2, c),
        KeyBinding::new("alt-3", Capture3, c),
        KeyBinding::new("alt-4", Capture4, c),
        KeyBinding::new("alt-5", Capture5, c),
        KeyBinding::new("alt-6", Capture6, c),
        KeyBinding::new("alt-7", Capture7, c),
        KeyBinding::new("alt-8", Capture8, c),
        KeyBinding::new("alt-9", Capture9, c),
    ]);
}

// ---------------------------------------------------------------------------
// the toolbar

/// The row of token buttons over the editor: `[⇥ tab stop] [⇧⇥ mirror]
/// [⌘E end] [⌘⇧V selection] [⌥1 letter]`, only the allowed ones. Clicking
/// one inserts at the caret and gives the editor the keyboard back.
#[derive(IntoElement)]
pub struct TokenToolbar {
    pub editor: Entity<TokenEditor>,
}

impl TokenToolbar {
    pub fn new(editor: Entity<TokenEditor>) -> TokenToolbar {
        TokenToolbar { editor }
    }
}

impl RenderOnce for TokenToolbar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ed = self.editor.read(cx);
        let allowed = ed.allowed;
        let post = ed.post_capture;
        let mk = |id: &'static str, label: &'static str, keys: &'static str, f: fn(&mut TokenEditor, &mut Window, &mut Context<TokenEditor>)| {
            let editor = self.editor.clone();
            Button::new(id, label).keys(keys).small().on_click(move |_: &ClickEvent, window, cx| {
                editor.update(cx, |ed, cx| {
                    window.focus(&ed.focus, cx);
                    f(ed, window, cx)
                });
            })
        };
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(8.))
            .child(mk("tok-tab", "tab stop", "⇥", |e, w, cx| e.insert_tab_stop(w, cx)))
            .child(mk("tok-mirror", "mirror", "⇧⇥", |e, w, cx| e.insert_mirror(w, cx)))
            .child(mk("tok-exit", "end", "⌘E", |e, w, cx| e.insert_exit(w, cx)))
            .when(allowed.selection, |d| d.child(mk("tok-sel", "selection", "⌘⇧V", |e, w, cx| e.insert_selection(w, cx))))
            .when(allowed.captures > 0, |d| {
                let label = if post { "letter" } else { "capture" };
                d.child(mk("tok-cap", label, "⌥1", |e, w, cx| e.insert_capture(1, w, cx)))
            })
    }
}
