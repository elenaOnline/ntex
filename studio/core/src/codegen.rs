//! Draft → the Lua line(s) of a snippet file, in the author's helper vocabulary
//! (lua/snips/helpers.lua, snippets/README.md). Output must look like her
//! files: `  math("ff", [[\frac{<>}{<>}]], "yours"),`
use crate::model::*;

/// The helper a draft becomes, and whether it needs `tab = true`.
pub fn helper_name(draft: &Draft) -> &'static str {
    match draft.kind {
        Kind::Plain => match (draft.context, draft.fires) {
            (Context::Math, Fires::Auto) => "math",
            (Context::Math, Fires::Tab) => "math_tab",
            (Context::Text, _) => "text",
        },
        Kind::Suffix => "suffix",
        Kind::Post => "post",
        Kind::Pattern => "pat",
        Kind::Visual => "vis",
        Kind::RawLua => "",
    }
}

/// The entry's Lua: one helper call (two for nothing — `Context` is one or the
/// other), comma-terminated, two-space indented, no marker line. For
/// `Kind::RawLua`, `draft.raw_lua` verbatim (trimmed, comma ensured).
pub fn entry(draft: &Draft) -> String {
    if draft.kind == Kind::RawLua {
        let t = draft.raw_lua.trim();
        if t.is_empty() {
            return String::new();
        }
        // several calls become one list item (h.group flattens nested lists),
        // so the inbox keeps them as one entry under one marker
        if crate::luaparse::calls_in(t).len() > 1 {
            let inner = t.trim_end_matches(|c: char| c == ',' || c.is_whitespace());
            return format!("  {{ {inner} }},");
        }
        let mut s = format!("  {t}");
        if !t.ends_with(',') {
            s.push(',');
        }
        return s;
    }
    let helper = helper_name(draft);
    let origin = if draft.origin.is_empty() { "yours" } else { draft.origin.as_str() };
    let mut output = draft.output.clone();
    if needs_exit(&output) {
        output.push(Piece::Exit);
    }
    let mut s = format!(
        "  {helper}({}, {}, {}",
        trigger_literal(&draft.trigger),
        lua_string(&body(&output)),
        quoted(origin)
    );
    let opts = opts(draft);
    if !opts.is_empty() {
        s.push_str(", { ");
        s.push_str(&opts.join(", "));
        s.push_str(" }");
    }
    s.push_str("),");
    s
}

/// Does the output end in whitespace holding a line break, with no exit
/// point? helpers.lua splits trailing whitespace off into a function node
/// (dropped in inline maths), and LuaSnip fails on one holding a newline
/// (and stays broken until restart), so `entry` writes `<0>` after it: the
/// cursor ends there anyway.
pub fn needs_exit(pieces: &[Piece]) -> bool {
    if pieces.iter().any(|p| matches!(p, Piece::Exit | Piece::Tab { n: 0, .. })) {
        return false;
    }
    match pieces.last() {
        Some(Piece::Text(t)) => {
            let ws: String = t.chars().rev().take_while(|c| c.is_whitespace()).collect();
            ws.contains('\n') || ws.contains('\r')
        }
        _ => false,
    }
}

/// The opts pairs, in the house order: `tab`, `where`, `line`, `ft`,
/// `priority`, `eg`, `desc`, `check` (`tab` first, as in her files:
/// `{ tab = true, line = "start", ft = "tex" }`).
fn opts(d: &Draft) -> Vec<String> {
    let mut o = Vec::new();
    let maths = d.context == Context::Math || matches!(d.kind, Kind::Suffix | Kind::Post | Kind::Pattern | Kind::Visual);
    let tab = d.fires == Fires::Tab
        && (matches!(d.kind, Kind::Pattern) || (d.kind == Kind::Plain && d.context == Context::Text));
    if tab {
        o.push("tab = true".to_string());
    }
    if maths {
        match d.math_where {
            MathWhere::Any => {}
            MathWhere::Inline => o.push("where = \"inline\"".into()),
            MathWhere::Display => o.push("where = \"display\"".into()),
        }
    }
    if d.kind == Kind::Plain && d.context == Context::Text {
        match d.line {
            LineRule::Anywhere => {}
            LineRule::Start => o.push("line = \"start\"".into()),
            LineRule::AfterText => o.push("line = \"after text\"".into()),
            LineRule::ListItem => o.push("line = \"list item\"".into()),
        }
    }
    match d.ft {
        FileType::Both => {}
        FileType::Tex => o.push("ft = \"tex\"".into()),
        FileType::Markdown => o.push("ft = \"markdown\"".into()),
    }
    if d.priority != 0 {
        o.push(format!("priority = {}", d.priority));
    }
    if !d.eg.is_empty() {
        o.push(format!("eg = {}", quoted(&d.eg)));
    }
    if !d.desc.is_empty() {
        o.push(format!("desc = {}", quoted(&d.desc)));
    }
    if !d.check.trim().is_empty() {
        o.push(format!("check = function(line, trigger, captures) {} end", d.check.trim()));
    }
    o
}

/// A trigger as she writes them: `"ff"`, `'"'` when it holds a double quote,
/// `[[\int]]` when it holds a backslash.
pub fn trigger_literal(s: &str) -> String {
    if s.contains('\\') && !s.contains('\n') && !s.contains('\r') {
        let l = lua_string(s);
        if l.starts_with('[') {
            return l;
        }
    }
    if s.contains('"') && !s.contains('\'') && !s.contains('\\') && !s.chars().any(|c| c.is_control()) {
        return format!("'{s}'");
    }
    quoted(s)
}

/// `"…"` with Lua escapes.
pub fn quoted(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' => o.push_str("\\n"),
            '\t' => o.push_str("\\t"),
            '\r' => o.push_str("\\r"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => o.push_str(&format!("\\{:03}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// One char of the body being written, and whether it is a literal `<`/`>`
/// from text (a candidate for the `%<`/`%>` escape).
struct Out {
    chars: Vec<char>,
    literal: Vec<bool>,
}

impl Out {
    fn token(&mut self, s: &str) {
        for c in s.chars() {
            self.chars.push(c);
            self.literal.push(false);
        }
    }
    fn text(&mut self, s: &str) {
        for c in s.chars() {
            if c == '%' {
                self.token("%%");
            } else {
                self.chars.push(c);
                self.literal.push(c == '<' || c == '>');
            }
        }
    }
}

/// Does the helper syntax read a tab stop at `i` (`<` …)? Returns the index of
/// its closing `>`. Mirrors helpers.parse: `^<(%d*)(:?)([^<>\n]*)>` with a
/// colon or an empty default.
pub(crate) fn tabstop_at(s: &[char], i: usize) -> Option<usize> {
    if s.get(i) != Some(&'<') {
        return None;
    }
    let mut j = i + 1;
    while j < s.len() && s[j].is_ascii_digit() {
        j += 1;
    }
    match s.get(j) {
        Some('>') => Some(j),
        Some(':') => {
            let mut k = j + 1;
            while k < s.len() && !matches!(s[k], '<' | '>' | '\n') {
                k += 1;
            }
            if s.get(k) == Some(&'>') { Some(k) } else { None }
        }
        _ => None,
    }
}

/// Should tab stops be written unnumbered? Yes when each appears once and in
/// order 1, 2, 3 …
fn unnumbered(pieces: &[Piece]) -> bool {
    let mut want = 1;
    for p in pieces {
        if let Piece::Tab { n, .. } = p {
            if *n == 0 {
                continue;
            }
            if *n != want {
                return false;
            }
            want += 1;
        }
    }
    true
}

/// When tab stops must be numbered, the one that can still be written `<>`
/// the way her files do (`plot`, `bigfun` in text.lua): the first stop if it
/// is 1, else the last if it is the highest; typed once, with no default, and
/// only if helpers.parse would give it that very number. Piece indices.
fn bare_stops(pieces: &[Piece]) -> Vec<usize> {
    let tabs: Vec<(usize, u32, bool)> = pieces
        .iter()
        .enumerate()
        .filter_map(|(i, p)| match p {
            Piece::Tab { n, default } if *n > 0 => Some((i, *n, default.is_empty())),
            _ => None,
        })
        .collect();
    let count = |n: u32| tabs.iter().filter(|t| t.1 == n).count();
    let mut cands = Vec::new();
    if let Some(&(i, n, empty)) = tabs.first() {
        if n == 1 && empty && count(1) == 1 {
            cands.push(i);
        }
    }
    let max = tabs.iter().map(|t| t.1).max().unwrap_or(0);
    if let Some(&(i, n, empty)) = tabs.last() {
        if n == max && empty && count(n) == 1 && !cands.contains(&i) {
            cands.push(i);
        }
    }
    // the bare ones take the free numbers in reading order (helpers.parse)
    let works = |bare: &[usize]| {
        let explicit: std::collections::HashSet<u32> = tabs.iter().filter(|t| !bare.contains(&t.0)).map(|t| t.1).collect();
        let mut next = 1;
        tabs.iter().filter(|t| bare.contains(&t.0)).all(|t| {
            while explicit.contains(&next) {
                next += 1;
            }
            let ok = next == t.1;
            next += 1;
            ok
        })
    };
    // at most one, the first if it can be (her `plot` writes `[<>]` first
    // and `<9>` last; `bigfun` ends in `<>`)
    let options: Vec<Vec<usize>> = cands.iter().map(|&c| vec![c]).collect();
    options.into_iter().find(|o| works(o)).unwrap_or_default()
}

/// The output as a helper body string: `\frac{<>}{<>}` (tab stops `<>`,
/// `<:x>`, `<1>`, `<0>`, `%1`, `%V`, `%%`, and `%<` `%>` for literal angle
/// brackets that would otherwise read as a tab stop).
pub fn body(pieces: &[Piece]) -> String {
    let plain = unnumbered(pieces);
    let bare = if plain { Vec::new() } else { bare_stops(pieces) };
    let mut out = Out { chars: Vec::new(), literal: Vec::new() };
    for (idx, p) in pieces.iter().enumerate() {
        match p {
            Piece::Text(t) => out.text(t),
            Piece::Tab { n: 0, .. } | Piece::Exit => out.token("<0>"),
            Piece::Tab { n, default } => {
                let num = if plain || bare.contains(&idx) { String::new() } else { n.to_string() };
                if default.is_empty() {
                    out.token(&format!("<{num}>"));
                } else {
                    out.token(&format!("<{num}:{default}>"));
                }
            }
            Piece::Capture(k) => out.token(&format!("%{k}")),
            Piece::Selection => out.token("%V"),
        }
    }
    // escape literal `<…>` that would read as a tab stop
    let mut esc = vec![false; out.chars.len()];
    let mut i = 0;
    while i < out.chars.len() {
        if out.literal[i] && out.chars[i] == '<' {
            if let Some(j) = tabstop_at(&out.chars, i) {
                esc[i] = true;
                if out.literal[j] {
                    esc[j] = true;
                }
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    let mut s = String::with_capacity(out.chars.len() + 8);
    for (k, c) in out.chars.iter().enumerate() {
        if esc[k] {
            s.push('%');
        }
        s.push(*c);
    }
    s
}

/// A Lua string literal for `s`: `[[…]]` (or `[=[…]=]`, … when `s` holds
/// `]]`; an extra leading newline when `s` starts with one, since Lua drops
/// the first), or `"…"` with escapes when a long string can't hold it.
///
/// House style: a multi-line string starts on the line after `[[` (that first
/// newline is dropped by Lua), as `beg` and `table` do in her files.
pub fn lua_string(s: &str) -> String {
    // long strings turn \r and \r\n into \n: they cannot hold a carriage return
    if s.contains('\r') {
        return quoted(s);
    }
    // a short bracket-free string ending in `]`: she writes "[%V]"
    if s.ends_with(']') && !s.contains('\\') && !s.contains('"') && !s.contains('\n') && !s.chars().any(|c| c.is_control()) {
        return quoted(s);
    }
    let mut level = 0;
    loop {
        let eq = "=".repeat(level);
        let close = format!("]{eq}]");
        let probe = format!("{s}{close}");
        // (Lua 5.1 also refuses a nested `[[` in a level-0 long string)
        if probe.find(&close) == Some(s.len()) && !s.contains(&format!("[{eq}[")) {
            let nl = if s.contains('\n') { "\n" } else { "" };
            return format!("[{eq}[{nl}{s}{close}");
        }
        level += 1;
    }
}

/// Plain text of the output with tab stops shown as `□` (their default text if
/// they have one), `%V` as `sel`, captures as `x` — for lists and previews.
pub fn plain(pieces: &[Piece]) -> String {
    let defaults = first_defaults(pieces);
    let mut s = String::new();
    for p in pieces {
        match p {
            Piece::Text(t) => s.push_str(t),
            Piece::Tab { n, default } if *n > 0 => {
                let d = if default.is_empty() { defaults.get(n).cloned().unwrap_or_default() } else { default.clone() };
                if d.is_empty() { s.push('□') } else { s.push_str(&d) }
            }
            Piece::Tab { .. } | Piece::Exit => {}
            Piece::Capture(_) => s.push('x'),
            Piece::Selection => s.push_str("sel"),
        }
    }
    s
}

fn first_defaults(pieces: &[Piece]) -> std::collections::HashMap<u32, String> {
    let mut m = std::collections::HashMap::new();
    for p in pieces {
        if let Piece::Tab { n, default } = p {
            if !default.is_empty() {
                m.entry(*n).or_insert_with(|| default.clone());
            }
        }
    }
    m
}

/// The blue box a tab stop renders as. Needs xcolor (the preview preamble
/// loads it); works in text and in maths.
pub const PREVIEW_TABSTOP: &str = r"\textcolor[HTML]{1E47B8}{\ensuremath{\square}}";

/// The output as LaTeX for the rendered preview: tab stops as a blue
/// `\square` (or their default text), mirrors the same, exit as nothing,
/// `%V` as `x+y`, captures as `x`.
pub fn preview_latex(pieces: &[Piece]) -> String {
    let defaults = first_defaults(pieces);
    let mut s = String::new();
    for p in pieces {
        match p {
            Piece::Text(t) => s.push_str(t),
            Piece::Tab { n, default } if *n > 0 => {
                let d = if default.is_empty() { defaults.get(n).cloned().unwrap_or_default() } else { default.clone() };
                if d.is_empty() { s.push_str(PREVIEW_TABSTOP) } else { s.push_str(&d) }
            }
            Piece::Tab { .. } | Piece::Exit => {}
            Piece::Capture(_) => s.push('x'),
            Piece::Selection => s.push_str("x+y"),
        }
    }
    s
}

/// Pieces builder: merges adjacent text.
struct Pieces(Vec<Piece>);

impl Pieces {
    fn text(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        if let Some(Piece::Text(t)) = self.0.last_mut() {
            t.push_str(s);
        } else {
            self.0.push(Piece::Text(s.to_string()));
        }
    }
    fn push(&mut self, p: Piece) {
        self.0.push(p);
    }
}

/// Exactly what helpers.parse reads (plus the `%<` / `%>` escape): the body of
/// a snippet in a Lua file. `<>` get the next free numbers.
pub fn parse_helper_body(s: &str) -> Vec<Piece> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Pieces(Vec::new());
    // (index into out, is unnumbered)
    let mut pending: Vec<usize> = Vec::new();
    let mut explicit = std::collections::HashSet::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch == '<' {
            if let Some(j) = tabstop_at(&c, i) {
                let inner: String = c[i + 1..j].iter().collect();
                let (num, default) = match inner.find(':') {
                    Some(k) => (&inner[..k], &inner[k + 1..]),
                    None => (inner.as_str(), ""),
                };
                if num.is_empty() {
                    pending.push(out.0.len());
                    out.push(Piece::Tab { n: u32::MAX, default: default.to_string() });
                } else {
                    let n: u32 = num.parse().unwrap_or(u32::MAX - 1);
                    if n == 0 {
                        out.push(Piece::Exit);
                    } else {
                        explicit.insert(n);
                        out.push(Piece::Tab { n, default: default.to_string() });
                    }
                }
                i = j + 1;
                continue;
            }
            out.text("<");
            i += 1;
        } else if ch == '%' {
            match c.get(i + 1) {
                Some(d) if d.is_ascii_digit() => {
                    out.push(Piece::Capture(d.to_digit(10).unwrap() as u8));
                    i += 2;
                }
                Some('V') => {
                    out.push(Piece::Selection);
                    i += 2;
                }
                Some('%') => {
                    out.text("%");
                    i += 2;
                }
                Some('<') => {
                    out.text("<");
                    i += 2;
                }
                Some('>') => {
                    out.text(">");
                    i += 2;
                }
                _ => {
                    out.text("%");
                    i += 1;
                }
            }
        } else {
            let start = i;
            while i < c.len() && c[i] != '<' && c[i] != '%' {
                i += 1;
            }
            let t: String = c[start..i].iter().collect();
            out.text(&t);
        }
    }
    let mut next = 1;
    for idx in pending {
        while explicit.contains(&next) {
            next += 1;
        }
        if let Piece::Tab { n, .. } = &mut out.0[idx] {
            *n = next;
        }
        next += 1;
    }
    out.0
}

/// Does `s` hold helper-syntax tokens (tab stops, `%V`, `%1`)?
fn has_helper_tokens(s: &str) -> bool {
    let c: Vec<char> = s.chars().collect();
    for i in 0..c.len() {
        if c[i] == '<' && tabstop_at(&c, i).is_some() {
            return true;
        }
        if c[i] == '%' {
            if let Some(n) = c.get(i + 1) {
                if n.is_ascii_digit() || *n == 'V' {
                    return true;
                }
            }
        }
    }
    false
}

/// A VS Code / UltiSnips token at `i` (`$1`, `${1}`, `${1:x}`, `$0`,
/// `${VISUAL}`, `$TM_SELECTED_TEXT`): the piece and the index after it.
fn vscode_at(c: &[char], i: usize) -> Option<(Piece, usize)> {
    if c.get(i) != Some(&'$') {
        return None;
    }
    let rest: String = c[i + 1..].iter().take(24).collect();
    for name in ["{VISUAL}", "{TM_SELECTED_TEXT}", "TM_SELECTED_TEXT", "VISUAL"] {
        if rest.starts_with(name) {
            // `$VISUAL` bare only when not followed by a word char
            let end = i + 1 + name.chars().count();
            if !name.starts_with('{') && c.get(end).is_some_and(|x| x.is_alphanumeric() || *x == '_') {
                continue;
            }
            return Some((Piece::Selection, end));
        }
    }
    if c.get(i + 1).is_some_and(|d| d.is_ascii_digit()) {
        let mut j = i + 1;
        while j < c.len() && c[j].is_ascii_digit() {
            j += 1;
        }
        let n: u32 = c[i + 1..j].iter().collect::<String>().parse().ok()?;
        return Some((if n == 0 { Piece::Exit } else { Piece::Tab { n, default: String::new() } }, j));
    }
    if c.get(i + 1) == Some(&'{') {
        let mut j = i + 2;
        while j < c.len() && c[j].is_ascii_digit() {
            j += 1;
        }
        if j == i + 2 {
            return None;
        }
        let n: u32 = c[i + 2..j].iter().collect::<String>().parse().ok()?;
        match c.get(j) {
            Some('}') => return Some((if n == 0 { Piece::Exit } else { Piece::Tab { n, default: String::new() } }, j + 1)),
            Some(':') => {
                // default up to the matching brace (nested ${…} flattened to their text)
                let mut depth = 1;
                let mut k = j + 1;
                let mut def = String::new();
                while k < c.len() {
                    match c[k] {
                        '\\' if matches!(c.get(k + 1), Some('}') | Some('$')) => {
                            def.push(c[k + 1]);
                            k += 2;
                            continue;
                        }
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    def.push(c[k]);
                    k += 1;
                }
                if depth != 0 {
                    return None;
                }
                let def = strip_nested_vscode(&def);
                return Some((if n == 0 { Piece::Exit } else { Piece::Tab { n, default: def } }, k + 1));
            }
            _ => return None,
        }
    }
    None
}

/// `foo ${2:bar} $3` → `foo bar ` (a default can't hold tab stops).
fn strip_nested_vscode(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut o = String::new();
    let mut i = 0;
    while i < c.len() {
        if let Some((p, j)) = vscode_at(&c, i) {
            if let Piece::Tab { default, .. } = p {
                o.push_str(&default);
            }
            i = j;
        } else {
            o.push(c[i]);
            i += 1;
        }
    }
    o
}

/// Helper-syntax text → pieces: understands `<>`, `<:x>`, `<n>`, `<n:x>`,
/// `<0>`, `%1`…`%9`, `%V`, `%%`, `%<`, `%>`, and also pasted UltiSnips /
/// VS Code syntax: `$1`, `${1:x}`, `$0`, `${VISUAL}`, `$TM_SELECTED_TEXT`.
/// Unnumbered `<>` get the next free numbers, as helpers.parse does.
///
/// Text with helper tokens is read as helper syntax only (`$` stays a
/// dollar); otherwise VS Code tokens are recognised (`%` rules apply in both).
pub fn parse_body(s: &str) -> Vec<Piece> {
    if has_helper_tokens(s) || !s.contains('$') {
        return parse_helper_body(s);
    }
    let c: Vec<char> = s.chars().collect();
    let mut out = Pieces(Vec::new());
    let mut i = 0;
    let mut buf = String::new();
    while i < c.len() {
        // `\$` is LaTeX's dollar sign, not a VS Code token
        if c[i] == '$' && !(i > 0 && c[i - 1] == '\\') {
            if let Some((p, j)) = vscode_at(&c, i) {
                for q in parse_helper_body(&std::mem::take(&mut buf)) {
                    match q {
                        Piece::Text(t) => out.text(&t),
                        q => out.push(q),
                    }
                }
                out.push(p);
                i = j;
                continue;
            }
        }
        buf.push(c[i]);
        i += 1;
    }
    for q in parse_helper_body(&buf) {
        match q {
            Piece::Text(t) => out.text(&t),
            q => out.push(q),
        }
    }
    out.0
}

/// Renumber tab stops 1, 2, 3 … in order of first appearance, keeping mirrors
/// together (call after every edit of the token editor). A mirror's default
/// moves to the first occurrence (helpers.lua only reads that one), and a
/// `Tab { n: 0 }` becomes `Exit`.
pub fn renumber(pieces: &mut Vec<Piece>) {
    let mut map: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    let mut first_idx: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    let mut next = 1;
    for idx in 0..pieces.len() {
        if let Piece::Tab { n: 0, .. } = pieces[idx] {
            pieces[idx] = Piece::Exit;
            continue;
        }
        if let Piece::Tab { n, default } = &mut pieces[idx] {
            let new = *map.entry(*n).or_insert_with(|| {
                let v = next;
                next += 1;
                v
            });
            *n = new;
            match first_idx.get(&new) {
                None => {
                    first_idx.insert(new, idx);
                }
                Some(&f) => {
                    if !default.is_empty() {
                        let d = std::mem::take(default);
                        if let Piece::Tab { default: fd, .. } = &mut pieces[f] {
                            if fd.is_empty() {
                                *fd = d;
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(n: u32) -> Piece {
        Piece::Tab { n, default: String::new() }
    }
    fn tabd(n: u32, d: &str) -> Piece {
        Piece::Tab { n, default: d.into() }
    }
    fn t(s: &str) -> Piece {
        Piece::Text(s.into())
    }
    fn draft(trigger: &str, out: Vec<Piece>) -> Draft {
        Draft { trigger: trigger.into(), output: out, ..Draft::default() }
    }

    #[test]
    fn entry_plain_maths() {
        let d = draft("ff", vec![t(r"\frac{"), tab(1), t("}{"), tab(2), t("}")]);
        assert_eq!(entry(&d), r#"  math("ff", [[\frac{<>}{<>}]], "yours"),"#);
    }

    #[test]
    fn entry_helpers_and_opts() {
        let mut d = draft("table", parse_body("\\begin{table}\n\t<>\n\\end{table}"));
        d.context = Context::Text;
        d.fires = Fires::Tab;
        d.line = LineRule::Start;
        d.ft = FileType::Tex;
        d.desc = "A table".into();
        d.origin = "castel".into();
        assert_eq!(
            entry(&d),
            "  text(\"table\", [[\n\\begin{table}\n\t<>\n\\end{table}]], \"castel\", { tab = true, line = \"start\", ft = \"tex\", desc = \"A table\" }),"
        );
        let mut d = draft("sr", parse_body("^{2}"));
        d.kind = Kind::Suffix;
        assert_eq!(entry(&d), r#"  suffix("sr", [[^{2}]], "yours"),"#);
        let mut d = draft("hat", parse_body(r"\hat{%1}"));
        d.kind = Kind::Post;
        d.priority = -1;
        assert_eq!(entry(&d), r#"  post("hat", [[\hat{%1}]], "yours", { priority = -1 }),"#);
        let mut d = draft("pa(%a)(%a)", parse_body(r"\frac{ \partial %1 }{ \partial %2 } "));
        d.kind = Kind::Pattern;
        d.fires = Fires::Tab;
        d.eg = "payx".into();
        assert_eq!(entry(&d), r#"  pat("pa(%a)(%a)", [[\frac{ \partial %1 }{ \partial %2 } ]], "yours", { tab = true, eg = "payx" }),"#);
        let mut d = draft("S", parse_body(r"\sqrt{ %V }"));
        d.kind = Kind::Visual;
        assert_eq!(entry(&d), r#"  vis("S", [[\sqrt{ %V }]], "yours"),"#);
        let mut d = draft("par", parse_body(r"\frac{ \partial <:y> }{ \partial <:x> } "));
        d.fires = Fires::Tab;
        d.math_where = MathWhere::Display;
        d.check = "return true".into();
        assert_eq!(
            entry(&d),
            r#"  math_tab("par", [[\frac{ \partial <:y> }{ \partial <:x> } ]], "yours", { where = "display", check = function(line, trigger, captures) return true end }),"#
        );
        let mut d = draft("mk", parse_body("$<>$"));
        d.context = Context::Text;
        assert_eq!(entry(&d), r#"  text("mk", [[$<>$]], "yours"),"#);
    }

    #[test]
    fn entry_trigger_forms() {
        let d = draft(r"\int", parse_body(r"\int <> \, d<:x> "));
        assert!(entry(&d).starts_with(r#"  math([[\int]], "#));
        let d = draft("\"", parse_body(r"\text{ <> }"));
        assert!(entry(&d).starts_with(r#"  math('"', "#));
        let d = draft(r"\\\", parse_body(r"\setminus{}"));
        assert!(entry(&d).starts_with(r#"  math([[\\\]], "#));
        let d = Draft { kind: Kind::RawLua, raw_lua: "  env(\"pmat\", \"pmatrix\", \"yours\")\n".into(), ..Draft::default() };
        assert_eq!(entry(&d), "  env(\"pmat\", \"pmatrix\", \"yours\"),");
    }

    #[test]
    fn body_tabstops() {
        assert_eq!(body(&[t("\\frac{"), tab(1), t("}{"), tab(2), t("}")]), r"\frac{<>}{<>}");
        assert_eq!(body(&[tabd(1, "0"), t("^"), tabd(2, "1"), t(" "), tab(3)]), "<:0>^<:1> <>");
        // a mirror → everything numbered
        assert_eq!(body(&[tabd(1, r"\psi"), t(" "), tab(1)]), r"<1:\psi> <1>");
        // out of order → numbered
        assert_eq!(body(&[tab(2), t("/"), tab(1)]), "<2>/<1>");
        assert_eq!(body(&[tab(1), t(" "), Piece::Exit, t(" "), tab(2)]), "<> <0> <>");
        assert_eq!(body(&[t(r"\hat{"), Piece::Capture(1), t("}")]), r"\hat{%1}");
        assert_eq!(body(&[t("("), Piece::Selection, t(")")]), "(%V)");
        assert_eq!(body(&[t("50% of "), tab(1)]), "50%% of <>");
        assert_eq!(body(&[t("100%")]), "100%%");
    }

    #[test]
    fn body_literal_angles() {
        assert_eq!(body(&[t("a <> b")]), "a %<%> b");
        assert_eq!(body(&[t("<3>")]), "%<3%>");
        assert_eq!(body(&[t("<:x>")]), "%<:x%>");
        // not tab-stop look-alikes: kept as is
        assert_eq!(body(&[t(r"\left< x \right>")]), r"\left< x \right>");
        assert_eq!(body(&[t("a<b>c")]), "a<b>c");
        assert_eq!(body(&[t("<<")]), "<<");
        // across pieces: `<:` + %V + `>` reads as a tab stop
        assert_eq!(body(&[t("<:"), Piece::Selection, t(">")]), "%<:%V%>");
        // `<` before a real tab stop
        assert_eq!(body(&[t("<"), tab(1)]), "<<>");
        for s in ["a <> b", "<3>", "<:x>", "<:5%>", "x<1:y>z", "<>"] {
            assert_eq!(parse_body(&body(&[t(s)])), vec![t(s)], "{s}");
        }
    }

    #[test]
    fn lua_strings() {
        assert_eq!(lua_string(r"\cup"), r"[[\cup]]");
        assert_eq!(lua_string("a]]b"), "[=[a]]b]=]");
        assert_eq!(lua_string("\\a]]b]=]"), "[==[\\a]]b]=]]==]");
        assert_eq!(lua_string(r"\left[ <> \right]"), r"[=[\left[ <> \right]]=]");
        assert_eq!(lua_string("[%V]"), "\"[%V]\"");
        assert_eq!(lua_string("\\b\n\tx"), "[[\n\\b\n\tx]]");
        assert_eq!(lua_string("\nx"), "[[\n\nx]]");
        assert_eq!(lua_string("x  "), "[[x  ]]");
        assert_eq!(lua_string("a\tb"), "[[a\tb]]");
        assert_eq!(lua_string("a\r\nb"), "\"a\\r\\nb\"");
        assert_eq!(lua_string(""), "[[]]");
    }

    #[test]
    fn parse_vscode() {
        assert_eq!(parse_body(r"\frac{$1}{$2}$0"), vec![t(r"\frac{"), tab(1), t("}{"), tab(2), t("}"), Piece::Exit]);
        assert_eq!(parse_body("${1:x} + ${2}"), vec![tabd(1, "x"), t(" + "), tab(2)]);
        assert_eq!(parse_body(r"\sqrt{${VISUAL}}"), vec![t(r"\sqrt{"), Piece::Selection, t("}")]);
        assert_eq!(parse_body("($TM_SELECTED_TEXT)"), vec![t("("), Piece::Selection, t(")")]);
        assert_eq!(parse_body("${1:a ${2:b}}"), vec![tabd(1, "a b")]);
        // helper syntax wins: `$` stays a dollar
        assert_eq!(parse_body("$<>$"), vec![t("$"), tab(1), t("$")]);
        // plain LaTeX stays text
        assert_eq!(parse_body(r"$x^2$ costs \$5"), vec![t(r"$x^2$ costs \$5")]);
    }

    #[test]
    fn parse_helper_numbering() {
        // `<>` take the free numbers around explicit ones, as helpers.parse
        assert_eq!(parse_body("<> <2:a> <>"), vec![tab(1), t(" "), tabd(2, "a"), t(" "), tab(3)]);
        assert_eq!(parse_body("<0>x%%%1%V%q"), vec![Piece::Exit, t("x%"), Piece::Capture(1), Piece::Selection, t("%q")]);
        assert_eq!(parse_body("%<>%>"), vec![t("<>>")]);
    }

    #[test]
    fn renumbering() {
        let mut p = vec![tab(5), t(" "), tabd(2, "a"), t(" "), tabd(5, "z"), Piece::Tab { n: 0, default: String::new() }];
        renumber(&mut p);
        assert_eq!(p, vec![tabd(1, "z"), t(" "), tabd(2, "a"), t(" "), tab(1), Piece::Exit]);
    }

    #[test]
    fn plain_and_preview() {
        let p = parse_body(r"\int_{<:0>}^{<:1>} <> \, d<:x> %V %1");
        assert_eq!(plain(&p), r"\int_{0}^{1} □ \, dx sel x");
        assert_eq!(preview_latex(&p), format!(r"\int_{{0}}^{{1}} {PREVIEW_TABSTOP} \, dx x+y x"));
        let p = parse_body(r"<1:f>(<2>) <1>");
        assert_eq!(plain(&p), "f(□) f");
    }

    // ---- the round-trip property: parse_body(body(p)) == p after renumber

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
        fn pick<'a>(&mut self, xs: &'a [&'a str]) -> &'a str {
            xs[self.below(xs.len() as u64) as usize]
        }
    }

    const BITS: &[&str] = &[
        "a", "x", " ", "\\frac{", "}", "{", "^", "_", "<", ">", "<>", "<3>", "<:y>", "<1:z>", "%", "%%", "%1", "%V", "\n", "\t",
        "\\left<", "]]", "]", "[", "$", "$x$", ":", "1", "\\\\", "é", "→", "%<", "%>",
    ];
    const DEFAULTS: &[&str] = &["", "", "x", "0", "\\infty", "a b", "50%", "f(x)", "%V", "1"];

    fn gen_text(r: &mut Rng) -> String {
        let n = 1 + r.below(5);
        (0..n).map(|_| r.pick(BITS)).collect()
    }

    fn gen_pieces(r: &mut Rng) -> Vec<Piece> {
        let n = r.below(9);
        let mut v = Vec::new();
        let mut stops = 0u32;
        let mut exit = false;
        for _ in 0..n {
            match r.below(8) {
                0 | 1 | 2 => v.push(Piece::Text(gen_text(r))),
                3 | 4 => {
                    stops += 1;
                    v.push(Piece::Tab { n: stops, default: r.pick(DEFAULTS).to_string() });
                }
                5 if stops > 0 => {
                    let k = 1 + r.below(stops as u64) as u32;
                    v.push(Piece::Tab { n: k, default: String::new() });
                }
                6 if !exit => {
                    exit = true;
                    v.push(Piece::Exit);
                }
                7 => v.push(if r.below(2) == 0 { Piece::Selection } else { Piece::Capture(1 + r.below(9) as u8) }),
                _ => v.push(Piece::Text("y".into())),
            }
        }
        normalise(v)
    }

    /// merge adjacent text, drop empty text, renumber.
    fn normalise(v: Vec<Piece>) -> Vec<Piece> {
        let mut out = Pieces(Vec::new());
        for p in v {
            match p {
                Piece::Text(t) => out.text(&t),
                p => out.push(p),
            }
        }
        let mut o = out.0;
        renumber(&mut o);
        o
    }

    #[test]
    fn round_trip_property() {
        let mut r = Rng(0x9E37_79B9_7F4A_7C15);
        let mut checked = 0;
        for _ in 0..20_000 {
            let p = gen_pieces(&mut r);
            let b = body(&p);
            // a text with `$1`-like runs and no helper tokens reads as VS Code
            // on paste; the helper reader is what Lua files get.
            let back = normalise(parse_helper_body(&b));
            assert_eq!(back, p, "body {b:?}");
            if !b.contains('$') || has_helper_tokens(&b) {
                assert_eq!(normalise(parse_body(&b)), p, "paste of {b:?}");
            }
            // and the Lua literal holds it exactly
            let lit = lua_string(&b);
            assert_eq!(crate::luaparse::lua_string_value(&lit).as_deref(), Some(b.as_str()), "{lit}");
            checked += 1;
        }
        assert_eq!(checked, 20_000);
    }
}
