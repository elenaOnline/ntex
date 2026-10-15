//! Independent review of figure-qol (classes A–E): model / parse / format /
//! clip cases that fail on figure-qol as reviewed and should pass once the
//! bugs are fixed. Each test names the finding it demonstrates (F-ids in the
//! review report).
mod core_a_common;
use core_a_common::*;
use ntex_figures::clip::{copy_objects, paste_into, to_shorthand};

fn cayley0(f: &Figure) -> &Cayley {
    cayleys(f).into_iter().next().expect("a Cayley graph")
}

fn vertex_label_drawn(f: &Figure, word: &str) -> Option<String> {
    let c = cayley0(f);
    let scene = ntex_figures::layout::layout(f);
    scene.draws.iter().find_map(|d| match d {
        Draw::Label { obj: ObjRef::CayleyVertex(n, w), latex, .. } if *n == c.name && w == word => Some(latex.clone()),
        _ => None,
    })
}

/// F2: a Cayley vertex renamed in place (class C) on a graph whose generators
/// are spelled with commands (ρ, σ) has a word with a space in it
/// (`\rho \sigma`); the formatter writes `vertex \rho \sigma "x"`, which
/// doesn't parse back: the name is lost on save and the line warns.
#[test]
fn cayley_vertex_name_on_command_generators_survives_save() {
    let (mut f, d) = parse("cayley D4 <ρ, σ>\n");
    no_errors(&d);
    let word = "\\rho \\sigma";
    assert!(vertex_label_drawn(&f, word).is_some(), "the graph has a vertex {word:?}");
    if let Item::Cayley(c) = &mut f.items[0] {
        c.vertex_names = vec![(word.to_string(), "x".to_string())];
    }
    assert_eq!(vertex_label_drawn(&f, word).as_deref(), Some("x"));
    let out = format(&f);
    let (g, d2) = parse(&out);
    no_errors(&d2);
    assert_eq!(cayley0(&g).vertex_names, cayley0(&f).vertex_names, "saved as:\n{out}");
    assert_eq!(vertex_label_drawn(&g, word).as_deref(), Some("x"), "after reload the vertex isn't named; saved as:\n{out}");
}

/// F2 (the shorthand side): with Greek generators there is no way to write a
/// working `vertex` line in her spelling: `vertex ρσ "x"` parses without a
/// word of warning and names nothing (the word is never matched).
#[test]
fn cayley_vertex_shorthand_in_her_spelling_names_the_vertex() {
    let (f, d) = parse("cayley D4 <ρ, σ> vertex ρσ \"x\"\n");
    let drawn = vertex_label_drawn(&f, "\\rho \\sigma");
    assert!(drawn.as_deref() == Some("x") || !warnings(&d).is_empty(), "`vertex ρσ \"x\"` silently does nothing (drawn {drawn:?})");
}

/// F4: ⌘C on a blob that sits inside another one puts `blob H {h} in G` on
/// the clipboard without G; ⌘V of it (same window, or anywhere) is refused
/// ("there is no blob `G` for `H` to be in").
#[test]
fn copying_an_inner_blob_gives_shorthand_that_pastes() {
    let src = "blob G {g, k}\nblob H in G {h}\n";
    let f = parse(src).0;
    let text = to_shorthand(&f, &[ObjRef::Blob("H".into())]).expect("something on the clipboard");
    let mut same = f.clone();
    let got = paste_into(&mut same, &text, Pt { x: 0.95, y: -0.95 });
    assert!(got.is_ok(), "pasting back what ⌘C copied was refused: {got:?}\nclipboard:\n{text}");
    let mut other = parse("A at 0,0\n").0;
    let got = paste_into(&mut other, &text, Pt::default());
    assert!(got.is_ok(), "pasting it into another figure was refused: {got:?}\nclipboard:\n{text}");
}

/// F9: `ngon P 5 rotation=0 "ρ"` keeps the label in the model, but the
/// formatter drops it (no arrow): parse → format → parse differs.
#[test]
fn rotation_label_without_an_arrow_round_trips() {
    let (f, _) = parse("ngon P 5 rotation=0 \"ρ\"\n");
    let out = format(&f);
    let (g, _) = parse(&out);
    assert_eq!(without_spans(&f), without_spans(&g), "formatted:\n{out}");
}

/// F10: `turn=-9223372036854775808` (i64::MIN) negates with overflow in
/// parse.rs (debug builds panic; pasting it panics inside ⌘V).
#[test]
fn turn_of_i64_min_does_not_panic() {
    let r = std::panic::catch_unwind(|| parse("ngon P 5 turn=-9223372036854775808\n"));
    assert!(r.is_ok(), "parsing turn=i64::MIN panicked");
    let mut dst = parse("A\n").0;
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| paste_into(&mut dst, "ngon P 5 turn=-9223372036854775808\n", Pt::default())));
    assert!(r.is_ok(), "pasting turn=i64::MIN panicked");
}

/// F10: copying an n-gon whose name ends in u32::MAX (`P4294967295`)
/// overflows `+ 1` in clip::fresh_ngon (debug builds panic on ⌥-drag/⌘D/⌘V).
#[test]
fn copying_an_ngon_with_a_huge_number_in_its_name_does_not_panic() {
    let f = parse("ngon P4294967295 3\n").0;
    let mut g = f.clone();
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| copy_objects(&mut g, &f, &[ObjRef::NGon("P4294967295".into())], Pt { x: 1.0, y: 0.0 }, false)));
    assert!(r.is_ok(), "copying ngon P4294967295 panicked");
}

/// F6: paste is quadratic in the number of objects (clip::copy_objects scans
/// the wanted names with `canon` for every item): 1000 nodes take ~5 s in a
/// debug build, 10 000 (well under the 400 kB cap) did not finish in 10 min.
/// The UI thread is frozen meanwhile. 2000 nodes should take well under 4 s.
#[test]
fn pasting_two_thousand_nodes_is_quick() {
    let text: String = (0..2000).map(|k| format!("N{k} at {},{}\n", k % 100, k / 100)).collect();
    let mut dst = parse("A\n").0;
    let t = std::time::Instant::now();
    let got = paste_into(&mut dst, &text, Pt::default());
    let took = t.elapsed();
    assert!(got.is_ok());
    assert!(took < std::time::Duration::from_secs(4), "pasting 2000 nodes took {took:?}");
}

/// F5 (helper): the child half of `pasting_deeply_nested_braces_does_not_crash`;
/// does nothing unless the parent test started it.
#[test]
fn deep_paste_child() {
    if std::env::var("REVIEW_DEEP_CHILD").is_err() {
        return;
    }
    // the main thread's 8 MB, as in the app
    let h = std::thread::Builder::new()
        .stack_size(8 << 20)
        .spawn(|| {
            let text = format!("B \"{}x{}\" at 1,0\n", "{".repeat(20_000), "}".repeat(20_000));
            let mut dst = parse("A\n").0;
            let _ = paste_into(&mut dst, &text, Pt::default());
        })
        .unwrap();
    let _ = h.join();
}

/// F5: a 40 kB clipboard of nested braces (`B "{{{…x…}}}"`), pasted with
/// ⌘V, overflows the stack in layout.rs `measure` (called from
/// clip::copy_objects before anything checks the text) and aborts the whole
/// studio: unsaved work in every window is lost. Run in a child process so
/// the abort doesn't take this test binary with it.
#[test]
fn pasting_deeply_nested_braces_does_not_crash() {
    let exe = std::env::current_exe().unwrap();
    let out = std::process::Command::new(exe)
        .args(["--exact", "deep_paste_child", "--nocapture", "--test-threads=1"])
        .env("REVIEW_DEEP_CHILD", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "pasting 20 000 nested braces killed the process ({:?}): {}",
        out.status,
        String::from_utf8_lossy(&out.stderr).lines().filter(|l| l.contains("overflow") || l.contains("fatal")).collect::<Vec<_>>().join(" / ")
    );
}
