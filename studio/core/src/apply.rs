//! Applying a sort plan: cut each entry from the inbox, paste it into its
//! topic file (after `after_trigger`, else before the list's closing `})`),
//! making sure the helper it calls is in that file's `local … = h.…` line
//! (or written `h.math(…)`), creating new files when asked. All or nothing:
//! every touched file is copied to a backup folder first, and the result is
//! checked by `check` (the bridge's loader check); any failure restores
//! the backup.
//!
//! A backup folder is `<state>/backups/<now>/`: `manifest.json` (every file's
//! path and its copy, or `null` for a file that did not exist and is deleted
//! on undo) and `files/<n>-<name>`, byte-for-byte copies.
use crate::model::*;
use crate::{codegen, inbox, luaparse};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

/// The helpers an entry may call bare, in the order a locals line lists them.
pub const HELPERS: &[&str] = &["math", "math_tab", "text", "suffix", "post", "pat", "vis", "env"];

/// Is this a name the studio may write in snippets/tex/? (`x.lua`, no
/// folders, not the inbox, not a hidden or `zz_` file)
pub fn valid_file_name(name: &str) -> bool {
    name.ends_with(".lua")
        && name.len() > 4
        && !name.contains('/')
        && !name.contains('\\')
        && !name.starts_with('.')
        && !name.starts_with("zz_")
        && name != "inbox.lua"
        && name.chars().all(|c| !c.is_control())
}

/// A new topic file: a header comment like the others' first lines, the
/// helper, its locals (`helpers`, if any), and an empty group.
pub fn new_file(title: &str, helpers: &[String]) -> String {
    let t = title.replace(['\n', '\r'], " ");
    let t = t.trim();
    let mut s = format!(
        "-- {t}: snippets sorted here from the inbox by Ntex Studio.\n\
         --\n\
         -- One snippet per line, like the other files: math(\"ff\", [[\\frac{{<>}}{{<>}}]], \"yours\").\n\
         --   Full vocabulary: snippets/README.md. Save the file: it reloads.\n\
         local h = require(\"snips.helpers\")\n"
    );
    let mut hs: Vec<&str> = HELPERS.iter().copied().filter(|h| helpers.iter().any(|x| x == h)).collect();
    hs.dedup();
    if !hs.is_empty() {
        let rhs: Vec<String> = hs.iter().map(|h| format!("h.{h}")).collect();
        s.push_str(&format!("local {} = {}\n", hs.join(", "), rhs.join(", ")));
    }
    s.push_str(&format!("\nreturn h.group({}, {{\n}})\n", codegen::quoted(t)));
    s
}

/// The names a file binds with top-level `local` lines.
fn bound_names(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for l in text.lines() {
        let Some(rest) = l.strip_prefix("local ") else { continue };
        let rest = rest.trim_start();
        if let Some(f) = rest.strip_prefix("function ") {
            let n: String = f.trim_start().chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
            out.push(n);
            continue;
        }
        let lhs = rest.split('=').next().unwrap_or("");
        for n in lhs.split(',') {
            let n = n.trim();
            if !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                out.push(n.to_string());
            }
        }
    }
    out
}

/// The helper-locals line: (byte start, byte end) of the first top-level
/// `local a, b = h.a, h.b` line (every value an `h.` field).
fn locals_line(text: &str) -> Option<(usize, usize)> {
    let mut at = 0;
    for l in text.split_inclusive('\n') {
        let start = at;
        at += l.len();
        let line = l.trim_end_matches(['\n', '\r']);
        let Some(rest) = line.strip_prefix("local ") else { continue };
        let code = rest.split("--").next().unwrap_or("");
        let Some((lhs, rhs)) = code.split_once('=') else { continue };
        let names: Vec<&str> = lhs.split(',').map(str::trim).collect();
        let vals: Vec<&str> = rhs.split(',').map(str::trim).collect();
        let ident = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if names.iter().all(|n| ident(n))
            && !vals.is_empty()
            && vals.iter().all(|v| v.strip_prefix("h.").is_some_and(ident))
        {
            return Some((start, start + line.len()));
        }
    }
    None
}

/// The text with `name = h.name` added to its helper-locals line.
fn add_local(text: &str, (a, b): (usize, usize), name: &str) -> String {
    let line = &text[a..b];
    let eq = line.find('=').unwrap();
    let lhs_end = line[..eq].trim_end().len();
    // the values end before a trailing comment (and its spaces)
    let code_end = line.find("--").unwrap_or(line.len());
    let rhs_end = line[..code_end].trim_end().len();
    let mut l = String::new();
    l.push_str(&line[..lhs_end]);
    l.push_str(&format!(", {name}"));
    l.push_str(&line[lhs_end..rhs_end]);
    l.push_str(&format!(", h.{name}"));
    l.push_str(&line[rhs_end..]);
    let mut out = text.to_string();
    out.replace_range(a..b, &l);
    out
}

/// Make every helper the entry calls bare available in `target`: added to
/// its locals line, or (no such line) the call rewritten as `h.name(…)`.
/// Returns (target, entry).
pub(crate) fn fix_helpers(target: &str, entry: &str) -> (String, String) {
    let mut target = target.to_string();
    let mut entry = entry.to_string();
    let calls = luaparse::calls_in(&entry);
    let mut rewrite: Vec<usize> = Vec::new();
    for (at, name, qualified) in calls {
        if qualified || !HELPERS.contains(&name.as_str()) {
            continue;
        }
        if bound_names(&target).contains(&name) {
            continue;
        }
        match locals_line(&target) {
            Some(span) => target = add_local(&target, span, &name),
            None => rewrite.push(at),
        }
    }
    rewrite.sort();
    for at in rewrite.into_iter().rev() {
        entry.insert_str(at, "h.");
    }
    (target, entry)
}

fn indent_at(text: &str, pos: usize) -> String {
    let ls = text[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
    text[ls..].chars().take_while(|c| *c == ' ' || *c == '\t').collect()
}

/// `entry` pasted into `text`'s list: after the entry with trigger `after`
/// (its run of same-trigger entries, then `skip` more: the ones this plan
/// already put there), else before the closing `}`. None if `text` is not a
/// snippet list. Returns the text and whether the anchor was found.
fn insert(text: &str, entry: &str, after: Option<&str>, skip: usize) -> Option<(String, bool)> {
    let list = luaparse::list(text)?;
    let spans = &list.spans;
    if let Some(t) = after {
        let trig = |i: usize| luaparse::trigger_of(&text[spans[i].start..spans[i].end]);
        if let Some(mut i) = (0..spans.len()).find(|&i| trig(i).as_deref() == Some(t)) {
            while i + 1 < spans.len() && trig(i + 1).as_deref() == Some(t) {
                i += 1;
            }
            i = (i + skip).min(spans.len() - 1);
            let sp = &spans[i];
            let indent = indent_at(text, sp.call_start);
            let mut out = text.to_string();
            out.insert_str(sp.end, &format!("\n{indent}{entry}"));
            if !sp.has_comma {
                out.insert(sp.call_end, ',');
            }
            return Some((out, true));
        }
    }
    let indent = spans.last().map(|sp| indent_at(text, sp.call_start)).unwrap_or_else(|| "  ".into());
    let indent = if indent.is_empty() { "  ".to_string() } else { indent };
    let close = list.close;
    let ls = text[..close].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let mut out = text.to_string();
    if text[ls..close].trim().is_empty() && ls > list.open {
        out.insert_str(ls, &format!("{indent}{entry}\n"));
    } else {
        out.insert_str(close, &format!("\n{indent}{entry}\n"));
    }
    if let Some(sp) = spans.last() {
        if !sp.has_comma {
            out.insert(sp.call_end, ',');
        }
    }
    Some((out, false))
}

/// An inbox entry's text as it goes into a topic file: the `-- studio:`
/// marker line dropped, comma-terminated, no indentation on its first line
/// (the other lines exactly as they were: they may be inside a `[[…]]`).
fn entry_text(inbox_text: &str, sp: &luaparse::Span) -> String {
    let mut s = inbox_text[sp.start..sp.end].to_string();
    if !sp.has_comma {
        s.insert(sp.call_end - sp.start, ',');
    }
    let mut out = String::new();
    let mut dropped = false;
    for l in s.split_inclusive('\n') {
        if !dropped && l.trim_start().starts_with(inbox::MARKER) && inbox::parse_marker(l).is_some() {
            dropped = true;
            continue;
        }
        out.push_str(l);
    }
    out.trim_start_matches([' ', '\t']).trim_end_matches(['\n', '\r']).to_string()
}

/// Plan → new file texts (path → text), without writing. Entries the plan
/// does not mention, or that it sends to a file that cannot take them, stay.
pub fn plan_files(paths: &Paths, plan: &SortPlan) -> io::Result<(Vec<(PathBuf, String)>, SortOutcome)> {
    let tex = paths.snippets.join("tex");
    let inbox_path = paths.inbox();
    let mut outcome = SortOutcome { moved: Vec::new(), kept: Vec::new(), backup: PathBuf::new() };
    let inbox_text = match std::fs::read_to_string(&inbox_path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            for m in &plan.moves {
                outcome.kept.push((m.id.clone(), "the inbox is empty".into()));
            }
            return Ok((Vec::new(), outcome));
        }
        Err(e) => return Err(e),
    };
    let (Some(list), Some((_, raws))) = (luaparse::list(&inbox_text), inbox::raw_entries(&inbox_text)) else {
        for m in &plan.moves {
            outcome.kept.push((m.id.clone(), "inbox.lua can't be read as a snippet list".into()));
        }
        return Ok((Vec::new(), outcome));
    };

    // file name → (original text or None for a new file, current text)
    let mut texts: HashMap<String, (Option<String>, String)> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut placed: HashMap<(String, String), usize> = HashMap::new();
    let mut moved_ids: Vec<String> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();

    for mv in &plan.moves {
        if seen.contains(&mv.id.as_str()) {
            continue;
        }
        seen.push(&mv.id);
        let mut keep = |why: String| outcome.kept.push((mv.id.clone(), why));
        let Some(k) = raws.iter().position(|r| r.id == mv.id) else {
            keep("it is not in the inbox".into());
            continue;
        };
        let file = mv.file.trim().to_string();
        if !valid_file_name(&file) {
            keep(format!("‘{file}’ is not a snippet file name"));
            continue;
        }
        let entry = entry_text(&inbox_text, &list.spans[k]);
        if !texts.contains_key(&file) {
            let path = tex.join(&file);
            match std::fs::read_to_string(&path) {
                Ok(t) => {
                    texts.insert(file.clone(), (Some(t.clone()), t));
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    let title = mv.new_title.as_deref().map(str::trim).unwrap_or("");
                    if title.is_empty() {
                        keep(format!("{file} does not exist, and the plan gave no title for a new file"));
                        continue;
                    }
                    let helpers: Vec<String> = luaparse::calls_in(&entry).into_iter().filter(|c| !c.2).map(|c| c.1).collect();
                    texts.insert(file.clone(), (None, new_file(title, &helpers)));
                }
                Err(e) => return Err(e),
            }
            order.push(file.clone());
        }
        let current = texts[&file].1.clone();
        let (target, entry) = fix_helpers(&current, &entry);
        let anchor = mv.after_trigger.as_deref().map(str::trim).filter(|s| !s.is_empty());
        let skip = anchor.map(|a| *placed.get(&(file.clone(), a.to_string())).unwrap_or(&0)).unwrap_or(0);
        let Some((text, found)) = insert(&target, &entry, anchor, skip) else {
            keep(format!("{file} can't be read as a snippet list"));
            continue;
        };
        if let (Some(a), true) = (anchor, found) {
            *placed.entry((file.clone(), a.to_string())).or_insert(0) += 1;
        }
        texts.get_mut(&file).unwrap().1 = text;
        moved_ids.push(mv.id.clone());
        outcome.moved.push((mv.id.clone(), file));
    }
    for r in &raws {
        if !seen.contains(&r.id.as_str()) {
            outcome.kept.push((r.id.clone(), "the plan left it in the inbox".into()));
        }
    }

    let mut files = Vec::new();
    for name in &order {
        let (orig, now) = &texts[name];
        if orig.as_deref() != Some(now.as_str()) && outcome.moved.iter().any(|(_, f)| f == name) {
            files.push((tex.join(name), now.clone()));
        }
    }
    if !moved_ids.is_empty() {
        files.push((inbox_path, inbox::without(&inbox_text, &moved_ids)));
    }
    Ok((files, outcome))
}

/// Write the plan's files after backing up. `check(path, text)` returns
/// Err(message) for a file that would not load; then everything is restored.
pub fn apply(
    paths: &Paths,
    plan: &SortPlan,
    now: &str,
    check: &mut dyn FnMut(&Path, &str) -> Result<(), String>,
) -> io::Result<SortOutcome> {
    let (files, mut outcome) = plan_files(paths, plan)?;
    let touched: Vec<PathBuf> = files.iter().map(|(p, _)| p.clone()).collect();
    let dir = backup(paths, &touched, now)?;
    outcome.backup = dir.clone();
    let res: Result<(), String> = (|| {
        for (p, t) in &files {
            inbox::write_atomic(p, t).map_err(|e| format!("could not write {}: {e}", name_of(p)))?;
        }
        for (p, t) in &files {
            check(p, t).map_err(|e| if e.contains(&name_of(p)) { e } else { format!("{}: {e}", name_of(p)) })?;
        }
        Ok(())
    })();
    if let Err(msg) = res {
        return Err(match undo(&dir) {
            Ok(()) => io::Error::other(format!("{msg} (nothing was changed)")),
            Err(e) => io::Error::other(format!("{msg}; restoring the backup also failed ({e}): the files are in {}", dir.display())),
        });
    }
    Ok(outcome)
}

fn name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Manifest {
    made: String,
    files: Vec<ManifestFile>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ManifestFile {
    /// where it lives
    path: PathBuf,
    /// its copy, relative to the backup folder; None: it did not exist
    copy: Option<String>,
}

/// Write bytes atomically (temp file in the same folder, then rename),
/// keeping the original's permissions.
pub fn write_atomic_bytes(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let tmp = dir.join(format!(".{}.{}-{nanos}.studio-tmp", name_of(path), std::process::id()));
    let res = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
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

/// Put back the files from a backup folder (Undo).
pub fn undo(backup: &Path) -> io::Result<()> {
    let m: Manifest = serde_json::from_str(&std::fs::read_to_string(backup.join("manifest.json"))?).map_err(io::Error::other)?;
    let mut first_err = None;
    for f in &m.files {
        let r = match &f.copy {
            Some(c) => std::fs::read(backup.join(c)).and_then(|b| write_atomic_bytes(&f.path, &b)),
            None => match std::fs::remove_file(&f.path) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                r => r,
            },
        };
        if let Err(e) = r {
            first_err.get_or_insert(io::Error::new(e.kind(), format!("{}: {e}", f.path.display())));
        }
    }
    match first_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Back up the given files to a new folder under state/backups/<now>/ and
/// return it (also used before saving and deleting, so ⌘Z works for those).
pub fn backup(paths: &Paths, files: &[std::path::PathBuf], now: &str) -> io::Result<std::path::PathBuf> {
    let root = paths.state.join("backups");
    let mut base: String = now.chars().map(|c| if c.is_ascii_alphanumeric() || "-_.T".contains(c) { c } else { '-' }).collect();
    if base.is_empty() || base.starts_with('.') {
        base = format!("b{base}");
    }
    let mut dir = root.join(&base);
    let mut k = 2;
    while dir.exists() {
        dir = root.join(format!("{base}-{k}"));
        k += 1;
    }
    std::fs::create_dir_all(dir.join("files"))?;
    let mut m = Manifest { made: now.to_string(), files: Vec::new() };
    let mut done: Vec<&PathBuf> = Vec::new();
    for (i, f) in files.iter().enumerate() {
        if done.contains(&f) {
            continue;
        }
        done.push(f);
        match std::fs::read(f) {
            Ok(bytes) => {
                let rel = format!("files/{i}-{}", name_of(f));
                std::fs::write(dir.join(&rel), bytes)?;
                m.files.push(ManifestFile { path: f.clone(), copy: Some(rel) });
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => m.files.push(ManifestFile { path: f.clone(), copy: None }),
            Err(e) => return Err(e),
        }
    }
    let json = serde_json::to_string_pretty(&m).map_err(io::Error::other)?;
    write_atomic_bytes(&dir.join("manifest.json"), json.as_bytes())?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locals() {
        let t = "local h = require(\"snips.helpers\")\nlocal cond = require(\"snips.cond\")\nlocal math, text = h.math, h.text -- the helpers\nreturn h.group(\"T\", {\n})\n";
        let span = locals_line(t).unwrap();
        assert_eq!(&t[span.0..span.1], "local math, text = h.math, h.text -- the helpers");
        let t2 = add_local(t, span, "vis");
        assert!(t2.contains("local math, text, vis = h.math, h.text, h.vis -- the helpers\n"), "{t2}");
        assert!(bound_names(&t2).contains(&"cond".to_string()));
        assert!(locals_line("local h = require(\"snips.helpers\")\nlocal x = 1\n").is_none());
    }

    #[test]
    fn helpers_fixed() {
        let with = "local h = require(\"snips.helpers\")\nlocal math = h.math\nreturn h.group(\"G\", {\n  math(\"a\", [[b]], \"o\"),\n})\n";
        let (t, e) = fix_helpers(with, "vis(\"S\", [[\\sqrt{%V}]], \"yours\"),");
        assert!(t.contains("local math, vis = h.math, h.vis\n"));
        assert_eq!(e, "vis(\"S\", [[\\sqrt{%V}]], \"yours\"),");
        let without = "local h = require(\"snips.helpers\")\nreturn h.group(\"G\", {\n})\n";
        let (t, e) = fix_helpers(without, "-- note\n  math(\"a\", [[b]], \"o\"),");
        assert_eq!(t, without);
        assert_eq!(e, "-- note\n  h.math(\"a\", [[b]], \"o\"),");
        // already qualified, or bound by the file: untouched
        let (t, e) = fix_helpers(with, "h.text(\"x\", [[y]], \"o\"),");
        assert_eq!((t.as_str(), e.as_str()), (with, "h.text(\"x\", [[y]], \"o\"),"));
    }

    #[test]
    fn new_files() {
        let f = new_file("Probability", &["math".into(), "text".into(), "math".into()]);
        assert!(f.starts_with("-- Probability: "));
        assert!(f.contains("local h = require(\"snips.helpers\")\nlocal math, text = h.math, h.text\n"));
        assert!(f.ends_with("return h.group(\"Probability\", {\n})\n"));
        assert!(luaparse::list(&f).is_some());
        assert!(valid_file_name("probability.lua"));
        for bad in ["inbox.lua", "../x.lua", "a/b.lua", ".x.lua", "zz_studio_try.lua", "x.txt", ".lua"] {
            assert!(!valid_file_name(bad), "{bad}");
        }
    }

    #[test]
    fn inserting() {
        let t = "return h.group(\"G\", {\n  math(\"a\", [[1]], \"o\"),\n  math(\"b\", [[2]], \"o\") -- last\n})\n";
        let (o, found) = insert(t, "math(\"n\", [[n]], \"o\"),", Some("a"), 0).unwrap();
        assert!(found);
        assert_eq!(o, "return h.group(\"G\", {\n  math(\"a\", [[1]], \"o\"),\n  math(\"n\", [[n]], \"o\"),\n  math(\"b\", [[2]], \"o\") -- last\n})\n");
        let (o, found) = insert(t, "math(\"n\", [[n]], \"o\"),", Some("zz"), 0).unwrap();
        assert!(!found);
        assert_eq!(o, "return h.group(\"G\", {\n  math(\"a\", [[1]], \"o\"),\n  math(\"b\", [[2]], \"o\"), -- last\n  math(\"n\", [[n]], \"o\"),\n})\n");
        let (o, _) = insert("return h.group(\"G\", {})", "math(\"n\", [[n]], \"o\"),", None, 0).unwrap();
        assert_eq!(o, "return h.group(\"G\", {\n  math(\"n\", [[n]], \"o\"),\n})");
    }
}
