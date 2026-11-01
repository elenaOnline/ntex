//! The notes look: how the PDF of a note looks (ntex-notes.sty, ntex-base.sty).
//! Studio's Notes page keeps it in `notes-look.json` and writes two files the
//! theme reads (templates/tex, next to ntex-base.sty):
//!
//!   ntex-notes-fonts.tex   the typeface, instead of Charter (ntex-base.sty)
//!   ntex-notes-look.tex    block colors and box styling, at \begin{document}
//!
//! Each file exists only while it differs from the theme's own look, so with
//! the defaults the PDF is exactly the theme's.
use crate::model::Paths;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Every color of the notes palette: (xcolor name in ntex-base.sty, label, the theme's own value).
pub const COLORS: &[(&str, &str, u32)] = &[
    ("ntexInk", "Text", 0x1e1e28),
    ("ntexBlue", "Accent (numbers, bullets, title rule)", 0x1e47b8),
    ("ntexSlate", "Captions, author, date", 0x5a6378),
    ("ntexBlueRule", "Theorem bar", 0x1e47b8),
    ("ntexBlueBack", "Theorem fill", 0xe0e7fb),
    ("ntexBlueHead", "Theorem head", 0x153079),
    ("ntexVioletRule", "Lemma bar", 0x661eb8),
    ("ntexVioletBack", "Lemma fill", 0xece0fb),
    ("ntexVioletHead", "Lemma head", 0x441579),
    ("ntexTealRule", "Definition bar", 0x1c829c),
    ("ntexTealBack", "Definition fill", 0xe2f4f8),
    ("ntexTealHead", "Definition head", 0x0f596b),
    ("ntexMaroonRule", "Example bar", 0xa92d4a),
    ("ntexMaroonBack", "Example fill", 0xf8eaed),
    ("ntexMaroonHead", "Example head", 0x79152d),
    ("ntexGreenRule", "Remark bar", 0x238b45),
    ("ntexGreenBack", "Remark fill", 0xe5f5ea),
    ("ntexGreenHead", "Remark head", 0x145d2c),
    ("ntexOchreRule", "Exercise bar", 0xb07d12),
    ("ntexOchreBack", "Exercise fill", 0xfaf3e2),
    ("ntexOchreHead", "Exercise head", 0x5e420a),
    ("ntexProofRule", "Proof bar", 0xb8bccb),
    ("ntexProofBack", "Proof fill", 0xfbfaf7),
];

/// The typefaces (value, label, the LaTeX that loads it). Each was compiled
/// with tectonic; Charter is the theme's own (no file written).
pub const FONTS: &[(&str, &str, &str)] = &[
    ("charter", "Charter (XCharter + newtxmath)", ""),
    ("palatino", "Palatino (newpx)", "\\RequirePackage{newpxtext}\n\\RequirePackage[vvarbb]{newpxmath}\n"),
    ("times", "Times (newtx)", "\\RequirePackage{newtxtext}\n\\RequirePackage[vvarbb]{newtxmath}\n"),
    ("utopia", "Utopia (Erewhon)", "\\RequirePackage{erewhon}\n\\RequirePackage[utopia,vvarbb]{newtxmath}\n"),
    // unicode-math: her notes type ⟹, π … directly, which libertinust1math lacks
    ("libertinus", "Libertinus", "\\RequirePackage{unicode-math}\n\\setmainfont{LibertinusSerif-Regular.otf}[BoldFont=LibertinusSerif-Bold.otf, ItalicFont=LibertinusSerif-Italic.otf, BoldItalicFont=LibertinusSerif-BoldItalic.otf]\n\\setmathfont{LibertinusMath-Regular.otf}\n"),
    ("stix", "STIX Two", "\\RequirePackage{stix2}\n"),
    ("fourier", "Fourier (Utopia + Fourier math)", "\\RequirePackage{fourier}\n"),
];

const CHARTER: &str = "\\RequirePackage{XCharter}\n\\RequirePackage[charter,vvarbb]{newtxmath}\n\\DeclareSymbolFont{operators}{OT1}{XCharter-TLF}{m}{n}\n\\SetSymbolFont{operators}{bold}{OT1}{XCharter-TLF}{b}{n}\n";

/// The box style choices: (key, label, options as (value, label)).
pub const STYLE_CHOICES: &[(&str, &str, &[(&str, &str)])] = &[
    ("fill", "Box fill", &[("tinted", "Tinted"), ("none", "None")]),
    ("frame", "Frame", &[("bar", "Bar only"), ("box", "Bar + outline")]),
    ("corners", "Corners", &[("sharp", "Sharp"), ("rounded", "Rounded")]),
    ("heads", "Heads", &[("colored", "In the block's color"), ("ink", "In the text color")]),
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NotesLook {
    pub font: String,
    /// Line spacing, percent (\linespread 1.04 = 104).
    pub spacing: u32,
    /// Width of the bar on the left of theorem-like blocks, in tenths of a point (30 = 3pt).
    pub bar: u32,
    pub fill: String,
    pub frame: String,
    pub corners: String,
    pub heads: String,
    /// xcolor name → "#rrggbb" (only the ones changed from the theme's own).
    pub colors: BTreeMap<String, String>,
}

impl Default for NotesLook {
    fn default() -> NotesLook {
        NotesLook {
            font: "charter".into(),
            spacing: 104,
            bar: 30,
            fill: "tinted".into(),
            frame: "bar".into(),
            corners: "sharp".into(),
            heads: "colored".into(),
            colors: BTreeMap::new(),
        }
    }
}

pub fn base_color(name: &str) -> u32 {
    COLORS.iter().find(|(n, ..)| *n == name).map(|c| c.2).unwrap_or(0)
}

impl NotesLook {
    pub fn color(&self, name: &str) -> u32 {
        match self.colors.get(name).and_then(|c| crate::look::parse_hex(c)) {
            Some(c) => c,
            // the accent follows the theorem bar unless it was set itself (ntex-base.sty's \colorlet)
            None if name == "ntexBlue" && self.colors.contains_key("ntexBlueRule") => self.color("ntexBlueRule"),
            None => base_color(name),
        }
    }

    pub fn edited(&self, name: &str) -> bool {
        self.colors.contains_key(name)
    }

    pub fn set_color(&mut self, name: &str, c: u32) {
        if !COLORS.iter().any(|(n, ..)| *n == name) {
            return;
        }
        if c == base_color(name) {
            self.colors.remove(name);
        } else {
            self.colors.insert(name.into(), crate::look::hex(c));
        }
    }

    pub fn get(&self, key: &str) -> &str {
        match key {
            "font" => &self.font,
            "fill" => &self.fill,
            "frame" => &self.frame,
            "corners" => &self.corners,
            "heads" => &self.heads,
            _ => "",
        }
    }

    pub fn set(&mut self, key: &str, value: &str) -> bool {
        let ok = if key == "font" {
            FONTS.iter().any(|(v, ..)| *v == value)
        } else {
            STYLE_CHOICES.iter().find(|(k, ..)| *k == key).is_some_and(|(_, _, o)| o.iter().any(|(v, _)| *v == value))
        };
        if !ok {
            return false;
        }
        let slot = match key {
            "font" => &mut self.font,
            "fill" => &mut self.fill,
            "frame" => &mut self.frame,
            "corners" => &mut self.corners,
            _ => &mut self.heads,
        };
        *slot = value.into();
        true
    }

    pub fn index(&self, key: &str) -> usize {
        if key == "font" {
            return FONTS.iter().position(|(v, ..)| *v == self.font).unwrap_or(0);
        }
        STYLE_CHOICES.iter().find(|(k, ..)| *k == key).and_then(|(_, _, o)| o.iter().position(|(v, _)| *v == self.get(key))).unwrap_or(0)
    }

    fn cleaned(mut self) -> NotesLook {
        let d = NotesLook::default();
        for key in ["font", "fill", "frame", "corners", "heads"] {
            let v = self.get(key).to_string();
            if !self.set(key, &v) {
                self.set(key, d.get(key));
            }
        }
        self.spacing = self.spacing.clamp(90, 150);
        self.bar = self.bar.min(80);
        self.colors.retain(|n, c| COLORS.iter().any(|(k, ..)| k == n) && crate::look::parse_hex(c).is_some());
        for c in self.colors.values_mut() {
            *c = c.to_lowercase();
        }
        self
    }

    fn style_is_default(&self) -> bool {
        let d = NotesLook::default();
        (self.bar, &self.fill, &self.frame, &self.corners, &self.heads) == (d.bar, &d.fill, &d.frame, &d.corners, &d.heads)
    }

    /// ntex-notes-fonts.tex, or None for the theme's own Charter at its spacing.
    pub fn fonts_tex(&self) -> Option<String> {
        if self.font == "charter" && self.spacing == 104 {
            return None;
        }
        let load = FONTS.iter().find(|(v, ..)| *v == self.font).map(|f| f.2).filter(|s| !s.is_empty()).unwrap_or(CHARTER);
        Some(format!(
            "% Written by Ntex Studio (Notes page): the typeface for your notes.\n% Delete this file to go back to Charter.\n{load}\\linespread{{{}}}\n",
            fmt_spread(self.spacing)
        ))
    }

    /// ntex-notes-look.tex, or None for the theme's own colors and boxes.
    pub fn look_tex(&self) -> Option<String> {
        if self.colors.is_empty() && self.style_is_default() {
            return None;
        }
        let mut out = String::from("% Written by Ntex Studio (Notes page): block colors and box styling.\n% Delete this file to go back to the theme's own look (ntex-base.sty).\n");
        for (name, label, _) in COLORS {
            if let Some(c) = self.colors.get(*name) {
                out += &format!("\\definecolor{{{name}}}{{HTML}}{{{}}}  % {label}\n", c.trim_start_matches('#').to_uppercase());
            }
        }
        // ntex-base.sty copies the theorem bar into the accent; keep them together
        if self.colors.contains_key("ntexBlueRule") && !self.colors.contains_key("ntexBlue") {
            out += "\\colorlet{ntexBlue}{ntexBlueRule}\n";
        }
        if self.colors.contains_key("ntexInk") {
            out += "\\color{ntexInk}\n";
        }
        if !self.style_is_default() {
            let frame = if self.frame == "box" { "colframe=ntex#1Rule, boxrule=0.5pt" } else { "frame hidden" };
            let corners = if self.corners == "rounded" { "arc=4pt" } else { "sharp corners" };
            let fill = if self.fill == "none" { "white" } else { "ntex#1Back" };
            let head = if self.heads == "ink" { "ntexInk" } else { "ntex#1Head" };
            let bar = if self.bar == 0 { String::new() } else { format!("borderline west={{{}pt}}{{0pt}}{{ntex#1Rule}},\n    ", fmt_tenths(self.bar)) };
            out += &format!(
                "\\ifdefined\\tcbset\n\\tcbset{{\n  ntexbox/.style={{\n    enhanced, breakable, {frame}, {corners},\n    colback={fill}, coltext=ntexInk,\n    {bar}left=11pt, right=10pt, top=7pt, bottom=7pt,\n    before skip=12pt, after skip=12pt,\n    before upper={{\\colorlet{{ntexHead}}{{{head}}}}},\n  }},\n}}\n\\fi\n"
            );
        }
        Some(out)
    }
}

fn fmt_spread(pct: u32) -> String {
    format!("{}.{:02}", pct / 100, pct % 100)
}

fn fmt_tenths(t: u32) -> String {
    if t % 10 == 0 { format!("{}", t / 10) } else { format!("{}.{}", t / 10, t % 10) }
}

pub fn path(paths: &Paths) -> PathBuf {
    paths.config.join("notes-look.json")
}

pub fn read(paths: &Paths) -> NotesLook {
    std::fs::read_to_string(path(paths)).map(|t| parse(&t)).unwrap_or_default()
}

pub fn parse(text: &str) -> NotesLook {
    serde_json::from_str::<NotesLook>(text).map(NotesLook::cleaned).unwrap_or_default()
}

fn write_or_remove(p: PathBuf, text: Option<String>) -> std::io::Result<()> {
    match text {
        Some(t) => {
            let tmp = p.with_extension("tex.tmp");
            std::fs::write(&tmp, t)?;
            std::fs::rename(&tmp, &p)
        }
        None => match std::fs::remove_file(&p) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
    }
}

/// Save notes-look.json and (re)write or remove the two files the theme reads.
pub fn write(paths: &Paths, look: &NotesLook) -> std::io::Result<()> {
    let p = path(paths);
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(look).map_err(std::io::Error::other)? + "\n")?;
    std::fs::rename(&tmp, &p)?;
    write_or_remove(paths.templates_tex.join("ntex-notes-fonts.tex"), look.fonts_tex())?;
    write_or_remove(paths.templates_tex.join("ntex-notes-look.tex"), look.look_tex())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_write_nothing() {
        let d = NotesLook::default();
        assert!(d.fonts_tex().is_none() && d.look_tex().is_none());
    }

    #[test]
    fn colors_and_style() {
        let mut l = NotesLook::default();
        l.set_color("ntexBlueRule", 0x123456);
        let t = l.look_tex().unwrap();
        assert!(t.contains("\\definecolor{ntexBlueRule}{HTML}{123456}") && t.contains("\\colorlet{ntexBlue}{ntexBlueRule}"));
        assert!(!t.contains("tcbset"));
        l.set("corners", "rounded");
        l.bar = 15;
        let t = l.look_tex().unwrap();
        assert!(t.contains("arc=4pt") && t.contains("borderline west={1.5pt}"));
        l.set_color("ntexBlueRule", 0x1e47b8); // back to the theme's own: the edit goes
        assert!(l.colors.is_empty());
    }

    #[test]
    fn fonts() {
        let mut l = NotesLook::default();
        l.spacing = 110;
        assert!(l.fonts_tex().unwrap().contains("XCharter") && l.fonts_tex().unwrap().contains("\\linespread{1.10}"));
        assert!(l.set("font", "palatino") && l.fonts_tex().unwrap().contains("newpxtext"));
        assert!(!l.set("font", "comic"));
        assert_eq!(parse(r##"{"font":"comic","spacing":500,"colors":{"ntexInk":"#ABCDEF","nope":"#000000"}}"##).font, "charter");
    }
}

/// `NOTES_DUMP=dir cargo test -p ntex-studio-core --lib notes::dump -- --ignored`:
/// every typeface and a restyled look as files, for compiling real notes with them.
#[cfg(test)]
#[test]
#[ignore]
fn dump() {
    let dir = PathBuf::from(std::env::var("NOTES_DUMP").expect("NOTES_DUMP"));
    for (font, ..) in FONTS {
        let mut l = NotesLook::default();
        l.set("font", font);
        l.spacing = 108;
        std::fs::write(dir.join(format!("fonts-{font}.tex")), l.fonts_tex().unwrap()).unwrap();
    }
    let mut l = NotesLook::default();
    for (k, v) in [("fill", "none"), ("frame", "box"), ("corners", "rounded"), ("heads", "ink")] {
        l.set(k, v);
    }
    l.bar = 15;
    l.set_color("ntexBlueRule", 0xc41982);
    l.set_color("ntexInk", 0x003e6b);
    l.set_color("ntexProofBack", 0xf0f0f0);
    std::fs::write(dir.join("look-styled.tex"), l.look_tex().unwrap()).unwrap();
}
