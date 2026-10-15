//! Single instance over a Unix socket, in-process: the first claim becomes
//! the studio, the second forwards to it.
mod common;
use common::*;
use ntex_studio_core::model::*;
use ntex_studio_io::instance::{Instance, claim, parse_args, socket_path};
use std::time::Duration;

fn paths_in(t: &TempDir) -> Paths {
    // a short state path: Unix socket paths are limited to 104 bytes
    Paths { config: real_config(), snippets: real_config().join("snippets"), templates_tex: real_config().join("templates/tex"), state: t.path().join("s") }
}

#[test]
fn two_claims_in_process() {
    let t = TempDir::new("inst");
    let paths = paths_in(&t);
    let first = OpenRequest { page: Some("snippet".into()), ..Default::default() };
    let Instance::Primary(rx) = claim(&paths, &first).unwrap() else { panic!("the first claim must be primary") };
    assert!(socket_path(&paths).exists());

    let second = OpenRequest { page: Some("inbox".into()), output: Some("x^2".into()), context: Some("math".into()), ft: Some("tex".into()), figure: None, background: true };
    match claim(&paths, &second).unwrap() {
        Instance::Forwarded => {}
        Instance::Primary(_) => panic!("the second claim must forward"),
    }
    let got = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(got, second);

    // a third, empty request
    assert!(matches!(claim(&paths, &OpenRequest::default()).unwrap(), Instance::Forwarded));
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), OpenRequest::default());

    // the studio quits (receiver dropped): the next claim takes over
    drop(rx);
    let Instance::Primary(rx2) = claim(&paths, &first).unwrap() else { panic!("after the studio is gone, a new one is primary") };
    assert!(matches!(claim(&paths, &second).unwrap(), Instance::Forwarded));
    assert_eq!(rx2.recv_timeout(Duration::from_secs(5)).unwrap(), second);
    let _ = std::fs::remove_file(socket_path(&paths));
}

#[test]
fn short_state_paths_use_their_own_socket() {
    let p = Paths { config: real_config(), snippets: real_config().join("snippets"), templates_tex: real_config().join("templates/tex"), state: home().join(".local/state/ntex-studio") };
    assert_eq!(socket_path(&p), home().join(".local/state/ntex-studio/studio.sock"));
}

#[test]
fn stale_socket_is_taken_over() {
    let t = TempDir::new("inst-stale");
    let paths = paths_in(&t);
    std::fs::create_dir_all(&paths.state).unwrap();
    // a socket file nobody listens on (a studio that crashed)
    let l = std::os::unix::net::UnixListener::bind(socket_path(&paths)).unwrap();
    drop(l);
    assert!(socket_path(&paths).exists());
    assert!(matches!(claim(&paths, &OpenRequest::default()).unwrap(), Instance::Primary(_)));
    let _ = std::fs::remove_file(socket_path(&paths));
}

#[test]
fn args() {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(parse_args(&s(&["ntex-studio"])), OpenRequest::default());
    let r = parse_args(&s(&["ntex-studio", "--page", "inbox", "--context", "text", "--ft", "md", "--bogus"]));
    assert_eq!(r.page.as_deref(), Some("inbox"));
    assert_eq!(r.context.as_deref(), Some("text"));
    assert_eq!(r.ft.as_deref(), Some("markdown"));
    assert_eq!(parse_args(&s(&["--page=block"])).page.as_deref(), Some("block"));
    assert_eq!(parse_args(&s(&["--page", "nonsense"])).page, None);

    // a selection file under $TMPDIR: read, then deleted
    let t = TempDir::new("inst-args");
    let f = t.path().join("sel.txt");
    std::fs::write(&f, "\\alpha + \\beta\n").unwrap();
    let r = parse_args(&s(&["ntex-studio", "--output-file", f.to_str().unwrap(), "--context", "math"]));
    assert_eq!(r.output.as_deref(), Some("\\alpha + \\beta\n"));
    assert!(!f.exists(), "a temp selection file is removed after reading");

    // a file outside $TMPDIR is read but left alone
    let keep_dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&keep_dir).unwrap();
    let keep = keep_dir.join(format!("keep-{}.txt", std::process::id()));
    std::fs::write(&keep, "x").unwrap();
    assert!(!keep.canonicalize().unwrap().starts_with(std::env::temp_dir().canonicalize().unwrap()));
    let r = parse_args(&s(&["--output-file", keep.to_str().unwrap()]));
    assert_eq!(r.output.as_deref(), Some("x"));
    assert!(keep.exists(), "not under $TMPDIR: kept");
    std::fs::remove_file(&keep).unwrap();

    // a missing file: no output, no crash
    assert_eq!(parse_args(&s(&["--output-file", "/nonexistent/x"])).output, None);
}

#[test]
fn background_flag_parses() {
    let a = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert!(parse_args(&a(&["--background"])).background);
    assert!(parse_args(&a(&["--page", "inbox", "--background"])).background);
    let r = parse_args(&a(&["--background", "--page", "inbox"]));
    assert!(r.background && r.page.as_deref() == Some("inbox"), "the flag takes no value");
    assert!(!parse_args(&a(&["--page", "inbox"])).background);
}

#[test]
fn background_flag_round_trips_over_the_socket_and_defaults_off() {
    let t = TempDir::new("instbg");
    let paths = paths_in(&t);
    let Instance::Primary(rx) = claim(&paths, &OpenRequest::default()).unwrap() else { panic!("primary") };
    let bg = OpenRequest { figure: Some("/tmp/x.fig".into()), background: true, ..Default::default() };
    assert!(matches!(claim(&paths, &bg).unwrap(), Instance::Forwarded));
    let got = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(got.background && got.figure.as_deref() == Some("/tmp/x.fig"));
    // a normal request after it is not background
    assert!(matches!(claim(&paths, &OpenRequest::default()).unwrap(), Instance::Forwarded));
    assert!(!rx.recv_timeout(Duration::from_secs(5)).unwrap().background);
    // an old client's JSON (no field) is a normal request
    let old: OpenRequest = serde_json::from_str(r#"{"page":"inbox"}"#).unwrap();
    assert!(!old.background);
}
