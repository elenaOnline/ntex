//! Ghostty's look, for the Terminal page (⌘7): the options (catalog.rs), the
//! look as overrides of Ghostty's own defaults, the file Studio writes
//! (`~/.config/ghostty/ntex-look.ghostty`), themes, and the line edits in
//! cmux's own config. Pure: the processes (ghostty, cmux, nvim) are in
//! `ntex_studio_io::ghostty`.
//!
//! The file (`to_file`): a header, then `theme = ""` (unless the look is a
//! light/dark pair), then every look option whose value differs from
//! Ghostty's default, plus every look option her other config files set (so
//! the look wins over them whatever they say; the main config includes this
//! file last with `config-file = ?ntex-look.ghostty`). Colors are written
//! resolved, never as a theme name. Repeatable keys are written as
//! `key = ""` (clear) and then their values.
pub mod catalog;
pub mod screen;

use catalog::Kind;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const FILE_NAME: &str = "ntex-look.ghostty";
/// Marks Studio's block in cmux's own config.ghostty.
pub const CMUX_START: &str = "# ntex-studio terminal look start (Ntex Studio writes this block; edits are overwritten)";
pub const CMUX_END: &str = "# ntex-studio terminal look end";

/// Ghostty 1.3.1's `+show-config --default --docs`, for when Ghostty itself
/// can't be asked (tests, Ghostty missing).
pub const BUNDLED_DEFAULTS: &str = include_str!("data/ghostty-defaults.txt");

pub type Values = Vec<String>;

// ---------------------------------------------------------------------------
// config lines

/// `key = value` lines of a Ghostty config (comments and blanks skipped,
/// one pair of surrounding quotes taken off).
pub fn parse_lines(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let Some((k, v)) = l.split_once('=') else { continue };
        out.push((k.trim().to_string(), unquote(v.trim())));
    }
    out
}

fn unquote(v: &str) -> String {
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') { v[1..v.len() - 1].to_string() } else { v.to_string() }
}

/// A value as a config line writes it.
fn quote(v: &str) -> String {
    if v.is_empty() || v.starts_with(' ') || v.ends_with(' ') || v.starts_with('?') { format!("\"{v}\"") } else { v.to_string() }
}

/// Fold lines into a full value set: `palette = N=#…` becomes `palette:N`,
/// a repeatable key appends (an empty value clears it), others replace.
pub fn apply_lines(map: &mut BTreeMap<String, Values>, lines: &[(String, String)]) {
    for (k, v) in lines {
        if k == "config-file" {
            continue;
        }
        if k == "palette" {
            if let Some((i, c)) = v.split_once('=') {
                if let Ok(i) = i.trim().parse::<u8>() {
                    map.insert(format!("palette:{i}"), vec![c.trim().to_string()]);
                }
            }
            continue;
        }
        if catalog::is_repeatable(k) {
            // (a fallback family named twice is still one)
            let e = map.entry(k.clone()).or_default();
            if v.is_empty() {
                e.clear();
            } else if !e.contains(v) {
                e.push(v.clone());
            }
            continue;
        }
        map.insert(k.clone(), vec![v.clone()]);
    }
}

// ---------------------------------------------------------------------------
// defaults

/// Ghostty's defaults and docs (`+show-config --default --docs`).
#[derive(Clone, Debug, Default)]
pub struct Defaults {
    pub values: BTreeMap<String, Values>,
    pub docs: BTreeMap<String, String>,
    /// Every key, in Ghostty's order.
    pub keys: Vec<String>,
}

impl Defaults {
    pub fn parse(text: &str) -> Defaults {
        let mut values: BTreeMap<String, Values> = BTreeMap::new();
        let mut keys = Vec::new();
        for (k, v) in parse_lines(text) {
            if !keys.contains(&k) {
                keys.push(k.clone());
            }
            if k == "palette" {
                apply_lines(&mut values, &[(k, v)]);
            } else if catalog::is_repeatable(&k) {
                let e = values.entry(k).or_default();
                if !v.is_empty() {
                    e.push(v);
                }
            } else {
                values.insert(k, vec![v]);
            }
        }
        Defaults { values, docs: catalog::parse_docs(text), keys }
    }

    pub fn bundled() -> Defaults {
        Defaults::parse(BUNDLED_DEFAULTS)
    }

    pub fn get(&self, key: &str) -> Values {
        self.values.get(key).cloned().unwrap_or_default()
    }

    /// Keys this Ghostty has that the catalog doesn't know but that look
    /// like look options (shown under "Other").
    pub fn unknown_look_keys(&self) -> Vec<String> {
        self.keys.iter().filter(|k| catalog::find(k).is_none() && k.as_str() != "palette" && catalog::looks_like_look(k)).cloned().collect()
    }
}

// ---------------------------------------------------------------------------
// the look

/// A terminal look: the values that differ from Ghostty's defaults (palette
/// entries as `palette:N`). Saved as a JSON map.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TermLook {
    pub set: BTreeMap<String, Values>,
}

/// Is `key` a look option (catalog, palette, or new-and-look-like)?
pub fn is_look_key(key: &str) -> bool {
    key.starts_with("palette:") || catalog::find(key).is_some() || catalog::looks_like_look(key)
}

/// Numbers as Ghostty means them, written short: `10.000000000000009%` → `10%`.
pub fn norm_number(v: &str) -> String {
    let (num, pct) = match v.strip_suffix('%') {
        Some(n) => (n, "%"),
        None => (v, ""),
    };
    match num.trim().parse::<f64>() {
        Ok(x) if num.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+')) => {
            let r = (x * 10000.0).round() / 10000.0;
            let mut s = format!("{r}");
            if s == "-0" {
                s = "0".into();
            }
            format!("{s}{pct}")
        }
        _ => v.to_string(),
    }
}

fn normalize(key: &str, vals: Values) -> Values {
    match catalog::find(key).map(|o| o.kind) {
        Some(Kind::Number { .. } | Kind::Metric | Kind::Padding) => vals.into_iter().map(|v| v.split(',').map(|p| norm_number(p.trim())).collect::<Vec<_>>().join(",")).collect(),
        Some(Kind::Color { .. }) => vals.into_iter().map(|v| if v.starts_with('#') { v.to_lowercase() } else { v }).collect(),
        _ if key.starts_with("palette:") => vals.into_iter().map(|v| v.to_lowercase()).collect(),
        _ => vals,
    }
}

impl TermLook {
    /// The look of a full value set (the effective config): every look key
    /// that differs from the defaults. Style families equal to the main
    /// family are dropped (Ghostty fills them in itself).
    pub fn from_values(values: &BTreeMap<String, Values>, d: &Defaults) -> TermLook {
        let mut look = TermLook::default();
        for (k, v) in values {
            if !is_look_key(k) {
                continue;
            }
            let v = normalize(k, v.clone());
            if v != normalize(k, d.get(k)) {
                look.set.insert(k.clone(), v);
            }
        }
        if let Some(fam) = look.set.get("font-family").cloned() {
            for k in ["font-family-bold", "font-family-italic", "font-family-bold-italic"] {
                if look.set.get(k) == Some(&fam) {
                    look.set.remove(k);
                }
            }
        }
        look
    }

    /// The look of `+show-config` output (the effective, theme-resolved config).
    pub fn from_show_config(text: &str, d: &Defaults) -> TermLook {
        let mut values = d.values.clone();
        let lines = parse_lines(text);
        // its lists are whole: they replace the defaults' rather than add
        for (k, _) in &lines {
            if catalog::is_repeatable(k) {
                values.remove(k);
            }
        }
        apply_lines(&mut values, &lines);
        TermLook::from_values(&values, d)
    }

    /// The look of config files folded in order onto the defaults (when
    /// Ghostty can't be asked). A `theme` named there is not resolved.
    pub fn from_config_texts(texts: &[&str], d: &Defaults) -> TermLook {
        let mut values = d.values.clone();
        for t in texts {
            apply_lines(&mut values, &parse_lines(t));
        }
        TermLook::from_values(&values, d)
    }

    pub fn get(&self, key: &str, d: &Defaults) -> Values {
        self.set.get(key).cloned().unwrap_or_else(|| d.get(key))
    }

    /// The first value ("" if none).
    pub fn one(&self, key: &str, d: &Defaults) -> String {
        self.get(key, d).into_iter().next().unwrap_or_default()
    }

    /// Set a key; the default value removes the override.
    pub fn put(&mut self, key: &str, vals: Values, d: &Defaults) {
        let vals = normalize(key, vals);
        if vals == normalize(key, d.get(key)) {
            self.set.remove(key);
        } else {
            self.set.insert(key.to_string(), vals);
        }
    }

    pub fn put1(&mut self, key: &str, v: impl Into<String>, d: &Defaults) {
        self.put(key, vec![v.into()], d)
    }

    pub fn reset(&mut self, key: &str) {
        self.set.remove(key);
    }

    pub fn edited(&self, key: &str) -> bool {
        self.set.contains_key(key)
    }

    /// A color (`#rrggbb` only; unset or a special word: None).
    pub fn color(&self, key: &str, d: &Defaults) -> Option<u32> {
        parse_color(&self.one(key, d))
    }

    /// A number (unset or not a number: `fallback`).
    pub fn number(&self, key: &str, d: &Defaults, fallback: f32) -> f32 {
        self.one(key, d).trim_end_matches('%').parse().unwrap_or(fallback)
    }

    pub fn flag(&self, key: &str, d: &Defaults) -> bool {
        self.one(key, d) == "true"
    }

    /// A light/dark pair (`theme = light:X,dark:Y`).
    pub fn pair(&self) -> Option<(String, String)> {
        let t = self.set.get("theme")?.first()?.clone();
        let mut light = None;
        let mut dark = None;
        for part in t.split(',') {
            if let Some(x) = part.trim().strip_prefix("light:") {
                light = Some(x.trim().to_string());
            } else if let Some(x) = part.trim().strip_prefix("dark:") {
                dark = Some(x.trim().to_string());
            }
        }
        Some((light?, dark?))
    }

    /// Start from a theme: every color it sets replaces the look's; the
    /// colors it leaves alone go back to Ghostty's defaults.
    pub fn apply_theme(&mut self, theme: &Theme, d: &Defaults) {
        self.set.retain(|k, _| !is_theme_key(k) && k != "theme");
        for (k, v) in &theme.look.set {
            self.put(k, v.clone(), d);
        }
    }

    /// The file Studio writes (see the module docs). `pinned`: look keys her
    /// other config files set (`pinned_keys`).
    pub fn to_file(&self, title: &str, d: &Defaults, pinned: &BTreeSet<String>) -> String {
        let mut out = String::new();
        out.push_str(&format!("# Terminal look: {title}\n"));
        out.push_str("# Written by Ntex Studio (⌘7, the Terminal page) on every change; edits here are overwritten.\n");
        out.push_str("# ~/.config/ghostty/config loads it last (`config-file = ?ntex-look.ghostty`), so these lines win.\n");
        let pair = self.pair();
        match &pair {
            Some(_) => out.push_str(&format!("theme = {}\n", self.one("theme", d))),
            None => out.push_str("theme = \"\"\n"),
        }
        let mut keys: Vec<String> = catalog::LOOK.iter().map(|o| o.key.to_string()).filter(|k| k != "theme").collect();
        let pal_at = keys.iter().position(|k| k == "minimum-contrast").unwrap_or(keys.len());
        let pal: Vec<String> = (0..=255).map(|i| format!("palette:{i}")).collect();
        keys.splice(pal_at..pal_at, pal);
        for k in self.set.keys().chain(pinned.iter()) {
            if !keys.contains(k) && k != "theme" {
                keys.push(k.clone());
            }
        }
        for k in &keys {
            // a pair leaves the theme's colors to the theme
            if pair.is_some() && is_theme_key(k) && !self.set.contains_key(k) {
                continue;
            }
            if !self.set.contains_key(k) && !pinned.contains(k) {
                continue;
            }
            let vals = self.get(k, d);
            if let Some(i) = k.strip_prefix("palette:") {
                out.push_str(&format!("palette = {i}={}\n", vals.first().cloned().unwrap_or_default()));
            } else if catalog::is_repeatable(k) {
                out.push_str(&format!("{k} = \"\"\n"));
                for v in vals {
                    out.push_str(&format!("{k} = {}\n", quote(&v)));
                }
            } else {
                out.push_str(&format!("{k} = {}\n", quote(&vals.first().cloned().unwrap_or_default())));
            }
        }
        out
    }

    /// Read back a file Studio wrote (or any config text) as a look.
    pub fn from_file(text: &str, d: &Defaults) -> TermLook {
        let mut values = d.values.clone();
        let lines: Vec<(String, String)> = parse_lines(text).into_iter().filter(|(k, v)| !(k == "theme" && v.is_empty())).collect();
        apply_lines(&mut values, &lines);
        TermLook::from_values(&values, d)
    }
}

/// The keys a theme file sets (and starting from a theme resets).
pub fn is_theme_key(k: &str) -> bool {
    k.starts_with("palette:") || matches!(k, "background" | "foreground" | "cursor-color" | "cursor-text" | "selection-background" | "selection-foreground")
}

/// The look keys her other config files set (palette entries one by one):
/// the file must write these, whatever the look says, to win over them.
pub fn pinned_keys(texts: &[&str]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for t in texts {
        for (k, v) in parse_lines(t) {
            if k == "palette" {
                if let Some((i, _)) = v.split_once('=') {
                    if let Ok(i) = i.trim().parse::<u8>() {
                        out.insert(format!("palette:{i}"));
                    }
                }
            } else if k != "theme" && is_look_key(&k) {
                out.insert(k);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// colors

/// `#rrggbb` / `rrggbb` (an X11 name or a special word: None).
pub fn parse_color(s: &str) -> Option<u32> {
    let h = s.trim().trim_start_matches('#');
    if h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()) {
        return u32::from_str_radix(h, 16).ok();
    }
    if h.len() == 3 && h.chars().all(|c| c.is_ascii_hexdigit()) {
        let v = u32::from_str_radix(h, 16).ok()?;
        let (r, g, b) = ((v >> 8) & 0xf, (v >> 4) & 0xf, v & 0xf);
        return Some((r * 17) << 16 | (g * 17) << 8 | b * 17);
    }
    None
}

pub fn hex(c: u32) -> String {
    format!("#{:06x}", c & 0xffffff)
}

fn lin(c: u32, shift: u32) -> f64 {
    let v = ((c >> shift) & 0xff) as f64 / 255.0;
    if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

/// WCAG relative luminance.
pub fn luminance(c: u32) -> f64 {
    0.2126 * lin(c, 16) + 0.7152 * lin(c, 8) + 0.0722 * lin(c, 0)
}

/// WCAG contrast ratio (1–21).
pub fn contrast(a: u32, b: u32) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// `t` of `b` into `a`.
pub fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |x: u32, s: u32| ((x >> s) & 0xff) as f32;
    let f = |s: u32| ((ch(a, s) + (ch(b, s) - ch(a, s)) * t).round().clamp(0.0, 255.0) as u32) << s;
    f(16) | f(8) | f(0)
}

/// What `minimum-contrast` does to a foreground: below the ratio, Ghostty
/// draws it black or white, whichever reads better on `bg`.
pub fn with_min_contrast(fg: u32, bg: u32, min: f32) -> u32 {
    if min <= 1.0 || contrast(fg, bg) >= min as f64 {
        return fg;
    }
    if contrast(0x000000, bg) >= contrast(0xffffff, bg) { 0x000000 } else { 0xffffff }
}

pub fn is_dark(bg: u32) -> bool {
    luminance(bg) < 0.25
}

/// xterm's 256 colors (Ghostty's defaults for 16–255 when not generated).
pub fn xterm256(i: u8) -> u32 {
    const BASE: [u32; 16] = [
        0x1d1f21, 0xcc6666, 0xb5bd68, 0xf0c674, 0x81a2be, 0xb294bb, 0x8abeb7, 0xc5c8c6, 0x666666, 0xd54e53, 0xb9ca4a, 0xe7c547, 0x7aa6da,
        0xc397d8, 0x70c0b1, 0xeaeaea,
    ];
    match i {
        0..=15 => BASE[i as usize],
        16..=231 => {
            let n = i as u32 - 16;
            let step = |v: u32| if v == 0 { 0 } else { 55 + v * 40 };
            step(n / 36) << 16 | step((n / 6) % 6) << 8 | step(n % 6)
        }
        _ => {
            let v = 8 + (i as u32 - 232) * 10;
            v << 16 | v << 8 | v
        }
    }
}

/// Every color the preview needs, resolved the way Ghostty resolves them.
#[derive(Clone, Debug, PartialEq)]
pub struct Colors {
    pub bg: u32,
    pub fg: u32,
    pub palette: [u32; 256],
    /// None: cell-foreground / cell-background / unset, resolved per cell.
    pub cursor: CellColor,
    pub cursor_text: CellColor,
    pub sel_bg: CellColor,
    pub sel_fg: CellColor,
    /// None: the text's own color; Some(None): `bright`.
    pub bold: Option<Option<u32>>,
    pub min_contrast: f32,
    pub faint: f32,
    pub divider: u32,
    pub unfocused_fill: u32,
    pub unfocused_opacity: f32,
    pub opacity: f32,
    pub cursor_opacity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CellColor {
    Rgb(u32),
    CellFg,
    CellBg,
}

impl CellColor {
    pub fn resolve(self, cell_fg: u32, cell_bg: u32) -> u32 {
        match self {
            CellColor::Rgb(c) => c,
            CellColor::CellFg => cell_fg,
            CellColor::CellBg => cell_bg,
        }
    }
}

fn cell_color(v: &str, unset: CellColor) -> CellColor {
    match v.trim() {
        "cell-foreground" => CellColor::CellFg,
        "cell-background" => CellColor::CellBg,
        s => parse_color(s).map(CellColor::Rgb).unwrap_or(unset),
    }
}

impl Colors {
    pub fn of(l: &TermLook, d: &Defaults) -> Colors {
        let bg = l.color("background", d).unwrap_or(0x282c34);
        let fg = l.color("foreground", d).unwrap_or(0xffffff);
        let mut palette = [0u32; 256];
        for (i, p) in palette.iter_mut().enumerate() {
            *p = l.color(&format!("palette:{i}"), d).unwrap_or_else(|| xterm256(i as u8));
        }
        let bold = match l.one("bold-color", d).as_str() {
            "" => None,
            "bright" => Some(None),
            s => Some(parse_color(s)),
        };
        // the divider Ghostty draws when none is set: the background, a step darker (lighter on dark)
        let divider = l.color("split-divider-color", d).unwrap_or_else(|| if is_dark(bg) { mix(bg, 0xffffff, 0.2) } else { mix(bg, 0x000000, 0.16) });
        Colors {
            bg,
            fg,
            palette,
            cursor: cell_color(&l.one("cursor-color", d), CellColor::Rgb(fg)),
            cursor_text: cell_color(&l.one("cursor-text", d), CellColor::Rgb(bg)),
            sel_bg: cell_color(&l.one("selection-background", d), CellColor::CellFg),
            sel_fg: cell_color(&l.one("selection-foreground", d), CellColor::CellBg),
            bold,
            min_contrast: l.number("minimum-contrast", d, 1.0),
            faint: l.number("faint-opacity", d, 0.5),
            divider,
            unfocused_fill: l.color("unfocused-split-fill", d).unwrap_or(bg),
            unfocused_opacity: l.number("unfocused-split-opacity", d, 0.7).clamp(0.15, 1.0),
            opacity: l.number("background-opacity", d, 1.0).clamp(0.0, 1.0),
            cursor_opacity: l.number("cursor-opacity", d, 1.0).clamp(0.0, 1.0),
        }
    }
}

// ---------------------------------------------------------------------------
// themes

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub name: String,
    /// The theme's colors as a look (over Ghostty's defaults).
    pub look: TermLook,
    pub bg: u32,
    pub fg: u32,
    pub ansi: [u32; 16],
    pub dark: bool,
}

/// A theme file (Ghostty config syntax: palette, background, …).
pub fn parse_theme(name: &str, text: &str, d: &Defaults) -> Theme {
    let mut look = TermLook::default();
    let mut values = BTreeMap::new();
    apply_lines(&mut values, &parse_lines(text));
    for (k, v) in values {
        if is_theme_key(&k) || is_look_key(&k) {
            look.put(&k, v, d);
        }
    }
    let c = Colors::of(&look, d);
    let mut ansi = [0u32; 16];
    ansi.copy_from_slice(&c.palette[..16]);
    Theme { name: name.to_string(), bg: c.bg, fg: c.fg, ansi, dark: is_dark(c.bg), look }
}

// ---------------------------------------------------------------------------
// the include line and cmux's own config

/// Does the main config load the look file?
pub fn has_include(main: &str) -> bool {
    parse_lines(main).iter().any(|(k, v)| k == "config-file" && v.trim_start_matches('?').trim_matches('"').ends_with(FILE_NAME))
}

/// The main config with the include appended (unchanged if it has it).
pub fn add_include(main: &str, date: &str) -> String {
    if has_include(main) {
        return main.to_string();
    }
    let mut s = main.to_string();
    if !s.ends_with('\n') && !s.is_empty() {
        s.push('\n');
    }
    s.push_str(&format!(
        "\n# Terminal look from Ntex Studio (⌘7, the Terminal page). Loaded after this\n# whole file, so its lines win over the ones above. Added {date}; delete this\n# line to go back to the look above.\nconfig-file = ?{FILE_NAME}\n"
    ));
    s
}

/// The `background` / `foreground` lines outside Studio's block get these
/// values (only lines that are there; nothing is added).
pub fn sync_cmux_colors(text: &str, bg: &str, fg: &str) -> String {
    let mut inside = false;
    let mut out: Vec<String> = Vec::new();
    for line in text.split_inclusive('\n') {
        let t = line.trim();
        if t == CMUX_START {
            inside = true;
        }
        let mut l = line.to_string();
        if !inside && !t.starts_with('#') {
            if let Some((k, _)) = t.split_once('=') {
                let nl = if line.ends_with('\n') { "\n" } else { "" };
                match k.trim() {
                    "background" => l = format!("background = {bg}{nl}"),
                    "foreground" => l = format!("foreground = {fg}{nl}"),
                    _ => {}
                }
            }
        }
        if t == CMUX_END {
            inside = false;
        }
        out.push(l);
    }
    out.concat()
}

/// Studio's block in cmux's config (None: not there).
pub fn cmux_block(text: &str) -> Option<String> {
    let s = text.find(CMUX_START)?;
    let e = text[s..].find(CMUX_END)? + s;
    let body = &text[s + CMUX_START.len()..e];
    Some(body.trim_start_matches('\n').to_string())
}

/// Put Studio's block at the end of cmux's config (None: take it out).
pub fn set_cmux_block(text: &str, block: Option<&str>) -> String {
    let mut base = text.to_string();
    if let Some(s) = base.find(CMUX_START) {
        if let Some(e) = base[s..].find(CMUX_END) {
            let mut end = s + e + CMUX_END.len();
            if base[end..].starts_with('\n') {
                end += 1;
            }
            base.replace_range(s..end, "");
            while base.ends_with("\n\n") {
                base.pop();
            }
        }
    }
    match block {
        None => base,
        Some(b) => {
            if !base.is_empty() && !base.ends_with('\n') {
                base.push('\n');
            }
            if !base.is_empty() {
                base.push('\n');
            }
            let mut b = b.to_string();
            if !b.ends_with('\n') {
                b.push('\n');
            }
            format!("{base}{CMUX_START}\n{b}{CMUX_END}\n")
        }
    }
}

// ---------------------------------------------------------------------------
// the page's own state (terminal-look.json)

/// Where a change goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppliesTo {
    #[default]
    Both,
    GhosttyOnly,
    CmuxOnly,
}

/// `terminal-look.json` in the config folder: the look being edited, where
/// it goes, and the look the other app keeps while it applies to one only.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TermState {
    pub look: TermLook,
    pub applies: AppliesTo,
    /// Ghostty only: cmux's look; cmux only: Ghostty's.
    pub other: Option<TermLook>,
    /// The saved look it started from (for the file's header).
    pub name: Option<String>,
}

#[cfg(test)]
mod tests;
