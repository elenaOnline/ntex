//! (figure-qol B) How many of GPUI's logical pixels make a centimetre on this
//! display, for ⌘1 "actual size". GPUI doesn't expose the panel's physical
//! size, so on macOS this asks CoreGraphics (`CGDisplayScreenSize`, in mm,
//! against `CGDisplayBounds`, in points). Anything implausible (a display that
//! reports nothing, or a made-up 160 × 90 mm) falls back to the classic 96 dpi.

/// 96 dpi: what a screen is assumed to be when nothing better is known.
pub const FALLBACK_PX_PER_CM: f32 = 96.0 / 2.54;
/// Plausible range of logical px per cm (a 5K 27" at its default is ≈ 43;
/// a 14" laptop ≈ 50; a 1080p 24" ≈ 32).
const PLAUSIBLE: std::ops::RangeInclusive<f32> = 20.0..=90.0;

/// Logical px per physical cm for the display with this CoreGraphics id.
#[cfg(target_os = "macos")]
pub fn px_per_cm_of(display: u32) -> Option<f32> {
    #[repr(C)]
    struct CGSize {
        width: f64,
        height: f64,
    }
    #[repr(C)]
    struct CGPoint {
        x: f64,
        y: f64,
    }
    #[repr(C)]
    struct CGRect {
        origin: CGPoint,
        size: CGSize,
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGDisplayScreenSize(display: u32) -> CGSize;
        fn CGDisplayBounds(display: u32) -> CGRect;
    }
    // SAFETY: plain value-returning CoreGraphics queries about a display id.
    let (mm, pts) = unsafe { (CGDisplayScreenSize(display), CGDisplayBounds(display)) };
    if mm.width <= 0.0 || pts.size.width <= 0.0 {
        return None;
    }
    let v = (pts.size.width / (mm.width / 10.0)) as f32;
    PLAUSIBLE.contains(&v).then_some(v)
}

#[cfg(not(target_os = "macos"))]
pub fn px_per_cm_of(_display: u32) -> Option<f32> {
    None
}

/// The window's display, or the fallback.
pub fn px_per_cm(window: &gpui::Window, cx: &gpui::App) -> f32 {
    window
        .display(cx)
        .and_then(|d| {
            let id: u64 = d.id().into();
            px_per_cm_of(id as u32)
        })
        .unwrap_or(FALLBACK_PX_PER_CM)
}
