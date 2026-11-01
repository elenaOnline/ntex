//! The Terminal page's outside world: asking Ghostty for its defaults and the
//! effective config, its themes, validating and writing the look file,
//! reloading Ghostty (SIGUSR2) and cmux (`cmux reload-config`) without
//! activating either, cmux's own config, and one screen of ntex captured from
//! a headless nvim with her config (studio/bridge/terminal-capture.lua).
//!
//! Nothing here opens a window or activates an app. Every function may block
//! (processes); the page calls them on a background thread.
//!
//! Tests point everything at a temp folder: `NTEX_STUDIO_GHOSTTY_DIR` (the
//! config folder; cmux's and Ghostty.app's own files then live in it as
//! `cmux.ghostty` / `app.ghostty` unless `NTEX_STUDIO_CMUX_CONFIG` says
//! otherwise), `NTEX_STUDIO_GHOSTTY_BIN` (empty: no Ghostty, use the bundled
//! defaults), and nothing is reloaded unless `NTEX_STUDIO_GHOSTTY_LIVE=1`.
use ntex_studio_core::ghostty::{self as g, Defaults, TermLook, Theme};
use crate::msgpack;
use ntex_studio_core::ghostty::screen::{self, Screen};
use ntex_studio_core::model::Paths;
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const GHOSTTY_APP_BIN: &str = "/Applications/Ghostty.app/Contents/MacOS/ghostty";

#[derive(Clone, Debug)]
pub struct Places {
    /// ~/.config/ghostty
    pub dir: PathBuf,
    /// Ghostty.app's own config (~/Library/Application Support/com.mitchellh.ghostty/config.ghostty).
    pub app: PathBuf,
    /// cmux's own config (~/Library/Application Support/com.cmuxterm.app/config.ghostty).
    pub cmux: PathBuf,
    pub bin: Option<PathBuf>,
    /// Signal Ghostty / run `cmux reload-config` after a write.
    pub live: bool,
}

fn env_path(k: &str) -> Option<PathBuf> {
    std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from)
}

impl Places {
    pub fn from_env() -> Places {
        let home = env_path("HOME").unwrap_or_else(|| PathBuf::from("/"));
        let support = home.join("Library/Application Support");
        let (dir, app, cmux, live) = match env_path("NTEX_STUDIO_GHOSTTY_DIR") {
            Some(d) => (d.clone(), d.join("app.ghostty"), d.join("cmux.ghostty"), std::env::var("NTEX_STUDIO_GHOSTTY_LIVE").is_ok_and(|v| v == "1")),
            None => (
                env_path("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config")).join("ghostty"),
                support.join("com.mitchellh.ghostty/config.ghostty"),
                support.join("com.cmuxterm.app/config.ghostty"),
                true,
            ),
        };
        let cmux = env_path("NTEX_STUDIO_CMUX_CONFIG").unwrap_or(cmux);
        let bin = match std::env::var_os("NTEX_STUDIO_GHOSTTY_BIN") {
            Some(b) if b.is_empty() => None,
            Some(b) => Some(PathBuf::from(b)),
            None => Some(PathBuf::from(GHOSTTY_APP_BIN)),
        }
        .filter(|b| b.exists());
        Places { dir, app, cmux, bin, live }
    }

    pub fn main(&self) -> PathBuf {
        self.dir.join("config")
    }

    pub fn look_file(&self) -> PathBuf {
        self.dir.join(g::FILE_NAME)
    }

    fn ghostty(&self, args: &[&str]) -> Result<String, String> {
        let bin = self.bin.as_ref().ok_or("Ghostty.app is not installed")?;
        let out = run(Command::new(bin).args(args), Duration::from_secs(10))?;
        Ok(out)
    }

    /// Ghostty's defaults and docs (the installed version's; bundled if it can't be asked).
    pub fn defaults(&self) -> Defaults {
        match self.ghostty(&["+show-config", "--default", "--docs"]) {
            Ok(t) if t.contains("font-size") => Defaults::parse(&t),
            _ => Defaults::bundled(),
        }
    }

    /// The effective look now (theme resolved), as Ghostty sees it.
    pub fn effective(&self, d: &Defaults) -> TermLook {
        if self.bin.is_some() && std::env::var_os("NTEX_STUDIO_GHOSTTY_DIR").is_none() {
            if let Ok(t) = self.ghostty(&["+show-config"]) {
                return TermLook::from_show_config(&t, d);
            }
        }
        let main = read(&self.main());
        let app = read(&self.app);
        let look = read(&self.look_file());
        TermLook::from_config_texts(&[&main, &app, &look], d)
    }

    /// The look keys her other config files set (the file must write these).
    pub fn pinned(&self) -> BTreeSet<String> {
        g::pinned_keys(&[&read(&self.main()), &read(&self.app)])
    }

    /// Does the main config include the look file?
    pub fn connected(&self) -> bool {
        g::has_include(&read(&self.main()))
    }

    /// Add the include line to the main config (backup first), then validate.
    pub fn connect(&self) -> Result<(), String> {
        let main = self.main();
        let text = read(&main);
        if g::has_include(&text) {
            return Ok(());
        }
        backup(&main, "ntex-look")?;
        write_atomic(&main, &g::add_include(&text, &today())).map_err(|e| format!("could not write {}: {e}", main.display()))?;
        self.validate(None)
    }

    /// Every theme Ghostty ships plus her own (~/.config/ghostty/themes), by name.
    pub fn themes(&self, d: &Defaults) -> Vec<Theme> {
        let mut dirs = vec![self.dir.join("themes")];
        if let Some(b) = &self.bin {
            if let Some(app) = b.ancestors().nth(2) {
                dirs.push(app.join("Resources/ghostty/themes"));
            }
        }
        if std::env::var_os("NTEX_STUDIO_GHOSTTY_DIR").is_some() || self.bin.is_none() {
            dirs.push(PathBuf::from(GHOSTTY_APP_BIN).ancestors().nth(2).unwrap().join("Resources/ghostty/themes"));
        }
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for dir in dirs {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            let mut files: Vec<_> = rd.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
            files.sort();
            for p in files {
                let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
                if name.starts_with('.') || !seen.insert(name.clone()) {
                    continue;
                }
                out.push(g::parse_theme(&name, &read(&p), d));
            }
        }
        out.sort_by_key(|t| t.name.to_lowercase());
        out
    }

    /// `ghostty +validate-config` on the look file (or `file`). Err: Ghostty's complaints.
    pub fn validate(&self, file: Option<&Path>) -> Result<(), String> {
        let Some(bin) = &self.bin else { return Ok(()) };
        let f = file.map(Path::to_path_buf).unwrap_or_else(|| self.look_file());
        let mut cmd = Command::new(bin);
        cmd.arg("+validate-config");
        if f.exists() {
            cmd.arg(format!("--config-file={}", f.display()));
        }
        if std::env::var_os("NTEX_STUDIO_GHOSTTY_DIR").is_some() {
            // a temp folder: keep Ghostty from reading her real config
            cmd.env("XDG_CONFIG_HOME", self.dir.parent().unwrap_or(&self.dir)).env("HOME", self.dir.parent().unwrap_or(&self.dir));
        }
        let out = cmd.stdin(Stdio::null()).output().map_err(|e| format!("could not run ghostty: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            let mut msg = String::from_utf8_lossy(&out.stdout).into_owned();
            msg.push_str(&String::from_utf8_lossy(&out.stderr));
            Err(msg.trim().to_string())
        }
    }

    /// Apply one change: write the look file and/or edit cmux's config, then
    /// reload what changed. Errors are collected, not fatal.
    pub fn apply(&self, job: &Apply) -> Applied {
        let mut done = Applied { live: self.live, ..Applied::default() };
        if let Some(text) = &job.ghostty {
            let f = self.look_file();
            if read(&f) != *text {
                let r = backup(&f, "studio").and_then(|_| write_atomic(&f, text).map_err(|e| format!("could not write {}: {e}", f.display())));
                match r {
                    Ok(()) => {
                        done.wrote.push(f.clone());
                        if let Err(e) = self.validate(Some(&f)) {
                            done.errors.push(format!("Ghostty says: {e}"));
                        }
                        if self.live {
                            done.ghostty_reloaded = reload_ghostty(self.bin.as_deref());
                        }
                    }
                    Err(e) => done.errors.push(e),
                }
            }
        }
        if let Some(edit) = &job.cmux {
            let f = &self.cmux;
            let before = read(f);
            let after = match edit {
                CmuxEdit::Sync { bg, fg, block } => g::set_cmux_block(&g::sync_cmux_colors(&before, bg, fg), block.as_deref()),
                CmuxEdit::Block(b) => g::set_cmux_block(&before, b.as_deref()),
            };
            if after != before && f.parent().is_some_and(|p| p.exists()) {
                match backup(f, "studio").and_then(|_| write_atomic(f, &after).map_err(|e| format!("could not write {}: {e}", f.display()))) {
                    Ok(()) => {
                        done.wrote.push(f.clone());
                        if self.live {
                            done.cmux_reloaded = Some(reload_cmux());
                        }
                    }
                    Err(e) => done.errors.push(e),
                }
            } else if job.ghostty.is_some() && !done.wrote.is_empty() && self.live {
                // cmux reads the main config (and so the look file) too
                done.cmux_reloaded = Some(reload_cmux());
            }
        } else if !done.wrote.is_empty() && self.live {
            done.cmux_reloaded = Some(reload_cmux());
        }
        done
    }
}

/// One change to apply (`Places::apply`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Apply {
    /// The look file's new text.
    pub ghostty: Option<String>,
    pub cmux: Option<CmuxEdit>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CmuxEdit {
    /// Ghostty + cmux: its own background / foreground lines follow the
    /// look (and Studio's block, if any, goes or becomes `block`).
    Sync { bg: String, fg: String, block: Option<String> },
    /// One app only: Studio's block holds cmux's look (None: no block).
    Block(Option<String>),
}

#[derive(Clone, Debug, Default)]
pub struct Applied {
    /// Reloads were on (off in tests).
    pub live: bool,
    pub wrote: Vec<PathBuf>,
    pub errors: Vec<String>,
    /// Ghostty processes signaled.
    pub ghostty_reloaded: usize,
    /// `cmux reload-config` ran (Err: why not).
    pub cmux_reloaded: Option<Result<(), String>>,
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

fn write_atomic(p: &Path, text: &str) -> std::io::Result<()> {
    let tmp = p.with_extension("ntex-studio.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, p)
}

/// The first write to a file in this run backs it up as
/// `<name>.bak-YYYYMMDD-HHMMSS-<why>` (if it exists).
fn backup(p: &Path, why: &str) -> Result<(), String> {
    static DONE: Mutex<Option<HashSet<PathBuf>>> = Mutex::new(None);
    let mut done = DONE.lock().unwrap_or_else(|e| e.into_inner());
    let set = done.get_or_insert_with(HashSet::new);
    if set.contains(p) {
        return Ok(());
    }
    if p.exists() {
        let name = p.file_name().unwrap_or_default().to_string_lossy();
        let b = p.with_file_name(format!("{name}.bak-{}-{why}", stamp()));
        std::fs::copy(p, &b).map_err(|e| format!("could not back up {}: {e}", p.display()))?;
    }
    set.insert(p.to_path_buf());
    Ok(())
}

fn local() -> libc::tm {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as libc::time_t).unwrap_or(0);
    // SAFETY: zeroed tm is a valid out-buffer; localtime_r writes only into it.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&secs, &mut tm) };
    tm
}

fn stamp() -> String {
    let t = local();
    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", t.tm_year + 1900, t.tm_mon + 1, t.tm_mday, t.tm_hour, t.tm_min, t.tm_sec)
}

fn today() -> String {
    let t = local();
    format!("{:04}-{:02}-{:02}", t.tm_year + 1900, t.tm_mon + 1, t.tm_mday)
}

/// Run a command to its end (killed after `limit`), stdout as text.
fn run(cmd: &mut Command, limit: Duration) -> Result<String, String> {
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    let mut stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut stdout, &mut s);
        s
    });
    let t0 = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                let out = reader.join().unwrap_or_default();
                return if st.success() { Ok(out) } else { Err(format!("exit {st}: {}", out.trim())) };
            }
            Ok(None) if t0.elapsed() > limit => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("timed out".into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => return Err(e.to_string()),
        }
    }
}

/// SIGUSR2 to every running Ghostty.app (the app itself, not `ghostty +…`
/// helpers): Ghostty 1.2+ reloads its config on it, without activating.
/// Returns how many were signaled.
pub fn reload_ghostty(bin: Option<&Path>) -> usize {
    let bin = bin.map(|b| b.to_string_lossy().into_owned()).unwrap_or_else(|| GHOSTTY_APP_BIN.into());
    let Ok(ps) = run(Command::new("/bin/ps").args(["-axo", "pid=,args="]), Duration::from_secs(5)) else { return 0 };
    let mut n = 0;
    for line in ps.lines() {
        let line = line.trim();
        let Some((pid, args)) = line.split_once(' ') else { continue };
        if args.trim() != bin {
            continue;
        }
        if let Ok(pid) = pid.parse::<i32>() {
            // SAFETY: plain kill(2) with a signal Ghostty handles.
            if unsafe { libc::kill(pid, libc::SIGUSR2) } == 0 {
                n += 1;
            }
        }
    }
    n
}

fn cmux_bin() -> Option<PathBuf> {
    ["/opt/homebrew/bin/cmux", "/usr/local/bin/cmux", "/Applications/cmux.app/Contents/Resources/bin/cmux"].iter().map(PathBuf::from).find(|p| p.exists())
}

pub fn cmux_running() -> bool {
    run(Command::new("/usr/bin/pgrep").args(["-x", "cmux"]), Duration::from_secs(3)).is_ok_and(|o| !o.trim().is_empty())
}

/// `cmux reload-config` (a running cmux reloads Ghostty's config and its own
/// in place; it does not come to the front).
pub fn reload_cmux() -> Result<(), String> {
    if !cmux_running() {
        return Err("cmux is not running".into());
    }
    let bin = cmux_bin().ok_or("the cmux command is not installed")?;
    run(Command::new(bin).arg("reload-config"), Duration::from_secs(5)).map(|_| ())
}

// ---------------------------------------------------------------------------
// ntex, captured

fn studio_file(paths: &Paths, rel: &str) -> PathBuf {
    let p = paths.config.join("studio").join(rel);
    if p.exists() {
        return p;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(rel)
}

/// One highlight as the UI was told it (`hl_attr_define`, rgb).
#[derive(Clone, Copy, Default)]
struct Attr {
    fg: Option<u32>,
    bg: Option<u32>,
    flags: u8,
}

/// The default grid, as `grid_line` / `grid_scroll` / `grid_clear` leave it.
struct Grid {
    cols: usize,
    rows: usize,
    cells: Vec<Vec<(String, u64)>>,
    cursor: (usize, usize),
    attrs: std::collections::HashMap<u64, Attr>,
    default_fg: Option<u32>,
    default_bg: Option<u32>,
    flushes: u32,
}

impl Grid {
    fn new(cols: usize, rows: usize) -> Grid {
        Grid { cols, rows, cells: vec![vec![(" ".into(), 0); cols]; rows], cursor: (0, 0), attrs: Default::default(), default_fg: None, default_bg: None, flushes: 0 }
    }

    fn event(&mut self, name: &str, args: &[msgpack::Value]) {
        let int = |v: &msgpack::Value| v.as_i64().unwrap_or(0);
        let color = |v: Option<&msgpack::Value>| v.and_then(|x| x.as_i64()).filter(|x| *x >= 0).map(|x| x as u32);
        match name {
            "default_colors_set" => {
                self.default_fg = color(args.first());
                self.default_bg = color(args.get(1));
            }
            "hl_attr_define" => {
                let id = int(&args[0]) as u64;
                let a = &args[1];
                let on = |k: &str| matches!(a.get(k), Some(msgpack::Value::Bool(true)));
                let flags = (on("bold") as u8) * screen::BOLD
                    | (on("italic") as u8) * screen::ITALIC
                    | ((on("underline") || on("undercurl") || on("underdouble") || on("underdotted") || on("underdashed")) as u8) * screen::UNDERLINE
                    | (on("reverse") as u8) * screen::REVERSE
                    | (on("strikethrough") as u8) * screen::STRIKE;
                self.attrs.insert(id, Attr { fg: color(a.get("foreground")), bg: color(a.get("background")), flags });
            }
            "grid_resize" => {
                let (w, h) = (int(&args[1]) as usize, int(&args[2]) as usize);
                self.cols = w;
                self.rows = h;
                self.cells.resize(h, vec![(" ".into(), 0); w]);
                for r in &mut self.cells {
                    r.resize(w, (" ".into(), 0));
                }
            }
            "grid_clear" => {
                for r in &mut self.cells {
                    r.iter_mut().for_each(|c| *c = (" ".into(), 0));
                }
            }
            "grid_cursor_goto" => self.cursor = (int(&args[1]) as usize, int(&args[2]) as usize),
            "grid_line" => {
                let row = int(&args[1]) as usize;
                let mut col = int(&args[2]) as usize;
                let mut hl = 0u64;
                for cell in args.get(3).and_then(|c| c.as_array()).unwrap_or(&[]) {
                    let Some(c) = cell.as_array() else { continue };
                    let text = c.first().and_then(|t| t.as_str()).unwrap_or(" ").to_string();
                    if let Some(h) = c.get(1).and_then(|h| h.as_i64()) {
                        hl = h as u64;
                    }
                    let repeat = c.get(2).and_then(|n| n.as_i64()).unwrap_or(1).max(1) as usize;
                    for _ in 0..repeat {
                        if row < self.rows && col < self.cols {
                            self.cells[row][col] = (text.clone(), hl);
                        }
                        col += 1;
                    }
                }
            }
            "grid_scroll" => {
                let (top, bot, left, right, rows) = (int(&args[1]) as usize, int(&args[2]) as usize, int(&args[3]) as usize, int(&args[4]) as usize, int(&args[5]));
                let bot = bot.min(self.rows);
                let right = right.min(self.cols);
                if rows > 0 {
                    for r in top..bot.saturating_sub(rows as usize) {
                        for c in left..right {
                            self.cells[r][c] = self.cells[r + rows as usize][c].clone();
                        }
                    }
                } else {
                    let n = (-rows) as usize;
                    for r in (top + n..bot).rev() {
                        for c in left..right {
                            self.cells[r][c] = self.cells[r - n][c].clone();
                        }
                    }
                }
            }
            "flush" => self.flushes += 1,
            _ => {}
        }
    }

    fn screen(&self) -> Screen {
        let paint = |c: Option<u32>, def: Option<u32>| match c.or(def) {
            Some(x) => screen::Paint::Rgb(x),
            None => screen::Paint::Default,
        };
        let cells = self
            .cells
            .iter()
            .map(|row| {
                row.iter()
                    .map(|(t, hl)| {
                        let a = self.attrs.get(hl).copied().unwrap_or_default();
                        screen::Cell { ch: t.clone(), fg: paint(a.fg, self.default_fg), bg: paint(a.bg, self.default_bg), flags: a.flags }
                    })
                    .collect()
            })
            .collect();
        Screen { cols: self.cols, rows: self.rows, cells, cursor: Some(self.cursor).filter(|(r, c)| *r < self.rows && *c < self.cols) }
    }
}

/// One screen of ntex editing the sample notes (`cols` × `rows`): her config
/// in `nvim --embed` (no language servers), with Studio attached as its UI,
/// exactly the cells and colors a terminal is sent. About a second.
pub fn capture_ntex(paths: &Paths, cols: usize, rows: usize) -> Result<Screen, String> {
    use msgpack::Value as V;
    use std::io::Write;
    let sample = studio_file(paths, "core/src/ghostty/data/notes.tex");
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("ntex-studio-capture-{}-{n}", std::process::id()));
    let notes = dir.join("notes");
    std::fs::create_dir_all(&notes).map_err(|e| e.to_string())?;
    std::fs::copy(&sample, notes.join("group-actions.tex")).map_err(|e| format!("{}: {e}", sample.display()))?;
    let appname = std::env::var("TEXNVIM_APPNAME").unwrap_or_else(|_| "texnvim".into());
    let nvim = ["/opt/homebrew/bin/nvim", "/usr/local/bin/nvim"].iter().map(PathBuf::from).find(|p| p.exists()).unwrap_or_else(|| PathBuf::from("nvim"));
    let mut child = Command::new(nvim)
        .current_dir(&notes)
        .args(["--embed", "-n", "-i", "NONE", "group-actions.tex"])
        .env("NVIM_APPNAME", appname)
        .env("TEXNVIM_NO_LSP", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("could not start nvim: {e}"))?;
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = std::io::BufReader::new(child.stdout.take().unwrap());
    let (tx, rx) = std::sync::mpsc::channel::<V>();
    std::thread::spawn(move || {
        while let Ok(v) = msgpack::read(&mut stdout) {
            if tx.send(v).is_err() {
                break;
            }
        }
    });
    let mut send = |id: i64, method: &str, args: Vec<V>| -> Result<(), String> {
        let mut b = Vec::new();
        msgpack::write(&V::Array(vec![V::Int(0), V::Int(id), V::from(method), V::Array(args)]), &mut b);
        stdin.write_all(&b).and_then(|_| stdin.flush()).map_err(|e| e.to_string())
    };
    let opts = V::Map(vec![(V::from("ext_linegrid"), V::Bool(true)), (V::from("rgb"), V::Bool(true))]);
    let result = (|| {
        send(1, "nvim_ui_attach", vec![V::Int(cols as i64), V::Int(rows as i64), opts])?;
        let mut grid = Grid::new(cols, rows);
        let t0 = Instant::now();
        let mut placed = false;
        let mut last = Instant::now();
        loop {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(V::Array(msg)) => {
                    last = Instant::now();
                    if msg.first().and_then(|x| x.as_i64()) == Some(2) && msg.get(1).and_then(|x| x.as_str()) == Some("redraw") {
                        for batch in msg.get(2).and_then(|x| x.as_array()).unwrap_or(&[]) {
                            let Some(b) = batch.as_array() else { continue };
                            let Some(name) = b.first().and_then(|x| x.as_str()) else { continue };
                            for args in &b[1..] {
                                if let Some(a) = args.as_array() {
                                    grid.event(name, a);
                                }
                            }
                        }
                    }
                }
                Ok(_) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => return Err("nvim quit".to_string()),
            }
            // once it has drawn and gone quiet: put the cursor in the theorem
            if !placed && grid.flushes > 0 && last.elapsed() > Duration::from_millis(250) {
                placed = true;
                send(2, "nvim_exec_lua", vec![V::from("vim.api.nvim_win_set_cursor(0, {13, 10}); vim.cmd('normal! zz')"), V::Array(vec![])])?;
                last = Instant::now();
            }
            // settled after that (deferred decorations included)
            if placed && last.elapsed() > Duration::from_millis(400) {
                return Ok(grid.screen());
            }
            if t0.elapsed() > Duration::from_secs(12) {
                return if grid.flushes > 0 { Ok(grid.screen()) } else { Err("nvim drew nothing".into()) };
            }
        }
    })();
    let _ = send(3, "nvim_command", vec![V::from("qa!")]);
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(500) {
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);
    result
}
