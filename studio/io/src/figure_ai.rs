//! The figure window's prompt box: rewrite a .fig source from an instruction.
//! Backends: Claude (`claude -p`) and OpenAI (`codex exec`, e.g. gpt-6-luna);
//! FIG-AI decides the defaults from the evaluation (figures/eval/REPORT.md).
//!
//! How it runs (pure text in, text out; nothing can touch her files):
//!   claude -p --model <m> --effort <e> --output-format json
//!     --tools ""                    no tools at all
//!     --permission-mode dontAsk --safe-mode --strict-mcp-config
//!     --disable-slash-commands      no hooks, plugins, MCP, CLAUDE.md, skills
//!     --no-session-persistence --system-prompt <system_prompt()>
//!   codex exec -m <m> -c model_reasoning_effort="<e>" -s read-only
//!     --skip-git-repo-check --ephemeral --ignore-user-config --ignore-rules
//!     --disable memories/shell_tool/apps/plugins/… -c skills.include_instructions=false
//!     [-c service_tier="priority"]  (off by default: see `codex_service_tier`)
//!     -o <answer file> -
//!   The prompt goes on stdin; cwd is a fresh empty temp dir; API-key
//!   variables are removed from the child's environment so both run on her
//!   subscriptions. The answer's ```fig block is parsed with
//!   `ntex_figures::parse::parse`; with errors, one retry feeds the
//!   diagnostics back. The whole ask is bounded by TIMEOUT (60 s); Cancel
//!   (and Drop) kill the process group.
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq)]
pub struct AskModel {
    /// "claude" | "codex"
    pub backend: String,
    /// "claude-sonnet-5-5", "gpt-6-luna", …
    pub model: String,
    /// "low" | "medium" | "high"
    pub effort: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AskEvent {
    Progress(String),
    /// The new .fig source (already parsed without errors) and a one-line summary.
    Done { source: String, summary: String, seconds: f32 },
    Failed(String),
}

/// Bumped when the system prompt changes (the eval records it).
pub const PROMPT_VERSION: u32 = 2;

/// The whole ask (both attempts) is stopped after this.
pub const TIMEOUT: Duration = Duration::from_secs(60);

/// TIMEOUT, or `NTEX_STUDIO_ASK_TIMEOUT_MS` (tests).
pub fn timeout() -> Duration {
    std::env::var("NTEX_STUDIO_ASK_TIMEOUT_MS").ok().and_then(|v| v.parse().ok()).map(Duration::from_millis).unwrap_or(TIMEOUT)
}

/// The Progress line sent before the second attempt starts (the eval counts it).
pub const RETRY_PREFIX: &str = "Fixing";

pub struct AskJob {
    child: Arc<Mutex<Option<Child>>>,
    done: Arc<AtomicBool>,
    workdir: PathBuf,
    tx: Sender<AskEvent>,
}

impl AskJob {
    pub fn start(source: &str, instruction: &str, model: &AskModel) -> std::io::Result<(AskJob, Receiver<AskEvent>)> {
        if model.backend != "claude" && model.backend != "codex" {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("unknown backend '{}'", model.backend)));
        }
        let (tx, rx) = mpsc::channel();
        let workdir = fresh_workdir()?;
        let child: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
        let done = Arc::new(AtomicBool::new(false));
        let job = AskJob { child: child.clone(), done: done.clone(), workdir: workdir.clone(), tx: tx.clone() };
        let t0 = Instant::now();
        let _ = tx.send(AskEvent::Progress(format!("Asking {}…", display_name(model))));
        // the watchdog: the whole ask, both attempts
        {
            let (child, done, tx) = (child.clone(), done.clone(), tx.clone());
            let limit = timeout();
            std::thread::spawn(move || {
                while t0.elapsed() < limit {
                    if done.load(Ordering::SeqCst) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                if !done.swap(true, Ordering::SeqCst) {
                    kill_child(&child);
                    let _ = tx.send(AskEvent::Failed(format!("No answer in {} s, so it was stopped.", limit.as_secs())));
                }
            });
        }
        let (source, instruction, model) = (source.to_string(), instruction.to_string(), model.clone());
        std::thread::spawn(move || {
            let ev = run(&source, &instruction, &model, &child, &done, &workdir, &tx, t0);
            if let Some(ev) = ev {
                if !done.swap(true, Ordering::SeqCst) {
                    let _ = tx.send(ev);
                }
            }
            let _ = std::fs::remove_dir_all(&workdir);
        });
        Ok((job, rx))
    }

    /// Stop the model (its whole process group). The receiver gets
    /// `Failed("Cancelled.")` unless an answer already arrived.
    pub fn cancel(&mut self) {
        let first = !self.done.swap(true, Ordering::SeqCst);
        kill_child(&self.child);
        let _ = std::fs::remove_dir_all(&self.workdir);
        if first {
            let _ = self.tx.send(AskEvent::Failed("Cancelled.".into()));
        }
    }

    /// Is a model process still running?
    pub fn running(&self) -> bool {
        self.child.lock().unwrap().is_some()
    }
}

impl Drop for AskJob {
    fn drop(&mut self) {
        self.done.store(true, Ordering::SeqCst);
        kill_child(&self.child);
        let _ = std::fs::remove_dir_all(&self.workdir);
    }
}

// ---------------------------------------------------------------------------
// the worker

#[allow(clippy::too_many_arguments)]
fn run(source: &str, instruction: &str, model: &AskModel, child: &Arc<Mutex<Option<Child>>>, done: &AtomicBool, workdir: &Path, tx: &Sender<AskEvent>, t0: Instant) -> Option<AskEvent> {
    let mut message = user_message(source, instruction);
    for attempt in 0..2 {
        let answer = match call(model, &message, attempt, child, done, workdir) {
            Ok(a) => a,
            Err(e) => return Some(AskEvent::Failed(e)),
        };
        if done.load(Ordering::SeqCst) {
            return None;
        }
        let (new_source, summary) = extract(&answer);
        let errors = check(&new_source);
        if errors.is_empty() {
            let summary = summary.unwrap_or_else(|| diff_summary(source, &new_source));
            return Some(AskEvent::Done { source: new_source, summary, seconds: t0.elapsed().as_secs_f32() });
        }
        if attempt == 0 {
            let _ = tx.send(AskEvent::Progress(format!("{RETRY_PREFIX} {} error{}…", errors.len(), if errors.len() == 1 { "" } else { "s" })));
            message = retry_message(source, instruction, &new_source, &errors);
        } else {
            return Some(AskEvent::Failed(format!("The answer still had errors: {}", short(&errors.join("; "), 240))));
        }
    }
    None
}

/// One model call: the answer text, or why there is none.
fn call(model: &AskModel, message: &str, attempt: usize, child: &Arc<Mutex<Option<Child>>>, done: &AtomicBool, workdir: &Path) -> Result<String, String> {
    use std::os::unix::process::CommandExt;
    let cwd = workdir.join(format!("run{attempt}"));
    std::fs::create_dir_all(&cwd).map_err(|e| format!("couldn't make a temp folder: {e}"))?;
    let answer_file = workdir.join(format!("answer{attempt}.txt"));
    let (program, args, stdin_text) = if model.backend == "claude" {
        (claude_path(), claude_args(model), message.to_string())
    } else {
        (codex_path(), codex_args(model, &answer_file), format!("{}\n\n{}", system_prompt(), message))
    };
    let mut cmd = Command::new(&program);
    cmd.args(&args).current_dir(&cwd).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).process_group(0);
    // her subscriptions, never an API key
    for k in ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "OPENAI_API_KEY", "CODEX_API_KEY"] {
        cmd.env_remove(k);
    }
    let (stdout, stderr, stdin) = {
        let mut slot = child.lock().unwrap();
        if done.load(Ordering::SeqCst) {
            return Err("Cancelled.".into());
        }
        let mut c = cmd.spawn().map_err(|e| format!("couldn't start {}: {e}", program.display()))?;
        let io = (c.stdout.take().unwrap(), c.stderr.take().unwrap(), c.stdin.take().unwrap());
        *slot = Some(c);
        io
    };
    {
        let text = stdin_text;
        let mut stdin = stdin;
        std::thread::spawn(move || {
            let _ = stdin.write_all(text.as_bytes());
        });
    }
    let out_t = std::thread::spawn(move || {
        let (mut s, mut r) = (String::new(), stdout);
        let _ = r.read_to_string(&mut s);
        s
    });
    let err_t = std::thread::spawn(move || {
        let (mut s, mut r) = (String::new(), stderr);
        let _ = r.read_to_string(&mut s);
        s
    });
    // wait for the exit (or for cancel / the watchdog to take the child)
    let status = loop {
        {
            let mut slot = child.lock().unwrap();
            match slot.as_mut() {
                None => break None,
                Some(c) => {
                    if let Ok(Some(st)) = c.try_wait() {
                        slot.take();
                        break Some(st);
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let out = out_t.join().unwrap_or_default();
    let err = err_t.join().unwrap_or_default();
    if done.load(Ordering::SeqCst) {
        return Err("Cancelled.".into());
    }
    let Some(status) = status else { return Err("Stopped.".into()) };
    if model.backend == "claude" {
        let v: serde_json::Value = match serde_json::from_str(out.trim()) {
            Ok(v) => v,
            Err(_) => return Err(last_line(&err).or_else(|| last_line(&out)).unwrap_or_else(|| format!("claude stopped without an answer ({status})"))),
        };
        let result = v.get("result").and_then(|r| r.as_str()).unwrap_or("").to_string();
        if v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false) {
            return Err(short(if result.is_empty() { v.get("subtype").and_then(|s| s.as_str()).unwrap_or("claude reported an error") } else { &result }, 300));
        }
        if result.trim().is_empty() {
            return Err("claude answered with nothing".into());
        }
        Ok(result)
    } else {
        let answer = std::fs::read_to_string(&answer_file).unwrap_or_default();
        if !status.success() || answer.trim().is_empty() {
            // codex prints its errors on stderr ("ERROR: …")
            let why = err.lines().rev().find(|l| l.contains("ERROR") || l.contains("error")).map(|l| short(l.trim(), 300)).or_else(|| last_line(&err)).unwrap_or_else(|| format!("codex stopped without an answer ({status})"));
            return Err(why);
        }
        Ok(answer)
    }
}

fn last_line(s: &str) -> Option<String> {
    s.lines().rev().map(str::trim).find(|l| !l.is_empty()).map(|l| short(l, 300))
}

fn short(s: &str, n: usize) -> String {
    let one = s.replace('\n', " ");
    if one.chars().count() > n { format!("{}…", one.chars().take(n).collect::<String>()) } else { one }
}

fn kill_child(child: &Arc<Mutex<Option<Child>>>) {
    if let Some(mut c) = child.lock().unwrap().take() {
        unsafe {
            libc::killpg(c.id() as i32, libc::SIGKILL);
        }
        let _ = c.kill();
        let _ = c.wait();
        // again: a grandchild forked while the first signal was in flight
        unsafe {
            libc::killpg(c.id() as i32, libc::SIGKILL);
        }
    }
}

fn fresh_workdir() -> std::io::Result<PathBuf> {
    let base = std::env::var_os("TMPDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = base.join(format!(
        "ntex-figure-ask.{}.{}.{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn find_program(env: &str, name: &str, extra: &[PathBuf]) -> PathBuf {
    if let Ok(p) = std::env::var(env) {
        return PathBuf::from(p);
    }
    for p in extra {
        if p.exists() {
            return p.clone();
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for d in std::env::split_paths(&path) {
            let p = d.join(name);
            if p.is_file() {
                return p;
            }
        }
    }
    PathBuf::from(name)
}

fn claude_path() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    find_program("NTEX_STUDIO_CLAUDE", "claude", &[home.join(".local/bin/claude"), PathBuf::from("/opt/homebrew/bin/claude"), PathBuf::from("/usr/local/bin/claude")])
}

fn codex_path() -> PathBuf {
    find_program("NTEX_STUDIO_CODEX", "codex", &[PathBuf::from("/opt/homebrew/bin/codex"), PathBuf::from("/usr/local/bin/codex")])
}

/// The AskModel for a model name from prefs (`figure_model`, `figure_fast_model`):
/// "claude-…" and the Claude aliases run through `claude -p`, anything else
/// ("gpt-…") through `codex exec`.
pub fn ask_model(model: &str, effort: &str) -> AskModel {
    let m = model.trim();
    let claude = m.starts_with("claude") || ["sonnet", "haiku", "opus", "fable"].iter().any(|a| m.eq_ignore_ascii_case(a));
    AskModel { backend: if claude { "claude" } else { "codex" }.into(), model: m.into(), effort: effort.trim().into() }
}

/// The models the window's menu offers (model id, menu name), evaluated in
/// figures/eval/REPORT.md. Prefs may name any other model.
pub const MENU_MODELS: &[(&str, &str)] = &[("claude-sonnet-5-5", "Sonnet"), ("claude-haiku-5-5", "Haiku"), ("gpt-6-luna", "Luna"), ("gpt-6.1-sol", "Sol")];

/// A short name for progress lines: "Sonnet", "Luna", …
pub fn display_name(model: &AskModel) -> String {
    let m = model.model.as_str();
    for (k, n) in [("haiku", "Haiku"), ("sonnet", "Sonnet"), ("opus", "Opus"), ("fable", "Fable"), ("luna", "Luna"), ("sol", "Sol"), ("astra", "Astra"), ("reserve", "Reserve")] {
        if m.contains(k) {
            return n.into();
        }
    }
    m.to_string()
}

/// The `claude` command line (the message goes on stdin).
pub fn claude_args(model: &AskModel) -> Vec<String> {
    [
        "-p",
        "--model",
        &model.model,
        "--effort",
        &model.effort,
        "--output-format",
        "json",
        "--tools",
        "",
        "--permission-mode",
        "dontAsk",
        "--safe-mode",
        "--strict-mcp-config",
        "--disable-slash-commands",
        "--no-session-persistence",
        "--max-budget-usd",
        "1",
        "--system-prompt",
        &system_prompt(),
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// The service tier for a codex model: "priority" for the models in
/// PRIORITY_MODELS, else the default. `NTEX_STUDIO_CODEX_TIER=priority|default`
/// overrides (the eval). The eval found no gain for these short answers
/// (Luna low, sequential A/B: median 4.03 s default vs 3.93 s priority, same
/// mean), so nothing runs on priority by default.
pub fn codex_service_tier(model: &AskModel) -> Option<String> {
    match std::env::var("NTEX_STUDIO_CODEX_TIER").ok().as_deref() {
        Some("priority") => return Some("priority".into()),
        Some("default") | Some("") => return None,
        _ => {}
    }
    PRIORITY_MODELS.contains(&model.model.as_str()).then(|| "priority".to_string())
}

/// Codex models that run on the priority tier by default (none: see above).
pub const PRIORITY_MODELS: &[&str] = &[];

/// The `codex exec` command line (the prompt goes on stdin).
pub fn codex_args(model: &AskModel, answer_file: &Path) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "exec".into(),
        "-m".into(),
        model.model.clone(),
        "-c".into(),
        format!("model_reasoning_effort=\"{}\"", model.effort),
        "-s".into(),
        "read-only".into(),
        "--skip-git-repo-check".into(),
        "--ephemeral".into(),
        "--ignore-user-config".into(),
        "--ignore-rules".into(),
    ];
    // a text-only task: no tools, no memories, no skills list, no extra instructions
    for f in [
        "memories", "shell_tool", "unified_exec", "apps", "plugins", "multi_agent", "image_generation", "browser_use", "computer_use", "hooks", "view_image",
        "skill_search", "tool_suggest", "goals", "sleep_tool", "code_mode_host",
    ] {
        a.push("--disable".into());
        a.push(f.into());
    }
    for c in [
        "skills.include_instructions=false",
        "skills.bundled.enabled=false",
        "include_permissions_instructions=false",
        "include_collaboration_mode_instructions=false",
        "include_apps_instructions=false",
        "web_search=\"disabled\"",
    ] {
        a.push("-c".into());
        a.push(c.into());
    }
    if let Some(t) = codex_service_tier(model) {
        a.push("-c".into());
        a.push(format!("service_tier=\"{t}\""));
    }
    a.push("-o".into());
    a.push(answer_file.to_string_lossy().into_owned());
    a.push("-".into());
    a
}

// ---------------------------------------------------------------------------
// prompts

/// The user message: the instruction and the current file.
pub fn user_message(source: &str, instruction: &str) -> String {
    let src = if source.trim().is_empty() { "(empty file)\n".to_string() } else { fence_body(source) };
    format!("Instruction: {}\n\nCurrent file:\n```fig\n{}```\n\nAnswer with the complete new file in one ```fig block, then the Summary line.\n", instruction.trim(), src)
}

/// The second attempt: the same task, the previous answer and its errors.
pub fn retry_message(source: &str, instruction: &str, previous: &str, errors: &[String]) -> String {
    let mut m = user_message(source, instruction);
    m.push_str(&format!(
        "\nYour previous answer was:\n```fig\n{}```\nIt does not parse. The parser said:\n{}\n\nFix these errors (use only the syntax in the reference) and answer again with the complete file in one ```fig block, then the Summary line.\n",
        fence_body(previous),
        errors.iter().map(|e| format!("- {e}")).collect::<Vec<_>>().join("\n")
    ));
    m
}

fn fence_body(s: &str) -> String {
    let mut b = s.to_string();
    if !b.ends_with('\n') {
        b.push('\n');
    }
    b
}

/// Split an answer into the .fig source and the model's one-line summary.
/// The source is the ```fig (or ```ntexfig / untagged) fenced block if there
/// is one, else the whole answer minus a trailing `Summary:` line.
pub fn extract(answer: &str) -> (String, Option<String>) {
    let mut blocks: Vec<(String, String)> = Vec::new(); // (tag, body)
    let mut summary = None;
    let mut in_block: Option<(String, Vec<&str>, usize)> = None; // (tag, lines, fence length)
    let mut outside: Vec<&str> = Vec::new();
    for line in answer.lines() {
        let t = line.trim_start();
        match &mut in_block {
            None => {
                let ticks = t.chars().take_while(|&c| c == '`').count();
                if ticks >= 3 {
                    in_block = Some((t[ticks..].trim().to_lowercase(), Vec::new(), ticks));
                } else {
                    if let Some(s) = t.strip_prefix("Summary:").or_else(|| t.strip_prefix("**Summary:**")).or_else(|| t.strip_prefix("summary:")) {
                        summary = Some(s.trim().to_string()).filter(|s| !s.is_empty());
                    } else {
                        outside.push(line);
                    }
                }
            }
            Some((tag, lines, n)) => {
                let ticks = t.chars().take_while(|&c| c == '`').count();
                if ticks >= *n && t[ticks..].trim().is_empty() {
                    let mut body = lines.join("\n");
                    body.push('\n');
                    blocks.push((tag.clone(), body));
                    in_block = None;
                } else {
                    lines.push(line);
                }
            }
        }
    }
    // an unclosed fence: take what's there
    if let Some((tag, lines, _)) = in_block {
        let mut body = lines.join("\n");
        body.push('\n');
        blocks.push((tag, body));
    }
    let pick = blocks
        .iter()
        .rev()
        .find(|(t, _)| t == "fig" || t == "ntexfig" || t == ".fig")
        .or_else(|| blocks.iter().filter(|(t, _)| t.is_empty() || t == "text" || t == "plain").max_by_key(|(_, b)| b.len()))
        .or_else(|| blocks.iter().max_by_key(|(_, b)| b.len()));
    let source = match pick {
        Some((_, b)) => b.clone(),
        None => {
            let mut s = outside.join("\n").trim_matches('\n').to_string();
            s.push('\n');
            s
        }
    };
    let summary = summary.map(|s| short(s.trim_matches('*').trim(), 140));
    (source, summary)
}

/// Parse errors in a source, as "line N: …" messages (empty = clean). An
/// empty figure counts as an error: the answer must be a whole file.
pub fn check(source: &str) -> Vec<String> {
    if source.trim().is_empty() {
        return vec!["the answer is empty: return the whole file".into()];
    }
    use ntex_figures::model::Severity;
    let parsed = std::panic::catch_unwind(|| ntex_figures::parse::parse(source));
    match parsed {
        Ok((fig, diags)) => {
            let mut errs: Vec<String> = diags
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .map(|d| if d.message.starts_with("line ") || d.span.line == 0 { d.message.clone() } else { format!("line {}: {}", d.span.line, d.message) })
                .collect();
            let drawable = fig.items.iter().any(|i| !matches!(i, ntex_figures::model::Item::Comment { .. }));
            if errs.is_empty() && !drawable && source.lines().any(|l| { let t = l.trim(); !t.is_empty() && !t.starts_with('%') }) {
                errs.push("nothing in the answer could be read as a figure".into());
            }
            errs
        }
        // the parser isn't there yet (or panicked): a structural check
        Err(_) => structural_check(source),
    }
}

/// Balanced quotes and braces per line, `grid` … `end` paired. Only used if
/// the real parser is unavailable.
fn structural_check(source: &str) -> Vec<String> {
    let mut errs = Vec::new();
    let mut in_grid = None;
    for (i, raw) in source.lines().enumerate() {
        let n = i + 1;
        let line = raw.split('%').next().unwrap_or("");
        let t = line.trim();
        if t == "grid" {
            in_grid = Some(n);
            continue;
        }
        if t == "end" {
            if in_grid.take().is_none() {
                errs.push(format!("line {n}: `end` without `grid`"));
            }
            continue;
        }
        if in_grid.is_some() {
            continue;
        }
        if line.matches('"').count() % 2 == 1 {
            errs.push(format!("line {n}: unbalanced quote"));
        }
        let (o, c) = (line.matches('{').count(), line.matches('}').count());
        if o != c {
            errs.push(format!("line {n}: unbalanced braces"));
        }
    }
    if let Some(n) = in_grid {
        errs.push(format!("line {n}: `grid` without `end`"));
    }
    errs
}

/// A one-line summary computed from the two sources (when the model gave none).
pub fn diff_summary(before: &str, after: &str) -> String {
    let norm = |s: &str| -> Vec<String> { s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect() };
    let (a, b) = (norm(before), norm(after));
    let added = b.iter().filter(|l| !a.contains(l)).count();
    let removed = a.iter().filter(|l| !b.contains(l)).count();
    match (added, removed) {
        (0, 0) => "No change.".into(),
        (x, 0) => format!("{x} line{} added.", if x == 1 { "" } else { "s" }),
        (0, y) => format!("{y} line{} removed.", if y == 1 { "" } else { "s" }),
        (x, y) => format!("{x} line{} changed or added, {y} replaced.", if x == 1 { "" } else { "s" }),
    }
}

/// The system prompt (the .fig reference + rules), exposed for the eval.
pub fn system_prompt() -> String {
    SYSTEM_PROMPT.to_string()
}

const SYSTEM_PROMPT: &str = r#"You edit figure files for a mathematician's LaTeX notes. A figure is a small text file in the ".fig" shorthand below; it compiles to TikZ. You get an instruction and the current file, and you answer with the complete new file.

ANSWER FORMAT
One fenced block ```fig … ``` holding the WHOLE new file, then one line `Summary: <what changed, under 12 words>`. Nothing else. No tools, no questions: if something is unclear, make the most sensible small change.

EDITING RULES
1. Return the whole file, never a diff or a fragment.
2. Keep everything the instruction doesn't touch exactly as it is: every % comment, blank line, statement, its order, node names, labels, options, positions, grid art.
3. Change only what was asked. No tidying, renaming, re-placing or relabelling of anything else (unless the instruction asks for a tidy-up). Put new statements next to related ones, or at the end.
4. Place new things with grid cells (`X at 1,0`), not absolute positions; keep any absolute positions already there.
5. Use only the syntax below. Names are unique. Labels are LaTeX maths without $.

THE SHORTHAND
Lines; `%` comments to end of line; `;` also separates statements. Keywords lower case.

Names and labels
- A node is a token: no spaces, arrows or any of , ; { } " (balanced braces inside are fine: \mathbb{Z}, G^{ab}). Examples: A, G/K, Z_n, H', \ker\varphi.
- Its label is the token itself (as LaTeX maths), unless a quoted label follows its FIRST mention: A "\mathbb{Z}/n\mathbb{Z}". First mention wins; later mentions just refer to it.
- Every mention of a name is the SAME node. In `1 -> K` … `Q -> 1` the last arrow goes back to the first 1, making a cycle. Two different nodes that both show "1" need two names: `1 -> K` … `Q -> 1' "1"`.
- Unicode is accepted: φ → \varphi, ℤ → \mathbb{Z}, ∘ × ⊗ ≅ ↦ →, x₁ → x_{1}.

Arrows: FROM <arrow> TO [options]
  A -> B                     arrow;  A -f-> B  labelled;  A -"g \circ f"-> B  (quote labels with spaces or dashes)
  A ->> B, A -π->> B         surjection (two heads)
  A >-> B, A >-ι-> B         injection (tail)
  A hook-> B, A -ι-hook-> B  inclusion (hook)
  A => B, A =f=> B           double arrow
  A --> B, A --f--> B        dashed (exists)
  A ..> B, A .f.> B          dotted
  B <-f- A                   reversed (same as A -f-> B)
  A <-> B, A <-f-> B         both ways
  A = B                      equals (double line, no head)
  g |-> h  or  g ↦ h          maps-to, element to element
Options after TO, any order: bend | bend-left | bend-right | bend=40 | loop (self-arrow: A -e-> A loop) | swap (label on the other side) | iso (adds ≅) | dashed | dotted | thick | color=NAME | near-start | near-end
Colours: blue violet teal maroon green ochre grey ink.
One map = one statement: put all its options on that one line (`A --f--> B` is already dashed and labelled). Never write the same arrow twice; to change an arrow, edit its statement.

Placement
- A at 1,0     grid cell: column 1, row 0; rows go DOWN (row 1 is below row 0). Fractions ok: A at 0.5,1.
- A at (3.2,-1)  absolute cm, y up (avoid for new things).
- Unplaced nodes are auto-placed (a 3-cycle becomes a triangle, a 4-cycle a square, chains go left to right).
- spacing 2.8,2   cell size in cm (default 1.9,1.35).

Grid blocks (ASCII art); columns are found by lining up tokens:
  grid
    A --f--> B
    |g       |h
    v        v
    C --k--> D
  end
- Horizontal: --f--> solid, <--f-- reversed, -->> surjection, >--> injection, - - > dashed, ..> dotted; the label sits between the dashes.
- Vertical: a | with its label right after it (|g), ending in v (down) or starting with ^ (up), under the node it leaves; ‖ or || is an equals sign; : instead of | is dashed.
- Diagonals: \label (down-right) or /label (down-left) on a between-line. Arrows the art can't show go after `end` as ordinary arrow statements using the grid's node names.
- To edit a grid arrow, edit the art. The art can only show plain, dashed, dotted, surjection, injection, up/down and equals arrows with short labels (no spaces). For anything else (a hook, bend, colour, iso, a label with spaces) leave that arrow out of the art and write it as a statement after `end`, e.g. `A -i-hook-> C`.

Blobs (sets, functions, kernels, images)
  blob NAME ["label"] [{elements}] [in PARENT] [options]
  blob G "G" {e, g, g^2, g^3}
  blob K "\ker\varphi" in G shaded {e, g^2}
- Elements are points (names unique in the whole figure). An element listed in a child blob is also in its parent: never list it in both; move it to the child.
- Options: shaded | color=NAME | shape=round|tall|wide|bean | size=w,h | at … | label=above|below|left|right|inside
- Arrows between blobs (G -φ-> H) are drawn blob to blob; between elements (g |-> h) point to point.

N-gons (dihedral pictures)
  ngon P 5                     vertices 1..5 counter-clockwise from the top (even n: from the top-left, on a flat bottom edge)
  ngon P 5 labels a,b,c,d,e axes rotation
  ngon Q 6 axis 1 axis 2.5 rotation=2
- labels x,y,… (default 1..n); axes = every reflection axis (dashed); axis k = the axis through vertex k; for even n, axis k.5 = through the midpoint of edge k.
- turn=k: the polygon drawn as it is after r^k (this is how to show "after r"); flip=k: as it is after the reflection in axis k. rotation[=k] only adds an arc arrow labelled r or r^k; it does not move anything.  at …, radius=r.
- Several ngons in a row show a sequence (give each its own name).

Cayley graphs
  cayley D4 <r, s>
  cayley <a, b | a^4, b^2, (ab)^2>      from a presentation (≤ 96 elements)
  cayley Z6 <1>                         additive
- Known groups: Cn/Zn, Dn (order 2n), V4/Z2xZ2, Q8, S3, S4, A4, products ZmxZn (write them without spaces: Z2xZ4).
- Always give the generators in <…>: cayley Q8 <i, j>.
- Options: layout=circle|prism|grid, legend, labels=none. Each generator gets its own colour automatically.

Other: text "a note" at (x,y);  scale 1.2;  size small|normal|large (label size).

EXAMPLES

Instruction: h should point up
Current file:
```fig
% the square from lemma 2
grid
  A --f--> B
  |g       |h
  v        v
  C --k--> D
end
```
Answer:
```fig
% the square from lemma 2
grid
  A --f--> B
  |g       ^h
  v        |
  C --k--> D
end
```
Summary: h now points up, from D to B.

Instruction: put 2 and 4 in a subset E inside X, green
Current file:
```fig
blob X "X" {1, 2, 3, 4}
blob Y "Y" {a, b}
X -f-> Y   % the map
```
Answer:
```fig
blob X "X" {1, 3}
blob E "E" in X color=green {2, 4}
blob Y "Y" {a, b}
X -f-> Y   % the map
```
Summary: added a green subset E = {2, 4} inside X.

Instruction: make g an injection and put C to the right of B, and add the dashed map C to D
Current file:
```fig
A -f-> B
A -g-> C
A at 0,0; B at 1,0; C at 0,1
D at 1,1
```
Answer:
```fig
A -f-> B
A >-g-> C
C --> D
A at 0,0; B at 1,0; C at 2,0
D at 1,1
```
Summary: g is an injection, C moved right of B, dashed C to D.

Instruction: show the hexagon flipped across the axis through 2, next to it
Current file:
```fig
% D6
ngon P 6 axis 2
```
Answer:
```fig
% D6
ngon P 6 axis 2
ngon Q 6 flip=2
```
Summary: added a copy flipped across the axis through vertex 2.
"#;
