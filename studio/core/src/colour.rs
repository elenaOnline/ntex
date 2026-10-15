//! The theme's palette and new tones from a hue.
//!
//! New hues are made in OKLCH (Björn Ottosson's OKLab in polar form): each of
//! the three tones takes the palette's mean lightness and chroma for that tone
//! (the five hued entries; Proof is grey and left out) at the new hue. When
//! that colour is outside sRGB, chroma is reduced (lightness and hue kept)
//! until it fits, so the tone keeps its place in the light/dark scale and the
//! contrast it was chosen for.
use crate::model::*;

/// The theme's ink (body text), `ntexInk` = 1E1E28.
pub const INK: Rgb = Rgb(0x1E, 0x1E, 0x28);

/// The palette in ntex-notes.sty, by name: Blue, Violet, Teal, Maroon, Green, Proof.
pub fn palette() -> Vec<(&'static str, Tones)> {
    vec![
        ("Blue", Tones { rule: Rgb(0x1E, 0x47, 0xB8), back: Rgb(0xE0, 0xE7, 0xFB), head: Rgb(0x15, 0x30, 0x79) }),
        ("Violet", Tones { rule: Rgb(0x66, 0x1E, 0xB8), back: Rgb(0xEC, 0xE0, 0xFB), head: Rgb(0x44, 0x15, 0x79) }),
        ("Teal", Tones { rule: Rgb(0x1C, 0x82, 0x9C), back: Rgb(0xE2, 0xF4, 0xF8), head: Rgb(0x0F, 0x59, 0x6B) }),
        ("Maroon", Tones { rule: Rgb(0xA9, 0x2D, 0x4A), back: Rgb(0xF8, 0xEA, 0xED), head: Rgb(0x79, 0x15, 0x2D) }),
        ("Green", Tones { rule: Rgb(0x23, 0x8B, 0x45), back: Rgb(0xE5, 0xF5, 0xEA), head: Rgb(0x14, 0x5D, 0x2C) }),
        // \definecolor{ntexProofRule}{gray}{0.55} → 8C, {gray}{0.955} → F4; the
        // proof box has no head colour of its own: its head is the ink.
        ("Proof", Tones { rule: Rgb(0x8C, 0x8C, 0x8C), back: Rgb(0xF4, 0xF4, 0xF4), head: INK }),
    ]
}

/// The palette entry with this name ("Grey"/"Gray" = Proof), any case.
pub fn palette_tones(name: &str) -> Option<Tones> {
    let n = name.trim();
    let n = if n.eq_ignore_ascii_case("grey") || n.eq_ignore_ascii_case("gray") { "Proof" } else { n };
    palette().into_iter().find(|(p, _)| p.eq_ignore_ascii_case(n)).map(|(_, t)| t)
}

/// The palette's own spelling of a name ("blue" → "Blue"), if it is one.
pub fn palette_name(name: &str) -> Option<&'static str> {
    let n = name.trim();
    let n = if n.eq_ignore_ascii_case("grey") || n.eq_ignore_ascii_case("gray") { "Proof" } else { n };
    palette().into_iter().find(|(p, _)| p.eq_ignore_ascii_case(n)).map(|(p, _)| p)
}

/// A colour's tones. An unknown palette name gives the neutral Proof tones.
pub fn tones(colour: &BlockColour) -> Tones {
    match colour {
        BlockColour::Palette(name) => palette_tones(name).unwrap_or_else(|| palette_tones("Proof").unwrap()),
        BlockColour::Hue(h) => tones_for_hue(*h),
    }
}

/// Three tones for a new hue, at the palette's lightness and chroma (work in
/// OKLCH: the mean L/C of the existing Rule, Back and Head tones).
pub fn tones_for_hue(hue: f32) -> Tones {
    let [rule, back, head] = targets();
    let h = (hue as f64).rem_euclid(360.0);
    Tones { rule: from_lch_clipped(rule.0, rule.1, h), back: from_lch_clipped(back.0, back.1, h), head: from_lch_clipped(head.0, head.1, h) }
}

/// The OKLCH hue (degrees) of a colour; for a new block's tones, the hue they
/// were made from (to within rounding).
pub fn hue_of(c: Rgb) -> f32 {
    let (_, _, h) = to_lch(c);
    h as f32
}

/// (L, C) per tone (rule, back, head): the mean over the hued palette entries.
fn targets() -> [(f64, f64); 3] {
    let hued: Vec<Tones> = palette().into_iter().filter(|(n, _)| *n != "Proof").map(|(_, t)| t).collect();
    let n = hued.len() as f64;
    let mean = |f: &dyn Fn(&Tones) -> Rgb| {
        let (mut l, mut c) = (0.0, 0.0);
        for t in &hued {
            let (tl, tc, _) = to_lch(f(t));
            l += tl;
            c += tc;
        }
        (l / n, c / n)
    };
    [mean(&|t| t.rule), mean(&|t| t.back), mean(&|t| t.head)]
}

/// WCAG contrast ratio.
pub fn contrast(a: Rgb, b: Rgb) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    ((hi + 0.05) / (lo + 0.05)) as f32
}

/// WCAG relative luminance.
pub fn luminance(c: Rgb) -> f64 {
    let ch = |v: u8| {
        let s = v as f64 / 255.0;
        if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * ch(c.0) + 0.7152 * ch(c.1) + 0.0722 * ch(c.2)
}

/// `1E47B8`: six uppercase hex digits, no `#`.
pub fn hex(c: Rgb) -> String {
    format!("{:02X}{:02X}{:02X}", c.0, c.1, c.2)
}

/// `1E47B8` / `#1e47b8` → Rgb.
pub fn parse_hex(s: &str) -> Option<Rgb> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
    Some(Rgb(v(0)?, v(2)?, v(4)?))
}

/// ΔE in OKLab (Euclidean distance, ×100 so 1 ≈ just noticeable).
pub fn delta_e(a: Rgb, b: Rgb) -> f32 {
    let (la, aa, ba) = to_oklab(a);
    let (lb, ab, bb) = to_oklab(b);
    (((la - lb).powi(2) + (aa - ab).powi(2) + (ba - bb).powi(2)).sqrt() * 100.0) as f32
}

// ---------------------------------------------------------------------------
// OKLab

fn to_linear(v: u8) -> f64 {
    let s = v as f64 / 255.0;
    if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
}

fn from_linear(v: f64) -> f64 {
    if v <= 0.0031308 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

/// sRGB → OKLab (L, a, b).
pub fn to_oklab(c: Rgb) -> (f64, f64, f64) {
    let (r, g, b) = (to_linear(c.0), to_linear(c.1), to_linear(c.2));
    let l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
    let m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
    let s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;
    let (l, m, s) = (l.cbrt(), m.cbrt(), s.cbrt());
    (
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    )
}

/// OKLab → linear sRGB (may be outside 0…1).
fn oklab_to_linear(l: f64, a: f64, b: f64) -> (f64, f64, f64) {
    let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = l - 0.0894841775 * a - 1.2914855480 * b;
    let (l, m, s) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    (
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    )
}

/// sRGB → OKLCH (L, C, h in degrees 0…360).
pub fn to_lch(c: Rgb) -> (f64, f64, f64) {
    let (l, a, b) = to_oklab(c);
    let h = b.atan2(a).to_degrees().rem_euclid(360.0);
    (l, (a * a + b * b).sqrt(), h)
}

fn lch_linear(l: f64, c: f64, h: f64) -> (f64, f64, f64) {
    let r = h.to_radians();
    oklab_to_linear(l, c * r.cos(), c * r.sin())
}

fn in_gamut((r, g, b): (f64, f64, f64)) -> bool {
    let ok = |v: f64| (-1e-7..=1.0 + 1e-7).contains(&v);
    ok(r) && ok(g) && ok(b)
}

/// OKLCH → sRGB, reducing chroma (bisection) until it is inside the gamut.
fn from_lch_clipped(l: f64, c: f64, h: f64) -> Rgb {
    let mut lin = lch_linear(l, c, h);
    if !in_gamut(lin) {
        let (mut lo, mut hi) = (0.0, c);
        for _ in 0..40 {
            let mid = (lo + hi) / 2.0;
            if in_gamut(lch_linear(l, mid, h)) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lin = lch_linear(l, lo, h);
    }
    let q = |v: f64| (from_linear(v.clamp(0.0, 1.0)) * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgb(q(lin.0), q(lin.1), q(lin.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_and_contrast() {
        assert_eq!(hex(Rgb(0x1e, 0x47, 0xb8)), "1E47B8");
        assert_eq!(hex(Rgb(0, 10, 255)), "000AFF");
        assert_eq!(parse_hex("#1e47b8"), Some(Rgb(0x1E, 0x47, 0xB8)));
        assert!((contrast(Rgb(0, 0, 0), Rgb(255, 255, 255)) - 21.0).abs() < 1e-3);
        assert!((contrast(Rgb(255, 255, 255), Rgb(255, 255, 255)) - 1.0).abs() < 1e-6);
        // symmetric
        assert_eq!(contrast(INK, Rgb(0xE0, 0xE7, 0xFB)), contrast(Rgb(0xE0, 0xE7, 0xFB), INK));
    }

    #[test]
    fn palette_matches_the_sty() {
        // the palette lives in ntex-base.sty (shared by ntex-notes.sty and
        // ntex-figure.sty); older themes had it in ntex-notes.sty
        let dir = std::path::PathBuf::from(std::env::var("HOME").unwrap()).join(".config/texnvim/templates/tex");
        let read = |n: &str| std::fs::read_to_string(dir.join(n)).unwrap_or_default();
        let sty = read("ntex-notes.sty") + &read("ntex-base.sty");
        if sty.is_empty() {
            return;
        }
        for (name, t) in palette() {
            if name == "Proof" {
                continue;
            }
            for (tone, c) in [("Rule", t.rule), ("Back", t.back), ("Head", t.head)] {
                let line = format!("\\definecolor{{ntex{name}{tone}}}{{HTML}}{{{}}}", hex(c));
                assert!(sty.contains(&line), "{line}");
            }
        }
        assert!(sty.contains("\\definecolor{ntexProofRule}{gray}{0.55}"));
        assert!(sty.contains("\\definecolor{ntexProofBack}{gray}{0.955}"));
        assert!(sty.contains("\\definecolor{ntexInk}{HTML}{1E1E28}"));
    }

    #[test]
    fn oklab_round_trip() {
        for c in [Rgb(0, 0, 0), Rgb(255, 255, 255), Rgb(0x1E, 0x47, 0xB8), Rgb(255, 0, 0), Rgb(12, 200, 99)] {
            let (l, a, b) = to_oklab(c);
            let lin = oklab_to_linear(l, a, b);
            let q = |v: f64| (from_linear(v.clamp(0.0, 1.0)) * 255.0).round() as u8;
            assert_eq!(Rgb(q(lin.0), q(lin.1), q(lin.2)), c);
        }
    }

    /// Each palette entry's own hue gives back (nearly) its own tones.
    #[test]
    fn palette_hues_round_trip() {
        let mut report = Vec::new();
        let mut worst: f32 = 0.0;
        for (name, t) in palette() {
            if name == "Proof" {
                continue;
            }
            let g = tones_for_hue(hue_of(t.rule));
            let d = [delta_e(g.rule, t.rule), delta_e(g.back, t.back), delta_e(g.head, t.head)];
            report.push(format!("{name}: hue {:.0}° ΔE rule {:.1} back {:.1} head {:.1} → {} {} {}", hue_of(t.rule), d[0], d[1], d[2], hex(g.rule), hex(g.back), hex(g.head)));
            // The palette varies lightness by hue on purpose (blue and violet
            // rules sit at L 0.445, teal and green at 0.56), so a mean can't
            // land on every one. The one outlier: Violet's rule (ΔE ≈ 8, the
            // most saturated tone in the theme).
            assert!(d[1] < 3.0, "{name} back {}", d[1]);
            assert!(d[2] < 6.5, "{name} head {}", d[2]);
            if name != "Violet" {
                assert!(d[0] < 6.5, "{name} rule {}", d[0]);
            }
            worst = worst.max(d[0]).max(d[1]).max(d[2]);
        }
        eprintln!("{}", report.join("\n"));
        assert!(worst < 9.0, "{}", report.join("\n"));
    }

    #[test]
    fn new_hues_keep_contrast_and_gamut() {
        for h in (0..360).step_by(5) {
            let t = tones_for_hue(h as f32);
            assert!(contrast(INK, t.back) >= 7.0, "{h}: body {}", contrast(INK, t.back));
            assert!(contrast(t.head, t.back) >= 4.5, "{h}: head {}", contrast(t.head, t.back));
            // the rule reads as its hue (unless clipped to grey, which never happens here)
            let (_, c, hh) = to_lch(t.rule);
            assert!(c > 0.05, "{h}: chroma {c}");
            let dh = ((hh - h as f64 + 540.0) % 360.0 - 180.0).abs();
            assert!(dh < 4.0, "{h}: hue came back as {hh}");
        }
        // 145° is green, near the palette's Green
        let g = tones_for_hue(145.0);
        assert!(delta_e(g.rule, palette_tones("Green").unwrap().rule) < 10.0);
        assert_eq!(tones_for_hue(-215.0), g);
    }

    #[test]
    fn tones_of_colours() {
        assert_eq!(tones(&BlockColour::Palette("blue".into())).rule, Rgb(0x1E, 0x47, 0xB8));
        assert_eq!(tones(&BlockColour::Palette("Grey".into())).back, Rgb(0xF4, 0xF4, 0xF4));
        assert_eq!(tones(&BlockColour::Palette("Nope".into())).head, INK);
        assert_eq!(tones(&BlockColour::Hue(200.0)), tones_for_hue(200.0));
    }
}
