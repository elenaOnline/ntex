//! (figure-qol B) How big the figure is on the page.
//!
//! The PDF `ntex-fig build` makes is a `standalone` with `border=3pt` around
//! the TikZ picture, whose bounding box is the Scene's `bounds` (tikz.rs
//! `\useasboundingbox`). So the size of the figure on the page is the Scene's
//! bounds plus that border, in centimetres, exactly. Geometry follows the
//! `scale` global; labels stay at document size, so the size is not strictly
//! proportional to `scale` and a requested width is solved for.
use crate::layout::layout;
use crate::model::*;

/// The standalone class's `border=3pt` (TeX points), in cm.
pub const BORDER_CM: f32 = 3.0 * 2.54 / 72.27;

/// The smallest and largest `scale` a width may ask for.
pub const SCALE_MIN: f32 = 0.05;
pub const SCALE_MAX: f32 = 20.0;

/// (width, height) of the PDF page in cm: the Scene's bounds and the border.
pub fn page_size(scene: &Scene) -> (f32, f32) {
    let b = &scene.bounds;
    (b.max.x - b.min.x + 2.0 * BORDER_CM, b.max.y - b.min.y + 2.0 * BORDER_CM)
}

/// The page width (cm) the figure would have at this scale.
pub fn width_at(fig: &Figure, scale: f32) -> f32 {
    let mut f = fig.clone();
    f.globals.scale = scale;
    page_size(&layout(&f)).0
}

/// "6.4 × 3.1 cm" (one decimal; the PDF is within a millimetre of it).
pub fn fmt_size(w: f32, h: f32) -> String {
    format!("{w:.1} × {h:.1} cm")
}

/// The `scale` that makes the page `target` cm wide, to three decimals; None
/// when the figure is empty or the width can't be had (labels alone are
/// wider, or past SCALE_MAX).
pub fn scale_for_width(fig: &Figure, target: f32) -> Option<f32> {
    if !(target > 0.0) || crate::build::is_empty(fig) {
        return None;
    }
    // width(scale) is nearly a line (geometry scales, labels do not), so a
    // secant iteration lands in a few layouts
    let mut s0 = fig.globals.scale.clamp(SCALE_MIN, SCALE_MAX);
    let mut w0 = width_at(fig, s0);
    let mut s1 = (s0 * target / w0.max(0.01)).clamp(SCALE_MIN, SCALE_MAX);
    for _ in 0..12 {
        let w1 = width_at(fig, s1);
        if (w1 - target).abs() < 0.0004 {
            break;
        }
        let slope = (w1 - w0) / (s1 - s0);
        let next = if slope.abs() < 1e-4 { s1 * target / w1.max(0.01) } else { s1 + (target - w1) / slope };
        s0 = s1;
        w0 = w1;
        s1 = next.clamp(SCALE_MIN, SCALE_MAX);
        if (s1 - s0).abs() < 1e-5 {
            break;
        }
    }
    let s = (s1 * 1000.0).round() / 1000.0;
    // honest about what it can't reach
    ((width_at(fig, s) - target).abs() < 0.05).then_some(s)
}
