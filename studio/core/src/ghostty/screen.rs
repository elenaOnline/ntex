//! The preview's two panes as cells that know their role: a cell's colors are
//! "the terminal's foreground", "palette 4", or a fixed color a program chose
//! (ntex draws its page in exact colors). Recoloring a role recolors every
//! cell that uses it; clicking a cell selects the option its role is.
use super::{CellColor, Colors, with_min_contrast};

pub const BOLD: u8 = 1;
pub const ITALIC: u8 = 2;
pub const UNDERLINE: u8 = 4;
pub const REVERSE: u8 = 8;
pub const STRIKE: u8 = 16;
pub const FAINT: u8 = 32;
/// Inside the (Ghostty) selection.
pub const SELECTED: u8 = 64;

/// Where a cell's color comes from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Paint {
    /// The terminal's foreground (for fg) or background (for bg).
    Default,
    Pal(u8),
    /// A program's own exact color.
    Rgb(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub ch: String,
    pub fg: Paint,
    pub bg: Paint,
    pub flags: u8,
}

impl Cell {
    pub fn blank() -> Cell {
        Cell { ch: " ".into(), fg: Paint::Default, bg: Paint::Default, flags: 0 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Screen {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<Vec<Cell>>,
    pub cursor: Option<(usize, usize)>,
}

/// What a click on the preview selects: a look option (or palette entry) by
/// key, or an exact color a program drew.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Key(String),
    Fixed(u32),
}

impl Target {
    pub fn key(k: &str) -> Target {
        Target::Key(k.to_string())
    }
}

/// One cell as Ghostty would draw it.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawn {
    pub fg: u32,
    pub bg: u32,
    /// Where the fg / bg came from (what clicking selects).
    pub fg_from: Target,
    pub bg_from: Target,
    /// The bg is the terminal background (drawn by the pane, not the cell).
    pub plain_bg: bool,
}

fn paint(p: Paint, c: &Colors, fg: bool, bold: bool) -> (u32, Target) {
    match p {
        Paint::Default if fg => match c.bold {
            Some(Some(b)) if bold => (b, Target::key("bold-color")),
            _ => (c.fg, Target::key("foreground")),
        },
        Paint::Default => (c.bg, Target::key("background")),
        Paint::Pal(n) => {
            let n = if fg && bold && n < 8 && c.bold == Some(None) { n + 8 } else { n };
            (c.palette[n as usize], Target::Key(format!("palette:{n}")))
        }
        Paint::Rgb(x) => (x, Target::Fixed(x)),
    }
}

/// Resolve one cell. `cursor`: this cell has the (block) cursor of the focused pane.
pub fn draw(cell: &Cell, c: &Colors, cursor: bool) -> Drawn {
    let bold = cell.flags & BOLD != 0;
    let (mut fg, mut fg_from) = paint(cell.fg, c, true, bold);
    let (mut bg, mut bg_from) = paint(cell.bg, c, false, false);
    let mut plain_bg = cell.bg == Paint::Default;
    if cell.flags & REVERSE != 0 {
        std::mem::swap(&mut fg, &mut bg);
        std::mem::swap(&mut fg_from, &mut bg_from);
        plain_bg = false;
    }
    if cell.flags & SELECTED != 0 {
        let (f, b) = (fg, bg);
        fg = c.sel_fg.resolve(f, b);
        bg = c.sel_bg.resolve(f, b);
        fg_from = Target::key("selection-foreground");
        bg_from = Target::key("selection-background");
        plain_bg = false;
    }
    if cursor {
        let (f, b) = (fg, bg);
        bg = c.cursor.resolve(f, b);
        fg = c.cursor_text.resolve(f, b);
        fg_from = Target::key("cursor-text");
        bg_from = Target::key("cursor-color");
        plain_bg = false;
    } else {
        fg = with_min_contrast(fg, bg, c.min_contrast);
    }
    Drawn { fg, bg, fg_from, bg_from, plain_bg }
}

/// The color the cursor is drawn in at this cell (any shape).
pub fn cursor_color(cell: &Cell, c: &Colors) -> u32 {
    let d = draw(cell, c, false);
    match c.cursor {
        CellColor::Rgb(x) => x,
        other => other.resolve(d.fg, d.bg),
    }
}

// ---------------------------------------------------------------------------
// ntex, captured (studio/bridge/terminal-capture.lua)

/// The capture's JSON: `{cols, rows, cursor: [r, c], cells: [[[text, fg, bg, flags]]]}`,
/// fg/bg `-1` for the terminal's own.
pub fn from_capture(json: &str) -> Result<Screen, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let cols = v["cols"].as_u64().ok_or("no cols")? as usize;
    let rows = v["rows"].as_u64().ok_or("no rows")? as usize;
    let mut cells = Vec::new();
    for row in v["cells"].as_array().ok_or("no cells")? {
        let mut line = Vec::new();
        for cell in row.as_array().ok_or("bad row")? {
            let a = cell.as_array().ok_or("bad cell")?;
            let ch = a.first().and_then(|x| x.as_str()).unwrap_or(" ");
            let col = |i: usize| match a.get(i).and_then(|x| x.as_i64()) {
                Some(x) if x >= 0 => Paint::Rgb(x as u32),
                _ => Paint::Default,
            };
            let flags = a.get(3).and_then(|x| x.as_u64()).unwrap_or(0) as u8;
            line.push(Cell { ch: if ch.is_empty() { String::new() } else { ch.to_string() }, fg: col(1), bg: col(2), flags });
        }
        line.resize(cols, Cell::blank());
        cells.push(line);
    }
    cells.resize(rows, vec![Cell::blank(); cols]);
    let cursor = v["cursor"].as_array().and_then(|a| Some((a.first()?.as_u64()? as usize, a.get(1)?.as_u64()? as usize)));
    Ok(Screen { cols, rows, cells, cursor: cursor.filter(|(r, c)| *r < rows && *c < cols) })
}

/// The capture's JSON for a screen (as `from_capture` reads it).
pub fn to_capture(s: &Screen) -> String {
    let col = |p: Paint| match p {
        Paint::Rgb(x) => x as i64,
        _ => -1,
    };
    let cells: Vec<Vec<serde_json::Value>> =
        s.cells.iter().map(|r| r.iter().map(|c| serde_json::json!([c.ch, col(c.fg), col(c.bg), c.flags])).collect()).collect();
    let cursor = s.cursor.map(|(r, c)| serde_json::json!([r, c])).unwrap_or(serde_json::Value::Null);
    serde_json::json!({ "cols": s.cols, "rows": s.rows, "cursor": cursor, "cells": cells }).to_string()
}

// ---------------------------------------------------------------------------
// the shell pane: a prompt and output that use all 16 colors

struct Builder {
    cols: usize,
    rows: Vec<Vec<Cell>>,
    line: Vec<Cell>,
}

impl Builder {
    fn put(&mut self, text: &str, fg: Paint, bg: Paint, flags: u8) -> &mut Self {
        for ch in text.chars() {
            self.line.push(Cell { ch: ch.to_string(), fg, bg, flags });
        }
        self
    }
    fn t(&mut self, text: &str) -> &mut Self {
        self.put(text, Paint::Default, Paint::Default, 0)
    }
    fn c(&mut self, text: &str, n: u8) -> &mut Self {
        self.put(text, Paint::Pal(n), Paint::Default, 0)
    }
    fn b(&mut self, text: &str, n: u8) -> &mut Self {
        self.put(text, Paint::Pal(n), Paint::Default, BOLD)
    }
    fn end(&mut self) {
        let mut l = std::mem::take(&mut self.line);
        l.truncate(self.cols);
        l.resize(self.cols, Cell::blank());
        self.rows.push(l);
    }
    fn prompt(&mut self, cmd: &str) {
        self.b("~/notes", 4).c(" main", 5).b(" ❯ ", 2).t(cmd);
        self.end();
    }
}

/// zsh in her notes folder: ls, git, a color table, text styles.
pub fn shell(cols: usize) -> Screen {
    let mut s = Builder { cols, rows: Vec::new(), line: Vec::new() };
    s.prompt("ls -F");
    s.b("algebra/", 4).t("  ").b("figures/", 4).t("  ");
    s.put("group-actions.tex", Paint::Default, Paint::Default, SELECTED);
    s.t("  notes.pdf  ").b("build.sh*", 2).t("  ").c("refs.bib@", 6);
    s.end();
    s.prompt("git status -s");
    s.c(" M", 1).t(" group-actions.tex");
    s.end();
    s.c("??", 1).t(" figures/orbits.svg");
    s.end();
    s.prompt("git log --oneline -2");
    s.c("3f2a91c (", 3).b("HEAD -> ", 6).b("main", 2).c(")", 3).t(" Orbit–stabilizer: the proof");
    s.end();
    s.c("b81c0de", 3).t(" Group actions: orbits and stabilizers");
    s.end();
    s.prompt("colors");
    for row in [0u8, 8] {
        s.t(" ");
        for n in row..row + 8 {
            let dark = crate::ghostty::is_dark(crate::ghostty::xterm256(n)) || matches!(n % 8, 1 | 4 | 5);
            let fg = if dark { Paint::Pal(15) } else { Paint::Pal(0) };
            s.put(&format!(" {n:>2} "), fg, Paint::Pal(n), 0);
            s.t(" ");
        }
        s.end();
    }
    s.put("bold", Paint::Default, Paint::Default, BOLD).t(" ");
    s.put("faint", Paint::Default, Paint::Default, FAINT).t(" ");
    s.put("italic", Paint::Default, Paint::Default, ITALIC).t(" ");
    s.put("underline", Paint::Default, Paint::Default, UNDERLINE).t(" ");
    s.put("strike", Paint::Default, Paint::Default, STRIKE).t(" ");
    s.put("reverse", Paint::Default, Paint::Default, REVERSE).t("  ");
    s.b("warning:", 3).t(" overfull \\hbox  ").b("error:", 1).t(" undefined \\Gx");
    s.end();
    s.prompt("");
    let rows = s.rows.len();
    let cursor_col = "~/notes main ❯ ".chars().count();
    Screen { cols, rows, cells: s.rows, cursor: Some((rows - 1, cursor_col)) }
}
