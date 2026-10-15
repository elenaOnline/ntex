//! .fig text → Figure + diagnostics (FIGURES.md "The shorthand"). Never fails:
//! bad lines become diagnostics and are skipped.
//!
//! Two phases. Phase 1 reads every line into raw statements (so a name may be
//! used before the line that defines it). Phase 2 resolves names against all
//! declarations in the file and emits the items in file order. A statement
//! that can't be read is kept verbatim as a `Comment` item (its text doesn't
//! start with `%`), so formatting never loses what she typed.
use crate::model::*;
use crate::unicode;
use std::collections::{HashMap, HashSet};

pub const KEYWORDS: &[&str] = &["grid", "end", "blob", "ngon", "cayley", "text", "scale", "spacing", "size"];
pub const PALETTE: &[&str] = &["blue", "violet", "teal", "maroon", "green", "ochre", "grey", "ink"];

pub fn parse(src: &str) -> (Figure, Vec<Diagnostic>) {
    let mut p = Phase1::default();
    p.read(src);
    phase2(p)
}

// ===========================================================================
// shared lexical helpers (also used by format and edit)

/// The label a token gets when none is given.
pub fn default_label(name: &str) -> String {
    unicode::to_latex(name)
}

/// Registry key: `φ` and `\varphi` name the same thing.
pub(crate) fn canon(name: &str) -> String {
    unicode::to_latex(name.trim())
}

/// Is `s` usable as a name as written: non-empty, no whitespace, none of
/// `,;{}"%` outside balanced braces, not an arrow, not a keyword.
pub fn is_token(s: &str) -> bool {
    if s.is_empty() || KEYWORDS.contains(&s) || s == "at" || s == "in" {
        return false;
    }
    let mut depth = 0i32;
    let mut esc = false;
    for c in s.chars() {
        if esc {
            esc = false;
            continue;
        }
        match c {
            '\\' => esc = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            '"' | '%' => return false,
            ',' | ';' if depth == 0 => return false,
            c if c.is_whitespace() => return false,
            _ => {}
        }
    }
    if depth != 0 || esc {
        return false;
    }
    if s.starts_with('(') {
        return false;
    }
    parse_arrow(s, false).is_none() && compact_split(s).len() == 1
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Chunk {
    pub text: String,
    /// 0-based char column of the first char
    pub col: usize,
    /// exclusive end column
    pub end: usize,
}

impl Chunk {
    fn is_quoted(&self) -> bool {
        self.text.len() >= 2 && self.text.starts_with('"') && self.text.ends_with('"')
    }
    fn unquote(&self) -> String {
        self.text[1..self.text.len() - 1].to_string()
    }
}

/// (figure-qol class C) A quoted label right after an option: `axis 1 "s"`,
/// `rotation "ρ"`. Takes it and moves past it.
fn take_quoted(cs: &[Chunk], i: &mut usize) -> Option<String> {
    let c = cs.get(*i)?;
    if c.text.len() >= 2 && c.text.starts_with('"') && c.text.ends_with('"') {
        *i += 1;
        Some(c.unquote())
    } else {
        None
    }
}

/// Split into whitespace-separated chunks. Quotes are atomic (may hold
/// spaces), braces group (`\mathbb{ Z }`, `{e, g}`), and a chunk starting
/// with `(` runs to its matching `)`.
pub(crate) fn chunks(line: &[char]) -> Vec<Chunk> {
    let mut out = Vec::new();
    let mut i = 0;
    let n = line.len();
    while i < n {
        if line[i].is_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        let paren_mode = line[i] == '(';
        let mut depth = 0i32;
        let mut pdepth = 0i32;
        while i < n {
            let c = line[i];
            if c == '\\' {
                i += 2;
                continue;
            }
            if c == '"' {
                i += 1;
                while i < n && line[i] != '"' {
                    i += 1;
                }
                i += 1;
                continue;
            }
            if c == '{' {
                depth += 1;
            } else if c == '}' {
                if depth > 0 {
                    depth -= 1;
                }
            } else if paren_mode && c == '(' {
                pdepth += 1;
            } else if paren_mode && c == ')' {
                pdepth -= 1;
            } else if c.is_whitespace() && depth == 0 && pdepth <= 0 {
                break;
            }
            i += 1;
        }
        let end = i.min(n);
        out.push(Chunk { text: line[start..end].iter().collect(), col: start, end });
        i = end;
    }
    out
}

/// Split a line into statements at `;` (outside quotes and braces, not `\;`)
/// and cut the `%` comment. Returns (statements with their start column,
/// the comment with its column).
pub(crate) fn split_line(line: &[char]) -> (Vec<(usize, String)>, Option<(usize, String)>) {
    let mut stmts: Vec<(usize, String)> = Vec::new();
    let mut start = 0;
    let mut depth = 0i32;
    let mut in_q = false;
    let mut i = 0;
    let mut comment = None;
    while i < line.len() {
        let c = line[i];
        if in_q {
            if c == '"' {
                in_q = false;
            }
            i += 1;
            continue;
        }
        match c {
            '\\' => {
                i += 2;
                continue;
            }
            '"' => in_q = true,
            '{' => depth += 1,
            '}' => depth = (depth - 1).max(0),
            ';' if depth == 0 => {
                stmts.push((start, line[start..i].iter().collect()));
                start = i + 1;
            }
            '%' => {
                comment = Some((i, line[i..].iter().collect::<String>().trim_end().to_string()));
                break;
            }
            _ => {}
        }
        i += 1;
    }
    let end = match &comment {
        Some((c, _)) => *c,
        None => line.len(),
    };
    if start <= end {
        stmts.push((start, line[start.min(end)..end].iter().collect()));
    }
    // trim, keeping columns
    let stmts = stmts
        .into_iter()
        .filter_map(|(col, s)| {
            let lead = s.chars().take_while(|c| c.is_whitespace()).count();
            let t = s.trim();
            if t.is_empty() { None } else { Some((col + lead, t.to_string())) }
        })
        .collect();
    (stmts, comment)
}

/// An arrow as written.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ArrowTok {
    pub head: Head,
    pub line: Line,
    /// as written (quotes removed), not yet converted from Unicode
    pub label: Option<String>,
    /// written right-to-left (`<-f-`): from and to swap
    pub reversed: bool,
}

const PLACEHOLDER: char = '\u{E000}';

/// Parse one chunk as an arrow (FIGURES.md's table). In `grid` mode a run of
/// `-` is solid at any length (`--f-->`), as ASCII art wants.
pub(crate) fn parse_arrow(s: &str, grid: bool) -> Option<ArrowTok> {
    let simple = |head, line, reversed| Some(ArrowTok { head, line, label: None, reversed });
    match s {
        "→" | "⟶" => return simple(Head::To, Line::Solid, false),
        "↦" | "⟼" => return simple(Head::MapsTo, Line::Solid, false),
        "↠" => return simple(Head::TwoHeads, Line::Solid, false),
        "↣" => return simple(Head::Tail, Line::Solid, false),
        "↪" => return simple(Head::Hook, Line::Solid, false),
        "⇒" => return simple(Head::Double, Line::Solid, false),
        "⇢" => return simple(Head::To, Line::Dashed, false),
        "↔" | "⟷" => return simple(Head::Both, Line::Solid, false),
        "←" | "⟵" => return simple(Head::To, Line::Solid, true),
        _ => {}
    }
    // a quoted label becomes a placeholder
    let mut quoted: Option<String> = None;
    let t: String = if let Some(q1) = s.find('"') {
        let rest = &s[q1 + 1..];
        let q2 = rest.find('"')?;
        quoted = Some(rest[..q2].to_string());
        let after = &rest[q2 + 1..];
        if after.contains('"') {
            return None;
        }
        format!("{}{}{}", &s[..q1], PLACEHOLDER, after)
    } else {
        s.to_string()
    };
    let cs: Vec<char> = t.chars().collect();
    if cs.is_empty() {
        return None;
    }
    let mut a = 0;
    let mut b = cs.len();
    let (mut tail, mut maps, mut rev) = (false, false, false);
    if cs[0] == '>' && cs.len() > 1 {
        tail = true;
        a = 1;
    } else if cs[0] == '|' {
        maps = true;
        a = 1;
    } else if cs[0] == '<' {
        rev = true;
        a = 1;
    }
    let mut head: Option<Head> = None;
    if b >= a + 2 && cs[b - 1] == '>' && cs[b - 2] == '>' {
        head = Some(Head::TwoHeads);
        b -= 2;
    } else if b > a && cs[b - 1] == '>' {
        head = Some(Head::To);
        b -= 1;
    }
    let mut hook = false;
    let body_s: String = cs[a..b].iter().collect();
    let mut body: Vec<char> = body_s.chars().collect();
    if head == Some(Head::To) && body_s.ends_with("hook-") && !tail && !maps && !rev {
        hook = true;
        body.truncate(body.len() - 5);
        if body.is_empty() {
            body = vec!['-'];
        }
    }
    if body.is_empty() {
        return None;
    }
    let c = body[0];
    if !matches!(c, '-' | '.' | '=') || *body.last().unwrap() != c {
        return None;
    }
    let k1 = body.iter().take_while(|&&x| x == c).count();
    let (label, n_or_k1): (Option<String>, usize) = if k1 == body.len() {
        (None, body.len())
    } else {
        let k2 = body.iter().rev().take_while(|&&x| x == c).count();
        let mid: String = body[k1..body.len() - k2].iter().collect();
        if mid.is_empty() {
            return None;
        }
        (Some(mid), k1)
    };
    // the placeholder must be the whole label, if present
    let label = match (label, quoted) {
        (Some(l), Some(q)) => {
            if l == PLACEHOLDER.to_string() { Some(q) } else { return None }
        }
        (Some(l), None) => {
            if l.contains(PLACEHOLDER) { return None }
            Some(l)
        }
        (None, Some(_)) => return None,
        (None, None) => None,
    };
    if hook && c != '-' {
        return None;
    }
    let mut line = match c {
        '.' => Line::Dotted,
        '-' => {
            if !grid && n_or_k1 >= 2 { Line::Dashed } else { Line::Solid }
        }
        _ => Line::Solid,
    };
    let headk = if c == '=' {
        if tail || maps || hook {
            return None;
        }
        match (head, rev) {
            (Some(Head::To), false) => Head::Double,
            (Some(Head::To), true) => return None,
            (None, true) => {
                return Some(ArrowTok { head: Head::Double, line, label, reversed: true });
            }
            (None, false) => Head::Equal,
            _ => return None,
        }
    } else if hook {
        Head::Hook
    } else if tail {
        if head != Some(Head::To) {
            return None;
        }
        Head::Tail
    } else if maps {
        if head != Some(Head::To) {
            return None;
        }
        Head::MapsTo
    } else if rev {
        match head {
            Some(Head::To) => Head::Both,
            None => {
                return Some(ArrowTok { head: Head::To, line, label, reversed: true });
            }
            _ => return None,
        }
    } else {
        head?
    };
    if headk == Head::Equal {
        line = Line::Solid;
    }
    Some(ArrowTok { head: headk, line, label, reversed: false })
}

/// Arrow spellings that may sit inside a chunk with no spaces (`A->B`).
const COMPACT: &[&str] = &[
    "|->", "->>", "-->", "<->", "->", "<-", "=>", "..>", "→", "↦", "↠", "↪", "⇒", "↔", "←", "⟶", "⟼", "↣", "⇢",
];

/// "A->B" → ["A", "->", "B"] (offsets in chars). Chunks that are a whole
/// arrow, or quoted, are left alone.
pub(crate) fn compact_split(s: &str) -> Vec<(usize, String)> {
    let cs: Vec<char> = s.chars().collect();
    if parse_arrow(s, false).is_some() || s.starts_with('"') {
        return vec![(0, s.to_string())];
    }
    let mut depth = 0i32;
    let mut i = 0;
    let mut in_q = false;
    while i < cs.len() {
        let c = cs[i];
        if in_q {
            if c == '"' {
                in_q = false;
            }
            i += 1;
            continue;
        }
        if c == '\\' {
            i += 2;
            continue;
        }
        if c == '"' {
            in_q = true;
        } else if c == '{' {
            depth += 1;
        } else if c == '}' {
            depth = (depth - 1).max(0);
        } else if depth == 0 {
            for pat in COMPACT {
                let pc: Vec<char> = pat.chars().collect();
                if i + pc.len() <= cs.len() && cs[i..i + pc.len()] == pc[..] {
                    let mut out = Vec::new();
                    if i > 0 {
                        out.push((0, cs[..i].iter().collect()));
                    }
                    out.push((i, pat.to_string()));
                    let rest: String = cs[i + pc.len()..].iter().collect();
                    if !rest.is_empty() {
                        for (o, r) in compact_split(&rest) {
                            out.push((i + pc.len() + o, r));
                        }
                    }
                    return out;
                }
            }
        }
        i += 1;
    }
    vec![(0, s.to_string())]
}

fn num(s: &str) -> Option<f32> {
    let s = s.trim().replace('−', "-");
    let v: f32 = s.parse().ok()?;
    if v.is_finite() { Some(v) } else { None }
}

fn pair(s: &str) -> Option<(f32, f32)> {
    let s = s.trim();
    let parts: Vec<&str> = if s.contains(',') { s.split(',').collect() } else { s.split('x').collect() };
    if parts.len() != 2 {
        return None;
    }
    Some((num(parts[0])?, num(parts[1])?))
}

/// After `at`: `1,0` / `1, 0` / `1 0` (cells) or `(3.2,-1)` (absolute).
fn take_pos(cs: &[Chunk], i: &mut usize) -> Option<Pos> {
    let first = cs.get(*i)?;
    if first.text.starts_with('(') {
        let t = first.text.trim_start_matches('(').trim_end_matches(')');
        let (x, y) = pair(t)?;
        *i += 1;
        return Some(Pos::Abs(x, y));
    }
    let mut s = first.text.clone();
    let mut j = *i + 1;
    loop {
        if s.ends_with(',') {
            if let Some(n) = cs.get(j) {
                s.push_str(&n.text);
                j += 1;
                continue;
            }
        }
        if let Some(n) = cs.get(j) {
            if n.text.starts_with(',') {
                s.push_str(&n.text);
                j += 1;
                continue;
            }
        }
        break;
    }
    if !s.contains(',') {
        if let (Some(_), Some(n)) = (num(&s), cs.get(j)) {
            if num(&n.text).is_some() {
                s = format!("{},{}", s, n.text);
                j += 1;
            }
        }
    }
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        return None;
    }
    let (x, y) = (num(parts[0])?, num(parts[1])?);
    *i = j;
    Some(Pos::Cell(x, y))
}

/// `key=value`, or `key value` when the next chunk exists.
fn key_value(cs: &[Chunk], i: &mut usize, key: &str, allow_space: bool) -> Option<Option<String>> {
    let t = &cs[*i].text;
    if let Some(rest) = t.strip_prefix(key) {
        if let Some(v) = rest.strip_prefix('=') {
            *i += 1;
            let mut v = v.to_string();
            // `size=2, 1` → join
            while v.ends_with(',') && *i < cs.len() {
                v.push_str(&cs[*i].text);
                *i += 1;
            }
            return Some(Some(v));
        }
        if rest.is_empty() {
            *i += 1;
            if allow_space {
                if let Some(n) = cs.get(*i) {
                    let mut v = n.text.clone();
                    *i += 1;
                    while v.ends_with(',') && *i < cs.len() {
                        v.push_str(&cs[*i].text);
                        *i += 1;
                    }
                    return Some(Some(v));
                }
            }
            return Some(None);
        }
    }
    None
}

/// Split at commas outside quotes/braces/parens, then re-join `at 1` + `0`.
fn comma_list(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let (mut depth, mut pd, mut in_q) = (0i32, 0i32, false);
    let mut esc = false;
    for c in s.chars() {
        if esc {
            cur.push(c);
            esc = false;
            continue;
        }
        if in_q {
            if c == '"' {
                in_q = false;
            }
            cur.push(c);
            continue;
        }
        match c {
            '\\' => esc = true,
            '"' => in_q = true,
            '{' => depth += 1,
            '}' => depth -= 1,
            '(' => pd += 1,
            ')' => pd -= 1,
            ',' if depth <= 0 && pd <= 0 => {
                parts.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    parts.push(cur);
    let mut out: Vec<String> = Vec::new();
    for p in parts {
        let t = p.trim().to_string();
        if let Some(last) = out.last_mut() {
            let lw: Vec<&str> = last.split_whitespace().collect();
            if num(&t).is_some() && lw.len() >= 2 && lw[lw.len() - 2] == "at" && num(lw[lw.len() - 1]).is_some() {
                last.push(',');
                last.push_str(&t);
                continue;
            }
        }
        out.push(t);
    }
    out
}

/// `45`, `-30`, `22.5°`, `90deg` → degrees.
fn angle_value(v: &str) -> Option<f32> {
    let v = v.trim();
    let v = v.strip_suffix('°').or_else(|| v.strip_suffix("deg")).unwrap_or(v);
    num(v)
}

/// (class E) The sizes `text … size=…` knows (`normal` is the default).
pub const TEXT_SIZES: &[&str] = &["tiny", "small", "normal", "large", "huge"];

fn color_name(v: &str) -> Option<String> {
    let v = v.trim().to_lowercase();
    let v = if v == "gray" { "grey".to_string() } else { v };
    if PALETTE.contains(&v.as_str()) { Some(v) } else { None }
}

// ===========================================================================
// phase 1: lines → raw statements

#[derive(Clone, Debug)]
struct Term {
    name: String,
    label: Option<String>,
    pos: Option<Pos>,
    col: usize,
}

#[derive(Clone, Debug, Default)]
struct ArrowOpts {
    bend: Option<f32>,
    is_loop: bool,
    swap: bool,
    iso: bool,
    line: Option<Line>,
    thick: bool,
    color: Option<String>,
    label_pos: LabelPos,
}

#[derive(Clone, Debug)]
struct Chain {
    terms: Vec<Term>,
    arrows: Vec<ArrowTok>,
    opts: ArrowOpts,
    node_color: Option<String>,
}

#[derive(Clone, Debug)]
struct ElemRaw {
    name: String,
    label: Option<String>,
    pos: Option<Pos>,
}

#[derive(Clone, Debug)]
struct BlobRaw {
    blob: Blob,
    elems: Vec<ElemRaw>,
    parent: Option<String>,
}

#[derive(Clone, Debug)]
enum SK {
    Blank,
    Comment(String),
    Bad(String),
    Grid { source: String, g: GridParsed },
    Chain(Chain),
    Blob(BlobRaw),
    NGon { ngon: NGon, named: bool },
    Cayley { c: Cayley, named: bool },
    Text(Text),
    Global,
}

#[derive(Clone, Debug)]
struct Stmt {
    line: usize,
    col: usize,
    raw: String,
    kind: SK,
    /// (FIG-POLISH) the label conversions read in this statement: (latex, as typed)
    spell: Vec<(String, String)>,
}

#[derive(Default)]
struct Phase1 {
    stmts: Vec<Stmt>,
    diags: Vec<Diagnostic>,
    globals: Globals,
    /// (FIG-POLISH) conversions of the statement being read
    spell: Vec<(String, String)>,
}

fn diag(sev: Severity, line: usize, col: usize, msg: impl AsRef<str>) -> Diagnostic {
    Diagnostic { severity: sev, span: Span { line, col }, message: format!("line {line}: {}", msg.as_ref()) }
}

fn expand_tabs(s: &str) -> Vec<char> {
    let mut out = Vec::new();
    for c in s.chars() {
        if c == '\t' {
            let n = 8 - out.len() % 8;
            out.extend(std::iter::repeat_n(' ', n));
        } else if c != '\r' {
            out.push(c);
        }
    }
    out
}

fn first_word(s: &str) -> &str {
    s.split(|c: char| c.is_whitespace() || c == '=').next().unwrap_or("")
}

impl Phase1 {
    fn err(&mut self, line: usize, col: usize, msg: impl AsRef<str>) {
        self.diags.push(diag(Severity::Error, line, col, msg));
    }
    fn warn(&mut self, line: usize, col: usize, msg: impl AsRef<str>) {
        self.diags.push(diag(Severity::Warning, line, col, msg));
    }
    fn note(&mut self, line: usize, col: usize, msg: impl AsRef<str>) {
        self.diags.push(diag(Severity::Note, line, col, msg));
    }

    /// A label as typed → LaTeX, remembering how she spelled it.
    fn tl(&mut self, raw: &str) -> String {
        let l = unicode::to_latex(raw);
        self.spell.push((l.clone(), raw.to_string()));
        l
    }

    fn read(&mut self, src: &str) {
        let src = src.strip_prefix('\u{feff}').unwrap_or(src);
        let lines: Vec<&str> = src.lines().collect();
        let mut li = 0;
        while li < lines.len() {
            let lineno = li + 1;
            let chars = expand_tabs(lines[li]);
            let (stmts, comment) = split_line(&chars);
            if stmts.is_empty() && comment.is_none() {
                self.stmts.push(Stmt { line: lineno, col: 1, raw: String::new(), kind: SK::Blank, spell: vec![] });
                li += 1;
                continue;
            }
            // a grid block
            if stmts.len() == 1 && first_word(&stmts[0].1) == "grid" {
                let rest = stmts[0].1["grid".len()..].trim().to_string();
                if !rest.is_empty() {
                    self.warn(lineno, stmts[0].0 + 1, format!("`grid` takes nothing after it; `{rest}` is ignored."));
                }
                let mut body: Vec<(usize, String)> = Vec::new();
                let mut src_lines = vec![lines[li].trim_end_matches('\r').to_string()];
                let mut j = li + 1;
                let mut closed = false;
                while j < lines.len() {
                    let cj = expand_tabs(lines[j]);
                    let (sj, _) = split_line(&cj);
                    let w = sj.first().map(|s| s.1.clone()).unwrap_or_default();
                    if sj.len() == 1 && w == "end" {
                        src_lines.push(lines[j].trim_end_matches('\r').to_string());
                        closed = true;
                        j += 1;
                        break;
                    }
                    let fw = first_word(&w);
                    if sj.len() == 1 && matches!(fw, "grid" | "blob" | "ngon" | "cayley") {
                        break;
                    }
                    body.push((j + 1, lines[j].trim_end_matches('\r').to_string()));
                    src_lines.push(lines[j].trim_end_matches('\r').to_string());
                    j += 1;
                }
                if !closed {
                    // drop trailing blank lines from the block, then close it
                    while body.last().map(|b| b.1.trim().is_empty()).unwrap_or(false) {
                        body.pop();
                        src_lines.pop();
                        j -= 1;
                    }
                    self.err(lineno, 1, format!("this `grid` has no `end`; it stops after line {}.", j));
                    src_lines.push("end".to_string());
                }
                let g = parse_grid(&body);
                self.diags.extend(g.diags.iter().cloned());
                let mut spell = Vec::new();
                for l in g.nodes.iter().filter_map(|n| n.label.as_ref()).chain(g.arrows.iter().filter_map(|a| a.label.as_ref())) {
                    spell.push((unicode::to_latex(l), l.clone()));
                }
                self.stmts.push(Stmt { line: lineno, col: 1, raw: String::new(), kind: SK::Grid { source: src_lines.join("\n"), g }, spell });
                li = j;
                continue;
            }
            for (col, text) in stmts {
                self.spell.clear();
                let kind = self.statement(lineno, col + 1, &text);
                let spell = std::mem::take(&mut self.spell);
                self.stmts.push(Stmt { line: lineno, col: col + 1, raw: text, kind, spell });
            }
            if let Some((col, text)) = comment {
                self.stmts.push(Stmt { line: lineno, col: col + 1, raw: String::new(), kind: SK::Comment(text), spell: vec![] });
            }
            li += 1;
        }
    }

    fn statement(&mut self, line: usize, col: usize, text: &str) -> SK {
        let n_before = self.diags.iter().filter(|d| d.severity == Severity::Error).count();
        let kind = match first_word(text) {
            "end" => {
                self.err(line, col, "`end` without a `grid` before it.");
                None
            }
            "grid" => {
                self.err(line, col, "`grid` must be on a line of its own.");
                None
            }
            "blob" => self.blob(line, col, text),
            "ngon" => self.ngon(line, col, text),
            "cayley" => self.cayley(line, col, text),
            "text" => self.text(line, col, text),
            "scale" | "spacing" | "size" => self.global(line, col, text),
            _ => self.chain(line, col, text),
        };
        match kind {
            Some(k) => k,
            None => {
                let n_after = self.diags.iter().filter(|d| d.severity == Severity::Error).count();
                if n_after == n_before {
                    self.err(line, col, format!("I can't read `{text}`."));
                }
                SK::Bad(text.to_string())
            }
        }
    }

    fn line_chunks(text: &str, col: usize) -> Vec<Chunk> {
        let cs: Vec<char> = text.chars().collect();
        let mut out = Vec::new();
        for ch in chunks(&cs) {
            // split a token that carries a quoted label with no space (`A"x"`)
            if !ch.text.starts_with(['"', '{', '(']) && ch.text.contains('"') && parse_arrow(&ch.text, false).is_none() {
                if let Some(q) = ch.text.find('"') {
                    let (a, b) = ch.text.split_at(q);
                    let ac = a.chars().count();
                    for (o, piece) in compact_split(a) {
                        let n = piece.chars().count();
                        out.push(Chunk { text: piece, col: col - 1 + ch.col + o, end: col - 1 + ch.col + o + n });
                    }
                    out.push(Chunk { text: b.to_string(), col: col - 1 + ch.col + ac, end: col - 1 + ch.end });
                    continue;
                }
            }
            for (o, piece) in compact_split(&ch.text) {
                let n = piece.chars().count();
                out.push(Chunk { text: piece, col: col - 1 + ch.col + o, end: col - 1 + ch.col + o + n });
            }
        }
        out
    }

    // ---- nodes and arrows ------------------------------------------------

    fn chain(&mut self, line: usize, col: usize, text: &str) -> Option<SK> {
        let cs = Self::line_chunks(text, col);
        let mut terms: Vec<Term> = Vec::new();
        let mut arrows: Vec<ArrowTok> = Vec::new();
        let mut i = 0;
        // term (arrow term)*
        loop {
            let Some(c) = cs.get(i) else { break };
            if let Some(a) = parse_arrow(&c.text, false) {
                if terms.len() == arrows.len() {
                    self.err(line, c.col + 1, format!("`{}` needs something on its left.", c.text));
                    return None;
                }
                arrows.push(a);
                i += 1;
                if cs.get(i).is_none() {
                    self.err(line, c.col + 1, format!("`{}` needs something to point at.", c.text));
                    return None;
                }
                continue;
            }
            if terms.len() > arrows.len() {
                break; // options start
            }
            if c.is_quoted() {
                self.err(line, c.col + 1, format!("{} needs a name before it: write `A {}`.", c.text, c.text));
                return None;
            }
            let name = c.text.clone();
            if name.starts_with('"') || name.contains(',') || name.starts_with('{') || name.starts_with('(') {
                self.err(line, c.col + 1, format!("`{name}` isn't a name (names can't contain `,` `{{` `\"` or start with `(`)."));
                return None;
            }
            let mut term = Term { name, label: None, pos: None, col: c.col };
            i += 1;
            // "label" and/or `at pos` right after the name
            loop {
                match cs.get(i) {
                    Some(q) if q.is_quoted() && term.label.is_none() => {
                        term.label = Some(self.tl(&q.unquote()));
                        i += 1;
                    }
                    Some(q) if q.text == "at" && term.pos.is_none() => {
                        let at_col = q.col;
                        i += 1;
                        match take_pos(&cs, &mut i) {
                            Some(p) => term.pos = Some(p),
                            None => {
                                self.err(line, at_col + 1, "`at` needs a position like `1,0` (a cell) or `(3.2,-1)` (centimetres).");
                                return None;
                            }
                        }
                    }
                    _ => break,
                }
            }
            terms.push(term);
        }
        if terms.is_empty() {
            return None;
        }
        for a in &arrows {
            if let Some(l) = &a.label {
                self.tl(l);
            }
        }
        // options
        let mut opts = ArrowOpts::default();
        let mut node_color = None;
        let is_node = arrows.is_empty();
        while i < cs.len() {
            let c = &cs[i];
            let t = c.text.as_str();
            let ccol = c.col + 1;
            if is_node {
                if let Some(v) = key_value(&cs, &mut i, "color", false) {
                    match v.as_deref().and_then(color_name) {
                        Some(cn) => node_color = Some(cn),
                        None => self.warn(line, ccol, format!("`{t}`: colours are {}; ignored.", PALETTE.join(" "))),
                    }
                    continue;
                }
                if c.is_quoted() {
                    if terms[0].label.is_none() {
                        terms[0].label = Some(self.tl(&c.unquote()));
                    } else {
                        self.warn(line, ccol, format!("`{}` already has a label; {} is ignored.", terms[0].name, t));
                    }
                    i += 1;
                    continue;
                }
                if t == "at" && terms[0].pos.is_none() {
                    i += 1;
                    match take_pos(&cs, &mut i) {
                        Some(p) => terms[0].pos = Some(p),
                        None => {
                            self.err(line, ccol, "`at` needs a position like `1,0` (a cell) or `(3.2,-1)` (centimetres).");
                            return None;
                        }
                    }
                    continue;
                }
                let hint = if parse_arrow(t, false).is_none() && !t.contains('=') && !is_arrow_option(t) {
                    format!(" (for an arrow write `{} -> {}`)", terms[0].name, t)
                } else {
                    String::new()
                };
                self.warn(line, ccol, format!("`{t}` isn't something a node takes; ignored{hint}."));
                i += 1;
                continue;
            }
            match self.arrow_option(&cs, &mut i, &mut opts, line) {
                true => {}
                false => {
                    let hint = if c.is_quoted() {
                        format!(" (an arrow's label goes inside it: `-{}->`)", t)
                    } else if parse_arrow(t, false).is_none() && !t.contains('=') {
                        format!(" (an arrow's label goes inside it: `-{t}->`)")
                    } else {
                        String::new()
                    };
                    self.warn(line, ccol, format!("`{t}` isn't an arrow option; ignored{hint}."));
                    i += 1;
                }
            }
        }
        Some(SK::Chain(Chain { terms, arrows, opts, node_color }))
    }

    /// Reads one option at cs[*i]; false = not an option (nothing consumed).
    fn arrow_option(&mut self, cs: &[Chunk], i: &mut usize, o: &mut ArrowOpts, line: usize) -> bool {
        let c = &cs[*i];
        let t = c.text.as_str();
        let ccol = c.col + 1;
        let simple = |o: &mut ArrowOpts| -> bool {
            match t {
                "bend" | "bend-left" => o.bend = Some(30.0),
                "bend-right" => o.bend = Some(-30.0),
                "loop" => o.is_loop = true,
                "swap" => o.swap = true,
                "iso" => o.iso = true,
                "dashed" => o.line = Some(Line::Dashed),
                "dotted" => o.line = Some(Line::Dotted),
                "solid" => o.line = Some(Line::Solid),
                "thick" => o.thick = true,
                "near-start" => o.label_pos = LabelPos::NearStart,
                "near-end" => o.label_pos = LabelPos::NearEnd,
                "middle" => o.label_pos = LabelPos::Middle,
                _ => return false,
            }
            true
        };
        if simple(o) {
            *i += 1;
            return true;
        }
        for (key, sign) in [("bend-left", 1.0f32), ("bend-right", -1.0), ("bend", 1.0)] {
            if let Some(v) = t.strip_prefix(key).and_then(|r| r.strip_prefix('=')) {
                *i += 1;
                match num(v) {
                    Some(x) => o.bend = Some(x * sign),
                    None => self.warn(line, ccol, format!("`{t}`: the bend is in degrees, like `bend=40`; ignored.")),
                }
                return true;
            }
        }
        if let Some(v) = t.strip_prefix("color=") {
            *i += 1;
            match color_name(v) {
                Some(cn) => o.color = Some(cn),
                None => self.warn(line, ccol, format!("`{t}`: colours are {}; ignored.", PALETTE.join(" "))),
            }
            return true;
        }
        false
    }

    // ---- blobs ------------------------------------------------------------

    fn blob(&mut self, line: usize, col: usize, text: &str) -> Option<SK> {
        let cs = Self::line_chunks(&text["blob".len()..], col + 4);
        let mut i = 0;
        let mut name: Option<String> = None;
        if let Some(c) = cs.first() {
            if !c.is_quoted() && !c.text.starts_with('{') && c.text != "in" && !c.text.contains('=') && c.text != "shaded" && c.text != "at" {
                name = Some(c.text.clone());
                i = 1;
            }
        }
        let mut b = Blob {
            angle: 0.0,
            name: String::new(),
            label: String::new(),
            elements: vec![],
            parent: None,
            shaded: false,
            color: None,
            shape: BlobShape::Round,
            size: None,
            pos: None,
            label_place: LabelPlace::Above,
            span: Span { line, col },
        };
        let mut label: Option<String> = None;
        let mut elems: Vec<ElemRaw> = Vec::new();
        let mut parent = None;
        let mut had_braces = false;
        while i < cs.len() {
            let c = cs[i].clone();
            let t = c.text.as_str();
            let ccol = c.col + 1;
            if c.is_quoted() {
                if label.is_none() {
                    label = Some(self.tl(&c.unquote()));
                } else {
                    self.warn(line, ccol, format!("a blob has one label; {t} is ignored."));
                }
                i += 1;
            } else if t.starts_with('{') {
                if !t.ends_with('}') {
                    self.err(line, ccol, "the `{` of the element list has no matching `}`.");
                    return None;
                }
                if had_braces {
                    self.warn(line, ccol, "a second `{…}`: its elements are added to the first.");
                }
                had_braces = true;
                let inner = &t[1..t.len() - 1];
                for e in comma_list(inner) {
                    if e.is_empty() {
                        continue;
                    }
                    if let Some(er) = self.element(line, ccol, &e) {
                        if elems.iter().any(|x| canon(&x.name) == canon(&er.name)) {
                            self.warn(line, ccol, format!("`{}` is listed twice in this blob; once is enough.", er.name));
                        } else {
                            elems.push(er);
                        }
                    }
                }
                i += 1;
            } else if t == "in" {
                i += 1;
                match cs.get(i) {
                    Some(p) if !p.is_quoted() => {
                        parent = Some(p.text.clone());
                        i += 1;
                    }
                    _ => {
                        self.warn(line, ccol, "`in` needs the name of the parent blob; ignored.");
                    }
                }
            } else if t == "shaded" {
                b.shaded = true;
                i += 1;
            } else if t == "at" {
                i += 1;
                match take_pos(&cs, &mut i) {
                    Some(p) => b.pos = Some(p),
                    None => self.warn(line, ccol, "`at` needs a position like `1,0` or `(3.2,-1)`; ignored."),
                }
            } else if let Some(v) = key_value(&cs, &mut i, "color", false) {
                match v.as_deref().and_then(color_name) {
                    Some(cn) => b.color = Some(cn),
                    None => self.warn(line, ccol, format!("`{t}`: colours are {}; ignored.", PALETTE.join(" "))),
                }
            } else if let Some(v) = key_value(&cs, &mut i, "angle", false) {
                match v.as_deref().and_then(angle_value) {
                    Some(a) => b.angle = a,
                    None => self.warn(line, ccol, format!("`{t}`: an angle is in degrees, like `angle=20`; ignored.")),
                }
            } else if let Some(v) = key_value(&cs, &mut i, "shape", false) {
                match v.as_deref() {
                    Some("round") => b.shape = BlobShape::Round,
                    Some("tall") => b.shape = BlobShape::Tall,
                    Some("wide") => b.shape = BlobShape::Wide,
                    Some("bean") => b.shape = BlobShape::Bean,
                    _ => self.warn(line, ccol, format!("`{t}`: shapes are round, tall, wide and bean; ignored.")),
                }
            } else if let Some(v) = key_value(&cs, &mut i, "size", false) {
                match v.as_deref().and_then(pair) {
                    Some((w, h)) if w > 0.0 && h > 0.0 => b.size = Some((w, h)),
                    _ => self.warn(line, ccol, format!("`{t}`: a size is `size=w,h` in centimetres; ignored.")),
                }
            } else if let Some(v) = key_value(&cs, &mut i, "label", false) {
                match v.as_deref() {
                    Some("above") => b.label_place = LabelPlace::Above,
                    Some("below") => b.label_place = LabelPlace::Below,
                    Some("left") => b.label_place = LabelPlace::Left,
                    Some("right") => b.label_place = LabelPlace::Right,
                    Some("inside") => b.label_place = LabelPlace::Inside,
                    _ => self.warn(line, ccol, format!("`{t}`: the label goes above, below, left, right or inside; ignored.")),
                }
            } else {
                self.warn(line, ccol, format!("`{t}` isn't a blob option; ignored."));
                i += 1;
            }
        }
        let name = match name {
            Some(n) => n,
            None => match &label {
                Some(l) if is_token(l) => {
                    self.note(line, col, format!("this blob has no name, so it is called `{l}` after its label."));
                    l.clone()
                }
                _ => {
                    self.err(line, col, "a blob needs a name: `blob G \"label\" {…}`.");
                    return None;
                }
            },
        };
        if !is_token(&name) {
            self.err(line, col, format!("`{name}` can't be a blob's name."));
            return None;
        }
        b.name = name.clone();
        b.label = label.unwrap_or_else(|| default_label(&name));
        Some(SK::Blob(BlobRaw { blob: b, elems, parent }))
    }

    fn element(&mut self, line: usize, col: usize, e: &str) -> Option<ElemRaw> {
        let cs = Self::line_chunks(e, 1);
        let first = cs.first()?;
        if first.is_quoted() || !is_token(&first.text) {
            self.warn(line, col, format!("`{e}` isn't an element name; skipped."));
            return None;
        }
        let mut er = ElemRaw { name: first.text.clone(), label: None, pos: None };
        let mut i = 1;
        while i < cs.len() {
            let c = &cs[i];
            if c.is_quoted() && er.label.is_none() {
                er.label = Some(self.tl(&c.unquote()));
                i += 1;
            } else if c.text == "at" {
                i += 1;
                match take_pos(&cs, &mut i) {
                    Some(p) => er.pos = Some(p),
                    None => {
                        self.warn(line, col, format!("`{e}`: `at` needs a position; ignored."));
                    }
                }
            } else {
                self.warn(line, col, format!("`{}` after the element `{}` is ignored (elements are separated by commas).", c.text, er.name));
                i += 1;
            }
        }
        Some(er)
    }

    // ---- n-gons -----------------------------------------------------------

    fn ngon(&mut self, line: usize, col: usize, text: &str) -> Option<SK> {
        let cs = Self::line_chunks(&text["ngon".len()..], col + 4);
        let mut i = 0;
        let mut name = None;
        let mut n: Option<u32> = None;
        if let Some(c) = cs.first() {
            if let Ok(k) = c.text.parse::<u32>() {
                n = Some(k);
                i = 1;
            } else if is_token(&c.text) && !c.text.contains('=') && !NGON_OPTS.contains(&c.text.as_str()) {
                name = Some(c.text.clone());
                i = 1;
                if let Some(c2) = cs.get(1) {
                    if let Ok(k) = c2.text.parse::<u32>() {
                        n = Some(k);
                        i = 2;
                    }
                }
            }
        }
        // `n=5`
        if n.is_none() {
            if let Some(c) = cs.get(i) {
                if let Some(v) = c.text.strip_prefix("n=") {
                    if let Ok(k) = v.parse::<u32>() {
                        n = Some(k);
                        i += 1;
                    }
                }
            }
        }
        let n = match n {
            Some(k) if (3..=64).contains(&k) => k,
            Some(k) => {
                self.err(line, col, format!("an n-gon needs between 3 and 64 sides, not {k}."));
                return None;
            }
            None => {
                self.err(line, col, "`ngon` needs a number of sides, like `ngon P 4`.");
                return None;
            }
        };
        let mut g = NGon {
            angle: 0.0,
            show_labels: true,
            shaded: true,
            color: None,
            name: name.clone().unwrap_or_default(),
            n,
            labels: (1..=n).map(|k| k.to_string()).collect(),
            all_axes: false,
            axes: vec![],
            rotation: 0,
            turn: 0,
            rotation_cw: false,
            rotation_edge: None,
            turn_neg: false,
            flip: None,
            radius: None,
            pos: None,
            span: Span { line, col },
            rotation_label: None,
            axis_labels: vec![],
        };
        while i < cs.len() {
            let c = cs[i].clone();
            let t = c.text.as_str();
            let ccol = c.col + 1;
            if t == "axes" {
                g.all_axes = true;
                i += 1;
            } else if t == "plain" {
                g.shaded = false;
                i += 1;
            } else if t == "shaded" {
                g.shaded = true;
                i += 1;
            } else if matches!(t, "labels=none" | "labels=off" | "labels=on") {
                g.show_labels = t == "labels=on";
                i += 1;
            } else if let Some(v) = key_value(&cs, &mut i, "angle", false) {
                match v.as_deref().and_then(angle_value) {
                    Some(a) => g.angle = a,
                    None => self.warn(line, ccol, format!("`{t}`: an angle is in degrees, like `angle=45`; ignored.")),
                }
            } else if let Some(v) = key_value(&cs, &mut i, "color", false) {
                match v.as_deref().and_then(color_name) {
                    Some(cn) => g.color = Some(cn),
                    None => self.warn(line, ccol, format!("`{t}`: colours are {}; ignored.", PALETTE.join(" "))),
                }
            } else if let Some(v) = key_value(&cs, &mut i, "rotation", false) {
                self.rotation_option(&mut g, &cs, &mut i, v, line, ccol, t);
            } else if t == "at" {
                i += 1;
                match take_pos(&cs, &mut i) {
                    Some(p) => g.pos = Some(p),
                    None => self.warn(line, ccol, "`at` needs a position like `1,0` or `(3.2,-1)`; ignored."),
                }
            } else if t == "labels" && cs.get(i + 1).is_some_and(|x| matches!(x.text.as_str(), "none" | "off")) {
                g.show_labels = false;
                i += 2;
            } else if t == "labels" || t.starts_with("labels=") {
                let mut s = match t.strip_prefix("labels=") {
                    Some(v) => {
                        i += 1;
                        v.to_string()
                    }
                    None => {
                        i += 1;
                        match cs.get(i) {
                            Some(x) => {
                                i += 1;
                                x.text.clone()
                            }
                            None => {
                                self.warn(line, ccol, "`labels` needs a list like `labels a,b,c`; ignored.");
                                continue;
                            }
                        }
                    }
                };
                loop {
                    if s.ends_with(',') && i < cs.len() {
                        s.push_str(&cs[i].text);
                        i += 1;
                    } else if i < cs.len() && cs[i].text.starts_with(',') {
                        s.push_str(&cs[i].text);
                        i += 1;
                    } else {
                        break;
                    }
                }
                // an empty entry (`labels 1,,3,4`, or `""`) hides that label
                let mut raws: Vec<String> = comma_list(&s)
                    .into_iter()
                    .map(|x| if x.len() >= 2 && x.starts_with('"') && x.ends_with('"') { x[1..x.len() - 1].to_string() } else { x })
                    .collect();
                // a stray trailing comma (`labels a,b,c,` for a triangle) isn't a label
                if raws.len() == n as usize + 1 && raws.last().is_some_and(|x| x.is_empty()) {
                    raws.pop();
                }
                if raws.iter().all(|x| x.is_empty()) {
                    self.warn(line, ccol, "`labels` needs a list like `labels a,b,c` (`labels=none` hides them); ignored.");
                    continue;
                }
                let mut ls: Vec<String> = raws.iter().map(|x| if x.trim().is_empty() { String::new() } else { self.tl(x) }).collect();
                if ls.len() != n as usize {
                    self.warn(line, ccol, format!("{} labels for {} vertices; {}.", ls.len(), n, if ls.len() < n as usize { "the rest are numbered" } else { "the extra ones are dropped" }));
                    while ls.len() < n as usize {
                        ls.push((ls.len() + 1).to_string());
                    }
                    ls.truncate(n as usize);
                }
                g.labels = ls;
            } else if let Some(v) = key_value(&cs, &mut i, "axis", true) {
                match v.as_deref().and_then(|v| self.axis(line, ccol, n, v)) {
                    Some(a) => {
                        if !g.axes.contains(&a) {
                            g.axes.push(a);
                        }
                        // `axis 1 "s"`: the label at that axis's end
                        if let Some(l) = take_quoted(&cs, &mut i) {
                            let l = self.tl(&l);
                            g.axis_labels.retain(|(c, _)| *c != a);
                            if !l.is_empty() {
                                g.axis_labels.push((a, l));
                            }
                        }
                    }
                    None => self.warn(line, ccol, format!("`axis` needs a vertex 1..{n}{}; ignored.", if n % 2 == 0 { " or an edge like `2.5`" } else { "" })),
                }
            } else if let Some(v) = key_value(&cs, &mut i, "turn", false) {
                // `turn=-1` turns the other way: the same turn mod n, written back as given
                match v.as_deref().and_then(|v| v.parse::<i64>().ok()) {
                    Some(k) if k >= 0 => {
                        g.turn = k as u32;
                        g.turn_neg = false;
                    }
                    Some(k) => {
                        // (review C10b) `unsigned_abs`: i64::MIN has no positive twin
                        let r = k.unsigned_abs() % n as u64;
                        g.turn = ((n as u64 - r) % n as u64) as u32;
                        g.turn_neg = r != 0;
                    }
                    None => self.warn(line, ccol, format!("`{t}`: write `turn=1` (or `turn=-1` for the other way); ignored.")),
                }
            } else if let Some(v) = key_value(&cs, &mut i, "flip", false) {
                match v.as_deref().and_then(|v| self.axis(line, ccol, n, v)) {
                    Some(a) => g.flip = Some(a),
                    None => self.warn(line, ccol, format!("`{t}`: write `flip=k` with an axis like `axis k`; ignored.")),
                }
            } else if let Some(v) = key_value(&cs, &mut i, "radius", false) {
                match v.as_deref().and_then(num) {
                    Some(r) if r > 0.0 => g.radius = Some(r),
                    _ => self.warn(line, ccol, format!("`{t}`: a radius is in centimetres, like `radius=1.5`; ignored.")),
                }
            } else {
                self.warn(line, ccol, format!("`{t}` isn't an n-gon option; ignored."));
                i += 1;
            }
        }
        Some(SK::NGon { ngon: g, named: name.is_some() })
    }

    /// `k` → 2k, `k.5` → 2k+1 (edge k's midpoint, n even). For odd n an edge
    /// midpoint's axis is the opposite vertex's.
    fn axis(&mut self, line: usize, col: usize, n: u32, v: &str) -> Option<u32> {
        if let Some(k) = v.strip_suffix(".5") {
            let k: u32 = k.parse().ok()?;
            if !(1..=n).contains(&k) {
                return None;
            }
            if n % 2 == 0 {
                return Some(2 * k + 1);
            }
            let m = (n - 1) / 2;
            let vtx = (k - 1 + m + 1) % n + 1;
            self.note(line, col, format!("for odd n the axis through edge {k}'s midpoint is the axis through vertex {vtx}."));
            return Some(2 * vtx);
        }
        let k: u32 = v.parse().ok()?;
        if (1..=n).contains(&k) { Some(2 * k) } else { None }
    }

    /// (figure-qol D) `rotation`, `rotation=2`, `rotation=-1`, then any of
    /// `cw`, `ccw`, `edge=3` (or `edge 3`): the arrow's steps, direction and edge.
    #[allow(clippy::too_many_arguments)]
    fn rotation_option(&mut self, g: &mut NGon, cs: &[Chunk], i: &mut usize, v: Option<String>, line: usize, ccol: usize, t: &str) {
        let mut k = 1u32;
        let mut cw = false;
        if let Some(s) = v.as_deref() {
            let (neg, digits) = match s.strip_prefix('-') {
                Some(d) => (true, d),
                None => (false, s),
            };
            match digits.parse::<u32>() {
                Ok(m) => {
                    k = m;
                    cw = neg;
                }
                Err(_) => self.warn(line, ccol, format!("`{t}`: write `rotation`, `rotation=2` or `rotation=-1` (clockwise); ignored.")),
            }
        }
        let mut edge: Option<u32> = None;
        while let Some(c2) = cs.get(*i) {
            let w = c2.text.as_str();
            if w == "cw" {
                cw = true;
                *i += 1;
            } else if w == "ccw" {
                cw = false;
                *i += 1;
            } else if let Some(e) = w.strip_prefix("edge=") {
                match e.parse::<u32>() {
                    Ok(e) if e >= 1 => edge = Some(e),
                    _ => self.warn(line, c2.col + 1, format!("`{w}`: an edge is a number from 1, like `edge=3`; ignored.")),
                }
                *i += 1;
            } else if w == "edge" && cs.get(*i + 1).is_some_and(|x| x.text.parse::<u32>().is_ok()) {
                edge = cs[*i + 1].text.parse::<u32>().ok().filter(|e| *e >= 1);
                *i += 2;
            } else if w.starts_with('"') {
                // (figure-qol C) `rotation "ρ"`: her name for the rotation arrow
                match take_quoted(cs, i) {
                    Some(l) => g.rotation_label = Some(self.tl(&l)).filter(|x| !x.is_empty()),
                    None => break,
                }
            } else {
                break;
            }
        }
        g.rotation = k;
        g.rotation_cw = cw && k > 0;
        g.rotation_edge = if k > 0 { edge.map(|e| (e - 1) % g.n.max(1) + 1) } else { None };
    }

    // ---- cayley -----------------------------------------------------------

    fn cayley(&mut self, line: usize, col: usize, text: &str) -> Option<SK> {
        let rest = &text["cayley".len()..];
        let (pre, inner, post): (String, Option<String>, String) = match rest.find('<') {
            Some(lt) => {
                let after = &rest[lt + 1..];
                let Some(gt) = after.find('>') else {
                    self.err(line, col, "the `<` of the generators has no matching `>`.");
                    return None;
                };
                (rest[..lt].trim().to_string(), Some(after[..gt].to_string()), after[gt + 1..].to_string())
            }
            None => {
                let t = rest.trim();
                // the group is the longest run of leading words that names one,
                // so `Z2 x Z4` / `ℤ2 × ℤ4` (spaced products) stay whole
                let words: Vec<&str> = t.split_whitespace().collect();
                let k = (1..=words.len())
                    .rev()
                    .find(|&k| {
                        let cand = words[..k].join(" ");
                        k == 1 || (!cand.contains('=') && known_group(&group_name(&cand)))
                    })
                    .unwrap_or(1);
                let (a, b) = if words.is_empty() { (String::new(), String::new()) } else { (words[..k].join(" "), words[k..].join(" ")) };
                if a.contains('=') || a == "legend" {
                    (String::new(), None, t.to_string())
                } else {
                    (a, None, b)
                }
            }
        };
        let mut c = Cayley {
            angle: 0.0,
            name: String::new(),
            group: GroupSpec::Named(String::new()),
            gens: vec![],
            layout: CayleyLayout::Auto,
            labels: true,
            legend: false,
            radius: None,
            pos: None,
            span: Span { line, col },
            gen_styles: vec![],
            vertex_names: vec![],
        };
        let mut gen_raws: Vec<String> = vec![];
        let mut explicit_name: Option<String> = None;
        match inner {
            Some(inner) if inner.contains('|') => {
                let (g, r) = inner.split_once('|').unwrap();
                let raws: Vec<String> = comma_list(g).into_iter().filter(|x| !x.is_empty()).collect();
                c.gens = raws.iter().map(|x| self.tl(x)).collect();
                gen_raws = raws.clone();
                let rels: Vec<String> = comma_list(r)
                    .into_iter()
                    .map(|x| x.chars().filter(|c| !c.is_whitespace()).collect::<String>())
                    .filter(|x| !x.is_empty())
                    .map(|x| {
                        let r = relator(&x);
                        self.spell.push((r.clone(), x));
                        r
                    })
                    .collect();
                if c.gens.is_empty() {
                    self.err(line, col, "a presentation needs generators before the `|`.");
                    return None;
                }
                c.group = GroupSpec::Presentation(c.gens.clone(), rels);
                if !pre.is_empty() {
                    if is_token(&pre) {
                        explicit_name = Some(pre.clone());
                    } else {
                        self.warn(line, col, format!("`{pre}` can't be a name; ignored."));
                    }
                }
            }
            other => {
                if pre.is_empty() {
                    self.err(line, col, "`cayley` needs a group (`cayley D4 <r, s>`) or a presentation (`cayley <a, b | a^4, b^2, (ab)^2>`).");
                    return None;
                }
                let g = group_name(&pre);
                if !known_group(&g) {
                    self.err(line, col, format!("I don't know the group `{pre}`; I know Cn, Zn, Dn, V4, Q8, S3, S4, A4 and Zm x Zn, or write a presentation `<a, b | …>`."));
                    return None;
                }
                self.spell.push((g.clone(), pre.trim().to_string()));
                c.group = GroupSpec::Named(g);
                if let Some(inner) = other {
                    let raws: Vec<String> = comma_list(&inner).into_iter().filter(|x| !x.is_empty()).collect();
                    c.gens = raws.iter().map(|x| self.tl(x)).collect();
                    gen_raws = raws.clone();
                }
            }
        }
        let cs = Self::line_chunks(&post, col);
        let mut i = 0;
        while i < cs.len() {
            let ch = cs[i].clone();
            let t = ch.text.as_str();
            let ccol = ch.col + 1;
            if t == "legend" {
                c.legend = true;
                i += 1;
            } else if t == "at" {
                i += 1;
                match take_pos(&cs, &mut i) {
                    Some(p) => c.pos = Some(p),
                    None => self.warn(line, ccol, "`at` needs a position like `1,0` or `(3.2,-1)`; ignored."),
                }
            } else if t == "gens" || t.starts_with("gens=") {
                // (figure-qol D) `gens r:maroon,s^-1,t:hidden`: per generator, a colour,
                // `^-1` (drawn as g⁻¹) and `:hidden`; listed like `labels`
                let mut list = match t.strip_prefix("gens=") {
                    Some(v) => {
                        i += 1;
                        v.to_string()
                    }
                    None => {
                        i += 1;
                        match cs.get(i) {
                            Some(x) => {
                                i += 1;
                                x.text.clone()
                            }
                            None => {
                                self.warn(line, ccol, "`gens` needs a list like `gens r:maroon,s^-1`; ignored.");
                                continue;
                            }
                        }
                    }
                };
                while i < cs.len() && (list.ends_with(',') || cs[i].text.starts_with(',')) {
                    list.push_str(&cs[i].text);
                    i += 1;
                }
                for item in comma_list(&list).into_iter().filter(|x| !x.is_empty()) {
                    if let Some(st) = self.gen_style(&item, &gen_raws, &c.gens, line, ccol) {
                        c.gen_styles.retain(|x| x.generator != st.generator);
                        if st != (GenStyle { generator: st.generator.clone(), ..Default::default() }) {
                            c.gen_styles.push(st);
                        }
                    }
                }
            } else if t == "vertex" {
                // (figure-qol class C) `vertex r^2s "ρ²s"`: her name for that vertex
                i += 1;
                // (review C2) the vertex word is one chunk (`r^2s`, `ρσ`), several
                // (`ρ σ`), or quoted (`"\rho \sigma"`, how the formatter writes a word
                // with a space in it); it's matched to the vertex after the options
                let mut word = String::new();
                let mut quoted_word = false;
                let before = i;
                if let Some(q) = take_quoted(&cs, &mut i) {
                    // a quoted word followed by another quoted chunk is word + name
                    if cs.get(i).is_some_and(|n| n.text.starts_with('"')) {
                        word = q;
                        quoted_word = true;
                    } else {
                        i = before;
                    }
                }
                if !quoted_word {
                    while i < cs.len() && !cs[i].text.starts_with('"') {
                        word.push_str(&cs[i].text);
                        if cs.get(i + 1).is_some_and(|n| !n.text.starts_with('"')) {
                            word.push(' ');
                        }
                        i += 1;
                    }
                }
                let word = (!word.is_empty()).then_some(word);
                match (word, take_quoted(&cs, &mut i)) {
                    (Some(w), Some(l)) => {
                        let l = self.tl(&l);
                        c.vertex_names.retain(|(x, _)| *x != w);
                        if !l.is_empty() {
                            c.vertex_names.push((w, l));
                        }
                    }
                    _ => self.warn(line, ccol, "`vertex` needs a vertex and its name, like `vertex r^2s \"x\"`; ignored."),
                }
            } else if let Some(v) = key_value(&cs, &mut i, "layout", false) {
                match v.as_deref() {
                    Some("circle") => c.layout = CayleyLayout::Circle,
                    Some("prism") => c.layout = CayleyLayout::Prism,
                    Some("grid") => c.layout = CayleyLayout::Grid,
                    Some("auto") => c.layout = CayleyLayout::Auto,
                    _ => self.warn(line, ccol, format!("`{t}`: layouts are circle, prism and grid; ignored.")),
                }
            } else if let Some(v) = key_value(&cs, &mut i, "labels", false) {
                match v.as_deref() {
                    Some("none") | Some("off") | Some("dots") => c.labels = false,
                    Some("words") | Some("on") => c.labels = true,
                    _ => self.warn(line, ccol, format!("`{t}`: write `labels=none` for dots; ignored.")),
                }
            } else if let Some(v) = key_value(&cs, &mut i, "radius", false) {
                match v.as_deref().and_then(num) {
                    Some(r) if r > 0.0 => c.radius = Some(r),
                    _ => self.warn(line, ccol, format!("`{t}`: a radius is in centimetres; ignored.")),
                }
            } else if let Some(v) = key_value(&cs, &mut i, "angle", false) {
                match v.as_deref().and_then(angle_value) {
                    Some(a) => c.angle = a,
                    None => self.warn(line, ccol, format!("`{t}`: an angle is in degrees, like `angle=30`; ignored.")),
                }
            } else if let Some(v) = key_value(&cs, &mut i, "name", false) {
                match v {
                    Some(nm) if is_token(&nm) => explicit_name = Some(nm),
                    _ => self.warn(line, ccol, format!("`{t}` isn't a usable name; ignored.")),
                }
            } else {
                self.warn(line, ccol, format!("`{t}` isn't a Cayley-graph option; ignored."));
                i += 1;
            }
        }
        // (review C2) a vertex word as she spelled it (`ρσ`, `r²s`) → the vertex's own word
        if !c.vertex_names.is_empty() {
            if let Ok(g) = crate::cayley::group(&c.group, &c.gens) {
                let key = |w: &str| -> String { unicode::to_latex(w).chars().filter(|ch| !ch.is_whitespace() && !"{}".contains(*ch)).collect() };
                for (w, _) in c.vertex_names.iter_mut() {
                    if g.elements.contains(w) {
                        continue;
                    }
                    let k = key(w);
                    if let Some(e) = g.elements.iter().find(|e| key(e) == k) {
                        *w = e.clone();
                    }
                }
            }
        }
        let named = explicit_name.is_some();
        c.name = explicit_name.unwrap_or_default();
        Some(SK::Cayley { c, named })
    }

    /// (figure-qol D) One entry of `gens …`: `r`, `r:maroon`, `s^-1`, `t:hidden`, `s^-1:blue:hidden`.
    fn gen_style(&mut self, item: &str, raws: &[String], gens: &[String], line: usize, ccol: usize) -> Option<GenStyle> {
        let mut parts = item.split(':');
        let head = parts.next().unwrap_or("").trim();
        let mut name = head;
        let mut inverse = false;
        for suf in ["^-1", "^{-1}", "⁻¹", "^−1"] {
            if let Some(n) = name.strip_suffix(suf) {
                name = n;
                inverse = true;
                break;
            }
        }
        if name.is_empty() {
            self.warn(line, ccol, format!("`{item}`: name a generator first, like `r:maroon`; ignored."));
            return None;
        }
        let generator = match raws.iter().position(|r| r == name) {
            Some(k) => gens[k].clone(),
            None => name.to_string(),
        };
        let mut st = GenStyle { generator, color: None, inverse, hidden: false };
        for a in parts {
            let a = a.trim();
            match a {
                "hidden" | "hide" => st.hidden = true,
                "shown" | "show" => st.hidden = false,
                "inverse" => st.inverse = true,
                _ => match color_name(a) {
                    Some(cn) => st.color = Some(cn),
                    None => self.warn(line, ccol, format!("`{a}` in `gens`: write a colour ({}), `hidden` or `^-1`; ignored.", PALETTE.join(" "))),
                },
            }
        }
        Some(st)
    }

    // ---- text and globals ---------------------------------------------------

    fn text(&mut self, line: usize, col: usize, text: &str) -> Option<SK> {
        let cs = Self::line_chunks(&text["text".len()..], col + 4);
        let mut words: Vec<String> = Vec::new();
        let mut pos = None;
        let (mut color, mut size) = (None, None);
        let mut i = 0;
        while i < cs.len() {
            let c = &cs[i];
            // (class E) `color=teal`, `size=large` after the text: only when they say something valid
            let opt = |key: &str| -> Option<String> { if c.is_quoted() { None } else { c.text.strip_prefix(key).and_then(|r| r.strip_prefix('=')).map(|v| v.to_string()) } };
            if let Some(v) = opt("color") {
                match color_name(&v) {
                    Some(cn) => color = Some(cn),
                    None => self.warn(line, c.col + 1, format!("`{}`: colours are {}; ignored.", c.text, PALETTE.join(" "))),
                }
                i += 1;
            } else if let Some(v) = opt("size") {
                let v = v.to_lowercase();
                if TEXT_SIZES.contains(&v.as_str()) {
                    size = if v == "normal" { None } else { Some(v) };
                } else {
                    self.warn(line, c.col + 1, format!("`{}`: sizes are {}; ignored.", c.text, TEXT_SIZES.join(" ")));
                }
                i += 1;
            } else if c.text == "at" {
                let ccol = c.col + 1;
                i += 1;
                match take_pos(&cs, &mut i) {
                    Some(p) => pos = Some(p),
                    None => self.warn(line, ccol, "`at` needs a position like `(3,1)`; ignored."),
                }
            } else if c.is_quoted() {
                words.push(c.unquote());
                i += 1;
            } else {
                words.push(c.text.clone());
                i += 1;
            }
        }
        if words.is_empty() {
            self.err(line, col, "`text` needs something to say: `text \"…\" at (x,y)`.");
            return None;
        }
        let pos = pos.unwrap_or_else(|| {
            self.warn(line, col, "this text has no `at`; it goes at (0,0).");
            Pos::Abs(0.0, 0.0)
        });
        let raw = words.join(" ");
        let text = unicode::to_latex_text(&raw);
        self.spell.push((text.clone(), raw));
        Some(SK::Text(Text { text, pos, span: Span { line, col }, color, size }))
    }

    fn global(&mut self, line: usize, col: usize, text: &str) -> Option<SK> {
        let key = first_word(text);
        let v = text[key.len()..].trim().trim_start_matches('=').trim();
        match key {
            "scale" => match num(v) {
                Some(x) if x > 0.0 => self.globals.scale = x,
                _ => {
                    self.err(line, col, format!("`scale` needs a positive number, like `scale 1.2`, not `{v}`."));
                    return None;
                }
            },
            "spacing" => {
                let s: String = v.split_whitespace().collect::<Vec<_>>().join(",").replace(",,", ",");
                let p = pair(&s).or_else(|| num(&s).map(|x| (x, x)));
                match p {
                    Some((a, b)) if a > 0.0 && b > 0.0 => self.globals.spacing = (a, b),
                    _ => {
                        self.err(line, col, format!("`spacing` needs two lengths in cm, like `spacing 2.8,2`, not `{v}`."));
                        return None;
                    }
                }
            }
            _ => match v {
                "small" | "normal" | "large" => self.globals.size = v.to_string(),
                _ => {
                    self.err(line, col, format!("`size` is small, normal or large, not `{v}`."));
                    return None;
                }
            },
        }
        Some(SK::Global)
    }
}

const NGON_OPTS: &[&str] = &["axes", "rotation", "labels", "axis", "at", "plain", "shaded"];

fn is_arrow_option(t: &str) -> bool {
    matches!(
        t,
        "bend" | "bend-left" | "bend-right" | "loop" | "swap" | "iso" | "dashed" | "dotted" | "thick" | "near-start" | "near-end"
    )
}

/// `a⁴` → `a^4`, `a^{4}` → `a^4` (simple exponents unbraced).
pub(crate) fn relator(s: &str) -> String {
    let t = unicode::to_latex(s);
    let mut out = String::new();
    let cs: Vec<char> = t.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '^' && cs.get(i + 1) == Some(&'{') {
            if let Some(close) = cs[i + 2..].iter().position(|&c| c == '}') {
                let inner: String = cs[i + 2..i + 2 + close].iter().collect();
                if inner.chars().all(|c| c.is_ascii_digit() || c == '-') {
                    out.push('^');
                    out.push_str(&inner);
                    i += 3 + close;
                    continue;
                }
            }
        }
        out.push(cs[i]);
        i += 1;
    }
    out
}

/// "z2 × ℤ3" → "Z2xZ3", "D_4" → "D4".
pub(crate) fn group_name(s: &str) -> String {
    let t: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_' && *c != '{' && *c != '}')
        .map(|c| match c {
            'ℤ' => 'Z',
            '×' => 'x',
            _ => c,
        })
        .collect();
    let t = t.replace("\\times", "x").replace("\\mathbb", "");
    let up = t.to_uppercase();
    // X between factors is the product sign
    let cs: Vec<char> = up.chars().collect();
    let mut out = String::new();
    for (k, &c) in cs.iter().enumerate() {
        if c == 'X' && k > 0 && cs[k - 1].is_ascii_digit() {
            out.push('x');
        } else {
            out.push(c);
        }
    }
    out
}

pub(crate) fn known_group(g: &str) -> bool {
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) && s.parse::<u32>().map(|n| n >= 1).unwrap_or(false);
    if ["V4", "K4", "Q8", "S3", "S4", "A4"].contains(&g) {
        return true;
    }
    if g.contains('x') {
        return g.split('x').all(|f| (f.starts_with('Z') || f.starts_with('C')) && digits(&f[1..]));
    }
    if let Some(r) = g.strip_prefix('Z').or_else(|| g.strip_prefix('C')).or_else(|| g.strip_prefix('D')) {
        return digits(r);
    }
    false
}

// ===========================================================================
// grid blocks

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GNode {
    pub name: String,
    pub label: Option<String>,
    pub cell: (f32, f32),
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GArrow {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub head: Head,
    pub line_style: Line,
    pub side: LabelSide,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct GridParsed {
    pub nodes: Vec<GNode>,
    pub arrows: Vec<GArrow>,
    pub diags: Vec<Diagnostic>,
}

/// The lines between `grid` and `end` (1-based line numbers, raw text).
pub(crate) fn grid_from_source(source: &str, first_line: usize) -> GridParsed {
    let lines: Vec<&str> = source.lines().collect();
    let mut body = Vec::new();
    for (k, l) in lines.iter().enumerate().skip(1) {
        let (st, _) = split_line(&expand_tabs(l));
        if k == lines.len() - 1 && st.len() == 1 && st[0].1 == "end" {
            break;
        }
        body.push((first_line + k, l.to_string()));
    }
    parse_grid(&body)
}

#[derive(Clone, Debug)]
struct RowTok {
    name: String,
    label: Option<String>,
    start: usize,
    end: usize,
}

#[derive(Clone, Debug)]
enum VMark {
    Pipe { equal: bool, dashed: bool, label: Option<String>, left: bool },
    Down,
    Up,
    Diag { down_right: bool, label: Option<String> },
}

fn strip_quotes(s: &str) -> String {
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') { s[1..s.len() - 1].to_string() } else { s.to_string() }
}

fn opt_label(s: &str) -> Option<String> {
    let s = strip_quotes(s.trim());
    if s.is_empty() { None } else { Some(s) }
}

/// Marks on a between-line, with the column of the mark.
fn vmarks(c: &Chunk) -> Option<Vec<(usize, VMark)>> {
    let t = c.text.as_str();
    if parse_arrow(t, true).is_some() && !matches!(t, "||") {
        return None;
    }
    match t {
        "v" | "V" | "↓" => return Some(vec![(c.col, VMark::Down)]),
        "^" | "↑" => return Some(vec![(c.col, VMark::Up)]),
        _ => {}
    }
    if let Some(r) = t.strip_prefix('^') {
        let inner = Chunk { text: r.to_string(), col: c.col + 1, end: c.end };
        if let Some(mut m) = vmarks(&inner) {
            if matches!(m.first(), Some((_, VMark::Pipe { .. }))) {
                m.insert(0, (c.col, VMark::Up));
                return Some(m);
            }
        }
        return None;
    }
    let pipe = |s: &str| -> Option<(bool, bool, String)> {
        if let Some(r) = s.strip_prefix("||") {
            return Some((true, false, r.to_string()));
        }
        if let Some(r) = s.strip_prefix('‖') {
            return Some((true, false, r.to_string()));
        }
        if let Some(r) = s.strip_prefix('|') {
            return Some((false, false, r.to_string()));
        }
        if let Some(r) = s.strip_prefix(':') {
            return Some((false, true, r.trim_start_matches(':').to_string()));
        }
        None
    };
    if let Some((equal, dashed, rest)) = pipe(t) {
        return Some(vec![(c.col, VMark::Pipe { equal, dashed, label: opt_label(&rest), left: false })]);
    }
    // label written on the left: `g|`
    for (suf, equal, dashed) in [("||", true, false), ("‖", true, false), ("|", false, false), (":", false, true)] {
        if let Some(r) = t.strip_suffix(suf) {
            if !r.is_empty() && !r.ends_with('|') {
                let col = c.end.saturating_sub(suf.chars().count());
                return Some(vec![(col, VMark::Pipe { equal, dashed, label: opt_label(r), left: true })]);
            }
        }
    }
    let diag = |r: &str| -> Option<String> {
        // `\\pi` → \pi; `\pi` (a known command) → \pi; `\h` → h
        if r.starts_with('\\') {
            return opt_label(r);
        }
        if r.len() >= 2 && r.chars().all(|c| c.is_ascii_alphabetic()) {
            let cmd = format!("\\{r}");
            if unicode::preview(&cmd) != r {
                return Some(cmd);
            }
        }
        opt_label(r)
    };
    if let Some(r) = t.strip_prefix('\\').or_else(|| t.strip_prefix('↘')) {
        return Some(vec![(c.col, VMark::Diag { down_right: true, label: diag(r) })]);
    }
    if let Some(r) = t.strip_prefix('/').or_else(|| t.strip_prefix('↙')) {
        return Some(vec![(c.col, VMark::Diag { down_right: false, label: opt_label(r) })]);
    }
    None
}

fn command_like(t: &str) -> bool {
    let cs: Vec<char> = t.chars().collect();
    cs.len() > 2 && cs[0] == '\\' && cs[1].is_ascii_alphabetic() && cs[2].is_ascii_alphabetic()
}

fn shaft_piece(t: &str) -> bool {
    !t.is_empty() && t.chars().all(|c| matches!(c, '-' | '.' | '=' | '<' | '>'))
}

pub(crate) fn parse_grid(lines: &[(usize, String)]) -> GridParsed {
    let mut out = GridParsed::default();
    enum Kind {
        Row(Vec<RowTok>, Vec<(usize, usize, ArrowTok, bool)>), // tokens, arrows (left tok idx, col, arrow, spaced-dashed)
        Between(Vec<(usize, VMark)>),
        Blank,
    }
    let mut kinds: Vec<(usize, Kind)> = Vec::new();
    for (lineno, raw) in lines {
        let chars = expand_tabs(raw);
        let (_, comment) = split_line(&chars);
        let cut = comment.map(|(c, _)| c).unwrap_or(chars.len());
        let body = &chars[..cut];
        let mut cs: Vec<Chunk> = Vec::new();
        for ch in chunks(body) {
            for (o, piece) in compact_split(&ch.text) {
                let n = piece.chars().count();
                cs.push(Chunk { text: piece, col: ch.col + o, end: ch.col + o + n });
            }
        }
        if cs.is_empty() {
            kinds.push((*lineno, Kind::Blank));
            continue;
        }
        let marks: Option<Vec<Vec<(usize, VMark)>>> = cs.iter().map(vmarks).collect();
        if let Some(m) = marks {
            let all_cmd = cs.iter().all(|c| command_like(&c.text));
            if !all_cmd {
                kinds.push((*lineno, Kind::Between(m.into_iter().flatten().collect())));
                continue;
            }
        }
        // a node row
        let mut toks: Vec<RowTok> = Vec::new();
        // sequence: Some(tok idx) for tokens, None for arrows
        let mut seq: Vec<Result<usize, (usize, ArrowTok, bool)>> = Vec::new();
        let mut i = 0;
        while i < cs.len() {
            let c = &cs[i];
            if c.is_quoted() {
                if let Some(Ok(k)) = seq.last() {
                    if toks[*k].label.is_none() {
                        toks[*k].label = Some(c.unquote());
                        toks[*k].end = c.end;
                        i += 1;
                        continue;
                    }
                }
                out.diags.push(diag(Severity::Warning, *lineno, c.col + 1, format!("{} in a grid labels the node right before it; there is none here, so it is ignored.", c.text)));
                i += 1;
                continue;
            }
            if let Some(a) = parse_arrow(&c.text, true) {
                // a reversed arrow may continue with spaced pieces (`< - -`)
                if !(a.reversed && cs.get(i + 1).map(|d| shaft_piece(&d.text)).unwrap_or(false)) {
                    seq.push(Err((c.col, a, false)));
                    i += 1;
                    continue;
                }
            }
            if shaft_piece(&c.text) {
                // spaced arrow: `- - >`, `-- f -->`
                let mut parts: Vec<String> = vec![c.text.clone()];
                let mut kinds_p: Vec<bool> = vec![true]; // true = shaft piece
                let mut label: Option<String> = None;
                let mut j = i + 1;
                let mut ok: Option<ArrowTok> = None;
                loop {
                    let joined: String = parts.concat();
                    let parsed = parse_arrow(&joined, true);
                    let next_shaft = cs.get(j).map(|d| shaft_piece(&d.text)).unwrap_or(false);
                    if let Some(a) = &parsed {
                        if joined.ends_with('>') || !next_shaft {
                            ok = Some(a.clone());
                            break;
                        }
                    }
                    let Some(d) = cs.get(j) else { break };
                    if shaft_piece(&d.text) {
                        parts.push(d.text.clone());
                        kinds_p.push(true);
                        j += 1;
                    } else if label.is_none() && cs.get(j + 1).map(|e| shaft_piece(&e.text)).unwrap_or(false) && !d.is_quoted() || (label.is_none() && d.is_quoted()) {
                        label = Some(strip_quotes(&d.text));
                        parts.push(format!("\"{}\"", strip_quotes(&d.text)));
                        kinds_p.push(false);
                        j += 1;
                    } else {
                        break;
                    }
                }
                match ok {
                    Some(mut a) => {
                        let spaced = kinds_p.windows(2).any(|w| w[0] && w[1]);
                        if spaced && a.head != Head::Equal {
                            a.line = Line::Dashed;
                        }
                        seq.push(Err((c.col, a, spaced)));
                        i = j;
                    }
                    None => {
                        out.diags.push(diag(Severity::Warning, *lineno, c.col + 1, format!("`{}` in this grid row isn't an arrow; ignored.", c.text)));
                        i += 1;
                    }
                }
                continue;
            }
            toks.push(RowTok { name: c.text.clone(), label: None, start: c.col, end: c.end });
            seq.push(Ok(toks.len() - 1));
            i += 1;
        }
        let mut arrows = Vec::new();
        for (k, el) in seq.iter().enumerate() {
            if let Err((col, a, sp)) = el {
                let left = if k > 0 { seq[k - 1].as_ref().ok().copied() } else { None };
                let right = seq.get(k + 1).and_then(|r| r.as_ref().ok().copied());
                match (left, right) {
                    (Some(l), Some(_)) => arrows.push((l, *col, a.clone(), *sp)),
                    (None, _) => out.diags.push(diag(Severity::Warning, *lineno, col + 1, "this grid arrow has no node on its left; ignored.")),
                    (_, None) => out.diags.push(diag(Severity::Warning, *lineno, col + 1, "this grid arrow has no node on its right; ignored.")),
                }
            }
        }
        kinds.push((*lineno, Kind::Row(toks, arrows)));
    }

    // column clustering over token start columns
    let rows: Vec<(usize, &Vec<RowTok>, &Vec<(usize, usize, ArrowTok, bool)>)> = kinds
        .iter()
        .filter_map(|(l, k)| match k {
            Kind::Row(t, a) => Some((*l, t, a)),
            _ => None,
        })
        .collect();
    let mut starts: Vec<usize> = rows.iter().flat_map(|r| r.1.iter().map(|t| t.start)).collect();
    starts.sort();
    starts.dedup();
    let cluster_of = |tol: usize| -> Vec<(usize, usize)> {
        // (start col, cluster index)
        let mut res = Vec::new();
        let mut ci = 0usize;
        let mut first: Option<usize> = None;
        for &s in &starts {
            match first {
                Some(f) if s - f <= tol => {}
                Some(_) => {
                    ci += 1;
                    first = Some(s);
                }
                None => first = Some(s),
            }
            res.push((s, ci));
        }
        res
    };
    let mut table = cluster_of(2);
    for tol in [2usize, 1, 0] {
        table = cluster_of(tol);
        let ok = rows.iter().all(|r| {
            let mut seen = HashSet::new();
            r.1.iter().all(|t| seen.insert(table.iter().find(|x| x.0 == t.start).map(|x| x.1).unwrap_or(0)))
        });
        if ok {
            break;
        }
    }
    let col_of = |s: usize| table.iter().find(|x| x.0 == s).map(|x| x.1).unwrap_or(0) as f32;

    // nodes
    // each token is a node; a token written twice (`0 --> K --> … --> 0`) is
    // two nodes, the second named `0#2` with the same label
    let mut seen: HashSet<String> = HashSet::new();
    let mut ids: HashMap<(usize, usize), String> = HashMap::new();
    for (r, (lineno, toks, _)) in rows.iter().enumerate() {
        for (k, t) in toks.iter().enumerate() {
            let cell = (col_of(t.start), r as f32);
            let mut name = t.name.clone();
            let mut label = t.label.clone();
            if seen.contains(&canon(&name)) {
                name = (2..).map(|n| format!("{}#{n}", t.name)).find(|c| !seen.contains(&canon(c))).unwrap();
                label = Some(label.unwrap_or_else(|| t.name.clone()));
            }
            seen.insert(canon(&name));
            ids.insert((r, k), name.clone());
            out.nodes.push(GNode { name, label, cell, line: *lineno, col: t.start + 1 });
        }
    }
    let id = |r: usize, k: usize| -> String { ids.get(&(r, k)).cloned().unwrap_or_default() };

    // arrows, in reading order
    let row_lines: Vec<usize> = rows.iter().map(|r| r.0).collect();
    let mut ri = 0;
    let mut k = 0;
    while k < kinds.len() {
        let (lineno, kind) = &kinds[k];
        match kind {
            Kind::Row(toks, arrows) => {
                let _ = toks;
                for (l, _col, a, _) in arrows {
                    let (lt, rt) = (id(ri, *l), id(ri, *l + 1));
                    let (from, to) = if a.reversed { (rt, lt) } else { (lt, rt) };
                    out.arrows.push(GArrow {
                        from,
                        to,
                        label: a.label.clone(),
                        head: a.head,
                        line_style: a.line,
                        // the label stays where the art writes it (above):
                        // tikz-cd's auto side is below a leftward arrow
                        side: if a.reversed { LabelSide::Swap } else { LabelSide::Auto },
                        line: *lineno,
                    });
                }
                ri += 1;
                k += 1;
            }
            Kind::Blank => k += 1,
            Kind::Between(_) => {
                // gather the between-lines up to the next row
                let mut marks: Vec<(usize, usize, VMark)> = Vec::new(); // (line, col, mark)
                let mut bl: Vec<usize> = Vec::new();
                while k < kinds.len() {
                    match &kinds[k].1 {
                        Kind::Between(m) => {
                            bl.push(kinds[k].0);
                            for (c, mk) in m {
                                marks.push((kinds[k].0, *c, mk.clone()));
                            }
                            k += 1;
                        }
                        Kind::Blank => k += 1,
                        Kind::Row(..) => break,
                    }
                }
                let upper = if ri > 0 { Some(ri - 1) } else { None };
                let lower = if ri < rows.len() { Some(ri) } else { None };
                let first_line = bl.first().copied().unwrap_or(*lineno);
                let (Some(ua), Some(lb)) = (upper, lower) else {
                    out.diags.push(diag(Severity::Warning, first_line, 1, "these grid marks have no node row above and below them; ignored."));
                    continue;
                };
                let up_toks = rows[ua].1;
                let lo_toks = rows[lb].1;
                let _ = &row_lines;
                // verticals: group by column (±1)
                let mut vs: Vec<(usize, usize, VMark)> = marks.iter().filter(|m| !matches!(m.2, VMark::Diag { .. })).cloned().collect();
                vs.sort_by_key(|m| (m.1, m.0));
                let mut groups: Vec<Vec<(usize, usize, VMark)>> = Vec::new();
                for m in vs {
                    match groups.last_mut() {
                        Some(g) if m.1 <= g.last().unwrap().1 + 1 => g.push(m),
                        _ => groups.push(vec![m]),
                    }
                }
                let find = |toks: &Vec<RowTok>, c: usize| -> Option<usize> {
                    if let Some(k) = toks.iter().position(|t| t.start <= c && c < t.end.max(t.start + 1)) {
                        return Some(k);
                    }
                    let mut best: Option<(usize, usize)> = None;
                    for (k, t) in toks.iter().enumerate() {
                        let d = if c < t.start { t.start - c } else { c + 1 - t.end.max(t.start + 1) };
                        if d <= 2 && best.map(|b| d < b.1).unwrap_or(true) {
                            best = Some((k, d));
                        }
                    }
                    best.map(|b| b.0)
                };
                for g in groups {
                    let (mut up, mut down, mut equal, mut dashed) = (false, false, false, false);
                    let mut label: Option<(String, bool)> = None;
                    for (_, _, m) in &g {
                        match m {
                            VMark::Up => up = true,
                            VMark::Down => down = true,
                            VMark::Pipe { equal: e, dashed: d, label: l, left } => {
                                equal |= *e;
                                dashed |= *d;
                                if label.is_none() {
                                    if let Some(l) = l {
                                        label = Some((l.clone(), *left));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    let (mline, mcol) = (g[0].0, g[0].1);
                    let u = g.iter().find_map(|m| find(up_toks, m.1));
                    let d = g.iter().find_map(|m| find(lo_toks, m.1));
                    let (Some(u), Some(d)) = (u, d) else {
                        out.diags.push(diag(
                            Severity::Warning,
                            mline,
                            mcol + 1,
                            format!("the vertical arrow at column {} has no node {} it within 2 columns; ignored.", mcol + 1, if u.is_none() { "above" } else { "below" }),
                        ));
                        continue;
                    };
                    let head = if equal {
                        Head::Equal
                    } else if up && down {
                        Head::Both
                    } else {
                        Head::To
                    };
                    if !equal && !up && !down {
                        out.diags.push(diag(Severity::Note, mline, mcol + 1, format!("the `|` at column {} has no `v` or `^`, so it points down.", mcol + 1)));
                    }
                    let (from, to) = if up && !down && !equal { (id(lb, d), id(ua, u)) } else { (id(ua, u), id(lb, d)) };
                    // the label stays where the art writes it: tikz-cd's auto
                    // side is right of a down arrow and left of an up arrow
                    let pointing_up = up && !down && !equal;
                    let left = matches!(&label, Some((_, true)));
                    let side = if left != pointing_up { LabelSide::Swap } else { LabelSide::Auto };
                    out.arrows.push(GArrow {
                        from,
                        to,
                        label: label.map(|l| l.0),
                        head,
                        line_style: if dashed && !equal { Line::Dashed } else { Line::Solid },
                        side,
                        line: mline,
                    });
                }
                // diagonals: chains of marks on successive between-lines
                let mut ds: Vec<(usize, usize, bool, Option<String>)> = marks
                    .iter()
                    .filter_map(|m| match &m.2 {
                        VMark::Diag { down_right, label } => Some((m.0, m.1, *down_right, label.clone())),
                        _ => None,
                    })
                    .collect();
                ds.sort_by_key(|d| (d.0, d.1));
                let mut chains: Vec<Vec<(usize, usize, bool, Option<String>)>> = Vec::new();
                for d in ds {
                    let li = bl.iter().position(|&x| x == d.0).unwrap_or(0);
                    let prev_line = if li > 0 { Some(bl[li - 1]) } else { None };
                    let hit = chains.iter_mut().find(|ch| {
                        let last = ch.last().unwrap();
                        let want = if d.2 { last.1 as isize + 1 } else { last.1 as isize - 1 };
                        Some(last.0) == prev_line && last.2 == d.2 && (d.1 as isize - want).abs() <= 1
                    });
                    match hit {
                        Some(ch) => ch.push(d),
                        None => chains.push(vec![d]),
                    }
                }
                for ch in chains {
                    let (top, bot) = (&ch[0], ch.last().unwrap());
                    let dr = top.2;
                    let label = ch.iter().find_map(|d| d.3.clone());
                    let nearest = |toks: &Vec<RowTok>, c: usize| -> Option<usize> {
                        toks.iter().enumerate().min_by_key(|(_, t)| if c < t.start { t.start - c } else { c.saturating_sub(t.end) }).map(|x| x.0)
                    };
                    let u = if dr {
                        up_toks.iter().enumerate().filter(|(_, t)| t.start <= top.1).max_by_key(|(_, t)| t.start).map(|x| x.0)
                    } else {
                        up_toks.iter().enumerate().filter(|(_, t)| t.end > top.1).min_by_key(|(_, t)| t.start).map(|x| x.0)
                    }
                    .or_else(|| nearest(up_toks, top.1));
                    let d = if dr {
                        lo_toks.iter().enumerate().filter(|(_, t)| t.end > bot.1).min_by_key(|(_, t)| t.start).map(|x| x.0)
                    } else {
                        lo_toks.iter().enumerate().filter(|(_, t)| t.start <= bot.1).max_by_key(|(_, t)| t.start).map(|x| x.0)
                    }
                    .or_else(|| nearest(lo_toks, bot.1));
                    let (Some(u), Some(d)) = (u, d) else {
                        out.diags.push(diag(Severity::Warning, top.0, top.1 + 1, "this diagonal has no node to connect; ignored."));
                        continue;
                    };
                    out.arrows.push(GArrow {
                        from: id(ua, u),
                        to: id(lb, d),
                        label,
                        head: Head::To,
                        line_style: Line::Solid,
                        side: LabelSide::Auto,
                        line: top.0,
                    });
                }
            }
        }
    }
    out
}

// ===========================================================================
// phase 2: names, then items

#[derive(Clone, Debug, PartialEq)]
enum DK {
    GridNode,
    Node,
    Blob,
    Element,
    NGon(u32, Vec<String>),
    Cayley,
}

#[derive(Clone, Debug)]
struct Decl {
    kind: DK,
    name: String,
    line: usize,
}

#[derive(Clone, Debug, Default)]
struct Attrs {
    label: Option<(String, usize)>,
    pos: Option<(Pos, usize)>,
    color: Option<String>,
    fixed_label: bool,
    fixed_pos: bool,
}

fn kind_word(k: &DK) -> &'static str {
    match k {
        DK::GridNode | DK::Node => "a node",
        DK::Blob => "a blob",
        DK::Element => "an element",
        DK::NGon(..) => "an n-gon",
        DK::Cayley => "a Cayley graph",
    }
}

fn phase2(p: Phase1) -> (Figure, Vec<Diagnostic>) {
    let Phase1 { mut stmts, mut diags, globals, .. } = p;
    let mut decls: HashMap<String, Decl> = HashMap::new();
    let mut used: HashSet<String> = HashSet::new();

    // every name written anywhere (auto names avoid them)
    for s in &stmts {
        match &s.kind {
            SK::Grid { g, .. } => g.nodes.iter().for_each(|n| {
                used.insert(canon(&n.name));
            }),
            SK::Chain(c) => c.terms.iter().for_each(|t| {
                used.insert(canon(&t.name));
                if let Some((a, _)) = t.name.split_once('.') {
                    used.insert(canon(a));
                }
            }),
            SK::Blob(b) => {
                used.insert(canon(&b.blob.name));
                b.elems.iter().for_each(|e| {
                    used.insert(canon(&e.name));
                });
            }
            SK::NGon { ngon, named: true } => {
                used.insert(canon(&ngon.name));
            }
            SK::Cayley { c, named: true } => {
                used.insert(canon(&c.name));
            }
            _ => {}
        }
    }

    // auto names for unnamed n-gons and Cayley graphs
    for s in stmts.iter_mut() {
        match &mut s.kind {
            SK::NGon { ngon, named: false } => {
                let mut cands: Vec<String> = ["P", "Q", "R", "S", "T", "U", "W"].iter().map(|x| x.to_string()).collect();
                cands.extend((1..).take(500).map(|k| format!("P{k}")));
                let nm = cands.into_iter().find(|c| !used.contains(c)).unwrap();
                used.insert(nm.clone());
                diags.push(diag(Severity::Note, s.line, s.col, format!("this n-gon has no name, so it is `{nm}` (write `ngon {nm} {}` to keep it).", ngon.n)));
                ngon.name = nm;
            }
            SK::Cayley { c, named: false } => {
                let base = match &c.group {
                    GroupSpec::Named(g) => g.clone(),
                    GroupSpec::Presentation(..) => "C".to_string(),
                };
                c.name = base;
            }
            _ => {}
        }
    }
    // a second n-gon with the same name (a sequence: `ngon P 4`, `ngon P 4 turn=1`)
    // gets the next free name
    {
        let mut seen: HashSet<String> = HashSet::new();
        for s in stmts.iter_mut() {
            if let SK::NGon { ngon, .. } = &mut s.kind {
                let key = canon(&ngon.name);
                if seen.contains(&key) {
                    let base = ngon.name.clone();
                    let nm = (2..).map(|k| format!("{base}{k}")).find(|c| !used.contains(&canon(c)) && !seen.contains(&canon(c))).unwrap();
                    diags.push(diag(Severity::Note, s.line, s.col, format!("there is already an n-gon `{base}`, so this one is `{nm}`.")));
                    used.insert(canon(&nm));
                    ngon.name = nm;
                }
                seen.insert(canon(&ngon.name));
            }
        }
    }
    // Cayley default names must not collide with other declarations
    {
        let mut taken: HashSet<String> = HashSet::new();
        for s in stmts.iter() {
            match &s.kind {
                SK::Grid { g, .. } => g.nodes.iter().for_each(|n| {
                    taken.insert(canon(&n.name));
                }),
                SK::Blob(b) => {
                    taken.insert(canon(&b.blob.name));
                    b.elems.iter().for_each(|e| {
                        taken.insert(canon(&e.name));
                    });
                }
                SK::NGon { ngon, .. } => {
                    taken.insert(canon(&ngon.name));
                }
                SK::Cayley { c, named: true } => {
                    taken.insert(canon(&c.name));
                }
                SK::Chain(ch) if ch.arrows.is_empty() => {
                    taken.insert(canon(&ch.terms[0].name));
                }
                _ => {}
            }
        }
        for s in stmts.iter_mut() {
            if let SK::Cayley { c, named: false } = &mut s.kind {
                let base = c.name.clone();
                let mut nm = base.clone();
                let mut k = 2;
                while taken.contains(&canon(&nm)) {
                    nm = format!("{base}_{k}");
                    k += 1;
                }
                taken.insert(canon(&nm));
                c.name = nm;
            }
        }
    }

    // ---- declarations: grids, blobs, elements, n-gons, Cayley graphs
    let mut grid_created: Vec<Vec<bool>> = vec![Vec::new(); stmts.len()];
    let mut dropped_elems: HashSet<(usize, String)> = HashSet::new();
    let declare = |decls: &mut HashMap<String, Decl>, diags: &mut Vec<Diagnostic>, name: &str, kind: DK, line: usize, col: usize| -> bool {
        let key = canon(name);
        if let Some(d) = decls.get(&key) {
            diags.push(diag(
                Severity::Error,
                line,
                col,
                format!("`{name}` is already {} (line {}); names must be unique, so this one is skipped.", kind_word(&d.kind), d.line),
            ));
            return false;
        }
        decls.insert(key, Decl { kind, name: name.to_string(), line });
        true
    };
    // elements may be listed again in a child blob: collect first
    let mut elem_listings: HashMap<String, Vec<(usize, ElemRaw)>> = HashMap::new();
    let mut elem_order: Vec<String> = Vec::new();
    let mut blob_ok: Vec<bool> = vec![true; stmts.len()];
    for (si, s) in stmts.iter().enumerate() {
        match &s.kind {
            SK::Grid { g, .. } => {
                for n in &g.nodes {
                    let ok = declare(&mut decls, &mut diags, &n.name, DK::GridNode, n.line, n.col);
                    grid_created[si].push(ok);
                }
            }
            SK::Blob(b) => {
                if !declare(&mut decls, &mut diags, &b.blob.name, DK::Blob, s.line, s.col) {
                    blob_ok[si] = false;
                    continue;
                }
                for e in &b.elems {
                    let key = canon(&e.name);
                    if !elem_listings.contains_key(&key) {
                        elem_order.push(key.clone());
                    }
                    elem_listings.entry(key).or_default().push((si, e.clone()));
                }
            }
            SK::NGon { ngon, .. } => {
                declare(&mut decls, &mut diags, &ngon.name, DK::NGon(ngon.n, ngon.labels.clone()), s.line, s.col);
            }
            SK::Cayley { c, .. } => {
                declare(&mut decls, &mut diags, &c.name, DK::Cayley, s.line, s.col);
            }
            _ => {}
        }
    }
    // blob parents
    let blob_stmt: HashMap<String, usize> = stmts
        .iter()
        .enumerate()
        .filter_map(|(i, s)| match &s.kind {
            SK::Blob(b) if blob_ok[i] => Some((canon(&b.blob.name), i)),
            _ => None,
        })
        .collect();
    let mut parent_of: HashMap<usize, usize> = HashMap::new();
    for (si, s) in stmts.iter().enumerate() {
        if let SK::Blob(b) = &s.kind {
            if !blob_ok[si] {
                continue;
            }
            if let Some(pn) = &b.parent {
                match blob_stmt.get(&canon(pn)) {
                    Some(&pi) if pi != si => {
                        // no cycles
                        let mut cur = pi;
                        let mut cyc = false;
                        let mut guard = 0;
                        while let Some(&up) = parent_of.get(&cur) {
                            if up == si {
                                cyc = true;
                                break;
                            }
                            cur = up;
                            guard += 1;
                            if guard > 1000 {
                                break;
                            }
                        }
                        if cyc {
                            diags.push(diag(Severity::Warning, s.line, s.col, format!("`{}` can't be inside `{pn}`: that would put a blob inside itself; ignored.", b.blob.name)));
                        } else {
                            parent_of.insert(si, pi);
                        }
                    }
                    _ => diags.push(diag(Severity::Warning, s.line, s.col, format!("there is no blob `{pn}` for `{}` to be in; it stands on its own.", b.blob.name))),
                }
            }
        }
    }
    let ancestor = |a: usize, d: usize| -> bool {
        // is a an ancestor of d
        let mut cur = d;
        let mut guard = 0;
        while let Some(&up) = parent_of.get(&cur) {
            if up == a {
                return true;
            }
            cur = up;
            guard += 1;
            if guard > 1000 {
                break;
            }
        }
        false
    };
    // element ownership: the innermost of a chain of listings
    let mut owner: HashMap<String, (usize, ElemRaw)> = HashMap::new();
    let mut lost: HashSet<usize> = HashSet::new(); // blobs whose list names elements of a child
    for key in &elem_order {
        let ls = &elem_listings[key];
        let (s0, e0) = &ls[0];
        if !declare(&mut decls, &mut diags, &e0.name, DK::Element, stmts[*s0].line, stmts[*s0].col) {
            for (si, e) in ls {
                dropped_elems.insert((*si, canon(&e.name)));
            }
            continue;
        }
        let mut group: Vec<(usize, ElemRaw)> = vec![ls[0].clone()];
        for (si, e) in ls.iter().skip(1) {
            if group.iter().all(|(g, _)| ancestor(*g, *si) || ancestor(*si, *g)) {
                group.push((*si, e.clone()));
            } else {
                let firstb = match &stmts[*s0].kind {
                    SK::Blob(b) => b.blob.name.clone(),
                    _ => String::new(),
                };
                diags.push(diag(
                    Severity::Error,
                    stmts[*si].line,
                    stmts[*si].col,
                    format!("`{}` is already an element of `{firstb}` (line {}); element names must be unique, so it is skipped here.", e.name, stmts[*s0].line),
                ));
                dropped_elems.insert((*si, canon(&e.name)));
            }
        }
        // deepest: the one every other is an ancestor of
        let deepest = group.iter().map(|g| g.0).find(|&d| group.iter().all(|(o, _)| *o == d || ancestor(*o, d))).unwrap_or(group[0].0);
        let mut el = ElemRaw { name: e0.name.clone(), label: None, pos: None };
        for (si, e) in &group {
            if let Some(l) = &e.label {
                match &el.label {
                    None => el.label = Some(l.clone()),
                    Some(old) if old != l => diags.push(diag(
                        Severity::Warning,
                        stmts[*si].line,
                        stmts[*si].col,
                        format!("`{}` already has the label `{old}`; `{l}` is ignored.", e.name),
                    )),
                    _ => {}
                }
            }
            if el.pos.is_none() {
                el.pos = e.pos;
            }
            if *si != deepest {
                lost.insert(*si);
            }
        }
        owner.insert(key.clone(), (deepest, el));
    }

    // ---- node declarations (standalone node statements)
    let mut decl_stmt: HashMap<String, usize> = HashMap::new(); // canon → declaring chain stmt
    let mut bad_stmt: HashSet<usize> = HashSet::new();
    for (si, s) in stmts.iter().enumerate() {
        if let SK::Chain(c) = &s.kind {
            if c.arrows.is_empty() {
                let t = &c.terms[0];
                let key = canon(&t.name);
                match decls.get(&key) {
                    None => {
                        if !is_token(&t.name) {
                            diags.push(diag(Severity::Error, s.line, s.col, format!("`{}` can't be a node's name.", t.name)));
                            bad_stmt.insert(si);
                            continue;
                        }
                        decls.insert(key.clone(), Decl { kind: DK::Node, name: t.name.clone(), line: s.line });
                        decl_stmt.insert(key, si);
                    }
                    Some(d) if matches!(d.kind, DK::Node | DK::GridNode) => {}
                    Some(d) => diags.push(diag(
                        Severity::Warning,
                        s.line,
                        s.col,
                        format!("`{}` is {} (line {}), not a node; this line is ignored.", t.name, kind_word(&d.kind), d.line),
                    )),
                }
            }
        }
    }

    // ---- node attributes: grids first (they win), then every mention in order
    let mut attrs: HashMap<String, Attrs> = HashMap::new();
    for (si, s) in stmts.iter().enumerate() {
        if let SK::Grid { g, .. } = &s.kind {
            for (k, n) in g.nodes.iter().enumerate() {
                if !grid_created[si].get(k).copied().unwrap_or(false) {
                    continue;
                }
                let a = attrs.entry(canon(&n.name)).or_default();
                a.pos = Some((Pos::Cell(n.cell.0, n.cell.1), n.line));
                a.fixed_pos = true;
                if let Some(l) = &n.label {
                    a.label = Some((unicode::to_latex(l), n.line));
                    a.fixed_label = true;
                }
            }
        }
    }
    for s in stmts.iter() {
        if let SK::Chain(c) = &s.kind {
            for t in &c.terms {
                let key = canon(&t.name);
                let is_node = match decls.get(&key) {
                    Some(d) => matches!(d.kind, DK::Node | DK::GridNode),
                    None => resolve_ref(&decls, &t.name).is_none(),
                };
                if !is_node {
                    if t.label.is_some() || t.pos.is_some() {
                        diags.push(diag(Severity::Warning, s.line, t.col + 1, format!("`{}` isn't a node, so its label/position here is ignored.", t.name)));
                    }
                    continue;
                }
                let a = attrs.entry(key).or_default();
                if let Some(l) = &t.label {
                    match &a.label {
                        None => a.label = Some((l.clone(), s.line)),
                        Some((old, ol)) if old != l => diags.push(diag(
                            Severity::Warning,
                            s.line,
                            t.col + 1,
                            format!("`{}` already has the label `{old}` (line {ol}); `{l}` is ignored.", t.name),
                        )),
                        _ => {}
                    }
                }
                if let Some(p) = t.pos {
                    match a.pos {
                        None => a.pos = Some((p, s.line)),
                        Some((old, ol)) if old != p => diags.push(diag(
                            Severity::Warning,
                            s.line,
                            t.col + 1,
                            format!("`{}` was already placed on line {ol}{}; this `at` is ignored.", t.name, if a.fixed_pos { " by its grid" } else { "" }),
                        )),
                        _ => {}
                    }
                }
                if c.arrows.is_empty() {
                    if let Some(col) = &c.node_color {
                        if a.color.is_none() {
                            a.color = Some(col.clone());
                        }
                    }
                }
            }
        }
    }
    let make_node = |name: &str, attrs: &HashMap<String, Attrs>, span: Span| -> Node {
        let a = attrs.get(&canon(name)).cloned().unwrap_or_default();
        Node {
            name: name.to_string(),
            label: a.label.map(|l| l.0).unwrap_or_else(|| default_label(name)),
            pos: a.pos.map(|p| p.0),
            color: a.color,
            span,
        }
    };

    // ---- emit items in order
    let mut fig = Figure { globals, ..Default::default() };
    let mut created_implicit: HashSet<String> = HashSet::new();
    let mut origin: Vec<usize> = Vec::new();
    for (si, s) in stmts.iter().enumerate() {
        origin.resize(fig.items.len(), si.saturating_sub(1));
        let span = Span { line: s.line, col: s.col };
        match &s.kind {
            SK::Blank => fig.items.push(Item::Comment { text: String::new(), span }),
            SK::Comment(t) => fig.items.push(Item::Comment { text: t.clone(), span }),
            SK::Bad(t) => fig.items.push(Item::Comment { text: t.clone(), span }),
            SK::Global => {
                let key = first_word(&s.raw).to_string();
                fig.hints.globals_at.retain(|(k, _)| *k != key);
                fig.hints.globals_at.push((key, fig.items.len()));
            }
            SK::Grid { source, g } => {
                let gi = fig.items.len();
                fig.items.push(Item::Grid { source: source.clone(), span: Span { line: s.line, col: 1 } });
                let gspan = Span { line: s.line, col: 0 };
                for (k, n) in g.nodes.iter().enumerate() {
                    if grid_created[si].get(k).copied().unwrap_or(false) {
                        fig.grid_items.push((fig.items.len(), gi));
                        fig.items.push(Item::Node(make_node(&n.name, &attrs, gspan)));
                    }
                }
                for a in &g.arrows {
                    let (Some(from), Some(to)) = (resolve_ref(&decls, &a.from), resolve_ref(&decls, &a.to)) else { continue };
                    fig.grid_items.push((fig.items.len(), gi));
                    fig.items.push(Item::Arrow(Arrow {
                        is_loop: from == to,
                        from,
                        to,
                        label: a.label.as_ref().map(|l| unicode::to_latex(l)),
                        head: a.head,
                        line: a.line_style,
                        bend: 0.0,
                        side: a.side,
                        label_pos: LabelPos::Middle,
                        iso: false,
                        thick: false,
                        color: None,
                        span: gspan,
                    }));
                }
            }
            SK::Chain(c) => {
                if c.arrows.is_empty() {
                    let key = canon(&c.terms[0].name);
                    if bad_stmt.contains(&si) {
                        fig.items.push(Item::Comment { text: s.raw.clone(), span });
                    } else if decl_stmt.get(&key) == Some(&si) {
                        let nm = decls[&key].name.clone();
                        fig.items.push(Item::Node(make_node(&nm, &attrs, Span { line: s.line, col: c.terms[0].col + 1 })));
                    }
                    continue;
                }
                // all or nothing: a bad name keeps the whole line as written
                let mut bad = false;
                for t in &c.terms {
                    if resolve_ref(&decls, &t.name).is_some() {
                        continue;
                    }
                    if let Some(msg) = bad_vertex_ref(&decls, &t.name) {
                        diags.push(diag(Severity::Error, s.line, t.col + 1, msg));
                        bad = true;
                    } else if !is_token(&t.name) {
                        diags.push(diag(Severity::Error, s.line, t.col + 1, format!("`{}` can't be a node's name.", t.name)));
                        bad = true;
                    }
                }
                if bad {
                    fig.items.push(Item::Comment { text: s.raw.clone(), span });
                    continue;
                }
                for (k, a) in c.arrows.iter().enumerate() {
                    let (ft, tt) = if a.reversed { (&c.terms[k + 1], &c.terms[k]) } else { (&c.terms[k], &c.terms[k + 1]) };
                    let mut ends: Vec<String> = Vec::new();
                    for t in [ft, tt] {
                        match resolve_ref(&decls, &t.name) {
                            Some(n) => ends.push(n),
                            None => {
                                let key = canon(&t.name);
                                if !created_implicit.contains(&key) {
                                    created_implicit.insert(key.clone());
                                    decls.insert(key.clone(), Decl { kind: DK::Node, name: t.name.clone(), line: s.line });
                                    fig.items.push(Item::Node(make_node(&t.name, &attrs, Span { line: s.line, col: t.col + 1 })));
                                    fig.hints.implicit.push(t.name.clone());
                                    diags.push(diag(Severity::Note, s.line, t.col + 1, format!("`{}` is new here, so it became a node.", t.name)));
                                }
                                ends.push(decls[&key].name.clone());
                            }
                        }
                    }
                    let (from, to) = (ends[0].clone(), ends[1].clone());
                    let o = &c.opts;
                    if o.is_loop && from != to {
                        diags.push(diag(Severity::Warning, s.line, s.col, "`loop` is for an arrow from a node to itself (`A -> A loop`); ignored."));
                    }
                    let mut label = a.label.as_ref().map(|l| unicode::to_latex(l));
                    if label.as_deref() == Some("") {
                        label = None;
                    }
                    fig.items.push(Item::Arrow(Arrow {
                        is_loop: from == to,
                        from,
                        to,
                        label,
                        head: a.head,
                        line: o.line.unwrap_or(a.line),
                        bend: o.bend.unwrap_or(0.0),
                        side: if o.swap { LabelSide::Swap } else { LabelSide::Auto },
                        label_pos: o.label_pos,
                        iso: o.iso,
                        thick: o.thick,
                        color: o.color.clone(),
                        span,
                    }));
                }
            }
            SK::Blob(b) => {
                if !blob_ok[si] {
                    fig.items.push(Item::Comment { text: s.raw.clone(), span });
                    continue;
                }
                let mut blob = b.blob.clone();
                blob.parent = parent_of.get(&si).map(|&pi| match &stmts[pi].kind {
                    SK::Blob(pb) => pb.blob.name.clone(),
                    _ => String::new(),
                });
                for e in &b.elems {
                    let key = canon(&e.name);
                    if dropped_elems.contains(&(si, key.clone())) {
                        continue;
                    }
                    if let Some((o, el)) = owner.get(&key) {
                        if *o == si {
                            blob.elements.push(Element { name: el.name.clone(), label: el.label.clone().unwrap_or_else(|| default_label(&el.name)), pos: el.pos });
                        }
                    }
                }
                if lost.contains(&si) {
                    let names: Vec<String> = b
                        .elems
                        .iter()
                        .filter(|e| !dropped_elems.contains(&(si, canon(&e.name))))
                        .filter_map(|e| owner.get(&canon(&e.name)).map(|(_, el)| el.name.clone()))
                        .collect();
                    fig.hints.listed.push((blob.name.clone(), names));
                }
                fig.items.push(Item::Blob(blob));
            }
            SK::NGon { ngon, .. } => {
                if decls.get(&canon(&ngon.name)).map(|d| matches!(d.kind, DK::NGon(..)) && d.line == s.line).unwrap_or(false) {
                    fig.items.push(Item::NGon(ngon.clone()));
                } else {
                    fig.items.push(Item::Comment { text: s.raw.clone(), span });
                }
            }
            SK::Cayley { c, .. } => {
                if decls.get(&canon(&c.name)).map(|d| d.kind == DK::Cayley && d.line == s.line).unwrap_or(false) {
                    fig.items.push(Item::Cayley(c.clone()));
                } else {
                    fig.items.push(Item::Comment { text: s.raw.clone(), span });
                }
            }
            SK::Text(t) => fig.items.push(Item::Text(t.clone())),
        }
    }
    origin.resize(fig.items.len(), stmts.len().saturating_sub(1));
    // (FIG-POLISH) her spelling, and where the globals were written
    let pools: Vec<Vec<(String, String)>> = stmts.iter().map(|s| s.spell.clone()).collect();
    fig.hints.spelling = crate::spelling::assign(&fig, &pools, &origin);
    // (review U14) what she wrote that the formatter would otherwise change:
    // a label equal to the default, and the order of a statement's options
    for i in 0..fig.items.len() {
        let Some(st) = origin.get(i).and_then(|&si| stmts.get(si)) else { continue };
        let (kind, name) = match &fig.items[i] {
            Item::Blob(b) => ("blob", b.name.clone()),
            Item::NGon(g) => ("ngon", g.name.clone()),
            Item::Cayley(c) => ("cayley", c.name.clone()),
            _ => continue,
        };
        let key = format!("{kind}:{name}");
        if let Item::Blob(b) = &fig.items[i] {
            let cs = Phase1::line_chunks(&st.raw, 1);
            if b.label == default_label(&b.name) && cs.get(2).is_some_and(|c| c.is_quoted()) {
                fig.hints.explicit_labels.push(key.clone());
            }
        }
        // (review C11) `turn=-9`: what she wrote, when it isn't the canonical `-(n - turn)`
        if let Item::NGon(g) = &fig.items[i] {
            let k = Phase1::line_chunks(&st.raw, 1).iter().rev().find_map(|c| c.text.strip_prefix("turn=").and_then(|v| v.parse::<i64>().ok()));
            if let Some(k) = k.filter(|k| *k < 0 && g.turn_neg && k.unsigned_abs() != u64::from(g.n.max(1) - g.turn % g.n.max(1))) {
                fig.hints.turn_spelled.push((g.name.clone(), k));
            }
        }
        let written = crate::format::option_heads(&st.raw);
        if written.len() > 1 && written != crate::format::option_heads(&crate::format::item(&fig, i)) {
            fig.hints.option_order.push((key, written));
        }
    }
    let d = Globals::default();
    let g = fig.globals.clone();
    fig.hints.globals_at.retain(|(k, _)| match k.as_str() {
        "scale" => g.scale != d.scale,
        "spacing" => g.spacing != d.spacing,
        _ => g.size != d.size,
    });
    fig.hints.globals_at.sort_by_key(|x| x.1);
    diags.sort_by_key(|d| d.span.line);
    (fig, diags)
}


/// A name in an arrow → the object's stored name: a declared object, an
/// n-gon vertex `P.2` (or `P.b` by label → `P.2`), or a Cayley vertex `C.rs`.
fn resolve_ref(decls: &HashMap<String, Decl>, name: &str) -> Option<String> {
    if let Some(d) = decls.get(&canon(name)) {
        return Some(d.name.clone());
    }
    for (k, _) in name.char_indices().filter(|(_, c)| *c == '.') {
        let (pre, suf) = (&name[..k], &name[k + 1..]);
        if suf.is_empty() {
            continue;
        }
        match decls.get(&canon(pre)) {
            Some(Decl { kind: DK::NGon(n, labels), name: pn, .. }) => {
                if let Ok(v) = suf.parse::<u32>() {
                    if (1..=*n).contains(&v) {
                        return Some(format!("{pn}.{v}"));
                    }
                    return None;
                }
                let lk = unicode::to_latex(suf);
                if let Some(ix) = labels.iter().position(|l| *l == lk || *l == suf) {
                    return Some(format!("{pn}.{}", ix + 1));
                }
                return None;
            }
            Some(Decl { kind: DK::Cayley, name: cn, .. }) => return Some(format!("{cn}.{}", suf.trim())),
            _ => {}
        }
    }
    None
}

fn bad_vertex_ref(decls: &HashMap<String, Decl>, name: &str) -> Option<String> {
    for (k, _) in name.char_indices().filter(|(_, c)| *c == '.') {
        let pre = &name[..k];
        if let Some(Decl { kind: DK::NGon(n, _), name: pn, .. }) = decls.get(&canon(pre)) {
            return Some(format!("`{name}`: `{pn}` has vertices {pn}.1 … {pn}.{n} (or its labels)."));
        }
    }
    None
}
