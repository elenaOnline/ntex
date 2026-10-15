//! Bundled fonts (assets/fonts/, extracted from her tectonic bundle by
//! build.rs / studio/build.sh): Latin Modern Roman (regular, italic, bold,
//! bold italic), Latin Modern Roman Caps, Latin Modern Mono, Latin Modern
//! Math, TeX Gyre Pagella (regular, italic, bold). Registered with GPUI's
//! text system at startup, process-local (nothing installed system-wide).
//!
//! Families as GPUI (CoreText) sees them: "Latin Modern Roman",
//! "Latin Modern Roman Caps", "Latin Modern Mono", "Latin Modern Math",
//! "TeX Gyre Pagella". `tests/widgets_gallery.rs` checks they are all there.
//! Also XCharter (regular, italic, bold, bold italic; family "XCharter"),
//! the figure window's draft labels: see [`CHARTER`].
use std::borrow::Cow;

macro_rules! font {
    ($name:literal) => {
        Cow::Borrowed(include_bytes!(concat!("../../../assets/fonts/", $name, ".otf")).as_slice())
    };
}

/// Every bundled face, as bytes.
pub fn faces() -> Vec<Cow<'static, [u8]>> {
    vec![
        font!("lmroman10-regular"),
        font!("lmroman10-italic"),
        font!("lmroman10-bold"),
        font!("lmroman10-bolditalic"),
        font!("lmromancaps10-regular"),
        font!("lmmono10-regular"),
        font!("lmmono10-italic"),
        font!("latinmodern-math"),
        font!("texgyrepagella-regular"),
        font!("texgyrepagella-italic"),
        font!("texgyrepagella-bold"),
        font!("texgyrepagella-bolditalic"),
        // FIG-UI: Charter for the figure window's native draft (the figures
        // themselves are set in XCharter by ntex-base.sty)
        font!("XCharter-Roman"),
        font!("XCharter-Italic"),
        font!("XCharter-Bold"),
        font!("XCharter-BoldItalic"),
    ]
}

/// The families the theme names (both typefaces).
pub const FAMILIES: [&str; 5] =
    ["Latin Modern Roman", "Latin Modern Roman Caps", "Latin Modern Mono", "Latin Modern Math", "TeX Gyre Pagella"];

/// The figures' face (XCharter, as ntex-base.sty sets them).
pub const CHARTER: &str = "XCharter";

pub fn register(cx: &mut gpui::App) {
    if let Err(err) = cx.text_system().add_fonts(faces()) {
        eprintln!("ntex-studio: could not register the bundled fonts: {err:#}");
    }
}
