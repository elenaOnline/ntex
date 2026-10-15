//! Visual evidence: every figures/examples/*.fig built with tectonic in her
//! theme and rasterised headlessly (CoreGraphics) into figures/evidence/.
//! Slow (one tectonic run per figure), so it only writes evidence when
//! NTEX_FIG_EVIDENCE=1; otherwise it builds two examples into a temp dir.
//!
//! The examples come from the .fig files once `parse` works, else from the
//! hand-built Figures in core_b_common.
mod core_b_common;
use core_b_common::*;
use ntex_figures::model::*;

fn figure_for(stem: &str, by_hand: &Figure) -> (Figure, &'static str) {
    let path = figures_dir().join("examples").join(format!("{stem}.fig"));
    if let Ok(src) = std::fs::read_to_string(&path) {
        let parsed = std::panic::catch_unwind(|| ntex_figures::parse::parse(&src));
        if let Ok((f, _)) = parsed {
            if !ntex_figures::build::is_empty(&f) {
                return (f, "parsed");
            }
        }
    }
    (by_hand.clone(), "by hand")
}

#[test]
fn evidence() {
    let all = std::env::var("NTEX_FIG_EVIDENCE").is_ok_and(|v| v == "1");
    let only = std::env::var("NTEX_FIG_ONLY").ok();
    let out_dir = if all { figures_dir().join("evidence") } else { tmp("evidence") };
    std::fs::create_dir_all(&out_dir).unwrap();
    let work = tmp("evidence-pdf");
    let mut n = 0;
    for (stem, by_hand) in examples() {
        if !all && !["01-square", "10-cayley-d4"].contains(&stem) {
            continue;
        }
        if only.as_deref().is_some_and(|o| !stem.contains(o)) {
            continue;
        }
        let (fig, how) = figure_for(stem, &by_hand);
        let scene = ntex_figures::layout::layout(&fig);
        let pdf = work.join(format!("{stem}.pdf"));
        if let Err(e) = ntex_figures::build::build_figure(&fig, &pdf, &templates_tex()) {
            std::fs::write(work.join(format!("{stem}.tex")), ntex_figures::tikz::document(&fig, &scene)).unwrap();
            panic!("{stem}: {e} (tex in {})", work.display());
        }
        if all {
            std::fs::write(out_dir.join(format!("{stem}.tikz.tex")), ntex_figures::tikz::picture(&fig, &scene)).unwrap();
        }
        let png = out_dir.join(format!("{stem}.png"));
        rasterise(&pdf, &png, 5.0).unwrap();
        eprintln!("{stem} ({how}) → {}", png.display());
        n += 1;
    }
    let _ = std::fs::remove_dir_all(&work);
    if !all {
        let _ = std::fs::remove_dir_all(&out_dir);
    }
    assert!(n > 0);
}
