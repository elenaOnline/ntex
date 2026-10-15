//! core::apply against a temp copy of ~/.config/texnvim/snippets (never the
//! real files). The check is a bracket-balance check, plus the real loader
//! check in a headless ntex (`require("snips").check(path)`) when nvim is
//! installed.
use ntex_studio_core::apply;
use ntex_studio_core::model::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const NOW: &str = "2026-10-07T15.00.00-1";

const INBOX: &str = r#"-- Snippets made in Ntex Studio, waiting to be sorted. Safe to edit by hand.
local h = require("snips.helpers")
local math, math_tab, text, suffix, post, pat, vis = h.math, h.math_tab, h.text, h.suffix, h.post, h.pat, h.vis

return h.group("Inbox", {
  -- studio: id=fun-1440 made=2026-10-07T14:40
  text("fun", [[
\begin{funfact}[<>]
	<>
\end{funfact}]], "yours", { tab = true, line = "start", ft = "tex", desc = "Fun fact block" }),
  -- studio: id=sq-1441 made=2026-10-07T14:41
  vis("Q", [[\sqrt[3]{ %V }]], "yours"),
  -- studio: id=pr-1450 made=2026-10-07T14:50
  math("prb", [[\mathbb{P}(<>)]], "yours"),
  -- studio: id=ex-1451 made=2026-10-07T14:51
  math("exx", [[\mathbb{E}[<>] ]], "yours"),
  -- studio: id=zz-1452 made=2026-10-07T14:52
  math("zzz", [[\zeta_{<>}]], "yours"),
  -- studio: id=kp-1454 made=2026-10-07T14:54
  math("kpp", [[\kappa]], "yours"),
  -- studio: id=st-1453 made=2026-10-07T14:53
  math("stay", [[\star]], "yours")
})
"#;

const BARE: &str = "-- Bare: a file without a helper-locals line.\nlocal h = require(\"snips.helpers\")\n\nreturn h.group(\"Bare\", {\n  h.math(\"bq\", [[b]], \"yours\"),\n})\n";

struct Temp {
    root: PathBuf,
    paths: Paths,
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            copy_dir(&p, &to.join(e.file_name()));
        } else {
            std::fs::copy(&p, to.join(e.file_name())).unwrap();
        }
    }
}

/// A temp config with a copy of her snippets/tex, the test inbox and bare.lua.
fn temp(tag: &str) -> Temp {
    let root = std::env::temp_dir().join(format!("ntex-core-apply-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let real = PathBuf::from(std::env::var("HOME").unwrap()).join(".config/texnvim/snippets/tex");
    copy_dir(&real, &root.join("snippets/tex"));
    // the real inbox (if any) is replaced by the test's
    std::fs::write(root.join("snippets/tex/inbox.lua"), INBOX).unwrap();
    std::fs::write(root.join("snippets/tex/bare.lua"), BARE).unwrap();
    let paths = Paths { config: root.clone(), snippets: root.join("snippets"), templates_tex: root.join("templates/tex"), state: root.join("state") };
    Temp { root, paths }
}

fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir).unwrap().flatten().map(|e| (e.file_name().to_string_lossy().into_owned(), std::fs::read(e.path()).unwrap())).collect()
}

fn mv(id: &str, file: &str, after: Option<&str>, title: Option<&str>) -> SortMove {
    SortMove { id: id.into(), file: file.into(), after_trigger: after.map(Into::into), new_title: title.map(Into::into), reason: "test".into() }
}

fn plan() -> SortPlan {
    SortPlan {
        moves: vec![
            // multi-line entry, after a multi-line entry
            mv("fun-1440", "text.lua", Some("beg"), None),
            // operations.lua binds math and suffix: vis is added to its locals
            mv("sq-1441", "operations.lua", None, None),
            // a new file, two entries in plan order
            mv("pr-1450", "probability.lua", None, Some("Probability")),
            mv("ex-1451", "probability.lua", None, Some("Probability")),
            // a file that doesn't exist, no title: stays
            mv("zz-1452", "nowhere.lua", None, None),
            // no locals line: written h.math(…); the inbox's last entry had no comma
            mv("st-1453", "bare.lua", Some("not-a-trigger"), None),
            mv("no-such-id", "greek.lua", None, None),
        ],
        notes: String::new(),
    }
}

/// Brackets balance outside strings and comments; the file returns a list.
fn balanced(text: &str) -> Result<(), String> {
    let b = text.as_bytes();
    let mut stack: Vec<u8> = Vec::new();
    let mut i = 0;
    let mut line = 1;
    let long_level = |i: usize| -> Option<usize> {
        if b.get(i) != Some(&b'[') {
            return None;
        }
        let mut j = i + 1;
        while b.get(j) == Some(&b'=') {
            j += 1;
        }
        (b.get(j) == Some(&b'[')).then_some(j - i - 1)
    };
    let skip_long = |i: usize, lvl: usize, line: &mut usize| -> Result<usize, String> {
        let close = format!("]{}]", "=".repeat(lvl));
        let start = i + lvl + 2;
        let rel = text[start..].find(&close).ok_or(format!("line {line}: unclosed long bracket"))?;
        *line += text[start..start + rel].matches('\n').count();
        Ok(start + rel + close.len())
    };
    while i < b.len() {
        let c = b[i];
        match c {
            b'\n' => {
                line += 1;
                i += 1;
            }
            b'-' if b.get(i + 1) == Some(&b'-') => {
                if let Some(l) = long_level(i + 2) {
                    i = skip_long(i + 2, l, &mut line)?;
                } else {
                    while i < b.len() && b[i] != b'\n' {
                        i += 1;
                    }
                }
            }
            b'"' | b'\'' => {
                i += 1;
                while i < b.len() && b[i] != c {
                    if b[i] == b'\n' {
                        return Err(format!("line {line}: unfinished string"));
                    }
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
            }
            b'[' if long_level(i).is_some() => {
                let l = long_level(i).unwrap();
                i = skip_long(i, l, &mut line)?;
            }
            b'(' | b'[' | b'{' => {
                stack.push(c);
                i += 1;
            }
            b')' | b']' | b'}' => {
                let want = match c {
                    b')' => b'(',
                    b']' => b'[',
                    _ => b'{',
                };
                if stack.pop() != Some(want) {
                    return Err(format!("line {line}: unbalanced '{}'", c as char));
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    if !stack.is_empty() {
        return Err("unclosed bracket at the end".into());
    }
    if !text.contains("return h.group(") {
        return Err("no `return h.group(…)`".into());
    }
    Ok(())
}

/// The real loader check in a headless ntex: one nvim for all `files`.
/// None when nvim isn't installed.
fn nvim_check(snippets: &Path, files: &[PathBuf]) -> Option<Result<(), String>> {
    let nvim = ["/opt/homebrew/bin/nvim", "/usr/local/bin/nvim"].iter().map(PathBuf::from).find(|p| p.exists())?;
    let script = snippets.parent().unwrap().join(format!("check-{}.lua", std::process::id()));
    let list: Vec<String> = files.iter().map(|f| format!("{:?}", f.display().to_string())).collect();
    std::fs::write(
        &script,
        format!(
            "for _, p in ipairs({{ {} }}) do\n  local ok, err = require('snips').check(p)\n  io.stdout:write(ok and 'OK\\n' or ('ERR ' .. tostring(err):gsub('\\n', ' ') .. '\\n'))\nend\n",
            list.join(", ")
        ),
    )
    .unwrap();
    let mut child = Command::new(nvim)
        .args(["--headless", "-n", "-i", "NONE", "-c"])
        .arg(format!("luafile {}", script.display()))
        .args(["-c", "qa!"])
        .env("NVIM_APPNAME", "texnvim")
        .env("TEXNVIM_SNIPPETS", snippets)
        .env("TEXNVIM_NO_VIEWER", "1")
        .env("TEXNVIM_NO_COMPILE", "1")
        .env("TEXNVIM_NO_REFERENCE", "1")
        .env("TEXNVIM_NO_LSP", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let start = Instant::now();
    loop {
        if child.try_wait().ok().flatten().is_some() {
            break;
        }
        if start.elapsed() > Duration::from_secs(60) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&script);
            return Some(Err("nvim timed out".into()));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let out = child.wait_with_output().unwrap();
    let _ = std::fs::remove_file(&script);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let lines: Vec<&str> = stdout.lines().filter(|l| l.starts_with("OK") || l.starts_with("ERR")).collect();
    if lines.len() != files.len() {
        return Some(Err(format!("nvim said: {stdout} {}", String::from_utf8_lossy(&out.stderr))));
    }
    match lines.iter().find(|l| l.starts_with("ERR")) {
        Some(e) => Some(Err(e[4..].to_string())),
        None => Some(Ok(())),
    }
}

/// The check the studio runs: brackets, then the real loader when available.
fn checker(snippets: PathBuf, log: &mut Vec<String>) -> impl FnMut(&Path, &str) -> Result<(), String> + '_ {
    move |path: &Path, text: &str| {
        log.push(path.file_name().unwrap().to_string_lossy().into_owned());
        balanced(text)?;
        // the file on disk is the text (apply writes before it checks)
        assert_eq!(std::fs::read_to_string(path).unwrap(), text);
        match nvim_check(&snippets, &[path.to_path_buf()]) {
            Some(r) => r,
            None => Ok(()),
        }
    }
}

#[test]
fn plan_files_writes_nothing() {
    let t = temp("plan");
    let tex = t.paths.snippets.join("tex");
    let before = snapshot(&tex);
    let (files, outcome) = apply::plan_files(&t.paths, &plan()).unwrap();
    assert_eq!(snapshot(&tex), before);
    let names: Vec<String> = files.iter().map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["text.lua", "operations.lua", "probability.lua", "bare.lua", "inbox.lua"]);
    assert_eq!(outcome.moved.len(), 5);
    assert_eq!(outcome.backup, PathBuf::new());
}

#[test]
fn apply_moves_entries() {
    let t = temp("apply");
    let tex = t.paths.snippets.join("tex");
    let before = snapshot(&tex);
    let mut log = Vec::new();
    let outcome = {
        let mut check = checker(t.paths.snippets.clone(), &mut log);
        apply::apply(&t.paths, &plan(), NOW, &mut check).unwrap()
    };
    // every touched file was checked
    log.sort();
    assert_eq!(log, ["bare.lua", "inbox.lua", "operations.lua", "probability.lua", "text.lua"]);

    let moved: Vec<(&str, &str)> = outcome.moved.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    assert_eq!(
        moved,
        [("fun-1440", "text.lua"), ("sq-1441", "operations.lua"), ("pr-1450", "probability.lua"), ("ex-1451", "probability.lua"), ("st-1453", "bare.lua")]
    );
    let kept: Vec<&str> = outcome.kept.iter().map(|(a, _)| a.as_str()).collect();
    assert_eq!(kept, ["zz-1452", "no-such-id", "kp-1454"]);
    assert!(outcome.kept[0].1.contains("nowhere.lua does not exist"), "{:?}", outcome.kept);

    let read = |n: &str| std::fs::read_to_string(tex.join(n)).unwrap();
    let orig = |n: &str| String::from_utf8(before[n].clone()).unwrap();

    // text.lua: the multi-line entry, byte-exact, right after the multi-line `beg`
    let fun = "  text(\"fun\", [[\n\\begin{funfact}[<>]\n\t<>\n\\end{funfact}]], \"yours\", { tab = true, line = \"start\", ft = \"tex\", desc = \"Fun fact block\" }),\n";
    let text = read("text.lua");
    let beg_end = "\"castel\", { tab = true, line = \"start\", ft = \"tex\", desc = \"begin/end, name typed once\" }),\n";
    assert!(text.contains(&format!("{beg_end}{fun}  text(\"enum\"")), "{text}");
    assert_eq!(text.replacen(fun, "", 1), orig("text.lua"));
    assert!(!text.contains("-- studio:"));

    // operations.lua: vis added to the locals line, the entry before `})`
    let ops = read("operations.lua");
    assert!(ops.contains("\nlocal math, suffix, vis = h.math, h.suffix, h.vis\n"), "{ops}");
    assert!(ops.ends_with("  math(\"trace\",    [[\\mathrm{Tr}]],       \"default\"),\n  vis(\"Q\", [[\\sqrt[3]{ %V }]], \"yours\"),\n})\n"), "{ops}");

    // probability.lua: a new file, header + locals + group, both entries in order
    let prob = read("probability.lua");
    assert_eq!(
        prob,
        "-- Probability: snippets sorted here from the inbox by Ntex Studio.\n\
         --\n\
         -- One snippet per line, like the other files: math(\"ff\", [[\\frac{<>}{<>}]], \"yours\").\n\
         --   Full vocabulary: snippets/README.md. Save the file: it reloads.\n\
         local h = require(\"snips.helpers\")\n\
         local math = h.math\n\
         \n\
         return h.group(\"Probability\", {\n  \
         math(\"prb\", [[\\mathbb{P}(<>)]], \"yours\"),\n  \
         math(\"exx\", [[\\mathbb{E}[<>] ]], \"yours\"),\n\
         })\n"
    );

    // bare.lua: no locals line → h.math; the comma-less last entry got its comma
    let bare = read("bare.lua");
    assert_eq!(bare, BARE.replace("})\n", "  h.math(\"stay\", [[\\star]], \"yours\"),\n})\n"));

    // the inbox keeps the rest, markers and all
    let inbox = read("inbox.lua");
    let entries = ntex_studio_core::inbox::parse(&inbox);
    assert_eq!(entries.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["zz-1452", "kp-1454"]);
    assert!(inbox.contains("  -- studio: id=kp-1454 made=2026-10-07T14:54\n  math(\"kpp\", [[\\kappa]], \"yours\"),\n})"), "{inbox}");

    // untouched files are untouched; no temp files left
    let after = snapshot(&tex);
    for (n, b) in &before {
        if !["text.lua", "operations.lua", "bare.lua", "inbox.lua"].contains(&n.as_str()) {
            assert_eq!(&after[n], b, "{n}");
        }
    }
    assert_eq!(after.len(), before.len() + 1);

    // the backup: manifest + copies, the new file recorded as absent
    assert_eq!(outcome.backup, t.paths.state.join("backups").join(NOW));
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(outcome.backup.join("manifest.json")).unwrap()).unwrap();
    let files = manifest["files"].as_array().unwrap();
    assert_eq!(files.len(), 5);
    let prob_entry = files.iter().find(|f| f["path"].as_str().unwrap().ends_with("probability.lua")).unwrap();
    assert!(prob_entry["copy"].is_null());

    // the whole folder loads in a real headless ntex
    let all: Vec<PathBuf> = after.keys().filter(|n| n.ends_with(".lua")).map(|n| tex.join(n)).collect();
    if let Some(r) = nvim_check(&t.paths.snippets, &all) {
        r.unwrap();
    }
}

#[test]
fn undo_restores_everything() {
    let t = temp("undo");
    let tex = t.paths.snippets.join("tex");
    let before = snapshot(&tex);
    let outcome = apply::apply(&t.paths, &plan(), NOW, &mut |_: &Path, text: &str| balanced(text)).unwrap();
    assert_ne!(snapshot(&tex), before);
    apply::undo(&outcome.backup).unwrap();
    assert_eq!(snapshot(&tex), before);
    // a second apply at the same minute gets its own backup folder
    let o2 = apply::apply(&t.paths, &plan(), NOW, &mut |_: &Path, text: &str| balanced(text)).unwrap();
    assert_ne!(o2.backup, outcome.backup);
    apply::undo(&o2.backup).unwrap();
    assert_eq!(snapshot(&tex), before);
}

#[test]
fn failing_check_restores_byte_for_byte() {
    let t = temp("fail");
    let tex = t.paths.snippets.join("tex");
    let before = snapshot(&tex);
    let mut checked = Vec::new();
    let err = apply::apply(&t.paths, &plan(), NOW, &mut |p: &Path, text: &str| {
        checked.push(p.file_name().unwrap().to_string_lossy().into_owned());
        balanced(text)?;
        if p.ends_with("probability.lua") { Err("probability.lua:9: attempt to call a nil value".into()) } else { Ok(()) }
    })
    .unwrap_err();
    assert!(err.to_string().contains("probability.lua:9"), "{err}");
    assert!(checked.contains(&"probability.lua".to_string()));
    // everything as it was, the new file gone, no temp files
    assert_eq!(snapshot(&tex), before);
}

/// The real loader refuses an entry calling something the target can't see;
/// everything is restored.
#[test]
fn real_check_failure_restores() {
    let t = temp("realfail");
    let tex = t.paths.snippets.join("tex");
    if nvim_check(&t.paths.snippets, &[tex.join("greek.lua")]).is_none() {
        return;
    }
    let inbox = INBOX.replace("math(\"kpp\", [[\\kappa]], \"yours\"),", "wrap(\"kpp\", [[\\kappa]], \"yours\"),");
    std::fs::write(tex.join("inbox.lua"), &inbox).unwrap();
    let before = snapshot(&tex);
    let mut log = Vec::new();
    let res = {
        let mut check = checker(t.paths.snippets.clone(), &mut log);
        let p = SortPlan { moves: vec![mv("fun-1440", "text.lua", Some("beg"), None), mv("kp-1454", "greek.lua", None, None)], notes: String::new() };
        apply::apply(&t.paths, &p, NOW, &mut check)
    };
    let err = res.unwrap_err().to_string();
    assert!(err.contains("greek.lua"), "{err}");
    assert_eq!(snapshot(&tex), before);
}

#[test]
fn nothing_to_move() {
    let t = temp("none");
    let tex = t.paths.snippets.join("tex");
    let before = snapshot(&tex);
    let p = SortPlan { moves: vec![mv("zz-1452", "nowhere.lua", None, None)], notes: String::new() };
    let o = apply::apply(&t.paths, &p, NOW, &mut |_: &Path, _: &str| panic!("nothing to check")).unwrap();
    assert!(o.moved.is_empty());
    assert_eq!(o.kept.len(), 7);
    assert_eq!(snapshot(&tex), before);
    apply::undo(&o.backup).unwrap();
    assert_eq!(snapshot(&tex), before);
}

#[test]
fn backup_and_undo_of_any_files() {
    let t = temp("backup");
    let a = t.root.join("a.txt");
    let missing = t.root.join("later.txt");
    std::fs::write(&a, b"one\xff\n").unwrap();
    let dir = apply::backup(&t.paths, &[a.clone(), missing.clone(), a.clone()], "2026-10-07T15:01").unwrap();
    assert!(dir.starts_with(t.paths.state.join("backups")));
    std::fs::write(&a, "two").unwrap();
    std::fs::write(&missing, "new").unwrap();
    apply::undo(&dir).unwrap();
    assert_eq!(std::fs::read(&a).unwrap(), b"one\xff\n");
    assert!(!missing.exists());
}
