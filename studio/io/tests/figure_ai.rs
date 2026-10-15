//! The figure Ask box: extraction, prompts, command lines, and the process
//! side with fake `claude` / `codex` scripts (no network): answer, retry,
//! cancel, timeout, errors. Two real calls (Luna, Sonnet) on request:
//!   cargo test -p ntex-studio-io --test figure_ai real_ -- --ignored --nocapture
mod common;
use common::*;
use ntex_studio_io::figure_ai::*;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Tests that set the fake-program variables run one at a time.
static ENV: Mutex<()> = Mutex::new(());

fn lock() -> std::sync::MutexGuard<'static, ()> {
    ENV.lock().unwrap_or_else(|e| e.into_inner())
}

fn claude(model: &str) -> AskModel {
    AskModel { backend: "claude".into(), model: model.into(), effort: "low".into() }
}

fn codex(model: &str) -> AskModel {
    AskModel { backend: "codex".into(), model: model.into(), effort: "low".into() }
}

const SQUARE: &str = "% a square\ngrid\n  A --f--> B\n  |        |\n  v        v\n  C --k--> D\nend\n";
const LABELLED: &str = "% a square\ngrid\n  A --f--> B\n  |g       |h\n  v        v\n  C --k--> D\nend\n";
/// Not a figure in any reading: the parser reports an error.
const BROKEN: &str = "A -\"unclosed-> B {\ngrid\n";

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

/// A fake `claude -p --output-format json` printing `answer` as its result.
fn fake_claude_answer(dir: &Path, answers: &[&str]) -> PathBuf {
    // answers[i] for the i-th call (the counter lives in dir/n)
    let mut body = format!("cat > \"{d}/stdin.$$\"\npwd > \"{d}/cwd\"\nls -A > \"{d}/cwd-listing\"\nn=$(cat \"{d}/n\" 2>/dev/null || echo 0)\necho $((n+1)) > \"{d}/n\"\ncp \"{d}/stdin.$$\" \"{d}/stdin$n\"\nprintf '%s\\n' \"$@\" > \"{d}/args$n\"\n", d = dir.display());
    for (i, a) in answers.iter().enumerate() {
        let json = serde_json::json!({"type":"result","subtype":"success","is_error":false,"result": a,"total_cost_usd":0.001,"duration_ms":900}).to_string();
        body.push_str(&format!("if [ \"$n\" = {i} ]; then cat <<'EOF'\n{json}\nEOF\nexit 0; fi\n"));
    }
    body.push_str("echo 'no more answers' >&2; exit 1");
    script(dir, "fake-claude", &body)
}

fn collect(rx: &std::sync::mpsc::Receiver<AskEvent>, secs: u64) -> Vec<AskEvent> {
    let mut evs = Vec::new();
    while let Ok(ev) = rx.recv_timeout(Duration::from_secs(secs)) {
        let last = matches!(ev, AskEvent::Done { .. } | AskEvent::Failed(_));
        evs.push(ev);
        if last {
            break;
        }
    }
    evs
}

fn no_orphan(pattern: &str) {
    let out = std::process::Command::new("pgrep").args(["-f", pattern]).output().unwrap();
    for p in String::from_utf8_lossy(&out.stdout).lines() {
        let ppid = std::process::Command::new("ps").args(["-o", "ppid=", "-p", p.trim()]).output().unwrap();
        assert_ne!(String::from_utf8_lossy(&ppid.stdout).trim(), "1", "orphaned {pattern} ({p})");
    }
}

// ---------------------------------------------------------------------------
// pure

#[test]
fn extraction() {
    // a fig block and a summary
    let (s, sum) = extract("Here you go.\n```fig\nA -> B\n```\nSummary: an arrow.\n");
    assert_eq!(s, "A -> B\n");
    assert_eq!(sum.as_deref(), Some("an arrow."));
    // an untagged fence, no summary
    let (s, sum) = extract("```\nA -> B\nB -> C\n```");
    assert_eq!(s, "A -> B\nB -> C\n");
    assert_eq!(sum, None);
    // no fence: the whole answer, minus the summary line
    let (s, sum) = extract("A -> B\n% kept\nSummary: plain\n");
    assert_eq!(s, "A -> B\n% kept\n");
    assert_eq!(sum.as_deref(), Some("plain"));
    // the fig block wins over a longer block in another language
    let (s, _) = extract("```latex\n\\begin{tikzcd} A \\arrow[r] & B \\end{tikzcd}\n```\n```fig\nA -> B\n```\n");
    assert_eq!(s, "A -> B\n");
    // an unclosed fence is taken as it is; indentation and grid art kept
    let (s, _) = extract("```fig\ngrid\n  A --f--> B\nend\n");
    assert_eq!(s, "grid\n  A --f--> B\nend\n");
    // a four-backtick fence holding three backticks inside stays whole
    let (s, _) = extract("````fig\ntext \"```\" at (0,0)\n````\n");
    assert_eq!(s, "text \"```\" at (0,0)\n");
    // markdown-bold summary
    let (_, sum) = extract("```fig\nA -> B\n```\n**Summary:** bold\n");
    assert_eq!(sum.as_deref(), Some("bold"));
}

#[test]
fn checks() {
    assert!(check(LABELLED).is_empty(), "{:?}", check(LABELLED));
    assert!(!check(BROKEN).is_empty());
    assert!(!check("").is_empty(), "an empty answer is an error");
    for e in check(BROKEN) {
        assert!(e.starts_with("line ") || e.contains("answer"), "{e}");
    }
}

#[test]
fn prompts_are_self_contained_and_private() {
    let sp = system_prompt();
    for must in ["whole file", "```fig", "Summary:", "grid", "blob", "ngon", "cayley", "->>", "hook->", "|->", "shaded", "axes", "rotation", "layout=", "EXAMPLES", "Change only what was asked", "grid cells"] {
        assert!(sp.contains(must), "system prompt lacks {must:?}");
    }
    let m = user_message(SQUARE, "label the vertical arrows g and h");
    assert!(m.contains("label the vertical arrows g and h") && m.contains("```fig\n% a square\ngrid") && m.contains("end\n```"));
    let r = retry_message(SQUARE, "x", BROKEN, &["line 1: bad".into()]);
    assert!(r.contains("line 1: bad") && r.contains("unclosed"));
    // nothing about the user or their machine
    let home = home().to_string_lossy().to_string();
    for p in [&sp, &m, &r] {
        assert!(!p.contains(&home), "no home path");
        assert!(!p.split_whitespace().any(|w| w.contains('@') && w.contains('.')), "no email address");
    }
    println!("system prompt: {} chars, ~{} tokens", sp.len(), sp.len() / 4);
}

#[test]
fn command_lines() {
    let _g = lock();
    let a = claude_args(&AskModel { backend: "claude".into(), model: "claude-sonnet-5-5".into(), effort: "medium".into() });
    let j = a.join(" ");
    for must in ["-p", "--model claude-sonnet-5-5", "--effort medium", "--output-format json", "--permission-mode dontAsk", "--safe-mode", "--strict-mcp-config", "--disable-slash-commands", "--no-session-persistence", "--system-prompt"] {
        assert!(j.contains(must), "claude args lack {must}");
    }
    let t = a.iter().position(|x| x == "--tools").unwrap();
    assert_eq!(a[t + 1], "", "no tools at all");
    assert!(!a.iter().any(|x| x == "--bare"), "--bare needs an API key; never");
    unsafe { std::env::remove_var("NTEX_STUDIO_CODEX_TIER") };
    let c = codex_args(&AskModel { backend: "codex".into(), model: "gpt-6-luna".into(), effort: "low".into() }, Path::new("/tmp/x/answer.txt"));
    let j = c.join(" ");
    for must in ["exec", "-m gpt-6-luna", "model_reasoning_effort=\"low\"", "-s read-only", "--skip-git-repo-check", "--ephemeral", "--ignore-user-config", "--ignore-rules", "--disable memories", "--disable shell_tool", "skills.include_instructions=false", "-o /tmp/x/answer.txt"] {
        assert!(j.contains(must), "codex args lack {must}: {j}");
    }
    assert_eq!(c.last().unwrap(), "-", "the prompt comes on stdin");
    // the tier: default (the eval found priority no faster here), overridable
    assert!(!j.contains("service_tier"));
    unsafe { std::env::set_var("NTEX_STUDIO_CODEX_TIER", "priority") };
    assert!(codex_args(&codex("gpt-6-luna"), Path::new("/tmp/a")).join(" ").contains("service_tier=\"priority\""));
    unsafe { std::env::remove_var("NTEX_STUDIO_CODEX_TIER") };
    assert_eq!(display_name(&codex("gpt-6-luna")), "Luna");
    assert_eq!(ask_model("gpt-6-luna", "low"), codex("gpt-6-luna"));
    assert_eq!(ask_model("claude-sonnet-5-5", "low"), claude("claude-sonnet-5-5"));
    assert_eq!(ask_model(" sonnet ", "medium").backend, "claude");
    assert_eq!(ask_model("gpt-6.1-sol", "low").backend, "codex");
    assert_eq!(display_name(&claude("claude-sonnet-5-5")), "Sonnet");
    // an unknown backend is refused up front
    assert!(AskJob::start(SQUARE, "x", &AskModel { backend: "ollama".into(), model: "m".into(), effort: "low".into() }).is_err());
}

// ---------------------------------------------------------------------------
// the process side, with fakes

#[test]
fn claude_answer_and_retry() {
    let _g = lock();
    let t = TempDir::new("fig-ai-claude");
    // 1. a clean answer: Done with the model's summary; prompt on stdin; an empty cwd
    let good = format!("```fig\n{LABELLED}```\nSummary: labelled g and h.");
    let s = fake_claude_answer(t.path(), &[&good]);
    unsafe { std::env::set_var("NTEX_STUDIO_CLAUDE", &s) };
    let (job, rx) = AskJob::start(SQUARE, "label the vertical arrows g and h", &claude("claude-sonnet-5-5")).unwrap();
    let evs = collect(&rx, 10);
    println!("{evs:?}");
    assert_eq!(evs.first(), Some(&AskEvent::Progress("Asking Sonnet…".into())));
    let Some(AskEvent::Done { source, summary, seconds }) = evs.last() else { panic!("{evs:?}") };
    assert_eq!(source, LABELLED);
    assert_eq!(summary, "labelled g and h.");
    assert!(*seconds < 10.0);
    let stdin = std::fs::read_to_string(t.path().join("stdin0")).unwrap();
    assert!(stdin.contains("label the vertical arrows g and h") && stdin.contains("|        |"));
    let cwd = std::fs::read_to_string(t.path().join("cwd")).unwrap();
    assert!(cwd.contains("ntex-figure-ask."), "{cwd}");
    assert_eq!(std::fs::read_to_string(t.path().join("cwd-listing")).unwrap(), "", "the model's folder is empty");
    let args = std::fs::read_to_string(t.path().join("args0")).unwrap();
    assert!(args.contains("--system-prompt") && args.contains("--safe-mode"));
    drop(job);
    std::thread::sleep(Duration::from_millis(100));
    assert!(!Path::new(cwd.trim()).exists(), "the temp folder is gone");

    // 2. a broken first answer: one retry with the diagnostics, then Done
    let _ = std::fs::remove_file(t.path().join("n"));
    let bad = format!("```fig\n{BROKEN}```\nSummary: oops");
    let s = fake_claude_answer(t.path(), &[&bad, &good]);
    unsafe { std::env::set_var("NTEX_STUDIO_CLAUDE", &s) };
    let (_job, rx) = AskJob::start(SQUARE, "label the vertical arrows g and h", &claude("claude-haiku-5-5")).unwrap();
    let evs = collect(&rx, 10);
    println!("{evs:?}");
    assert!(evs.iter().any(|e| matches!(e, AskEvent::Progress(p) if p.starts_with(RETRY_PREFIX))), "{evs:?}");
    assert!(matches!(evs.last(), Some(AskEvent::Done { source, .. }) if source == LABELLED));
    let second = std::fs::read_to_string(t.path().join("stdin1")).unwrap();
    assert!(second.contains("Your previous answer was") && second.contains("unclosed") && second.contains("line "), "{second}");

    // 3. broken twice: Failed, with the errors, after exactly two calls
    let _ = std::fs::remove_file(t.path().join("n"));
    let s = fake_claude_answer(t.path(), &[&bad, &bad, &good]);
    unsafe { std::env::set_var("NTEX_STUDIO_CLAUDE", &s) };
    let (_job, rx) = AskJob::start(SQUARE, "x", &claude("claude-haiku-5-5")).unwrap();
    let evs = collect(&rx, 10);
    assert!(matches!(evs.last(), Some(AskEvent::Failed(f)) if f.starts_with("The answer still had errors")), "{evs:?}");
    assert_eq!(std::fs::read_to_string(t.path().join("n")).unwrap().trim(), "2");

    // 4. claude fails: its last stderr line
    let s = script(t.path(), "fake-claude-fail", "cat > /dev/null\necho 'Not logged in' >&2\nexit 1");
    unsafe { std::env::set_var("NTEX_STUDIO_CLAUDE", &s) };
    let (_job, rx) = AskJob::start(SQUARE, "x", &claude("claude-haiku-5-5")).unwrap();
    assert_eq!(collect(&rx, 10).last(), Some(&AskEvent::Failed("Not logged in".into())));

    // 5. claude reports an error result
    let json = serde_json::json!({"type":"result","subtype":"error_during_execution","is_error":true,"result":"Overloaded"}).to_string();
    let s = script(t.path(), "fake-claude-err", &format!("cat > /dev/null\ncat <<'EOF'\n{json}\nEOF"));
    unsafe { std::env::set_var("NTEX_STUDIO_CLAUDE", &s) };
    let (_job, rx) = AskJob::start(SQUARE, "x", &claude("claude-haiku-5-5")).unwrap();
    assert_eq!(collect(&rx, 10).last(), Some(&AskEvent::Failed("Overloaded".into())));
    unsafe { std::env::remove_var("NTEX_STUDIO_CLAUDE") };
}

#[test]
fn codex_answer() {
    let _g = lock();
    let t = TempDir::new("fig-ai-codex");
    // the fake writes the answer to the -o file, like codex exec does
    let body = format!(
        "cat > \"{d}/stdin\"\npwd > \"{d}/cwd\"\nprintf '%s\\n' \"$@\" > \"{d}/args\"\nout=''\nprev=''\nfor a in \"$@\"; do [ \"$prev\" = -o ] && out=\"$a\"; prev=\"$a\"; done\ncat > \"$out\" <<'EOF'\n```fig\n{LABELLED}```\nSummary: done by codex.\nEOF\necho 'tokens used' >&2",
        d = t.path().display()
    );
    let s = script(t.path(), "fake-codex", &body);
    unsafe { std::env::set_var("NTEX_STUDIO_CODEX", &s) };
    let (_job, rx) = AskJob::start(SQUARE, "label the vertical arrows g and h", &codex("gpt-6-luna")).unwrap();
    let evs = collect(&rx, 10);
    println!("{evs:?}");
    assert_eq!(evs.first(), Some(&AskEvent::Progress("Asking Luna…".into())));
    assert!(matches!(evs.last(), Some(AskEvent::Done { source, summary, .. }) if source == LABELLED && summary == "done by codex."));
    let stdin = std::fs::read_to_string(t.path().join("stdin")).unwrap();
    assert!(stdin.starts_with(&system_prompt()), "codex gets the reference first, on stdin");
    assert!(stdin.contains("label the vertical arrows g and h"));
    let args = std::fs::read_to_string(t.path().join("args")).unwrap();
    assert!(args.starts_with("exec\n-m\ngpt-6-luna\n") && args.trim_end().ends_with("\n-"));

    // codex fails without writing an answer: its ERROR line
    let s = script(t.path(), "fake-codex-fail", "cat > /dev/null\necho 'starting' >&2\necho 'ERROR: unexpected status 401 Unauthorized' >&2\nexit 1");
    unsafe { std::env::set_var("NTEX_STUDIO_CODEX", &s) };
    let (_job, rx) = AskJob::start(SQUARE, "x", &codex("gpt-6-luna")).unwrap();
    assert_eq!(collect(&rx, 10).last(), Some(&AskEvent::Failed("ERROR: unexpected status 401 Unauthorized".into())));
    unsafe { std::env::remove_var("NTEX_STUDIO_CODEX") };
}

#[test]
fn cancel_and_timeout_kill_the_group() {
    let _g = lock();
    let t = TempDir::new("fig-ai-cancel");
    // a model that never answers (and has a child of its own)
    let s = script(t.path(), "fake-slow", "cat > /dev/null\nsleep 31 &\nwait");
    unsafe { std::env::set_var("NTEX_STUDIO_CLAUDE", &s) };
    let (mut job, rx) = AskJob::start(SQUARE, "x", &claude("claude-sonnet-5-5")).unwrap();
    assert!(matches!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), AskEvent::Progress(_)));
    std::thread::sleep(Duration::from_millis(300));
    assert!(job.running());
    let t0 = Instant::now();
    job.cancel();
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), AskEvent::Failed("Cancelled.".into()));
    assert!(t0.elapsed() < Duration::from_secs(2));
    assert!(!job.running());
    job.cancel(); // twice is harmless, and says nothing more
    assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
    std::thread::sleep(Duration::from_millis(200));
    no_orphan("^sleep 31$");

    // the timeout (shortened): Failed, the group killed
    unsafe { std::env::set_var("NTEX_STUDIO_ASK_TIMEOUT_MS", "800") };
    let s = script(t.path(), "fake-slow2", "cat > /dev/null\nsleep 32 &\nwait");
    unsafe { std::env::set_var("NTEX_STUDIO_CLAUDE", &s) };
    let t0 = Instant::now();
    let (job, rx) = AskJob::start(SQUARE, "x", &claude("claude-sonnet-5-5")).unwrap();
    let evs = collect(&rx, 5);
    assert!(matches!(evs.last(), Some(AskEvent::Failed(f)) if f.starts_with("No answer in")), "{evs:?}");
    assert!(t0.elapsed() < Duration::from_secs(3));
    assert!(!job.running());
    std::thread::sleep(Duration::from_millis(200));
    no_orphan("^sleep 32$");
    unsafe { std::env::remove_var("NTEX_STUDIO_ASK_TIMEOUT_MS") };

    // dropping a running job kills it too
    let s = script(t.path(), "fake-slow3", "cat > /dev/null\nsleep 33 &\nwait");
    unsafe { std::env::set_var("NTEX_STUDIO_CLAUDE", &s) };
    let (job, _rx) = AskJob::start(SQUARE, "x", &claude("claude-sonnet-5-5")).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    drop(job);
    std::thread::sleep(Duration::from_millis(200));
    no_orphan("^sleep 33$");
    unsafe { std::env::remove_var("NTEX_STUDIO_CLAUDE") };
}

// ---------------------------------------------------------------------------
// real calls (network, her subscriptions): opt-in

fn real(model: AskModel) {
    let t0 = Instant::now();
    let (_job, rx) = AskJob::start(SQUARE, "label the vertical arrows g and h", &model).unwrap();
    let evs = collect(&rx, TIMEOUT.as_secs() + 10);
    println!("{:?} in {:.1} s: {evs:?}", model.model, t0.elapsed().as_secs_f32());
    let Some(AskEvent::Done { source, .. }) = evs.last() else { panic!("{evs:?}") };
    assert!(source.contains("|g") && source.contains("|h"), "{source}");
    assert!(source.contains("% a square"), "the comment is kept");
}

#[test]
#[ignore]
fn real_luna() {
    let _g = lock();
    real(AskModel { backend: "codex".into(), model: "gpt-6-luna".into(), effort: "low".into() });
}

#[test]
#[ignore]
fn real_sonnet() {
    let _g = lock();
    real(AskModel { backend: "claude".into(), model: "claude-sonnet-5-5".into(), effort: "low".into() });
}
