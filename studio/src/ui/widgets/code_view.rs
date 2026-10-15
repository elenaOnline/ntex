//! Read-only code (the generated Lua / LaTeX in the Proof box): LM Mono,
//! light syntax colouring (strings, comments, \commands, tab-stop syntax),
//! selectable, ⌘C copies.
//!
//! Colours stay inside the theme: comments italic slate, Lua strings the
//! definition head (teal), `\commands` theme blue, and the snippet syntax in
//! its chip's colour (`<>`/`<1:x>` blue, `<0>` green, `%V` violet, `%1`
//! maroon, escapes `%%` `%<` `%>` slate).
//!
//! Selection state is kept per (language, code): a new code resets it.
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, ClipboardItem, ElementId, Font, Hsla, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    ParentElement, RenderOnce, SharedString, StyledText, TextRun, Window, div, prelude::*, px,
};

use super::theme::{ActiveTheme as _, Theme};

#[derive(IntoElement)]
pub struct CodeView {
    pub code: SharedString,
    /// "lua" | "latex"
    pub language: &'static str,
}

impl CodeView {
    pub fn new(code: impl Into<SharedString>, language: &'static str) -> CodeView {
        CodeView { code: code.into(), language }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Comment,
    Str,
    Command,
    Tab,
    Exit,
    Sel,
    Capture,
    Escape,
}

/// Colour spans over `code` (byte ranges, in order, covering everything).
pub fn highlight(code: &str, language: &str) -> Vec<(Range<usize>, Tone)> {
    let b = code.as_bytes();
    let mut out: Vec<(Range<usize>, Tone)> = Vec::new();
    let push = |out: &mut Vec<(Range<usize>, Tone)>, r: Range<usize>, t: Tone| {
        if r.is_empty() {
            return;
        }
        if let Some(last) = out.last_mut()
            && last.1 == t
            && last.0.end == r.start
        {
            last.0.end = r.end;
            return;
        }
        out.push((r, t));
    };
    // snippet body / LaTeX inner tokens within [start, end)
    fn inner(code: &str, start: usize, end: usize, base: Tone, latex_comments: bool, out: &mut Vec<(Range<usize>, Tone)>, push: &dyn Fn(&mut Vec<(Range<usize>, Tone)>, Range<usize>, Tone)) {
        let b = code.as_bytes();
        let mut i = start;
        let mut plain = start;
        while i < end {
            let c = b[i];
            let tok: Option<(usize, Tone)> = if c == b'\\' && i + 1 < end && (b[i + 1] as char).is_ascii_alphabetic() {
                let mut j = i + 1;
                while j < end && (b[j] as char).is_ascii_alphabetic() {
                    j += 1;
                }
                Some((j, Tone::Command))
            } else if c == b'\\' && i + 1 < end && b[i + 1] != b'\n' {
                Some((i + 2, base))
            } else if c == b'%' && i + 1 < end {
                match b[i + 1] {
                    b'%' | b'<' | b'>' => Some((i + 2, Tone::Escape)),
                    b'V' => Some((i + 2, Tone::Sel)),
                    d if d.is_ascii_digit() => Some((i + 2, Tone::Capture)),
                    _ if latex_comments => {
                        let j = code[i..end].find('\n').map(|k| i + k).unwrap_or(end);
                        Some((j, Tone::Comment))
                    }
                    _ => None,
                }
            } else if c == b'%' && latex_comments {
                Some((end.min(i + 1), Tone::Comment))
            } else if c == b'<' {
                // <>, <:x>, <n>, <n:x>, <0>
                let mut j = i + 1;
                while j < end && b[j].is_ascii_digit() {
                    j += 1;
                }
                let digits = &code[i + 1..j];
                if j < end && b[j] == b':' {
                    let close = code[j..end].find('>').map(|k| j + k);
                    close.map(|k| (k + 1, Tone::Tab))
                } else if j < end && b[j] == b'>' {
                    Some((j + 1, if digits == "0" { Tone::Exit } else { Tone::Tab }))
                } else {
                    None
                }
            } else {
                None
            };
            match tok {
                Some((j, t)) => {
                    push(out, plain..i, base);
                    push(out, i..j, t);
                    i = j;
                    plain = j;
                }
                None => {
                    i += code[i..].chars().next().map(char::len_utf8).unwrap_or(1);
                }
            }
        }
        push(out, plain..end, base);
    }
    if language == "lua" {
        let mut i = 0;
        let mut plain = 0;
        while i < b.len() {
            let rest = &code[i..];
            if rest.starts_with("--") {
                push(&mut out, plain..i, Tone::Plain);
                let j = rest.find('\n').map(|k| i + k).unwrap_or(b.len());
                push(&mut out, i..j, Tone::Comment);
                i = j;
                plain = j;
            } else if rest.starts_with('[') && long_open(rest).is_some() {
                let level = long_open(rest).unwrap();
                let open = 2 + level;
                let close = format!("]{}]", "=".repeat(level));
                let body_end = code[i + open..].find(&close).map(|k| i + open + k).unwrap_or(b.len());
                let end = (body_end + close.len()).min(b.len());
                push(&mut out, plain..i, Tone::Plain);
                push(&mut out, i..i + open, Tone::Str);
                inner(code, i + open, body_end, Tone::Str, false, &mut out, &push);
                push(&mut out, body_end..end, Tone::Str);
                i = end;
                plain = end;
            } else if rest.starts_with('"') || rest.starts_with('\'') {
                let q = b[i];
                let mut j = i + 1;
                while j < b.len() && b[j] != q && b[j] != b'\n' {
                    if b[j] == b'\\' {
                        j += 1;
                    }
                    j += 1;
                }
                let end = (j + 1).min(b.len());
                push(&mut out, plain..i, Tone::Plain);
                push(&mut out, i..end, Tone::Str);
                i = end;
                plain = end;
            } else {
                i += rest.chars().next().map(char::len_utf8).unwrap_or(1);
            }
        }
        push(&mut out, plain..b.len(), Tone::Plain);
    } else {
        inner(code, 0, b.len(), Tone::Plain, true, &mut out, &push);
    }
    out
}

/// `[[` or `[==[` at the start: the level (number of `=`).
fn long_open(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    if b.first() != Some(&b'[') {
        return None;
    }
    let mut j = 1;
    while j < b.len() && b[j] == b'=' {
        j += 1;
    }
    (j < b.len() && b[j] == b'[').then_some(j - 1)
}

fn tone_style(t: &Theme, tone: Tone) -> (Hsla, bool) {
    match tone {
        Tone::Plain => (t.ink, false),
        Tone::Comment => (t.slate, true),
        Tone::Str => (t.definition.head, false),
        Tone::Command => (t.blue, false),
        Tone::Tab => (t.theorem.rule, false),
        Tone::Exit => (t.remark.rule, false),
        Tone::Sel => (t.lemma.rule, false),
        Tone::Capture => (t.example.rule, false),
        Tone::Escape => (t.slate, false),
    }
}

#[derive(Default)]
struct ViewState {
    sel: Option<(usize, usize)>,
    dragging: bool,
    layout: Option<gpui::TextLayout>,
}

impl ViewState {
    fn range(&self) -> Option<Range<usize>> {
        self.sel.map(|(a, b)| a.min(b)..a.max(b)).filter(|r| !r.is_empty())
    }
}

fn index_at(state: &ViewState, p: gpui::Point<gpui::Pixels>) -> Option<usize> {
    let layout = state.layout.as_ref()?;
    Some(match layout.index_for_position(p) {
        Ok(i) | Err(i) => i,
    })
}

impl RenderOnce for CodeView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme().clone();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.code.hash(&mut h);
        let key = format!("code-view-{}-{:x}", self.language, h.finish());
        let id = ElementId::Name(key.into());
        let state = window
            .use_keyed_state(id.clone(), cx, |_, _| Rc::new(RefCell::new(ViewState::default())))
            .read(cx)
            .clone();
        let focus = window
            .use_keyed_state(ElementId::Name("code-view-focus".into()), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();

        let code = self.code.clone();
        let sel = state.borrow().range();
        let mono: Font = t.mono();
        let italic: Font = t.mono_italic();
        let mut runs = Vec::new();
        for (r, tone) in highlight(&code, self.language) {
            let (color, it) = tone_style(&t, tone);
            // split at the selection's ends, so the selection gets a fill
            let mut cuts = vec![r.start, r.end];
            if let Some(s) = &sel {
                for c in [s.start, s.end] {
                    if c > r.start && c < r.end {
                        cuts.push(c);
                    }
                }
            }
            cuts.sort_unstable();
            cuts.dedup();
            for w in cuts.windows(2) {
                let selected = sel.as_ref().is_some_and(|s| w[0] >= s.start && w[1] <= s.end);
                runs.push(TextRun {
                    len: w[1] - w[0],
                    font: if it { italic.clone() } else { mono.clone() },
                    color,
                    background_color: selected.then_some(t.selection),
                    underline: None,
                    strikethrough: None,
                });
            }
        }
        let text = StyledText::new(code.clone()).with_runs(runs);
        state.borrow_mut().layout = Some(text.layout().clone());

        let (s1, s2, s3, s4) = (state.clone(), state.clone(), state.clone(), state.clone());
        let focus2 = focus.clone();
        div()
            .id(id)
            .key_context("CodeView")
            .track_focus(&focus)
            .cursor(gpui::CursorStyle::IBeam)
            .w_full()
            .font(t.mono())
            .text_size(t.code_size)
            .line_height(t.line(t.code_size))
            .text_color(t.ink)
            .on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, window, cx| {
                window.focus(&focus2, cx);
                let mut s = s1.borrow_mut();
                if let Some(i) = index_at(&s, e.position) {
                    if e.click_count >= 2 {
                        // a word (double) or everything (triple)
                        let code = &code;
                        let r = if e.click_count >= 3 { 0..code.len() } else { word_at(code, i) };
                        s.sel = Some((r.start, r.end));
                        s.dragging = false;
                    } else {
                        s.sel = Some((i, i));
                        s.dragging = true;
                    }
                }
                window.refresh();
            })
            .on_mouse_move(move |e: &MouseMoveEvent, window, _| {
                let mut s = s2.borrow_mut();
                if !s.dragging {
                    return;
                }
                if e.pressed_button != Some(MouseButton::Left) {
                    s.dragging = false;
                    return;
                }
                if let (Some(i), Some((a, _))) = (index_at(&s, e.position), s.sel) {
                    s.sel = Some((a, i));
                    window.refresh();
                }
            })
            .on_mouse_up(MouseButton::Left, move |_, _, _| s3.borrow_mut().dragging = false)
            .on_key_down({
                let code = self.code.clone();
                move |e: &KeyDownEvent, window, cx| {
                    let k = &e.keystroke;
                    if !k.modifiers.platform {
                        return;
                    }
                    match k.key.as_str() {
                        "c" => {
                            let s = s4.borrow();
                            let text = s.range().map(|r| code[r].to_string()).unwrap_or_else(|| code.to_string());
                            cx.write_to_clipboard(ClipboardItem::new_string(text));
                            cx.stop_propagation();
                        }
                        "a" => {
                            s4.borrow_mut().sel = Some((0, code.len()));
                            window.refresh();
                            cx.stop_propagation();
                        }
                        _ => {}
                    }
                }
            })
            .child(text)
            .child(div().h(px(0.)))
    }
}

fn word_at(code: &str, i: usize) -> Range<usize> {
    let is_w = |c: char| c.is_alphanumeric() || c == '_' || c == '\\';
    let mut a = i.min(code.len());
    while a > 0 && !code.is_char_boundary(a) {
        a -= 1;
    }
    let mut b = a;
    while let Some(c) = code[..a].chars().next_back().filter(|c| is_w(*c)) {
        a -= c.len_utf8();
    }
    while let Some(c) = code[b..].chars().next().filter(|c| is_w(*c)) {
        b += c.len_utf8();
    }
    a..b
}
