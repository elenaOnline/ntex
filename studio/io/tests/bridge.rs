//! The headless ntex bridge against a temp copy of the snippets folder.
//! Everything runs in one test so the server starts once (and its timings
//! are measured once); each part prints what it checked.
mod common;
use common::*;
use ntex_studio_core::model::*;
use ntex_studio_io::bridge::{Bridge, check_file_once};
use std::time::{Duration, Instant};

const T: Duration = Duration::from_secs(60);

fn try_req(entry: &str, place: &str, keys: &str, tab: bool, fills: &[&str]) -> TryRequest {
    TryRequest { entry: entry.into(), ft: "tex".into(), place: place.into(), keys: keys.into(), tab, fills: fills.iter().map(|s| s.to_string()).collect() }
}

fn lsp_processes() -> Vec<u32> {
    let out = std::process::Command::new("pgrep").args(["-f", "harper-ls|copilot-language-server|texlab"]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| l.trim().parse().ok()).collect()
}

#[test]
fn bridge_end_to_end() {
    let t = TempDir::new("bridge");
    let paths = temp_paths(&t);
    let lsp_before = lsp_processes();

    let t0 = Instant::now();
    let mut b = Bridge::start(&paths).expect("start");
    assert!(b.wait_ready(T), "the bridge never got ready");
    let ready_wall = t0.elapsed();
    println!("bridge ready: {:.0} ms (server-reported {:.0} ms)", ready_wall.as_secs_f64() * 1000.0, b.ready_ms().unwrap());
    let pid = b.pid().unwrap();
    let dir = b.private_dir().unwrap();
    assert!(dir.join("snippets/tex/sets.lua").exists(), "the private copy holds the snippets");

    // server + child nvim, and nothing else in their process groups (no
    // language servers); none new anywhere either (checked at the end)
    let pids = b.pids();
    assert_eq!(pids.len(), 2, "server + child nvim: {pids:?}");
    let members: Vec<(u32, String)> = pids.iter().flat_map(|p| group_members(*p)).collect();
    println!("bridge processes: {members:?}");
    assert!(members.iter().all(|(_, c)| c.contains("nvim")), "only nvim: {members:?}");
    assert_eq!(members.len(), 2, "{members:?}");

    // list
    let list = b.list().recv_timeout(T).unwrap().expect("list");
    let uu = list.iter().find(|s| s.trigger == "uu").expect("uu is loaded");
    assert_eq!(uu.file, "sets.lua");
    assert_eq!(uu.body.as_deref(), Some("\\cup"));
    let hat = list.iter().find(|s| s.trigger == "hat" && s.kind == "post").expect("post hat");
    assert_eq!(hat.pattern.as_deref(), Some("(%a)hat"));
    println!("list: {} snippets, uu → {:?} in {}", list.len(), uu.body, uu.file);

    // check: a syntax error, with its line
    let bad = "local h = require(\"snips.helpers\")\nreturn h.group(\"Inbox\", {\n  h.math(\"zz\", [[\\zeta]] \"yours\"),\n})\n";
    let err = b.check_file("inbox.lua", bad).recv_timeout(T).unwrap().expect_err("syntax error caught");
    println!("check (bad): {err}");
    assert!(err.starts_with("inbox.lua:3:"), "{err}");
    let good = "local h = require(\"snips.helpers\")\nreturn h.group(\"Inbox\", {\n  h.math(\"zz\", [[\\zeta]], \"yours\"),\n})\n";
    b.check_file("inbox.lua", good).recv_timeout(T).unwrap().expect("good file passes");
    let notlist = "return 42\n";
    let err = b.check_file("inbox.lua", notlist).recv_timeout(T).unwrap().expect_err("must return a list");
    assert!(err.starts_with("inbox.lua"), "{err}");
    let err = b.check_file("inbox.lua", "local h = require(\"snips.helpers\")\nreturn h.group(\"x\", {\n  h.nope(\"zz\", [[a]]),\n})\n").recv_timeout(T).unwrap().expect_err("runtime error caught");
    assert!(err.starts_with("inbox.lua:3:"), "{err}");

    // try: ff → \frac{…}{…}, then a, b
    let ff = try_req(r#"  math("ff", [[\frac{<>}{<>}]], "yours"),"#, "inline", "ff", false, &["a", "b"]);
    let r = b.try_draft(ff.clone()).recv_timeout(T).unwrap().expect("try ff");
    println!("try ff: {r:?}");
    assert!(r.expanded);
    assert_eq!(r.expanded_other, None);
    assert_eq!(r.expanded_file.as_deref(), Some("zz_studio_try.lua"));
    let bufs: Vec<(&str, &str)> = r.steps.iter().map(|s| (s.keys.as_str(), s.buffer.as_str())).collect();
    assert_eq!(bufs, vec![("ff", "\\frac{‸}{}"), ("a ⇥", "\\frac{a}{‸}"), ("b ⇥", "\\frac{a}{b}‸")]);

    // warm timing
    let mut times = Vec::new();
    for _ in 0..10 {
        let t1 = Instant::now();
        let r = b.try_draft(ff.clone()).recv_timeout(T).unwrap().unwrap();
        times.push(t1.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(r.steps.len(), 3);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!("try ff warm: median {:.1} ms, max {:.1} ms", times[times.len() / 2], times[times.len() - 1]);

    // the draft is gone afterwards: ff doesn't exist in her snippets
    let list2 = b.list().recv_timeout(T).unwrap().unwrap();
    assert!(!list2.iter().any(|s| s.file == "zz_studio_try.lua"), "the try file was unloaded");
    assert_eq!(list2.len(), list.len());

    // a Tab text snippet at a line start
    let env = try_req(r#"  text("zfoo", [[\begin{foo}<>\end{foo}]], "yours", { tab = true, line = "start" }),"#, "text-start", "zfoo", true, &["x"]);
    let r = b.try_draft(env).recv_timeout(T).unwrap().expect("try text tab");
    println!("try text tab: {r:?}");
    assert!(r.expanded);
    assert_eq!(r.steps[0].keys, "zfoo ⇥");
    assert_eq!(r.steps[0].buffer, "\\begin{foo}‸\\end{foo}");
    assert_eq!(r.steps[1].buffer, "\\begin{foo}x\\end{foo}‸");
    // …and not after text: nothing expands, with a reason
    let env2 = try_req(r#"  text("zfoo", [[\begin{foo}<>\end{foo}]], "yours", { tab = true, line = "start" }),"#, "text", "zfoo", true, &[]);
    let r = b.try_draft(env2).recv_timeout(T).unwrap().unwrap();
    println!("try text tab after text: {r:?}");
    assert!(!r.expanded);
    assert_eq!(r.reason.as_deref(), Some("it only fires at the start of a line"));

    // a maths snippet tried in text: nothing, because of the place
    let r = b.try_draft(try_req(r#"  math("ff", [[\frac{<>}{<>}]], "yours"),"#, "text", "ff", false, &[])).recv_timeout(T).unwrap().unwrap();
    assert!(!r.expanded);
    assert_eq!(r.reason.as_deref(), Some("the cursor wasn't in maths"));

    // a visual one
    let vis = try_req(r#"  vis("Q", [[\overline{ %V }]], "yours"),"#, "visual", "Q", false, &[]);
    let r = b.try_draft(vis).recv_timeout(T).unwrap().expect("try visual");
    println!("try visual: {r:?}");
    assert!(r.expanded);
    assert_eq!(r.steps[0].selected.as_deref(), Some("\\overline{ x+y }"));
    assert_eq!(r.steps[0].buffer, "‸\\overline{ x+y }");

    // a collision: her uu (\cup) wins over a lower-priority draft uu
    let col = try_req(r#"  math("uu", [[\uplus]], "yours", { priority = -1 }),"#, "inline", "uu", false, &[]);
    let r = b.try_draft(col).recv_timeout(T).unwrap().unwrap();
    println!("try colliding uu: {r:?}");
    assert!(r.expanded);
    assert_eq!(r.expanded_other.as_deref(), Some("uu from sets.lua"));
    assert_eq!(r.steps[0].buffer, "\\cup‸");
    // with priority 1 the draft wins
    let col = try_req(r#"  math("uu", [[\uplus]], "yours", { priority = 1 }),"#, "inline", "uu", false, &[]);
    let r = b.try_draft(col).recv_timeout(T).unwrap().unwrap();
    assert_eq!(r.expanded_other, None);
    assert_eq!(r.steps[0].buffer, "\\uplus‸");
    // a longer draft trigger shadowed while typing: uux never fires, uu does
    let r = b.try_draft(try_req(r#"  math("uux", [[\biguplus]], "yours"),"#, "inline", "uux", false, &[])).recv_timeout(T).unwrap().unwrap();
    assert_eq!(r.expanded_other.as_deref(), Some("uu from sets.lua"));

    // a broken entry: an error, not a crash
    let r = b.try_draft(try_req(r#"  math("ff", [[\frac]] "yours"),"#, "inline", "ff", false, &[])).recv_timeout(T).unwrap().unwrap();
    assert!(!r.expanded);
    assert!(r.error.as_deref().unwrap_or("").starts_with("the entry doesn't load"), "{r:?}");

    // refresh: a new snippet in her (temp) inbox shows up in list
    std::fs::write(
        paths.snippets.join("tex/inbox.lua"),
        "local h = require(\"snips.helpers\")\nlocal math = h.math\nreturn h.group(\"Inbox\", {\n  math(\"zzq\", [[\\quux]], \"yours\"),\n})\n",
    )
    .unwrap();
    let changed = b.refresh_done().recv_timeout(T).unwrap().unwrap();
    assert!(changed.contains(&"inbox.lua".to_string()), "{changed:?}");
    let list3 = b.list().recv_timeout(T).unwrap().unwrap();
    assert!(list3.iter().any(|s| s.trigger == "zzq" && s.file == "inbox.lua"));
    // and a removed file's snippets go away
    std::fs::remove_file(paths.snippets.join("tex/inbox.lua")).unwrap();
    b.refresh_done().recv_timeout(T).unwrap().unwrap();
    let list4 = b.list().recv_timeout(T).unwrap().unwrap();
    assert!(!list4.iter().any(|s| s.trigger == "zzq"));

    // restart after a crash
    b.kill_for_test();
    let t2 = Instant::now();
    while b.starts() < 2 && t2.elapsed() < T {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(b.wait_ready(T), "restarted");
    assert!(!alive(pid), "the old server is gone");
    std::thread::sleep(Duration::from_millis(300));
    for p in &pids {
        assert!(!alive(*p), "{p} of the killed server is gone");
    }
    assert!(!dir.exists(), "the old private folder is gone");
    let list5 = b.list().recv_timeout(T).unwrap().expect("list after restart");
    // inbox.lua was deleted above: everything else is back
    assert_eq!(list5.len(), list.iter().filter(|s| s.file != "inbox.lua").count());
    println!("restarted after a kill in {:.0} ms", t2.elapsed().as_secs_f64() * 1000.0);

    // stop: the whole tree and the folder go
    let dir2 = b.private_dir().unwrap();
    let members: Vec<(u32, String)> = b.pids().iter().flat_map(|p| group_members(*p)).collect();
    assert_eq!(members.len(), 2);
    b.stop();
    std::thread::sleep(Duration::from_millis(100));
    for (p, c) in &members {
        assert!(!alive(*p), "{p} ({c}) still running after stop");
    }
    assert!(!dir2.exists(), "private folder removed");
    assert!(b.list().recv_timeout(T).unwrap().is_err(), "a stopped bridge answers with an error");

    // no language server appeared during all this
    let lsp_after = lsp_processes();
    let new: Vec<_> = lsp_after.iter().filter(|p| !lsp_before.contains(p)).collect();
    assert!(new.is_empty(), "language servers started: {new:?}");

    // the one-shot check
    let t3 = Instant::now();
    let err = check_file_once(&paths, "inbox.lua", bad).expect_err("one-shot check");
    assert!(err.starts_with("inbox.lua:3:"), "{err}");
    println!("check_file_once: {:.0} ms", t3.elapsed().as_secs_f64() * 1000.0);
}

#[test]
fn drop_kills_everything() {
    let t = TempDir::new("bridge-drop");
    let paths = temp_paths(&t);
    let b = Bridge::start(&paths).unwrap();
    assert!(b.wait_ready(T));
    let dir = b.private_dir().unwrap();
    let members: Vec<(u32, String)> = b.pids().iter().flat_map(|p| group_members(*p)).collect();
    assert_eq!(members.len(), 2);
    // a request in flight while it is dropped
    let rx = b.list();
    drop(b);
    std::thread::sleep(Duration::from_millis(100));
    for (p, c) in &members {
        assert!(!alive(*p), "{p} ({c}) survived Drop");
    }
    assert!(!dir.exists());
    let _ = rx.recv_timeout(Duration::from_secs(5));
}
