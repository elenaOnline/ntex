//! ntex as a terminal sees it: Studio attached as nvim's UI to her config.
//! `cargo test -p ntex-studio-io --test terminal_capture -- --ignored` also
//! rewrites the bundled screen (core/src/ghostty/data/ntex-capture.json).
use ntex_studio_core::ghostty::screen::{self, Paint};
use ntex_studio_core::model::Paths;

#[test]
#[ignore = "runs nvim with her config; also rewrites the bundled capture"]
fn capture_her_ntex() {
    let paths = Paths::from_env();
    let s = ntex_studio_io::ghostty::capture_ntex(&paths, 100, 19).expect("capture");
    let text: Vec<String> = s.cells.iter().map(|r| r.iter().map(|c| c.ch.as_str()).collect::<String>()).collect();
    for l in &text {
        eprintln!("{}", l.trim_end());
    }
    assert!(text.iter().any(|l| l.contains("Orbit")), "the sample is on screen");
    // ntex paints its page itself: its cells carry exact colors
    assert!(s.cells.iter().flatten().any(|c| matches!(c.bg, Paint::Rgb(_))));
    assert!(s.cursor.is_some());
    let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../core/src/ghostty/data/ntex-capture.json");
    std::fs::write(&out, screen::to_capture(&s)).unwrap();
    eprintln!("wrote {}", out.display());
}

#[test]
#[ignore = "asks the installed Ghostty about her real config (read-only)"]
fn her_effective_look() {
    let places = ntex_studio_io::ghostty::Places::from_env();
    let d = places.defaults();
    let look = places.effective(&d);
    for (k, v) in &look.set {
        eprintln!("{k} = {v:?}");
    }
    assert_eq!(look.one("background", &d), "#ffffff");
    assert!(places.themes(&d).len() > 400);
    eprintln!("connected: {}, pinned: {:?}", places.connected(), places.pinned());
}
