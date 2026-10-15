//! Page furniture: \section headings, the notes theme's tcolorbox blocks,
//! booktabs tables, footnotes, the running head, log lines, margin notes.
//!
//! Text helpers at the bottom (`label`, `sup`, `pmb`, `rich`) are what the
//! widgets themselves use; SCREENS may use them for one-off copy.
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, ElementId, Font, Hsla, IntoElement, ParentElement, Pixels,
    RenderOnce, SharedString, StyledText, TextRun, Window, div, prelude::*, px, relative,
};

use super::theme::{ActiveTheme as _, Theme};
use crate::model::Severity;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxKind {
    Theorem,
    Lemma,
    Definition,
    Example,
    Remark,
    /// The quiet grey proof block; draws a □ at the bottom right.
    Proof,
}

impl BoxKind {
    /// The environment's printed name: "Theorem", …, "Proof".
    pub fn name(self) -> &'static str {
        match self {
            BoxKind::Theorem => "Theorem",
            BoxKind::Lemma => "Lemma",
            BoxKind::Definition => "Definition",
            BoxKind::Example => "Example",
            BoxKind::Remark => "Remark",
            BoxKind::Proof => "Proof",
        }
    }
}

// ---------------------------------------------------------------------------
// \section

/// "1  New snippet": bold, the number in theme blue (titlesec as in her theme).
#[derive(IntoElement)]
pub struct Section {
    pub number: SharedString,
    pub title: SharedString,
}

impl Section {
    pub fn new(number: impl Into<SharedString>, title: impl Into<SharedString>) -> Section {
        Section { number: number.into(), title: title.into() }
    }
}

impl RenderOnce for Section {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme();
        let size = t.section_size;
        div()
            .flex()
            .flex_row()
            .items_start()
            .text_size(size)
            .line_height(t.line(size))
            .font(t.serif_bold())
            .text_color(t.ink)
            .when(!self.number.is_empty(), |d| {
                // \titleformat{…}{\color{ntexBlue}\thesection}{0.75em}
                d.child(div().text_color(t.blue).mr(size * 0.75).child(self.number))
            })
            .child(self.title)
    }
}

// ---------------------------------------------------------------------------
// tcolorbox theorem blocks

/// A theorem-like block: tinted fill, 3px rule on the left, run-in head
/// "Definition 1.1 (Trigger)." in the head tone, then the body.
///
/// By default the head sits on its own first line and the children stack
/// under it (the mockups' layout). `run_in()` sets the first child on the
/// head's line instead, as amsthm does with a paragraph. `italic_body()` is
/// the theorem/lemma body style. `flash(…)` is the save-error pulse.
#[derive(IntoElement)]
pub struct TBox {
    pub kind: BoxKind,
    /// "Definition 1.1" / "Remark" / "Proof"
    pub head: SharedString,
    /// The optional name in parentheses: "Trigger".
    pub name: Option<SharedString>,
    pub children: Vec<AnyElement>,
    /// Draw the head on its own line instead of run-in.
    pub head_on_own_line: bool,
    // ---- added by WIDGETS
    /// Set the first child on the head's line (amsthm run-in).
    pub run_in: bool,
    /// \itshape body (theorem, lemma).
    pub italic_body: bool,
    /// Pulse the rule blue twice (the save-error flash). The id must change
    /// for each new flash so the animation restarts.
    pub flash: Option<ElementId>,
    /// An element id, for screens that need to find the box (scroll-to, bounds).
    pub id: Option<ElementId>,
    /// Hide the □ of a Proof box.
    pub no_qed: bool,
}

impl TBox {
    pub fn new(kind: BoxKind, head: impl Into<SharedString>) -> TBox {
        TBox {
            kind,
            head: head.into(),
            name: None,
            children: Vec::new(),
            head_on_own_line: false,
            run_in: false,
            italic_body: false,
            flash: None,
            id: None,
            no_qed: false,
        }
    }
    pub fn name(mut self, name: impl Into<SharedString>) -> TBox {
        self.name = Some(name.into());
        self
    }
    pub fn own_line(mut self) -> TBox {
        self.head_on_own_line = true;
        self
    }
    pub fn run_in(mut self) -> TBox {
        self.run_in = true;
        self
    }
    pub fn italic_body(mut self) -> TBox {
        self.italic_body = true;
        self
    }
    pub fn flash(mut self, id: impl Into<ElementId>) -> TBox {
        self.flash = Some(id.into());
        self
    }
    pub fn id(mut self, id: impl Into<ElementId>) -> TBox {
        self.id = Some(id.into());
        self
    }
    pub fn no_qed(mut self) -> TBox {
        self.no_qed = true;
        self
    }
}

impl ParentElement for TBox {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// The run-in head as one styled line: **Definition 1.1** (Trigger)**.**
/// Proof: *Proof (Generated Lua).* (amsthm's proof head is italic).
pub fn box_head(t: &Theme, kind: BoxKind, head: &str, name: Option<&str>) -> StyledText {
    let tone = t.tones(kind).head;
    let mut r = Rich::new();
    if kind == BoxKind::Proof {
        // amsthm: the whole head italic, "*Proof (Generated Lua).*". Pagella's
        // italic has upright parentheses (by design), which reads as half
        // italic, half upright; take the sloped ones from LM Roman Italic.
        let it = t.serif_italic();
        let paren = t.italic_parens();
        r = r.span(head, it.clone(), tone);
        if let Some(name) = name {
            r = r.span(" ", it.clone(), tone).span("(", paren.clone(), tone).span(name, it.clone(), tone).span(")", paren, tone);
        }
        r.span(".", it, tone).build()
    } else {
        r = r.span(head, t.serif_bold(), tone);
        if let Some(name) = name {
            r = r.span(format!(" ({name})"), t.serif(), tone);
        }
        r.span(".", t.serif_bold(), tone).build()
    }
}

impl RenderOnce for TBox {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme().clone();
        let tones = t.tones(self.kind);
        let proof = self.kind == BoxKind::Proof;
        let head = box_head(&t, self.kind, &self.head, self.name.as_deref());
        let gap = if self.head_on_own_line { px(6.) } else { px(4.) };
        let mut children = self.children.into_iter();
        let head_row = if self.run_in {
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_start()
                .gap(t.body_size * 0.6) // amsthm's .6em after the head
                .child(div().flex_none().child(head))
                .children(children.next())
        } else {
            div().child(head)
        };
        let body = div()
            .flex()
            .flex_col()
            .gap(gap)
            .when(self.italic_body, |d| d.italic())
            .child(head_row)
            .children(children);
        let qed = (proof && !self.no_qed).then(|| {
            div()
                .flex()
                .flex_row()
                .justify_end()
                .font(t.math_font())
                .text_color(t.qed)
                .text_size(t.body_size)
                .line_height(px(12.))
                .child("□")
        });
        let rule_w = if proof { px(1.) } else { px(3.) };
        let (pl, pr, pt, pb) = if proof { (10., 10., 6., 6.) } else { (11., 10., 7., 7.) };
        let base = div()
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .bg(tones.back)
            .border_l(rule_w)
            .border_color(tones.rule)
            .pl(px(pl))
            .pr(px(pr))
            .pt(px(pt))
            .pb(px(pb))
            .font(t.serif())
            .text_size(t.body_size)
            .line_height(t.line(t.body_size))
            .text_color(t.ink)
            .child(body)
            .children(qed);
        let base = match self.id {
            Some(id) => base.id(id).into_any_element(),
            None => base.into_any_element(),
        };
        match self.flash {
            None => base,
            Some(id) => {
                // the rule pulses blue twice: an overlay rule fading in/out
                let blue = t.blue;
                let pulse = div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .w(px(3.))
                    .bg(blue)
                    .with_animation(id, Animation::new(Duration::from_millis(900)), move |d, delta| {
                        let a = (delta * std::f32::consts::TAU * 2.0 - std::f32::consts::FRAC_PI_2).sin() * 0.5 + 0.5;
                        d.opacity(a)
                    });
                div().relative().w_full().child(base).child(pulse).into_any_element()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// booktabs

/// A booktabs table: \toprule, header row in small caps, \midrule, rows,
/// \bottomrule. Columns size to content; `widths` (fractions) optional
/// (then the table spans the line).
#[derive(IntoElement)]
pub struct Table {
    pub header: Vec<SharedString>,
    pub rows: Vec<Vec<AnyElement>>,
    pub widths: Option<Vec<f32>>,
}

impl Table {
    pub fn new(header: Vec<SharedString>) -> Table {
        Table { header, rows: Vec::new(), widths: None }
    }
    pub fn row(mut self, cells: Vec<AnyElement>) -> Table {
        self.rows.push(cells);
        self
    }
    pub fn rows(mut self, rows: impl IntoIterator<Item = Vec<AnyElement>>) -> Table {
        self.rows.extend(rows);
        self
    }
    pub fn widths(mut self, widths: Vec<f32>) -> Table {
        self.widths = Some(widths);
        self
    }
}

impl RenderOnce for Table {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme().clone();
        let cols = self.header.len().max(self.rows.iter().map(Vec::len).max().unwrap_or(0)).max(1);
        let heavy = px(1.2);
        let light = px(0.6);
        let cell = |first: bool| div().pl(if first { px(0.) } else { px(6.) }).pr(px(6.)).min_w_0();
        let header = self.header.iter().enumerate().map(|(i, h)| {
            cell(i == 0)
                .pt(px(3.))
                .pb(px(3.))
                .border_b(light)
                .border_color(t.rule)
                .font(t.caps())
                .child(h.clone())
                .into_any_element()
        });
        let mut body: Vec<AnyElement> = Vec::new();
        let nrows = self.rows.len();
        for (r, row) in self.rows.into_iter().enumerate() {
            let mut row: Vec<AnyElement> = row;
            row.resize_with(cols, || div().into_any_element());
            for (i, c) in row.into_iter().enumerate() {
                body.push(
                    cell(i == 0)
                        .pt(px(if r == 0 { 4. } else { 1. }))
                        .pb(px(if r + 1 == nrows { 4. } else { 1. }))
                        .child(c)
                        .into_any_element(),
                );
            }
        }
        let frame = div()
            .border_t(heavy)
            .border_b(heavy)
            .border_color(t.rule)
            .font(t.serif())
            .text_size(t.body_size)
            .line_height(t.line(t.body_size))
            .text_color(t.ink);
        match self.widths {
            Some(widths) => {
                // full-width rows with fractional columns
                let mut all: Vec<AnyElement> = header.collect();
                all.extend(body);
                let mut out = frame.w_full().flex().flex_col();
                let mut it = all.into_iter().peekable();
                while it.peek().is_some() {
                    let row: Vec<AnyElement> = it.by_ref().take(cols).collect();
                    out = out.child(div().flex().flex_row().w_full().children(row.into_iter().enumerate().map(
                        |(i, c)| div().w(relative(widths.get(i).copied().unwrap_or(1.0 / cols as f32))).child(c),
                    )));
                }
                out.into_any_element()
            }
            None => frame
                .self_start()
                .grid()
                .grid_cols_max_content(cols as u16)
                .children(header)
                .children(body)
                .into_any_element(),
        }
    }
}

// ---------------------------------------------------------------------------
// footnotes

/// The footnote area at the bottom of the page: a short rule, then numbered
/// small notes (the keyboard hints for whatever is focused).
#[derive(IntoElement)]
pub struct Footnotes {
    pub notes: Vec<SharedString>,
    /// Told each note's laid-out bounds (tests measure that none wraps).
    pub on_bounds: Option<std::rc::Rc<dyn Fn(usize, gpui::Bounds<Pixels>)>>,
}

impl Footnotes {
    pub fn new(notes: Vec<SharedString>) -> Footnotes {
        Footnotes { notes, on_bounds: None }
    }
    pub fn on_bounds(mut self, f: impl Fn(usize, gpui::Bounds<Pixels>) + 'static) -> Footnotes {
        self.on_bounds = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for Footnotes {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme().clone();
        let size = t.footnote_size;
        div()
            .flex()
            .flex_col()
            .w_full()
            .gap(px(1.))
            .font(t.serif())
            .text_size(size)
            .line_height(t.line(size))
            .text_color(t.ink)
            .child(div().w(relative(1. / 3.)).h(px(1.)).mb(px(4.)).bg(t.head_rule))
            .children(self.notes.into_iter().enumerate().map(|(i, note)| {
                let rec = self.on_bounds.clone();
                let rec2 = self.on_bounds.clone();
                div()
                    .relative()
                    .flex()
                    .flex_row()
                    .items_start()
                    .child(sup(&t, superscript_number(i + 1)))
                    .child(
                        div()
                            .relative()
                            .flex_none()
                            .child(prose(&t, &note, t.serif(), t.ink))
                            .children(rec2.map(|f| gpui::canvas(move |b, _, _| f(i + 1000, b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())),
                    )
                    .children(rec.map(|f| gpui::canvas(move |b, _, _| f(i, b), |_, _, _, _| {}).absolute().top_0().left_0().size_full()))
            }))
    }
}

// ---------------------------------------------------------------------------
// the LaTeX log

/// A LaTeX-log style message line: errors "! …" in the error ink, warnings
/// "Studio Warning: …" in italic slate, notes plain slate.
#[derive(IntoElement)]
pub struct LogLine {
    pub severity: crate::model::Severity,
    pub text: SharedString,
}

impl LogLine {
    pub fn new(severity: Severity, text: impl Into<SharedString>) -> LogLine {
        LogLine { severity, text: text.into() }
    }
    /// The line as it is printed (prefix added when missing).
    pub fn printed(&self) -> String {
        let text = self.text.as_ref();
        match self.severity {
            Severity::Error if !text.starts_with('!') => format!("! {text}"),
            Severity::Warning if !text.starts_with("Studio Warning") => format!("Studio Warning: {text}"),
            _ => text.to_string(),
        }
    }
}

impl RenderOnce for LogLine {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme().clone();
        let printed = self.printed();
        let d = div().line_height(t.line(t.body_size));
        match self.severity {
            Severity::Error => d.font(t.mono()).text_size(t.mono_size).text_color(t.error).child(printed),
            Severity::Warning => {
                d.font(t.serif_italic()).text_size(t.body_size).text_color(t.slate).child(prose(&t, &printed, t.serif_italic(), t.slate))
            }
            Severity::Note => d.font(t.serif()).text_size(t.body_size).text_color(t.slate).child(prose(&t, &printed, t.serif(), t.slate)),
        }
    }
}

// ---------------------------------------------------------------------------
// fancyhdr running head

type SelectHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// The running head: *Ntex Studio* (italic slate) on the left; the pages in
/// small caps on the right, `Snippet · Block · Inbox³`, the current one ink
/// and bold (\pmb, LM has no bold caps), the others slate; clickable; a
/// 0.4 pt rule under it.
#[derive(IntoElement)]
pub struct RunningHead {
    pub id: ElementId,
    pub title: SharedString,
    /// (name, superscript count)
    pub pages: Vec<(SharedString, Option<usize>)>,
    pub current: usize,
    pub on_select: Option<SelectHandler>,
}

impl RunningHead {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> RunningHead {
        RunningHead { id: id.into(), title: title.into(), pages: Vec::new(), current: 0, on_select: None }
    }
    pub fn page(mut self, name: impl Into<SharedString>, count: Option<usize>) -> RunningHead {
        self.pages.push((name.into(), count));
        self
    }
    pub fn current(mut self, current: usize) -> RunningHead {
        self.current = current;
        self
    }
    pub fn on_select(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> RunningHead {
        self.on_select = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for RunningHead {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme().clone();
        let size = t.body_size;
        let n = self.pages.len();
        let mut right = div().flex().flex_row().items_start().font(t.caps()).text_size(size);
        for (i, (name, count)) in self.pages.into_iter().enumerate() {
            let current = i == self.current;
            let colour = if current { t.ink } else { t.slate };
            let word = if current { pmb(name.clone()).into_any_element() } else { div().child(name.clone()).into_any_element() };
            let mut entry = div()
                .id(ElementId::NamedInteger("running-head-page".into(), i as u64))
                .flex()
                .flex_row()
                .items_start()
                .min_h(px(24.))
                .cursor_pointer()
                .text_color(colour)
                .hover(|s| s.text_color(t.ink))
                .child(word)
                .when_some(count.filter(|c| *c > 0), |d, c| d.child(sup(&t, c.to_string()).text_color(colour)));
            if let Some(f) = self.on_select.clone() {
                entry = entry.on_click(move |_, window, cx| f(i, window, cx));
            }
            right = right.child(entry);
            if i + 1 < n {
                right = right.child(div().font(t.serif()).text_color(t.slate).px(px(5.)).child("·"));
            }
        }
        div()
            .id(self.id)
            .flex()
            .flex_row()
            .w_full()
            .justify_between()
            .items_end()
            .pb(px(3.))
            .border_b_1()
            .border_color(t.head_rule)
            .line_height(t.line(size))
            .child(div().font(t.serif_italic()).text_size(size).text_color(t.slate).child(self.title))
            .child(right)
    }
}

// ---------------------------------------------------------------------------
// a margin note (toast)

/// A margin note: small italic slate text that fades in (150 ms), stays, and
/// fades out by `duration` (4 s). Give each new note a new `id` so the
/// animation restarts; drop it from the page after `duration`.
#[derive(IntoElement)]
pub struct MarginNote {
    pub id: ElementId,
    pub text: SharedString,
    pub duration: Duration,
    /// Driven from outside (the studio's clock) instead of the animation.
    pub opacity: Option<f32>,
}

/// The toast is a margin note.
pub type Toast = MarginNote;

impl MarginNote {
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> MarginNote {
        MarginNote { id: id.into(), text: text.into(), duration: Duration::from_millis(3900), opacity: None }
    }
    /// Set the opacity directly (no animation).
    pub fn opacity(mut self, a: f32) -> MarginNote {
        self.opacity = Some(a.clamp(0.0, 1.0));
        self
    }
    pub fn duration(mut self, d: Duration) -> MarginNote {
        self.duration = d;
        self
    }
    /// The fade at the end of a note's life: quick, so the note is never
    /// left half-faded (unreadable) for long.
    pub const FADE: Duration = Duration::from_millis(400);

    /// Opacity at a fraction of the note's life (0…1): full slate ink until
    /// the last 400 ms, then a quick fade.
    pub fn opacity_at(duration: Duration, delta: f32) -> f32 {
        let total = duration.as_secs_f32().max(0.5);
        let t = delta * total;
        let fade_out = Self::FADE.as_secs_f32();
        if t > total - fade_out { ((total - t) / fade_out).clamp(0.0, 1.0) } else { 1.0 }
    }
}

impl RenderOnce for MarginNote {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme().clone();
        let duration = self.duration;
        let note = div()
            .font(t.serif_italic())
            .text_size(t.small_size)
            .line_height(t.line(t.small_size))
            .text_color(t.slate)
            .child(prose(&t, &self.text, t.serif_italic(), t.slate));
        match self.opacity {
            Some(a) => note.opacity(a).into_any_element(),
            None => note
                .with_animation(self.id, Animation::new(duration), move |d, delta| d.opacity(MarginNote::opacity_at(duration, delta)))
                .into_any_element(),
        }
    }
}

// ---------------------------------------------------------------------------
// text helpers

/// A small-caps label styled from the theme.
pub fn caps_label(t: &Theme, text: impl Into<SharedString>) -> gpui::Div {
    div().font(t.caps()).text_size(t.body_size).text_color(t.ink).child(text.into())
}

/// An italic slate aside: *(guessed)*, *trying…*.
pub fn aside(t: &Theme, text: impl Into<SharedString>) -> gpui::Div {
    let text: SharedString = text.into();
    div().font(t.serif_italic()).text_color(t.slate).child(prose(t, &text, t.serif_italic(), t.slate))
}

/// A superscript (footnote marks, page counts).
pub fn sup(t: &Theme, text: impl Into<SharedString>) -> gpui::Div {
    div().text_size(t.sup_size).line_height(t.sup_size).mt(px(1.)).mr(px(2.)).child(text.into())
}

/// A footnote mark's text (set as a superscript by `sup`).
pub fn superscript_number(n: usize) -> String {
    n.to_string()
}

/// \pmb, poor man's bold: the text twice, the second copy 0.4 px to the right.
/// For faces with no bold (LM Roman Caps).
pub fn pmb(text: impl Into<SharedString>) -> gpui::Div {
    let text = text.into();
    div().relative().child(text.clone()).child(div().absolute().top_0().left(px(0.45)).child(text))
}

/// Mixed-font inline text, as one shaped paragraph.
#[derive(Default)]
pub struct Rich {
    text: String,
    runs: Vec<TextRun>,
}

impl Rich {
    pub fn new() -> Rich {
        Rich::default()
    }
    pub fn span(mut self, s: impl AsRef<str>, font: Font, color: Hsla) -> Rich {
        let s = s.as_ref();
        if s.is_empty() {
            return self;
        }
        self.text.push_str(s);
        self.runs.push(TextRun { len: s.len(), font, color, background_color: None, underline: None, strikethrough: None });
        self
    }
    pub fn build(self) -> StyledText {
        StyledText::new(self.text).with_runs(self.runs)
    }
}

/// A hairline of the theme's rule colour, `w` wide (\rule).
pub fn hrule(t: &Theme, w: Pixels) -> gpui::Div {
    div().w(w).h(px(1.)).bg(t.head_rule)
}

// ---------------------------------------------------------------------------
// prose with \texttt

/// What a stretch of prose is set in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpanKind {
    /// The surrounding serif.
    Prose,
    /// `\texttt`: a trigger, file name, command or environment name.
    Code,
    /// A key symbol (⇧ ⌘ ⌥ ⌃ ⌫ ↵ ⇥): the keycaps' face, so it looks the same
    /// in both typefaces (Pagella has its own, narrow ⇧ and small ↵).
    Key,
}

/// The key symbols the hints and keycaps use.
pub fn is_key_glyph(c: char) -> bool {
    matches!(c, '⇧' | '⌘' | '⌥' | '⌃' | '⌫' | '⌦' | '↵' | '⇥' | '⏎' | '⎋' | '↩')
}

const FILE_EXTS: [&str; 7] = ["lua", "sty", "tex", "md", "json", "cls", "png"];

/// `inbox.lua`, `ntex-blocks.sty`: a name, a dot, a known extension.
fn is_file_name(w: &str) -> bool {
    match w.rsplit_once('.') {
        Some((name, ext)) => {
            !name.is_empty()
                && FILE_EXTS.contains(&ext)
                && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/'))
        }
        None => false,
    }
}

/// Split prose into serif, `\texttt` and key-symbol stretches (byte ranges).
/// Code is: the text inside ‘…’ (how core and the screens quote triggers
/// and names), a LaTeX command (`\begin{funfact}`, `\frac{…}{…}`), and a
/// file name (`inbox.lua`). This is what keeps "ff" from becoming the "ﬀ"
/// ligature in a sentence, and what LaTeX would set with `\texttt`.
pub fn spans(text: &str) -> Vec<(std::ops::Range<usize>, SpanKind)> {
    let mut out: Vec<(std::ops::Range<usize>, SpanKind)> = Vec::new();
    let mut push = |r: std::ops::Range<usize>, k: SpanKind| {
        if r.is_empty() {
            return;
        }
        if let Some(last) = out.last_mut()
            && last.1 == k
            && last.0.end == r.start
        {
            last.0.end = r.end;
            return;
        }
        out.push((r, k));
    };
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let end_of = |j: usize| chars.get(j).map_or(text.len(), |c| c.0);
    let word_start = |j: usize| j == 0 || matches!(chars[j - 1].1, ' ' | '\t' | '\n' | '(' | '[' | '“' | '—' | '–' | '/');
    let mut i = 0;
    while i < chars.len() {
        let (at, c) = chars[i];
        // ‘code’
        if c == '‘'
            && let Some(close) = (i + 1..chars.len()).find(|&j| chars[j].1 == '’')
            && close > i + 1
            && !chars[i + 1..close].iter().any(|x| x.1 == '\n')
        {
            push(at..end_of(i + 1), SpanKind::Prose);
            push(end_of(i + 1)..chars[close].0, SpanKind::Code);
            push(chars[close].0..end_of(close + 1), SpanKind::Prose);
            i = close + 1;
            continue;
        }
        if is_key_glyph(c) {
            push(at..end_of(i + 1), SpanKind::Key);
            i += 1;
            continue;
        }
        if word_start(i) && (c == '\\' || c.is_ascii_alphanumeric()) {
            // the word, to the next space
            let mut j = i;
            while j < chars.len() && !matches!(chars[j].1, ' ' | '\t' | '\n' | '’' | '”') {
                j += 1;
            }
            // trailing punctuation belongs to the sentence
            let mut k = j;
            while k > i + 1 && matches!(chars[k - 1].1, '.' | ',' | ';' | ':' | '!' | '?') {
                k -= 1;
            }
            if k > i + 1 && chars[k - 1].1 == ')' && !text[at..end_of(k)].contains('(') {
                k -= 1;
            }
            let word = &text[at..end_of(k)];
            let code = if c == '\\' { word.len() > 1 } else { is_file_name(word) };
            if code {
                push(at..end_of(k), SpanKind::Code);
                i = k;
                continue;
            }
        }
        push(at..end_of(i + 1), SpanKind::Prose);
        i += 1;
    }
    out
}

/// Prose in `font`, with code (‘ff’, `inbox.lua`, `\begin{funfact}`) in the
/// mono face and key symbols in the keycaps' face. One shaped paragraph, so
/// it wraps like text.
pub fn prose(t: &Theme, text: &str, font: Font, color: Hsla) -> StyledText {
    let mut r = Rich::new();
    for (range, kind) in spans(text) {
        let f = match kind {
            SpanKind::Prose => font.clone(),
            SpanKind::Code => t.mono(),
            SpanKind::Key => t.keys_font(),
        };
        r = r.span(&text[range], f, color);
    }
    if text.is_empty() {
        return StyledText::new("");
    }
    r.build()
}

/// Mono text with the cursor mark ‸ drawn as a thin theme-blue bar (as the
/// Try-it table does): the glyph itself comes from a fallback font and
/// hangs below the line. `size` is the mono size; `line` the line height.
pub fn caret_text(t: &Theme, text: &str, size: Pixels, line: Pixels) -> gpui::Div {
    let mut row = div().flex().flex_row().items_center().font(t.mono()).text_size(size).line_height(line);
    for (i, part) in text.split('‸').enumerate() {
        if i > 0 {
            row = row.child(caret_bar(t, size));
        }
        if !part.is_empty() {
            row = row.child(div().flex_none().child(part.to_string()));
        }
    }
    row
}

/// The caret: a 1.5 px theme-blue bar about the height of the type.
pub fn caret_bar(t: &Theme, size: Pixels) -> gpui::Div {
    div().flex_none().w(px(1.5)).h(size * 1.15).mx(px(0.5)).bg(t.blue)
}
