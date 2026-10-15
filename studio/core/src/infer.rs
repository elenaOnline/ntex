//! The studio's guesses and checks. Guesses fill un-pinned fields as she
//! types; checks become Findings (errors block saving, warnings don't).
//!
//! The rules come from `lua/snips/helpers.lua` and `lua/snips/cond.lua`:
//! - autosnippets fire as soon as the text before the cursor ends with the
//!   trigger, so a shorter auto trigger fires before a longer one can be typed;
//! - "word" boundary (math, math_tab, text, pat): never right after a
//!   backslash, and a letter trigger never right after a letter;
//!   "suffix" boundary (suffix, post): may follow a letter, not a backslash;
//! - `pat` is always maths (`where = "math"`); `{ tab = true }` only changes
//!   how it fires, not where;
//! - `vis` only expands from a selection.
use crate::model::*;
use crate::{codegen, luaparse, words};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// small helpers

fn pinned(d: &Draft, field: &str) -> bool {
    d.pinned.iter().any(|p| p == field)
}

fn pin(d: &mut Draft, field: &str) {
    if !pinned(d, field) {
        d.pinned.push(field.to_string());
    }
}

/// ‘x’, as UX.md's copy writes triggers.
fn q(s: &str) -> String {
    format!("‘{s}’")
}

/// The output's text: Text pieces, tab stops as their defaults.
fn out_text(p: &[Piece]) -> String {
    let mut s = String::new();
    for x in p {
        match x {
            Piece::Text(t) => s.push_str(t),
            Piece::Tab { default, .. } => s.push_str(default),
            _ => s.push(' '),
        }
    }
    s
}

fn has_selection(p: &[Piece]) -> bool {
    p.iter().any(|x| matches!(x, Piece::Selection))
}

fn captures(p: &[Piece]) -> Vec<u8> {
    p.iter().filter_map(|x| if let Piece::Capture(k) = x { Some(*k) } else { None }).collect()
}

fn letters(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphabetic())
}

fn starts_with_letter(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
}

/// `\name` command names in `s` (letters only; `\\` and `\[` are skipped).
fn commands(s: &str) -> Vec<&str> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' {
            let st = i + 1;
            let mut j = st;
            while j < b.len() && b[j].is_ascii_alphabetic() {
                j += 1;
            }
            if j > st {
                out.push(&s[st..j]);
                i = j;
                continue;
            }
            i += 2;
            continue;
        }
        i += 1;
    }
    out
}

/// Environments that live inside maths (`\begin{pmatrix}` is typed in maths).
const MATH_ENVS: &[&str] = &[
    "matrix", "pmatrix", "bmatrix", "Bmatrix", "vmatrix", "Vmatrix", "smallmatrix", "cases", "dcases", "rcases", "array",
    "aligned", "alignedat", "gathered", "split", "subarray", "CD",
];

/// Commands that only make sense in text (structure and fonts).
const TEXT_CMDS: &[&str] = &[
    "section", "subsection", "subsubsection", "chapter", "paragraph", "subparagraph", "part", "item", "textbf", "textit",
    "emph", "textsc", "texttt", "textsf", "textrm", "footnote", "usepackage", "documentclass", "includegraphics", "caption",
    "centering", "cite", "citep", "citet", "maketitle", "title", "author", "tableofcontents", "newpage", "noindent",
    "url", "href", "input", "include",
];

/// Commands that start a line of LaTeX structure (no maths around them).
const STRUCTURE_CMDS: &[&str] = &[
    "section", "subsection", "subsubsection", "chapter", "paragraph", "subparagraph", "part", "usepackage", "documentclass",
];

/// The environment names of `\begin{…}` at the start of a line.
fn line_start_envs(s: &str) -> Vec<&str> {
    s.lines()
        .filter_map(|l| l.trim_start().strip_prefix("\\begin{"))
        .filter_map(|r| r.split('}').next())
        .map(|e| e.trim_end_matches('*'))
        .collect()
}

fn text_env_at_line_start(s: &str) -> bool {
    line_start_envs(s).iter().any(|e| !e.is_empty() && !MATH_ENVS.contains(e))
}

/// An unescaped `$`.
fn has_dollar(s: &str) -> bool {
    let b = s.as_bytes();
    (0..b.len()).any(|i| b[i] == b'$' && !codegen_escaped(b, i))
}

/// Is the byte at `i` escaped by an odd run of backslashes?
fn codegen_escaped(b: &[u8], i: usize) -> bool {
    let mut n = 0;
    let mut k = i;
    while k > 0 && b[k - 1] == b'\\' {
        n += 1;
        k -= 1;
    }
    n % 2 == 1
}

/// Does the output look like text structure (typed outside maths)?
pub fn looks_like_text(output: &[Piece]) -> bool {
    let s = out_text(output);
    if text_env_at_line_start(&s) || has_dollar(&s) || s.contains("\\[") || s.contains("\\(") {
        return true;
    }
    commands(&s).iter().any(|c| TEXT_CMDS.contains(c))
}

/// Does the output say "maths" (`^`, `_`, a maths command, a capture, …)?
fn looks_like_math(output: &[Piece]) -> bool {
    if output.iter().any(|p| matches!(p, Piece::Capture(_) | Piece::Selection)) {
        return true;
    }
    let s = out_text(output);
    if s.contains('^') || s.contains('_') {
        return true;
    }
    let math_env = s.split("\\begin{").skip(1).filter_map(|r| r.split('}').next()).any(|e| MATH_ENVS.contains(&e.trim_end_matches('*')));
    if math_env {
        return true;
    }
    commands(&s).iter().any(|c| !TEXT_CMDS.contains(c) && !matches!(*c, "begin" | "end" | "label" | "ref" | "quad" | "qquad"))
}

/// `\begin{…}` (a text environment), `\section`, `\usepackage` … at the start:
/// the outputs that belong at the start of a line, on Tab (Castel's `beg`).
fn env_like(output: &[Piece]) -> bool {
    let s = out_text(output);
    let first = s.trim_start();
    if text_env_at_line_start(first) && first.starts_with("\\begin{") {
        return true;
    }
    commands(first).first().is_some_and(|c| STRUCTURE_CMDS.contains(c) && first.starts_with(&format!("\\{c}")))
}

/// Only for .tex: `\[`, a text `\begin{…}`, `\section`-like structure.
fn tex_only(output: &[Piece]) -> bool {
    let s = out_text(output);
    s.contains("\\[") || text_env_at_line_start(&s) || commands(&s).iter().any(|c| STRUCTURE_CMDS.contains(c))
}

/// Lua pattern magic: `%a`, `%d`, `%.` … (a class or an escaped punctuation).
pub fn has_pattern_magic(trigger: &str) -> bool {
    let b = trigger.as_bytes();
    (0..b.len().saturating_sub(1)).any(|i| {
        b[i] == b'%' && {
            let c = b[i + 1];
            b"adwslupxcgADWSLUPXCGbf".contains(&c) || (c.is_ascii_punctuation() && c != b'%')
        }
    })
}

/// A text trigger that would get in the way of ordinary writing: a word, or
/// the start of common words.
fn wordish(trigger: &str) -> bool {
    starts_with_letter(trigger) && (words::is_word(trigger) || words::starts_common_words(trigger))
}

// ---------------------------------------------------------------------------
// guess

/// Fill every field not in `draft.pinned` with the best guess from the
/// trigger and output (context from what the output looks like; Tab-only
/// for text triggers that start common English words; Kind::Visual when the
/// output holds %V; Post when it holds %1 and the trigger is letters; …).
/// `place`: where she was in ntex when she opened the studio, if known.
pub fn guess(draft: &mut Draft, place: Option<Context>) {
    if draft.kind == Kind::RawLua {
        return;
    }
    let d = draft;
    if !pinned(d, "kind") {
        d.kind = if has_selection(&d.output) {
            Kind::Visual
        } else if has_pattern_magic(&d.trigger) {
            Kind::Pattern
        } else if captures(&d.output).contains(&1) && letters(&d.trigger) {
            Kind::Post
        } else {
            Kind::Plain
        };
    }
    if !pinned(d, "context") {
        d.context = match d.kind {
            Kind::Suffix | Kind::Post | Kind::Pattern | Kind::Visual => Context::Math,
            _ if looks_like_text(&d.output) => Context::Text,
            _ if looks_like_math(&d.output) => Context::Math,
            _ => place.unwrap_or(Context::Math),
        };
    }
    let text_plain = d.kind == Kind::Plain && d.context == Context::Text;
    let envy = env_like(&d.output);
    if !pinned(d, "fires") {
        d.fires = if text_plain && (wordish(&d.trigger) || envy) { Fires::Tab } else { Fires::Auto };
    }
    if !pinned(d, "line") {
        d.line = if text_plain && d.fires == Fires::Tab && envy { LineRule::Start } else { LineRule::Anywhere };
    }
    if !pinned(d, "math_where") {
        d.math_where = MathWhere::Any;
    }
    if !pinned(d, "ft") {
        d.ft = if tex_only(&d.output) { FileType::Tex } else { FileType::Both };
    }
}

// ---------------------------------------------------------------------------
// where a snippet fires, for conflicts

fn draft_where(d: &Draft) -> &'static str {
    // pat/post/suffix/vis are maths whatever the context says
    let text = d.context == Context::Text && matches!(d.kind, Kind::Plain | Kind::RawLua);
    if text {
        return "text";
    }
    match d.math_where {
        MathWhere::Any => "math",
        MathWhere::Inline => "inline",
        MathWhere::Display => "display",
    }
}

fn where_overlap(a: &str, b: &str) -> bool {
    match (a, b) {
        ("text", "text") => true,
        ("text", _) | (_, "text") => false,
        ("math", _) | (_, "math") => true,
        (x, y) => x == y,
    }
}

fn ft_overlap(d: FileType, k: Option<&str>) -> bool {
    match (d, k) {
        (FileType::Both, _) | (_, None) => true,
        (FileType::Tex, Some(f)) => f == "tex",
        (FileType::Markdown, Some(f)) => f == "markdown",
    }
}

fn line_name(l: LineRule) -> Option<&'static str> {
    match l {
        LineRule::Anywhere => None,
        LineRule::Start => Some("start"),
        LineRule::AfterText => Some("after text"),
        LineRule::ListItem => Some("list item"),
    }
}

/// "start" (only spaces before) can't meet "after text" / "list item".
fn line_overlap(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (None, _) | (_, None) => true,
        (Some(x), Some(y)) => x == y || !(x == "start" || y == "start"),
    }
}

fn place_overlap(d: &Draft, k: &KnownSnippet) -> bool {
    let dw = draft_where(d);
    where_overlap(dw, &k.where_)
        && ft_overlap(d.ft, k.ft.as_deref())
        && (dw != "text" || line_overlap(line_name(d.line), k.line_rule.as_deref()))
}

/// The draft's own inbox entry (she is editing it): not a conflict.
fn is_own(d: &Draft, k: &KnownSnippet) -> bool {
    d.id.is_some()
        && k.file.rsplit('/').next() == Some("inbox.lua")
        && (k.trigger == d.trigger || k.pattern.as_deref() == Some(d.trigger.as_str()))
}

#[derive(PartialEq, Debug)]
enum Ident {
    /// math, math_tab, text, suffix: the literal trigger
    Lit(String),
    Post(String),
    Pat(String),
    Vis(String),
}

fn draft_ident(d: &Draft) -> Ident {
    match d.kind {
        Kind::Post => Ident::Post(d.trigger.clone()),
        Kind::Pattern => Ident::Pat(d.trigger.clone()),
        Kind::Visual => Ident::Vis(d.trigger.clone()),
        _ => Ident::Lit(d.trigger.clone()),
    }
}

fn known_ident(k: &KnownSnippet) -> Ident {
    match k.kind.as_str() {
        "post" => Ident::Post(k.trigger.clone()),
        "pattern" => Ident::Pat(k.pattern.clone().unwrap_or_else(|| k.trigger.clone())),
        "visual" => Ident::Vis(k.trigger.clone()),
        _ => Ident::Lit(k.trigger.clone()),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Boundary {
    Word,
    Suffix,
}

/// Something that fires by itself while she types.
enum Firer {
    Lit(String, Boundary),
    Pat(String),
}

fn known_firer(k: &KnownSnippet) -> Option<Firer> {
    if k.how != "auto" {
        return None;
    }
    match k.kind.as_str() {
        "post" | "pattern" => k.pattern.clone().map(Firer::Pat),
        "suffix" => Some(Firer::Lit(k.trigger.clone(), Boundary::Suffix)),
        "visual" => None,
        _ => Some(Firer::Lit(k.trigger.clone(), Boundary::Word)),
    }
}

fn draft_firer(d: &Draft) -> Option<Firer> {
    if d.fires != Fires::Auto && matches!(d.kind, Kind::Plain | Kind::Pattern) {
        return None;
    }
    match d.kind {
        Kind::Plain => Some(Firer::Lit(d.trigger.clone(), Boundary::Word)),
        Kind::Suffix => Some(Firer::Lit(d.trigger.clone(), Boundary::Suffix)),
        Kind::Post => Some(Firer::Pat(format!("(%a){}", lua_pesc(&d.trigger)))),
        Kind::Pattern => Some(Firer::Pat(d.trigger.clone())),
        Kind::Visual | Kind::RawLua => None,
    }
}

/// helpers.lua's LuaSnip priority, minus the shared 1000: `100 * priority +
/// #trigger` (post: one more for the letter; pat: its `len`, 0 by default).
fn known_rank(k: &KnownSnippet) -> i64 {
    let len = match k.kind.as_str() {
        "post" => k.trigger.len() + 1,
        "pattern" => 0,
        _ => k.trigger.len(),
    };
    100 * k.priority as i64 + len as i64
}

fn draft_rank(d: &Draft) -> i64 {
    let len = match d.kind {
        Kind::Post => d.trigger.len() + 1,
        Kind::Pattern => 0,
        _ => d.trigger.len(),
    };
    100 * d.priority as i64 + len as i64
}

/// What she types to fire a known snippet (None: can't tell, e.g. a pattern).
fn known_typed(k: &KnownSnippet) -> Option<String> {
    match k.kind.as_str() {
        "post" => Some(format!("x{}", k.trigger)),
        "pattern" | "visual" => None,
        _ => Some(k.trigger.clone()),
    }
}

/// What she types to fire the draft.
fn draft_typed(d: &Draft) -> Option<String> {
    match d.kind {
        Kind::Plain | Kind::Suffix => Some(d.trigger.clone()),
        Kind::Post => Some(format!("x{}", d.trigger)),
        Kind::Pattern => Some(if d.eg.is_empty() { pattern_example(&d.trigger) } else { d.eg.clone() }),
        Kind::Visual | Kind::RawLua => None,
    }
}

/// cond.lua's after_backslash / in_command, on bytes.
fn after_backslash(b: &[u8], pos: usize) -> bool {
    codegen_escaped(b, pos)
}

fn in_command(b: &[u8], pos: usize) -> bool {
    let mut k = pos;
    while k > 0 && b[k - 1].is_ascii_alphabetic() {
        k -= 1;
    }
    after_backslash(b, k)
}

/// Does `f` fire at the end of `line` (the text typed so far)?
fn fires_at_end(f: &Firer, line: &str) -> bool {
    match f {
        Firer::Lit(t, bd) => {
            if t.is_empty() || !line.ends_with(t.as_str()) {
                return false;
            }
            let b = line.as_bytes();
            let from = line.len() - t.len();
            let first_letter = starts_with_letter(t);
            let prev_letter = from > 0 && b[from - 1].is_ascii_alphabetic();
            match bd {
                Boundary::Word => !after_backslash(b, from) && !(first_letter && prev_letter),
                Boundary::Suffix => {
                    if first_letter && prev_letter {
                        !in_command(b, from - 1)
                    } else {
                        !after_backslash(b, from)
                    }
                }
            }
        }
        Firer::Pat(p) => matches!(lua_find(line, &format!("{p}$")), Ok(Some(_))),
    }
}

/// The first proper prefix of `typed` at which `f` fires (its length).
fn fires_within(f: &Firer, typed: &str) -> Option<usize> {
    typed.char_indices().map(|(i, _)| i).skip(1).find(|&j| fires_at_end(f, &typed[..j]))
}

/// Where `f` gets in the way of typing `typed`: Some(j < len) at a proper
/// prefix (it always wins: it fires first), Some(len) on the whole of it when
/// `wins` (LuaSnip takes the higher priority; a tie is load order).
fn gets_in_way(f: &Firer, typed: &str, wins: bool) -> Option<usize> {
    fires_within(f, typed).or_else(|| (wins && fires_at_end(f, typed)).then_some(typed.len()))
}

/// "‘sum’ (symbols.lua), ‘sub’ (sets.lua) and 3 more"
fn name_list(items: &[(String, String)]) -> String {
    let shown: Vec<String> = items.iter().take(3).map(|(t, f)| format!("{} ({f})", q(t))).collect();
    let rest = items.len().saturating_sub(3);
    match (shown.len(), rest) {
        (1, _) => shown[0].clone(),
        (_, 0) => format!("{} and {}", shown[..shown.len() - 1].join(", "), shown[shown.len() - 1]),
        _ => format!("{} and {rest} more", shown.join(", ")),
    }
}

fn file_name(k: &KnownSnippet) -> String {
    k.file.rsplit('/').next().unwrap_or(&k.file).to_string()
}

/// A known body for a message: tab stops as `…`, one line, short.
fn short_body(body: &str) -> String {
    let mut s = String::new();
    for p in codegen::parse_body(body) {
        match p {
            Piece::Text(t) => s.push_str(&t),
            Piece::Tab { default, .. } if !default.is_empty() => s.push_str(&default),
            Piece::Tab { .. } => s.push('…'),
            Piece::Exit => {}
            Piece::Capture(k) => s.push_str(&format!("%{k}")),
            Piece::Selection => s.push_str("%V"),
        }
    }
    let one: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > 32 {
        let cut: String = one.chars().take(31).collect();
        format!("{cut}…")
    } else {
        one
    }
}

// ---------------------------------------------------------------------------
// check

fn finding(severity: Severity, code: &str, message: String, fix: Option<Fix>) -> Finding {
    Finding { severity, code: code.into(), message, fix }
}

fn fix(label: &str, d: &Draft, f: impl FnOnce(&mut Draft)) -> Option<Fix> {
    let mut nd = d.clone();
    f(&mut nd);
    Some(Fix { label: label.into(), draft: Some(nd), block: None })
}

/// "Make it Tab-only", for the kinds that can be (plain and pattern).
fn tab_only(d: &Draft) -> Option<Fix> {
    if d.fires == Fires::Tab || !matches!(d.kind, Kind::Plain | Kind::Pattern) {
        return None;
    }
    fix("Make it Tab-only", d, |n| {
        n.fires = Fires::Tab;
        pin(n, "fires");
    })
}

fn severity_rank(s: Severity) -> u8 {
    match s {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Note => 2,
    }
}

/// Common LaTeX command names, for "‘\in’ fires inside \int".
const LATEX_CMDS: &[&str] = &[
    "alpha", "approx", "arccos", "arcsin", "arctan", "bar", "beta", "bigcap", "bigcup", "binom", "boldsymbol", "cap",
    "cdot", "cdots", "chi", "circ", "cos", "cosh", "cot", "cup", "ddot", "delta", "det", "dfrac", "dim", "displaystyle",
    "div", "dot", "dots", "ell", "emptyset", "epsilon", "equiv", "eta", "exists", "exp", "forall", "frac", "gamma", "geq",
    "hat", "hbar", "iff", "implies", "in", "inf", "infty", "int", "iint", "iota", "kappa", "lambda", "land", "langle",
    "ldots", "left", "leftarrow", "leq", "lim", "liminf", "limsup", "ln", "log", "lor", "mapsto", "mathbb", "mathbf",
    "mathcal", "mathrm", "max", "min", "mid", "mu", "nabla", "neq", "nu", "omega", "oint", "operatorname", "otimes",
    "overline", "partial", "phi", "pi", "pm", "prod", "psi", "quad", "rangle", "rho", "right", "rightarrow", "setminus",
    "sigma", "sim", "sin", "sinh", "sqrt", "subset", "subseteq", "sum", "sup", "supset", "tan", "tau", "text", "textbf",
    "theta", "tilde", "times", "to", "underbrace", "underline", "varepsilon", "varphi", "vec", "wedge", "widehat", "xi",
    "zeta",
];

/// Everything wrong or worth knowing, most severe first. `known`: every
/// snippet loaded in ntex (empty until the bridge answers); the draft's own
/// inbox entry (same id) is not a conflict with itself.
///
/// Codes: empty-trigger, empty-output, bad-default, pattern-text, bad-pattern,
/// lua-error (Errors); duplicate, shadows, shadowed-by, english-words (Note
/// in maths), single-letter, trigger-backslash, no-selection, no-capture,
/// selection-unused, capture-unused (Warnings); literal-angle, mirror-default,
/// trailing-newline, pattern-eg, lua-check (Notes).
pub fn check(draft: &Draft, known: &[KnownSnippet]) -> Vec<Finding> {
    let d = draft;
    let mut out: Vec<Finding> = Vec::new();

    if d.kind == Kind::RawLua {
        if d.raw_lua.trim().is_empty() {
            out.push(finding(Severity::Error, "empty-output", "The Lua is empty.".into(), None));
        } else if !lua_reads(&d.raw_lua) {
            out.push(finding(
                Severity::Error,
                "lua-error",
                "The Lua doesn't read: a string, comment or bracket is left open.".into(),
                None,
            ));
        }
        return out;
    }

    let trig = d.trigger.as_str();
    let empty_trigger = trig.trim().is_empty();
    if empty_trigger {
        out.push(finding(Severity::Error, "empty-trigger", "Trigger is empty.".into(), None));
    }
    let empty_output = d.output.iter().all(|p| matches!(p, Piece::Text(t) if t.is_empty()));
    if empty_output {
        out.push(finding(Severity::Error, "empty-output", "Output is empty.".into(), None));
    }

    // tab-stop defaults helpers.parse can't hold
    if let Some(bad) = d.output.iter().find_map(|p| match p {
        Piece::Tab { default, .. } if default.contains(['<', '>', '\n']) => Some(default.clone()),
        _ => None,
    }) {
        let f = fix("Remove them", d, |n| {
            for p in &mut n.output {
                if let Piece::Tab { default, .. } = p {
                    *default = default.replace(['<', '>'], "").replace('\n', " ");
                }
            }
        });
        out.push(finding(
            Severity::Error,
            "bad-default",
            format!("A tab stop's default can't hold <, > or a line break ({}).", q(&bad.replace('\n', "⏎"))),
            f,
        ));
    }

    // patterns
    let mut pattern_caps = None;
    if d.kind == Kind::Pattern {
        if d.context == Context::Text {
            let f = fix("Make it maths", d, |n| {
                n.context = Context::Math;
                pin(n, "context");
            });
            out.push(finding(Severity::Error, "pattern-text", "Pattern triggers work in maths only.".into(), f));
        }
        if !empty_trigger {
            match lua_pattern_check(trig) {
                Ok(n) => pattern_caps = Some(n),
                Err(why) => out.push(finding(
                    Severity::Error,
                    "bad-pattern",
                    format!("{} isn't a Lua pattern: {why}.", q(trig)),
                    None,
                )),
            }
        }
    }

    // selection and captures
    let caps = captures(&d.output);
    match d.kind {
        Kind::Visual if !has_selection(&d.output) => {
            let f = fix("Make it plain", d, |n| {
                n.kind = Kind::Plain;
                pin(n, "kind");
            });
            out.push(finding(
                Severity::Warning,
                "no-selection",
                format!("{} wraps a selection, but the output has no %V: the selection is replaced, not wrapped.", q(trig)),
                f,
            ));
        }
        Kind::Post if !caps.contains(&1) => {
            let f = fix("Make it a suffix", d, |n| {
                n.kind = Kind::Suffix;
                pin(n, "kind");
            });
            out.push(finding(
                Severity::Warning,
                "no-capture",
                format!("{} grabs the letter before it, but the output has no %1: the letter disappears.", q(trig)),
                f,
            ));
        }
        _ => {}
    }
    if d.kind == Kind::Post {
        if let Some(k) = caps.iter().find(|&&k| k > 1) {
            out.push(finding(
                Severity::Warning,
                "no-capture",
                format!("The output uses %{k}, but {} grabs only one letter (%1).", q(trig)),
                None,
            ));
        }
    }
    if let (Kind::Pattern, Some(n)) = (d.kind, pattern_caps) {
        let n = n.max(1); // no parentheses: %1 is the whole match
        if let Some(k) = caps.iter().find(|&&k| k as usize > n) {
            out.push(finding(
                Severity::Warning,
                "no-capture",
                format!("The output uses %{k}, but {} captures only {n}.", q(trig)),
                None,
            ));
        }
    }
    if d.kind != Kind::Visual && has_selection(&d.output) {
        let f = fix("Make it wrap a selection", d, |n| {
            n.kind = Kind::Visual;
            pin(n, "kind");
        });
        out.push(finding(
            Severity::Warning,
            "selection-unused",
            "%V is only filled when the snippet wraps a selection; here it comes out empty.".into(),
            f,
        ));
    }
    if matches!(d.kind, Kind::Plain | Kind::Suffix | Kind::Visual) && !caps.is_empty() {
        let f = if letters(trig) {
            fix("Make it grab the letter before", d, |n| {
                n.kind = Kind::Post;
                pin(n, "kind");
            })
        } else {
            None
        };
        out.push(finding(
            Severity::Warning,
            "capture-unused",
            format!(
                "%{} is only filled when the trigger grabs the letter before it or is a pattern; here it comes out empty.",
                caps[0]
            ),
            f,
        ));
    }

    // conflicts with her snippets
    if !empty_trigger {
        conflicts(d, known, &mut out);
    }

    // the trigger itself
    // suffix has no Tab form: it is always automatic
    let auto = d.kind == Kind::Suffix || (d.kind == Kind::Plain && d.fires == Fires::Auto);
    if !empty_trigger && auto {
        let in_text = draft_where(d) == "text";
        if in_text && starts_with_letter(trig) && (words::is_word(trig) || words::starts_common_words(trig)) {
            let mut ex: Vec<String> = Vec::new();
            if words::is_word(trig) {
                ex.push(trig.to_string());
            }
            ex.extend(words::starting_with(trig, 3 - ex.len()));
            let n = words::count_starting_with(trig) + usize::from(words::is_word(trig));
            let msg = if n == 1 {
                format!("{} is an English word: it fires every time you write it.", q(trig))
            } else {
                format!("{} starts {n} English words ({}, …): it fires as you write them.", q(trig), ex.join(", "))
            };
            out.push(finding(Severity::Warning, "english-words", msg, tab_only(d)));
        } else if !in_text && trig.chars().count() > 1 && letters(trig) && words::is_word(trig) {
            out.push(finding(
                Severity::Note,
                "english-words",
                format!("{} is an English word: in maths it fires whenever you type it (never inside \\text{{…}}).", q(trig)),
                tab_only(d),
            ));
        }
        let mut cs = trig.chars();
        if let (Some(c), None) = (cs.next(), cs.next()) {
            if c.is_alphabetic() {
                let place = if in_text { "in text" } else { "in maths" };
                out.push(finding(
                    Severity::Warning,
                    "single-letter",
                    format!("{} is a single letter: it fires every time you type {c} {place}.", q(trig)),
                    tab_only(d),
                ));
            }
        }
        let b = trig.as_bytes();
        if b.len() > 1 && b[0] == b'\\' && b[1].is_ascii_alphabetic() {
            let stem = &trig[1..];
            let longer: Vec<String> =
                LATEX_CMDS.iter().filter(|c| c.len() > stem.len() && c.starts_with(stem)).take(3).map(|c| format!("\\{c}")).collect();
            let msg = if longer.is_empty() {
                format!(
                    "{} starts with a backslash, so it fires inside any longer command that begins with it; letter snippets keep quiet after a backslash, this one doesn't.",
                    q(trig)
                )
            } else {
                format!(
                    "{} starts with a backslash, so it fires inside longer commands as you type them ({}, …); letter snippets keep quiet after a backslash, this one doesn't.",
                    q(trig),
                    longer.join(", ")
                )
            };
            out.push(finding(Severity::Warning, "trigger-backslash", msg, tab_only(d)));
        }
    }

    // notes
    if let Some(lit) = literal_angle(&d.output) {
        out.push(finding(
            Severity::Note,
            "literal-angle",
            format!("{} in the output is literal text; it is written %{}%> so it isn't read as a tab stop.", q(&lit), &lit[..lit.len() - 1]),
            None,
        ));
    }
    if let Some(n) = mirror_default(&d.output) {
        let f = fix("Move it to the first", d, |nd| codegen::renumber(&mut nd.output));
        out.push(finding(
            Severity::Note,
            "mirror-default",
            format!("Tab stop {n} has a default on a later copy; only the first copy's default is used."),
            f,
        ));
    }
    if codegen::needs_exit(&d.output) {
        out.push(finding(
            Severity::Note,
            "trailing-newline",
            "The output ends with a line break, so the exit point (<0>) goes after it; without one LuaSnip fails.".into(),
            None,
        ));
    }
    if d.kind == Kind::Pattern && d.eg.is_empty() && !empty_trigger && pattern_caps.is_some() {
        let ex = pattern_example(trig);
        let f = fix(&format!("Use ‘{ex}’"), d, |n| n.eg = ex.clone());
        out.push(finding(
            Severity::Note,
            "pattern-eg",
            format!("No example keys: Try it and the reference page type {}, a guess.", q(&ex)),
            f,
        ));
    }
    if !d.check.trim().is_empty() {
        out.push(finding(
            Severity::Note,
            "lua-check",
            "The condition is Lua: it isn't checked until you save, then ntex reports a broken one in one line.".into(),
            None,
        ));
    }

    out.sort_by_key(|f| severity_rank(f.severity));
    out
}

/// duplicate, shadows, shadowed-by.
fn conflicts(d: &Draft, known: &[KnownSnippet], out: &mut Vec<Finding>) {
    let trig = d.trigger.as_str();
    let mine = draft_ident(d);
    let others: Vec<&KnownSnippet> = known.iter().filter(|k| !is_own(d, k) && place_overlap(d, k)).collect();

    // the same trigger, in the same place
    let dups: Vec<&&KnownSnippet> = others.iter().filter(|k| known_ident(k) == mine).collect();
    if let Some(first) = dups.first() {
        let more = if dups.len() > 1 { format!(" (and {} more)", dups.len() - 1) } else { String::new() };
        let origin = if first.origin.is_empty() { String::new() } else { format!(" ({})", first.origin) };
        let body = first.body.as_deref().map(short_body).unwrap_or_else(|| "a computed output".into());
        let msg = format!("{} is already a snippet: {body} in {}{origin}{more}.", q(trig), file_name(first));
        let theirs = dups.iter().map(|k| k.priority).max().unwrap_or(0);
        if d.priority > theirs {
            // settled: hers loses whatever the load order
            out.push(finding(
                Severity::Note,
                "duplicate",
                format!("{} is also in {}; this one wins (priority {}).", q(trig), file_name(first), d.priority),
                None,
            ));
        } else {
            // equal priorities: the winner depends on load order, which differs
            // between now (inbox reloaded last) and after a restart
            let top = theirs + 1;
            let fix_p = fix(&format!("Use priority {top}"), d, |n| n.priority = top);
            // (Tab-only is no fix here: an automatic one still fires first, and
            // between two Tab ones priority decides.)
            out.push(finding(Severity::Warning, "duplicate", msg, fix_p));
        }
    }

    // the draft fires while she types another snippet
    if let Some(f) = draft_firer(d) {
        let (mut before, mut instead): (Vec<(String, String)>, Vec<(String, String)>) = (Vec::new(), Vec::new());
        for k in &others {
            if known_ident(k) == mine {
                continue;
            }
            if let Some(typed) = known_typed(k) {
                if let Some(j) = gets_in_way(&f, &typed, draft_rank(d) >= known_rank(k)) {
                    let item = (k.trigger.clone(), file_name(k));
                    let list = if j < typed.len() { &mut before } else { &mut instead };
                    if !list.contains(&item) {
                        list.push(item);
                    }
                }
            }
        }
        let msg = match (before.is_empty(), instead.is_empty()) {
            (false, true) => {
                let them = if before.len() == 1 { "it stops" } else { "those stop" };
                Some(format!("{} fires before you can finish {}, so {them} working.", q(trig), name_list(&before)))
            }
            (true, false) => Some(format!("{} fires instead of {} on the same keys.", q(trig), name_list(&instead))),
            (false, false) => Some(format!(
                "{} fires before you can finish {}, and instead of {}.",
                q(trig),
                name_list(&before),
                name_list(&instead)
            )),
            (true, true) => None,
        };
        if let Some(msg) = msg {
            out.push(finding(Severity::Warning, "shadows", msg, tab_only(d)));
        }
    }

    // another snippet fires while she types the draft
    if let Some(typed) = draft_typed(d) {
        let mut hit: Vec<(String, String, String)> = Vec::new();
        for k in &others {
            if let Some(f) = known_firer(k) {
                if known_ident(k) == mine {
                    continue;
                }
                if let Some(j) = gets_in_way(&f, &typed, known_rank(k) >= draft_rank(d)) {
                    let item = (k.trigger.clone(), file_name(k), typed[..j].to_string());
                    if !hit.iter().any(|h| h.0 == item.0 && h.1 == item.1) {
                        hit.push(item);
                    }
                }
            }
        }
        if let Some((t, file, at)) = hit.first() {
            let more = if hit.len() > 1 { format!(" ({} more fire on the way too)", hit.len() - 1) } else { String::new() };
            let msg = if *at == typed {
                format!("{} ({file}) fires instead when you type {}, so this one never expands{more}.", q(t), q(&typed))
            } else if *at == *t {
                format!("{} ({file}) fires as you type {}, so {} can never be typed{more}.", q(t), q(&typed), q(&typed))
            } else {
                format!("{} ({file}) fires at {} as you type {}, so {} can never be typed{more}.", q(t), q(at), q(&typed), q(&typed))
            };
            out.push(finding(Severity::Warning, "shadowed-by", msg, None));
        }
    }
}

/// The first literal `<…>` in the output that would read as a tab stop.
fn literal_angle(output: &[Piece]) -> Option<String> {
    for p in output {
        if let Piece::Text(t) = p {
            let c: Vec<char> = t.chars().collect();
            for i in 0..c.len() {
                if let Some(j) = codegen::tabstop_at(&c, i) {
                    return Some(c[i..=j].iter().collect());
                }
            }
        }
    }
    // across pieces (`<:` + %V + `>`): ask codegen
    let b = codegen::body(output);
    let bb = b.as_bytes();
    for i in 0..bb.len().saturating_sub(1) {
        if bb[i] == b'%' && bb[i + 1] == b'<' {
            let mut n = 0;
            let mut k = i;
            while k > 0 && bb[k - 1] == b'%' {
                n += 1;
                k -= 1;
            }
            if n % 2 == 0 {
                return Some("<…>".into());
            }
        }
    }
    None
}

/// A tab stop whose later copy carries a default the first one doesn't use.
fn mirror_default(output: &[Piece]) -> Option<u32> {
    let mut first: HashMap<u32, &str> = HashMap::new();
    for p in output {
        if let Piece::Tab { n, default } = p {
            if *n == 0 {
                continue;
            }
            match first.get(n) {
                None => {
                    first.insert(*n, default);
                }
                Some(f) => {
                    if !default.is_empty() && *f != default.as_str() {
                        return Some(*n);
                    }
                }
            }
        }
    }
    None
}

/// Does entry Lua at least tokenise, with brackets balanced?
fn lua_reads(src: &str) -> bool {
    let Some(toks) = luaparse::lex(src) else { return false };
    let mut depth: i32 = 0;
    for t in toks {
        if let luaparse::K::Punct(p) = t.k {
            match p {
                "(" | "{" | "[" => depth += 1,
                ")" | "}" | "]" => {
                    depth -= 1;
                    if depth < 0 {
                        return false;
                    }
                }
                _ => {}
            }
        }
    }
    depth == 0
}

// ---------------------------------------------------------------------------
// try plan

/// A sensible default for "Try it": where to type and what (`TryRequest`
/// without `entry`, which the caller fills from codegen).
pub fn try_plan(draft: &Draft) -> TryRequest {
    let d = draft;
    if d.kind == Kind::RawLua {
        // a raw entry that reads as a helper call plans like that call
        if let Some(p) = luaparse::parse_entry(d.raw_lua.trim()) {
            return try_plan(&p);
        }
        let keys = luaparse::trigger_of(&d.raw_lua).unwrap_or_else(|| d.trigger.clone());
        return TryRequest {
            entry: String::new(),
            ft: if d.ft == FileType::Markdown { "markdown" } else { "tex" }.into(),
            place: "inline".into(),
            keys,
            tab: false,
            fills: Vec::new(),
        };
    }
    let ft = if d.ft == FileType::Markdown { "markdown" } else { "tex" };
    let place = match d.kind {
        Kind::Visual => "visual",
        _ if draft_where(d) == "text" => match d.line {
            LineRule::Start => "text-start",
            LineRule::ListItem => "list-item",
            _ => "text",
        },
        _ => match d.math_where {
            MathWhere::Display => "display",
            _ => "inline",
        },
    };
    let keys = match d.kind {
        Kind::Plain | Kind::Visual | Kind::RawLua => d.trigger.clone(),
        Kind::Suffix | Kind::Post => format!("x{}", d.trigger),
        Kind::Pattern => {
            if d.eg.is_empty() {
                pattern_example(&d.trigger)
            } else {
                d.eg.clone()
            }
        }
    };
    let tab = d.fires == Fires::Tab && matches!(d.kind, Kind::Plain | Kind::Pattern);
    let mut stops: Vec<u32> = Vec::new();
    for p in &d.output {
        if let Piece::Tab { n, .. } = p {
            if *n > 0 && !stops.contains(n) {
                stops.push(*n);
            }
        }
    }
    let fills = (0..stops.len())
        .map(|i| if i < 26 { ((b'a' + i as u8) as char).to_string() } else { format!("a{i}") })
        .collect();
    TryRequest { entry: String::new(), ft: ft.into(), place: place.into(), keys, tab, fills }
}

// ---------------------------------------------------------------------------
// Lua patterns (lstrlib's matcher, for pattern triggers: shadowing and examples)

/// Lua's `pesc`: escape magic characters so the string matches itself.
pub fn lua_pesc(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        if "^$()%.[]*+-?".contains(c) {
            o.push('%');
        }
        o.push(c);
    }
    o
}

/// `[set]` end: the index after its `]`, as lstrlib's classEnd.
fn class_end(p: &[u8], mut i: usize) -> Option<usize> {
    let c = *p.get(i)?;
    i += 1;
    match c {
        b'%' => {
            p.get(i)?;
            Some(i + 1)
        }
        b'[' => {
            if p.get(i) == Some(&b'^') {
                i += 1;
            }
            loop {
                let c = *p.get(i)?;
                i += 1;
                if c == b'%' {
                    p.get(i)?;
                    i += 1;
                }
                if p.get(i)? == &b']' {
                    return Some(i + 1);
                }
            }
        }
        _ => Some(i),
    }
}

/// Is `p` a valid Lua pattern? Ok(number of captures) or Err(why).
pub fn lua_pattern_check(p: &str) -> Result<usize, String> {
    let b = p.as_bytes();
    let mut i = usize::from(b.first() == Some(&b'^'));
    let mut open: Vec<usize> = Vec::new();
    let mut closed: Vec<usize> = Vec::new();
    let mut ncap = 0;
    while i < b.len() {
        match b[i] {
            b'(' => {
                if b.get(i + 1) == Some(&b')') {
                    closed.push(ncap);
                    ncap += 1;
                    i += 2;
                    continue;
                }
                open.push(ncap);
                ncap += 1;
                i += 1;
            }
            b')' => {
                match open.pop() {
                    Some(c) => closed.push(c),
                    None => return Err("a ‘)’ with no ‘(’".into()),
                }
                i += 1;
            }
            b'%' => match b.get(i + 1) {
                None => return Err("it ends with %".into()),
                Some(b'b') => {
                    if b.len() < i + 4 {
                        return Err("%b needs two characters".into());
                    }
                    i += 4;
                }
                Some(b'f') => {
                    if b.get(i + 2) != Some(&b'[') {
                        return Err("%f needs a [set]".into());
                    }
                    i = class_end(b, i + 2).ok_or("a ‘[’ with no ‘]’")?;
                }
                Some(d) if d.is_ascii_digit() => {
                    let k = (d - b'0') as usize;
                    if k == 0 || !closed.contains(&(k - 1)) {
                        return Err(format!("%{k} refers to no finished capture"));
                    }
                    i += 2;
                }
                _ => i += 2,
            },
            b'[' => i = class_end(b, i).ok_or("a ‘[’ with no ‘]’")?,
            _ => i += 1,
        }
        if ncap > 32 {
            return Err("too many captures".into());
        }
    }
    if !open.is_empty() {
        return Err("a ‘(’ with no ‘)’".into());
    }
    Ok(ncap)
}

const CAP_UNF: isize = -1;
const CAP_POS: isize = -2;

struct Ms<'a> {
    src: &'a [u8],
    pat: &'a [u8],
    level: usize,
    cap: [(usize, isize); 32],
    depth: usize,
}

fn class_match(c: u8, cl: u8) -> bool {
    let r = match cl.to_ascii_lowercase() {
        b'a' => c.is_ascii_alphabetic(),
        b'c' => c.is_ascii_control(),
        b'd' => c.is_ascii_digit(),
        b'g' => c.is_ascii_graphic(),
        b'l' => c.is_ascii_lowercase(),
        b'p' => c.is_ascii_punctuation(),
        b's' => c.is_ascii_whitespace() || c == 0x0b,
        b'u' => c.is_ascii_uppercase(),
        b'w' => c.is_ascii_alphanumeric(),
        b'x' => c.is_ascii_hexdigit(),
        _ => return cl == c,
    };
    if cl.is_ascii_uppercase() { !r } else { r }
}

impl Ms<'_> {
    /// `p` at `[`, `ec` at its `]`.
    fn bracket(&self, c: u8, p: usize, ec: usize) -> bool {
        let pat = self.pat;
        let mut sig = true;
        let mut p = p + 1;
        if pat[p] == b'^' {
            sig = false;
            p += 1;
        }
        while p < ec {
            if pat[p] == b'%' && p + 1 < ec {
                if class_match(c, pat[p + 1]) {
                    return sig;
                }
                p += 2;
            } else if pat.get(p + 1) == Some(&b'-') && p + 2 < ec {
                if pat[p] <= c && c <= pat[p + 2] {
                    return sig;
                }
                p += 3;
            } else {
                if pat[p] == c {
                    return sig;
                }
                p += 1;
            }
        }
        !sig
    }

    fn single(&self, s: usize, p: usize, ep: usize) -> bool {
        let Some(&c) = self.src.get(s) else { return false };
        match self.pat[p] {
            b'.' => true,
            b'%' => class_match(c, self.pat[p + 1]),
            b'[' => self.bracket(c, p, ep - 1),
            x => x == c,
        }
    }

    fn do_match(&mut self, s: usize, p: usize) -> Option<usize> {
        self.depth += 1;
        let r = if self.depth > 4000 { None } else { self.match_inner(s, p) };
        self.depth -= 1;
        r
    }

    fn match_inner(&mut self, mut s: usize, mut p: usize) -> Option<usize> {
        loop {
            if p >= self.pat.len() {
                return Some(s);
            }
            match self.pat[p] {
                b'(' => {
                    return if self.pat.get(p + 1) == Some(&b')') {
                        self.start_capture(s, p + 2, CAP_POS)
                    } else {
                        self.start_capture(s, p + 1, CAP_UNF)
                    };
                }
                b')' => return self.end_capture(s, p + 1),
                b'$' if p + 1 == self.pat.len() => return if s == self.src.len() { Some(s) } else { None },
                b'%' if self.pat.get(p + 1) == Some(&b'b') => {
                    s = self.balance(s, p + 2)?;
                    p += 4;
                    continue;
                }
                b'%' if self.pat.get(p + 1) == Some(&b'f') => {
                    p += 2;
                    let ep = class_end(self.pat, p)?;
                    let prev = if s == 0 { 0 } else { self.src[s - 1] };
                    let cur = self.src.get(s).copied().unwrap_or(0);
                    if !self.bracket(prev, p, ep - 1) && self.bracket(cur, p, ep - 1) {
                        p = ep;
                        continue;
                    }
                    return None;
                }
                b'%' if self.pat.get(p + 1).is_some_and(|c| c.is_ascii_digit()) => {
                    s = self.back_ref(s, self.pat[p + 1])?;
                    p += 2;
                    continue;
                }
                _ => {
                    let ep = class_end(self.pat, p)?;
                    let m = self.single(s, p, ep);
                    match self.pat.get(ep) {
                        Some(b'?') => {
                            if m {
                                if let Some(r) = self.do_match(s + 1, ep + 1) {
                                    return Some(r);
                                }
                            }
                            p = ep + 1;
                            continue;
                        }
                        Some(b'+') => return if m { self.max_expand(s + 1, p, ep) } else { None },
                        Some(b'*') => return self.max_expand(s, p, ep),
                        Some(b'-') => return self.min_expand(s, p, ep),
                        _ => {
                            if !m {
                                return None;
                            }
                            s += 1;
                            p = ep;
                        }
                    }
                }
            }
        }
    }

    fn max_expand(&mut self, s: usize, p: usize, ep: usize) -> Option<usize> {
        let mut i = 0;
        while self.single(s + i, p, ep) {
            i += 1;
        }
        loop {
            if let Some(r) = self.do_match(s + i, ep + 1) {
                return Some(r);
            }
            if i == 0 {
                return None;
            }
            i -= 1;
        }
    }

    fn min_expand(&mut self, mut s: usize, p: usize, ep: usize) -> Option<usize> {
        loop {
            if let Some(r) = self.do_match(s, ep + 1) {
                return Some(r);
            }
            if self.single(s, p, ep) {
                s += 1;
            } else {
                return None;
            }
        }
    }

    fn start_capture(&mut self, s: usize, p: usize, what: isize) -> Option<usize> {
        if self.level >= self.cap.len() {
            return None;
        }
        self.cap[self.level] = (s, what);
        self.level += 1;
        let r = self.do_match(s, p);
        if r.is_none() {
            self.level -= 1;
        }
        r
    }

    fn end_capture(&mut self, s: usize, p: usize) -> Option<usize> {
        let l = (0..self.level).rev().find(|&l| self.cap[l].1 == CAP_UNF)?;
        self.cap[l].1 = (s - self.cap[l].0) as isize;
        let r = self.do_match(s, p);
        if r.is_none() {
            self.cap[l].1 = CAP_UNF;
        }
        r
    }

    fn balance(&self, s: usize, p: usize) -> Option<usize> {
        let (b, e) = (*self.pat.get(p)?, *self.pat.get(p + 1)?);
        if self.src.get(s) != Some(&b) {
            return None;
        }
        let mut cont = 1;
        let mut i = s + 1;
        while i < self.src.len() {
            let c = self.src[i];
            if c == e {
                cont -= 1;
                if cont == 0 {
                    return Some(i + 1);
                }
            } else if c == b {
                cont += 1;
            }
            i += 1;
        }
        None
    }

    fn back_ref(&self, s: usize, d: u8) -> Option<usize> {
        let l = (d as usize).checked_sub(b'1' as usize)?;
        if l >= self.level || self.cap[l].1 < 0 {
            return None;
        }
        let (st, len) = (self.cap[l].0, self.cap[l].1 as usize);
        let want = &self.src[st..st + len];
        if self.src.len() - s >= len && &self.src[s..s + len] == want { Some(s + len) } else { None }
    }

    fn captures(&self, s: usize, e: usize) -> Vec<String> {
        if self.level == 0 {
            return vec![String::from_utf8_lossy(&self.src[s..e]).into_owned()];
        }
        (0..self.level)
            .map(|l| {
                let (st, len) = self.cap[l];
                if len == CAP_POS {
                    (st + 1).to_string()
                } else {
                    String::from_utf8_lossy(&self.src[st..st + len.max(0) as usize]).into_owned()
                }
            })
            .collect()
    }
}

/// Lua's `string.find(s, p)`: Ok(Some((start, end, captures))) with byte
/// offsets (end exclusive), Ok(None) for no match, Err for a bad pattern.
pub fn lua_find(s: &str, p: &str) -> Result<Option<(usize, usize, Vec<String>)>, String> {
    lua_pattern_check(p)?;
    let pat = p.as_bytes();
    let (anchor, p0) = if pat.first() == Some(&b'^') { (true, 1) } else { (false, 0) };
    let mut ms = Ms { src: s.as_bytes(), pat, level: 0, cap: [(0, 0); 32], depth: 0 };
    let mut st = 0;
    loop {
        ms.level = 0;
        if let Some(e) = ms.do_match(st, p0) {
            return Ok(Some((st, e, ms.captures(st, e))));
        }
        st += 1;
        if anchor || st > s.len() {
            return Ok(None);
        }
    }
}

/// Keys that a pattern trigger matches: `pa(%a)(%a)` → `paxy`, `(%a)(%d)` →
/// `x2`. Falls back to the pattern itself when the guess doesn't match.
pub fn pattern_example(p: &str) -> String {
    let b = p.as_bytes();
    let letters = ["x", "y", "z", "w", "u", "v"];
    let digits = ["2", "3", "4", "5"];
    let (mut nl, mut nd) = (0, 0);
    let mut out = String::new();
    let mut i = usize::from(b.first() == Some(&b'^'));
    while i < b.len() {
        let c = b[i];
        match c {
            b'(' | b')' | b'*' | b'+' | b'-' | b'?' => i += 1,
            b'$' if i + 1 == b.len() => i += 1,
            b'%' => {
                let Some(&k) = b.get(i + 1) else { break };
                match k {
                    b'a' | b'l' | b'w' | b'g' => {
                        out.push_str(letters[nl % letters.len()]);
                        nl += 1;
                    }
                    b'u' => out.push('X'),
                    b'd' => {
                        out.push_str(digits[nd % digits.len()]);
                        nd += 1;
                    }
                    b'x' => out.push('a'),
                    b's' => out.push(' '),
                    b'p' => out.push('.'),
                    b'b' => {
                        if let (Some(&x), Some(&y)) = (b.get(i + 2), b.get(i + 3)) {
                            out.push(x as char);
                            out.push(y as char);
                        }
                        i += 2;
                    }
                    b'f' => {
                        i = class_end(b, i + 2).unwrap_or(b.len());
                        continue;
                    }
                    k if k.is_ascii_alphanumeric() => {}
                    k => out.push(k as char),
                }
                i += 2;
            }
            b'[' => {
                let end = class_end(b, i).unwrap_or(b.len());
                let ms = Ms { src: &[], pat: b, level: 0, cap: [(0, 0); 32], depth: 0 };
                if let Some(ch) = b"xyzab23 .,X(".iter().find(|&&ch| ms.bracket(ch, i, end - 1)) {
                    out.push(*ch as char);
                }
                i = end;
            }
            b'.' => {
                out.push_str(letters[nl % letters.len()]);
                nl += 1;
                i += 1;
            }
            _ => {
                // a whole UTF-8 character
                let ch_len = p[i..].chars().next().map(char::len_utf8).unwrap_or(1);
                out.push_str(&p[i..i + ch_len]);
                i += ch_len;
            }
        }
    }
    match lua_find(&out, &format!("{p}$")) {
        Ok(Some(_)) => out,
        _ => p.to_string(),
    }
}
