//! A long-lived headless ntex (studio/bridge/server.lua, run by
//! `nvim --clean --headless -l`, which drives the real config in a child
//! `nvim --embed --headless` over RPC, like tests/driver.lua). JSON lines on
//! stdin/stdout. Started once in the background when the studio starts;
//! restarted if it dies. Every call is async (a background thread) and never
//! blocks the UI thread.
//!
//! - The server runs in its own process group (the child nvim joins it), so
//!   `stop()` / Drop end the whole tree; if the studio itself dies, the
//!   server sees its stdin close and quits on its own.
//! - Its private folder (`$TMPDIR/ntex-studio-bridge.*`: the snippets copy,
//!   scratch buffers) is removed on quit by the server and again by `stop()`.
//! - The child has no language servers (`TEXNVIM_NO_LSP=1`), no viewer, no
//!   compile, no reference rebuild, no folder watcher.
//! - Self-heal: a try that sees a Lua error in the child (LuaSnip throwing)
//!   answers with `TryResult.error`, then the server says
//!   `{"event":"restart"}` and exits; the next one starts at once, before
//!   any later request is handled. Planned restarts don't count as failures.
//! - Requests survive a crash: each is kept with its line until answered,
//!   resent once to the next server if the server dies, and queued (not
//!   failed) while none runs. Servers that keep dying before they're ready
//!   back off, then the bridge gives up and fails what's queued.
use ntex_studio_core::model::*;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

type Reply = Box<dyn FnOnce(Result<Value, String>) + Send>;

/// A one-line LaTeX field, with byte offsets (selection end is exclusive).
#[derive(Clone, Debug, PartialEq, Default, serde::Deserialize)]
pub struct FieldState {
    /// The buffer line.
    pub text: String,
    /// Byte offset of the insert cursor in `text`.
    pub cursor: usize,
    /// Select-mode tab-stop default, as an exclusive byte range.
    pub selected: Option<(usize, usize)>,
    /// LuaSnip can jump forward, so Tab belongs to the snippet.
    pub in_snippet: bool,
}

/// Consecutive servers that die before getting ready: after this many the
/// bridge gives up (and fails what's queued). Quick retries for the first
/// few, then a 2 s back-off.
const GIVE_UP_AFTER: u32 = 6;
const QUICK_RETRIES: u32 = 3;

pub struct Bridge {
    shared: Arc<Shared>,
}

struct Shared {
    paths: Paths,
    next_id: AtomicU64,
    next_field_id: AtomicU64,
    stopping: AtomicBool,
    state: Mutex<State>,
    ready_cv: Condvar,
}

/// A request without an answer yet. Its line is kept so that it can be
/// resent, once, to the next server if this one dies.
struct Pending {
    line: String,
    reply: Reply,
    retried: bool,
}

#[derive(Default)]
struct State {
    child: Option<Child>,
    /// lines for the server's stdin, written by its writer thread (so a
    /// request never blocks on a full pipe); None while no server runs
    stdin: Option<mpsc::Sender<String>>,
    dir: Option<PathBuf>,
    /// bumped per spawned server; a reader only touches its own generation
    generation: u64,
    /// every request not answered yet: sent to the current server, or waiting
    /// for the next one (spawn writes them to it)
    pending: HashMap<u64, Pending>,
    /// Last acknowledged field text/cursor. Reopened before replaying requests.
    fields: HashMap<u64, FieldState>,
    /// the child nvim (nvim gives it its own process group, so it is killed
    /// by pid as well as by the server's group)
    child_nvim: Option<i32>,
    ready: bool,
    /// ms from spawn to the server's "ready" line, for the last start
    ready_ms: Option<f64>,
    starts: u32,
    last_error: Option<String>,
    /// the current server said {"event":"restart"} (a try saw a Lua error in
    /// its child): its exit is planned, not a crash
    restart_planned: bool,
    /// consecutive servers that died before getting ready (reset on ready)
    failures: u32,
    /// restarting was given up: requests fail at once
    gave_up: bool,
}

/// Where server.lua lives: `<config>/studio/bridge/server.lua`, falling back
/// to the copy next to this crate (a temp `NTEX_STUDIO_CONFIG` may not hold
/// the studio's sources).
pub fn server_script(paths: &Paths) -> PathBuf {
    let p = paths.config.join("studio/bridge/server.lua");
    if p.exists() {
        return p;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bridge/server.lua")
}

fn nvim_path() -> PathBuf {
    for p in ["/opt/homebrew/bin/nvim", "/usr/local/bin/nvim"] {
        if Path::new(p).exists() {
            return PathBuf::from(p);
        }
    }
    PathBuf::from("nvim")
}

/// The NVIM_APPNAME of the config: `texnvim` normally; a temp copy's folder
/// name only if it is under ~/.config (where nvim looks for it).
fn appname(paths: &Paths) -> String {
    if let Ok(a) = std::env::var("TEXNVIM_APPNAME") {
        return a;
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    if paths.config.parent() == Some(&home.join(".config")) {
        if let Some(n) = paths.config.file_name() {
            return n.to_string_lossy().into_owned();
        }
    }
    "texnvim".into()
}

fn temp_root() -> PathBuf {
    std::env::var_os("TMPDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn make_private_dir() -> std::io::Result<PathBuf> {
    let base = temp_root();
    for n in 0..1000u32 {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
        let p = base.join(format!("ntex-studio-bridge.{}.{:x}{}", std::process::id(), nanos, n));
        match std::fs::create_dir(&p) {
            Ok(()) => return Ok(p),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::other("can't make a temp folder"))
}

pub(crate) fn kill_tree(child: &mut Child, child_nvim: Option<i32>) {
    let pid = child.id() as i32;
    unsafe {
        // the server leads its own group
        libc::killpg(pid, libc::SIGKILL);
        // the child nvim leads another (and would exit by itself once its
        // RPC pipe closes; this makes sure)
        if let Some(c) = child_nvim.filter(|c| *c > 1) {
            libc::killpg(c, libc::SIGKILL);
            libc::kill(c, libc::SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

impl Shared {
    /// Start a server. Requests waiting for one are written to it at once
    /// (the server reads them once it is ready).
    fn spawn(self: &Arc<Self>) -> std::io::Result<()> {
        use std::os::unix::process::CommandExt;
        if self.stopping.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("the ntex bridge is stopped"));
        }
        let dir = make_private_dir()?;
        let mut cmd = Command::new(nvim_path());
        cmd.arg("--clean")
            .arg("--headless")
            .arg("-l")
            .arg(server_script(&self.paths))
            .env("NTEX_BRIDGE_DIR", &dir)
            .env("NTEX_BRIDGE_SOURCE", &self.paths.snippets)
            .env("NTEX_BRIDGE_APPNAME", appname(&self.paths))
            .env_remove("NVIM")
            .env_remove("NVIM_LISTEN_ADDRESS")
            .current_dir(&dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let t0 = Instant::now();
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&dir);
                return Err(e);
            }
        };
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let stdin = child.stdin.take().unwrap();
        let (tx, rx) = mpsc::channel::<String>();
        let generation = {
            let mut st = self.state.lock().unwrap();
            // stop() ran meanwhile: this server must not outlive it
            if self.stopping.load(Ordering::SeqCst) {
                drop(st);
                kill_tree(&mut child, None);
                let _ = std::fs::remove_dir_all(&dir);
                return Err(std::io::Error::other("the ntex bridge is stopped"));
            }
            st.generation += 1;
            st.starts += 1;
            st.child = Some(child);
            st.dir = Some(dir);
            st.child_nvim = None;
            st.ready = false;
            st.ready_ms = None;
            st.restart_planned = false;
            let mut fields: Vec<_> = st.fields.iter().collect();
            fields.sort_by_key(|(id, _)| **id);
            for (id, field) in fields {
                let _ = tx.send(format!("{}\n", json!({ "id": 0, "op": "field_open", "field": id, "text": field.text, "cursor": field.cursor })));
            }
            // what's waiting (queued while no server ran, or requeued from
            // the last one), in the order it was asked
            let mut waiting: Vec<(&u64, &Pending)> = st.pending.iter().collect();
            waiting.sort_by_key(|(id, _)| **id);
            for (_, p) in waiting {
                let _ = tx.send(p.line.clone());
            }
            st.stdin = Some(tx);
            st.generation
        };
        let started = (|| -> std::io::Result<()> {
            // stdin: a writer thread, so callers never block on the pipe
            std::thread::Builder::new().name("ntex-bridge-writer".into()).spawn(move || {
                let mut w = stdin;
                for line in rx {
                    if w.write_all(line.as_bytes()).and_then(|_| w.flush()).is_err() {
                        break;
                    }
                }
            })?;
            // stderr: the server's log; kept short in last_error, and appended
            // to <state>/bridge.log (started afresh once it passes 512 kB)
            let me = self.clone();
            let log_path = self.paths.state.join("bridge.log");
            std::thread::Builder::new().name("ntex-bridge-log".into()).spawn(move || {
                if std::fs::metadata(&log_path).is_ok_and(|m| m.len() > 512 * 1024) {
                    let _ = std::fs::remove_file(&log_path);
                }
                let mut log = std::fs::create_dir_all(log_path.parent().unwrap_or(Path::new(".")))
                    .and_then(|_| std::fs::OpenOptions::new().create(true).append(true).open(&log_path))
                    .ok();
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    if std::env::var_os("NTEX_STUDIO_DEBUG").is_some() {
                        eprintln!("[bridge] {line}");
                    }
                    if let Some(f) = log.as_mut() {
                        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                        let _ = writeln!(f, "{secs} {line}");
                    }
                    let mut st = me.state.lock().unwrap();
                    if st.generation == generation {
                        st.last_error = Some(line.chars().take(500).collect());
                    }
                }
            })?;
            let me = self.clone();
            std::thread::Builder::new().name("ntex-bridge-reader".into()).spawn(move || me.read_loop(stdout, generation, t0))?;
            Ok(())
        })();
        if let Err(e) = started {
            // no reader: nobody would notice its death, so end it now
            let mut st = self.state.lock().unwrap();
            if st.generation == generation {
                let nv = st.child_nvim.take();
                if let Some(mut c) = st.child.take() {
                    kill_tree(&mut c, nv);
                }
                st.stdin = None;
                if let Some(d) = st.dir.take() {
                    let _ = std::fs::remove_dir_all(d);
                }
            }
            return Err(e);
        }
        Ok(())
    }

    fn read_loop(self: Arc<Self>, stdout: std::process::ChildStdout, generation: u64, t0: Instant) {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
            if let Some(ev) = v.get("event").and_then(Value::as_str) {
                let mut st = self.state.lock().unwrap();
                if st.generation != generation {
                    return;
                }
                if let Some(c) = v.get("child").and_then(Value::as_i64) {
                    st.child_nvim = Some(c as i32);
                }
                if ev == "ready" {
                    st.ready = true;
                    st.ready_ms = Some(t0.elapsed().as_secs_f64() * 1000.0);
                    st.failures = 0;
                    self.ready_cv.notify_all();
                } else if ev == "failed" {
                    st.last_error = v.get("error").and_then(Value::as_str).map(str::to_string);
                } else if ev == "restart" {
                    // a try saw a Lua error in the child; the server exits
                    // next, and the next one starts at once
                    st.restart_planned = true;
                }
                continue;
            }
            let Some(id) = v.get("id").and_then(Value::as_u64) else { continue };
            let pending = {
                let mut st = self.state.lock().unwrap();
                if st.generation != generation {
                    return;
                }
                if let (Some(field), Some(result)) = (v["field"].as_u64(), v.get("state")) {
                    if let Some(saved) = st.fields.get_mut(&field) {
                        if let Ok(result) = serde_json::from_value::<FieldState>(result.clone()) {
                            *saved = result;
                        }
                    }
                }
                st.pending.remove(&id)
            };
            if let Some(p) = pending {
                let res = if v.get("ok").and_then(Value::as_bool) == Some(true) {
                    Ok(v)
                } else {
                    Err(v.get("error").and_then(Value::as_str).unwrap_or("the bridge failed").to_string())
                };
                (p.reply)(res);
            }
        }
        // the server is gone: clean up, requeue what it didn't answer (each
        // request gets one more go), restart unless stopping
        let (failed, planned, why) = {
            let mut st = self.state.lock().unwrap();
            if st.generation != generation {
                return;
            }
            let nv = st.child_nvim.take();
            if let Some(mut c) = st.child.take() {
                kill_tree(&mut c, nv);
            }
            st.stdin = None;
            let was_ready = std::mem::replace(&mut st.ready, false);
            if let Some(d) = st.dir.take() {
                let _ = std::fs::remove_dir_all(d);
            }
            let planned = st.restart_planned;
            if !was_ready && !planned {
                st.failures += 1;
            }
            let mut failed = Vec::new();
            if !planned {
                // a planned restart answered the request that caused it; the
                // rest were never handled, so they keep their retry
                let retried: Vec<u64> = st.pending.iter().filter(|(_, p)| p.retried).map(|(id, _)| *id).collect();
                for id in retried {
                    if let Some(p) = st.pending.remove(&id) {
                        failed.push(p.reply);
                    }
                }
                for p in st.pending.values_mut() {
                    p.retried = true;
                }
            }
            self.ready_cv.notify_all();
            (failed, planned, st.last_error.clone())
        };
        let msg = match why {
            Some(w) if !w.is_empty() => format!("the ntex bridge stopped ({w})"),
            _ => "the ntex bridge stopped".to_string(),
        };
        for reply in failed {
            reply(Err(msg.clone()));
        }
        self.respawn(planned);
    }

    /// Start the next server (on the dying one's reader thread). A planned
    /// restart goes at once; a crash waits a little; servers that keep dying
    /// before they're ready back off, then the bridge gives up.
    fn respawn(self: &Arc<Self>, planned: bool) {
        let mut first = true;
        loop {
            if self.stopping.load(Ordering::SeqCst) {
                return;
            }
            let failures = self.state.lock().unwrap().failures;
            if failures >= GIVE_UP_AFTER {
                let (waiting, why) = {
                    let mut st = self.state.lock().unwrap();
                    st.gave_up = true;
                    (std::mem::take(&mut st.pending), st.last_error.clone())
                };
                let msg = gave_up_message(why);
                for (_, p) in waiting {
                    (p.reply)(Err(msg.clone()));
                }
                return;
            }
            let delay = if planned && first {
                0
            } else if failures > QUICK_RETRIES {
                2000
            } else {
                100
            };
            first = false;
            std::thread::sleep(Duration::from_millis(delay));
            if self.stopping.load(Ordering::SeqCst) {
                return;
            }
            match self.spawn() {
                Ok(()) => return,
                Err(e) => {
                    let mut st = self.state.lock().unwrap();
                    st.failures += 1;
                    st.last_error = Some(format!("couldn't start: {e}"));
                }
            }
        }
    }

    /// Send a request; `reply` runs on the reader thread with the answer. If
    /// the server dies first, the request is resent once to the next one;
    /// while no server runs, it waits for the next one.
    fn request(self: &Arc<Self>, mut req: Value, reply: Reply) {
        let mut st = self.state.lock().unwrap();
        if self.stopping.load(Ordering::SeqCst) {
            drop(st);
            reply(Err("the ntex bridge is stopped".into()));
            return;
        }
        if st.gave_up {
            let msg = gave_up_message(st.last_error.clone());
            drop(st);
            reply(Err(msg));
            return;
        }
        if matches!(req["op"].as_str(), Some("field_open" | "field_set"))
            && req["text"].as_str().is_none_or(|text| text.contains(['\r', '\n']))
        {
            drop(st);
            reply(Err("a snippet field must have one line".into()));
            return;
        }
        // Allocate ids under the queue lock: replay order equals send order,
        // including calls made from different threads.
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        req["id"] = json!(id);
        if let Some(field) = req["field"].as_u64() {
            match req["op"].as_str() {
                Some("field_open") => {
                    st.fields.insert(field, FieldState { text: req["text"].as_str().unwrap_or_default().into(), cursor: req["cursor"].as_u64().unwrap_or(0) as usize, ..FieldState::default() });
                }
                Some("field_close") => { st.fields.remove(&field); }
                Some("field_keys" | "field_set") if !st.fields.contains_key(&field) => {
                    drop(st);
                    reply(Err(format!("unknown field: {field}")));
                    return;
                }
                _ => {}
            }
        }
        let line = format!("{}\n", req);
        if let Some(tx) = st.stdin.as_ref() {
            if tx.send(line.clone()).is_err() {
                // the writer is gone (the server died): wait for the next one
                st.stdin = None;
            }
        }
        st.pending.insert(id, Pending { line, reply, retried: false });
    }
}

fn gave_up_message(why: Option<String>) -> String {
    match why {
        Some(w) if !w.is_empty() => format!("the ntex bridge keeps failing to start; gave up ({w})"),
        _ => "the ntex bridge keeps failing to start; gave up".to_string(),
    }
}

impl Bridge {
    /// Spawn the server (does not wait for it).
    pub fn start(paths: &Paths) -> std::io::Result<Bridge> {
        let shared = Arc::new(Shared {
            paths: paths.clone(),
            next_id: AtomicU64::new(1),
            next_field_id: AtomicU64::new(1),
            stopping: AtomicBool::new(false),
            state: Mutex::new(State::default()),
            ready_cv: Condvar::new(),
        });
        shared.spawn()?;
        Ok(Bridge { shared })
    }

    /// Has the server loaded the config yet? (Requests sent before then just
    /// wait in its queue.)
    pub fn is_ready(&self) -> bool {
        self.shared.state.lock().unwrap().ready
    }

    /// Block until ready (or `timeout`); tests and check_file_once.
    pub fn wait_ready(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut st = self.shared.state.lock().unwrap();
        while !st.ready {
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            st = self.shared.ready_cv.wait_timeout(st, deadline - now).unwrap().0;
        }
        true
    }

    /// Milliseconds from spawning the server to its "ready" line (last start).
    pub fn ready_ms(&self) -> Option<f64> {
        self.shared.state.lock().unwrap().ready_ms
    }

    /// How many times the server was started (1 + restarts).
    pub fn starts(&self) -> u32 {
        self.shared.state.lock().unwrap().starts
    }

    /// The server's process id (its process group), if running.
    pub fn pid(&self) -> Option<u32> {
        self.shared.state.lock().unwrap().child.as_ref().map(|c| c.id())
    }

    /// The server and its child nvim (once ready), if running.
    pub fn pids(&self) -> Vec<u32> {
        let st = self.shared.state.lock().unwrap();
        st.child.as_ref().map(|c| c.id()).into_iter().chain(st.child_nvim.map(|c| c as u32)).collect()
    }

    /// The private folder (snippets copy, scratch buffers), if running.
    pub fn private_dir(&self) -> Option<PathBuf> {
        self.shared.state.lock().unwrap().dir.clone()
    }

    /// Open a scratch tex buffer in the hidden ntex, initially in text mode.
    pub fn field_open(&self, text: &str, cursor: usize) -> Receiver<Result<(u64, FieldState), String>> {
        let id = self.shared.next_field_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        self.shared.request(json!({ "op": "field_open", "field": id, "text": text, "cursor": cursor }), Box::new(move |res| {
            let _ = tx.send(parse_field(res).map(|state| (id, state)));
        }));
        rx
    }

    /// Type nvim key notation, processing snippets before returning the state.
    pub fn field_keys(&self, id: u64, keys: &str) -> Receiver<Result<FieldState, String>> {
        self.field_request(json!({ "op": "field_keys", "field": id, "keys": keys }))
    }

    /// Replace the text/cursor without typing or firing snippets.
    pub fn field_set(&self, id: u64, text: &str, cursor: usize) -> Receiver<Result<FieldState, String>> {
        self.field_request(json!({ "op": "field_set", "field": id, "text": text, "cursor": cursor }))
    }

    pub fn field_close(&self, id: u64) {
        self.shared.request(json!({ "op": "field_close", "field": id }), Box::new(|_| {}));
    }

    fn field_request(&self, req: Value) -> Receiver<Result<FieldState, String>> {
        let (tx, rx) = mpsc::channel();
        self.shared.request(req, Box::new(move |res| { let _ = tx.send(parse_field(res)); }));
        rx
    }

    /// Every loaded snippet (inbox included).
    pub fn list(&self) -> Receiver<Result<Vec<KnownSnippet>, String>> {
        let (tx, rx) = mpsc::channel();
        self.shared.request(
            json!({ "op": "list" }),
            Box::new(move |res| {
                let out = res.and_then(|v| serde_json::from_value::<Vec<KnownSnippet>>(v["snippets"].clone()).map_err(|e| format!("bad list from the bridge: {e}")));
                let _ = tx.send(out);
            }),
        );
        rx
    }

    /// Would this snippet file load? Err = the loader's message, with line;
    /// or, when the check itself could not run (the bridge failed), a message
    /// starting with [`UNCHECKED`], which says nothing about the file.
    pub fn check_file(&self, name: &str, text: &str) -> Receiver<Result<(), String>> {
        let (tx, rx) = mpsc::channel();
        self.shared.request(
            json!({ "op": "check", "name": name, "text": text }),
            Box::new(move |res| {
                let out = match res {
                    Ok(v) if v.get("loads").and_then(Value::as_bool) == Some(false) => {
                        Err(v.get("error").and_then(Value::as_str).unwrap_or("did not load").to_string())
                    }
                    Ok(_) => Ok(()),
                    Err(e) => Err(format!("{UNCHECKED}{e}")),
                };
                let _ = tx.send(out);
            }),
        );
        rx
    }

    /// Type the draft in a real ntex buffer beside all her snippets.
    pub fn try_draft(&self, req: TryRequest) -> Receiver<Result<TryResult, String>> {
        self.try_draft_hiding(req, None)
    }

    /// `try_draft`, with one of her files swapped for `(name, text)` while it
    /// runs: the file without the entry being edited, so the old version
    /// doesn't shadow the draft. The bridge's copy is put back afterwards.
    pub fn try_draft_hiding(&self, req: TryRequest, hide: Option<(String, String)>) -> Receiver<Result<TryResult, String>> {
        let (tx, rx) = mpsc::channel();
        let mut v = serde_json::to_value(&req).unwrap_or_else(|_| json!({}));
        v["op"] = json!("try");
        if let Some((file, text)) = hide {
            v["hide"] = json!({ "file": file, "text": text });
        }
        self.shared.request(
            v,
            Box::new(move |res| {
                let out = res.and_then(|v| serde_json::from_value::<TryResult>(v["result"].clone()).map_err(|e| format!("bad try result from the bridge: {e}")));
                let _ = tx.send(out);
            }),
        );
        rx
    }

    /// The real snippet files changed (saved, sorted): re-copy them into the
    /// bridge's private folder.
    pub fn refresh(&self) {
        self.shared.request(json!({ "op": "refresh" }), Box::new(|_| {}));
    }

    /// The same, with a receiver for when it's done (Ok: the files that changed).
    pub fn refresh_done(&self) -> Receiver<Result<Vec<String>, String>> {
        let (tx, rx) = mpsc::channel();
        self.shared.request(
            json!({ "op": "refresh" }),
            Box::new(move |res| {
                let _ = tx.send(res.map(|v| v["changed"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()));
            }),
        );
        rx
    }

    /// Kill the server as if it crashed (tests: it must come back).
    pub fn kill_for_test(&self) {
        let st = self.shared.state.lock().unwrap();
        if let Some(c) = st.child.as_ref() {
            unsafe {
                libc::killpg(c.id() as i32, libc::SIGKILL);
            }
        }
        // the child nvim is left to notice on its own (its pipe closes)
    }

    /// Stop the server and its child (also on Drop). No orphans, ever.
    /// Everything pending or queued fails.
    pub fn stop(&mut self) {
        self.shared.stopping.store(true, Ordering::SeqCst);
        let (child, child_nvim, stdin, dir, pending) = {
            let mut st = self.shared.state.lock().unwrap();
            st.ready = false;
            st.generation += 1; // the reader of the old server stands down
            (st.child.take(), st.child_nvim.take(), st.stdin.take(), st.dir.take(), std::mem::take(&mut st.pending))
        };
        for (_, p) in pending {
            (p.reply)(Err("the ntex bridge was stopped".into()));
        }
        if let Some(mut child) = child {
            // ask nicely (it quits the child nvim and removes its folder) …
            if let Some(tx) = stdin {
                let _ = tx.send("{\"id\":0,\"op\":\"quit\"}\n".into());
                drop(tx); // the writer closes the server's stdin after it
            }
            let deadline = Instant::now() + Duration::from_millis(1500);
            while Instant::now() < deadline {
                if let Ok(Some(_)) = child.try_wait() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            // … then make sure: both process groups
            kill_tree(&mut child, child_nvim);
        }
        if let Some(d) = dir {
            let _ = std::fs::remove_dir_all(d);
        }
        self.shared.ready_cv.notify_all();
    }
}

fn parse_field(res: Result<Value, String>) -> Result<FieldState, String> {
    res.and_then(|v| serde_json::from_value(v["state"].clone()).map_err(|e| format!("bad field state from the bridge: {e}")))
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Prefix of a check_file error that means the check did not run (the
/// bridge failed), as opposed to the file not loading.
pub const UNCHECKED: &str = "unchecked: ";

/// Blocking check of one file, without the long-lived server (used by
/// core::apply's check closure from a background thread, and by tests).
pub fn check_file_once(paths: &Paths, name: &str, text: &str) -> Result<(), String> {
    let mut b = Bridge::start(paths).map_err(|e| format!("the ntex bridge didn't start: {e}"))?;
    let res = b.check_file(name, text).recv_timeout(Duration::from_secs(60)).unwrap_or_else(|_| Err("the ntex bridge didn't answer".into()));
    b.stop();
    res
}
