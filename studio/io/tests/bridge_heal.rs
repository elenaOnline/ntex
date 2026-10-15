//! The bridge heals itself: after a Lua error in its child nvim (LuaSnip
//! throwing while a draft is typed) it restarts before the next request, and
//! a request in flight when the server dies is retried, once, on the next
//! one. Temp copies only; headless; every process it starts is checked gone.
mod common;
use common::*;
use ntex_studio_core::model::*;
use ntex_studio_io::bridge::Bridge;
use std::time::{Duration, Instant};

const T: Duration = Duration::from_secs(60);

fn try_req(entry: &str, place: &str, keys: &str, fills: &[&str]) -> TryRequest {
    TryRequest { entry: entry.into(), ft: "tex".into(), place: place.into(), keys: keys.into(), tab: false, fills: fills.iter().map(|s| s.to_string()).collect() }
}

fn ff() -> TryRequest {
    try_req(r#"  math("ff", [[\frac{<>}{<>}]], "yours"),"#, "inline", "ff", &["a", "b"])
}

fn assert_ff(r: &TryResult, what: &str) {
    let bufs: Vec<&str> = r.steps.iter().map(|s| s.buffer.as_str()).collect();
    assert_eq!(bufs, vec!["\\frac{‸}{}", "\\frac{a}{‸}", "\\frac{a}{b}‸"], "{what}: {r:?}");
    assert!(r.expanded && r.error.is_none(), "{what}: {r:?}");
}

fn wait_for(timeout: Duration, mut f: impl FnMut() -> bool) -> bool {
    let t = Instant::now();
    while t.elapsed() < timeout {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    f()
}

/// The server, its child nvim, and everything in their process groups.
fn tree(b: &Bridge) -> Vec<(u32, String)> {
    let pids = b.pids();
    assert_eq!(pids.len(), 2, "server + child nvim: {pids:?}");
    let mut out: Vec<(u32, String)> = pids.iter().flat_map(|p| group_members(*p)).collect();
    for p in pids {
        if !out.iter().any(|(q, _)| *q == p) {
            out.push((p, "?".into()));
        }
    }
    out
}

/// No orphans: every process of an old server is gone (within 3 s).
fn assert_gone(procs: &[(u32, String)], what: &str) {
    let gone = wait_for(Duration::from_secs(3), || procs.iter().all(|(p, _)| !alive(*p)));
    let left: Vec<_> = procs.iter().filter(|(p, _)| alive(*p)).collect();
    assert!(gone, "{what}: still running {left:?}");
    for (p, _) in procs {
        let g = group_members(*p);
        assert!(g.is_empty(), "{what}: process group {p} still has {g:?}");
    }
}

#[test]
fn self_heal_after_a_luasnip_error() {
    let t = TempDir::new("heal");
    let paths = temp_paths(&t);
    let mut b = Bridge::start(&paths).expect("start");
    assert!(b.wait_ready(T), "the bridge never got ready");
    let mut seen = tree(&b);
    let old = seen.clone();
    let old_dir = b.private_dir().unwrap();
    let starts = b.starts();

    // a draft whose condition throws: the error is raised inside LuaSnip's
    // expansion (helpers.lua calls `check` from the snippet's condition)
    let zqe = try_req(r#"  math("zqe", [[\zeta]], "yours", { check = function(line, trigger, captures) error("studio self-heal test") end }),"#, "inline", "zqe", &[]);
    let t0 = Instant::now();
    let r = b.try_draft(zqe).recv_timeout(T).unwrap().expect("the try still answers");
    let answered = t0.elapsed();
    println!("try zqe (throws): {r:?}");
    let err = r.error.as_deref().unwrap_or("");
    assert!(err.contains("studio self-heal test"), "the error is reported: {r:?}");
    assert!(!r.steps.is_empty(), "the steps recorded are kept: {r:?}");

    // restarted before the next request: one more start, ready again
    assert!(wait_for(T, || b.starts() == starts + 1 && b.is_ready()), "restarted after the error (starts {} → {})", starts, b.starts());
    println!(
        "self-heal: answered in {:.0} ms, fresh server ready {:.0} ms later (server-reported start {:.0} ms)",
        answered.as_secs_f64() * 1000.0,
        (t0.elapsed() - answered).as_secs_f64() * 1000.0,
        b.ready_ms().unwrap_or(-1.0)
    );
    assert_gone(&old, "the server that saw the error");
    assert!(!old_dir.exists(), "its private folder is gone");
    seen.extend(tree(&b));

    // the next try is right
    let r = b.try_draft(ff()).recv_timeout(T).unwrap().expect("try ff after the self-heal");
    assert_ff(&r, "ff after the self-heal");
    assert_eq!(b.starts(), starts + 1, "no restart after a clean try");

    // a request sent right behind the throwing one, before the restart: it is
    // handled by the fresh server, not lost
    let starts = b.starts();
    let zqe2 = try_req(r#"  math("zqe", [[\zeta]], "yours", { check = function() error("studio self-heal test") end }),"#, "inline", "zqe", &[]);
    let t1 = Instant::now();
    let rx1 = b.try_draft(zqe2);
    let rx2 = b.try_draft(ff());
    let rx3 = b.list();
    let r1 = rx1.recv_timeout(T).unwrap().expect("the throwing try answers");
    println!("a throwing try on a warm server answered in {:.0} ms", t1.elapsed().as_secs_f64() * 1000.0);
    assert!(r1.error.as_deref().unwrap_or("").contains("studio self-heal test"), "{r1:?}");
    let r2 = rx2.recv_timeout(T).unwrap().expect("ff queued behind it");
    assert_ff(&r2, "ff queued behind a throwing try");
    let l = rx3.recv_timeout(T).unwrap().expect("list queued behind it");
    assert!(l.iter().any(|s| s.trigger == "uu"), "list after the restart");
    assert_eq!(b.starts(), starts + 1, "one restart for one error");
    seen.extend(tree(&b));

    // the adversary's sequence: a body ending in a newline, in display, then ff
    let nl = try_req("  math(\"nlz\", [[\\quad\n]], \"yours\"),", "display", "nlz", &[]);
    let starts = b.starts();
    let r = b.try_draft(nl).recv_timeout(T).unwrap();
    println!("try nlz (trailing newline): {r:?}");
    let r = b.try_draft(ff()).recv_timeout(T).unwrap().expect("ff after nlz");
    assert_ff(&r, "ff after nlz");
    println!("nlz then ff: ff right; restarts in between: {}", b.starts() - starts);
    seen.extend(tree(&b));

    b.stop();
    assert_gone(&seen, "after stop");
    assert!(b.private_dir().is_none());
}

#[test]
fn requests_survive_a_crash() {
    let t = TempDir::new("retry");
    let paths = temp_paths(&t);
    let mut b = Bridge::start(&paths).expect("start");
    assert!(b.wait_ready(T), "the bridge never got ready");
    let n = b.list().recv_timeout(T).unwrap().expect("list").len();
    let mut seen = tree(&b);

    // killed, then asked at once: both answers come from the next server
    let old = tree(&b);
    let t0 = Instant::now();
    b.kill_for_test();
    let rx_list = b.list();
    let rx_ff = b.try_draft(ff());
    let l = rx_list.recv_timeout(T).unwrap().expect("list right after a crash");
    assert_eq!(l.len(), n);
    let r = rx_ff.recv_timeout(T).unwrap().expect("try right after a crash");
    assert_ff(&r, "ff right after a crash");
    println!("retry: list + try sent at the crash answered by the next server after {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
    assert_eq!(b.starts(), 2);
    assert_gone(&old, "the killed server");
    seen.extend(tree(&b));

    // asked while no server is running at all: queued, answered once it's back
    let old = tree(&b);
    b.kill_for_test();
    assert!(wait_for(Duration::from_secs(10), || b.pid().is_none()), "the death is noticed");
    let t0 = Instant::now();
    let rx = b.list();
    let rx_ff = b.try_draft(ff());
    let none_running = b.pid().is_none();
    let l = rx.recv_timeout(T).unwrap().expect("list sent while no server ran");
    assert_eq!(l.len(), n);
    assert_ff(&rx_ff.recv_timeout(T).unwrap().expect("try sent while no server ran"), "ff sent while no server ran");
    assert!(none_running, "the requests were sent while no server ran");
    println!("queued: list + try sent with no server running answered after {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
    assert_eq!(b.starts(), 3);
    assert_gone(&old, "the second killed server");
    seen.extend(tree(&b));

    // stop: everything pending fails, nothing is left
    let rx = b.try_draft(ff());
    b.stop();
    assert!(rx.recv_timeout(T).unwrap().is_err(), "pending fails on stop");
    assert!(b.list().recv_timeout(T).unwrap().is_err(), "a stopped bridge answers with an error");
    assert_gone(&seen, "after stop");
}

#[test]
fn gives_up_on_a_server_that_never_starts() {
    // a config whose server.lua dies at once (server_script prefers
    // <config>/studio/bridge/server.lua)
    let t = TempDir::new("giveup");
    let mut paths = temp_paths(&t);
    let config = t.path().join("config");
    std::fs::create_dir_all(config.join("studio/bridge")).unwrap();
    std::fs::write(config.join("studio/bridge/server.lua"), "io.stderr:write('broken on purpose\\n')\nos.exit(3)\n").unwrap();
    paths.config = config.clone();

    let t0 = Instant::now();
    let mut b = Bridge::start(&paths).expect("start");
    // a request waits through one restart, then fails (not hangs)
    let early = b.list().recv_timeout(T).unwrap();
    println!("request to a server that never starts: {early:?} after {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
    assert!(early.is_err());
    // then the bridge gives up: requests fail at once, nothing respawns
    assert!(
        wait_for(T, || b.list().recv_timeout(Duration::from_millis(50)).ok().and_then(|r| r.err()).map(|e| e.contains("gave up")).unwrap_or(false)),
        "the bridge gives up"
    );
    let starts = b.starts();
    println!("gave up after {starts} starts, {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
    assert_eq!(starts, 6);
    assert!(!b.is_ready() && b.pid().is_none());
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(b.starts(), starts, "no more starts after giving up");
    b.stop();
    let o = std::process::Command::new("pgrep").args(["-f", &config.to_string_lossy()]).output().unwrap();
    assert!(o.stdout.is_empty(), "left running: {}", String::from_utf8_lossy(&o.stdout));
}

/// 2026-10-08, the author's first save: the child nvim died between requests, the
/// server (blocked reading stdin) never reaped it, and every later request
/// failed with "Invalid channel: 3" — shown as "inbox.lua would not load".
/// Now the server notices, asks for a restart, and the request is answered by
/// the fresh one; a load error and a failed check are told apart.
#[test]
fn a_child_that_dies_between_requests_is_replaced() {
    let t = TempDir::new("deadchild");
    let paths = temp_paths(&t);
    let mut b = Bridge::start(&paths).expect("start");
    assert!(b.wait_ready(T), "the bridge never got ready");
    let old = tree(&b);
    let child = b.pids()[1];
    unsafe {
        libc::kill(child as i32, libc::SIGKILL);
    }
    std::thread::sleep(Duration::from_millis(300));
    let good = "local h = require(\"snips.helpers\")\nreturn h.group(\"Inbox\", {\n  h.math(\"alp\", [[\\alpha]], \"yours\"),\n})\n";
    let r = b.check_file("inbox.lua", good).recv_timeout(T).expect("answer");
    assert_eq!(r, Ok(()), "check after the child died");
    let r = b.check_file("inbox.lua", "return h.group(").recv_timeout(T).expect("answer");
    let e = r.expect_err("a broken file");
    assert!(e.starts_with("inbox.lua:") && !e.starts_with(ntex_studio_io::bridge::UNCHECKED), "a load error, not a bridge failure: {e}");
    let r = b.try_draft(ff()).recv_timeout(T).expect("answer").expect("try");
    assert_ff(&r, "try after the restart");
    let live: Vec<u32> = b.pids();
    assert!(!live.contains(&child), "a fresh child: {live:?}");
    b.stop();
    assert_gone(&old, "the old server");
}
