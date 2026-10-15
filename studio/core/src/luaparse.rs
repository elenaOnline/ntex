//! Just enough Lua to read snippet entries back: a helper call
//! `name(args…)` with string literals (all forms), numbers, booleans, and an
//! opts table of `key = literal` pairs. Anything else → None (the entry opens
//! as raw Lua).
//!
//! "Never wrong": a parsed draft is regenerated with `codegen::entry` and the
//! result read again; unless both say the same thing (helper, trigger, body
//! tokens, origin, every opt), the answer is None.
use crate::codegen;
use crate::model::*;
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// lexer

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum K {
    Name(String),
    Str(String),
    Num(f64),
    Punct(&'static str),
    Comment,
}

#[derive(Clone, Debug)]
pub(crate) struct Tok {
    pub k: K,
    /// byte offsets in the source
    pub start: usize,
    pub end: usize,
    /// 1-based line of `start` / of `end`
    pub line: usize,
    pub end_line: usize,
}

const PUNCTS: &[&str] = &[
    "...", "..", "==", "~=", "<=", ">=", "::", "//", "<<", ">>", "+", "-", "*", "/", "%", "^", "#", "&", "~", "|", "<", ">", "=", "(", ")",
    "{", "}", "[", "]", ";", ":", ",", ".",
];

/// `[` `=`* `[` at `i`: the level.
fn long_open(b: &[u8], i: usize) -> Option<usize> {
    if b.get(i) != Some(&b'[') {
        return None;
    }
    let mut j = i + 1;
    while b.get(j) == Some(&b'=') {
        j += 1;
    }
    if b.get(j) == Some(&b'[') { Some(j - i - 1) } else { None }
}

/// A long bracket's content and the index after it (Lua drops one newline
/// right after the opening bracket; \r\n etc. become \n).
fn long_body(src: &str, i: usize, level: usize) -> Option<(String, usize)> {
    let b = src.as_bytes();
    let mut j = i + level + 2;
    // skip the first newline
    if b.get(j) == Some(&b'\r') {
        j += 1;
        if b.get(j) == Some(&b'\n') {
            j += 1;
        }
    } else if b.get(j) == Some(&b'\n') {
        j += 1;
        if b.get(j) == Some(&b'\r') {
            j += 1;
        }
    }
    let close = format!("]{}]", "=".repeat(level));
    let rel = src[j..].find(&close)?;
    let raw = &src[j..j + rel];
    let val = raw.replace("\r\n", "\n").replace("\n\r", "\n").replace('\r', "\n");
    Some((val, j + rel + close.len()))
}

fn short_string(src: &str, i: usize) -> Option<(String, usize)> {
    let b = src.as_bytes();
    let q = b[i];
    let mut j = i + 1;
    let mut out: Vec<u8> = Vec::new();
    loop {
        let c = *b.get(j)?;
        if c == q {
            return Some((String::from_utf8(out).ok()?, j + 1));
        }
        match c {
            b'\n' | b'\r' => return None,
            b'\\' => {
                let e = *b.get(j + 1)?;
                j += 2;
                match e {
                    b'n' => out.push(b'\n'),
                    b't' => out.push(b'\t'),
                    b'r' => out.push(b'\r'),
                    b'a' => out.push(7),
                    b'b' => out.push(8),
                    b'f' => out.push(12),
                    b'v' => out.push(11),
                    b'\\' => out.push(b'\\'),
                    b'"' => out.push(b'"'),
                    b'\'' => out.push(b'\''),
                    b'\n' => out.push(b'\n'),
                    b'x' => {
                        let h = src.get(j..j + 2)?;
                        out.push(u8::from_str_radix(h, 16).ok()?);
                        j += 2;
                    }
                    b'z' => {
                        while b.get(j).is_some_and(|c| c.is_ascii_whitespace()) {
                            j += 1;
                        }
                    }
                    b'u' => {
                        if b.get(j) != Some(&b'{') {
                            return None;
                        }
                        let close = src[j..].find('}')? + j;
                        let cp = u32::from_str_radix(&src[j + 1..close], 16).ok()?;
                        let ch = char::from_u32(cp)?;
                        let mut buf = [0u8; 4];
                        out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        j = close + 1;
                    }
                    d if d.is_ascii_digit() => {
                        let mut k = j - 1;
                        let mut v: u32 = 0;
                        let mut n = 0;
                        while n < 3 && b.get(k).is_some_and(|c| c.is_ascii_digit()) {
                            v = v * 10 + (b[k] - b'0') as u32;
                            k += 1;
                            n += 1;
                        }
                        if v > 255 {
                            return None;
                        }
                        out.push(v as u8);
                        j = k;
                    }
                    _ => return None,
                }
            }
            _ => {
                out.push(c);
                j += 1;
            }
        }
    }
}

/// Tokenise Lua source (comments included as tokens). None on an
/// unterminated string or comment, or an unknown character.
pub(crate) fn lex(src: &str) -> Option<Vec<Tok>> {
    let b = src.as_bytes();
    let mut toks = Vec::new();
    let mut i = 0;
    let mut line = 1;
    let count_lines = |s: &str| s.bytes().filter(|&c| c == b'\n').count();
    while i < b.len() {
        let c = b[i];
        if c == b'\n' {
            line += 1;
            i += 1;
            continue;
        }
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        let k;
        if c == b'-' && b.get(i + 1) == Some(&b'-') {
            if let Some(level) = long_open(b, i + 2) {
                let (_, end) = long_body(src, i + 2, level)?;
                i = end;
            } else {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            k = K::Comment;
        } else if c == b'"' || c == b'\'' {
            let (v, end) = short_string(src, i)?;
            i = end;
            k = K::Str(v);
        } else if let Some(level) = long_open(b, i) {
            let (v, end) = long_body(src, i, level)?;
            i = end;
            k = K::Str(v);
        } else if c.is_ascii_alphabetic() || c == b'_' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            k = K::Name(src[start..i].to_string());
        } else if c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            if c == b'0' && matches!(b.get(i + 1), Some(b'x') | Some(b'X')) {
                i += 2;
                while i < b.len() && b[i].is_ascii_hexdigit() {
                    i += 1;
                }
                k = K::Num(i64::from_str_radix(&src[start + 2..i], 16).ok()? as f64);
            } else {
                while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                    i += 1;
                }
                if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
                    i += 1;
                    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
                        i += 1;
                    }
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                k = K::Num(src[start..i].parse().ok()?);
            }
        } else {
            let p = PUNCTS.iter().find(|p| src[i..].starts_with(**p))?;
            i += p.len();
            k = K::Punct(p);
        }
        let end_line = line + count_lines(&src[start..i]);
        toks.push(Tok { k, start, end: i, line, end_line });
        line = end_line;
    }
    Some(toks)
}

/// The value of a single Lua string literal (for tests and checks).
pub fn lua_string_value(lit: &str) -> Option<String> {
    let t = lex(lit)?;
    match t.as_slice() {
        [Tok { k: K::Str(s), .. }] => Some(s.clone()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// values and calls

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Val {
    Str(String),
    Num(f64),
    Bool(bool),
    Nil,
    /// `{ key = value, … }` (array items get None keys)
    Table(Vec<(Option<String>, Val)>),
    /// `function(params) body end`
    Func { params: Vec<String>, body: String },
    /// a bare (possibly dotted) name
    Name(String),
}

pub(crate) struct P<'a> {
    pub src: &'a str,
    pub t: Vec<Tok>,
    pub i: usize,
}

impl<'a> P<'a> {
    /// Tokens without comments.
    pub fn new(src: &'a str) -> Option<P<'a>> {
        let t = lex(src)?.into_iter().filter(|t| t.k != K::Comment).collect();
        Some(P { src, t, i: 0 })
    }
    fn peek(&self) -> Option<&K> {
        self.t.get(self.i).map(|t| &t.k)
    }
    fn is(&self, p: &str) -> bool {
        matches!(self.peek(), Some(K::Punct(q)) if *q == p)
    }
    fn eat(&mut self, p: &str) -> bool {
        if self.is(p) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn name(&mut self) -> Option<String> {
        match self.peek() {
            Some(K::Name(n)) => {
                let n = n.clone();
                self.i += 1;
                Some(n)
            }
            _ => None,
        }
    }

    /// One simple value, followed by `,` `;` `)` or `}` (anything else, such
    /// as `..` or a call, is not a literal: None).
    pub fn value(&mut self) -> Option<Val> {
        let v = self.value_inner()?;
        match self.peek() {
            None => Some(v),
            Some(K::Punct(p)) if matches!(*p, "," | ";" | ")" | "}") => Some(v),
            _ => None,
        }
    }

    fn value_inner(&mut self) -> Option<Val> {
        match self.peek()?.clone() {
            K::Str(s) => {
                self.i += 1;
                Some(Val::Str(s))
            }
            K::Num(n) => {
                self.i += 1;
                Some(Val::Num(n))
            }
            K::Punct("-") => {
                self.i += 1;
                match self.peek()? {
                    K::Num(n) => {
                        let n = *n;
                        self.i += 1;
                        Some(Val::Num(-n))
                    }
                    _ => None,
                }
            }
            K::Punct("{") => {
                self.i += 1;
                let mut items = Vec::new();
                loop {
                    if self.eat("}") {
                        return Some(Val::Table(items));
                    }
                    let key = match (self.t.get(self.i).map(|t| &t.k), self.t.get(self.i + 1).map(|t| &t.k)) {
                        (Some(K::Name(n)), Some(K::Punct("="))) => {
                            let n = n.clone();
                            self.i += 2;
                            Some(n)
                        }
                        (Some(K::Punct("[")), Some(K::Str(s))) => {
                            let s = s.clone();
                            self.i += 2;
                            if !self.eat("]") || !self.eat("=") {
                                return None;
                            }
                            Some(s)
                        }
                        _ => None,
                    };
                    let v = self.value()?;
                    items.push((key, v));
                    if !self.eat(",") && !self.eat(";") {
                        if self.eat("}") {
                            return Some(Val::Table(items));
                        }
                        return None;
                    }
                }
            }
            K::Name(n) if n == "true" || n == "false" => {
                self.i += 1;
                Some(Val::Bool(n == "true"))
            }
            K::Name(n) if n == "nil" => {
                self.i += 1;
                Some(Val::Nil)
            }
            K::Name(n) if n == "function" => {
                self.i += 1;
                if !self.eat("(") {
                    return None;
                }
                let mut params = Vec::new();
                while !self.eat(")") {
                    match self.peek()? {
                        K::Name(p) => params.push(p.clone()),
                        K::Punct("...") => params.push("...".into()),
                        _ => return None,
                    }
                    self.i += 1;
                    if !self.eat(",") && !self.is(")") {
                        return None;
                    }
                }
                let body_start = self.t.get(self.i)?.start;
                let mut depth = 1;
                loop {
                    let tk = self.t.get(self.i)?;
                    if let K::Name(w) = &tk.k {
                        match w.as_str() {
                            "function" | "if" | "do" => depth += 1,
                            "end" => {
                                depth -= 1;
                                if depth == 0 {
                                    let body = self.src[body_start..tk.start].trim().to_string();
                                    let body = if tk.start <= body_start { String::new() } else { body };
                                    self.i += 1;
                                    return Some(Val::Func { params, body });
                                }
                            }
                            _ => {}
                        }
                    }
                    self.i += 1;
                }
            }
            K::Name(n) if !is_keyword(&n) => {
                self.i += 1;
                let mut full = n.clone();
                while self.is(".") {
                    self.i += 1;
                    full.push('.');
                    full.push_str(&self.name()?);
                }
                Some(Val::Name(full))
            }
            _ => None,
        }
    }
}

fn is_keyword(s: &str) -> bool {
    matches!(
        s,
        "and" | "break" | "do" | "else" | "elseif" | "end" | "false" | "for" | "function" | "goto" | "if" | "in" | "local" | "nil" | "not"
            | "or" | "repeat" | "return" | "then" | "true" | "until" | "while"
    )
}

/// A helper call: `name(args)` or `h.name(args)`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Call {
    pub qualified: bool,
    pub name: String,
    pub args: Vec<Val>,
}

/// Exactly one call (optionally comma-terminated) and nothing else.
pub(crate) fn parse_call(src: &str) -> Option<Call> {
    let mut p = P::new(src)?;
    let mut name = p.name()?;
    let mut qualified = false;
    if p.eat(".") {
        if name != "h" {
            return None;
        }
        qualified = true;
        name = p.name()?;
    }
    if !p.eat("(") {
        return None;
    }
    let mut args = Vec::new();
    if !p.eat(")") {
        loop {
            args.push(p.value()?);
            if p.eat(",") {
                continue;
            }
            if p.eat(")") {
                break;
            }
            return None;
        }
    }
    p.eat(",");
    if p.i != p.t.len() {
        return None;
    }
    Some(Call { qualified, name, args })
}

// ---------------------------------------------------------------------------
// call → draft

const HELPERS: &[&str] = &["math", "math_tab", "text", "suffix", "post", "pat", "vis"];

/// What a call means, normalised for comparing two calls.
#[derive(Debug, PartialEq)]
struct Meaning {
    helper: String,
    trig: String,
    body: Vec<Piece>,
    origin: String,
    opts: BTreeMap<String, Val>,
}

fn meaning(c: &Call) -> Option<Meaning> {
    if !HELPERS.contains(&c.name.as_str()) {
        return None;
    }
    let mut args = c.args.clone();
    // (trig, body, opts) form
    if args.len() == 3 && matches!(args[2], Val::Table(_)) {
        args.insert(2, Val::Nil);
    }
    if args.len() < 2 || args.len() > 4 {
        return None;
    }
    let trig = match &args[0] {
        Val::Str(s) => s.clone(),
        _ => return None,
    };
    let mut body = match &args[1] {
        Val::Str(s) => codegen::parse_helper_body(s),
        _ => return None,
    };
    // `<0>` after a closing line break is where the cursor ends anyway; codegen
    // writes it (needs_exit), a hand-written entry may not
    if matches!(body.last(), Some(Piece::Exit)) && codegen::needs_exit(&body[..body.len() - 1]) {
        body.pop();
    }
    let mut opts = BTreeMap::new();
    if let Some(Val::Table(items)) = args.get(3) {
        for (k, v) in items {
            let k = k.clone()?;
            if matches!(v, Val::Nil) {
                continue;
            }
            if k == "tab" && *v == Val::Bool(false) {
                continue;
            }
            if opts.insert(k, v.clone()).is_some() {
                return None;
            }
        }
    } else if args.len() == 4 && args[3] != Val::Nil {
        return None;
    }
    let origin = match args.get(2) {
        Some(Val::Str(s)) => s.clone(),
        None | Some(Val::Nil) => match opts.get("origin") {
            Some(Val::Str(s)) => s.clone(),
            None => "new".into(),
            _ => return None,
        },
        _ => return None,
    };
    opts.remove("origin");
    Some(Meaning { helper: c.name.clone(), trig, body, origin, opts })
}

const ALL_PINNED: &[&str] = &["kind", "context", "fires", "math_where", "line", "ft"];

fn draft_of(m: &Meaning) -> Option<Draft> {
    let mut d = Draft { trigger: m.trig.clone(), origin: m.origin.clone(), output: m.body.clone(), ..Draft::default() };
    d.pinned = ALL_PINNED.iter().map(|s| s.to_string()).collect();
    match m.helper.as_str() {
        "math" => {}
        "math_tab" => d.fires = Fires::Tab,
        "text" => d.context = Context::Text,
        "suffix" => d.kind = Kind::Suffix,
        "post" => d.kind = Kind::Post,
        "pat" => d.kind = Kind::Pattern,
        "vis" => d.kind = Kind::Visual,
        _ => return None,
    }
    for (k, v) in &m.opts {
        match (k.as_str(), v) {
            ("tab", Val::Bool(true)) => d.fires = Fires::Tab,
            ("where", Val::Str(s)) => {
                d.math_where = match s.as_str() {
                    "inline" => MathWhere::Inline,
                    "display" => MathWhere::Display,
                    _ => return None,
                }
            }
            ("line", Val::Str(s)) => {
                d.line = match s.as_str() {
                    "start" => LineRule::Start,
                    "after text" => LineRule::AfterText,
                    "list item" => LineRule::ListItem,
                    _ => return None,
                }
            }
            ("ft", Val::Str(s)) => {
                d.ft = match s.as_str() {
                    "tex" => FileType::Tex,
                    "markdown" => FileType::Markdown,
                    _ => return None,
                }
            }
            ("priority", Val::Num(n)) if n.fract() == 0.0 && n.abs() < 1e6 => d.priority = *n as i32,
            ("eg", Val::Str(s)) => d.eg = s.clone(),
            ("desc", Val::Str(s)) => d.desc = s.clone(),
            ("check", Val::Func { params, body }) if params == &["line", "trigger", "captures"] => d.check = body.clone(),
            _ => return None,
        }
    }
    Some(d)
}

/// One entry's source (one helper call) → the draft that would generate it.
pub fn parse_entry(src: &str) -> Option<Draft> {
    let call = parse_call(src)?;
    let m = meaning(&call)?;
    let d = draft_of(&m)?;
    // never wrong: what we would write must mean the same
    let again = parse_call(&codegen::entry(&d))?;
    let m2 = meaning(&again)?;
    if m2 != m {
        return None;
    }
    Some(d)
}

// ---------------------------------------------------------------------------
// files

/// One entry of a snippet list, as byte offsets into the file.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Span {
    /// 1-based line where the entry's text starts (its first comment, else the call)
    pub line: usize,
    /// start of the entry's text (first leading comment, else the call)
    pub start: usize,
    /// start of the call itself
    pub call_start: usize,
    /// end of the call (before the comma)
    pub call_end: usize,
    /// end of the entry: after its comma and a comment on the same line
    pub end: usize,
    pub has_comma: bool,
}

/// The list of `return h.group("…", { … })` (or `return { … }`): the offsets
/// of its `{` and `}`, and its entries.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct List {
    pub open: usize,
    pub close: usize,
    pub spans: Vec<Span>,
}

pub(crate) fn list(file: &str) -> Option<List> {
    let t = lex(file)?;
    let code: Vec<usize> = (0..t.len()).filter(|&i| t[i].k != K::Comment).collect();
    let is = |ci: usize, k: &K| code.get(ci).is_some_and(|&i| &t[i].k == k);
    // find `group ( <str> , {` after a `return`, or `return {`
    let mut open_ci = None;
    for ci in 0..code.len() {
        if !is(ci, &K::Name("return".into())) {
            continue;
        }
        if is(ci + 1, &K::Punct("{")) {
            open_ci = Some(ci + 1);
            break;
        }
        let mut cj = ci + 1;
        if is(cj, &K::Name("h".into())) && is(cj + 1, &K::Punct(".")) {
            cj += 2;
        }
        if is(cj, &K::Name("group".into()))
            && is(cj + 1, &K::Punct("("))
            && code.get(cj + 2).is_some_and(|&i| matches!(t[i].k, K::Str(_)))
            && is(cj + 3, &K::Punct(","))
            && is(cj + 4, &K::Punct("{"))
        {
            open_ci = Some(cj + 4);
            break;
        }
    }
    let open_ci = open_ci?;
    let open_ti = code[open_ci];
    // walk the raw tokens from after `{`
    let mut spans = Vec::new();
    let mut ti = open_ti + 1;
    let mut prev_line = t[open_ti].end_line;
    loop {
        // trailing comments on the previous item's line belong to it
        if let Some(last) = spans.last_mut() {
            let last: &mut Span = last;
            while ti < t.len() && t[ti].k == K::Comment && t[ti].line == prev_line {
                last.end = t[ti].end;
                ti += 1;
            }
        }
        // leading comments
        let mut lead: Option<usize> = None;
        while ti < t.len() && t[ti].k == K::Comment {
            lead.get_or_insert(ti);
            ti += 1;
        }
        let tk = t.get(ti)?;
        if tk.k == K::Punct("}") {
            return Some(List { open: t[open_ti].start, close: tk.start, spans });
        }
        // one item: up to a top-level `,`/`;` or the closing `}`
        let call_ti = ti;
        let mut depth = 0i32;
        let mut last_code = ti;
        loop {
            let tk = t.get(ti)?;
            match &tk.k {
                K::Punct("(") | K::Punct("{") | K::Punct("[") => depth += 1,
                K::Punct(")") | K::Punct("]") => depth -= 1,
                K::Punct("}") => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }
                K::Punct(",") | K::Punct(";") if depth == 0 => break,
                K::Name(w) if w == "function" || w == "if" || w == "do" => depth += 1,
                K::Name(w) if w == "end" => depth -= 1,
                _ => {}
            }
            if tk.k != K::Comment {
                last_code = ti;
            }
            ti += 1;
        }
        let call_end = t[last_code].end;
        let has_comma = matches!(t[ti].k, K::Punct(",") | K::Punct(";"));
        let end = if has_comma { t[ti].end } else { call_end };
        let start_ti = lead.unwrap_or(call_ti);
        spans.push(Span { line: t[start_ti].line, start: t[start_ti].start, call_start: t[call_ti].start, call_end, end, has_comma });
        prev_line = if has_comma { t[ti].end_line } else { t[last_code].end_line };
        if has_comma {
            ti += 1;
        }
    }
}

/// Split a `return h.group("…", { … })` list into its top-level entries:
/// (1-based start line, source text) of each call, comments between them kept
/// with the entry that follows.
pub fn split_entries(file: &str) -> Vec<(usize, String)> {
    match list(file) {
        Some(l) => l.spans.iter().map(|s| (s.line, file[s.start..s.end].to_string())).collect(),
        None => Vec::new(),
    }
}

/// An entry's code without its comments and trailing comma: the call(s) as
/// written (for comparing with what codegen writes).
pub fn call_text(src: &str) -> Option<String> {
    let t = lex(src)?;
    let code: Vec<&Tok> = t.iter().filter(|x| x.k != K::Comment).collect();
    let first = code.first()?.start;
    let last = code.iter().rev().find(|x| !matches!(x.k, K::Punct(",") | K::Punct(";")))?.end;
    Some(src[first..last].to_string())
}

/// The trigger of the first helper call in `src` (for lists), if readable.
pub fn trigger_of(src: &str) -> Option<String> {
    let p = P::new(src)?;
    let t = &p.t;
    for i in 0..t.len() {
        if let K::Name(n) = &t[i].k {
            if is_keyword(n) || n == "h" {
                continue;
            }
            if t.get(i + 1).map(|x| &x.k) == Some(&K::Punct("(")) {
                if let Some(K::Str(s)) = t.get(i + 2).map(|x| &x.k) {
                    return Some(s.clone());
                }
                return None;
            }
        }
    }
    None
}

/// The top-level helper calls in an entry: (byte offset of the name, name,
/// already `h.`-qualified).
pub(crate) fn calls_in(src: &str) -> Vec<(usize, String, bool)> {
    let Some(t) = lex(src) else { return Vec::new() };
    let t: Vec<Tok> = t.into_iter().filter(|x| x.k != K::Comment).collect();
    let mut out = Vec::new();
    let mut depth = 0;
    for i in 0..t.len() {
        match &t[i].k {
            K::Punct("(") | K::Punct("{") | K::Punct("[") => depth += 1,
            K::Punct(")") | K::Punct("}") | K::Punct("]") => depth -= 1,
            K::Name(n) if depth == 0 && t.get(i + 1).map(|x| &x.k) == Some(&K::Punct("(")) => {
                let qualified = i >= 2 && t[i - 1].k == K::Punct(".") && t[i - 2].k == K::Name("h".into());
                let dotted = i >= 1 && t[i - 1].k == K::Punct(".");
                if dotted && !qualified {
                    continue;
                }
                out.push((t[i].start, n.clone(), qualified));
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings() {
        assert_eq!(lua_string_value(r#""a\\b\"c\n""#).as_deref(), Some("a\\b\"c\n"));
        assert_eq!(lua_string_value("[[\nx]]").as_deref(), Some("x"));
        assert_eq!(lua_string_value("[[\n\nx]]").as_deref(), Some("\nx"));
        assert_eq!(lua_string_value("[==[a]]=]b]==]").as_deref(), Some("a]]=]b"));
        assert_eq!(lua_string_value(r#"'"'"#).as_deref(), Some("\""));
        assert_eq!(lua_string_value(r#""\65\x42\u{43}""#).as_deref(), Some("ABC"));
    }

    #[test]
    fn entries() {
        let d = parse_entry(r#"  math("ff", [[\frac{<>}{<>}]], "yours"),"#).unwrap();
        assert_eq!(d.trigger, "ff");
        assert_eq!(d.kind, Kind::Plain);
        assert_eq!(d.output.len(), 5);
        let d = parse_entry(r#"text("dm", [[x]], "yours", { line = "after text", priority = 1, desc = "D" })"#).unwrap();
        assert_eq!((d.context, d.line, d.priority, d.desc.as_str()), (Context::Text, LineRule::AfterText, 1, "D"));
        let d = parse_entry(r#"h.pat("pa(%a)(%a)", [[\frac{%1}{%2}]], "default", { tab = true, eg = "payx" })"#).unwrap();
        assert_eq!((d.kind, d.fires, d.eg.as_str()), (Kind::Pattern, Fires::Tab, "payx"));
        let d = parse_entry(r#"math("x", [[y]], "o", { check = function(line, trigger, captures) return #line > 2 end })"#).unwrap();
        assert_eq!(d.check, "return #line > 2");
        // not drafts
        assert!(parse_entry(r#"post("dot", [[\dot{%1}]], "default", { priority = -1, check = not_c }),"#).is_none());
        assert!(parse_entry(r#"env("pmat", "pmatrix", "default"),"#).is_none());
        assert!(parse_entry(r#"text("dm", block, "changed"),"#).is_none());
        assert!(parse_entry(r#"text("x", [[y]], "o", { where = "display" }),"#).is_none());
        assert!(parse_entry(r#"math("x", [[y]], "o", { tab = true }),"#).is_none());
        assert!(parse_entry(r#"math("x", [[y]] .. "z", "o"),"#).is_none());
        assert!(parse_entry(r#"math("a", [[b]], "o"), math("c", [[d]], "o"),"#).is_none());
    }

    #[test]
    fn split() {
        let f = "local h = require('snips.helpers')\nreturn h.group(\"T\", {\n  -- head\n  math(\"a\", [[b]], \"o\"), -- tail\n\n  math(\">=\", [[\\geq]], \"d\"), math(\"geq\", [[\\geq]], \"d\"),\n  text(\"beg\", [[\nx\n]], \"c\", { tab = true })\n})\n";
        let e = split_entries(f);
        assert_eq!(e.len(), 4);
        assert_eq!(e[0], (3, "-- head\n  math(\"a\", [[b]], \"o\"), -- tail".to_string()));
        assert_eq!(e[1].0, 6);
        assert_eq!(e[2], (6, "math(\"geq\", [[\\geq]], \"d\"),".to_string()));
        assert_eq!(e[3].0, 7);
        assert!(e[3].1.ends_with("{ tab = true })"));
        assert_eq!(trigger_of(&e[0].1).as_deref(), Some("a"));
        assert_eq!(trigger_of("h.math([[\\int]], [[x]])").as_deref(), Some("\\int"));
    }

    /// Every entry of every real snippet file (read-only): how many parse to a
    /// draft, and how many regenerate to the same text modulo whitespace.
    #[test]
    fn real_files() {
        let dir = std::path::PathBuf::from(std::env::var("HOME").unwrap()).join(".config/texnvim/snippets/tex");
        let Ok(rd) = std::fs::read_dir(&dir) else { return };
        let mut files: Vec<_> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "lua")).collect();
        files.sort();
        let squash = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        let (mut total, mut drafts, mut same) = (0, 0, 0);
        let mut diffs = Vec::new();
        let mut nones = Vec::new();
        for f in &files {
            let name = f.file_name().unwrap().to_string_lossy().to_string();
            if name == "inbox.lua" || name.starts_with("zz_") {
                continue;
            }
            let text = std::fs::read_to_string(f).unwrap();
            let entries = split_entries(&text);
            assert!(!entries.is_empty(), "{name}: no entries found");
            for (line, src) in entries {
                total += 1;
                // the call alone (comments are not regenerated)
                let call: String = {
                    let p = P::new(&src).unwrap();
                    let first = p.t.first().unwrap().start;
                    let last = p.t.iter().rev().find(|t| t.k != K::Punct(",")).unwrap().end;
                    src[first..last].to_string()
                };
                match parse_entry(&src) {
                    Some(d) => {
                        drafts += 1;
                        let regen = codegen::entry(&d);
                        let regen = regen.trim().trim_end_matches(',');
                        if squash(regen) == squash(&call) {
                            same += 1;
                        } else {
                            diffs.push(format!("{name}:{line}\n    was {call}\n    now {regen}"));
                        }
                    }
                    None => nones.push(format!("{name}:{line} {}", call.lines().next().unwrap_or(""))),
                }
            }
        }
        eprintln!("luaparse on real files: {total} entries, {drafts} parse to a Draft, {same} regenerate identically (modulo whitespace)");
        eprintln!("-- regenerated differently:\n{}", diffs.join("\n"));
        eprintln!("-- not drafts (None):\n{}", nones.join("\n"));
        assert!(total > 200);
        assert!(drafts * 10 >= total * 8, "{drafts}/{total}");
    }
}
