//! (figure-qol B) The size on the page: the readout, the width solver, and
//! the real PDF (tectonic, in a temp dir) must agree within a millimetre.
mod core_b_common;
use core_b_common::*;
use ntex_figures::build::build_figure;
use ntex_figures::layout::layout;
use ntex_figures::model::*;
use ntex_figures::pagesize::*;

/// The PDF's page in cm, via the headless rasteriser at 10 px per bp.
fn pdf_cm(f: &Figure, dir: &std::path::Path, stem: &str) -> (f32, f32) {
    // the tool is compiled once (the two tests must not both compile it)
    static TOOL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    drop(TOOL.lock().map(|_| pdf2png()));
    let out = dir.join(format!("{stem}.pdf"));
    build_figure(f, &out, &templates_tex()).expect("builds");
    let (w, h) = rasterise_size(&out, &dir.join(format!("{stem}.png")), 10.0).expect("rasterises");
    (w as f32 / 10.0 / 72.0 * 2.54, h as f32 / 10.0 / 72.0 * 2.54)
}

fn at_scale(mut f: Figure, s: f32) -> Figure {
    f.globals.scale = s;
    f
}

#[test]
fn the_readout_matches_the_real_pdf_within_a_millimetre() {
    let dir = tmp("pagesize");
    for (stem, f) in examples().into_iter().filter(|(s, _)| ["01-square", "03-pullback", "06-kernel", "07-d4-square", "11-cayley-q8"].contains(s)) {
        for s in [1.0, 0.6, 1.7] {
            let f = at_scale(f.clone(), s);
            let (w, h) = page_size(&layout(&f));
            let (pw, ph) = pdf_cm(&f, &dir, &format!("{stem}-{s}"));
            assert!((w - pw).abs() < 0.1 && (h - ph).abs() < 0.1, "{stem} at {s}: readout {w:.3} × {h:.3}, PDF {pw:.3} × {ph:.3}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn typing_a_width_solves_for_the_scale_and_the_pdf_has_that_width() {
    let dir = tmp("pagesolve");
    for (stem, f) in examples().into_iter().filter(|(s, _)| ["01-square", "03-pullback", "10-cayley-d4", "07-d4-square"].contains(s)) {
        for target in [4.0f32, 8.0, 12.5] {
            let Some(s) = scale_for_width(&f, target) else {
                // only when labels alone are wider (they don't scale), or past the largest scale
                assert!(width_at(&f, SCALE_MIN) > target - 0.05 || width_at(&f, SCALE_MAX) < target + 0.05, "{stem}: no scale for {target} cm, yet reachable");
                continue;
            };
            let g = at_scale(f.clone(), s);
            let (w, _) = page_size(&layout(&g));
            assert!((w - target).abs() < 0.05, "{stem}: scale {s} gives {w:.3}, wanted {target}");
            let (pw, _) = pdf_cm(&g, &dir, &format!("{stem}-{target}"));
            assert!((pw - target).abs() < 0.1, "{stem}: PDF is {pw:.3} cm wide, wanted {target}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn width_solving_copes_with_what_cannot_be_had() {
    let f = examples().into_iter().find(|(s, _)| *s == "01-square").unwrap().1;
    // labels don't scale: nothing under their own width, and an empty figure has none
    assert_eq!(scale_for_width(&f, 0.2), None);
    assert_eq!(scale_for_width(&fig(vec![]), 5.0), None);
    assert_eq!(scale_for_width(&f, -3.0), None);
    assert_eq!(scale_for_width(&f, f32::NAN), None);
    // and the readout's wording
    assert_eq!(fmt_size(6.44, 3.06), "6.4 × 3.1 cm");
}
