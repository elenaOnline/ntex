//! The editor look ntex paints with: `look.json` in the config folder, read
//! and written whole. ntex (`lua/config/look.lua`) watches the file, so a
//! change made here shows in every open ntex at once. The choices mirror
//! look.lua's `CHOICES`; the defaults are the author's pick from the ntex Syntax
//! Lab (2026-10-18).
use crate::model::Paths;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Look {
    pub palette: String,
    pub sections: String,
    pub kinds: String,
    pub lines: String,
    pub guides: bool,
    pub ink: String,
    /// The Look page's color edits: palette → role → "#rrggbb", painted over
    /// that palette (lua/config/palettes.lua M.ROLES).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub colors: BTreeMap<String, BTreeMap<String, String>>,
    /// The fine structure settings (structure.lua `M.FINE`).
    #[serde(skip_serializing_if = "Structure::is_default")]
    pub structure: Structure,
}

/// How the structure marks lay out: glyphs, which rules and lines show, and
/// the shades of the whole begin/end lines. Defaults are the designed look.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Structure {
    pub rule_glyph: String,
    pub subrule: String,
    pub subrule_len: u32,
    pub numbers: String,
    pub bar_glyph: String,
    pub bar_side: String,
    pub guide_glyph: String,
    pub guide_scope: String,
    pub title_bold: bool,
    pub begin_shade: u32,
    pub brace_tint: u32,
    pub guide_fade: u32,
}

impl Default for Structure {
    fn default() -> Structure {
        Structure {
            rule_glyph: "─".into(),
            subrule: "short".into(),
            subrule_len: 24,
            numbers: "end".into(),
            bar_glyph: "▎".into(),
            bar_side: "right".into(),
            guide_glyph: "│".into(),
            guide_scope: "all".into(),
            title_bold: true,
            begin_shade: 38,
            brace_tint: 35,
            guide_fade: 62,
        }
    }
}

/// The structure choices the Look page offers: (key, label, options as (value, label)).
pub const STRUCTURE_CHOICES: &[(&str, &str, &[(&str, &str)])] = &[
    ("rule_glyph", "Section rule", &[("─", "─ thin"), ("━", "━ heavy"), ("┄", "┄ dashed"), ("═", "═ double"), ("·", "· dotted")]),
    ("subrule", "Subsection rule", &[("none", "None"), ("short", "Short"), ("full", "Full width")]),
    ("numbers", "Section numbers", &[("end", "§2.1 at the end"), ("none", "None")]),
    ("bar_glyph", "Gutter bar", &[("▏", "▏ hairline"), ("▎", "▎ thin"), ("▌", "▌ half"), ("┃", "┃ heavy line"), ("█", "█ full")]),
    ("bar_side", "Bar side", &[("right", "Right of the number"), ("left", "Left of the number")]),
    ("guide_glyph", "Nesting line", &[("│", "│ solid"), ("┊", "┊ dotted"), ("╎", "╎ dashed"), ("▏", "▏ hairline")]),
    ("guide_scope", "Lines for", &[("all", "Every block and list"), ("blocks", "Blocks only")]),
];

/// The structure sliders: (key, label, min, max, unit).
pub const STRUCTURE_SLIDERS: &[(&str, &str, u32, u32, &str)] = &[
    ("subrule_len", "Short rule", 8, 60, " cols"),
    ("begin_shade", "\\begin shade", 0, 70, "%"),
    ("brace_tint", "Brace tint", 0, 100, "%"),
    ("guide_fade", "Line fade", 20, 90, "%"),
];

impl Structure {
    pub fn is_default(&self) -> bool {
        *self == Structure::default()
    }

    pub fn get(&self, key: &str) -> &str {
        match key {
            "rule_glyph" => &self.rule_glyph,
            "subrule" => &self.subrule,
            "numbers" => &self.numbers,
            "bar_glyph" => &self.bar_glyph,
            "bar_side" => &self.bar_side,
            "guide_glyph" => &self.guide_glyph,
            "guide_scope" => &self.guide_scope,
            _ => "",
        }
    }

    pub fn set(&mut self, key: &str, value: &str) -> bool {
        let Some((_, _, options)) = STRUCTURE_CHOICES.iter().find(|(k, _, _)| *k == key) else { return false };
        if !options.iter().any(|(v, _)| *v == value) {
            return false;
        }
        let slot = match key {
            "rule_glyph" => &mut self.rule_glyph,
            "subrule" => &mut self.subrule,
            "numbers" => &mut self.numbers,
            "bar_glyph" => &mut self.bar_glyph,
            "bar_side" => &mut self.bar_side,
            "guide_glyph" => &mut self.guide_glyph,
            _ => &mut self.guide_scope,
        };
        *slot = value.to_string();
        true
    }

    pub fn index(&self, key: &str) -> usize {
        STRUCTURE_CHOICES
            .iter()
            .find(|(k, _, _)| *k == key)
            .and_then(|(_, _, o)| o.iter().position(|(v, _)| *v == self.get(key)))
            .unwrap_or(0)
    }

    pub fn number(&self, key: &str) -> u32 {
        match key {
            "subrule_len" => self.subrule_len,
            "begin_shade" => self.begin_shade,
            "brace_tint" => self.brace_tint,
            _ => self.guide_fade,
        }
    }

    pub fn set_number(&mut self, key: &str, v: u32) {
        let Some((_, _, lo, hi, _)) = STRUCTURE_SLIDERS.iter().find(|(k, ..)| *k == key) else { return };
        let v = v.clamp(*lo, *hi);
        match key {
            "subrule_len" => self.subrule_len = v,
            "begin_shade" => self.begin_shade = v,
            "brace_tint" => self.brace_tint = v,
            _ => self.guide_fade = v,
        }
    }

    /// Unknown values fall back to the default.
    fn cleaned(mut self) -> Structure {
        let d = Structure::default();
        for (k, _, _) in STRUCTURE_CHOICES {
            let v = self.get(k).to_string();
            if !self.set(k, &v) {
                self.set(k, d.get(k));
            }
        }
        for (k, ..) in STRUCTURE_SLIDERS {
            let v = self.number(k);
            self.set_number(k, v);
        }
        self
    }
}

impl Default for Look {
    fn default() -> Look {
        Look {
            palette: "slate".into(),
            sections: "rule".into(),
            kinds: "bar".into(),
            lines: "whole".into(),
            guides: true,
            ink: "content".into(),
            colors: BTreeMap::new(),
            structure: Structure::default(),
        }
    }
}

/// One setting: its look.json key, its label, and its (value, label) options.
pub struct Setting {
    pub key: &'static str,
    pub label: &'static str,
    pub options: &'static [(&'static str, &'static str)],
}

/// Her palettes first (`CUSTOM` of them), then the standard editor themes to
/// compare against (lua/config/palettes.lua `M.standard`).
pub const PALETTES: &[(&str, &str)] = &[
    ("slate", "Slate"),
    ("code", "Code"),
    ("code-dark", "Code Dark"),
    ("alabaster", "Alabaster"),
    ("vscode-light", "VS Code Light+"),
    ("vscode-dark", "VS Code Dark+"),
    ("github-light", "GitHub Light"),
    ("one-light", "One Light"),
    ("solarized-light", "Solarized Light"),
    ("solarized-dark", "Solarized Dark"),
    ("catppuccin-latte", "Catppuccin Latte"),
    ("catppuccin-mocha", "Catppuccin Mocha"),
    ("tokyonight", "Tokyo Night"),
    ("gruvbox-dark", "Gruvbox Dark"),
    ("newpaper-light", "Newpaper Light"),
    ("newpaper-dark", "Newpaper Dark"),
];
pub const CUSTOM: usize = 4;

/// A standard theme (its own colors; the ink setting doesn't apply).
pub fn is_standard(palette: &str) -> bool {
    PALETTES.iter().position(|(v, _)| *v == palette).is_some_and(|i| i >= CUSTOM)
}

/// The string settings, in the order the Look page shows them (guides is the checkbox).
pub const SETTINGS: &[Setting] = &[
    Setting { key: "palette", label: "Palette", options: PALETTES },
    Setting { key: "sections", label: "Sections", options: &[("none", "None"), ("rule", "Rule"), ("band", "Band")] },
    Setting { key: "kinds", label: "Blocks", options: &[("none", "Plain"), ("color", "Names"), ("bar", "Names + bar")] },
    Setting { key: "lines", label: "Begin/end", options: &[("name", "Name only"), ("whole", "Whole line")] },
    Setting { key: "ink", label: "Black ink", options: &[("content", "Content only"), ("all", "Commands too")] },
];

impl Look {
    pub fn get(&self, key: &str) -> &str {
        match key {
            "palette" => &self.palette,
            "sections" => &self.sections,
            "kinds" => &self.kinds,
            "lines" => &self.lines,
            "ink" => &self.ink,
            _ => "",
        }
    }

    /// Set a string setting; false (and no change) for an unknown key or value.
    pub fn set(&mut self, key: &str, value: &str) -> bool {
        let Some(s) = SETTINGS.iter().find(|s| s.key == key) else { return false };
        if !s.options.iter().any(|(v, _)| *v == value) {
            return false;
        }
        let slot = match key {
            "palette" => &mut self.palette,
            "sections" => &mut self.sections,
            "kinds" => &mut self.kinds,
            "lines" => &mut self.lines,
            _ => &mut self.ink,
        };
        *slot = value.to_string();
        true
    }

    /// Index of the current value among `key`'s options (0 if unknown).
    pub fn index(&self, key: &str) -> usize {
        SETTINGS
            .iter()
            .find(|s| s.key == key)
            .and_then(|s| s.options.iter().position(|(v, _)| *v == self.get(key)))
            .unwrap_or(0)
    }

    /// Unknown values (a hand edit, an older file) fall back to the default.
    fn cleaned(mut self) -> Look {
        let d = Look::default();
        for s in SETTINGS {
            let v = self.get(s.key).to_string();
            if !self.set(s.key, &v) {
                self.set(s.key, d.get(s.key));
            }
        }
        self.structure = self.structure.clone().cleaned();
        self.colors.retain(|pal, roles| {
            roles.retain(|role, c| ROLES.iter().any(|(r, _)| r == role) && parse_hex(c).is_some());
            for c in roles.values_mut() {
                *c = c.to_lowercase();
            }
            PALETTES.iter().any(|(p, _)| p == pal) && !roles.is_empty()
        });
        self
    }
}

/// Every color of a palette the Look page edits: (role, label). The kinds
/// (capitalized) are the block colors.
pub const ROLES: &[(&str, &str)] = &[
    ("bg", "Page"),
    ("ink", "Text"),
    ("math", "Math"),
    ("mathcmd", "Math commands"),
    ("mathdelim", "Math $ $"),
    ("markup", "Commands"),
    ("quiet", "Braces, options"),
    ("comment", "Comments"),
    ("file", "File names"),
    ("Statement", "Theorems"),
    ("Definition", "Definitions"),
    ("Example", "Examples"),
    ("Proof", "Proofs"),
    ("Problem", "Problems"),
    ("Layout", "Lists, layout"),
];

/// A palette's own colors, in `ROLES` order (read from a headless ntex:
/// palettes.lua `M.base` after painting each; regenerate when palettes change).
fn bases(palette: &str) -> [u32; 15] {
    match palette {
        "code" => [0xffffff, 0x1f2328, 0x0550ae, 0xbf1d78, 0x8c959f, 0xcf222e, 0x8c959f, 0x6e7781, 0x0a3069, 0x7a3e9d, 0x2e7d32, 0x8a5a00, 0x56647a, 0x0d6e8f, 0x8a8a8a],
        "code-dark" => [0x0f1318, 0xe6edf3, 0x79c0ff, 0xf78fc6, 0x6e7681, 0xff7b72, 0x6e7681, 0x8b949e, 0xa5d6ff, 0xc7a0e6, 0x7fd08a, 0xe3b55c, 0xa8b4c8, 0x5cc8e6, 0x7a7f8a],
        "alabaster" => [0xffffff, 0x000000, 0x325cc0, 0x325cc0, 0x777777, 0x6b7180, 0xa3a8b2, 0xaa3731, 0xa3a8b2, 0x7a3e9d, 0x2e7d32, 0x8a5a00, 0x56647a, 0x0d6e8f, 0x8a8a8a],
        "vscode-light" => [0xffffff, 0x343434, 0x800000, 0xaf00db, 0x343434, 0xaf00db, 0x343434, 0x008000, 0xaf00db, 0xaf00db, 0xa31515, 0x098658, 0x008000, 0x0000ff, 0x098658],
        "vscode-dark" => [0x1f1f1f, 0xd4d4d4, 0xd7ba7d, 0xc586c0, 0xd4d4d4, 0xc586c0, 0xd4d4d4, 0x6a9955, 0xc586c0, 0xc586c0, 0xce9178, 0xb5cea8, 0x6a9955, 0x569cd6, 0x5a5a5a],
        "github-light" => [0xffffff, 0x1f2328, 0x1f2328, 0xcf222e, 0x1f2328, 0xcf222e, 0x1f2328, 0x57606a, 0xcf222e, 0xcf222e, 0x0a3069, 0x0550ae, 0x57606a, 0x953800, 0x6e7781],
        "one-light" => [0xfafafa, 0x6a6a6a, 0x118dc3, 0x9a77cf, 0x6a6a6a, 0x9a77cf, 0x6a6a6a, 0x9b9fa6, 0x9a77cf, 0x9a77cf, 0x1da912, 0xee9025, 0x9b9fa6, 0xeea825, 0xcccccc],
        "solarized-light" => [0xfdf6e3, 0x657b83, 0x6c71c4, 0x859900, 0x657b83, 0x859900, 0x657b83, 0x93a1a1, 0xcb4b16, 0x586e75, 0x2aa198, 0xd33682, 0x93a1a1, 0xb58900, 0x586e75],
        "solarized-dark" => [0x002b36, 0x839496, 0x6c71c4, 0x859900, 0x839496, 0x859900, 0x839496, 0x586e75, 0x93a1a1, 0x93a1a1, 0x2aa198, 0x6c71c4, 0x586e75, 0xcb4b16, 0x586e75],
        "catppuccin-latte" => [0xeff1f5, 0x4c4f69, 0xea76cb, 0x8839ef, 0x7c7f93, 0x8839ef, 0x7c7f93, 0x7c7f93, 0x8839ef, 0x8839ef, 0x40a02b, 0xfe640b, 0x7c7f93, 0xdf8e1d, 0xbcc0cc],
        "catppuccin-mocha" => [0x1e1e2e, 0xcdd6f4, 0xf5c2e7, 0xcba6f7, 0x9399b2, 0xcba6f7, 0x9399b2, 0x9399b2, 0xcba6f7, 0xcba6f7, 0xa6e3a1, 0xfab387, 0x9399b2, 0xf9e2af, 0x45475a],
        "tokyonight" => [0x1a1b26, 0xc0caf5, 0x2ac3de, 0xbb9af7, 0x2ac3de, 0xbb9af7, 0x2ac3de, 0x565f89, 0x7dcfff, 0x7dcfff, 0x9ece6a, 0xff9e64, 0x565f89, 0x2ac3de, 0x3b4261],
        "gruvbox-dark" => [0x282828, 0xebdbb2, 0xfe8019, 0xfb4934, 0xfe8019, 0xfb4934, 0xfe8019, 0x928374, 0x8ec07c, 0xfb4934, 0xb8bb26, 0xd3869b, 0x928374, 0xfabd2f, 0x7c6f64],
        "newpaper-light" => [0xf1f3f2, 0x2f2a3d, 0x008000, 0x636d1d, 0x89802b, 0x0089b3, 0x684d99, 0x878787, 0x1e40c2, 0x8700af, 0x0072c1, 0xe04a4a, 0x878787, 0x007200, 0x878787],
        "newpaper-dark" => [0x303030, 0xc6c8cd, 0x5faf5f, 0x989769, 0xa9a875, 0x6da6cf, 0x8e8ebb, 0x787878, 0x738ae5, 0xa274d1, 0x7db1d5, 0xf76b68, 0x787878, 0x589a58, 0x787878],
        _ => [0xffffff, 0x111111, 0x2a2f6b, 0x5a5fa8, 0xb5b8d6, 0x7b8fae, 0xc3cddc, 0xa8455a, 0x9fb0c8, 0x7a3e9d, 0x2e7d32, 0x8a5a00, 0x56647a, 0x0d6e8f, 0x8a8a8a],
    }
}

/// The palette's own color for a role (0 for an unknown role).
pub fn base_color(palette: &str, role: &str) -> u32 {
    ROLES.iter().position(|(r, _)| *r == role).map(|i| bases(palette)[i]).unwrap_or(0)
}

pub fn parse_hex(s: &str) -> Option<u32> {
    let h = s.trim().trim_start_matches('#');
    (h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit())).then(|| u32::from_str_radix(h, 16).ok()).flatten()
}

pub fn hex(c: u32) -> String {
    format!("#{:06x}", c & 0xffffff)
}

/// RGB → (hue 0–360, saturation 0–1, value 0–1).
pub fn to_hsv(c: u32) -> (f32, f32, f32) {
    let r = ((c >> 16) & 0xff) as f32 / 255.0;
    let g = ((c >> 8) & 0xff) as f32 / 255.0;
    let b = (c & 0xff) as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let s = if max == 0.0 { 0.0 } else { d / max };
    (h, s, max)
}

/// (hue 0–360, saturation 0–1, value 0–1) → RGB.
pub fn from_hsv(h: f32, s: f32, v: f32) -> u32 {
    let (s, v) = (s.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
    let c = v * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let to = |u: f32| (((u + m) * 255.0).round() as u32).min(255);
    (to(r) << 16) | (to(g) << 8) | to(b)
}

impl Look {
    /// The current palette's color for a role: her edit, else the palette's own.
    pub fn color(&self, role: &str) -> u32 {
        self.colors.get(&self.palette).and_then(|m| m.get(role)).and_then(|c| parse_hex(c)).unwrap_or_else(|| base_color(&self.palette, role))
    }

    pub fn edited(&self, role: &str) -> bool {
        self.colors.get(&self.palette).is_some_and(|m| m.contains_key(role))
    }

    /// Set a role's color on the current palette (back to the palette's own: the edit goes).
    pub fn set_color(&mut self, role: &str, c: u32) {
        if !ROLES.iter().any(|(r, _)| *r == role) {
            return;
        }
        if c == base_color(&self.palette, role) {
            self.reset_color(role);
            return;
        }
        self.colors.entry(self.palette.clone()).or_default().insert(role.to_string(), hex(c));
    }

    pub fn reset_color(&mut self, role: &str) {
        if let Some(m) = self.colors.get_mut(&self.palette) {
            m.remove(role);
            if m.is_empty() {
                self.colors.remove(&self.palette);
            }
        }
    }

    /// Every edit on the current palette goes.
    pub fn reset_colors(&mut self) {
        self.colors.remove(&self.palette);
    }

    /// The sample's colors: the palette's, with her edits.
    pub fn swatch(&self) -> Swatch {
        let mut s = swatch(&self.palette);
        if let Some(m) = self.colors.get(&self.palette) {
            for (role, c) in m {
                let Some(c) = parse_hex(c) else { continue };
                match role.as_str() {
                    "bg" => s.bg = c,
                    "ink" => s.ink = c,
                    "math" => s.math = c,
                    "mathcmd" => s.mathcmd = c,
                    "markup" => {
                        s.markup = c;
                        s.env = c;
                    }
                    "quiet" => s.quiet = c,
                    "comment" => s.comment = c,
                    "Definition" => s.definition = c,
                    _ => {}
                }
            }
        }
        s
    }
}

pub fn path(paths: &Paths) -> PathBuf {
    paths.config.join("look.json")
}

/// look.json over the defaults; a missing or broken file gives the defaults.
pub fn read(paths: &Paths) -> Look {
    std::fs::read_to_string(path(paths)).map(|t| parse(&t)).unwrap_or_default()
}

pub fn parse(text: &str) -> Look {
    serde_json::from_str::<Look>(text).map(Look::cleaned).unwrap_or_default()
}

/// Write look.json whole (temp file + rename: ntex never reads half a file).
pub fn write(paths: &Paths, look: &Look) -> std::io::Result<()> {
    let p = path(paths);
    let tmp = p.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(look).map_err(std::io::Error::other)? + "\n";
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, &p)
}

/// How a palette draws LaTeX, for the Look page's sample line: the colors
/// ntex really uses (read from a headless ntex with each palette applied:
/// Normal, texMathZone, texMathCmd, texCmd, texEnvArgName, texDelim,
/// texComment, NtexKindDefinition). Regenerate when palettes.lua changes.
#[derive(Clone, Copy, Debug)]
pub struct Swatch {
    pub bg: u32,
    pub ink: u32,
    pub math: u32,
    pub mathcmd: u32,
    pub markup: u32,
    pub env: u32,
    pub quiet: u32,
    pub comment: u32,
    pub comment_italic: bool,
    pub definition: u32,
    pub standard: bool,
}

impl Swatch {
    pub fn dark(&self) -> bool {
        let ch = |s: u32| ((self.bg >> s) & 0xff) as f32;
        0.299 * ch(16) + 0.587 * ch(8) + 0.114 * ch(0) < 128.0
    }
}

pub fn swatch(palette: &str) -> Swatch {
    match palette {
        "code" => Swatch { bg: 0xffffff, ink: 0x1f2328, math: 0x0550ae, mathcmd: 0xbf1d78, markup: 0xcf222e, env: 0xcf222e, quiet: 0x8c959f, comment: 0x6e7781, comment_italic: true, definition: 0x2e7d32, standard: false },
        "code-dark" => Swatch { bg: 0x0f1318, ink: 0xe6edf3, math: 0x79c0ff, mathcmd: 0xf78fc6, markup: 0xff7b72, env: 0xff7b72, quiet: 0x6e7681, comment: 0x8b949e, comment_italic: true, definition: 0x7fd08a, standard: false },
        "alabaster" => Swatch { bg: 0xffffff, ink: 0x000000, math: 0x325cc0, mathcmd: 0x325cc0, markup: 0x6b7180, env: 0x6b7180, quiet: 0xa3a8b2, comment: 0xaa3731, comment_italic: false, definition: 0x2e7d32, standard: false },
        "vscode-light" => Swatch { bg: 0xffffff, ink: 0x343434, math: 0x800000, mathcmd: 0xaf00db, markup: 0xaf00db, env: 0xaf00db, quiet: 0x343434, comment: 0x008000, comment_italic: false, definition: 0xa31515, standard: true },
        "vscode-dark" => Swatch { bg: 0x1f1f1f, ink: 0xd4d4d4, math: 0xd7ba7d, mathcmd: 0xc586c0, markup: 0xc586c0, env: 0xc586c0, quiet: 0xd4d4d4, comment: 0x6a9955, comment_italic: false, definition: 0xce9178, standard: true },
        "github-light" => Swatch { bg: 0xffffff, ink: 0x1f2328, math: 0x1f2328, mathcmd: 0xcf222e, markup: 0xcf222e, env: 0xcf222e, quiet: 0x1f2328, comment: 0x57606a, comment_italic: false, definition: 0x0a3069, standard: true },
        "one-light" => Swatch { bg: 0xfafafa, ink: 0x6a6a6a, math: 0x118dc3, mathcmd: 0x9a77cf, markup: 0x9a77cf, env: 0xeea825, quiet: 0x6a6a6a, comment: 0x9b9fa6, comment_italic: false, definition: 0x1da912, standard: true },
        "solarized-light" => Swatch { bg: 0xfdf6e3, ink: 0x657b83, math: 0x6c71c4, mathcmd: 0x859900, markup: 0x859900, env: 0x859900, quiet: 0x657b83, comment: 0x93a1a1, comment_italic: false, definition: 0x2aa198, standard: true },
        "solarized-dark" => Swatch { bg: 0x002b36, ink: 0x839496, math: 0x6c71c4, mathcmd: 0x859900, markup: 0x859900, env: 0x859900, quiet: 0x839496, comment: 0x586e75, comment_italic: false, definition: 0x2aa198, standard: true },
        "catppuccin-latte" => Swatch { bg: 0xeff1f5, ink: 0x4c4f69, math: 0xea76cb, mathcmd: 0x8839ef, markup: 0x8839ef, env: 0xea76cb, quiet: 0x7c7f93, comment: 0x7c7f93, comment_italic: true, definition: 0x40a02b, standard: true },
        "catppuccin-mocha" => Swatch { bg: 0x1e1e2e, ink: 0xcdd6f4, math: 0xf5c2e7, mathcmd: 0xcba6f7, markup: 0xcba6f7, env: 0xf5c2e7, quiet: 0x9399b2, comment: 0x9399b2, comment_italic: true, definition: 0xa6e3a1, standard: true },
        "tokyonight" => Swatch { bg: 0x1a1b26, ink: 0xc0caf5, math: 0x2ac3de, mathcmd: 0xbb9af7, markup: 0xbb9af7, env: 0x7dcfff, quiet: 0x2ac3de, comment: 0x565f89, comment_italic: true, definition: 0x9ece6a, standard: true },
        "gruvbox-dark" => Swatch { bg: 0x282828, ink: 0xebdbb2, math: 0xfe8019, mathcmd: 0xfb4934, markup: 0xfb4934, env: 0x8ec07c, quiet: 0xfe8019, comment: 0x928374, comment_italic: true, definition: 0xb8bb26, standard: true },
        "newpaper-light" => Swatch { bg: 0xf1f3f2, ink: 0x2f2a3d, math: 0x008000, mathcmd: 0x636d1d, markup: 0x0089b3, env: 0xba6400, quiet: 0x684d99, comment: 0x878787, comment_italic: true, definition: 0x0072c1, standard: true },
        "newpaper-dark" => Swatch { bg: 0x303030, ink: 0xc6c8cd, math: 0x5faf5f, mathcmd: 0x989769, markup: 0x6da6cf, env: 0xd28f3f, quiet: 0x8e8ebb, comment: 0x787878, comment_italic: true, definition: 0x7db1d5, standard: true },
        _ => Swatch { bg: 0xffffff, ink: 0x111111, math: 0x2a2f6b, mathcmd: 0x5a5fa8, markup: 0x7b8fae, env: 0x7b8fae, quiet: 0xc3cddc, comment: 0xa8455a, comment_italic: false, definition: 0x2e7d32, standard: false },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_her_pick() {
        let l = Look::default();
        assert_eq!((l.palette.as_str(), l.sections.as_str(), l.kinds.as_str(), l.lines.as_str(), l.guides, l.ink.as_str()), ("slate", "rule", "bar", "whole", true, "content"));
    }

    #[test]
    fn parse_keeps_known_values_and_drops_the_rest() {
        let l = parse(r#"{"palette":"code-dark","sections":"wobbly","guides":false,"extra":1}"#);
        assert_eq!(l.palette, "code-dark");
        assert_eq!(l.sections, "rule");
        assert!(!l.guides);
        assert_eq!(parse("not json"), Look::default());
    }

    #[test]
    fn set_checks_the_value() {
        let mut l = Look::default();
        assert!(l.set("kinds", "color"));
        assert!(!l.set("kinds", "stripes"));
        assert!(!l.set("nope", "x"));
        assert_eq!(l.kinds, "color");
        assert_eq!(l.index("kinds"), 1);
        assert!(l.set("palette", "gruvbox-dark") && is_standard(&l.palette) && swatch("gruvbox-dark").dark());
        assert!(!is_standard("slate") && !swatch("slate").dark());
    }

    #[test]
    fn structure_settings() {
        let l = parse(r#"{"structure":{"rule_glyph":"━","subrule":"wavy","begin_shade":500}}"#);
        assert_eq!(l.structure.rule_glyph, "━");
        assert_eq!(l.structure.subrule, "short");
        assert_eq!(l.structure.begin_shade, 70);
        assert!(!serde_json::to_string(&Look::default()).unwrap().contains("structure"), "defaults aren't written");
    }

    #[test]
    fn hsv_round_trips() {
        for c in [0x2a2f6b, 0xffffff, 0x000000, 0x7fd08a, 0xcf222e, 0x0f1318] {
            let (h, s, v) = to_hsv(c);
            assert_eq!(from_hsv(h, s, v), c, "{c:06x}");
        }
        assert_eq!(parse_hex("#2A2F6B"), Some(0x2a2f6b));
        assert_eq!(parse_hex("2a2f6"), None);
    }

    #[test]
    fn color_edits_per_palette() {
        let mut l = Look::default();
        assert_eq!(l.color("math"), 0x2a2f6b);
        l.set_color("math", 0xff0000);
        assert!(l.edited("math") && l.color("math") == 0xff0000);
        l.set("palette", "code");
        assert!(!l.edited("math") && l.color("math") == 0x0550ae);
        l.set("palette", "slate");
        l.set_color("math", 0x2a2f6b); // back to Slate's own: the edit goes
        assert!(l.colors.is_empty());
        let p = parse(r##"{"colors":{"slate":{"math":"#FF0000","nope":"#000000","ink":"red"},"bogus":{"math":"#000000"}}}"##);
        assert_eq!(p.colors.len(), 1);
        assert_eq!(p.colors["slate"].get("math").map(String::as_str), Some("#ff0000"));
        assert_eq!(p.colors["slate"].len(), 1);
    }

    #[test]
    fn write_then_read() {
        let dir = std::env::temp_dir().join(format!("ntex-look-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let paths = Paths { config: dir.clone(), snippets: dir.clone(), templates_tex: dir.clone(), state: dir.clone() };
        let mut l = Look::default();
        l.set("palette", "code");
        write(&paths, &l).unwrap();
        assert_eq!(read(&paths), l);
        assert!(!dir.join("look.json.tmp").exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
