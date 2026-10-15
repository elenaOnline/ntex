//! The inbox sorter: `claude -p` with Haiku 5.5 at high effort, read-only
//! tools (Read, Grep, Glob) in the snippets folder, a JSON-schema'd SortPlan
//! as the answer, progress streamed (stream-json) into AgentEvents.
//!
//! How it runs (all non-interactive, read-only, no hooks):
//!   claude -p --model <sort_model> --effort <sort_effort>
//!     --output-format stream-json --verbose --json-schema <plan_schema>
//!     --tools Read,Grep,Glob --allowedTools Read Grep Glob
//!     --permission-mode dontAsk     anything else is refused, never asked
//!     --safe-mode                   no hooks, plugins, MCP, CLAUDE.md, skills
//!     --strict-mcp-config --no-session-persistence --max-budget-usd 1
//!     --system-prompt <short, ours>  (no environment / git sections)
//!   prompt on stdin; cwd = a private copy of the snippets folder (tex/*.lua,
//!   README.md), so the agent can't write her files and sees nothing outside.
//! `--bare` (which also drops Claude Code's own account context) is used only
//! when ANTHROPIC_API_KEY is set: with a claude.ai login it can't
//! authenticate. `--max-turns` doesn't exist in this CLI; the 180 s timeout
//! bounds the run. Cancel (and Drop) kill the process group.
use ntex_studio_core::model::*;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const TIMEOUT: Duration = Duration::from_secs(180);

pub struct SortJob {
    child: Arc<Mutex<Option<Child>>>,
    done: Arc<AtomicBool>,
    workdir: PathBuf,
    tx: Sender<AgentEvent>,
}

const SYSTEM_PROMPT: &str = "You sort LaTeX snippets into the topic files of one person's snippet collection. \
You work only from what the user message gives you and the files in the current folder. \
You never edit anything. You answer with the structured plan only.";

/// One snippet line of a topic file, as the prompt shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct FileEntry {
    pub helper: String,
    pub trigger: String,
    pub body: String,
}

/// A topic file, summarised for the prompt.
#[derive(Clone, Debug, PartialEq)]
pub struct TopicFile {
    pub name: String,
    pub first_comment: String,
    pub title: String,
    pub entries: Vec<FileEntry>,
}

const HELPERS: &[&str] = &["math", "math_tab", "text", "suffix", "post", "pat", "vis", "env"];

/// Read a Lua string literal at `s[i..]` ("…", '…' or [[…]]/[=[…]=]).
fn lua_string(s: &[u8], i: usize) -> Option<(String, usize)> {
    let c = *s.get(i)?;
    if c == b'"' || c == b'\'' {
        let mut out = Vec::new();
        let mut j = i + 1;
        while j < s.len() {
            match s[j] {
                b'\\' if j + 1 < s.len() => {
                    let e = s[j + 1];
                    out.push(match e {
                        b'n' => b'\n',
                        b't' => b'\t',
                        _ => e,
                    });
                    j += 2;
                }
                x if x == c => return Some((String::from_utf8_lossy(&out).into_owned(), j + 1)),
                b'\n' => return None,
                x => {
                    out.push(x);
                    j += 1;
                }
            }
        }
        None
    } else if c == b'[' {
        let mut j = i + 1;
        let mut level = 0;
        while s.get(j) == Some(&b'=') {
            level += 1;
            j += 1;
        }
        if s.get(j) != Some(&b'[') {
            return None;
        }
        let close: Vec<u8> = std::iter::once(b']').chain(std::iter::repeat_n(b'=', level)).chain(std::iter::once(b']')).collect();
        let start = j + 1;
        let rest = &s[start..];
        let pos = rest.windows(close.len()).position(|w| w == close.as_slice())?;
        let mut body = &rest[..pos];
        if body.first() == Some(&b'\n') {
            body = &body[1..];
        }
        Some((String::from_utf8_lossy(body).into_owned(), start + pos + close.len()))
    } else {
        None
    }
}

fn skip_ws(s: &[u8], mut i: usize) -> usize {
    while i < s.len() && (s[i] as char).is_whitespace() {
        i += 1;
    }
    i
}

/// The snippets of a snippet file: helper, trigger, body (light scan of the
/// Lua; comments and strings are skipped properly).
pub fn scan_entries(src: &str) -> Vec<FileEntry> {
    let s = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        // comments
        if c == b'-' && s.get(i + 1) == Some(&b'-') {
            if let Some((_, end)) = lua_string(s, i + 2).filter(|_| s.get(i + 2) == Some(&b'[')) {
                i = end;
            } else {
                while i < s.len() && s[i] != b'\n' {
                    i += 1;
                }
            }
            continue;
        }
        // strings outside a helper call
        if c == b'"' || c == b'\'' || (c == b'[' && matches!(s.get(i + 1), Some(b'[') | Some(b'='))) {
            match lua_string(s, i) {
                Some((_, end)) => i = end,
                None => i += 1,
            }
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < s.len() && (s[i].is_ascii_alphanumeric() || s[i] == b'_') {
                i += 1;
            }
            let word = &src[start..i];
            let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
            // `math(` or `h.math(`, not `foo.math(` / `xmath(`
            let prev_ok = match start {
                0 => true,
                _ if s[start - 1] == b'.' => start >= 2 && s[start - 2] == b'h' && (start == 2 || !is_ident(s[start - 3])),
                _ => !is_ident(s[start - 1]),
            };
            if prev_ok && HELPERS.contains(&word) {
                let j = skip_ws(s, i);
                if s.get(j) == Some(&b'(') {
                    let k = skip_ws(s, j + 1);
                    if let Some((trigger, end)) = lua_string(s, k) {
                        let mut body = String::new();
                        let m = skip_ws(s, end);
                        if s.get(m) == Some(&b',') {
                            let n = skip_ws(s, m + 1);
                            if let Some((b, _)) = lua_string(s, n) {
                                body = b;
                            } else if src[n..].starts_with("function") {
                                body = "(computed by a function)".into();
                            }
                        }
                        out.push(FileEntry { helper: word.to_string(), trigger, body });
                        i = end;
                    }
                }
            }
            continue;
        }
        i += 1;
    }
    out
}

/// Summarise a topic file.
pub fn topic_file(name: &str, src: &str) -> TopicFile {
    let first_comment = src
        .lines()
        .find(|l| l.trim_start().starts_with("--"))
        .map(|l| l.trim_start().trim_start_matches('-').trim().to_string())
        .unwrap_or_default();
    let title = src
        .find("group(")
        .and_then(|p| lua_string(src.as_bytes(), skip_ws(src.as_bytes(), p + 6)))
        .map(|(t, _)| t)
        .unwrap_or_default();
    TopicFile { name: name.to_string(), first_comment, title, entries: scan_entries(src) }
}

/// Every topic file in snippets/tex (not the inbox, not studio scratch files), sorted.
pub fn topic_files(snippets: &Path) -> Vec<TopicFile> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(snippets.join("tex")) {
        let mut names: Vec<String> = rd
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with(".lua") && n != "inbox.lua" && !n.starts_with("zz_studio"))
            .collect();
        names.sort();
        for n in names {
            if let Ok(src) = std::fs::read_to_string(snippets.join("tex").join(&n)) {
                out.push(topic_file(&n, &src));
            }
        }
    }
    out
}

fn short(s: &str, n: usize) -> String {
    let one: String = s.replace('\n', "⏎").replace('\t', " ");
    if one.chars().count() > n {
        format!("{}…", one.chars().take(n).collect::<String>())
    } else {
        one
    }
}

/// The prompt (exposed for tests and for the plan's "why" view).
pub fn prompt(paths: &Paths, entries: &[InboxEntry]) -> String {
    let files = topic_files(&paths.snippets);
    let mut p = String::new();
    p.push_str(
        "Sort the new snippets waiting in tex/inbox.lua into this person's topic files.\n\
\n\
The folder is a LaTeX snippet collection for Neovim (LuaSnip). Each file tex/<topic>.lua holds one topic, one snippet per line, written with small helpers:\n\
  math / math_tab   maths, fires by itself / on Tab        suffix   maths, right after a letter (xsr → x^{2})\n\
  post              maths, wraps the letter before (xhat)   pat      maths, a Lua-pattern trigger (x2 → x_{2})\n\
  env               a maths \\begin…\\end pair               text     outside maths (prose, document structure)\n\
  vis               select text in maths, then a key\n\
In an output, <> is a tab stop, %1 a captured letter, %V the selection.\n\
\n\
Rules\n\
1. Every inbox entry gets exactly one move. Copy each id exactly.\n\
2. Choose by what the snippet PRODUCES (its output), not by its trigger's letters. A fraction goes where fractions are, a blackboard-bold letter where the other \\mathbb letters are.\n\
3. Prefer an existing file. Maths helpers (math, math_tab, suffix, post, pat, env) go to maths files; text(...) goes to text.lua (document structure, environments typed in prose); vis(...) goes to visual.lua.\n\
4. Keep related snippets together: after_trigger is the trigger of the most similar snippet ALREADY IN THAT FILE (same command family or same construction), copied exactly from that file's list below. If nothing there is close, leave after_trigger out: the entry goes at the end.\n\
5. A new file only for a clearly new topic that no file covers (for example probability notation when there is no probability file). Then file is a new lowercase name like \"probability.lua\" and new_title its title (\"Probability\"). Never make a new file for something an existing file already covers, and never use inbox.lua.\n\
6. reason: one short plain sentence, under 12 words, that a person scans in a list (\"fractions live there, next to //\").\n\
7. Everything you need is below. Use Read/Grep only if an entry is genuinely unclear; never edit.\n\
\n\
Examples of good placement (made-up entries):\n\
- math(\"tf\", [[\\tfrac{<>}{<>}]]) → operations.lua, after_trigger \"//\": both make fractions.\n\
- post(\"tld\", [[\\tilde{%1}]]) → accents.lua, after_trigger \"tilde\": accents on a letter.\n\
- math(\"HB\", [[\\mathbb{H}]]) → symbols.lua, after_trigger \"QQ\": the blackboard letters.\n\
- text(\"prf\", [[\\begin{proof}…\\end{proof}]], { tab = true, line = \"start\" }) → text.lua, after_trigger \"beg\": the other Tab environments.\n\
- math(\"EE\", [[\\mathbb{E}\\left[ <> \\right]]]) with no probability file → probability.lua, new_title \"Probability\", no after_trigger.\n\
\n\
Topic files (name, title, what its first line says, then trigger → output for every snippet):\n",
    );
    for f in &files {
        p.push_str(&format!("\n## {} — \"{}\": {}\n", f.name, f.title, f.first_comment));
        let list: Vec<String> = f.entries.iter().map(|e| format!("{} → {}", short(&e.trigger, 24), short(&e.body, 40))).collect();
        p.push_str("  ");
        p.push_str(&list.join("  |  "));
        p.push('\n');
    }
    p.push_str("\nInbox entries to sort:\n");
    for e in entries {
        p.push_str(&format!("\nid={}\n{}\n", e.id, e.source.trim_end()));
    }
    p.push_str("\nAnswer with the structured plan: one move per inbox entry, and notes (usually empty).\n");
    p
}

/// The JSON schema passed to --json-schema.
pub fn plan_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "moves": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "description": "the inbox entry id, exactly as given" },
                        "file": { "type": "string", "description": "target file in tex/, e.g. operations.lua" },
                        "after_trigger": { "type": "string", "description": "trigger of the snippet in that file to insert after; omit for the end" },
                        "new_title": { "type": "string", "description": "only for a new file: its group title" },
                        "reason": { "type": "string", "description": "one short sentence" }
                    },
                    "required": ["id", "file", "reason"],
                    "additionalProperties": false
                }
            },
            "notes": { "type": "string" }
        },
        "required": ["moves"],
        "additionalProperties": false
    })
}

fn valid_file_name(f: &str) -> bool {
    f.ends_with(".lua")
        && f != "inbox.lua"
        && !f.starts_with("zz_studio")
        && f.len() <= 64
        && f.trim_end_matches(".lua").chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        && !f.trim_end_matches(".lua").is_empty()
}

/// Turn the structured answer into a SortPlan: unknown ids, bad file names
/// and duplicate moves are dropped (and said so in the notes); an
/// after_trigger the target file doesn't have is cleared.
pub fn plan_from_value(v: &Value, entries: &[InboxEntry], snippets: &Path) -> Result<SortPlan, String> {
    let raw = v.get("moves").and_then(Value::as_array).ok_or("the answer has no moves")?;
    let ids: std::collections::HashSet<&str> = entries.iter().map(|e| e.id.as_str()).collect();
    let files = topic_files(snippets);
    let mut seen = std::collections::HashSet::new();
    let mut moves = Vec::new();
    let mut dropped = Vec::new();
    for m in raw {
        let id = m.get("id").and_then(Value::as_str).unwrap_or("").to_string();
        let file = m.get("file").and_then(Value::as_str).unwrap_or("").trim().trim_start_matches("tex/").to_string();
        if !ids.contains(id.as_str()) || !seen.insert(id.clone()) {
            dropped.push(format!("ignored a move for unknown or repeated id '{id}'"));
            continue;
        }
        if !valid_file_name(&file) {
            dropped.push(format!("'{id}' had an unusable file name '{file}'"));
            continue;
        }
        let existing = files.iter().find(|f| f.name == file);
        let mut after = m.get("after_trigger").and_then(Value::as_str).map(str::to_string).filter(|s| !s.is_empty());
        if let (Some(a), Some(f)) = (&after, existing) {
            if !f.entries.iter().any(|e| &e.trigger == a) {
                after = None;
            }
        }
        if existing.is_none() {
            after = None;
        }
        let new_title = if existing.is_none() {
            Some(m.get("new_title").and_then(Value::as_str).map(str::to_string).filter(|s| !s.trim().is_empty()).unwrap_or_else(|| {
                let stem = file.trim_end_matches(".lua").replace(['_', '-'], " ");
                let mut c = stem.chars();
                c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
            }))
        } else {
            None
        };
        let reason = m.get("reason").and_then(Value::as_str).unwrap_or("").trim().to_string();
        moves.push(SortMove { id, file, after_trigger: after, new_title, reason });
    }
    let mut notes = v.get("notes").and_then(Value::as_str).unwrap_or("").trim().to_string();
    if !dropped.is_empty() {
        if !notes.is_empty() {
            notes.push(' ');
        }
        notes.push_str(&format!("(Studio: {}.)", dropped.join("; ")));
    }
    Ok(SortPlan { moves, notes })
}

/// A progress line for a stream-json event, if it's worth showing.
pub fn progress_lines(v: &Value) -> Vec<String> {
    let mut out = Vec::new();
    match v.get("type").and_then(Value::as_str) {
        // (the studio announces the model itself when it starts the job)
        Some("assistant") => {
            for c in v.pointer("/message/content").and_then(Value::as_array).into_iter().flatten() {
                match c.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        let t = c.get("text").and_then(Value::as_str).unwrap_or("").trim();
                        if let Some(first) = t.lines().find(|l| !l.trim().is_empty()) {
                            out.push(short(first.trim(), 100));
                        }
                    }
                    Some("tool_use") => {
                        let input = c.get("input").cloned().unwrap_or(Value::Null);
                        let base = |p: &str| Path::new(p).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_else(|| p.to_string());
                        match c.get("name").and_then(Value::as_str) {
                            Some("Read") => out.push(format!("Reading {}", base(input["file_path"].as_str().unwrap_or("a file")))),
                            Some("Grep") => out.push(format!("Searching for “{}”", short(input["pattern"].as_str().unwrap_or(""), 40))),
                            Some("Glob") => out.push(format!("Listing {}", short(input["pattern"].as_str().unwrap_or("files"), 40))),
                            Some("StructuredOutput") => out.push("Writing the plan".into()),
                            Some(other) => out.push(format!("Using {other}")),
                            None => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    out
}

fn copy_snippets(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to.join("tex"))?;
    for e in std::fs::read_dir(from.join("tex"))?.flatten() {
        let n = e.file_name();
        let ns = n.to_string_lossy();
        // read + write, not fs::copy: an APFS clone shows up as an event on
        // the source folder, which her running ntex sessions would act on
        if ns.ends_with(".lua") && !ns.starts_with("zz_studio") {
            std::fs::write(to.join("tex").join(&n), std::fs::read(e.path())?)?;
        }
    }
    let readme = from.join("README.md");
    if readme.exists() {
        std::fs::write(to.join("README.md"), std::fs::read(readme)?)?;
    }
    Ok(())
}

fn claude_path() -> PathBuf {
    if let Ok(p) = std::env::var("NTEX_STUDIO_CLAUDE") {
        return PathBuf::from(p);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    for p in [home.join(".local/bin/claude"), PathBuf::from("/opt/homebrew/bin/claude"), PathBuf::from("/usr/local/bin/claude")] {
        if p.exists() {
            return p;
        }
    }
    PathBuf::from("claude")
}

/// The command line (without the prompt, which goes on stdin).
pub fn command_args(prefs: &StudioPrefs) -> Vec<String> {
    let mut a: Vec<String> = vec!["-p".into()];
    if std::env::var_os("ANTHROPIC_API_KEY").is_some() {
        a.push("--bare".into());
    }
    a.extend(
        [
            "--model",
            &prefs.sort_model,
            "--effort",
            &prefs.sort_effort,
            "--output-format",
            "stream-json",
            "--verbose",
            "--json-schema",
            &plan_schema().to_string(),
            "--tools",
            "Read,Grep,Glob",
            "--permission-mode",
            "dontAsk",
            "--safe-mode",
            "--strict-mcp-config",
            "--no-session-persistence",
            "--max-budget-usd",
            "1",
            "--system-prompt",
            SYSTEM_PROMPT,
            "--allowedTools",
            "Read",
            "Grep",
            "Glob",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    a
}

fn kill_child(child: &Arc<Mutex<Option<Child>>>) {
    if let Some(mut c) = child.lock().unwrap().take() {
        unsafe {
            libc::killpg(c.id() as i32, libc::SIGKILL);
        }
        let _ = c.kill();
        let _ = c.wait();
        // again after the wait: a grandchild forked while the first signal
        // was in flight is still in the group
        unsafe {
            libc::killpg(c.id() as i32, libc::SIGKILL);
        }
    }
}

impl SortJob {
    /// Start sorting the given entries. Events arrive on the receiver; the
    /// last one is Plan or Failed.
    pub fn start(paths: &Paths, prefs: &StudioPrefs, entries: &[InboxEntry]) -> std::io::Result<(SortJob, Receiver<AgentEvent>)> {
        use std::os::unix::process::CommandExt;
        let (tx, rx) = mpsc::channel();
        let tx_job = tx.clone();
        let base = std::env::var_os("TMPDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
        let workdir = base.join(format!(
            "ntex-studio-sort.{}.{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
        ));
        let folder = workdir.join("snippets");
        copy_snippets(&paths.snippets, &folder)?;
        let text = prompt(paths, entries);
        let mut cmd = Command::new(claude_path());
        cmd.args(command_args(prefs))
            .current_dir(&folder)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&workdir);
                return Err(e);
            }
        };
        let mut stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let child = Arc::new(Mutex::new(Some(child)));
        let done = Arc::new(AtomicBool::new(false));
        std::thread::spawn(move || {
            let _ = stdin.write_all(text.as_bytes());
            // dropping stdin closes it: claude starts
        });
        let err_buf = Arc::new(Mutex::new(String::new()));
        {
            let err_buf = err_buf.clone();
            std::thread::spawn(move || {
                for l in BufReader::new(stderr).lines().map_while(Result::ok) {
                    let mut b = err_buf.lock().unwrap();
                    if b.len() < 4000 {
                        b.push_str(&l);
                        b.push('\n');
                    }
                }
            });
        }
        // the watchdog: 180 s, then the whole group goes
        {
            let (child, done, tx) = (child.clone(), done.clone(), tx.clone());
            std::thread::spawn(move || {
                let t0 = Instant::now();
                while t0.elapsed() < TIMEOUT {
                    if done.load(Ordering::SeqCst) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                if !done.swap(true, Ordering::SeqCst) {
                    kill_child(&child);
                    let _ = tx.send(AgentEvent::Failed("Sorting took longer than 3 minutes, so it was stopped.".into()));
                }
            });
        }
        {
            let (child, done, entries, snippets, workdir) = (child.clone(), done.clone(), entries.to_vec(), paths.snippets.clone(), workdir.clone());
            std::thread::spawn(move || {
                read_stream(stdout, &tx, &done, &entries, &snippets);
                // the stream ended: reap the process, report if nothing was said yet
                let status = child.lock().unwrap().as_mut().and_then(|c| c.wait().ok());
                child.lock().unwrap().take();
                if !done.swap(true, Ordering::SeqCst) {
                    let err = err_buf.lock().unwrap().trim().to_string();
                    let why = if !err.is_empty() {
                        short(err.lines().last().unwrap_or(""), 200)
                    } else {
                        format!("claude stopped without a plan ({})", status.map(|s| s.to_string()).unwrap_or_else(|| "killed".into()))
                    };
                    let _ = tx.send(AgentEvent::Failed(why));
                }
                let _ = std::fs::remove_dir_all(&workdir);
            });
        }
        Ok((SortJob { child, done, workdir, tx: tx_job }, rx))
    }

    /// Stop claude (the whole process group). The receiver gets
    /// `Failed("Cancelled.")` unless a plan already arrived.
    pub fn cancel(&mut self) {
        let first = !self.done.swap(true, Ordering::SeqCst);
        kill_child(&self.child);
        let _ = std::fs::remove_dir_all(&self.workdir);
        if first {
            let _ = self.tx.send(AgentEvent::Failed("Cancelled.".into()));
        }
    }

    /// Is claude still running?
    pub fn running(&self) -> bool {
        self.child.lock().unwrap().is_some()
    }
}

impl Drop for SortJob {
    fn drop(&mut self) {
        self.done.store(true, Ordering::SeqCst);
        kill_child(&self.child);
        let _ = std::fs::remove_dir_all(&self.workdir);
    }
}

fn read_stream(stdout: std::process::ChildStdout, tx: &Sender<AgentEvent>, done: &AtomicBool, entries: &[InboxEntry], snippets: &Path) {
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        if done.load(Ordering::SeqCst) {
            continue;
        }
        if v.get("type").and_then(Value::as_str) == Some("result") {
            let cost = v.get("total_cost_usd").and_then(Value::as_f64);
            let ms = v.get("duration_ms").and_then(Value::as_f64);
            if let (Some(c), Some(ms)) = (cost, ms) {
                let _ = tx.send(AgentEvent::Progress(format!("Done in {:.0} s (${c:.4})", ms / 1000.0)));
            }
            let is_error = v.get("is_error").and_then(Value::as_bool).unwrap_or(false);
            let ev = if is_error {
                AgentEvent::Failed(short(v.get("result").and_then(Value::as_str).or(v.get("subtype").and_then(Value::as_str)).unwrap_or("claude reported an error"), 300))
            } else {
                let structured = v.get("structured_output").cloned().filter(|s| !s.is_null()).or_else(|| v.get("result").and_then(Value::as_str).and_then(|s| serde_json::from_str::<Value>(s).ok()));
                match structured {
                    Some(s) => match plan_from_value(&s, entries, snippets) {
                        Ok(p) => AgentEvent::Plan(p),
                        Err(e) => AgentEvent::Failed(e),
                    },
                    None => AgentEvent::Failed("claude answered without a plan".into()),
                }
            };
            if !done.swap(true, Ordering::SeqCst) {
                let _ = tx.send(ev);
            }
            continue;
        }
        for p in progress_lines(&v) {
            let _ = tx.send(AgentEvent::Progress(p));
        }
    }
}
