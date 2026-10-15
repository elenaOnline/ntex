//! New theorem-like blocks: templates/tex/ntex-blocks.sty (always exists;
//! ntex-notes.sty does `\RequirePackage{ntex-blocks}`), one stanza per block
//! between `% ── block: <env> ──` and `% ── end: <env> ──` lines.
//!
//! A stanza (the form in ARCHITECTURE.md, plus one `% studio:` line that
//! remembers what LaTeX can't: the "Like" choice, the hue and the snippet):
//!
//! ```latex
//! % ── block: funfact ── made in Ntex Studio 2026-10-07 ──
//! % studio: like=remark colour=hue:145 title=yes trigger=fun
//! \definecolor{ntexFunfactRule}{HTML}{…}
//! \definecolor{ntexFunfactBack}{HTML}{…}
//! \definecolor{ntexFunfactHead}{HTML}{…}
//! \theoremstyle{ntexupright}
//! \newtheorem*{funfact}{Fun fact}
//! \tcolorboxenvironment{funfact}{ntexbox=Funfact}
//! % ── end: funfact ──
//! ```
//!
//! Palette colours reuse the theme's tones (`ntexbox=Blue`). The Proof grey
//! has no `ntexProofHead`, and the proof box is a different shape (1pt rule,
//! tighter padding), so a grey block uses `ntexquiet`: the proof box's look as
//! a tcolorbox style, defined by the stanza itself (a stanza stays
//! self-contained: removing one never breaks another, and the header of a
//! hand-made or older ntex-blocks.sty doesn't matter).
use crate::colour;
use crate::model::*;
use std::io;

/// `% ── block: ` (then the env, then ` ──`).
pub const BEGIN: &str = "% ── block: ";
/// `% ── end: ` (then the env, then ` ──`).
pub const END: &str = "% ── end: ";
/// The metadata line inside a stanza.
pub const META: &str = "% studio:";
/// The line ntex-notes.sty needs (before its `\endinput`).
pub const REQUIRE: &str = "\\RequirePackage{ntex-blocks}";

/// The file as first written (a header saying what it is, `\ProvidesPackage`,
/// no blocks, `\endinput`).
pub fn empty_file() -> String {
    "% ntex-blocks.sty — theorem-like blocks made in Ntex Studio (⌘2, New block).\n\
     % ntex-notes.sty loads it (\\RequirePackage{ntex-blocks}, just before its\n\
     % \\endinput), so each block here works in every .tex that uses the theme.\n\
     % One stanza per block, between its \"% ── block: <env> ──\" and\n\
     % \"% ── end: <env> ──\" lines. The studio rewrites only those stanzas: safe to\n\
     % edit by hand. Delete a stanza to delete a block (documents that use it\n\
     % stop compiling). To share a .tex: copy this file next to ntex-notes.sty.\n\
     \\NeedsTeXFormat{LaTeX2e}\n\
     \\ProvidesPackage{ntex-blocks}[2026/10/07 v1 blocks made in Ntex Studio]\n\
     \n\
     \\endinput\n"
        .to_string()
}

/// Today's local date, `2026-10-07`.
pub fn today() -> String {
    #[repr(C)]
    struct Tm {
        sec: i32,
        min: i32,
        hour: i32,
        mday: i32,
        mon: i32,
        year: i32,
        wday: i32,
        yday: i32,
        isdst: i32,
        gmtoff: std::ffi::c_long,
        zone: *mut std::ffi::c_char,
    }
    unsafe extern "C" {
        fn localtime_r(t: *const i64, out: *mut Tm) -> *mut Tm;
    }
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let mut tm = Tm { sec: 0, min: 0, hour: 0, mday: 1, mon: 0, year: 70, wday: 0, yday: 0, isdst: 0, gmtoff: 0, zone: std::ptr::null_mut() };
    // SAFETY: localtime_r writes only into `tm`.
    unsafe { localtime_r(&secs, &mut tm) };
    format!("{:04}-{:02}-{:02}", tm.year + 1900, tm.mon + 1, tm.mday)
}

/// `funfact` → `Funfact` (the colour names' middle part).
pub fn capitalised(env: &str) -> String {
    let mut c = env.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// `Fun fact` → `fun fact`.
fn decapitalised(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Text for a LaTeX argument: specials escaped, line breaks become spaces.
pub fn tex_escape(s: &str) -> String {
    let mut o = String::new();
    for ch in s.chars() {
        match ch {
            '\\' => o.push_str("\\textbackslash{}"),
            '{' => o.push_str("\\{"),
            '}' => o.push_str("\\}"),
            '%' | '#' | '&' | '$' | '_' => {
                o.push('\\');
                o.push(ch);
            }
            '~' => o.push_str("\\textasciitilde{}"),
            '^' => o.push_str("\\textasciicircum{}"),
            '\n' | '\r' | '\t' => o.push(' '),
            c => o.push(c),
        }
    }
    o
}

/// The inverse of `tex_escape`.
pub fn tex_unescape(s: &str) -> String {
    let mut o = s.to_string();
    for (a, b) in [("\\textbackslash{}", "\\"), ("\\textasciitilde{}", "~"), ("\\textasciicircum{}", "^")] {
        o = o.replace(a, b);
    }
    let mut out = String::new();
    let mut it = o.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\\' && it.peek().is_some_and(|n| "{}%#&$_".contains(*n)) {
            out.push(it.next().unwrap());
        } else {
            out.push(c);
        }
    }
    out
}

fn base_name(b: BlockBase) -> &'static str {
    match b {
        BlockBase::Theorem => "theorem",
        BlockBase::Lemma => "lemma",
        BlockBase::Definition => "definition",
        BlockBase::Example => "example",
        BlockBase::Remark => "remark",
    }
}

fn base_from(s: &str) -> Option<BlockBase> {
    Some(match s {
        "theorem" => BlockBase::Theorem,
        "lemma" => BlockBase::Lemma,
        "definition" => BlockBase::Definition,
        "example" => BlockBase::Example,
        "remark" => BlockBase::Remark,
        _ => return None,
    })
}

/// `\crefname` singular/plural, defaulting to the title in lower case + s.
fn cref_names(block: &BlockDraft) -> (String, String) {
    let sing = if block.cref.0.trim().is_empty() { block.title.trim().to_lowercase() } else { block.cref.0.trim().to_string() };
    let plural = if block.cref.1.trim().is_empty() {
        if block.cref.0.trim().is_empty() { format!("{sing}s") } else { format!("{}s", block.cref.0.trim()) }
    } else {
        block.cref.1.trim().to_string()
    };
    (sing, plural)
}

/// The proof box's look as a style (ntex-notes.sty's proof tcolorbox, with
/// the head in ink and block spacing).
const QUIET: &str = "\\tcbset{ntexquiet/.style={enhanced, breakable, frame hidden, sharp corners,\n  colback=ntexProofBack, borderline west={1pt}{0pt}{ntexProofRule},\n  left=10pt, right=10pt, top=6pt, bottom=6pt, before skip=12pt, after skip=12pt,\n  before upper={\\colorlet{ntexHead}{ntexInk}}}}\n";

/// The LaTeX stanza for one block: colours (unless a palette name),
/// `\theoremstyle`, `\newtheorem`/`\newtheorem*`, `\tcolorboxenvironment`,
/// `\crefname` (numbered only). Dated today; see `stanza_made`.
pub fn stanza(block: &BlockDraft) -> String {
    stanza_made(block, &today())
}

/// The stanza with its marker dated `date` (`2026-10-07`).
pub fn stanza_made(block: &BlockDraft, date: &str) -> String {
    let env = block.env.trim();
    let mut s = String::new();
    s.push_str(&format!("{BEGIN}{env} ── made in Ntex Studio {date} ──\n"));
    let colour_meta = match &block.colour {
        BlockColour::Palette(n) => colour::palette_name(n).unwrap_or(n.as_str()).to_string(),
        BlockColour::Hue(h) => format!("hue:{}", fmt_hue(*h)),
    };
    let trig = block.trigger.trim();
    s.push_str(&format!(
        "{META} like={} colour={colour_meta} title={}{}\n",
        base_name(block.base),
        if block.with_title { "yes" } else { "no" },
        if trig.is_empty() { String::new() } else { format!(" trigger={trig}") }
    ));
    let style = match &block.colour {
        BlockColour::Hue(h) => {
            let t = colour::tones_for_hue(*h);
            let name = capitalised(env);
            for (tone, c) in [("Rule", t.rule), ("Back", t.back), ("Head", t.head)] {
                s.push_str(&format!("\\definecolor{{ntex{name}{tone}}}{{HTML}}{{{}}}\n", colour::hex(c)));
            }
            format!("ntexbox={name}")
        }
        BlockColour::Palette(n) => match colour::palette_name(n) {
            Some("Proof") => "ntexquiet".to_string(),
            Some(p) => format!("ntexbox={p}"),
            // not a palette name: check() reports it; the box still compiles
            None => "ntexquiet".to_string(),
        },
    };
    s.push_str(match block.body {
        BodyFont::Italic => "\\theoremstyle{ntexplain}\n",
        BodyFont::Upright => "\\theoremstyle{ntexupright}\n",
    });
    let title = tex_escape(block.title.trim());
    s.push_str(&match block.numbering {
        Numbering::None => format!("\\newtheorem*{{{env}}}{{{title}}}\n"),
        Numbering::WithTheorems => format!("\\newtheorem{{{env}}}[theorem]{{{title}}}\n"),
        Numbering::Own => format!("\\newtheorem{{{env}}}{{{title}}}[section]\n"),
    });
    if style == "ntexquiet" {
        s.push_str(QUIET);
    }
    s.push_str(&format!("\\tcolorboxenvironment{{{env}}}{{{style}}}\n"));
    if block.numbering != Numbering::None {
        // ntex-notes.sty loads cleveref with `capitalise`, which uses a
        // \crefname as given: capitalised here too, so \cref reads "Claim 1.2"
        // like "Theorem 1.1".
        let (sing, plural) = cref_names(block);
        let (sing, plural) = (tex_escape(&capitalised(&sing)), tex_escape(&capitalised(&plural)));
        s.push_str(&format!("\\crefname{{{env}}}{{{sing}}}{{{plural}}}\n"));
        s.push_str(&format!("\\Crefname{{{env}}}{{{sing}}}{{{plural}}}\n"));
    }
    s.push_str(&format!("{END}{env} ──\n"));
    s
}

fn fmt_hue(h: f32) -> String {
    let r = (h * 10.0).round() / 10.0;
    if r.fract() == 0.0 { format!("{}", r as i64) } else { format!("{r}") }
}

/// The snippet that types the block (text, Tab, line start, .tex only), or
/// None when `block.trigger` is empty.
pub fn snippet(block: &BlockDraft) -> Option<Draft> {
    let trig = block.trigger.trim();
    if trig.is_empty() {
        return None;
    }
    let env = block.env.trim();
    let mut output = Vec::new();
    if block.with_title {
        output.push(Piece::Text(format!("\\begin{{{env}}}[")));
        output.push(Piece::Tab { n: 1, default: String::new() });
        output.push(Piece::Text("]\n\t".into()));
        output.push(Piece::Tab { n: 2, default: String::new() });
    } else {
        output.push(Piece::Text(format!("\\begin{{{env}}}\n\t")));
        output.push(Piece::Tab { n: 1, default: String::new() });
    }
    output.push(Piece::Text(format!("\n\\end{{{env}}}")));
    Some(Draft {
        trigger: trig.to_string(),
        kind: Kind::Plain,
        context: Context::Text,
        fires: Fires::Tab,
        line: LineRule::Start,
        ft: FileType::Tex,
        desc: format!("{} block", block.title.trim()),
        origin: "yours".into(),
        output,
        pinned: ["kind", "context", "fires", "math_where", "line", "ft"].iter().map(|s| s.to_string()).collect(),
        ..Draft::default()
    })
}

// ---------------------------------------------------------------------------
// the file

/// One stanza in a file: env, the byte range of its lines, its date.
struct Found {
    env: String,
    start: usize,
    end: usize,
    date: String,
}

fn line_spans(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut at = 0;
    for l in text.split_inclusive('\n') {
        out.push((at, at + l.len()));
        at += l.len();
    }
    out
}

/// `% ── block: funfact ── made in Ntex Studio 2026-10-07 ──` → (env, date).
fn parse_begin(line: &str) -> Option<(String, String)> {
    let rest = line.trim().strip_prefix(BEGIN.trim_end())?.trim_start();
    let env: String = rest.chars().take_while(|c| !c.is_whitespace()).collect();
    if env.is_empty() || env.starts_with('─') {
        return None;
    }
    let date = rest.split("made in Ntex Studio").nth(1).map(|d| d.trim().trim_end_matches('─').trim().to_string()).unwrap_or_default();
    Some((env, date))
}

fn parse_end(line: &str) -> Option<String> {
    let rest = line.trim().strip_prefix(END.trim_end())?.trim_start();
    let env: String = rest.chars().take_while(|c| !c.is_whitespace()).collect();
    if env.is_empty() { None } else { Some(env) }
}

fn stanzas(text: &str) -> Vec<Found> {
    let lines = line_spans(text);
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let (a, b) = lines[i];
        if let Some((env, date)) = parse_begin(&text[a..b]) {
            // to its end line (or up to the next block / \endinput / the end)
            let mut j = i + 1;
            let mut end = None;
            while j < lines.len() {
                let l = &text[lines[j].0..lines[j].1];
                if parse_end(l).is_some_and(|e| e == env) {
                    end = Some(lines[j].1);
                    break;
                }
                if parse_begin(l).is_some() || l.trim() == "\\endinput" {
                    end = Some(lines[j].0);
                    j -= 1;
                    break;
                }
                j += 1;
            }
            let end = end.unwrap_or(text.len());
            out.push(Found { env, start: a, end, date });
            i = j + 1;
            continue;
        }
        i += 1;
    }
    out
}

/// `{…}` at the start of `s` (balanced, `\{` skipped) → (content, rest).
fn braced(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    let b = s.as_bytes();
    if b.first() != Some(&b'{') {
        return None;
    }
    let mut depth = 0;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 1,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&s[1..i], &s[i + 1..]));
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn bracketed(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    let rest = s.strip_prefix('[')?;
    let close = rest.find(']')?;
    Some((&rest[..close], &rest[close + 1..]))
}

/// A stanza's text → the draft that would write it (best effort: a stanza
/// edited past recognition still gives its env and title).
pub fn parse_stanza(src: &str) -> Option<BlockDraft> {
    let mut env = None;
    let mut title = None;
    let mut numbering = Numbering::None;
    let mut body = BodyFont::Upright;
    let mut style = None;
    let mut cref = (String::new(), String::new());
    let mut colours: Vec<(String, Rgb)> = Vec::new();
    let mut meta_base = None;
    let mut meta_colour: Option<String> = None;
    let mut meta_title = None;
    let mut meta_trigger = String::new();
    for line in src.lines() {
        let l = line.trim();
        if let Some((e, _)) = parse_begin(l) {
            env = Some(e);
        } else if let Some(m) = l.strip_prefix(META) {
            let (m, trig) = match m.split_once(" trigger=") {
                Some((a, t)) => (a, t.to_string()),
                None => (m, String::new()),
            };
            meta_trigger = trig;
            for part in m.split_whitespace() {
                if let Some(v) = part.strip_prefix("like=") {
                    meta_base = base_from(v);
                } else if let Some(v) = part.strip_prefix("colour=") {
                    meta_colour = Some(v.to_string());
                } else if let Some(v) = part.strip_prefix("title=") {
                    meta_title = Some(v == "yes");
                }
            }
        } else if let Some(r) = l.strip_prefix("\\definecolor") {
            if let Some((name, r)) = braced(r) {
                if let Some(("HTML", r)) = braced(r) {
                    if let Some((hx, _)) = braced(r) {
                        if let Some(c) = colour::parse_hex(hx) {
                            colours.push((name.to_string(), c));
                        }
                    }
                }
            }
        } else if let Some(r) = l.strip_prefix("\\theoremstyle") {
            if let Some((s, _)) = braced(r) {
                body = if s == "ntexplain" { BodyFont::Italic } else { BodyFont::Upright };
            }
        } else if let Some(r) = l.strip_prefix("\\newtheorem*") {
            if let Some((e, r)) = braced(r) {
                env.get_or_insert(e.to_string());
                title = braced(r).map(|(t, _)| tex_unescape(t));
                numbering = Numbering::None;
            }
        } else if let Some(r) = l.strip_prefix("\\newtheorem") {
            if let Some((e, r)) = braced(r) {
                env.get_or_insert(e.to_string());
                if let Some((_, r)) = bracketed(r) {
                    title = braced(r).map(|(t, _)| tex_unescape(t));
                    numbering = Numbering::WithTheorems;
                } else if let Some((t, _)) = braced(r) {
                    title = Some(tex_unescape(t));
                    numbering = Numbering::Own;
                }
            }
        } else if let Some(r) = l.strip_prefix("\\tcolorboxenvironment") {
            if let Some((_, r)) = braced(r) {
                style = braced(r).map(|(s, _)| s.trim().to_string());
            }
        } else if let Some(r) = l.strip_prefix("\\crefname") {
            if let Some((_, r)) = braced(r) {
                if let Some((s1, r)) = braced(r) {
                    if let Some((s2, _)) = braced(r) {
                        // written capitalised (see stanza_made); read back as typed
                        cref = (decapitalised(&tex_unescape(s1)), decapitalised(&tex_unescape(s2)));
                    }
                }
            }
        }
    }
    let env = env?;
    let name = capitalised(&env);
    let own_rule = colours.iter().find(|(n, _)| *n == format!("ntex{name}Rule")).map(|(_, c)| *c);
    let colour = match meta_colour.as_deref() {
        Some(m) if m.starts_with("hue:") => BlockColour::Hue(m[4..].parse().unwrap_or_else(|_| own_rule.map(colour::hue_of).unwrap_or(0.0))),
        Some(m) => BlockColour::Palette(m.to_string()),
        None => match style.as_deref() {
            Some("ntexquiet") => BlockColour::Palette("Proof".into()),
            Some(s) if s.starts_with("ntexbox=") => {
                let n = &s[8..];
                match colour::palette_name(n) {
                    Some(p) => BlockColour::Palette(p.into()),
                    None => BlockColour::Hue(own_rule.map(|c| colour::hue_of(c).round()).unwrap_or(0.0)),
                }
            }
            _ => BlockColour::Palette("Proof".into()),
        },
    };
    let base = meta_base.unwrap_or(match &colour {
        BlockColour::Palette(p) if p == "Blue" => BlockBase::Theorem,
        BlockColour::Palette(p) if p == "Violet" => BlockBase::Lemma,
        BlockColour::Palette(p) if p == "Teal" => BlockBase::Definition,
        BlockColour::Palette(p) if p == "Maroon" => BlockBase::Example,
        BlockColour::Palette(p) if p == "Green" => BlockBase::Remark,
        _ => match (numbering, body) {
            (Numbering::None, _) => BlockBase::Remark,
            (_, BodyFont::Italic) => BlockBase::Theorem,
            (_, BodyFont::Upright) => BlockBase::Definition,
        },
    });
    // a cref that is the default (title in lower case + s) reads back as ""
    let title = title.unwrap_or_else(|| env.clone());
    if cref.0 == title.trim().to_lowercase() && cref.1 == format!("{}s", cref.0) {
        cref = (String::new(), String::new());
    }
    Some(BlockDraft { title, env, base, numbering, body, colour, cref, trigger: meta_trigger.trim().to_string(), with_title: meta_title.unwrap_or(true) })
}

/// The blocks in a file's text.
pub fn parse_file(text: &str) -> Vec<KnownBlock> {
    stanzas(text)
        .into_iter()
        .filter_map(|f| {
            let source = text[f.start..f.end].to_string();
            parse_stanza(&source).map(|draft| KnownBlock { draft, source })
        })
        .collect()
}

pub fn read(paths: &Paths) -> io::Result<Vec<KnownBlock>> {
    match std::fs::read_to_string(paths.blocks_sty()) {
        Ok(t) => Ok(parse_file(&t)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

/// The file text with `block` added (before `\endinput`) or replaced in
/// place (same env; it keeps its original date unless `date` is given).
pub fn with_block(current: &str, block: &BlockDraft, date: Option<&str>) -> String {
    let env = block.env.trim();
    let found = stanzas(current);
    if let Some(f) = found.iter().find(|f| f.env == env) {
        let d = date.map(|d| d.to_string()).unwrap_or_else(|| if f.date.is_empty() { today() } else { f.date.clone() });
        let mut out = current.to_string();
        out.replace_range(f.start..f.end, &stanza_made(block, &d));
        return out;
    }
    let st = stanza_made(block, &date.map(|d| d.to_string()).unwrap_or_else(today));
    let mut out = current.to_string();
    // before the last `\endinput` line
    let lines = line_spans(current);
    let endinput = lines.iter().rev().find(|(a, b)| current[*a..*b].trim() == "\\endinput").map(|(a, _)| *a);
    match endinput {
        Some(at) => {
            // one blank line before the stanza
            let before = &current[..at];
            let pad = if before.ends_with("\n\n") || before.is_empty() {
                ""
            } else if before.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            out.insert_str(at, &format!("{pad}{st}\n"));
        }
        None => {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            if !out.is_empty() && !out.ends_with("\n\n") {
                out.push('\n');
            }
            out.push_str(&st);
        }
    }
    out
}

/// The file text without the block `env` (and the blank line after it).
pub fn without_block(current: &str, env: &str) -> String {
    let mut out = current.to_string();
    if let Some(f) = stanzas(current).into_iter().find(|f| f.env == env) {
        let mut end = f.end;
        if current[end..].starts_with('\n') {
            end += 1;
        }
        out.replace_range(f.start..end, "");
    }
    out
}

/// Does this ntex-notes.sty load ntex-blocks (outside a comment)?
pub fn requires_blocks(notes: &str) -> bool {
    notes.lines().any(|l| {
        let code = l.split('%').next().unwrap_or("");
        code.contains("{ntex-blocks}") && (code.contains("\\RequirePackage") || code.contains("\\usepackage"))
    })
}

/// ntex-notes.sty with `\RequirePackage{ntex-blocks}` before its `\endinput`
/// (or at the end), unless it is already there.
pub fn with_require(notes: &str) -> String {
    if requires_blocks(notes) {
        return notes.to_string();
    }
    let line = format!("{REQUIRE}   % blocks made in Ntex Studio (ntex-blocks.sty)\n");
    let lines = line_spans(notes);
    match lines.iter().rev().find(|(a, b)| notes[*a..*b].trim() == "\\endinput") {
        Some((at, _)) => {
            let mut out = notes.to_string();
            out.insert_str(*at, &format!("{line}\n"));
            out
        }
        None => {
            let mut out = notes.to_string();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&line);
            out
        }
    }
}

/// Add or replace (same env) a block. Atomic. Creates the file, and adds
/// `\RequirePackage{ntex-blocks}` to ntex-notes.sty (before `\endinput`) if
/// it is missing.
pub fn save(paths: &Paths, block: &BlockDraft) -> io::Result<()> {
    let path = paths.blocks_sty();
    let current = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => empty_file(),
        Err(e) => return Err(e),
    };
    let text = with_block(&current, block, None);
    crate::inbox::write_atomic(&path, &text)?;
    ensure_required(paths)
}

/// Make sure ntex-notes.sty loads ntex-blocks (never twice). A missing
/// ntex-notes.sty is left alone.
pub fn ensure_required(paths: &Paths) -> io::Result<()> {
    let notes = paths.notes_sty();
    match std::fs::read_to_string(&notes) {
        Ok(t) => {
            let n = with_require(&t);
            if n != t {
                crate::inbox::write_atomic(&notes, &n)?;
            }
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

pub fn remove(paths: &Paths, env: &str) -> io::Result<()> {
    let path = paths.blocks_sty();
    let current = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let text = without_block(&current, env.trim());
    if text != current {
        crate::inbox::write_atomic(&path, &text)?;
    }
    Ok(())
}

/// Environment names a block may not take: the theme's own kinds, standard
/// LaTeX / amsmath / mathtools / tcolorbox environments, and words whose
/// `\name` or `\endname` LaTeX already defines (`\newtheorem` would fail).
pub const TAKEN: &[&str] = &[
    // the theme
    "theorem", "lemma", "proposition", "corollary", "definition", "example", "remark", "proof",
    "exercise", "solution", "problem",
    // LaTeX
    "document", "abstract", "array", "center", "description", "enumerate", "itemize", "list", "trivlist", "equation", "eqnarray",
    "figure", "table", "tabular", "tabbing", "flushleft", "flushright", "minipage", "picture", "quotation", "quote", "verse",
    "verbatim", "thebibliography", "theindex", "titlepage", "letter", "filecontents", "displaymath", "math", "sloppypar", "appendix",
    // amsmath, mathtools
    "align", "alignat", "flalign", "gather", "multline", "split", "aligned", "alignedat", "gathered", "cases", "matrix", "pmatrix",
    "bmatrix", "Bmatrix", "vmatrix", "Vmatrix", "smallmatrix", "subequations", "subarray", "dcases", "rcases", "multlined",
    // tcolorbox, tikz, graphics
    "tcolorbox", "tcbraster", "tcbitemize", "tcblisting", "tcbposter", "tikzpicture", "axis", "scope", "frame", "block",
    // commands \newtheorem would clash with
    "box", "par", "end", "begin", "item", "label", "ref", "text", "line", "hline", "vline", "mbox", "hbox", "vbox", "fbox", "frac",
    "sqrt", "sum", "int", "lim", "max", "min", "sup", "inf", "det", "log", "exp", "sin", "cos", "tan", "deg", "dim", "gcd", "hom",
    "ker", "arg", "bf", "it", "rm", "sf", "tt", "sc", "em", "sl", "large", "small", "tiny", "huge", "footnote", "caption", "section",
    "chapter", "part", "paragraph", "cite", "index", "title", "author", "date", "and", "today", "emph", "textbf", "space", "hfill",
    "vfill", "newline", "linebreak", "pagebreak", "newpage", "clearpage", "relax", "def", "let", "font", "input", "include", "color",
    "rule", "skip", "kern", "penalty", "mark", "insert", "accent", "char", "number", "string", "the", "or", "else", "fi", "if",
    "count", "dimen", "toks", "show", "message", "write", "read", "open", "close", "day", "month", "year", "time", "output",
    "span", "omit", "cr", "over", "above", "atop", "left", "right", "middle", "limits", "hat", "bar", "dot", "vec", "tilde", "check",
    "breve", "acute", "grave", "not", "neg", "quad", "qquad", "cdot", "ldots", "dots", "top", "bot", "prime", "angle", "iff",
    "implies", "to", "gets", "in", "ni", "subset", "supset", "cap", "cup", "wedge", "vee", "forall", "exists", "nabla", "partial",
    "infty", "emptyset", "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa", "lambda", "mu",
    "nu", "xi", "pi", "rho", "sigma", "tau", "upsilon", "phi", "chi", "psi", "omega", "ell", "hbar", "qed", "qedhere", "cref",
    "Cref", "url", "href", "tag", "notag", "nonumber", "intertext", "boxed", "operatorname", "mathrm", "mathbf", "mathbb",
    "mathcal", "colorbox", "textcolor", "pagecolor", "geometry", "titleformat", "tcbset", "newtheorem", "theoremstyle",
];

/// Checks: env name letters only and not taken (theorem, lemma, proposition,
/// corollary, definition, example, remark, proof, the standard LaTeX/amsmath
/// environments, other blocks); title not empty; contrast (body ≥ 7:1 on the
/// fill, head ≥ 4.5:1); trigger conflicts (via infer::check on the snippet).
pub fn check(block: &BlockDraft, existing: &[KnownBlock], known: &[KnownSnippet]) -> Vec<Finding> {
    let mut out = Vec::new();
    let f = |severity, code: &str, message: String| Finding { severity, code: code.into(), message, fix: None };
    let title = block.title.trim();
    let env = block.env.trim();
    if title.is_empty() {
        out.push(f(Severity::Error, "empty-title", "Title is empty.".into()));
    }
    if env.is_empty() {
        out.push(f(Severity::Error, "empty-env", "Environment name is empty.".into()));
    } else if !env.chars().all(|c| c.is_ascii_alphabetic()) {
        let suggestion = env_from_title(env);
        out.push(Finding {
            severity: Severity::Error,
            code: "env-letters".into(),
            message: format!("Environment names are letters only (a–z, A–Z): ‘{env}’."),
            fix: (!suggestion.is_empty()).then(|| Fix { label: format!("Use ‘{suggestion}’"), draft: None, block: Some(BlockDraft { env: suggestion, ..block.clone() }) }),
        });
    } else if TAKEN.contains(&env) {
        out.push(f(Severity::Error, "env-taken", format!("‘{env}’ is already a LaTeX environment or command; pick another name.")));
    } else if colour::palette_name(env).is_some() || ["Ink", "Slate", "Head"].iter().any(|n| n.eq_ignore_ascii_case(env)) {
        out.push(f(Severity::Error, "env-taken", format!("‘{env}’ is one of the theme's colour names; pick another name.")));
    } else if let Some(k) = existing.iter().find(|k| k.draft.env == env) {
        out.push(f(Severity::Error, "env-exists", format!("‘{env}’ is already one of your blocks (“{}”).", k.draft.title)));
    }
    if let BlockColour::Palette(p) = &block.colour {
        if colour::palette_name(p).is_none() {
            out.push(f(Severity::Error, "unknown-colour", format!("‘{p}’ is not one of the theme's colours (Blue, Violet, Teal, Maroon, Green, Grey).")));
        }
    }
    let t = colour::tones(&block.colour);
    let body = colour::contrast(colour::INK, t.back);
    if body < 7.0 {
        out.push(f(Severity::Warning, "low-contrast-body", format!("Body text on the fill is {body:.1} : 1, below 7 : 1.")));
    }
    let head = colour::contrast(t.head, t.back);
    if head < 4.5 {
        out.push(f(Severity::Warning, "low-contrast-head", format!("The head on the fill is {head:.1} : 1, below 4.5 : 1.")));
    }
    if let Some(snip) = snippet(block) {
        for x in crate::infer::check(&snip, known) {
            if x.severity != Severity::Note {
                out.push(Finding { fix: None, ..x });
            }
        }
    }
    out.sort_by_key(|x| match x.severity {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Note => 2,
    });
    out
}

/// Env name from a title: "Fun fact" → "funfact", "Côté" → "cote".
pub fn env_from_title(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        let c = c.to_lowercase().next().unwrap_or(c);
        if c.is_ascii_lowercase() {
            out.push(c);
        } else if let Some(f) = fold(c) {
            out.push_str(f);
        }
    }
    out
}

/// Latin letters with accents → ASCII.
fn fold(c: char) -> Option<&'static str> {
    Some(match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'æ' => "ae",
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
        'ď' | 'đ' | 'ð' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
        'ĥ' | 'ħ' => "h",
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
        'ĵ' => "j",
        'ķ' => "k",
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
        'ñ' | 'ń' | 'ņ' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => "o",
        'œ' => "oe",
        'ŕ' | 'ŗ' | 'ř' => "r",
        'ś' | 'ŝ' | 'ş' | 'š' | 'ș' => "s",
        'ß' => "ss",
        'ţ' | 'ť' | 'ŧ' | 'ț' => "t",
        'þ' => "th",
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'ŵ' => "w",
        'ý' | 'ÿ' | 'ŷ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        _ => return None,
    })
}

/// The env a specimen defines the new block under, so it never clashes with
/// the same block already in ntex-blocks.sty.
pub const SPECIMEN_ENV: &str = "ntexspecimen";

/// A small .tex that shows the new block beside the one it looks like, with
/// her theme (for the rendered preview). The block is defined in the
/// specimen's own preamble (under `ntexspecimen`), so it renders before it is
/// saved and still compiles once ntex-blocks.sty has it.
pub fn specimen(block: &BlockDraft) -> String {
    let base = base_name(block.base);
    let mut def = BlockDraft { env: SPECIMEN_ENV.into(), ..block.clone() };
    if def.title.trim().is_empty() {
        def.title = "New block".into();
    }
    let st = stanza_made(&def, &today());
    let base_text = match block.base {
        BlockBase::Theorem => "Every bounded monotone sequence converges; for instance $a_n = 1 - 2^{-n} \\to 1$.",
        BlockBase::Lemma => "If $a \\mid b$ and $b \\mid c$, then $a \\mid c$.",
        BlockBase::Definition => "A group is \\emph{abelian} if $ab = ba$ for all $a, b \\in G$.",
        BlockBase::Example => "The integers under addition, $(\\mathbb{Z}, +)$, form a group.",
        BlockBase::Remark => "The identity element of a group is unique.",
    };
    let opt = if block.with_title { "[Specimen]" } else { "" };
    format!(
        "% Ntex Studio block specimen: a {base} beside the new ‘{title_plain}’ block.\n\
         \\documentclass[11pt]{{article}}\n\
         \\usepackage{{ntex-notes}}\n\
         \\geometry{{paperwidth=13cm, paperheight=7cm, margin=6mm}}\n\
         \\pagestyle{{empty}}\n\
         \\setcounter{{section}}{{1}}\n\
         {st}\
         \\begin{{document}}\n\
         \\begin{{{base}}}\n\
         {base_text}\n\
         \\end{{{base}}}\n\
         \\begin{{{SPECIMEN_ENV}}}{opt}\n\
         The new block, with a little maths: $1 + 2 + \\dots + n = n(n+1)/2$ and $e^{{i\\pi}} + 1 = 0$.\n\
         \\end{{{SPECIMEN_ENV}}}\n\
         \\end{{document}}\n",
        title_plain = def.title.trim().replace('\n', " "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fun() -> BlockDraft {
        BlockDraft {
            title: "Fun fact".into(),
            env: "funfact".into(),
            base: BlockBase::Remark,
            numbering: Numbering::None,
            body: BodyFont::Upright,
            colour: BlockColour::Hue(145.0),
            cref: (String::new(), String::new()),
            trigger: "fun".into(),
            with_title: true,
        }
    }

    #[test]
    fn env_names() {
        assert_eq!(env_from_title("Fun fact"), "funfact");
        assert_eq!(env_from_title("Côté"), "cote");
        assert_eq!(env_from_title("Straße & Œuvre 2"), "strasseoeuvre");
        assert_eq!(env_from_title("  "), "");
    }

    #[test]
    fn escape_round_trip() {
        for s in ["Fun fact", "50% & #1 $x_y$ {a} ~^\\", "Côté"] {
            assert_eq!(tex_unescape(&tex_escape(s)), s);
        }
    }

    #[test]
    fn stanza_round_trip() {
        for colour in [BlockColour::Hue(145.0), BlockColour::Hue(12.5), BlockColour::Palette("Blue".into()), BlockColour::Palette("Proof".into())] {
            for numbering in [Numbering::None, Numbering::WithTheorems, Numbering::Own] {
                let b = BlockDraft { colour: colour.clone(), numbering, title: "Fun & fact".into(), ..fun() };
                let s = stanza_made(&b, "2026-10-07");
                let back = parse_stanza(&s).unwrap();
                assert_eq!(back, b, "{s}");
            }
        }
    }
}
