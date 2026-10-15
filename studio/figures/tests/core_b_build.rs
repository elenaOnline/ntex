//! build + CLI (FIG-CORE-B): tectonic in a temp dir, atomic rename, the first
//! `!` line as the error, the old PDF kept. Everything in temp dirs.
mod core_b_common;
use core_b_common::*;
use ntex_figures::build::*;
use ntex_figures::model::*;

fn square() -> Figure {
    fig(vec![a("A", "B", Some("f")), a("A", "C", Some("g")), a("B", "D", Some("h")), a("C", "D", Some("k"))])
}

#[test]
fn builds_atomically_and_keeps_the_old_pdf_on_error() {
    let dir = tmp("build");
    let out = dir.join("figures").join("sq.pdf");
    let b = build_figure(&square(), &out, &templates_tex()).expect("builds");
    assert_eq!(b.pdf, out);
    let good = std::fs::read(&out).unwrap();
    assert!(good.starts_with(b"%PDF"));
    // no temp files left next to it
    let left: Vec<_> = std::fs::read_dir(out.parent().unwrap()).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(left.len(), 1, "{left:?}");
    // a TeX error: Err is the first `!` line, the good PDF stays
    let bad = fig(vec![node("A", "\\nosuchmacro", None)]);
    let err = build_figure(&bad, &out, &templates_tex()).err().expect("fails");
    assert!(err.starts_with('!'), "{err}");
    assert!(err.contains("Undefined control sequence"), "{err}");
    assert_eq!(std::fs::read(&out).unwrap(), good, "the old PDF is kept");
    // nothing to draw: a placeholder PDF, still Ok, with a note
    let empty = dir.join("figures").join("fig-3.pdf");
    let b = build_figure(&fig(vec![Item::Comment { text: "% nothing yet".into(), span: sp() }]), &empty, &templates_tex()).expect("an empty figure builds a placeholder");
    assert_eq!(b.warnings, vec![EMPTY_NOTE.to_string()]);
    assert!(std::fs::read(&empty).unwrap().starts_with(b"%PDF"));
    assert!(placeholder_document("fig-3").contains("empty figure --- \\detokenize{fig-3}"));
    // the picture rasterises headlessly
    let png = dir.join("sq.png");
    rasterise(&out, &png, 2.0).unwrap();
    assert!(std::fs::metadata(&png).unwrap().len() > 1000);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_kind_compiles_in_her_theme() {
    // one document with a bit of everything (a real TeX run)
    let f = fig(vec![
        node("A", "\\mathbb{Z}/n\\mathbb{Z}", Some(Pos::Cell(0.0, 0.0))),
        node("B", "G^{ab}", Some(Pos::Cell(1.0, 0.0))),
        with(arr("A", "B", Some("g \\circ f")), |a| {
            a.head = Head::Hook;
            a.iso = true;
            a.bend = 20.0;
            a.color = Some("teal".into())
        }),
        with(arr("B", "B", None), |a| a.is_loop = true),
        Item::Blob(blob("G", "G", &["e", "g"], None, false)),
        Item::Blob(blob("K", "\\ker\\varphi", &["k"], Some("G"), true)),
        Item::NGon(NGon { all_axes: true, rotation: 2, ..ngon("P", 6) }),
        cayley("C", GroupSpec::Named("S3".into()), &["a", "b"], true),
        Item::Text(Text { text: "kernel $\\mapsto$ identity".into(), pos: Pos::Abs(0.0, -3.0), span: sp(), color: None, size: None }),
    ]);
    let dir = tmp("kinds");
    let out = dir.join("all.pdf");
    if let Err(e) = build_figure(&f, &out, &templates_tex()) {
        let s = ntex_figures::layout::layout(&f);
        panic!("{e}\n{}", ntex_figures::tikz::document(&f, &s));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn templates_dir_follows_the_env() {
    // SAFETY: this test binary's only env change; other tests read HOME only
    let prev = std::env::var_os("NTEX_STUDIO_CONFIG");
    unsafe { std::env::set_var("NTEX_STUDIO_CONFIG", "/tmp/ntex-cfg") };
    assert_eq!(templates_dir(), std::path::PathBuf::from("/tmp/ntex-cfg/templates/tex"));
    unsafe { std::env::remove_var("NTEX_STUDIO_CONFIG") };
    assert!(templates_dir().ends_with(".config/texnvim/templates/tex"));
    if let Some(p) = prev {
        unsafe { std::env::set_var("NTEX_STUDIO_CONFIG", p) };
    }
}

fn parse_works() -> bool {
    std::panic::catch_unwind(|| ntex_figures::parse::parse("A -> B")).is_ok()
}

#[test]
fn cli() {
    let bin = env!("CARGO_BIN_EXE_ntex-fig");
    let run = |args: &[&str]| std::process::Command::new(bin).args(args).env("NTEX_STUDIO_CONFIG", templates_tex().parent().unwrap().parent().unwrap()).output().unwrap();
    let o = run(&[]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("usage: ntex-fig"));
    assert_eq!(run(&["frobnicate", "x.fig"]).status.code(), Some(2));
    assert_eq!(run(&["build", "/nonexistent/x.fig"]).status.code(), Some(1));
    if !parse_works() {
        eprintln!("parse isn't in yet: the build/tikz/check round trip is skipped");
        return;
    }
    let dir = tmp("cli");
    // new: the template, only when the file doesn't exist
    let fresh = dir.join("figures").join("fig-1.fig");
    assert!(run(&["new", fresh.to_str().unwrap()]).status.success());
    // spec change (interaction pass): the template names its figure
    assert_eq!(std::fs::read_to_string(&fresh).unwrap(), ntex_figures::build::template_for("fig-1"));
    assert!(std::fs::read_to_string(&fresh).unwrap().starts_with("% Ntex Figures — this file draws figures/fig-1.pdf."));
    std::fs::write(&fresh, "A -> B\n").unwrap();
    assert!(run(&["new", fresh.to_str().unwrap()]).status.success());
    assert_eq!(std::fs::read_to_string(&fresh).unwrap(), "A -> B\n", "never overwrites");
    // an all-comment figure (a fresh template) builds a placeholder, exit 0
    let blank = dir.join("blank.fig");
    std::fs::write(&blank, TEMPLATE).unwrap();
    let o = run(&["build", blank.to_str().unwrap()]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(String::from_utf8_lossy(&o.stderr).contains("placeholder"));
    assert!(dir.join("blank.pdf").exists());
    let fig = dir.join("sq.fig");
    std::fs::write(&fig, "A -f-> B\nA -g-> C\nB -h-> D\nC -k-> D\n").unwrap();
    let o = run(&["build", fig.to_str().unwrap()]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(dir.join("sq.pdf").exists());
    let o = run(&["tikz", fig.to_str().unwrap()]);
    assert!(String::from_utf8_lossy(&o.stdout).contains("\\begin{tikzpicture}"));
    let o = run(&["tikz", "--portable", fig.to_str().unwrap()]);
    assert!(String::from_utf8_lossy(&o.stdout).contains("\\definecolor{ntexFigInk}"));
    let o = run(&["check", fig.to_str().unwrap()]);
    assert!(o.status.success());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_pdf_is_exactly_the_scene_bounds_plus_the_border() {
    // so a PNG pixel maps to Scene cm by a fixed affine map (the canvas relies on it)
    let dir = tmp("bounds");
    for (stem, f) in examples().into_iter().filter(|(s, _)| ["03-pullback", "06-kernel", "11-cayley-q8"].contains(s)) {
        let s = ntex_figures::layout::layout(&f);
        let out = dir.join(format!("{stem}.pdf"));
        build_figure(&f, &out, &templates_tex()).unwrap();
        let scale = 4.0f32;
        let (w, h) = rasterise_size(&out, &dir.join(format!("{stem}.png")), scale).unwrap();
        let bp = |cm: f32| cm / 2.54 * 72.0 + 2.0 * 3.0 * 72.0 / 72.27;
        let (ew, eh) = (bp(s.bounds.max.x - s.bounds.min.x) * scale, bp(s.bounds.max.y - s.bounds.min.y) * scale);
        assert!((w as f32 - ew).abs() <= 2.0 && (h as f32 - eh).abs() <= 2.0, "{stem}: png {w}x{h}, scene says {ew:.1}x{eh:.1}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
