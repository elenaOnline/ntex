//! The theme split (ntex-notes.sty → ntex-base.sty): notes must look
//! exactly the same. Compiles a COPY of the workshop specimen in temp dirs, once with the pre-split ntex-notes v8
//! (tests/core_b_fixtures) and once with the real templates/tex, rasterises
//! both headlessly and compares every pixel.
//!
//! Slow (four tectonic runs), so it runs when NTEX_FIG_THEME_DIFF=1. It is a
//! check of the refactor itself: once she changes the theme on purpose the
//! v8 baseline no longer applies.
mod core_b_common;
use core_b_common::*;
use std::path::{Path, PathBuf};

fn compile(tex: &Path, search: &Path) -> PathBuf {
    let dir = tex.parent().unwrap();
    let st = std::process::Command::new("tectonic")
        .args(["--chatter", "minimal", "-Z"])
        .arg(format!("search-path={}", search.display()))
        .arg(tex)
        .current_dir(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(st.success(), "tectonic failed on {}", tex.display());
    tex.with_extension("pdf")
}

fn png_bytes(p: &Path) -> Vec<u8> {
    std::fs::read(p).unwrap()
}

#[test]
fn ntex_base_split_is_pixel_identical() {
    if !std::env::var("NTEX_FIG_THEME_DIFF").is_ok_and(|v| v == "1") {
        eprintln!("skipped (set NTEX_FIG_THEME_DIFF=1)");
        return;
    }
    let real = templates_tex();
    assert!(real.join("ntex-base.sty").exists() && real.join("ntex-figure.sty").exists());
    let notes = std::fs::read_to_string(real.join("ntex-notes.sty")).unwrap();
    assert!(notes.contains("\\RequirePackage{ntex-base}"));
    assert!(!notes.lines().any(|l| l.trim_start().starts_with("\\definecolor")), "the palette lives in ntex-base.sty now");
    assert!(!notes.lines().any(|l| l.trim_start().starts_with("\\RequirePackage{XCharter}")), "the fonts live in ntex-base.sty now");

    let base = tmp("theme");
    // the pre-split theme: v8 + her blocks, in a temp search path
    let old = base.join("old-tex");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::copy(figures_dir().join("tests/core_b_fixtures/ntex-notes.v8.sty"), old.join("ntex-notes.sty")).unwrap();
    std::fs::copy(real.join("ntex-blocks.sty"), old.join("ntex-blocks.sty")).unwrap();

    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap();
    let docs = [
        ("specimen", home.join(".config/texnvim/templates/workshop/specimen.tex")),
    ];
    for (tag, src) in docs {
        if !src.exists() {
            eprintln!("{tag}: {} missing, skipped", src.display());
            continue;
        }
        let mut pngs = vec![];
        for (which, search) in [("before", &old), ("after", &real)] {
            let d = base.join(format!("{tag}-{which}"));
            std::fs::create_dir_all(&d).unwrap();
            let tex = d.join(src.file_name().unwrap());
            std::fs::copy(&src, &tex).unwrap(); // a copy: her file is only read
            let pdf = compile(&tex, search);
            let png = d.join("page.png");
            rasterise(&pdf, &png, 3.0).unwrap();
            pngs.push(png);
        }
        let (a, b) = (png_bytes(&pngs[0]), png_bytes(&pngs[1]));
        eprintln!("{tag}: before {} bytes, after {} bytes, identical: {}", a.len(), b.len(), a == b);
        assert_eq!(a, b, "{tag} renders differently after the split ({} vs {})", pngs[0].display(), pngs[1].display());
    }
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn ntex_figure_sty_has_every_palette_style() {
    let fig = std::fs::read_to_string(templates_tex().join("ntex-figure.sty")).unwrap();
    assert!(fig.contains("\\RequirePackage{ntex-base}"));
    assert!(fig.contains("\\usetikzlibrary{cd,arrows.meta,calc,hobby}"));
    for c in ["ink", "grey", "blue", "violet", "teal", "maroon", "green", "ochre"] {
        assert!(fig.contains(&format!("ntex {c}/.style")), "missing style ntex {c}");
        assert!(fig.contains(&format!("ntex {c} shade/.style")), "missing style ntex {c} shade");
    }
}

#[test]
fn portable_palette_matches_ntex_base() {
    let base = std::fs::read_to_string(templates_tex().join("ntex-base.sty")).unwrap();
    for (name, rule, back) in ntex_figures::tikz::PALETTE {
        let (rn, bn) = match name {
            "ink" => ("ntexInk".to_string(), None),
            "grey" => ("ntexSlate".to_string(), None),
            _ => {
                let c = name[..1].to_uppercase() + &name[1..];
                (format!("ntex{c}Rule"), Some(format!("ntex{c}Back")))
            }
        };
        assert!(base.contains(&format!("\\definecolor{{{rn}}}{{HTML}}{{{rule}}}")), "{name}: {rule} isn't ntex-base's {rn}");
        if let Some(bn) = bn {
            assert!(base.contains(&format!("\\definecolor{{{bn}}}{{HTML}}{{{back}}}")), "{name}: {back} isn't ntex-base's {bn}");
        }
    }
}
