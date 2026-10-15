//! Every snippet in snippets/tex/*.lua, for the Snippets page: read as rows
//! ("what you type" → "what you get"), grouped by each file's group title,
//! searched fuzzily, and edited in place.
//!
//! Editing never reformats her files. When only the trigger and/or the
//! output change, just those two string literals are swapped (keeping the
//! column the next field starts in, when the line is aligned). Any other
//! change rewrites the one call with `codegen::entry`, keeping its comments
//! and position, and makes sure the helper it now calls is bound in the file.
use crate::model::*;
use crate::{apply, codegen, luaparse};
use crate::luaparse::{K, Val};
use std::io;


/// One snippet as it sits in a topic file.
#[derive(Clone, Debug, PartialEq)]
pub struct LibEntry {
    /// File name in snippets/tex, e.g. "calculus.lua".
    pub file: String,
    /// The file's group title, e.g. "Calculus" (the category).
    pub category: String,
    /// Position in the file's list (0-based).
    pub index: usize,
    /// 1-based line of the call.
    pub line: usize,
    /// The call exactly as written (no comments, no trailing comma).
    pub call: String,
    /// The helper it calls ("math", "env", …; "" when not a helper call).
    pub helper: String,
    /// What she types, for display: the trigger, a pattern's `name`, or "?".
    pub trigger: String,
    /// What she gets, for display (tab stops as □).
    pub output: String,
    /// The output as pieces, when it is a literal body.
    pub pieces: Option<Vec<Piece>>,
    /// "by itself" | "Tab" | "on a selection" | "pattern" | "Lua"
    pub fires: &'static str,
    /// true: fires in text; false: in maths.
    pub text: bool,
    pub desc: String,
    pub origin: String,
    /// A pattern's example keys (`eg`), "" if none.
    pub eg: String,
    /// The draft the studio's editor can open; None: read-only here (a
    /// computed output, `env`, a `name` opt, a hand-written function …).
    pub draft: Option<Draft>,
}

impl LibEntry {
    /// Where to find it again when saving.
    pub fn loc(&self) -> Loc {
        Loc { file: self.file.clone(), index: self.index, call: self.call.clone() }
    }
    pub fn editable(&self) -> bool {
        self.draft.is_some()
    }
    /// A key unique within the library (file + position).
    pub fn key(&self) -> String {
        format!("{}#{}", self.file, self.index)
    }
}

/// An entry's address in its file: position, plus the call text to confirm
/// nobody changed it in the meantime.
#[derive(Clone, Debug, PartialEq)]
pub struct Loc {
    pub file: String,
    pub index: usize,
    pub call: String,
}

/// A snippet file that could not be read as a list.
#[derive(Clone, Debug, PartialEq)]
pub struct Unreadable {
    pub file: String,
    pub why: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Library {
    /// Category order: titles A–Z, the inbox last.
    pub entries: Vec<LibEntry>,
    pub unreadable: Vec<Unreadable>,
}

impl Library {
    /// The categories in display order, with how many entries each has.
    pub fn categories(&self) -> Vec<(String, usize)> {
        let mut out: Vec<(String, usize)> = Vec::new();
        for e in &self.entries {
            match out.last_mut() {
                Some((c, n)) if *c == e.category => *n += 1,
                _ => out.push((e.category.clone(), 1)),
            }
        }
        out
    }
}

/// Every `snippets/tex/*.lua` (not `zz_` files), in category order.
pub fn read(paths: &Paths) -> io::Result<Library> {
    let dir = paths.snippets.join("tex");
    let mut names: Vec<String> = Vec::new();
    for e in std::fs::read_dir(&dir)? {
        let e = e?;
        let n = e.file_name().to_string_lossy().into_owned();
        if n.ends_with(".lua") && !n.starts_with('.') && !n.starts_with("zz_") {
            names.push(n);
        }
    }
    let mut files: Vec<(String, String, Vec<LibEntry>)> = Vec::new();
    let mut lib = Library::default();
    for n in names {
        let text = match std::fs::read_to_string(dir.join(&n)) {
            Ok(t) => t,
            Err(e) => {
                lib.unreadable.push(Unreadable { file: n, why: e.to_string() });
                continue;
            }
        };
        match parse_file(&n, &text) {
            Some((title, entries)) => files.push((n, title, entries)),
            None => lib.unreadable.push(Unreadable { file: n, why: "not a `return h.group(…, { … })` list".into() }),
        }
    }
    files.sort_by(|a, b| {
        let inbox = |n: &str| n == "inbox.lua";
        inbox(&a.0).cmp(&inbox(&b.0)).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase())).then_with(|| a.0.cmp(&b.0))
    });
    for (_, _, es) in files {
        lib.entries.extend(es);
    }
    lib.unreadable.sort_by(|a, b| a.file.cmp(&b.file));
    Ok(lib)
}

/// The title of `return h.group("Title", {`: the string token before `, {`.
fn group_title(text: &str, open: usize) -> Option<String> {
    let t = luaparse::lex(text)?;
    let code: Vec<&luaparse::Tok> = t.iter().filter(|x| x.k != K::Comment).collect();
    let i = code.iter().position(|x| x.start == open)?;
    if i >= 2 && code[i - 1].k == K::Punct(",") {
        if let K::Str(s) = &code[i - 2].k {
            return Some(s.clone());
        }
    }
    None
}

/// "calculus.lua" → "Calculus" (a file without a group title).
fn title_from_name(name: &str) -> String {
    let stem = name.trim_end_matches(".lua").replace(['_', '-'], " ");
    let mut c = stem.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => name.to_string(),
    }
}

/// One file's entries (None: not a snippet list).
pub fn parse_file(name: &str, text: &str) -> Option<(String, Vec<LibEntry>)> {
    let list = luaparse::list(text)?;
    let title = group_title(text, list.open).unwrap_or_else(|| title_from_name(name));
    let category = if name == "inbox.lua" { "Inbox".to_string() } else { title.clone() };
    let mut out = Vec::new();
    for (index, sp) in list.spans.iter().enumerate() {
        let call = &text[sp.call_start..sp.call_end];
        let line = text[..sp.call_start].matches('\n').count() + 1;
        out.push(entry_of(name, &category, index, line, call));
    }
    Some((title, out))
}

fn str_opt(opts: &[(Option<String>, Val)], key: &str) -> Option<String> {
    opts.iter().find_map(|(k, v)| match (k.as_deref(), v) {
        (Some(k), Val::Str(s)) if k == key => Some(s.clone()),
        _ => None,
    })
}

fn one_line(s: &str) -> String {
    s.trim_end_matches(['\n', ' ', '\t']).replace("\r\n", "\n").replace('\n', " ⏎ ").replace('\t', " ")
}

fn entry_of(file: &str, category: &str, index: usize, line: usize, call: &str) -> LibEntry {
    let mut e = LibEntry {
        file: file.into(),
        category: category.into(),
        index,
        line,
        call: call.into(),
        helper: String::new(),
        trigger: luaparse::trigger_of(call).unwrap_or_else(|| "?".into()),
        output: call.lines().next().unwrap_or("").trim().to_string(),
        pieces: None,
        fires: "Lua",
        text: false,
        desc: String::new(),
        origin: String::new(),
        eg: String::new(),
        draft: luaparse::parse_entry(call),
    };
    let Some(c) = luaparse::parse_call(call) else { return e };
    e.helper = c.name.clone();
    let opts: Vec<(Option<String>, Val)> = match c.args.iter().find(|a| matches!(a, Val::Table(_))) {
        Some(Val::Table(t)) => t.clone(),
        _ => Vec::new(),
    };
    let tab = opts.iter().any(|(k, v)| k.as_deref() == Some("tab") && *v == Val::Bool(true));
    e.desc = str_opt(&opts, "desc").unwrap_or_default();
    e.eg = str_opt(&opts, "eg").unwrap_or_default();
    if let Some(Val::Str(o)) = c.args.get(2) {
        e.origin = o.clone();
    }
    if let Some(n) = str_opt(&opts, "name") {
        e.trigger = n;
    }
    e.text = c.name == "text";
    e.fires = match c.name.as_str() {
        "math_tab" => "Tab",
        "vis" => "on a selection",
        "pat" if tab => "Tab",
        "pat" => "pattern",
        _ if tab => "Tab",
        "math" | "text" | "suffix" | "post" | "env" => "by itself",
        _ => "Lua",
    };
    match (c.name.as_str(), c.args.get(1)) {
        ("env", Some(Val::Str(env))) => {
            e.output = format!("\\begin{{{env}}} □ \\end{{{env}}}");
            e.pieces = Some(vec![
                Piece::Text(format!("\\begin{{{env}}}")),
                Piece::Tab { n: 1, default: String::new() },
                Piece::Text(format!("\\end{{{env}}}")),
            ]);
        }
        (_, Some(Val::Str(body))) => {
            let pieces = codegen::parse_helper_body(body);
            e.output = one_line(&codegen::plain(&pieces));
            e.pieces = Some(pieces);
        }
        (_, Some(Val::Name(f))) => {
            e.output = if e.desc.is_empty() { format!("(computed by {f})") } else { format!("(computed) {}", e.desc) };
        }
        (_, Some(Val::Func { .. })) => {
            e.output = if e.desc.is_empty() { "(computed)".into() } else { format!("(computed) {}", e.desc) };
        }
        _ => {}
    }
    e
}

// ---------------------------------------------------------------------------
// search

/// fzf-style score of `needle` (lowercase) in `hay`: None unless every char
/// appears in order. Rewards consecutive runs, word starts and an early start.
fn fuzzy_score(needle: &[char], hay: &str) -> Option<i64> {
    if needle.is_empty() {
        return Some(0);
    }
    let h: Vec<char> = hay.chars().flat_map(|c| c.to_lowercase()).collect();
    // greedy from each possible start; keep the best
    let mut best: Option<i64> = None;
    for s in 0..h.len() {
        if h[s] != needle[0] {
            continue;
        }
        let mut score = 0i64;
        let mut j = s;
        let mut prev: Option<usize> = None;
        let mut ok = true;
        for &nc in needle {
            while j < h.len() && h[j] != nc {
                j += 1;
            }
            if j == h.len() {
                ok = false;
                break;
            }
            score += 10;
            if prev.is_some_and(|p| p + 1 == j) {
                score += 15;
            } else if let Some(p) = prev {
                score -= (j - p - 1).min(10) as i64;
            }
            let boundary = j == 0 || !h[j - 1].is_alphanumeric() || (h[j - 1].is_alphanumeric() != h[j].is_alphanumeric());
            if boundary {
                score += 8;
            }
            prev = Some(j);
            j += 1;
        }
        if !ok {
            break;
        }
        score -= (s as i64).min(20) / 2;
        best = Some(best.map_or(score, |b: i64| b.max(score)));
    }
    best
}

/// The entries matching `query` (each space-separated term must match), best
/// first; ties keep library order. The trigger counts most, then the output,
/// then the description, category and file. An exact trigger wins outright.
pub fn search(entries: &[LibEntry], query: &str) -> Vec<usize> {
    let terms: Vec<Vec<char>> = query.split_whitespace().map(|t| t.chars().flat_map(|c| c.to_lowercase()).collect()).collect();
    if terms.is_empty() {
        return (0..entries.len()).collect();
    }
    let mut scored: Vec<(i64, usize)> = Vec::new();
    'entries: for (i, e) in entries.iter().enumerate() {
        let mut total = 0i64;
        for t in &terms {
            let ts: String = t.iter().collect();
            let mut best: Option<i64> = None;
            let mut take = |s: Option<i64>, weight: i64| {
                if let Some(s) = s {
                    let v = s * weight / 4;
                    best = Some(best.map_or(v, |b: i64| b.max(v)));
                }
            };
            if e.trigger.to_lowercase() == ts {
                take(Some(1000), 4);
            }
            // a scattered match in a long text means nothing: the trigger may
            // be abbreviated (dnt → dint), the rest must be nearly contiguous
            let n = t.len() as i64;
            let tight = |s: Option<i64>, per_char: i64| s.filter(|v| *v >= per_char * n);
            take(tight(fuzzy_score(t, &e.trigger), 8), 4);
            take(tight(fuzzy_score(t, &e.output), 16), 3);
            take(tight(fuzzy_score(t, &e.desc), 16), 2);
            take(tight(fuzzy_score(t, &e.category), 14), 2);
            take(tight(fuzzy_score(t, &e.eg), 16), 2);
            take(tight(fuzzy_score(t, &e.file), 16), 1);
            match best {
                Some(b) => total += b,
                None => continue 'entries,
            }
        }
        scored.push((total, i));
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, i)| i).collect()
}

// ---------------------------------------------------------------------------
// editing

/// The span of `loc` in `text`: at its index if the call there is unchanged,
/// else the one entry with exactly that call. Err: it moved or changed.
fn find(text: &str, loc: &Loc) -> Result<(luaparse::List, usize), String> {
    let list = luaparse::list(text).ok_or_else(|| format!("{} is no longer a snippet list", loc.file))?;
    if let Some(sp) = list.spans.get(loc.index) {
        if text[sp.call_start..sp.call_end] == loc.call {
            return Ok((list, loc.index));
        }
    }
    let same: Vec<usize> = (0..list.spans.len()).filter(|&i| text[list.spans[i].call_start..list.spans[i].call_end] == loc.call).collect();
    match same.as_slice() {
        [i] => {
            let i = *i;
            Ok((list, i))
        }
        _ => Err(format!("that snippet changed in {} since the list was read; reload and try again", loc.file)),
    }
}

/// The string-literal tokens of a call's first two arguments: byte ranges
/// within the call. None unless the call is `name(` or `h.name(` followed by
/// two literals.
fn literal_ranges(call: &str) -> Option<((usize, usize), (usize, usize))> {
    let t: Vec<luaparse::Tok> = luaparse::lex(call)?.into_iter().filter(|x| x.k != K::Comment).collect();
    let mut i = 0;
    if matches!(t.get(i).map(|x| &x.k), Some(K::Name(n)) if n == "h") && t.get(i + 1).map(|x| &x.k) == Some(&K::Punct(".")) {
        i += 2;
    }
    if !matches!(t.get(i).map(|x| &x.k), Some(K::Name(_))) || t.get(i + 1).map(|x| &x.k) != Some(&K::Punct("(")) {
        return None;
    }
    let a = t.get(i + 2)?;
    let comma = t.get(i + 3)?;
    let b = t.get(i + 4)?;
    if !matches!(a.k, K::Str(_)) || comma.k != K::Punct(",") || !matches!(b.k, K::Str(_)) {
        return None;
    }
    Some(((a.start, a.end), (b.start, b.end)))
}

/// Replace `s[range]` with `new`; if the literal is followed by `,` and a run
/// of two or more spaces on the same line (an aligned column), give back or
/// take spaces so the next field stays where it was (at least one space).
fn swap_aligned(s: &str, (a, b): (usize, usize), new: &str) -> String {
    let old_len = s[a..b].chars().count() as i64;
    let new_len = new.chars().count() as i64;
    let rest = &s[b..];
    let mut out = String::with_capacity(s.len() + new.len());
    out.push_str(&s[..a]);
    out.push_str(new);
    // only a single-line literal sits in a column
    let one_line = !s[a..b].contains('\n') && !new.contains('\n');
    if one_line && rest.starts_with(',') {
        let spaces = rest[1..].chars().take_while(|c| *c == ' ').count();
        let after = &rest[1 + spaces..];
        let next_on_line = after.chars().next().is_some_and(|c| c != '\n' && c != '\r');
        if spaces >= 2 && next_on_line {
            let want = (spaces as i64 - (new_len - old_len)).max(1) as usize;
            out.push(',');
            out.push_str(&" ".repeat(want));
            out.push_str(after);
            return out;
        }
    }
    out.push_str(rest);
    out
}

/// Are `a` and `b` the same snippet apart from trigger and output?
fn same_but_text(a: &Draft, b: &Draft) -> bool {
    let norm = |d: &Draft| Draft { id: None, pinned: Vec::new(), trigger: String::new(), output: Vec::new(), ..d.clone() };
    norm(a) == norm(b)
}

/// The output pieces as the file should hold them (an exit after a closing
/// line break, as codegen::entry writes).
fn body_literal(pieces: &[Piece]) -> String {
    let mut p = pieces.to_vec();
    if codegen::needs_exit(&p) {
        p.push(Piece::Exit);
    }
    codegen::lua_string(&codegen::body(&p))
}

/// `text` with the entry at `loc` replaced by `draft`. Comments, alignment
/// and every other entry stay byte-for-byte.
pub fn replace(text: &str, loc: &Loc, draft: &Draft) -> Result<String, String> {
    let (list, i) = find(text, loc)?;
    let sp = &list.spans[i];
    let call = &text[sp.call_start..sp.call_end];
    let old = luaparse::parse_entry(call);
    // trigger and/or output only: swap the literals in place
    if let (Some(old), Some(((ta, tb), (ba, bb)))) = (&old, literal_ranges(call)) {
        if same_but_text(old, draft) {
            let mut c = call.to_string();
            // the output first (it is to the right, so the trigger's offsets hold)
            // (compared as the file would read them: the editor may renumber
            // tab stops without changing what they mean)
            let new_body = body_literal(&draft.output);
            let reads_as = luaparse::lua_string_value(&new_body).map(|v| codegen::parse_helper_body(&v));
            let old_reads = luaparse::lua_string_value(&call[ba..bb]).map(|v| codegen::parse_helper_body(&v));
            if reads_as != old_reads {
                c = swap_aligned(&c, (ba, bb), &new_body);
            }
            if old.trigger != draft.trigger {
                c = swap_aligned(&c, (ta, tb), &codegen::trigger_literal(&draft.trigger));
            }
            // a qualified call stays qualified (literal_ranges skips `h.`)
            let mut out = text.to_string();
            out.replace_range(sp.call_start..sp.call_end, &c);
            return Ok(out);
        }
    }
    // hand-written Lua: in place of the call, as typed
    if draft.kind == Kind::RawLua {
        let lua = draft.raw_lua.trim().trim_end_matches(',').trim_end();
        if lua.is_empty() {
            return Err("the Lua is empty".into());
        }
        let (t2, lua) = apply::fix_helpers(text, lua);
        let (list2, i2) = find(&t2, loc)?;
        let sp2 = &list2.spans[i2];
        let mut out = t2.clone();
        out.replace_range(sp2.call_start..sp2.call_end, &lua);
        return Ok(out);
    }
    // anything else: the call regenerated, the helper bound in this file
    let mut d = draft.clone();
    if let Some(old) = &old {
        if d.origin.is_empty() {
            d.origin = old.origin.clone();
        }
    }
    let generated = codegen::entry(&d);
    let mut new_call = generated.trim().trim_end_matches(',').to_string();
    if call.starts_with("h.") && !new_call.starts_with("h.") {
        new_call.insert_str(0, "h.");
    }
    let (t2, new_call) = apply::fix_helpers(text, &new_call);
    let (list2, i2) = find(&t2, loc)?;
    let sp2 = &list2.spans[i2];
    let mut out = t2.clone();
    out.replace_range(sp2.call_start..sp2.call_end, &new_call);
    Ok(out)
}

/// `text` without the entry at `loc` (its comments and line too): what the
/// hidden ntex loads while trying the edited version, so the old one doesn't
/// shadow it.
pub fn without(text: &str, loc: &Loc) -> Result<String, String> {
    let (list, i) = find(text, loc)?;
    let sp = &list.spans[i];
    // whole lines when the entry is alone on them
    let ls = text[..sp.start].rfind('\n').map(|k| k + 1).unwrap_or(0);
    let le = text[sp.end..].find('\n').map(|k| sp.end + k + 1).unwrap_or(text.len());
    let alone = text[ls..sp.start].trim().is_empty() && text[sp.end..le].trim().is_empty();
    let (a, b) = if alone { (ls, le) } else { (sp.start, sp.end) };
    let mut out = text.to_string();
    out.replace_range(a..b, "");
    Ok(out)
}

/// Read, replace, write atomically. Returns the new text.
pub fn save(paths: &Paths, loc: &Loc, draft: &Draft) -> io::Result<String> {
    let path = paths.snippets.join("tex").join(&loc.file);
    let text = std::fs::read_to_string(&path)?;
    let new = replace(&text, loc, draft).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    crate::inbox::write_atomic(&path, &new)?;
    Ok(new)
}

/// The path of a library file.
pub fn path_of(paths: &Paths, file: &str) -> std::path::PathBuf {
    paths.snippets.join("tex").join(file)
}


#[cfg(test)]
mod tests {
    use super::*;

    const CALC: &str = r#"-- Derivatives
local h = require("snips.helpers")
local math, math_tab, pat = h.math, h.math_tab, h.pat

return h.group("Calculus", {
  -- trigger        output                                                origin
  math_tab("par",   [[\frac{ \partial <:y> }{ \partial <:x> } ]],         "default"),
  pat("pa(%a)(%a)", [[\frac{ \partial %1 }{ \partial %2 } ]],             "default", { tab = true, name = "pa‹y›‹x›", eg = "payx" }),
  math("ddt",       [[\frac{d}{dt} ]],                                    "default"),

  math("dint",      [[\int_{<:0>}^{<:1>} <> \, d<:x> ]],                  "changed", { desc = "Definite integral" }),
  text("dm",        block,                                                "changed", { line = "start" }),
})
"#;

    fn entries() -> Vec<LibEntry> {
        parse_file("calculus.lua", CALC).unwrap().1
    }

    #[test]
    fn reads_rows() {
        let (title, es) = parse_file("calculus.lua", CALC).unwrap();
        assert_eq!(title, "Calculus");
        assert_eq!(es.len(), 5);
        assert_eq!(es[0].trigger, "par");
        assert_eq!(es[0].fires, "Tab");
        assert_eq!(es[1].trigger, "pa‹y›‹x›");
        assert!(es[1].draft.is_none(), "name opt: read-only");
        assert_eq!(es[2].output, r"\frac{d}{dt}");
        assert_eq!(es[3].desc, "Definite integral");
        assert!(es[4].output.starts_with("(computed"));
        assert!(es[4].text);
        assert_eq!(es[2].line, 9);
    }

    #[test]
    fn fuzzy_finds() {
        let es = entries();
        let r = search(&es, "ddt");
        assert_eq!(es[r[0]].trigger, "ddt");
        let r = search(&es, "integral");
        assert_eq!(es[r[0]].trigger, "dint");
        let r = search(&es, "partial");
        assert!(r.len() >= 2);
        assert!(search(&es, "zzzq").is_empty());
        // terms are ANDed
        let r = search(&es, "frac dt");
        assert_eq!(es[r[0]].trigger, "ddt");
        // abbreviations of a trigger work; letters scattered through an output don't
        let r = search(&es, "dnt");
        assert_eq!(es[r[0]].trigger, "dint");
        assert!(search(&es, "fdt").iter().all(|&i| es[i].trigger != "dint"));
    }

    #[test]
    fn edits_only_the_literals_and_keeps_columns() {
        let es = entries();
        let e = &es[2];
        let mut d = e.draft.clone().unwrap();
        d.trigger = "dtt".into();
        d.output = codegen::parse_helper_body(r"\frac{\mathrm{d}}{\mathrm{d}t} ");
        let out = replace(CALC, &e.loc(), &d).unwrap();
        let line = out.lines().nth(8).unwrap();
        assert!(line.starts_with(r#"  math("dtt",       [[\frac{\mathrm{d}}{\mathrm{d}t} ]],"#), "{line}");
        // origin column unchanged
        let col = |s: &str| s.find("\"default\"").unwrap();
        assert_eq!(col(line), col(CALC.lines().nth(8).unwrap()));
        // everything else byte-for-byte
        let a: Vec<&str> = CALC.lines().collect();
        let b: Vec<&str> = out.lines().collect();
        assert_eq!(a.len(), b.len());
        for (k, (x, y)) in a.iter().zip(&b).enumerate() {
            if k != 8 {
                assert_eq!(x, y);
            }
        }
        // and it reads back as the edit
        let again = parse_file("calculus.lua", &out).unwrap().1;
        let d2 = again[2].draft.clone().unwrap();
        assert_eq!(d2.trigger, "dtt");
        assert_eq!(d2.output, d.output);
    }

    #[test]
    fn long_output_takes_one_space() {
        let es = entries();
        let e = &es[2];
        let mut d = e.draft.clone().unwrap();
        d.output = codegen::parse_helper_body(&"x".repeat(80));
        let out = replace(CALC, &e.loc(), &d).unwrap();
        assert!(out.contains(&format!("[[{}]], \"default\")", "x".repeat(80))));
    }

    #[test]
    fn other_changes_regenerate_the_call_and_bind_the_helper() {
        let es = entries();
        let e = &es[2];
        let mut d = e.draft.clone().unwrap();
        d.context = Context::Text;
        d.fires = Fires::Tab;
        let out = replace(CALC, &e.loc(), &d).unwrap();
        assert!(out.contains(r#"  text("ddt", [[\frac{d}{dt} ]], "default", { tab = true }),"#), "{out}");
        assert!(out.contains("local math, math_tab, pat, text = h.math, h.math_tab, h.pat, h.text"), "{out}");
        assert!(parse_file("calculus.lua", &out).is_some());
    }

    #[test]
    fn refuses_a_changed_entry() {
        let es = entries();
        let e = &es[2];
        let changed = CALC.replace(r#"math("ddt""#, r#"math("ddx""#);
        let d = e.draft.clone().unwrap();
        assert!(replace(&changed, &e.loc(), &d).is_err());
        // moved but unchanged: found by its text
        let moved = CALC.replace("  math(\"ddt\"", "  math(\"zz\", [[z]], \"yours\"),\n  math(\"ddt\"");
        let out = replace(&moved, &e.loc(), &Draft { trigger: "ddq".into(), ..d }).unwrap();
        assert!(out.contains("math(\"ddq\""));
        assert!(out.contains("math(\"zz\""));
    }

    #[test]
    fn raw_lua_replaces_the_call() {
        let es = entries();
        let e = &es[1];
        let d = Draft { kind: Kind::RawLua, raw_lua: e.call.replace("payx", "pazx") + ",", ..Draft::default() };
        let out = replace(CALC, &e.loc(), &d).unwrap();
        assert_eq!(out, CALC.replace("payx", "pazx"));
    }

    #[test]
    fn without_drops_the_line() {
        let es = entries();
        let out = without(CALC, &es[2].loc()).unwrap();
        assert!(!out.contains("ddt"));
        assert_eq!(out.lines().count(), CALC.lines().count() - 1);
    }

    #[test]
    fn real_files_round_trip_unchanged() {
        // every editable entry of her real files, saved with no change, is
        // byte-for-byte the same file
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../snippets/tex");
        let Ok(rd) = std::fs::read_dir(&dir) else { return };
        let mut n = 0;
        for f in rd.flatten() {
            let name = f.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".lua") {
                continue;
            }
            let text = std::fs::read_to_string(f.path()).unwrap();
            let Some((_, es)) = parse_file(&name, &text) else { continue };
            for e in es.iter().filter(|e| e.editable()) {
                let out = replace(&text, &e.loc(), e.draft.as_ref().unwrap()).unwrap();
                assert_eq!(out, text, "{name}: {}", e.trigger);
                n += 1;
            }
        }
        assert!(n > 100, "only {n} editable entries");
    }
}
