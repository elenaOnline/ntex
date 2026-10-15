//! The LaTeX look as tokens (UX.md "The look"). Colours are the notes theme's
//! (templates/tex/ntex-notes.sty); type is Latin Modern (or TeX Gyre Pagella
//! when prefs say "Palatino").
//!
//! Use the `Font` helpers (`theme.serif()`, `theme.caps()`, `theme.mono()`)
//! rather than the bare family names: the caps face of Pagella is the regular
//! face with the `smcp` feature, which a family name alone can't say.
use gpui::{App, Font, FontFeatures, FontStyle, FontWeight, Global, Hsla, Pixels, SharedString, px, rgb};
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub struct BoxTones {
    pub rule: Hsla,
    pub back: Hsla,
    pub head: Hsla,
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub paper: Hsla,
    pub ink: Hsla,
    pub slate: Hsla,
    /// \hrule: thin rules, field underlines, \fbox borders
    pub rule: Hsla,
    pub blue: Hsla,
    pub error: Hsla,
    pub theorem: BoxTones,
    pub lemma: BoxTones,
    pub definition: BoxTones,
    pub example: BoxTones,
    pub remark: BoxTones,
    pub proof: BoxTones,
    pub serif: SharedString,
    pub caps: SharedString,
    pub mono: SharedString,
    pub math: SharedString,
    /// \normalsize
    pub body_size: Pixels,

    // ---- added by WIDGETS ------------------------------------------------
    /// "Computer Modern" | "Palatino", as passed to `Theme::latex`.
    pub typeface: SharedString,
    /// OpenType features for the caps face (`smcp` when the serif is used for
    /// small caps, i.e. Pagella; none for Latin Modern Roman Caps).
    pub caps_features: FontFeatures,
    /// The proof block's □ (`ntexProofRule!80!black`).
    pub qed: Hsla,
    /// Text selection fill (theorem back, a touch stronger).
    pub selection: Hsla,
    /// The running head's rule: ink at 60 %.
    pub head_rule: Hsla,
    /// \section: 21 px bold.
    pub section_size: Pixels,
    /// \small: notes, keycap-adjacent copy, the toast.
    pub small_size: Pixels,
    /// \footnotesize: the hints at the bottom.
    pub footnote_size: Pixels,
    /// \texttt in fields and the output editor.
    pub mono_size: Pixels,
    /// The generated code in the proof box.
    pub code_size: Pixels,
    /// The trigger field: `\verb|ff|` set large.
    pub trigger_size: Pixels,
    /// Token chips' labels.
    pub chip_size: Pixels,
    /// \keys keycaps.
    pub keycap_size: Pixels,
    /// Superscripts (footnote marks, page counts).
    pub sup_size: Pixels,
    /// Line height as a multiple of the size (\baselinestretch-ish 1.25).
    pub leading: f32,
    /// Page margins: 44 px left/right, 28 px top under the running head.
    pub margin_x: Pixels,
    pub margin_top: Pixels,
    /// tcolorbox before/after skip.
    pub box_skip: Pixels,
}

impl Global for Theme {}

impl Theme {
    /// typeface: "Computer Modern" | "Palatino"
    pub fn latex(typeface: &str) -> Theme {
        let h = |c: u32| -> Hsla { rgb(c).into() };
        let tones = |r, b, hd| BoxTones { rule: h(r), back: h(b), head: h(hd) };
        let palatino = typeface.eq_ignore_ascii_case("palatino") || typeface.contains("Pagella");
        let (serif, caps, caps_features): (SharedString, SharedString, FontFeatures) = if palatino {
            (
                "TeX Gyre Pagella".into(),
                "TeX Gyre Pagella".into(),
                FontFeatures(Arc::new(vec![("smcp".into(), 1)])),
            )
        } else {
            ("Latin Modern Roman".into(), "Latin Modern Roman Caps".into(), FontFeatures::default())
        };
        let mut ink_60 = h(0x1E1E28);
        ink_60.a = 0.6;
        let mut selection = h(0x1E47B8);
        selection.a = 0.16;
        Theme {
            paper: h(0xFFFFFF),
            ink: h(0x1E1E28),
            slate: h(0x5A6378),
            rule: h(0x1E1E28),
            blue: h(0x1E47B8),
            error: h(0xA9182D),
            theorem: tones(0x1E47B8, 0xE0E7FB, 0x153079),
            lemma: tones(0x661EB8, 0xECE0FB, 0x441579),
            definition: tones(0x1C829C, 0xE2F4F8, 0x0F596B),
            example: tones(0xA92D4A, 0xF8EAED, 0x79152D),
            remark: tones(0x238B45, 0xE5F5EA, 0x145D2C),
            // {gray}{0.55} rule, {gray}{0.955} back, ink head
            proof: tones(0x8C8C8C, 0xF4F4F4, 0x1E1E28),
            serif,
            caps,
            mono: "Latin Modern Mono".into(),
            math: "Latin Modern Math".into(),
            body_size: px(15.),
            typeface: if palatino { "Palatino".into() } else { "Computer Modern".into() },
            caps_features,
            qed: h(0x707070),
            selection,
            head_rule: ink_60,
            section_size: px(21.),
            small_size: px(13.),
            footnote_size: px(12.),
            mono_size: px(14.),
            code_size: px(13.),
            trigger_size: px(26.),
            chip_size: px(11.),
            keycap_size: px(12.),
            sup_size: px(9.),
            leading: 1.25,
            margin_x: px(44.),
            margin_top: px(28.),
            box_skip: px(12.),
        }
    }

    fn face(family: &SharedString, features: FontFeatures, weight: FontWeight, style: FontStyle) -> Font {
        Font { family: family.clone(), features, fallbacks: None, weight, style }
    }

    /// The body serif, upright.
    pub fn serif(&self) -> Font {
        Self::face(&self.serif, FontFeatures::default(), FontWeight::NORMAL, FontStyle::Normal)
    }
    pub fn serif_italic(&self) -> Font {
        Self::face(&self.serif, FontFeatures::default(), FontWeight::NORMAL, FontStyle::Italic)
    }
    pub fn serif_bold(&self) -> Font {
        Self::face(&self.serif, FontFeatures::default(), FontWeight::BOLD, FontStyle::Normal)
    }
    pub fn serif_bold_italic(&self) -> Font {
        Self::face(&self.serif, FontFeatures::default(), FontWeight::BOLD, FontStyle::Italic)
    }
    /// \textsc: write the label in its natural case ("Trigger"); the face
    /// makes the lowercase small caps.
    pub fn caps(&self) -> Font {
        Self::face(&self.caps, self.caps_features.clone(), FontWeight::NORMAL, FontStyle::Normal)
    }
    pub fn mono(&self) -> Font {
        Self::face(&self.mono, FontFeatures::disable_ligatures(), FontWeight::NORMAL, FontStyle::Normal)
    }
    pub fn mono_italic(&self) -> Font {
        Self::face(&self.mono, FontFeatures::disable_ligatures(), FontWeight::NORMAL, FontStyle::Italic)
    }
    /// Sloped parentheses for an italic head: Pagella's italic ones are
    /// upright, so in Palatino mode they come from LM Roman Italic.
    pub fn italic_parens(&self) -> Font {
        if self.typeface == "Palatino" {
            Self::face(&"Latin Modern Roman".into(), FontFeatures::default(), FontWeight::NORMAL, FontStyle::Italic)
        } else {
            self.serif_italic()
        }
    }
    /// The key symbols (⌘ ⇧ ⌥ ⌫ ↵ ⇥), in hints and keycaps alike: neither
    /// Latin Modern nor its Mono has them, and Pagella's own ⇧ is a narrow
    /// fallback-looking arrow. macOS's UI face has the standard set, drawn
    /// the way the menus draw them; it is used for these symbols only.
    pub fn keys_font(&self) -> Font {
        Self::face(&".SystemUIFont".into(), FontFeatures::default(), FontWeight::NORMAL, FontStyle::Normal)
    }
    /// Latin Modern Math (●, ○, □, ☐-likes, arrows).
    pub fn math_font(&self) -> Font {
        Self::face(&self.math, FontFeatures::default(), FontWeight::NORMAL, FontStyle::Normal)
    }

    /// The three tones of a box kind.
    pub fn tones(&self, kind: super::boxes::BoxKind) -> BoxTones {
        use super::boxes::BoxKind::*;
        match kind {
            Theorem => self.theorem,
            Lemma => self.lemma,
            Definition => self.definition,
            Example => self.example,
            Remark => self.remark,
            Proof => self.proof,
        }
    }

    /// `size * leading`, the line box for a size.
    pub fn line(&self, size: Pixels) -> Pixels {
        size * self.leading
    }

    /// A colour at an opacity.
    pub fn alpha(c: Hsla, a: f32) -> Hsla {
        Hsla { a: c.a * a, ..c }
    }

    /// \textcolor from a model `Rgb`.
    pub fn rgb_of(c: crate::model::Rgb) -> Hsla {
        rgb((u32::from(c.0) << 16) | (u32::from(c.1) << 8) | u32::from(c.2)).into()
    }
}

pub fn init(cx: &mut App) {
    cx.set_global(Theme::latex("Computer Modern"));
}

/// Swap the serif for `prefs.typeface` ("Palatino" → TeX Gyre Pagella) and
/// redraw every window.
pub fn set_typeface(typeface: &str, cx: &mut App) {
    cx.set_global(Theme::latex(typeface));
    cx.refresh_windows();
}

pub trait ActiveTheme {
    fn theme(&self) -> &Theme;
}

impl ActiveTheme for App {
    fn theme(&self) -> &Theme {
        self.global::<Theme>()
    }
}
