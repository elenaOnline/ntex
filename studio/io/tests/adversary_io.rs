//! ADVERSARY: independent acceptance for ntex-studio-io (bridge, renders).
//! Expected values: ../verification/expected.json, measured in a real headless
//! ntex by verification/try.lua (see verification/acceptance.md).
//! Headless only; temp copies only; no viewer. Run:
//!   cargo test -p ntex-studio-io --test adversary_io -- --test-threads=1

use ntex_studio_core::codegen;
use ntex_studio_core::model::*;
use ntex_studio_io::bridge::{self, Bridge};
use ntex_studio_io::render::{RenderJob, Renderer};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

fn studio() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}
fn expected() -> &'static Value {
    static E: OnceLock<Value> = OnceLock::new();
    E.get_or_init(|| serde_json::from_str(&fs::read_to_string(studio().join("verification/expected.json")).unwrap()).unwrap())
}
fn tmpdir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "ntex-adversary-io-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}
fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let t = to.join(e.file_name());
        if e.path().is_dir() { copy_dir(&e.path(), &t) } else { fs::copy(e.path(), &t).unwrap(); }
    }
}
fn temp_paths(tag: &str) -> Paths {
    let root = tmpdir(tag);
    let real = studio().parent().unwrap().to_path_buf();
    let config = root.join("texnvim");
    copy_dir(&real.join("snippets"), &config.join("snippets"));
    copy_dir(&real.join("templates/tex"), &config.join("templates/tex"));
    let _ = fs::remove_file(config.join("snippets/tex/inbox.lua"));
    let state = root.join("state");
    fs::create_dir_all(&state).unwrap();
    Paths { templates_tex: config.join("templates/tex"), snippets: config.join("snippets"), config, state }
}
fn report(test: &str, fails: &[String], total: usize) {
    let dir = studio().join("verification/results");
    let _ = fs::create_dir_all(&dir);
    let mut s = format!("{test}: {} of {total} checks failed\n\n", fails.len());
    for f in fails {
        s.push_str(f);
        s.push_str("\n\n");
    }
    let _ = fs::write(dir.join(format!("{test}.txt")), &s);
    if !fails.is_empty() {
        panic!("{s}");
    }
}
fn recv<T>(rx: std::sync::mpsc::Receiver<T>, secs: u64) -> Option<T> {
    rx.recv_timeout(Duration::from_secs(secs)).ok()
}
fn started_bridge(paths: &Paths) -> Bridge {
    let b = Bridge::start(paths).expect("Bridge::start");
    assert!(b.wait_ready(Duration::from_secs(120)), "bridge not ready in 120 s");
    b
}
/// pids in a process group
fn group(pgid: u32) -> Vec<String> {
    let o = Command::new("pgrep").arg("-g").arg(pgid.to_string()).output().unwrap();
    String::from_utf8_lossy(&o.stdout).split_whitespace().map(String::from).collect()
}
fn descendants(pid: u32) -> Vec<u32> {
    let mut out = vec![];
    let mut stack = vec![pid];
    while let Some(p) = stack.pop() {
        let o = Command::new("pgrep").arg("-P").arg(p.to_string()).output().unwrap();
        for c in String::from_utf8_lossy(&o.stdout).split_whitespace() {
            if let Ok(c) = c.parse::<u32>() {
                out.push(c);
                stack.push(c);
            }
        }
    }
    out
}
fn alive(pid: u32) -> bool {
    Command::new("kill").arg("-0").arg(pid.to_string()).status().map(|s| s.success()).unwrap_or(false)
}

/// Strip what's outside the interesting line(s): $…$, \[…\], surrounding blank lines.
fn norm(s: &str) -> String {
    let mut t = s.trim().to_string();
    if let Some(x) = t.strip_prefix("\\[\n").and_then(|x| x.strip_suffix("\n\\]")) { t = x.to_string(); }
    if let Some(x) = t.strip_prefix("$$\n").and_then(|x| x.strip_suffix("\n$$")) { t = x.to_string(); }
    if t.len() >= 2 && t.starts_with('$') && t.ends_with('$') && !t.starts_with("$$") { t = t[1..t.len() - 1].to_string(); }
    t.trim().to_string()
}

/// Turn one of my typings into a TryRequest, when its starting buffer is one of
/// the bridge's places.
fn request(entry: &str, ft: &str, text: &str, select: &Value, steps: &[Value]) -> Option<TryRequest> {
    let place = if !select.is_null() {
        "visual"
    } else {
        match text {
            "$‸$" => "inline",
            "\\[\n‸\n\\]" | "$$\n‸\n$$" => "display",
            "‸" => "text-start",
            "- ‸" => "list-item",
            _ => return None,
        }
    };
    let first = steps[0]["keys"].as_str()?;
    let (keys, tab) = match first.strip_suffix("<Tab>") { Some(k) => (k.to_string(), true), None => (first.to_string(), false) };
    let mut fills = vec![];
    let mut pending = String::new();
    for s in &steps[1..] {
        let k = s["keys"].as_str()?;
        match k.strip_suffix("<Tab>") {
            Some(f) => { pending.push_str(f); fills.push(std::mem::take(&mut pending)); }
            None => pending.push_str(k),
        }
    }
    if !pending.is_empty() { return None; }
    Some(TryRequest { entry: entry.to_string(), ft: if ft == "md" { "markdown".into() } else { "tex".into() }, place: place.into(), keys, tab, fills })
}

#[test]
fn bridge_try_matches_measured_expansions() {
    let paths = temp_paths("try");
    let b = started_bridge(&paths);
    let mut fails = vec![];
    let mut n = 0;
    let mut skipped = vec![];
    for c in expected()["snippets"].as_array().unwrap() {
        let id = c["id"].as_str().unwrap();
        let d: Draft = serde_json::from_value(c["draft"].clone()).unwrap();
        let entry = codegen::entry(&d);
        for (k, t) in c["tries"].as_array().unwrap().iter().enumerate() {
            let steps = t["steps"].as_array().unwrap();
            let Some(req) = request(&entry, t["ft"].as_str().unwrap(), t["text"].as_str().unwrap(), &t["select"], steps) else {
                skipped.push(format!("{id}#{k}"));
                continue;
            };
            // the bridge's visual place selects "x+y"; my measurement selected "a+b"
            let visual = req.place == "visual";
            let req_place_list = req.place == "list-item";
            let started = Instant::now();
            let r = recv(b.try_draft(req.clone()), 60);
            n += 1;
            let label = format!("{id}#{k} ({})", c["title"].as_str().unwrap());
            match r {
                None => { fails.push(format!("{label}: try_draft gave no answer in 60 s")); continue; }
                Some(Err(e)) => { fails.push(format!("{label}: try_draft Err({e})\n  request: {req:?}")); continue; }
                Some(Ok(res)) => {
                    let exp_files: Vec<&str> = steps[0]["expanded"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
                    let want_exp = !exp_files.is_empty();
                    let other_file = exp_files.iter().find(|f| **f != "zz_adv.lua").copied();
                    let other_ok = match other_file {
                        Some(f) => res.expanded_other.as_deref().map(|o| o.contains(f)).unwrap_or(false),
                        None => res.expanded_other.is_none(),
                    };
                    if res.expanded != want_exp || !other_ok {
                        fails.push(format!("{label}: expanded={} other={:?}; measured: {}", res.expanded, res.expanded_other,
                            match (want_exp, other_file) { (false, _) => "nothing expanded".to_string(), (true, None) => "the draft expanded".to_string(), (true, Some(f)) => format!("{f} expanded") }));
                    }
                    // a typed step without Tab is folded into the next fill: keep the expectations of the first step and of Tab steps
                    let steps: Vec<&Value> = steps.iter().enumerate().filter(|(i, s)| *i == 0 || s["keys"].as_str().unwrap().ends_with("<Tab>")).map(|(_, s)| s).collect();
                    if res.steps.len() != steps.len() {
                        fails.push(format!("{label}: {} steps, expected {}: {:?}", res.steps.len(), steps.len(), res.steps));
                        continue;
                    }
                    for (s, w) in res.steps.iter().zip(steps.iter().copied()) {
                        n += 1;
                        let mut want = w["buffer"].as_str().unwrap().to_string();
                        if visual { want = want.replace("a+b", "x+y"); }
                        let mut got = s.buffer.clone();
                        if w["mode"] != json!("i") {
                            // select mode: compare the text only
                            want = want.replace('‸', "");
                            got = got.replace('‸', "");
                        }
                        let item = req_place_list && norm(&want).ends_with(&norm(&got)) && !norm(&got).is_empty();
                        if norm(&got) != norm(&want) && !item {
                            fails.push(format!("{label} step {:?}: bridge {:?}, measured {:?}{}", w["keys"], s.buffer, w["buffer"],
                                w.get("measured_today").map(|m| format!(" (today's ntex: {m})")).unwrap_or_default()));
                        }
                    }
                    if started.elapsed() > Duration::from_secs(5) {
                        fails.push(format!("{label}: try took {:?} (the Example box should feel live)", started.elapsed()));
                    }
                }
            }
        }
    }
    eprintln!("skipped (start text is not a bridge place): {skipped:?}");
    // conflicts: the bridge must name the other snippet
    for c in expected()["conflicts"].as_array().unwrap() {
        let id = c["id"].as_str().unwrap();
        let d: Draft = serde_json::from_value(c["draft"].clone()).unwrap();
        let entry = codegen::entry(&d);
        let t = &c["tries"][0];
        let steps = t["with"].as_array().unwrap();
        let Some(req) = request(&entry, t["ft"].as_str().unwrap(), t["text"].as_str().unwrap(), &t["select"], steps) else { continue };
        let exp: Vec<&str> = steps[0]["expanded"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
        let other = exp.iter().find(|f| **f != "zz_adv.lua").copied();
        n += 1;
        match recv(b.try_draft(req), 60) {
            Some(Ok(res)) => {
                if let Some(f) = other {
                    if !res.expanded_other.as_deref().map(|o| o.contains(f)).unwrap_or(false) {
                        fails.push(format!("{id} ({}): measured: {f} expands instead; bridge says expanded={} other={:?} steps={:?}",
                            c["title"].as_str().unwrap(), res.expanded, res.expanded_other, res.steps));
                    }
                } else if exp.is_empty() && res.expanded {
                    fails.push(format!("{id}: measured nothing expands; bridge says expanded ({:?})", res.steps));
                }
            }
            other => fails.push(format!("{id}: try_draft failed: {other:?}")),
        }
    }
    drop(b);
    report("bridge_try_matches_measured_expansions", &fails, n);
}

#[test]
fn bridge_survives_a_poisoning_try_and_a_crash() {
    let paths = temp_paths("poison");
    let b = started_bridge(&paths);
    let mut fails = vec![];
    let ff = TryRequest { entry: "  math(\"ff\", [[\\frac{<>}{<>}]], \"yours\"),".into(), ft: "tex".into(), place: "inline".into(), keys: "ff".into(), tab: false, fills: vec!["a".into(), "b".into()] };
    // a body ending in a newline: LuaSnip errors mid-expansion (measured), which
    // corrupts every later expansion in that Neovim
    let nl = TryRequest { entry: "  math(\"nlz\", [[\\quad\n]], \"yours\"),".into(), ft: "tex".into(), place: "display".into(), keys: "nlz".into(), tab: false, fills: vec![] };
    let r1 = recv(b.try_draft(nl), 60);
    let r2 = recv(b.try_draft(ff.clone()), 60);
    match &r2 {
        Some(Ok(r)) if r.expanded && norm(&r.steps[0].buffer) == "\\frac{‸}{}" => {}
        other => fails.push(format!("after a try whose snippet raised a LuaSnip error ({r1:?}), the next try of ff gives {other:?} (expected \\frac{{‸}}{{}}): the bridge keeps a poisoned Neovim")),
    }
    let starts = b.starts();
    b.kill_for_test();
    // one request may be lost with the crash; the next must work
    let first = recv(b.try_draft(ff.clone()), 120);
    std::thread::sleep(Duration::from_secs(3));
    match recv(b.try_draft(ff), 120) {
        Some(Ok(r)) if r.expanded => {}
        other => fails.push(format!("after kill_for_test, the first try gives {first:?} and a second (3 s later) gives {other:?}: the server is not restarted")),
    }
    if b.starts() <= starts {
        fails.push(format!("starts() did not go up after a crash ({} → {})", starts, b.starts()));
    }
    report("bridge_survives_a_poisoning_try_and_a_crash", &fails, 3);
}

#[test]
fn bridge_list_and_check_file() {
    let paths = temp_paths("list");
    let b = started_bridge(&paths);
    let mut fails = vec![];
    let list = recv(b.list(), 60);
    let known: Vec<KnownSnippet> = serde_json::from_value(expected()["known"].clone()).unwrap();
    match list {
        Some(Ok(l)) => {
            if l.len() != known.len() {
                fails.push(format!("list: {} snippets, measured {} (require(\"snips\").list() on the same files)", l.len(), known.len()));
            }
            let hat = l.iter().find(|k| k.kind == "post" && (k.trigger == "hat" || k.pattern.as_deref() == Some("(%a)hat")));
            if hat.map(|h| h.file != "accents.lua" || h.how != "auto" || h.where_ != "math").unwrap_or(true) {
                fails.push(format!("list: post hat from accents.lua not reported as such: {hat:?}"));
            }
            let beg = l.iter().find(|k| k.trigger == "beg" && k.kind == "text");
            if beg.map(|b| b.how != "tab" || b.line_rule.as_deref() != Some("start") || b.ft.as_deref() != Some("tex")).unwrap_or(true) {
                fails.push(format!("list: text beg should be how=tab, line_rule=start, ft=tex: {beg:?}"));
            }
        }
        other => fails.push(format!("list failed: {other:?}")),
    }
    let good = format!("{}", ntex_studio_core::inbox::empty_file());
    match recv(b.check_file("inbox.lua", &good), 60) {
        Some(Ok(())) => {}
        other => fails.push(format!("check_file(empty inbox) = {other:?}")),
    }
    let bad = "local h = require(\"snips.helpers\")\nreturn h.group(\"x\", {\n  h.math(\"a\", [[\\alpha]] \"yours\"),\n})\n";
    match recv(b.check_file("inbox.lua", bad), 60) {
        Some(Err(e)) if e.contains("inbox.lua") && e.contains(":3") => {}
        other => fails.push(format!("check_file(syntax error on line 3) = {other:?} (expected Err naming inbox.lua:3)")),
    }
    let runtime = "local h = require(\"snips.helpers\")\nreturn h.group(\"x\", {\n  post(\"bbm\", [[\\mathbb{%1}]], \"yours\"),\n})\n";
    match recv(b.check_file("operations.lua", runtime), 60) {
        Some(Err(_)) => {}
        other => fails.push(format!("check_file(calls an unbound helper `post`) = {other:?} (expected Err)")),
    }
    match bridge::check_file_once(&paths, "inbox.lua", bad) {
        Err(e) if e.contains(":3") => {}
        other => fails.push(format!("check_file_once(bad) = {other:?}")),
    }
    report("bridge_list_and_check_file", &fails, 6);
}

#[test]
fn bridge_leaves_no_orphans() {
    let paths = temp_paths("orph");
    let mut fails = vec![];
    let b = started_bridge(&paths);
    let pid = b.pid().expect("pid");
    let private = b.private_dir();
    // make it do some work first
    let _ = recv(b.try_draft(TryRequest { entry: "  math(\"ff\", [[\\frac{<>}{<>}]], \"yours\"),".into(), ft: "tex".into(), place: "inline".into(), keys: "ff".into(), tab: false, fills: vec![] }), 60);
    let tree = descendants(pid);
    let names: Vec<String> = tree.iter().map(|p| {
        let o = Command::new("ps").args(["-o", "command=", "-p", &p.to_string()]).output().unwrap();
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    }).collect();
    for (p, nm) in tree.iter().zip(&names) {
        if nm.contains("harper") || nm.contains("copilot") || nm.contains("texlab") {
            fails.push(format!("the bridge started a language server: {p} {nm}"));
        }
    }
    drop(b);
    std::thread::sleep(Duration::from_secs(2));
    if alive(pid) {
        fails.push(format!("server {pid} still alive after Drop"));
    }
    for (p, nm) in tree.iter().zip(&names) {
        if alive(*p) {
            fails.push(format!("orphan after Drop: {p} {nm}"));
        }
    }
    let g = group(pid);
    if !g.is_empty() {
        fails.push(format!("process group {pid} still has {g:?}"));
    }
    if let Some(d) = private {
        if d.exists() {
            fails.push(format!("private folder left behind: {}", d.display()));
        }
    }
    report("bridge_leaves_no_orphans", &fails, 4);
}

#[test]
fn render_good_and_broken() {
    let paths = temp_paths("render");
    let cache = paths.state.join("render-cache");
    let r = Renderer::with_dirs(&paths.templates_tex, &cache);
    let mut fails = vec![];
    let t0 = Instant::now();
    match recv(r.render(RenderJob::Snippet { latex: "\\frac{\\color{blue}\\square}{2}".into(), math: true, display: false }), 120) {
        Some(Ok(x)) => {
            if !x.png.exists() || x.width <= 0.0 || x.height <= 0.0 {
                fails.push(format!("good render: {x:?}"));
            }
            if !x.png.starts_with(&cache) {
                fails.push(format!("PNG not in the cache dir: {}", x.png.display()));
            }
        }
        other => fails.push(format!("good render failed: {other:?} after {:?}", t0.elapsed())),
    }
    let t1 = Instant::now();
    match recv(r.render(RenderJob::Snippet { latex: "\\frac{".into(), math: true, display: false }), 60) {
        Some(Err(e)) => {
            if e.contains('\n') {
                fails.push(format!("broken render error is not one line: {e:?}"));
            }
        }
        Some(Ok(x)) => fails.push(format!("broken LaTeX rendered Ok: {x:?}")),
        None => fails.push(format!("broken render: no answer within 60 s (hang) after {:?}", t1.elapsed())),
    }
    std::thread::sleep(Duration::from_millis(500));
    // no tectonic of ours left (match on our temp dirs only: the author's own compiles may be running)
    let o = Command::new("pgrep").args(["-fl", "tectonic"]).output().unwrap();
    for l in String::from_utf8_lossy(&o.stdout).lines() {
        if l.contains(paths.state.to_string_lossy().as_ref()) || l.contains(paths.config.to_string_lossy().as_ref()) {
            fails.push(format!("tectonic left running: {l}"));
        }
    }
    report("render_good_and_broken", &fails, 4);
}
