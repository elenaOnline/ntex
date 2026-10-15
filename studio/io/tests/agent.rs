//! The sorter: prompt, schema, plan checks, stream parsing (with a fake
//! `claude` script), and one real run on request:
//!   NTEX_STUDIO_REAL_SORT=1 cargo test -p ntex-studio-io --test agent real_sort_run -- --ignored --nocapture
mod common;
use common::*;
use ntex_studio_core::model::*;
use ntex_studio_io::agent::*;
use std::time::{Duration, Instant};

fn entry(id: &str, trigger: &str, source: &str) -> InboxEntry {
    InboxEntry { id: id.into(), made: "2026-10-07T14:32".into(), source: source.into(), draft: None, trigger: trigger.into(), line: 1 }
}

fn seeded() -> Vec<InboxEntry> {
    vec![
        entry("ff-1432", "ff", "  math(\"ff\", [[\\frac{<>}{<>}]], \"yours\"),"),
        entry("ora-1433", "ora", "  post(\"ora\", [[\\overrightarrow{%1}]], \"yours\"),"),
        entry("thm-1434", "thm", "  text(\"thm\", [[\\begin{theorem}\n\t<>\n\\end{theorem}]], \"yours\", { tab = true, line = \"start\", ft = \"tex\" }),"),
    ]
}

/// Write the seeded entries into the temp copy's inbox.lua, the way the studio would.
fn seed_inbox(paths: &Paths, entries: &[InboxEntry]) {
    let mut s = String::from("-- Snippets made in Ntex Studio, waiting to be sorted. Safe to edit by hand.\nlocal h = require(\"snips.helpers\")\nlocal math, math_tab, text, suffix, post, pat, vis = h.math, h.math_tab, h.text, h.suffix, h.post, h.pat, h.vis\n\nreturn h.group(\"Inbox\", {\n");
    for e in entries {
        s.push_str(&format!("  -- studio: id={} made={}\n{}\n", e.id, e.made, e.source));
    }
    s.push_str("})\n");
    std::fs::write(paths.inbox(), s).unwrap();
}

#[test]
fn scans_her_files() {
    let ops = std::fs::read_to_string(real_config().join("snippets/tex/operations.lua")).unwrap();
    let f = topic_file("operations.lua", &ops);
    assert_eq!(f.title, "Operations");
    assert!(f.first_comment.starts_with("Powers, roots, fractions"));
    let slash = f.entries.iter().find(|e| e.trigger == "//").expect("// in operations.lua");
    assert_eq!(slash.body, "\\frac{<>}{<>}");
    assert!(!f.entries.iter().any(|e| e.trigger == "sq\", [[\\sqrt{ <> }]], \"default\")"), "comment lines are not entries");
    // text.lua: multi-line bodies, Tab environments
    let txt = std::fs::read_to_string(real_config().join("snippets/tex/text.lua")).unwrap();
    let t = topic_file("text.lua", &txt);
    let table = t.entries.iter().find(|e| e.trigger == "table").unwrap();
    assert!(table.body.contains("\\end{table}"));
    // the inbox and studio scratch files are not topic files
    let all = topic_files(&real_config().join("snippets"));
    assert!(all.len() >= 14);
    assert!(!all.iter().any(|f| f.name == "inbox.lua"));
    let n: usize = all.iter().map(|f| f.entries.len()).sum();
    assert!(n > 200, "{n} entries");
}

#[test]
fn prompt_is_self_contained_and_private() {
    let t = TempDir::new("agent-prompt");
    let paths = temp_paths(&t);
    let p = prompt(&paths, &seeded());
    for must in ["## operations.lua", "// → \\frac{<>}{<>}", "## accents.lua", "id=ff-1432", "id=thm-1434", "\\begin{theorem}", "Rules", "Examples of good placement", "after_trigger"] {
        assert!(p.contains(must), "prompt lacks {must:?}");
    }
    assert!(!p.contains("## inbox.lua"));
    // nothing about the user or their machine
    let home = home().to_string_lossy().to_string();
    assert!(!p.contains(&home), "no home path");
    // no email address (an @ only as in "@a" from greek.lua's comment)
    assert!(!p.split_whitespace().any(|w| w.contains('@') && w.contains('.') && w.find('@').unwrap() > 0), "no email address");
    println!("prompt: {} chars, ~{} tokens", p.len(), p.len() / 4);
}

#[test]
fn schema_and_args() {
    let s = plan_schema();
    assert_eq!(s["properties"]["moves"]["items"]["required"], serde_json::json!(["id", "file", "reason"]));
    let args = command_args(&StudioPrefs::default());
    let joined = args.join(" ");
    for must in ["-p", "--model claude-haiku-5-5", "--effort high", "--output-format stream-json", "--verbose", "--json-schema", "--tools Read,Grep,Glob", "--permission-mode dontAsk", "--safe-mode", "--no-session-persistence", "--allowedTools Read Grep Glob"] {
        assert!(joined.contains(must), "args lack {must}: {joined}");
    }
    // the prompt goes on stdin, so nothing positional follows --allowedTools
    assert_eq!(args.last().unwrap(), "Glob");
}

#[test]
fn plan_checks() {
    let t = TempDir::new("agent-plan");
    let paths = temp_paths(&t);
    let v = serde_json::json!({
        "moves": [
            { "id": "ff-1432", "file": "operations.lua", "after_trigger": "//", "reason": "fractions" },
            { "id": "ora-1433", "file": "tex/accents.lua", "after_trigger": "nonexistent", "reason": "accents" },
            { "id": "thm-1434", "file": "theorems.lua", "after_trigger": "beg", "reason": "new" },
            { "id": "ff-1432", "file": "sets.lua", "reason": "repeat" },
            { "id": "nope", "file": "sets.lua", "reason": "unknown" },
        ],
        "notes": ""
    });
    let plan = plan_from_value(&v, &seeded(), &paths.snippets).unwrap();
    assert_eq!(plan.moves.len(), 3);
    assert_eq!(plan.moves[0].after_trigger.as_deref(), Some("//"));
    assert_eq!(plan.moves[1].file, "accents.lua");
    assert_eq!(plan.moves[1].after_trigger, None, "a trigger the file doesn't have is cleared");
    assert_eq!(plan.moves[2].new_title.as_deref(), Some("Theorems"), "a new file gets a title");
    assert_eq!(plan.moves[2].after_trigger, None);
    assert!(plan.notes.contains("unknown or repeated"));
    for bad in ["../evil.lua", "/etc/x.lua", "inbox.lua", "Ops.lua", "x.txt"] {
        let v = serde_json::json!({ "moves": [{ "id": "ff-1432", "file": bad, "reason": "x" }] });
        assert!(plan_from_value(&v, &seeded(), &paths.snippets).unwrap().moves.is_empty(), "{bad} refused");
    }
}

#[test]
fn progress_from_stream() {
    let read = serde_json::json!({"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/x/y/tex/operations.lua"}}]}});
    assert_eq!(progress_lines(&read), vec!["Reading operations.lua".to_string()]);
    let text = serde_json::json!({"type":"assistant","message":{"content":[{"type":"text","text":"\nff makes a fraction, so operations.lua.\nMore."}]}});
    assert_eq!(progress_lines(&text), vec!["ff makes a fraction, so operations.lua.".to_string()]);
    let init = serde_json::json!({"type":"system","subtype":"init","model":"claude-haiku-5-5"});
    assert!(progress_lines(&init).is_empty(), "the studio announces the model itself");
    assert!(progress_lines(&serde_json::json!({"type":"user"})).is_empty());
}

/// A stand-in `claude` (a shell script) for the process side: no network.
fn fake_claude(dir: &std::path::Path, body: &str) -> std::path::PathBuf {
    let p = dir.join("fake-claude");
    std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

#[test]
fn job_with_a_fake_claude() {
    let t = TempDir::new("agent-fake");
    let paths = temp_paths(&t);
    let entries = seeded();
    // 1. a plan arrives through the stream; the prompt came on stdin, cwd is a copy
    let script = fake_claude(
        t.path(),
        r#"cat > "$PWD/../prompt.txt"
pwd > "$PWD/../cwd.txt"
echo '{"type":"system","subtype":"init","model":"fake"}'
echo '{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"tex/operations.lua"}}]}}'
echo '{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.001,"duration_ms":1000,"structured_output":{"moves":[{"id":"ff-1432","file":"operations.lua","after_trigger":"//","reason":"fractions"}],"notes":""}}'"#,
    );
    // SAFETY: the only test in this binary that sets the variable
    unsafe { std::env::set_var("NTEX_STUDIO_CLAUDE", &script) };
    let (job, rx) = SortJob::start(&paths, &StudioPrefs::default(), &entries).unwrap();
    let mut events = Vec::new();
    while let Ok(ev) = rx.recv_timeout(Duration::from_secs(10)) {
        let last = matches!(ev, AgentEvent::Plan(_) | AgentEvent::Failed(_));
        events.push(ev);
        if last {
            break;
        }
    }
    println!("{events:?}");
    assert!(events.contains(&AgentEvent::Progress("Reading operations.lua".into())));
    let AgentEvent::Plan(plan) = events.last().unwrap() else { panic!("no plan: {events:?}") };
    assert_eq!(plan.moves[0].file, "operations.lua");
    drop(job);
    // the job's private folder is gone once claude finished
    std::thread::sleep(Duration::from_millis(200));
    let leftovers: Vec<_> = std::fs::read_dir(std::env::var("TMPDIR").unwrap()).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with(&format!("ntex-studio-sort.{}.", std::process::id()))).collect();
    assert!(leftovers.is_empty(), "sort folders left: {leftovers:?}");

    // 2. cancel kills it (and its children) at once
    let script = fake_claude(t.path(), "cat > /dev/null\necho '{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"Looking.\"}]}}'\nsleep 30 &\nwait");
    unsafe { std::env::set_var("NTEX_STUDIO_CLAUDE", &script) };
    let (mut job, rx) = SortJob::start(&paths, &StudioPrefs::default(), &entries).unwrap();
    assert!(matches!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), AgentEvent::Progress(_)));
    let t0 = Instant::now();
    job.cancel();
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), AgentEvent::Failed("Cancelled.".into()));
    assert!(!job.running());
    assert!(t0.elapsed() < Duration::from_secs(2));
    let sleepers = std::process::Command::new("pgrep").args(["-f", "^sleep 30$"]).output().unwrap();
    let still: Vec<u32> = String::from_utf8_lossy(&sleepers.stdout).lines().filter_map(|l| l.parse().ok()).collect();
    // the fake's own `sleep 30` was in its process group, so it went too
    for p in &still {
        let ppid = std::process::Command::new("ps").args(["-o", "ppid=", "-p", &p.to_string()]).output().unwrap();
        assert_ne!(String::from_utf8_lossy(&ppid.stdout).trim(), "1", "orphaned sleep {p}");
    }

    // 3. claude exits without a plan: Failed with its last stderr line
    let script = fake_claude(t.path(), "cat > /dev/null\necho 'Not logged in' >&2\nexit 1");
    unsafe { std::env::set_var("NTEX_STUDIO_CLAUDE", &script) };
    let (_job, rx) = SortJob::start(&paths, &StudioPrefs::default(), &entries).unwrap();
    assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), AgentEvent::Failed("Not logged in".into()));
    unsafe { std::env::remove_var("NTEX_STUDIO_CLAUDE") };
}

/// The one real run (Haiku, network, costs a cent or so). Opt-in.
#[test]
#[ignore]
fn real_sort_run() {
    if std::env::var("NTEX_STUDIO_REAL_SORT").as_deref() != Ok("1") {
        eprintln!("set NTEX_STUDIO_REAL_SORT=1 to run");
        return;
    }
    let t = TempDir::new("agent-real");
    let paths = temp_paths(&t);
    let entries = seeded();
    seed_inbox(&paths, &entries);
    let before: Vec<(String, Vec<u8>)> = std::fs::read_dir(paths.snippets.join("tex")).unwrap().flatten().map(|e| (e.file_name().to_string_lossy().into(), std::fs::read(e.path()).unwrap())).collect();
    let t0 = Instant::now();
    let (_job, rx) = SortJob::start(&paths, &StudioPrefs::default(), &entries).unwrap();
    let mut plan = None;
    while let Ok(ev) = rx.recv_timeout(TIMEOUT + Duration::from_secs(10)) {
        match ev {
            AgentEvent::Progress(p) => println!("  progress: {p}"),
            AgentEvent::Plan(p) => {
                plan = Some(p);
                break;
            }
            AgentEvent::Failed(f) => panic!("failed: {f}"),
        }
    }
    let plan = plan.expect("a plan");
    println!("real sort: {:.1} s", t0.elapsed().as_secs_f64());
    for m in &plan.moves {
        println!("  {} → {} after {:?} (new title {:?}): {}", m.id, m.file, m.after_trigger, m.new_title, m.reason);
    }
    println!("  notes: {:?}", plan.notes);
    assert_eq!(plan.moves.len(), 3);
    // read-only: the (temp) snippet files are untouched
    for (name, bytes) in before {
        assert_eq!(std::fs::read(paths.snippets.join("tex").join(&name)).unwrap(), bytes, "{name} changed");
    }
}

#[test]
#[ignore]
fn print_prompt() {
    let t = TempDir::new("agent-print");
    let paths = temp_paths(&t);
    println!("{}", prompt(&paths, &seeded()));
}
