//! The notes look: how the PDF of a note looks (ntex-notes.sty, ntex-base.sty).
//! Studio's Notes page keeps it in `notes-look.json` and writes two files the
//! theme reads (templates/tex, next to ntex-base.sty):
//!
//!   ntex-notes-fonts.tex   the typeface, instead of Charter (ntex-base.sty)
//!   ntex-notes-look.tex    block colors and box styling, at \begin{document}
//!
//! Each file exists only while it differs from the theme's own look, so with
//! the defaults the PDF is exactly the theme's.
//!
//! Boxes: one shared shape (`ntexbox`) that every kind inherits, and each kind
//! (theorem, lemma, …, proof, exercise) may set its own bar, fill, frame,
//! corners, head colour and body font on top (`kinds`). Proof and exercise are
//! ordinary kinds whose own settings are the theme's look for them.
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

/// Body font, per kind: (value, label).
pub const BODY_CHOICES: &[(&str, &str)] = &[("italic", "Italic"), ("upright", "Upright")];

/// The proof's end mark: (value, label).
pub const QED_CHOICES: &[(&str, &str)] = &[("open", "□ Open square"), ("filled", "■ Filled square"), ("tombstone", "∎ Tombstone"), ("none", "None")];

/// A kind of box the theme draws, through its own style `ntex <env>` (ntex-notes.sty).
pub struct Kind {
    pub env: &'static str,
    pub label: &'static str,
    /// Its colours: ntex<family>Rule / Back / Head.
    pub family: &'static str,
    /// The theme's body font for it.
    pub italic: bool,
    /// Its head takes ntexHead (proof's "Proof." is amsthm's own, in ink).
    pub head: bool,
    /// An amsthm theorem: its body font goes through \ntexbody, not the box.
    pub theorem: bool,
    /// The kind's own spacing, which no setting changes.
    pub breakable: bool,
    pub pad: &'static str,
}

const PAD: &str = "left=11pt, right=10pt, top=7pt, bottom=7pt,\n    before skip=12pt, after skip=12pt";

const fn thm(env: &'static str, label: &'static str, family: &'static str, italic: bool) -> Kind {
    Kind { env, label, family, italic, head: true, theorem: true, breakable: true, pad: PAD }
}

/// Every kind, in the order a page shows them.
pub const KINDS: &[Kind] = &[
    thm("theorem", "Theorem", "Blue", true),
    thm("lemma", "Lemma", "Violet", true),
    thm("proposition", "Proposition", "Violet", true),
    thm("corollary", "Corollary", "Violet", true),
    thm("definition", "Definition", "Teal", false),
    thm("example", "Example", "Maroon", false),
    thm("remark", "Remark", "Green", false),
    Kind {
        env: "proof", label: "Proof", family: "Proof", italic: false, head: false, theorem: false, breakable: true,
        pad: "left=10pt, right=10pt, top=6pt, bottom=6pt,\n    before skip=8pt, after skip=12pt",
    },
    Kind {
        env: "exercise", label: "Exercise", family: "Ochre", italic: false, head: true, theorem: false, breakable: false,
        pad: "left=11pt, right=10pt, top=7pt, bottom=7pt,\n    before skip=18pt, after skip=8pt",
    },
];

pub fn kind(env: &str) -> Option<&'static Kind> {
    KINDS.iter().find(|k| k.env == env)
}

/// One kind's own settings; what it leaves unset comes from the shared shape.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KindLook {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bar: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corners: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heads: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

impl KindLook {
    fn slot(&mut self, key: &str) -> Option<&mut Option<String>> {
        match key {
            "fill" => Some(&mut self.fill),
            "frame" => Some(&mut self.frame),
            "corners" => Some(&mut self.corners),
            "heads" => Some(&mut self.heads),
            "body" => Some(&mut self.body),
            _ => None,
        }
    }

    fn get(&self, key: &str) -> Option<String> {
        match key {
            "bar" => self.bar.map(|b| b.to_string()),
            "fill" => self.fill.clone(),
            "frame" => self.frame.clone(),
            "corners" => self.corners.clone(),
            "heads" => self.heads.clone(),
            "body" => self.body.clone(),
            _ => None,
        }
    }

    fn clear(&mut self, key: &str) {
        if key == "bar" {
            self.bar = None;
        } else if let Some(s) = self.slot(key) {
            *s = None;
        }
    }
}

/// The theme's own settings for proof and exercise (ntex-notes.sty): they don't
/// follow the shared shape until the look says so.
pub fn default_kinds() -> BTreeMap<String, KindLook> {
    let own = |bar: u32, heads: Option<&str>| KindLook {
        bar: Some(bar),
        fill: Some("tinted".into()),
        frame: Some("bar".into()),
        corners: Some("sharp".into()),
        heads: heads.map(Into::into),
        body: None,
    };
    BTreeMap::from([("proof".into(), own(10, None)), ("exercise".into(), own(30, Some("colored")))])
}

/// The keys a kind can set for itself, beyond the colours.
pub const KIND_KEYS: &[&str] = &["bar", "fill", "frame", "corners", "heads", "body"];

/// Whether `key` means anything for this kind (proof's head isn't coloured).
pub fn applies(env: &str, key: &str) -> bool {
    kind(env).is_some_and(|k| KIND_KEYS.contains(&key) && (key != "heads" || k.head))
}

fn valid(key: &str, value: &str) -> bool {
    match key {
        "bar" => value.parse::<u32>().is_ok_and(|b| b <= 80),
        "body" => BODY_CHOICES.iter().any(|(v, _)| *v == value),
        "qed" => QED_CHOICES.iter().any(|(v, _)| *v == value),
        _ => STYLE_CHOICES.iter().find(|(k, ..)| *k == key).is_some_and(|(_, _, o)| o.iter().any(|(v, _)| *v == value)),
    }
}

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
    /// env → that kind's own box settings, over the shared ones above.
    pub kinds: BTreeMap<String, KindLook>,
    /// The proof's end mark (QED_CHOICES).
    pub qed: String,
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
            kinds: default_kinds(),
            qed: "open".into(),
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
            "qed" => &self.qed,
            _ => "",
        }
    }

    /// What a kind draws with: its own setting, else the shared one (the
    /// theme's body font for `body`). Bar in tenths of a point, as text.
    pub fn kind_get(&self, env: &str, key: &str) -> String {
        if let Some(v) = self.kinds.get(env).and_then(|o| o.get(key)) {
            return v;
        }
        match key {
            "bar" => self.bar.to_string(),
            "body" => (if kind(env).is_some_and(|k| k.italic) { "italic" } else { "upright" }).into(),
            _ => self.get(key).into(),
        }
    }

    /// Whether the kind has its own `key` (rather than the shared one).
    pub fn kind_has_own(&self, env: &str, key: &str) -> bool {
        self.kinds.get(env).is_some_and(|o| o.get(key).is_some())
    }

    /// Give one kind its own setting ("Proof only").
    pub fn kind_set(&mut self, env: &str, key: &str, value: &str) -> bool {
        if !applies(env, key) || !valid(key, value) {
            return false;
        }
        let own = self.kinds.entry(env.into()).or_default();
        if key == "bar" {
            own.bar = value.parse().ok();
        } else if let Some(s) = own.slot(key) {
            *s = Some(value.into());
        }
        true
    }

    /// Drop one kind's own setting, so it follows the shared one again.
    pub fn kind_clear(&mut self, env: &str, key: &str) {
        if let Some(own) = self.kinds.get_mut(env) {
            own.clear(key);
        }
    }

    /// Set `key` for every box ("Every box"): the shared setting, and no kind
    /// keeps its own. Body has no shared setting, so every kind gets it.
    pub fn set_every(&mut self, key: &str, value: &str) -> bool {
        if !KIND_KEYS.contains(&key) || !valid(key, value) {
            return false;
        }
        if key == "body" {
            for k in KINDS {
                self.kind_set(k.env, key, value);
            }
            return true;
        }
        if key == "bar" {
            self.bar = value.parse().unwrap_or(self.bar);
        } else {
            self.set(key, value);
        }
        for own in self.kinds.values_mut() {
            own.clear(key);
        }
        true
    }

    /// The kinds whose boxes `set_every(key, value)` would change, in page order.
    pub fn every_changes(&self, key: &str, value: &str) -> Vec<&'static str> {
        let mut after = self.clone();
        if !after.set_every(key, value) {
            return Vec::new();
        }
        KINDS.iter().filter(|k| applies(k.env, key) && after.kind_get(k.env, key) != self.kind_get(k.env, key)).map(|k| k.env).collect()
    }

    pub fn set(&mut self, key: &str, value: &str) -> bool {
        if key == "qed" {
            if valid(key, value) {
                self.qed = value.into();
            }
            return valid(key, value);
        }
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
        if !valid("qed", &self.qed) {
            self.qed = d.qed;
        }
        // a kind present with nothing valid stays, empty: it follows the shared shape
        let kinds = std::mem::take(&mut self.kinds);
        for (env, own) in kinds.into_iter().filter(|(e, _)| kind(e).is_some()) {
            let mut clean = KindLook::default();
            for key in KIND_KEYS {
                if let Some(v) = own.get(key).filter(|v| applies(&env, key) && valid(key, v)) {
                    if *key == "bar" {
                        clean.bar = v.parse().ok();
                    } else if let Some(s) = clean.slot(key) {
                        *s = Some(v);
                    }
                }
            }
            self.kinds.insert(env, clean);
        }
        self
    }

    fn shared_is_default(&self) -> bool {
        let d = NotesLook::default();
        (self.bar, &self.fill, &self.frame, &self.corners, &self.heads) == (d.bar, &d.fill, &d.frame, &d.corners, &d.heads)
    }

    fn style_is_default(&self) -> bool {
        self.shared_is_default() && self.kinds_differing().is_empty() && self.qed == "open"
    }

    /// The kinds whose own settings aren't the theme's own for them.
    fn kinds_differing(&self) -> Vec<&'static Kind> {
        let d = default_kinds();
        let none = KindLook::default();
        KINDS.iter().filter(|k| self.kinds.get(k.env).unwrap_or(&none) != d.get(k.env).unwrap_or(&none)).collect()
    }

    /// The kinds whose box needs its own style: an amsthm kind's body font goes
    /// through \ntexbody instead.
    fn kinds_restyled(&self) -> Vec<&'static Kind> {
        let d = default_kinds();
        let boxed = |o: Option<&KindLook>, k: &Kind| {
            let mut o = o.cloned().unwrap_or_default();
            if k.theorem {
                o.body = None;
            }
            o
        };
        KINDS.iter().filter(|k| boxed(self.kinds.get(k.env), k) != boxed(d.get(k.env), k)).collect()
    }

    /// The tcolorbox options for one box: `family` is a colour family, or `#1` in ntexbox.
    fn box_options(&self, family: &str, env: Option<&Kind>) -> String {
        let get = |key: &str| match env {
            Some(k) => self.kind_get(k.env, key),
            None if key == "bar" => self.bar.to_string(),
            None => self.get(key).to_string(),
        };
        let frame = if get("frame") == "box" { format!("colframe=ntex{family}Rule, boxrule=0.5pt") } else { "frame hidden".into() };
        let corners = if get("corners") == "rounded" { "arc=4pt" } else { "sharp corners" };
        let fill = if get("fill") == "none" { "white".into() } else { format!("ntex{family}Back") };
        let bar: u32 = get("bar").parse().unwrap_or(0);
        let bar = if bar == 0 { String::new() } else { format!("borderline west={{{}pt}}{{0pt}}{{ntex{family}Rule}},\n    ", fmt_tenths(bar)) };
        let breakable = if env.is_none_or(|k| k.breakable) { "breakable, " } else { "" };
        let pad = env.map_or(PAD, |k| k.pad);
        let mut out = format!("enhanced, {breakable}{frame}, {corners},\n    colback={fill}, coltext=ntexInk,\n    {bar}{pad},\n");
        if env.is_none_or(|k| k.head) {
            let head = if get("heads") == "ink" { "ntexInk".into() } else { format!("ntex{family}Head") };
            out += &format!("    before upper={{\\colorlet{{ntexHead}}{{{head}}}}},\n");
        }
        if env.is_some_and(|k| !k.theorem) && get("body") == "italic" {
            out += "    fontupper=\\itshape,\n";
        }
        out
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
        let kinds = self.kinds_restyled();
        if !self.shared_is_default() || !kinds.is_empty() {
            out += "\\ifdefined\\tcbset\n\\tcbset{\n";
            if !self.shared_is_default() {
                out += &format!("  ntexbox/.style={{\n    {}  }},\n", self.box_options("#1", None));
            }
            for k in &kinds {
                out += &format!("  ntex {}/.style={{\n    {}  }},\n", k.env, self.box_options(k.family, Some(k)));
            }
            out += "}\n\\fi\n";
        }
        // an amsthm kind's body font: \ntexbody in ntex-notes.sty reads this
        for k in KINDS.iter().filter(|k| k.theorem && (self.kind_get(k.env, "body") == "italic") != k.italic) {
            let font = if k.italic { "\\upshape" } else { "\\itshape" };
            out += &format!("\\expandafter\\def\\csname ntexbody@{}\\endcsname{{{font}}}\n", k.env);
        }
        let qed = match self.qed.as_str() {
            "filled" => "\\textcolor{ntexProofRule!80!black}{$\\blacksquare$}",
            "tombstone" => "\\textcolor{ntexProofRule!80!black}{\\rule{0.5em}{0.75em}}",
            "none" => "",
            _ => "\\textcolor{ntexProofRule!80!black}{$\\square$}",
        };
        if self.qed != "open" {
            out += &format!("\\ifdefined\\qedsymbol\\renewcommand{{\\qedsymbol}}{{{qed}}}\\fi\n");
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

    #[test]
    fn older_files_keep_the_theme_proof_and_exercise() {
        // notes-look.json from before kinds: proof and exercise keep their own look
        let l = parse(r##"{"bar":15,"fill":"none"}"##);
        assert_eq!(l.kinds, default_kinds());
        assert_eq!((l.kind_get("theorem", "bar").as_str(), l.kind_get("proof", "bar").as_str()), ("15", "10"));
        assert_eq!(l.kind_get("exercise", "fill"), "tinted");
        let t = l.look_tex().unwrap();
        assert!(t.contains("ntexbox/.style") && !t.contains("ntex proof/.style") && !t.contains("ntex exercise/.style"));
    }

    #[test]
    fn one_kind_its_own() {
        let mut l = NotesLook::default();
        assert!(l.kind_set("proof", "bar", "20") && l.kind_set("proof", "corners", "rounded"));
        assert!(!l.kind_set("proof", "heads", "ink"), "proof's head isn't coloured");
        assert!(!l.kind_set("proof", "bar", "81") && !l.kind_set("nope", "bar", "10") && !l.kind_set("lemma", "fill", "plaid"));
        let t = l.look_tex().unwrap();
        assert!(!t.contains("ntexbox/.style"), "the shared shape is untouched");
        assert!(t.contains("ntex proof/.style") && t.contains("borderline west={2pt}{0pt}{ntexProofRule}") && t.contains("arc=4pt"));
        assert!(t.contains("left=10pt, right=10pt, top=6pt, bottom=6pt") && !t.contains("ntexProofHead"));
        l.kind_set("proof", "bar", "10");
        l.kind_set("proof", "corners", "sharp");
        assert!(l.look_tex().is_none(), "back to the theme's own proof: nothing written");
        l.kind_set("lemma", "frame", "box");
        let t = l.look_tex().unwrap();
        assert!(t.contains("ntex lemma/.style") && t.contains("colframe=ntexVioletRule") && t.contains("before upper={\\colorlet{ntexHead}{ntexVioletHead}}"));
        assert!(!t.contains("ntex theorem/.style"));
    }

    #[test]
    fn every_box_links() {
        let mut l = NotesLook::default();
        assert!(l.every_changes("frame", "bar").is_empty(), "every box already has a bar only");
        assert_eq!(l.every_changes("fill", "none").len(), KINDS.len());
        l.kind_set("theorem", "bar", "50");
        assert_eq!(l.every_changes("bar", "30"), vec!["theorem", "proof"]);
        assert!(l.set_every("bar", "30"));
        assert!(KINDS.iter().all(|k| l.kind_get(k.env, "bar") == "30" && !l.kind_has_own(k.env, "bar")));
        // the proof now follows the shared shape: its box is written from it, with its own spacing
        let t = l.look_tex().unwrap();
        assert!(t.contains("ntex proof/.style") && t.contains("borderline west={3pt}{0pt}{ntexProofRule}") && t.contains("before skip=8pt"));
        // and a later shared change reaches it
        l.set_every("corners", "rounded");
        assert_eq!(l.kind_get("proof", "corners"), "rounded");
        let back = parse(&serde_json::to_string(&l).unwrap());
        assert_eq!(back, l, "a proof with no own settings survives a round trip");
        assert!(!l.set_every("qed", "filled") && !l.set_every("bar", "x"));
    }

    #[test]
    fn body_and_end_mark() {
        let mut l = NotesLook::default();
        assert_eq!((l.kind_get("theorem", "body").as_str(), l.kind_get("definition", "body").as_str()), ("italic", "upright"));
        l.kind_set("definition", "body", "italic");
        l.kind_set("theorem", "body", "upright");
        l.kind_set("proof", "body", "italic");
        let t = l.look_tex().unwrap();
        assert!(t.contains("\\csname ntexbody@definition\\endcsname{\\itshape}") && t.contains("\\csname ntexbody@theorem\\endcsname{\\upshape}"));
        assert!(t.contains("fontupper=\\itshape") && !t.contains("ntexbody@proof"));
        assert!(!t.contains("ntex theorem/.style") && !t.contains("ntex definition/.style"), "a body font alone writes no box");
        let mut l = NotesLook::default();
        assert!(l.set("qed", "filled") && !l.set("qed", "heart"));
        assert!(l.look_tex().unwrap().contains("\\renewcommand{\\qedsymbol}{\\textcolor{ntexProofRule!80!black}{$\\blacksquare$}}"));
        l.set_every("body", "upright");
        assert_eq!(l.kind_get("lemma", "body"), "upright");
    }

    #[test]
    fn cleaned_kinds() {
        let l = parse(r##"{"qed":"heart","kinds":{"proof":{"bar":900,"heads":"ink","fill":"none"},"nope":{"bar":10},"lemma":{"body":"bold"}}}"##);
        assert_eq!(l.qed, "open");
        assert_eq!(l.kinds.get("proof"), Some(&KindLook { fill: Some("none".into()), ..Default::default() }));
        assert_eq!(l.kinds.get("lemma"), Some(&KindLook::default()));
        assert!(!l.kinds.contains_key("nope") && !l.kinds.contains_key("exercise"));
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
    // one kind restyled, the rest the theme's
    let mut l = NotesLook::default();
    l.kind_set("proof", "bar", "0");
    l.kind_set("proof", "frame", "box");
    l.kind_set("proof", "corners", "rounded");
    l.kind_set("definition", "body", "italic");
    l.kind_set("theorem", "body", "upright");
    l.kind_set("lemma", "heads", "ink");
    l.set("qed", "filled");
    std::fs::write(dir.join("look-kinds.tex"), l.look_tex().unwrap()).unwrap();
    // every box linked: proof and exercise follow the shared shape
    let mut l = NotesLook::default();
    l.set_every("fill", "none");
    l.set_every("frame", "box");
    l.set_every("bar", "15");
    l.set("qed", "tombstone");
    std::fs::write(dir.join("look-every.tex"), l.look_tex().unwrap()).unwrap();
}
