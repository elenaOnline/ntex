//! The editing core shared by TextField, TriggerField and TokenEditor: a
//! buffer of characters and atomic tokens with selection, undo, word/line
//! motions and UTF-16 mapping for the IME; a line layout of text segments
//! and chip boxes; and the element that paints it and registers the
//! platform input handler.
//!
//! A position is an index into `items` (0…=len). A token counts as one
//! position (and one UTF-16 unit, U+FFFC, for the IME).
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, BorderStyle, Bounds, ClipboardItem, Context, Element, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Font, GlobalElementId, Hsla, KeyBinding, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, ShapedLine, SharedString, Style, TextAlign, TextRun, Window,
    actions, fill, point, px, quad, relative, size,
};

use crate::model::Piece;

// ---------------------------------------------------------------------------
// the buffer

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Ch(char),
    /// Never `Piece::Text`.
    Tok(Piece),
}

impl Item {
    pub fn is_tok(&self) -> bool {
        matches!(self, Item::Tok(_))
    }
    pub fn ch(&self) -> Option<char> {
        match self {
            Item::Ch(c) => Some(*c),
            Item::Tok(_) => None,
        }
    }
    fn utf16(&self) -> usize {
        match self {
            Item::Ch(c) => c.len_utf16(),
            Item::Tok(_) => 1,
        }
    }
}

pub fn items_from_str(s: &str) -> Vec<Item> {
    s.chars().map(Item::Ch).collect()
}

pub fn items_from_pieces(pieces: &[Piece]) -> Vec<Item> {
    let mut out = Vec::new();
    for p in pieces {
        match p {
            Piece::Text(s) => out.extend(s.chars().map(Item::Ch)),
            other => out.push(Item::Tok(other.clone())),
        }
    }
    out
}

pub fn pieces_from_items(items: &[Item]) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    for it in items {
        match it {
            Item::Ch(c) => match out.last_mut() {
                Some(Piece::Text(s)) => s.push(*c),
                _ => out.push(Piece::Text(c.to_string())),
            },
            Item::Tok(p) => out.push(p.clone()),
        }
    }
    out
}

/// Plain text of items (tokens dropped).
pub fn plain_text(items: &[Item]) -> String {
    items.iter().filter_map(Item::ch).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditKind {
    None,
    Typing,
    Deleting,
}

#[derive(Clone)]
struct Snap {
    items: Vec<Item>,
    sel: Range<usize>,
    reversed: bool,
}

#[derive(Clone)]
pub struct Buffer {
    pub items: Vec<Item>,
    pub sel: Range<usize>,
    pub reversed: bool,
    /// IME composition range.
    pub marked: Option<Range<usize>>,
    /// Bumped on every content change.
    pub version: u64,
    /// Column kept across ↑/↓.
    goal_col: Option<usize>,
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    last_kind: EditKind,
}

impl Default for Buffer {
    fn default() -> Self {
        Buffer::new(Vec::new())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Space,
    Word,
    Punct,
    Newline,
    Tok,
}

fn class(it: &Item) -> Class {
    match it {
        Item::Tok(_) => Class::Tok,
        Item::Ch('\n') => Class::Newline,
        Item::Ch(c) if c.is_whitespace() => Class::Space,
        Item::Ch(c) if c.is_alphanumeric() || *c == '_' => Class::Word,
        Item::Ch(_) => Class::Punct,
    }
}

impl Buffer {
    pub fn new(items: Vec<Item>) -> Buffer {
        let n = items.len();
        Buffer {
            items,
            sel: n..n,
            reversed: false,
            marked: None,
            version: 0,
            goal_col: None,
            undo: Vec::new(),
            redo: Vec::new(),
            last_kind: EditKind::None,
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn caret(&self) -> usize {
        if self.reversed { self.sel.start } else { self.sel.end }
    }
    pub fn anchor(&self) -> usize {
        if self.reversed { self.sel.end } else { self.sel.start }
    }
    pub fn text(&self) -> String {
        plain_text(&self.items)
    }

    /// Replace everything (not undoable as typing; pushes one undo step).
    pub fn set_items(&mut self, items: Vec<Item>) {
        if items == self.items {
            return;
        }
        self.push_undo(EditKind::None);
        let n = items.len();
        self.items = items;
        self.sel = n..n;
        self.reversed = false;
        self.marked = None;
        self.version += 1;
    }

    /// Replace without touching undo (initial content / external loads).
    pub fn reset(&mut self, items: Vec<Item>) {
        let n = items.len();
        self.items = items;
        self.sel = n..n;
        self.reversed = false;
        self.marked = None;
        self.undo.clear();
        self.redo.clear();
        self.last_kind = EditKind::None;
        self.version += 1;
    }

    pub fn move_to(&mut self, i: usize) {
        let i = i.min(self.len());
        self.sel = i..i;
        self.reversed = false;
        self.last_kind = EditKind::None;
    }

    pub fn select_to(&mut self, i: usize) {
        let i = i.min(self.len());
        let anchor = self.anchor();
        if i < anchor {
            self.sel = i..anchor;
            self.reversed = true;
        } else {
            self.sel = anchor..i;
            self.reversed = false;
        }
        self.last_kind = EditKind::None;
    }

    pub fn select(&mut self, r: Range<usize>) {
        self.sel = r.start.min(self.len())..r.end.min(self.len());
        self.reversed = false;
        self.last_kind = EditKind::None;
    }

    pub fn select_all(&mut self) {
        self.select(0..self.len());
    }

    // ---- boundaries

    pub fn prev(&self, i: usize) -> usize {
        i.saturating_sub(1)
    }
    pub fn next(&self, i: usize) -> usize {
        (i + 1).min(self.len())
    }

    /// ⌥←: skip spaces, then one run of the same class (a token is its own run).
    pub fn word_start(&self, mut i: usize) -> usize {
        while i > 0 && class(&self.items[i - 1]) == Class::Space {
            i -= 1;
        }
        if i == 0 {
            return 0;
        }
        let c = class(&self.items[i - 1]);
        if matches!(c, Class::Tok | Class::Newline) {
            return i - 1;
        }
        while i > 0 && class(&self.items[i - 1]) == c {
            i -= 1;
        }
        i
    }

    /// ⌥→
    pub fn word_end(&self, mut i: usize) -> usize {
        let n = self.len();
        while i < n && class(&self.items[i]) == Class::Space {
            i += 1;
        }
        if i == n {
            return n;
        }
        let c = class(&self.items[i]);
        if matches!(c, Class::Tok | Class::Newline) {
            return i + 1;
        }
        while i < n && class(&self.items[i]) == c {
            i += 1;
        }
        i
    }

    /// The word (or token) around `i`, for double-click.
    pub fn word_at(&self, i: usize) -> Range<usize> {
        let n = self.len();
        if n == 0 {
            return 0..0;
        }
        let i = i.min(n - 1);
        let c = class(&self.items[i]);
        if matches!(c, Class::Tok | Class::Newline) {
            return i..i + 1;
        }
        let (mut a, mut b) = (i, i + 1);
        while a > 0 && class(&self.items[a - 1]) == c {
            a -= 1;
        }
        while b < n && class(&self.items[b]) == c {
            b += 1;
        }
        a..b
    }

    pub fn line_start(&self, i: usize) -> usize {
        let i = i.min(self.len());
        self.items[..i].iter().rposition(|it| *it == Item::Ch('\n')).map(|p| p + 1).unwrap_or(0)
    }
    pub fn line_end(&self, i: usize) -> usize {
        let i = i.min(self.len());
        self.items[i..].iter().position(|it| *it == Item::Ch('\n')).map(|p| p + i).unwrap_or(self.len())
    }

    /// ↑ / ↓ by columns (positions), keeping the goal column.
    pub fn vertical(&mut self, from: usize, down: bool) -> usize {
        let start = self.line_start(from);
        let col = *self.goal_col.get_or_insert(from - start);
        if down {
            let end = self.line_end(from);
            if end == self.len() {
                return self.len();
            }
            let next_start = end + 1;
            let next_end = self.line_end(next_start);
            (next_start + col).min(next_end)
        } else {
            if start == 0 {
                return 0;
            }
            let prev_start = self.line_start(start - 1);
            (prev_start + col).min(start - 1)
        }
    }
    pub fn clear_goal(&mut self) {
        self.goal_col = None;
    }

    // ---- edits

    fn push_undo(&mut self, kind: EditKind) {
        let coalesce = kind != EditKind::None && kind == self.last_kind && !self.undo.is_empty();
        if !coalesce {
            self.undo.push(Snap { items: self.items.clone(), sel: self.sel.clone(), reversed: self.reversed });
            if self.undo.len() > 500 {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last_kind = kind;
    }

    /// Replace `range` with `new`, caret after it. Returns the inserted range.
    pub fn replace(&mut self, range: Range<usize>, new: Vec<Item>) -> Range<usize> {
        let kind = if new.is_empty() {
            EditKind::Deleting
        } else if new.len() == 1 && !new[0].is_tok() && range.is_empty() && new[0] != Item::Ch('\n') {
            EditKind::Typing
        } else {
            EditKind::None
        };
        self.replace_as(range, new, kind)
    }

    fn replace_as(&mut self, range: Range<usize>, new: Vec<Item>, kind: EditKind) -> Range<usize> {
        let range = range.start.min(self.len())..range.end.min(self.len());
        self.push_undo(kind);
        let n = new.len();
        self.items.splice(range.clone(), new);
        let end = range.start + n;
        self.sel = end..end;
        self.reversed = false;
        self.marked = None;
        self.version += 1;
        self.goal_col = None;
        if kind == EditKind::None {
            self.last_kind = EditKind::None;
        }
        range.start..end
    }

    /// Replace the selection (typing).
    pub fn insert(&mut self, new: Vec<Item>) -> Range<usize> {
        self.replace(self.sel.clone(), new)
    }

    /// Delete the selection, or from the caret to `to`.
    pub fn delete_to(&mut self, to: usize) -> bool {
        let r = if self.sel.is_empty() {
            let c = self.caret();
            c.min(to)..c.max(to)
        } else {
            self.sel.clone()
        };
        if r.is_empty() {
            return false;
        }
        self.replace(r, Vec::new());
        true
    }

    /// A non-coalescing marker: the next edit starts a new undo step.
    pub fn break_undo(&mut self) {
        self.last_kind = EditKind::None;
    }

    /// Record the current state as an undo step before a programmatic edit
    /// of `items` (tokens renumbered, a chip's default changed).
    pub fn checkpoint(&mut self) {
        self.push_undo(EditKind::None);
    }

    pub fn undo(&mut self) -> bool {
        let Some(s) = self.undo.pop() else { return false };
        self.redo.push(Snap { items: self.items.clone(), sel: self.sel.clone(), reversed: self.reversed });
        self.restore(s);
        true
    }
    pub fn redo(&mut self) -> bool {
        let Some(s) = self.redo.pop() else { return false };
        self.undo.push(Snap { items: self.items.clone(), sel: self.sel.clone(), reversed: self.reversed });
        self.restore(s);
        true
    }
    fn restore(&mut self, s: Snap) {
        self.items = s.items;
        self.sel = s.sel;
        self.reversed = s.reversed;
        self.marked = None;
        self.version += 1;
        self.last_kind = EditKind::None;
        self.goal_col = None;
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    // ---- UTF-16 (IME)

    pub fn to_utf16(&self, i: usize) -> usize {
        self.items[..i.min(self.len())].iter().map(Item::utf16).sum()
    }
    pub fn from_utf16(&self, u: usize) -> usize {
        let mut acc = 0;
        for (i, it) in self.items.iter().enumerate() {
            if acc >= u {
                return i;
            }
            acc += it.utf16();
        }
        self.len()
    }
    pub fn range_to_utf16(&self, r: &Range<usize>) -> Range<usize> {
        self.to_utf16(r.start)..self.to_utf16(r.end)
    }
    pub fn range_from_utf16(&self, r: &Range<usize>) -> Range<usize> {
        self.from_utf16(r.start)..self.from_utf16(r.end)
    }
    /// The text the IME sees: tokens as U+FFFC.
    pub fn ime_text(&self, r: Range<usize>) -> String {
        self.items[r]
            .iter()
            .map(|it| match it {
                Item::Ch(c) => *c,
                Item::Tok(_) => '\u{FFFC}',
            })
            .collect()
    }

    /// IME / platform insert: replace `range` (or the marked text, or the selection).
    pub fn ime_replace(&mut self, range: Option<Range<usize>>, text: &str) {
        let r = range.or(self.marked.clone()).unwrap_or(self.sel.clone());
        let was_marked = self.marked.is_some();
        let new = items_from_str(text);
        if was_marked {
            // the composition was not an undo step of its own
            self.items.splice(r.clone(), new.clone());
            let end = r.start + new.len();
            self.sel = end..end;
            self.reversed = false;
            self.marked = None;
            self.version += 1;
        } else {
            self.replace(r, new);
        }
    }

    pub fn ime_mark(&mut self, range: Option<Range<usize>>, text: &str, sel_in_new: Option<Range<usize>>) {
        let r = range.or(self.marked.clone()).unwrap_or(self.sel.clone());
        if self.marked.is_none() {
            self.push_undo(EditKind::None);
        }
        let new = items_from_str(text);
        let n = new.len();
        self.items.splice(r.clone(), new);
        self.marked = (n > 0).then(|| r.start..r.start + n);
        self.sel = match sel_in_new {
            Some(s) => r.start + s.start..r.start + s.end,
            None => r.start + n..r.start + n,
        };
        self.reversed = false;
        self.version += 1;
    }
}

// ---------------------------------------------------------------------------
// operations, actions and keys

actions!(
    ntex_edit,
    [
        MoveLeft,
        MoveRight,
        MoveUp,
        MoveDown,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        LineStart,
        LineEnd,
        SelectLineStart,
        SelectLineEnd,
        DocStart,
        DocEnd,
        SelectDocStart,
        SelectDocEnd,
        SelectAll,
        Backspace,
        DeleteForward,
        DeleteWordLeft,
        DeleteWordRight,
        DeleteLineLeft,
        Copy,
        Cut,
        Paste,
        Undo,
        Redo,
        Newline,
        Submit,
        Cancel
    ]
);

/// The shared keys, in the "NtexEdit" key context (every field has it).
pub fn bind_keys(cx: &mut App) {
    let c = Some("NtexEdit");
    cx.bind_keys([
        KeyBinding::new("left", MoveLeft, c),
        KeyBinding::new("right", MoveRight, c),
        KeyBinding::new("up", MoveUp, c),
        KeyBinding::new("down", MoveDown, c),
        KeyBinding::new("shift-left", SelectLeft, c),
        KeyBinding::new("shift-right", SelectRight, c),
        KeyBinding::new("shift-up", SelectUp, c),
        KeyBinding::new("shift-down", SelectDown, c),
        KeyBinding::new("alt-left", WordLeft, c),
        KeyBinding::new("alt-right", WordRight, c),
        KeyBinding::new("alt-shift-left", SelectWordLeft, c),
        KeyBinding::new("alt-shift-right", SelectWordRight, c),
        KeyBinding::new("cmd-left", LineStart, c),
        KeyBinding::new("cmd-right", LineEnd, c),
        KeyBinding::new("home", LineStart, c),
        KeyBinding::new("end", LineEnd, c),
        KeyBinding::new("ctrl-a", LineStart, c),
        KeyBinding::new("ctrl-e", LineEnd, c),
        KeyBinding::new("cmd-shift-left", SelectLineStart, c),
        KeyBinding::new("cmd-shift-right", SelectLineEnd, c),
        KeyBinding::new("shift-home", SelectLineStart, c),
        KeyBinding::new("shift-end", SelectLineEnd, c),
        KeyBinding::new("cmd-up", DocStart, c),
        KeyBinding::new("cmd-down", DocEnd, c),
        KeyBinding::new("cmd-shift-up", SelectDocStart, c),
        KeyBinding::new("cmd-shift-down", SelectDocEnd, c),
        KeyBinding::new("cmd-a", SelectAll, c),
        KeyBinding::new("backspace", Backspace, c),
        KeyBinding::new("shift-backspace", Backspace, c),
        KeyBinding::new("ctrl-h", Backspace, c),
        KeyBinding::new("delete", DeleteForward, c),
        KeyBinding::new("ctrl-d", DeleteForward, c),
        KeyBinding::new("alt-backspace", DeleteWordLeft, c),
        KeyBinding::new("alt-delete", DeleteWordRight, c),
        KeyBinding::new("cmd-backspace", DeleteLineLeft, c),
        KeyBinding::new("cmd-c", Copy, c),
        KeyBinding::new("cmd-x", Cut, c),
        KeyBinding::new("cmd-v", Paste, c),
        KeyBinding::new("cmd-z", Undo, c),
        KeyBinding::new("cmd-shift-z", Redo, c),
        KeyBinding::new("enter", Newline, c),
        KeyBinding::new("shift-enter", Newline, c),
        KeyBinding::new("cmd-enter", Submit, c),
        KeyBinding::new("escape", Cancel, c),
    ]);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Left(bool),
    Right(bool),
    Up(bool),
    Down(bool),
    WordLeft(bool),
    WordRight(bool),
    LineStart(bool),
    LineEnd(bool),
    DocStart(bool),
    DocEnd(bool),
    SelectAll,
    Backspace,
    Delete,
    DeleteWordLeft,
    DeleteWordRight,
    DeleteLineLeft,
    Copy,
    Cut,
    Paste,
    Undo,
    Redo,
    Newline,
    Submit,
    Cancel,
}

/// What the core did, for the widget to react (emit Changed, …).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing changed.
    None,
    /// The caret or selection moved.
    Moved,
    /// The content changed.
    Edited,
}

/// The editing core's handling of an op that is the same in every field.
/// Paste, Copy, Cut, Newline, Submit and Cancel are the widget's business
/// (it knows its clipboard format and what Return means); they return None.
pub fn apply_op(b: &mut Buffer, op: Op) -> Outcome {
    let vertical = matches!(op, Op::Up(_) | Op::Down(_));
    if !vertical {
        b.clear_goal();
    }
    let mv = |b: &mut Buffer, to: usize, select: bool| {
        if select {
            b.select_to(to)
        } else {
            b.move_to(to)
        }
        Outcome::Moved
    };
    match op {
        Op::Left(sel) => {
            let to = if !sel && !b.sel.is_empty() { b.sel.start } else { b.prev(b.caret()) };
            mv(b, to, sel)
        }
        Op::Right(sel) => {
            let to = if !sel && !b.sel.is_empty() { b.sel.end } else { b.next(b.caret()) };
            mv(b, to, sel)
        }
        Op::Up(sel) => {
            let to = b.vertical(b.caret(), false);
            let goal = b.goal_col;
            let o = mv(b, to, sel);
            b.goal_col = goal;
            o
        }
        Op::Down(sel) => {
            let to = b.vertical(b.caret(), true);
            let goal = b.goal_col;
            let o = mv(b, to, sel);
            b.goal_col = goal;
            o
        }
        Op::WordLeft(sel) => mv(b, b.word_start(b.caret()), sel),
        Op::WordRight(sel) => mv(b, b.word_end(b.caret()), sel),
        Op::LineStart(sel) => mv(b, b.line_start(b.caret()), sel),
        Op::LineEnd(sel) => mv(b, b.line_end(b.caret()), sel),
        Op::DocStart(sel) => mv(b, 0, sel),
        Op::DocEnd(sel) => mv(b, b.len(), sel),
        Op::SelectAll => {
            b.select_all();
            Outcome::Moved
        }
        Op::Backspace => edited(b.delete_to(b.prev(b.caret()))),
        Op::Delete => edited(b.delete_to(b.next(b.caret()))),
        Op::DeleteWordLeft => edited(b.delete_to(b.word_start(b.caret()))),
        Op::DeleteWordRight => edited(b.delete_to(b.word_end(b.caret()))),
        Op::DeleteLineLeft => {
            let c = b.caret();
            let start = b.line_start(c);
            // at a line start, ⌘⌫ joins with the line above
            edited(b.delete_to(if start == c { b.prev(c) } else { start }))
        }
        Op::Undo => edited(b.undo()),
        Op::Redo => edited(b.redo()),
        Op::Copy | Op::Cut | Op::Paste | Op::Newline | Op::Submit | Op::Cancel => Outcome::None,
    }
}

fn edited(yes: bool) -> Outcome {
    if yes { Outcome::Edited } else { Outcome::None }
}

/// A widget built on the core.
pub trait Editable: 'static + Sized + EntityInputHandler {
    fn buffer(&self) -> &Buffer;
    fn buffer_mut(&mut self) -> &mut Buffer;
    fn focus_handle_ref(&self) -> &FocusHandle;
    /// Handle one op (keys and toolbar).
    fn op(&mut self, op: Op, window: &mut Window, cx: &mut Context<Self>);
    /// How to draw it.
    fn look(&self, cx: &App) -> Look;
    /// A chip's look (only called for tokens).
    fn chip(&self, _index: usize, _piece: &Piece, _cx: &App) -> ChipLook {
        ChipLook::default()
    }
    fn layout_cache(&mut self) -> &mut Option<Rc<TextLayout>>;
    /// Character ranges with a faint structural underline (e.g. maths).
    fn underlines(&self) -> Vec<Range<usize>> { Vec::new() }
    fn selecting(&mut self) -> &mut bool;
    /// Called after any content change (emit Changed, renumber, notify).
    fn after_edit(&mut self, cx: &mut Context<Self>);
    fn composition_changed(&mut self, cx: &mut Context<Self>) { self.after_edit(cx); }
    /// Text from the platform: typing and IME commits.
    fn input_text(&mut self, range_utf16: Option<Range<usize>>, text: &str, _window: &mut Window, cx: &mut Context<Self>) {
        let b = self.buffer_mut();
        let r = range_utf16.map(|r| b.range_from_utf16(&r));
        b.ime_replace(r, text);
        self.after_edit(cx);
    }
    /// A double-click; default: select the word.
    fn double_click(&mut self, at: usize, _p: Point<Pixels>, _window: &mut Window, cx: &mut Context<Self>) {
        let r = self.buffer().word_at(at);
        self.buffer_mut().select(r);
        cx.notify();
    }
}

/// Register the shared action listeners on a widget's root element.
pub fn listen<V: Editable, E: gpui::InteractiveElement>(el: E, cx: &mut Context<V>) -> E {
    macro_rules! on {
        ($el:expr, $action:ty, $op:expr) => {
            $el.on_action(cx.listener(|this: &mut V, _: &$action, window, cx| this.op($op, window, cx)))
        };
    }
    let el = on!(el, MoveLeft, Op::Left(false));
    let el = on!(el, MoveRight, Op::Right(false));
    let el = on!(el, MoveUp, Op::Up(false));
    let el = on!(el, MoveDown, Op::Down(false));
    let el = on!(el, SelectLeft, Op::Left(true));
    let el = on!(el, SelectRight, Op::Right(true));
    let el = on!(el, SelectUp, Op::Up(true));
    let el = on!(el, SelectDown, Op::Down(true));
    let el = on!(el, WordLeft, Op::WordLeft(false));
    let el = on!(el, WordRight, Op::WordRight(false));
    let el = on!(el, SelectWordLeft, Op::WordLeft(true));
    let el = on!(el, SelectWordRight, Op::WordRight(true));
    let el = on!(el, LineStart, Op::LineStart(false));
    let el = on!(el, LineEnd, Op::LineEnd(false));
    let el = on!(el, SelectLineStart, Op::LineStart(true));
    let el = on!(el, SelectLineEnd, Op::LineEnd(true));
    let el = on!(el, DocStart, Op::DocStart(false));
    let el = on!(el, DocEnd, Op::DocEnd(false));
    let el = on!(el, SelectDocStart, Op::DocStart(true));
    let el = on!(el, SelectDocEnd, Op::DocEnd(true));
    let el = on!(el, SelectAll, Op::SelectAll);
    let el = on!(el, Backspace, Op::Backspace);
    let el = on!(el, DeleteForward, Op::Delete);
    let el = on!(el, DeleteWordLeft, Op::DeleteWordLeft);
    let el = on!(el, DeleteWordRight, Op::DeleteWordRight);
    let el = on!(el, DeleteLineLeft, Op::DeleteLineLeft);
    let el = on!(el, Copy, Op::Copy);
    let el = on!(el, Cut, Op::Cut);
    let el = on!(el, Paste, Op::Paste);
    let el = on!(el, Undo, Op::Undo);
    let el = on!(el, Redo, Op::Redo);
    let el = on!(el, Newline, Op::Newline);
    let el = on!(el, Submit, Op::Submit);
    let el = on!(el, Cancel, Op::Cancel);
    el.on_mouse_down(MouseButton::Left, cx.listener(mouse_down::<V>))
        .on_mouse_up(MouseButton::Left, cx.listener(mouse_up::<V>))
        .on_mouse_up_out(MouseButton::Left, cx.listener(mouse_up::<V>))
        .on_mouse_move(cx.listener(mouse_move::<V>))
}

fn mouse_down<V: Editable>(this: &mut V, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<V>) {
    let focus = this.focus_handle_ref().clone();
    window.focus(&focus, cx);
    let Some(layout) = this.layout_cache().clone() else { return };
    let at = layout.index_for_point(e.position);
    match e.click_count {
        2 => this.double_click(at, e.position, window, cx),
        3 => {
            let b = this.buffer_mut();
            let (s, end) = (b.line_start(at), b.line_end(at));
            b.select(s..end);
        }
        _ => {
            *this.selecting() = true;
            let b = this.buffer_mut();
            b.clear_goal();
            if e.modifiers.shift { b.select_to(at) } else { b.move_to(at) }
        }
    }
    cx.notify();
}

fn mouse_up<V: Editable>(this: &mut V, _: &MouseUpEvent, _: &mut Window, _: &mut Context<V>) {
    *this.selecting() = false;
}

fn mouse_move<V: Editable>(this: &mut V, e: &MouseMoveEvent, _: &mut Window, cx: &mut Context<V>) {
    if !*this.selecting() {
        return;
    }
    if e.pressed_button != Some(MouseButton::Left) {
        *this.selecting() = false;
        return;
    }
    let Some(layout) = this.layout_cache().clone() else { return };
    let at = layout.index_for_point(e.position);
    this.buffer_mut().select_to(at);
    cx.notify();
}

/// Printable keys (no ⌘/⌃/⌥/fn; ⇧ is fine) go straight to a focused field,
/// before any key binding: an app binding for `f` or `space` can't steal
/// typing. IME input sources and compositions still go through the IME first
/// (the platform sends those keys to the input handler before dispatch).
pub fn intercept_typing<V: Editable>(cx: &mut Context<V>) -> gpui::Subscription {
    let this = cx.weak_entity();
    cx.intercept_keystrokes(move |e, window, cx| {
        let k = &e.keystroke;
        let m = k.modifiers;
        if m.platform || m.control || m.function || m.alt {
            return;
        }
        let Some(ch) = k.key_char.as_ref().filter(|s| !s.is_empty() && s.chars().all(|c| !c.is_control())) else {
            return;
        };
        let Some(this) = this.upgrade() else { return };
        let ch = ch.clone();
        let taken = this.update(cx, |v, cx| {
            if !v.focus_handle_ref().is_focused(window) || v.buffer().marked.is_some() {
                return false;
            }
            v.input_text(None, &ch, window, cx);
            true
        });
        if taken {
            cx.stop_propagation();
        }
    })
}

/// Copy/cut plain text of the selection (fields without tokens).
pub fn copy_plain(b: &Buffer, cx: &mut App) -> bool {
    if b.sel.is_empty() {
        return false;
    }
    cx.write_to_clipboard(ClipboardItem::new_string(plain_text(&b.items[b.sel.clone()])));
    true
}

/// Shared `EntityInputHandler` body for an `Editable`.
#[macro_export]
macro_rules! impl_editable_input_handler {
    ($ty:ty) => {
        impl gpui::EntityInputHandler for $ty {
            fn text_for_range(
                &mut self,
                range_utf16: std::ops::Range<usize>,
                actual: &mut Option<std::ops::Range<usize>>,
                _: &mut gpui::Window,
                _: &mut gpui::Context<Self>,
            ) -> Option<String> {
                use $crate::ui::widgets::edit::Editable as _;
                let b = self.buffer();
                let r = b.range_from_utf16(&range_utf16);
                actual.replace(b.range_to_utf16(&r));
                Some(b.ime_text(r))
            }
            fn selected_text_range(
                &mut self,
                _ignore_disabled_input: bool,
                _: &mut gpui::Window,
                _: &mut gpui::Context<Self>,
            ) -> Option<gpui::UTF16Selection> {
                use $crate::ui::widgets::edit::Editable as _;
                let b = self.buffer();
                Some(gpui::UTF16Selection { range: b.range_to_utf16(&b.sel), reversed: b.reversed })
            }
            fn marked_text_range(
                &self,
                _: &mut gpui::Window,
                _: &mut gpui::Context<Self>,
            ) -> Option<std::ops::Range<usize>> {
                use $crate::ui::widgets::edit::Editable as _;
                let b = self.buffer();
                b.marked.as_ref().map(|r| b.range_to_utf16(r))
            }
            fn unmark_text(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) {
                use $crate::ui::widgets::edit::Editable as _;
                self.buffer_mut().marked = None;
            }
            fn replace_text_in_range(
                &mut self,
                range_utf16: Option<std::ops::Range<usize>>,
                text: &str,
                window: &mut gpui::Window,
                cx: &mut gpui::Context<Self>,
            ) {
                use $crate::ui::widgets::edit::Editable as _;
                self.input_text(range_utf16, text, window, cx);
            }
            fn replace_and_mark_text_in_range(
                &mut self,
                range_utf16: Option<std::ops::Range<usize>>,
                text: &str,
                new_selected_range_utf16: Option<std::ops::Range<usize>>,
                _: &mut gpui::Window,
                cx: &mut gpui::Context<Self>,
            ) {
                use $crate::ui::widgets::edit::Editable as _;
                let b = self.buffer_mut();
                let r = range_utf16.map(|r| b.range_from_utf16(&r));
                let chars = |u: usize| {
                    let (mut acc, mut n) = (0, 0);
                    for ch in text.chars() {
                        if acc >= u {
                            break;
                        }
                        acc += ch.len_utf16();
                        n += 1;
                    }
                    n
                };
                let sel = new_selected_range_utf16.map(|s| chars(s.start)..chars(s.end));
                b.ime_mark(r, text, sel);
                self.composition_changed(cx);
            }
            fn bounds_for_range(
                &mut self,
                range_utf16: std::ops::Range<usize>,
                _bounds: gpui::Bounds<gpui::Pixels>,
                _: &mut gpui::Window,
                _: &mut gpui::Context<Self>,
            ) -> Option<gpui::Bounds<gpui::Pixels>> {
                use $crate::ui::widgets::edit::Editable as _;
                let r = self.buffer().range_from_utf16(&range_utf16);
                let layout = self.layout_cache().clone()?;
                Some(layout.bounds_for(r))
            }
            fn character_index_for_point(
                &mut self,
                p: gpui::Point<gpui::Pixels>,
                _: &mut gpui::Window,
                _: &mut gpui::Context<Self>,
            ) -> Option<usize> {
                use $crate::ui::widgets::edit::Editable as _;
                let layout = self.layout_cache().clone()?;
                let i = layout.index_for_point(p);
                Some(self.buffer().to_utf16(i))
            }
        }
    };
}

// ---------------------------------------------------------------------------
// layout

/// How a field draws its text.
#[derive(Clone)]
pub struct Look {
    pub font: Font,
    pub size: Pixels,
    pub line_height: Pixels,
    pub color: Hsla,
    pub placeholder: SharedString,
    pub placeholder_font: Font,
    pub placeholder_color: Hsla,
    pub caret: Hsla,
    pub selection: Hsla,
    /// Show the caret (focused, not composing elsewhere).
    pub focused: bool,
    /// Minimum number of lines to reserve.
    pub min_lines: usize,
    /// Width of a tab (in spaces of the font).
    pub tab_spaces: usize,
    /// Size the area to its text (single-line fields with things after
    /// them, like the trigger's ⇥), at least this wide. None: fill the width.
    pub fit: Option<Pixels>,
}

/// A token chip's look.
#[derive(Clone, Default)]
pub struct ChipLook {
    pub label: SharedString,
    /// The default text, in mono ink after the label.
    pub default: Option<SharedString>,
    pub border: Hsla,
    pub fill: Hsla,
    pub text: Hsla,
    pub default_color: Hsla,
    pub dashed: bool,
    pub label_font: Option<Font>,
    pub label_size: Pixels,
    pub default_font: Option<Font>,
    pub default_size: Pixels,
    /// Being edited: draw the default's caret at its end; `all` = selected.
    pub editing: Option<bool>,
}

pub enum SegKind {
    Text { line: ShapedLine, offsets: Vec<usize> },
    Tab,
    Chip { look: ChipLook, label: ShapedLine, default: Option<ShapedLine> },
}

pub struct Seg {
    pub start: usize,
    pub len: usize,
    pub x: Pixels,
    pub width: Pixels,
    pub kind: SegKind,
}

pub struct Line {
    pub start: usize,
    pub end: usize,
    pub y: Pixels,
    pub segs: Vec<Seg>,
    pub width: Pixels,
}

/// One frame's layout, kept for hit tests and the IME.
pub struct TextLayout {
    pub bounds: Bounds<Pixels>,
    pub line_height: Pixels,
    pub lines: Vec<Line>,
    /// Chip rects (item index, window bounds) for tooltips and hit tests.
    pub chips: Vec<(usize, Bounds<Pixels>)>,
}

const CHIP_PAD: f32 = 3.0;
const CHIP_GAP: f32 = 4.0;
const CHIP_MARGIN: f32 = 1.5;

impl TextLayout {
    fn line_for(&self, i: usize) -> &Line {
        self.lines.iter().find(|l| i >= l.start && i <= l.end).unwrap_or_else(|| self.lines.last().unwrap())
    }

    /// x (relative to the bounds) of position `i`.
    pub fn x_for(&self, i: usize) -> Pixels {
        let line = self.line_for(i);
        for s in &line.segs {
            if i >= s.start && i < s.start + s.len {
                return match &s.kind {
                    SegKind::Text { line: shaped, offsets } => s.x + shaped.x_for_index(offsets[i - s.start]),
                    _ => s.x,
                };
            }
        }
        line.width
    }

    pub fn point_for(&self, i: usize) -> Point<Pixels> {
        let line = self.line_for(i);
        point(self.bounds.left() + self.x_for(i), self.bounds.top() + line.y)
    }

    pub fn bounds_for(&self, r: Range<usize>) -> Bounds<Pixels> {
        let a = self.point_for(r.start);
        let b = self.point_for(r.end);
        Bounds::from_corners(a, point(b.x.max(a.x + px(1.)), b.y + self.line_height))
    }

    /// The position nearest a window point.
    pub fn index_for_point(&self, p: Point<Pixels>) -> usize {
        let local_y = p.y - self.bounds.top();
        let row = if local_y < px(0.) { 0 } else { (f32::from(local_y) / f32::from(self.line_height)) as usize };
        let line = &self.lines[row.min(self.lines.len() - 1)];
        let x = p.x - self.bounds.left();
        if x <= px(0.) {
            return line.start;
        }
        for s in &line.segs {
            if x < s.x + s.width {
                return match &s.kind {
                    SegKind::Text { line: shaped, offsets } => {
                        let byte = shaped.closest_index_for_x(x - s.x);
                        let k = offsets.partition_point(|o| *o < byte);
                        s.start + k.min(s.len)
                    }
                    _ => {
                        if x - s.x < s.width / 2. {
                            s.start
                        } else {
                            s.start + s.len
                        }
                    }
                };
            }
        }
        line.end
    }

    /// The chip under a window point.
    pub fn chip_at(&self, p: Point<Pixels>) -> Option<usize> {
        self.chips.iter().find(|(_, b)| b.contains(&p)).map(|(i, _)| *i)
    }
}

fn run(font: &Font, len: usize, color: Hsla) -> TextRun {
    TextRun { len, font: font.clone(), color, background_color: None, underline: None, strikethrough: None }
}

/// Lay the buffer out: one line per '\n', each a row of text, tab and chip
/// segments.
pub fn layout<V: Editable>(view: &V, look: &Look, bounds: Bounds<Pixels>, window: &mut Window, cx: &App) -> TextLayout {
    let b = view.buffer();
    let ts = window.text_system().clone();
    let space_w = ts.shape_line(" ".into(), look.size, &[run(&look.font, 1, look.color)], None).width();
    let tab_w = space_w * look.tab_spaces as f32;
    let mut lines = Vec::new();
    let mut chips = Vec::new();
    let mut start = 0;
    let n = b.items.len();
    let mut y = px(0.);
    loop {
        let end = b.items[start..].iter().position(|it| *it == Item::Ch('\n')).map(|p| p + start).unwrap_or(n);
        let mut segs = Vec::new();
        let mut x = px(0.);
        let mut i = start;
        while i < end {
            match &b.items[i] {
                Item::Ch('\t') => {
                    // to the next tab stop
                    let w = tab_w - px(f32::from(x) % f32::from(tab_w).max(1.0));
                    let w = if w < space_w { w + tab_w } else { w };
                    segs.push(Seg { start: i, len: 1, x, width: w, kind: SegKind::Tab });
                    x += w;
                    i += 1;
                }
                Item::Ch(_) => {
                    let mut j = i;
                    let mut text = String::new();
                    let mut offsets = Vec::new();
                    while j < end {
                        match &b.items[j] {
                            Item::Ch(c) if *c != '\t' => {
                                offsets.push(text.len());
                                text.push(*c);
                                j += 1;
                            }
                            _ => break,
                        }
                    }
                    let len = text.len();
                    let shaped = ts.shape_line(text.into(), look.size, &[run(&look.font, len, look.color)], None);
                    let w = shaped.width();
                    segs.push(Seg { start: i, len: j - i, x, width: w, kind: SegKind::Text { line: shaped, offsets } });
                    x += w;
                    i = j;
                }
                Item::Tok(p) => {
                    let cl = view.chip(i, p, cx);
                    let lf = cl.label_font.clone().unwrap_or(look.font.clone());
                    let label = ts.shape_line(
                        cl.label.clone(),
                        cl.label_size,
                        &[run(&lf, cl.label.len(), cl.text)],
                        None,
                    );
                    let default = cl.default.as_ref().filter(|d| !d.is_empty() || cl.editing.is_some()).map(|d| {
                        let df = cl.default_font.clone().unwrap_or(look.font.clone());
                        ts.shape_line(d.clone(), cl.default_size, &[run(&df, d.len(), cl.default_color)], None)
                    });
                    let mut w = px(CHIP_PAD * 2.) + label.width();
                    if let Some(d) = &default {
                        w += px(CHIP_GAP) + d.width().max(px(4.));
                    }
                    let w = w + px(CHIP_MARGIN * 2.);
                    chips.push((
                        i,
                        Bounds::new(
                            point(bounds.left() + x + px(CHIP_MARGIN), bounds.top() + y),
                            size(w - px(CHIP_MARGIN * 2.), look.line_height),
                        ),
                    ));
                    segs.push(Seg { start: i, len: 1, x, width: w, kind: SegKind::Chip { look: cl, label, default } });
                    x += w;
                    i += 1;
                }
            }
        }
        lines.push(Line { start, end, y, segs, width: x });
        y += look.line_height;
        if end >= n {
            break;
        }
        start = end + 1;
    }
    TextLayout { bounds, line_height: look.line_height, lines, chips }
}

// ---------------------------------------------------------------------------
// the element

/// Paints an `Editable`'s buffer: text, chips, selection, caret; registers
/// the input handler.
pub struct EditArea<V: Editable> {
    view: Entity<V>,
}

impl<V: Editable> EditArea<V> {
    pub fn new(view: Entity<V>) -> Self {
        EditArea { view }
    }
}

impl<V: Editable> gpui::IntoElement for EditArea<V> {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

pub struct EditPrepaint {
    layout: Rc<TextLayout>,
    look: Look,
    placeholder: Option<ShapedLine>,
}

impl<V: Editable> Element for EditArea<V> {
    type RequestLayoutState = ();
    type PrepaintState = EditPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let view = self.view.read(cx);
        let look = view.look(cx);
        let lines = 1 + view.buffer().items.iter().filter(|it| **it == Item::Ch('\n')).count();
        let mut style = Style::default();
        style.size.width = match look.fit {
            None => relative(1.).into(),
            Some(min) => {
                let b = view.buffer();
                let text: String = if b.is_empty() {
                    look.placeholder.to_string()
                } else {
                    b.items
                        .iter()
                        .map(|it| match it {
                            Item::Ch('\t') => "    ".to_string(),
                            Item::Ch(c) => c.to_string(),
                            Item::Tok(_) => "    ".to_string(),
                        })
                        .collect()
                };
                let font = if b.is_empty() { &look.placeholder_font } else { &look.font };
                let w = window.text_system().shape_line(text.clone().into(), look.size, &[run(font, text.len(), look.color)], None).width();
                (w + px(2.)).max(min).into()
            }
        };
        style.size.height = (look.line_height * lines.max(look.min_lines) as f32).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> EditPrepaint {
        let view = self.view.read(cx);
        let look = view.look(cx);
        let layout = Rc::new(layout(view, &look, bounds, window, cx));
        let placeholder = (view.buffer().is_empty() && !look.placeholder.is_empty()).then(|| {
            window.text_system().shape_line(
                look.placeholder.clone(),
                look.size,
                &[run(&look.placeholder_font, look.placeholder.len(), look.placeholder_color)],
                None,
            )
        });
        EditPrepaint { layout, look, placeholder }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        pre: &mut EditPrepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.view.read(cx).focus_handle_ref().clone();
        window.handle_input(&focus, ElementInputHandler::new(bounds, self.view.clone()), cx);
        let (sel, marked, caret) = {
            let b = self.view.read(cx).buffer();
            (b.sel.clone(), b.marked.clone(), b.caret())
        };
        let layout = pre.layout.clone();
        let look = &pre.look;
        let lh = look.line_height;
        let origin = bounds.origin;

        // selection (fainter when the field doesn't have the keyboard)
        let sel_fill = if look.focused { look.selection } else { Hsla { a: look.selection.a * 0.45, ..look.selection } };
        if !sel.is_empty() {
            for line in &layout.lines {
                let (a, z) = (sel.start.max(line.start), sel.end.min(line.end));
                let through_newline = sel.end > line.end && sel.start <= line.end && line.end < self.view.read(cx).buffer().len();
                if a > z || (a == z && !through_newline) {
                    continue;
                }
                let x0 = layout.x_for(a);
                let mut x1 = layout.x_for(z);
                if through_newline {
                    x1 += px(6.);
                }
                window.paint_quad(fill(
                    Bounds::from_corners(point(origin.x + x0, origin.y + line.y), point(origin.x + x1, origin.y + line.y + lh)),
                    sel_fill,
                ));
            }
        }

        // text and chips
        for line in &layout.lines {
            for s in &line.segs {
                let at = point(origin.x + s.x, origin.y + line.y);
                match &s.kind {
                    SegKind::Text { line: shaped, .. } => {
                        shaped.paint(at, lh, TextAlign::Left, None, window, cx).ok();
                    }
                    SegKind::Tab => {}
                    SegKind::Chip { look: cl, label, default } => {
                        let h = (cl.label_size.max(cl.default_size) * 1.45).min(lh - px(2.));
                        let top = at.y + (lh - h) / 2.;
                        let rect = Bounds::new(
                            point(at.x + px(CHIP_MARGIN), top),
                            size(s.width - px(CHIP_MARGIN * 2.), h),
                        );
                        let style = if cl.dashed { BorderStyle::Dashed } else { BorderStyle::Solid };
                        window.paint_quad(quad(rect, px(0.), cl.fill, px(1.), cl.border, style));
                        let lx = rect.origin.x + px(CHIP_PAD);
                        let label_lh = cl.label_size * 1.2;
                        let ly = top + (h - label_lh) / 2.;
                        label.paint(point(lx, ly), label_lh, TextAlign::Left, None, window, cx).ok();
                        if let Some(d) = default {
                            let dx = lx + label.width() + px(CHIP_GAP);
                            let dlh = cl.default_size * 1.2;
                            let dy = top + (h - dlh) / 2.;
                            if cl.editing == Some(true) && d.width() > px(0.) {
                                window.paint_quad(fill(
                                    Bounds::new(point(dx, dy), size(d.width(), dlh)),
                                    look.selection,
                                ));
                            }
                            d.paint(point(dx, dy), dlh, TextAlign::Left, None, window, cx).ok();
                            if cl.editing == Some(false) {
                                window.paint_quad(fill(
                                    Bounds::new(point(dx + d.width(), dy), size(px(1.), dlh)),
                                    look.caret,
                                ));
                            }
                        }
                    }
                }
            }
        }

        for range in self.view.read(cx).underlines() {
            for line in &layout.lines {
                let a = range.start.max(line.start);
                let z = range.end.min(line.end);
                if a >= z { continue; }
                window.paint_quad(fill(
                    Bounds::new(point(origin.x + layout.x_for(a), origin.y + line.y + lh - px(2.)),
                        size(layout.x_for(z) - layout.x_for(a), px(1.))),
                    Hsla { a: 0.3, ..look.caret },
                ));
            }
        }

        // the IME composition: a thin underline
        if let Some(m) = marked.filter(|m| !m.is_empty()) {
            let a = layout.point_for(m.start);
            let z = layout.point_for(m.end);
            window.paint_quad(fill(Bounds::new(point(a.x, a.y + lh - px(3.)), size(z.x - a.x, px(1.))), look.color));
        }

        if let Some(p) = pre.placeholder.take() {
            p.paint(origin, lh, TextAlign::Left, None, window, cx).ok();
        }

        // the caret
        if look.focused && sel.is_empty() {
            let p = layout.point_for(caret);
            let inset = (lh - look.size * 1.15).max(px(0.)) / 2.;
            window.paint_quad(fill(
                Bounds::new(point(p.x - px(0.5), p.y + inset), size(px(1.25), lh - inset * 2.)),
                look.caret,
            ));
        }

        self.view.update(cx, |v, _| *v.layout_cache() = Some(layout));
    }
}
