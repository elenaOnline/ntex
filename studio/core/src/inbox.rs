//! snippets/tex/inbox.lua: the studio's dumping ground. It is an ordinary
//! snippet file (ntex hot-reloads it), with a `-- studio: id=… made=…` marker
//! line above each entry. Hand edits are fine: entries without a marker get
//! one (id from the trigger) the next time the studio writes the file.
use crate::model::*;
use crate::{codegen, luaparse};
use std::io;
use std::path::Path;

pub const MARKER: &str = "-- studio:";

/// The helper locals line every studio-written file binds.
pub const LOCALS: &str = "local math, math_tab, text, suffix, post, pat, vis = h.math, h.math_tab, h.text, h.suffix, h.post, h.pat, h.vis";

/// The file as written when it does not exist yet (header comment explaining
/// it, helper locals, `return h.group("Inbox", { })`).
pub fn empty_file() -> String {
    format!(
        "-- Snippets made in Ntex Studio, waiting to be sorted. Safe to edit by hand.\n\
         --\n\
         -- Each entry has a `-- studio: id=… made=…` line above it; keep it, so the\n\
         -- studio can find the entry again. Ntex Studio's Inbox page sorts these into\n\
         -- your files. Full vocabulary: snippets/README.md. Save the file: it reloads.\n\
         local h = require(\"snips.helpers\")\n\
         {LOCALS}\n\
         \n\
         return h.group(\"Inbox\", {{\n\
         }})\n"
    )
}

/// `-- studio: id=ff-1432 made=2026-10-07T14:32` → (id, made).
pub fn parse_marker(line: &str) -> Option<(String, String)> {
    let rest = line.trim().strip_prefix(MARKER)?;
    let mut id = None;
    let mut made = String::new();
    for part in rest.split_whitespace() {
        if let Some(v) = part.strip_prefix("id=") {
            id = Some(v.to_string());
        } else if let Some(v) = part.strip_prefix("made=") {
            made = v.to_string();
        }
    }
    Some((id.filter(|s| !s.is_empty())?, made))
}

pub fn marker(id: &str, made: &str) -> String {
    format!("{MARKER} id={id} made={made}")
}

/// A trigger → the readable part of an id: letters and digits kept, other
/// runs become `-` ("//" → "s").
pub fn slug(trigger: &str) -> String {
    let mut s = String::new();
    for c in trigger.chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('-') {
            s.push('-');
        }
    }
    let s = s.trim_matches('-').to_string();
    if s.is_empty() { "s".into() } else { s }
}

/// `2026-10-07T14:32` → `1432`.
fn hhmm(now: &str) -> String {
    match now.split_once('T') {
        Some((_, t)) => t.chars().filter(|c| c.is_ascii_digit()).take(4).collect(),
        None => now.chars().filter(|c| c.is_ascii_digit()).rev().take(4).collect::<Vec<_>>().into_iter().rev().collect(),
    }
}

/// An id not in `taken`: `<slug>-<HHMM>`, then `-2`, `-3` …
pub fn new_id(trigger: &str, now: &str, taken: &[String]) -> String {
    let base = format!("{}-{}", slug(trigger), hhmm(now));
    if !taken.iter().any(|t| *t == base) {
        return base;
    }
    (2..).map(|k| format!("{base}-{k}")).find(|c| !taken.iter().any(|t| t == c)).unwrap()
}

/// The id read() gives an entry without a marker: stable while its text is.
fn unmarked_id(src: &str) -> String {
    let trig = luaparse::trigger_of(src).unwrap_or_default();
    // FNV-1a of the trimmed source
    let mut h: u32 = 0x811c9dc5;
    for b in src.trim().bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    format!("{}-h{:04x}", slug(&trig), h & 0xffff)
}

/// One entry as it sits in the file.
#[derive(Clone, Debug)]
pub(crate) struct Raw {
    pub id: String,
    pub made: String,
    pub marked: bool,
    /// byte range of the whole entry (marker and comments included)
    pub start: usize,
    pub end: usize,
    /// byte range of the call itself (no comments, no comma)
    pub call_start: usize,
    pub call_end: usize,
    /// the entry's text without the marker line
    pub source: String,
    /// 1-based line of the marker (or of the entry when unmarked)
    pub line: usize,
}

/// The entries of an inbox text, with stable unique ids.
pub(crate) fn raw_entries(text: &str) -> Option<(luaparse::List, Vec<Raw>)> {
    let list = luaparse::list(text)?;
    let mut out: Vec<Raw> = Vec::new();
    for sp in &list.spans {
        let full = &text[sp.start..sp.end];
        let mut source_lines = Vec::new();
        let mut found = None;
        let mut line = sp.line;
        // marker lines sit above the call; the one nearest the call wins (a
        // marker left behind by a hand-deleted entry is stale), and none of
        // them is part of the source
        let call_rel = sp.call_start - sp.start;
        let mut off = 0;
        for (k, l) in full.split('\n').enumerate() {
            let before_call = off < call_rel;
            off += l.len() + 1;
            if before_call && l.trim_start().starts_with(MARKER) {
                if let Some(m) = parse_marker(l) {
                    found = Some(m);
                    line = sp.line + k;
                    continue;
                }
            }
            source_lines.push(l);
        }
        // the first kept line lost its indentation in the span; re-trim
        let source = source_lines.join("\n").trim_start().to_string();
        let (id, made, marked) = match found {
            Some((id, made)) => (id, made, true),
            None => (unmarked_id(&source), String::new(), false),
        };
        out.push(Raw { id, made, marked, start: sp.start, end: sp.end, call_start: sp.call_start, call_end: sp.call_end, source, line });
    }
    // make ids unique (a hand-copied marker): later duplicates get -2, -3 …
    let mut seen: Vec<String> = Vec::new();
    for r in &mut out {
        if seen.contains(&r.id) {
            let base = r.id.clone();
            r.id = (2..).map(|k| format!("{base}-{k}")).find(|c| !seen.contains(c)).unwrap();
            r.marked = false;
        }
        seen.push(r.id.clone());
    }
    Some((list, out))
}

fn entries_of(text: &str) -> Vec<InboxEntry> {
    let Some((_, raws)) = raw_entries(text) else { return Vec::new() };
    raws.into_iter()
        .map(|r| {
            let draft = luaparse::parse_entry(&r.source).map(|mut d| {
                d.id = Some(r.id.clone());
                d
            });
            InboxEntry {
                trigger: luaparse::trigger_of(&r.source).unwrap_or_else(|| "?".into()),
                id: r.id,
                made: r.made,
                source: r.source,
                draft,
                line: r.line,
            }
        })
        .collect()
}

/// Parse an inbox text (for tests and for the UI's own copies).
pub fn parse(text: &str) -> Vec<InboxEntry> {
    entries_of(text)
}

pub fn read(paths: &Paths) -> io::Result<Vec<InboxEntry>> {
    match std::fs::read_to_string(paths.inbox()) {
        Ok(t) => Ok(entries_of(&t)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

fn read_or_empty(path: &Path) -> io::Result<String> {
    match std::fs::read_to_string(path) {
        Ok(t) => Ok(t),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(empty_file()),
        Err(e) => Err(e),
    }
}

/// Write `text` to `path` atomically: a temp file in the same folder (not
/// ending in .lua, so the snippet loader never sees it), then rename.
pub fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    use std::io::Write;
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let tmp = dir.join(format!(".{name}.{}-{nanos}.studio-tmp", std::process::id()));
    let res = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        drop(f);
        // keep the original's permissions
        if let Ok(m) = std::fs::metadata(path) {
            let _ = std::fs::set_permissions(&tmp, m.permissions());
        }
        std::fs::rename(&tmp, path)
    })();
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

/// Add (draft.id None) or replace (draft.id Some) an entry; returns its id.
/// Writes atomically (temp file + rename) so ntex never reloads half a file.
pub fn save(paths: &Paths, draft: &Draft, now: &str) -> io::Result<String> {
    let path = paths.inbox();
    let current = read_or_empty(&path)?;
    let (text, id) = with_draft(&current, draft, now);
    write_atomic(&path, &text)?;
    Ok(id)
}

/// Remove entries by id (delete, or after sorting). Atomic.
pub fn remove(paths: &Paths, ids: &[String]) -> io::Result<()> {
    let path = paths.inbox();
    let current = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let text = without(&current, ids);
    if text != current {
        write_atomic(&path, &text)?;
    }
    Ok(())
}

/// The text with the given entries cut out (marker, comments and line included).
pub fn without(current: &str, ids: &[String]) -> String {
    let Some((_, raws)) = raw_entries(current) else { return current.to_string() };
    let mut cuts: Vec<(usize, usize)> = raws.iter().filter(|r| ids.contains(&r.id)).map(|r| line_range(current, r.start, r.end)).collect();
    cuts.sort();
    let mut out = current.to_string();
    for (a, b) in cuts.into_iter().rev() {
        out.replace_range(a..b, "");
    }
    out
}

/// Widen [start, end) to whole lines when the entry is alone on them
/// (leading indentation and the trailing newline go too).
pub(crate) fn line_range(text: &str, start: usize, end: usize) -> (usize, usize) {
    let ls = text[..start].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let le = text[end..].find('\n').map(|i| end + i).unwrap_or(text.len());
    let before_blank = text[ls..start].trim().is_empty();
    let after_blank = text[end..le].trim().is_empty();
    match (before_blank, after_blank) {
        (true, true) => (ls, (le + 1).min(text.len())),
        (true, false) => (start, end),
        (false, _) => (start, end),
    }
}

/// The whole file text with `draft` added/replaced, without writing (for
/// checking it with the bridge before saving).
pub fn with_draft(current: &str, draft: &Draft, now: &str) -> (String, String) {
    let base = if luaparse::list(current).is_some() { current.to_string() } else { recover(current) };
    let (list, raws) = raw_entries(&base).expect("inbox list");
    let taken: Vec<String> = raws.iter().map(|r| r.id.clone()).collect();
    let code = codegen::entry(draft);

    // replace an entry with this id?
    let target = draft.id.as_ref().and_then(|id| raws.iter().find(|r| &r.id == id));
    if code.trim().is_empty() {
        // nothing to write (an empty raw entry): leave the file alone
        let id = target.map(|r| r.id.clone()).or_else(|| draft.id.clone()).unwrap_or_default();
        return (base, id);
    }
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut assigned = taken.clone();
    let id;
    match target {
        Some(r) if draft.kind != Kind::RawLua => {
            // only the call changes: her comments around it stay
            id = r.id.clone();
            let call = code.trim_start().trim_end_matches(',');
            edits.push((r.call_start, r.call_end, call.to_string()));
            if !r.marked {
                let ls = base[..r.start].rfind('\n').map(|i| i + 1).unwrap_or(0);
                let indent: String = base[ls..r.start].chars().take_while(|c| c.is_whitespace()).collect();
                edits.push((r.start, r.start, format!("{}\n{indent}", marker(&id, now))));
            }
        }
        Some(r) => {
            // raw Lua is the whole entry, comments included (its source)
            id = r.id.clone();
            let made = if r.made.is_empty() { now.to_string() } else { r.made.clone() };
            edits.push((r.start, r.end, format!("{}\n{}", marker(&id, &made), code)));
        }
        None => {
            id = match &draft.id {
                Some(i) if !i.is_empty() && !taken.contains(i) => i.clone(),
                _ => new_id(&draft.trigger, now, &taken),
            };
            assigned.push(id.clone());
            // before the line holding the closing `}`
            let close = list.close;
            let ls = base[..close].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let entry = format!("  {}\n{}\n", marker(&id, now), code);
            if base[ls..close].trim().is_empty() {
                edits.push((ls, ls, entry));
            } else {
                edits.push((close, close, format!("\n{entry}")));
            }
            // the last entry needs its comma
            if let Some(last) = list.spans.last() {
                if !last.has_comma {
                    edits.push((last.call_end, last.call_end, ",".into()));
                }
            }
        }
    }
    // unmarked entries get a marker now
    for r in &raws {
        if r.marked || target.is_some_and(|t| t.id == r.id) {
            continue;
        }
        let trig = luaparse::trigger_of(&r.source).unwrap_or_default();
        let nid = new_id(&trig, now, &assigned);
        assigned.push(nid.clone());
        let ls = base[..r.start].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let indent: String = base[ls..r.start].chars().take_while(|c| c.is_whitespace()).collect();
        if base[ls..r.start].trim().is_empty() {
            edits.push((r.start, r.start, format!("{}\n{indent}", marker(&nid, now))));
        } else {
            edits.push((r.start, r.start, format!("\n  {}\n  ", marker(&nid, now))));
        }
    }
    edits.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    let mut out = base.clone();
    for (a, b, s) in edits.into_iter().rev() {
        out.replace_range(a..b, &s);
    }
    (out, id)
}

/// A file the studio can't read as a snippet list: start a fresh inbox and
/// keep the old text below it as a comment, so nothing is lost.
fn recover(current: &str) -> String {
    if current.trim().is_empty() {
        return empty_file();
    }
    let mut s = String::from("-- The previous inbox.lua could not be read as a snippet list; it is kept\n-- here, commented out. Copy back what you need.\n");
    for l in current.lines() {
        s.push_str("-- | ");
        s.push_str(l);
        s.push('\n');
    }
    s.push_str(&empty_file());
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(trig: &str, body: &str) -> Draft {
        Draft { trigger: trig.into(), output: codegen::parse_body(body), ..Draft::default() }
    }

    fn temp_paths(tag: &str) -> Paths {
        let base = std::env::temp_dir().join(format!("ntex-core-inbox-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("snippets/tex")).unwrap();
        Paths { config: base.clone(), snippets: base.join("snippets"), templates_tex: base.join("templates/tex"), state: base.join("state") }
    }

    #[test]
    fn empty_file_is_a_list() {
        let f = empty_file();
        assert!(f.starts_with("-- Snippets made in Ntex Studio, waiting to be sorted. Safe to edit by hand.\n"));
        assert!(f.contains("local h = require(\"snips.helpers\")\n"));
        assert!(f.contains(LOCALS));
        assert!(f.trim_end().ends_with("return h.group(\"Inbox\", {\n})"));
        assert!(parse(&f).is_empty());
        assert!(luaparse::list(&f).is_some());
    }

    #[test]
    fn add_replace_ids() {
        let (t1, id1) = with_draft(&empty_file(), &d("ff", r"\frac{<>}{<>}"), "2026-10-07T14:32");
        assert_eq!(id1, "ff-1432");
        assert!(t1.contains("  -- studio: id=ff-1432 made=2026-10-07T14:32\n  math(\"ff\", [[\\frac{<>}{<>}]], \"yours\"),\n})"), "{t1}");
        let (t2, id2) = with_draft(&t1, &d("ff", r"\dfrac{<>}{<>}"), "2026-10-07T14:32");
        assert_eq!(id2, "ff-1432-2");
        let (t3, id3) = with_draft(&t2, &d("//", "x"), "2026-10-07T14:32");
        assert_eq!(id3, "s-1432");
        let e = parse(&t3);
        assert_eq!(e.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["ff-1432", "ff-1432-2", "s-1432"]);
        assert_eq!(e[0].line, t3.lines().position(|l| l.contains("id=ff-1432 ")).unwrap() + 1);
        assert_eq!(e[1].source, "math(\"ff\", [[\\dfrac{<>}{<>}]], \"yours\"),");
        assert_eq!(e[1].draft.as_ref().unwrap().id.as_deref(), Some("ff-1432-2"));
        // replace keeps id, made and place
        let mut r = d("ff", r"\tfrac{<>}{<>}");
        r.id = Some("ff-1432".into());
        let (t4, id4) = with_draft(&t3, &r, "2026-10-07T15:00");
        assert_eq!(id4, "ff-1432");
        let e = parse(&t4);
        assert_eq!(e.len(), 3);
        assert_eq!(e[0].id, "ff-1432");
        assert_eq!(e[0].made, "2026-10-07T14:32");
        assert!(e[0].source.contains("tfrac"));
        assert!(t4.contains("  -- studio: id=ff-1432 made=2026-10-07T14:32\n  math(\"ff\", [[\\tfrac"), "{t4}");
        // remove
        let t5 = without(&t4, &["ff-1432-2".into()]);
        assert_eq!(parse(&t5).iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["ff-1432", "s-1432"]);
        assert!(!t5.contains("dfrac"));
        assert!(!t5.contains("\n\n  -- studio: id=s"));
    }

    #[test]
    fn hand_edits() {
        let mut t = empty_file();
        t = t.replace(
            "{\n})",
            "{\n  math(\"hh\", [[\\hbar]], \"yours\"),\n  -- a note\n  env(\"pmat\", \"pmatrix\", \"yours\")\n})",
        );
        let e = parse(&t);
        assert_eq!(e.len(), 2);
        assert!(e[0].id.starts_with("hh-h"));
        assert_eq!(e[0].made, "");
        assert!(e[0].draft.is_some());
        assert!(e[1].draft.is_none());
        assert_eq!(e[1].trigger, "pmat");
        assert_eq!(e[1].source, "-- a note\n  env(\"pmat\", \"pmatrix\", \"yours\")");
        // stable while the text is
        assert_eq!(parse(&t)[0].id, e[0].id);
        // edit the unmarked one by its read id: replaced, and the other gets a marker
        let mut dr = e[0].draft.clone().unwrap();
        dr.output = codegen::parse_body(r"\hslash");
        let (t2, id) = with_draft(&t, &dr, "2026-10-07T09:05");
        assert_eq!(id, e[0].id);
        let e2 = parse(&t2);
        assert_eq!(e2.len(), 2);
        assert!(e2[0].source.contains("hslash"));
        assert_eq!(e2[1].id, "pmat-0905");
        assert!(t2.contains("  -- studio: id=pmat-0905 made=2026-10-07T09:05\n  -- a note\n  env("), "{t2}");
        // a new entry after a comma-less last entry
        let (t3, _) = with_draft(&t, &d("q", "x"), "2026-10-07T09:06");
        assert!(t3.contains("\"pmatrix\", \"yours\"),"), "{t3}");
        assert_eq!(parse(&t3).len(), 3);
    }

    #[test]
    fn unreadable_file_is_kept() {
        let (t, id) = with_draft("this is not lua {", &d("ff", "x"), "2026-10-07T14:32");
        assert!(t.contains("-- | this is not lua {"));
        assert_eq!(parse(&t).len(), 1);
        assert_eq!(id, "ff-1432");
    }

    #[test]
    fn save_and_remove_on_disk() {
        let p = temp_paths("disk");
        assert!(read(&p).unwrap().is_empty());
        let id = save(&p, &d("ff", r"\frac{<>}{<>}"), "2026-10-07T14:32").unwrap();
        let id2 = save(&p, &d("gg", r"\gamma"), "2026-10-07T14:33").unwrap();
        let e = read(&p).unwrap();
        assert_eq!(e.len(), 2);
        // no temp files left behind
        let names: Vec<String> = std::fs::read_dir(p.snippets.join("tex")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        assert_eq!(names, ["inbox.lua"]);
        remove(&p, &[id.clone()]).unwrap();
        let e = read(&p).unwrap();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].id, id2);
        let _ = std::fs::remove_dir_all(&p.config);
    }
}
