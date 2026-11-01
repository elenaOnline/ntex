//! The Terminal page (⌘7): Ghostty's look as a design tool. A live window of
//! Ghostty (ntex above, a shell below, both in your real font and metrics)
//! where clicking anything selects the option that draws it, dragging the
//! padding or the line gaps sets them, and ⌘-scrolling sets the font size;
//! a palette strip, an inspector for the selected option, a gallery of
//! Ghostty's themes, and every other look option in a drawer.
//!
//! Each change is written about 150 ms after the last one to
//! ~/.config/ghostty/ntex-look.ghostty (core::ghostty explains the file),
//! checked with `ghostty +validate-config`, and Ghostty (SIGUSR2) and cmux
//! (`cmux reload-config`) reload it without coming to the front. "Applies
//! to" decides whether cmux follows (its own config.ghostty: its background
//! and foreground lines kept in step, or Studio's marked block pinning a
//! look of its own). The page's state is terminal-look.json; saved looks
//! are in looks.json ("terminal").
mod controls;
pub mod preview;

use crate::core::ghostty::screen::{self, Screen, Target};
use crate::core::ghostty::{self as g, AppliesTo, Defaults, TermLook, TermState, Theme};
use crate::core::looks::Saved;
use crate::io::ghostty::{Applied, Apply, CmuxEdit, Places};
use crate::ui::app::Studio;
use crate::ui::widgets::text_field::{TextField, TextFieldEvent};
use gpui::{App, AppContext as _, Bounds, Context, Entity, Pixels, SharedString, Subscription, Window, px};
use std::cell::Cell;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

/// Write this long after the last change.
pub const DEBOUNCE: Duration = Duration::from_millis(150);
/// Re-capture ntex this long after the preview's grid size settles.
const RECAPTURE: Duration = Duration::from_millis(400);

/// The bundled capture of ntex (first frame, tests, no nvim).
const BUNDLED_CAPTURE: &str = include_str!("../../../../core/src/ghostty/data/ntex-capture.json");

/// What the background loader brings.
struct Loaded {
    defaults: Defaults,
    effective: TermLook,
    themes: Vec<Theme>,
    pinned: BTreeSet<String>,
    connected: bool,
    fonts: Vec<String>,
}

/// A drag on the preview.
#[derive(Clone, Debug, PartialEq)]
pub enum DragKind {
    /// A padding edge: which key, which half of an `a,b` pair, +1 if moving
    /// the mouse right/down grows it, -1 if left/up does.
    Padding { key: &'static str, half: usize, sign: f32 },
    /// The boundary under row `row` (0-based) of a pane: cell height.
    CellHeight { row: usize },
    /// A palette chip being dragged to swap.
    Swap(u8),
}

#[derive(Clone, Debug)]
pub struct Drag {
    pub kind: DragKind,
    pub start: gpui::Point<Pixels>,
    /// The option's value when the drag began.
    pub from: String,
    pub moved: bool,
    /// What a click (no movement) selects.
    pub click: Target,
}

/// Where the panes' grids were drawn (for turning a click into a cell).
#[derive(Default)]
pub struct Geometry {
    pub ntex: Cell<Option<Bounds<Pixels>>>,
    pub shell: Cell<Option<Bounds<Pixels>>>,
}

pub struct TerminalPage {
    pub places: Places,
    pub defaults: Rc<Defaults>,
    pub state: TermState,
    pub saved: Vec<Saved<TermLook>>,
    pub themes: Rc<Vec<Theme>>,
    pub pinned: BTreeSet<String>,
    pub connected: bool,
    pub fonts: Rc<Vec<String>>,
    loader: Option<Receiver<Loaded>>,
    pub loaded: bool,
    pub ntex: Rc<Screen>,
    capture_rx: Option<Receiver<Result<Screen, String>>>,
    /// The grid size the preview wants for ntex, and since when.
    want: Option<((usize, usize), Instant)>,
    pub captured: (usize, usize),
    pub capture_error: Option<String>,
    /// The option the inspector shows.
    pub selected: Target,
    /// An option hovered in the drawer / inspector: the preview marks it.
    pub hover: Option<String>,
    pub drag: Option<Drag>,
    pub geometry: Rc<Geometry>,
    /// The font's own cell height at the last draw (cell-height drags).
    pub last_base_ch: Cell<f32>,
    pub hex: Entity<TextField>,
    pub name: Entity<TextField>,
    pub filter: Entity<TextField>,
    /// The raw value field (Text options), and which option it edits.
    pub raw: Entity<TextField>,
    pub raw_key: Option<String>,
    pub saved_selected: Option<String>,
    pub gallery: Gallery,
    pub drawer_open: bool,
    pub groups_open: BTreeSet<&'static str>,
    /// Preview the dark half of a light/dark pair.
    pub show_dark: bool,
    dirty: Option<Instant>,
    apply_rx: Option<Receiver<Applied>>,
    /// The last write, as a line for the page.
    pub status: Option<(bool, String)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GalleryShow {
    Light,
    Dark,
    All,
}

pub struct Gallery {
    pub show: GalleryShow,
    pub all: bool,
    /// Picking the two halves of a light/dark pair instead of a base.
    pub pairing: bool,
    pub pair: (Option<String>, Option<String>),
}

impl TerminalPage {
    pub fn new(window: &mut Window, cx: &mut Context<Studio>, subs: &mut Vec<Subscription>) -> TerminalPage {
        let hex = cx.new(|cx| TextField::new(cx).mono().width(px(96.)).placeholder("#rrggbb"));
        subs.push(cx.subscribe_in(&hex, window, |s: &mut Studio, _, ev: &TextFieldEvent, window, cx| match ev {
            TextFieldEvent::Submit | TextFieldEvent::Blur => s.term_hex_entered(cx),
            TextFieldEvent::Cancel => {
                s.term_sync_hex(cx);
                s.focus.clone().focus(window, cx);
            }
            TextFieldEvent::Changed => {}
        }));
        let name = cx.new(|cx| TextField::new(cx).width(px(220.)).placeholder("a name for this look"));
        subs.push(cx.subscribe_in(&name, window, |s: &mut Studio, _, ev: &TextFieldEvent, _, cx| {
            if let TextFieldEvent::Submit = ev {
                s.term_save_as(cx)
            }
        }));
        let filter = cx.new(|cx| TextField::new(cx).width(px(200.)).placeholder("find a theme"));
        subs.push(cx.subscribe_in(&filter, window, |_: &mut Studio, _, _: &TextFieldEvent, _, cx| cx.notify()));
        let raw = cx.new(|cx| TextField::new(cx).mono().width(px(320.)).placeholder("as Ghostty writes it"));
        subs.push(cx.subscribe_in(&raw, window, |s: &mut Studio, _, ev: &TextFieldEvent, window, cx| match ev {
            TextFieldEvent::Submit => s.term_raw_entered(cx),
            TextFieldEvent::Cancel => {
                s.terminal.raw_key = None;
                s.focus.clone().focus(window, cx);
                cx.notify();
            }
            _ => {}
        }));
        let defaults = Defaults::bundled();
        TerminalPage {
            places: Places::from_env(),
            defaults: Rc::new(defaults),
            state: TermState::default(),
            saved: Vec::new(),
            themes: Rc::new(Vec::new()),
            pinned: BTreeSet::new(),
            connected: false,
            fonts: Rc::new(Vec::new()),
            loader: None,
            loaded: false,
            ntex: Rc::new(screen::from_capture(BUNDLED_CAPTURE).unwrap_or_else(|_| screen::shell(80))),
            capture_rx: None,
            want: None,
            captured: (0, 0),
            capture_error: None,
            selected: Target::key("background"),
            hover: None,
            drag: None,
            geometry: Rc::new(Geometry::default()),
            last_base_ch: Cell::new(16.0),
            hex,
            name,
            filter,
            raw,
            raw_key: None,
            saved_selected: None,
            gallery: Gallery { show: GalleryShow::Light, all: false, pairing: false, pair: (None, None) },
            drawer_open: false,
            groups_open: BTreeSet::new(),
            show_dark: false,
            dirty: None,
            apply_rx: None,
            status: None,
        }
    }

    /// No change waiting to be written, no write running.
    pub fn idle(&self) -> bool {
        self.dirty.is_none() && self.apply_rx.is_none()
    }

    pub fn d(&self) -> &Defaults {
        &self.defaults
    }

    /// The look the preview draws: the look, or for a light/dark pair the
    /// shown half's theme under it.
    pub fn shown(&self) -> TermLook {
        let l = &self.state.look;
        if let Some((light, dark)) = l.pair() {
            let name = if self.show_dark { dark } else { light };
            if let Some(t) = self.themes.iter().find(|t| t.name == name) {
                let mut s = l.clone();
                s.apply_theme(t, &self.defaults);
                return s;
            }
        }
        l.clone()
    }
}

fn state_path(s: &Studio) -> PathBuf {
    s.paths.config.join("terminal-look.json")
}

pub const TODAY: &str = "Ghostty today";
pub const GHOSTTY_DEFAULT: &str = "Ghostty default";

impl Studio {
    /// The page was opened: read the state, ask Ghostty (in the background).
    pub fn reload_terminal(&mut self, cx: &mut Context<Studio>) {
        let p = state_path(self);
        self.terminal.places = Places::from_env();
        if let Some(st) = std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str::<TermState>(&t).ok()) {
            self.terminal.state = st;
        } else if !self.terminal.loaded {
            // until Ghostty answers: her config files as they read
            let pl = &self.terminal.places;
            let read = |f: PathBuf| std::fs::read_to_string(f).unwrap_or_default();
            let (main, app) = (read(pl.main()), read(pl.app.clone()));
            let mut l = TermLook::from_config_texts(&[&main, &app], &self.terminal.defaults);
            l.reset("theme");
            self.terminal.state.look = l;
        }
        self.terminal.saved = crate::core::looks::read(&self.paths).terminal;
        if self.terminal.loader.is_none() && !self.terminal.loaded {
            let places = self.terminal.places.clone();
            let fonts_from = cx.text_system().clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let defaults = places.defaults();
                let effective = places.effective(&defaults);
                let themes = places.themes(&defaults);
                let mut fonts = fonts_from.all_font_names();
                fonts.sort();
                fonts.dedup();
                let _ = tx.send(Loaded { pinned: places.pinned(), connected: places.connected(), defaults, effective, themes, fonts });
            });
            self.terminal.loader = Some(rx);
        } else {
            self.terminal.connected = self.terminal.places.connected();
            self.terminal.pinned = self.terminal.places.pinned();
        }
        self.term_sync_hex(cx);
    }

    /// Drain the page's receivers, run the write debounce and the re-capture.
    pub fn pump_terminal(&mut self, _now: Instant, cx: &mut Context<Studio>) -> bool {
        // the page's own timers run on the wall clock (its writes and
        // captures are real processes, whatever clock the app runs on)
        let now = Instant::now();
        let mut changed = false;
        let t = &mut self.terminal;
        if let Some(rx) = &t.loader {
            match rx.try_recv() {
                Ok(l) => {
                    t.loader = None;
                    t.loaded = true;
                    t.defaults = Rc::new(l.defaults);
                    t.themes = Rc::new(l.themes);
                    t.pinned = l.pinned;
                    t.connected = l.connected;
                    t.fonts = Rc::new(l.fonts);
                    let first = !state_path(self).exists();
                    self.term_first_run(l.effective, first, cx);
                    changed = true;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.terminal.loader = None,
            }
        }
        let t = &mut self.terminal;
        if let Some(rx) = &t.capture_rx {
            match rx.try_recv() {
                Ok(r) => {
                    t.capture_rx = None;
                    match r {
                        Ok(s) => {
                            t.ntex = Rc::new(s);
                            t.capture_error = None;
                        }
                        Err(e) => t.capture_error = Some(e),
                    }
                    changed = true;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => t.capture_rx = None,
            }
        }
        if t.capture_rx.is_none() {
            if let Some((size, since)) = t.want {
                if size != t.captured && now.saturating_duration_since(since) >= RECAPTURE && !self.quiet_capture() {
                    let paths = self.paths.clone();
                    let (tx, rx) = std::sync::mpsc::channel();
                    std::thread::spawn(move || {
                        let _ = tx.send(crate::io::ghostty::capture_ntex(&paths, size.0, size.1));
                    });
                    let t = &mut self.terminal;
                    t.capture_rx = Some(rx);
                    t.captured = size;
                }
            }
        }
        let t = &mut self.terminal;
        if let Some(rx) = &t.apply_rx {
            match rx.try_recv() {
                Ok(done) => {
                    t.apply_rx = None;
                    t.status = Some(applied_line(&done, t.state.applies));
                    changed = true;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => t.apply_rx = None,
            }
        }
        if t.apply_rx.is_none() && t.dirty.is_some_and(|at| now.saturating_duration_since(at) >= DEBOUNCE) {
            t.dirty = None;
            let job = self.term_job();
            let places = self.terminal.places.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(places.apply(&job));
            });
            self.terminal.apply_rx = Some(rx);
        }
        changed
    }

    /// Captures run headless nvim with her config; tests set
    /// NTEX_STUDIO_NO_CAPTURE to stay on the bundled one.
    fn quiet_capture(&self) -> bool {
        std::env::var_os("NTEX_STUDIO_NO_CAPTURE").is_some()
    }

    /// Ask for ntex at this grid size (the preview calls it while drawing).
    pub fn term_want_capture(&mut self, size: (usize, usize)) {
        let now = Instant::now();
        let t = &mut self.terminal;
        if size == t.captured || size.0 < 20 || size.1 < 4 {
            return;
        }
        match t.want {
            Some((s, _)) if s == size => {}
            _ => t.want = Some((size, now)),
        }
    }

    /// Ghostty answered: the effective look becomes "Ghostty today" (once,
    /// protected) and, without a saved state, the look being edited.
    fn term_first_run(&mut self, effective: TermLook, first: bool, cx: &mut Context<Studio>) {
        let mut looks = crate::core::looks::read(&self.paths);
        if !looks.terminal.iter().any(|s| s.name == TODAY) {
            let mut snap = effective.clone();
            snap.reset("theme");
            looks.terminal.insert(0, Saved { name: TODAY.into(), protected: true, look: snap });
            if let Err(e) = crate::core::looks::write(&self.paths, &looks) {
                self.toast(format!("! Could not write looks.json: {e}"), cx);
            }
        }
        self.terminal.saved = looks.terminal;
        if first {
            let mut l = effective;
            l.reset("theme");
            self.terminal.state = TermState { look: l, applies: AppliesTo::Both, other: None, name: Some(TODAY.into()) };
            self.term_save_state(cx);
        }
        self.term_sync_hex(cx);
    }

    fn term_save_state(&mut self, cx: &mut Context<Studio>) {
        let p = state_path(self);
        let text = serde_json::to_string_pretty(&self.terminal.state).unwrap_or_default() + "\n";
        if let Err(e) = std::fs::write(&p, text) {
            self.toast(format!("! Could not write {}: {e}", p.display()), cx);
        }
    }

    /// Change the look; it is written after the debounce.
    pub fn set_term(&mut self, change: impl FnOnce(&mut TermLook, &Defaults), cx: &mut Context<Studio>) {
        let before = self.terminal.state.look.clone();
        let d = self.terminal.defaults.clone();
        change(&mut self.terminal.state.look, &d);
        if self.terminal.state.look == before {
            return;
        }
        self.term_changed(cx);
    }

    /// The look (or where it applies) changed: save the state, write soon.
    pub fn term_changed(&mut self, cx: &mut Context<Studio>) {
        self.term_save_state(cx);
        self.terminal.dirty = Some(Instant::now());
        self.term_sync_hex(cx);
        cx.notify();
    }

    /// The header name: the saved look it matches, or "… (edited)".
    fn term_title(&self) -> String {
        let t = &self.terminal;
        if let Some(s) = self.term_all_saved().iter().find(|s| s.look == t.state.look) {
            return s.name.clone();
        }
        match &t.state.name {
            Some(n) => format!("{n} (edited)"),
            None => "edited".into(),
        }
    }

    /// The write for the current state (see the module docs).
    fn term_job(&self) -> Apply {
        let t = &self.terminal;
        let d = t.d();
        let title = self.term_title();
        let mut all = t.pinned.clone();
        all.extend(t.state.look.set.keys().cloned());
        if let Some(o) = &t.state.other {
            all.extend(o.set.keys().cloned());
        }
        let look_file = t.state.look.to_file(&title, d, &t.pinned);
        if !t.connected {
            // Ghostty doesn't read it yet: write the file only, touch no app
            return Apply { ghostty: Some(look_file), cmux: None };
        }
        match t.state.applies {
            AppliesTo::Both => {
                let c = g::Colors::of(&t.shown(), d);
                Apply { ghostty: Some(look_file), cmux: Some(CmuxEdit::Sync { bg: g::hex(c.bg), fg: g::hex(c.fg), block: None }) }
            }
            AppliesTo::GhosttyOnly => {
                let other = t.state.other.clone().unwrap_or_default();
                Apply { ghostty: Some(look_file), cmux: Some(CmuxEdit::Block(Some(other.to_file("cmux keeps this (Ghostty only)", d, &all)))) }
            }
            AppliesTo::CmuxOnly => {
                let other = t.state.other.clone().unwrap_or_default();
                Apply {
                    ghostty: Some(other.to_file("Ghostty keeps this (cmux only)", d, &t.pinned)),
                    cmux: Some(CmuxEdit::Block(Some(t.state.look.to_file(&format!("{title} (cmux only)"), d, &all)))),
                }
            }
        }
    }

    /// Where changes go. Leaving "both" pins the other app to the look as it is now.
    pub fn term_applies(&mut self, to: AppliesTo, cx: &mut Context<Studio>) {
        let st = &mut self.terminal.state;
        if st.applies == to {
            return;
        }
        st.other = match to {
            AppliesTo::Both => None,
            _ => Some(st.other.clone().filter(|_| st.applies != AppliesTo::Both).unwrap_or_else(|| st.look.clone())),
        };
        st.applies = to;
        self.term_changed(cx);
    }

    /// Add the include line to her Ghostty config (her press: the button).
    pub fn term_connect(&mut self, cx: &mut Context<Studio>) {
        match self.terminal.places.connect() {
            Ok(()) => {
                self.terminal.connected = true;
                self.terminal.pinned = self.terminal.places.pinned();
                self.toast("Ghostty now loads ntex-look.ghostty (the last line of ~/.config/ghostty/config).", cx);
                self.term_changed(cx);
            }
            Err(e) => self.toast(format!("! {e}"), cx),
        }
    }

    /// The saved look this one started from (for "changed" marks).
    pub fn term_base(&self) -> Option<TermLook> {
        let name = self.terminal.state.name.as_deref()?;
        self.term_all_saved().into_iter().find(|s| s.name == name).map(|s| s.look)
            .or_else(|| self.terminal.themes.iter().find(|t| t.name == name).map(|t| {
                let mut l = self.terminal.state.look.clone();
                l.apply_theme(t, self.terminal.d());
                l
            }))
    }

    /// Has `key` changed since the look it started from?
    pub fn term_changed_key(&self, key: &str, base: &Option<TermLook>) -> bool {
        let t = &self.terminal;
        match base {
            Some(b) => b.get(key, t.d()) != t.state.look.get(key, t.d()),
            None => t.state.look.edited(key),
        }
    }

    // -- selection and colors ------------------------------------------------

    pub fn term_select(&mut self, target: Target, cx: &mut Context<Studio>) {
        self.terminal.selected = target;
        self.terminal.raw_key = None;
        self.term_sync_hex(cx);
        cx.notify();
    }

    /// The selected option's key, if it is a color option.
    pub fn term_color_key(&self) -> Option<String> {
        match &self.terminal.selected {
            Target::Key(k) if k.starts_with("palette:") => Some(k.clone()),
            Target::Key(k) => matches!(g::catalog::find(k).map(|o| o.kind), Some(g::catalog::Kind::Color { .. })).then(|| k.clone()),
            Target::Fixed(_) => None,
        }
    }

    /// The color the selected color option draws in (resolved).
    pub fn term_selected_color(&self) -> Option<u32> {
        let t = &self.terminal;
        let key = self.term_color_key()?;
        let shown = t.shown();
        shown.color(&key, t.d()).or_else(|| {
            let c = g::Colors::of(&shown, t.d());
            Some(match key.as_str() {
                "cursor-color" => c.cursor.resolve(c.fg, c.bg),
                "cursor-text" => c.cursor_text.resolve(c.fg, c.bg),
                "selection-background" => c.sel_bg.resolve(c.fg, c.bg),
                "selection-foreground" => c.sel_fg.resolve(c.fg, c.bg),
                "split-divider-color" => c.divider,
                "unfocused-split-fill" => c.unfocused_fill,
                "bold-color" => c.fg,
                _ => c.fg,
            })
        })
    }

    pub fn term_sync_hex(&mut self, cx: &mut Context<Studio>) {
        let text = self.term_selected_color().map(g::hex).unwrap_or_default();
        self.terminal.hex.update(cx, |f, cx| {
            if f.text != text {
                f.set_text(text, cx);
            }
        });
    }

    pub fn term_hex_entered(&mut self, cx: &mut Context<Studio>) {
        let text = self.terminal.hex.read(cx).text.clone();
        let Some(key) = self.term_color_key() else { return };
        match g::parse_color(&text) {
            Some(c) => self.set_term(|l, d| l.put1(&key, g::hex(c), d), cx),
            None if text.trim().is_empty() => {}
            None => self.toast(format!("! {text} is not a color: write it as #rrggbb."), cx),
        }
        self.term_sync_hex(cx);
    }

    pub fn term_set_color(&mut self, key: &str, c: u32, cx: &mut Context<Studio>) {
        let key = key.to_string();
        self.set_term(|l, d| l.put1(&key, g::hex(c), d), cx);
    }

    /// Swap two palette entries (drag one chip onto another).
    pub fn term_swap(&mut self, a: u8, b: u8, cx: &mut Context<Studio>) {
        if a == b {
            return;
        }
        let c = g::Colors::of(&self.terminal.state.look, self.terminal.d());
        let (ca, cb) = (c.palette[a as usize], c.palette[b as usize]);
        self.set_term(
            |l, d| {
                l.put1(&format!("palette:{a}"), g::hex(cb), d);
                l.put1(&format!("palette:{b}"), g::hex(ca), d);
            },
            cx,
        );
        self.toast(format!("Swapped palette {a} and {b}."), cx);
    }

    pub fn term_raw_edit(&mut self, key: &str, window: &mut Window, cx: &mut Context<Studio>) {
        let v = self.terminal.state.look.get(key, self.terminal.d()).join(", ");
        self.terminal.raw_key = Some(key.to_string());
        self.terminal.raw.update(cx, |f, cx| {
            f.set_text(v, cx);
            f.select_all(cx);
        });
        self.terminal.raw.read(cx).focus.clone().focus(window, cx);
        cx.notify();
    }

    /// Return in the raw field: the value as Ghostty writes it (a list:
    /// comma-separated for repeatable keys), checked by Ghostty.
    pub fn term_raw_entered(&mut self, cx: &mut Context<Studio>) {
        let Some(key) = self.terminal.raw_key.take() else { return };
        let text = self.terminal.raw.read(cx).text.trim().to_string();
        let vals: Vec<String> = if g::catalog::is_repeatable(&key) {
            text.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
        } else {
            vec![text]
        };
        // ask Ghostty before taking it
        let mut probe = TermLook::default();
        probe.put(&key, vals.clone(), self.terminal.d());
        let tmp = std::env::temp_dir().join(format!("ntex-studio-probe-{}.ghostty", std::process::id()));
        let _ = std::fs::write(&tmp, probe.to_file("probe", self.terminal.d(), &BTreeSet::new()));
        let check = self.terminal.places.validate(Some(&tmp));
        let _ = std::fs::remove_file(&tmp);
        match check {
            Ok(()) => self.set_term(|l, d| l.put(&key, vals, d), cx),
            Err(e) => self.toast(format!("! Ghostty doesn't take that: {}", e.lines().last().unwrap_or("")), cx),
        }
        cx.notify();
    }

    // -- saved looks ---------------------------------------------------------

    /// The presets (Ghostty default) and her saved looks.
    pub fn term_all_saved(&self) -> Vec<Saved<TermLook>> {
        let mut v = vec![Saved { name: GHOSTTY_DEFAULT.into(), protected: true, look: TermLook::default() }];
        v.extend(self.terminal.saved.iter().cloned());
        v
    }

    fn term_write_saved(&mut self, what: String, cx: &mut Context<Studio>) {
        let mut looks = crate::core::looks::read(&self.paths);
        looks.terminal = self.terminal.saved.clone();
        match crate::core::looks::write(&self.paths, &looks) {
            Ok(()) => self.toast(what, cx),
            Err(e) => self.toast(format!("! Could not write looks.json: {e}"), cx),
        }
        cx.notify();
    }

    pub fn term_restore(&mut self, name: &str, cx: &mut Context<Studio>) {
        let Some(saved) = self.term_all_saved().into_iter().find(|s| s.name == name) else { return };
        self.terminal.saved_selected = Some(name.to_string());
        self.terminal.name.update(cx, |f, cx| f.set_text(name, cx));
        self.terminal.state.look = saved.look;
        self.terminal.state.name = Some(name.to_string());
        self.term_changed(cx);
        self.toast(format!("Back to “{name}”."), cx);
    }

    pub fn term_save_as(&mut self, cx: &mut Context<Studio>) {
        let name = self.terminal.name.read(cx).text.trim().to_string();
        if name.is_empty() {
            return self.toast("! Give the look a name first.", cx);
        }
        if self.term_all_saved().iter().any(|s| s.name == name && s.protected) {
            return self.toast(format!("! “{name}” is protected; pick another name."), cx);
        }
        let look = self.terminal.state.look.clone();
        match self.terminal.saved.iter_mut().find(|s| s.name == name) {
            Some(s) => s.look = look,
            None => self.terminal.saved.push(Saved { name: name.clone(), protected: false, look }),
        }
        self.terminal.saved_selected = Some(name.clone());
        self.terminal.state.name = Some(name.clone());
        self.term_save_state(cx);
        self.term_write_saved(format!("Saved the terminal look as “{name}”."), cx);
    }

    pub fn term_rename(&mut self, cx: &mut Context<Studio>) {
        let Some(from) = self.terminal.saved_selected.clone() else { return };
        let to = self.terminal.name.read(cx).text.trim().to_string();
        if to.is_empty() || self.term_all_saved().iter().any(|s| s.name == to) {
            return self.toast(format!("! “{to}” is taken or empty."), cx);
        }
        let Some(s) = self.terminal.saved.iter_mut().find(|s| s.name == from && !s.protected) else { return };
        s.name = to.clone();
        if self.terminal.state.name.as_deref() == Some(from.as_str()) {
            self.terminal.state.name = Some(to.clone());
            self.term_save_state(cx);
        }
        self.terminal.saved_selected = Some(to.clone());
        self.term_write_saved(format!("Renamed “{from}” to “{to}”."), cx);
    }

    pub fn term_delete(&mut self, cx: &mut Context<Studio>) {
        let Some(name) = self.terminal.saved_selected.clone() else { return };
        let before = self.terminal.saved.len();
        self.terminal.saved.retain(|s| !(s.name == name && !s.protected));
        if self.terminal.saved.len() == before {
            return;
        }
        self.terminal.saved_selected = None;
        self.term_write_saved(format!("Deleted “{name}”."), cx);
    }

    // -- themes --------------------------------------------------------------

    /// Click a theme: start from it (or, pairing, make it one half).
    pub fn term_theme(&mut self, name: &str, cx: &mut Context<Studio>) {
        let Some(theme) = self.terminal.themes.iter().find(|t| t.name == name).cloned() else { return };
        if self.terminal.gallery.pairing {
            let g = &mut self.terminal.gallery;
            if theme.dark {
                g.pair.1 = Some(theme.name.clone());
            } else {
                g.pair.0 = Some(theme.name.clone());
            }
            if let (Some(l), Some(dk)) = g.pair.clone() {
                let v = format!("light:{l},dark:{dk}");
                self.set_term(|look, d| look.put1("theme", v, d), cx);
                self.toast(format!("Light/dark pair: {l} and {dk}."), cx);
            }
            cx.notify();
            return;
        }
        self.set_term(|l, d| l.apply_theme(&theme, d), cx);
        self.terminal.state.name = Some(theme.name.clone());
        self.term_save_state(cx);
        self.toast(format!("Started from {}: tweak anything.", theme.name), cx);
    }

    pub fn terminal_hints(&self, _window: &Window, _cx: &App) -> Vec<SharedString> {
        vec![
            "Click anything in the window to edit what draws it. Drag the padding, or between two lines for the cell height; ⌘-scroll for the font size.".into(),
            "Every change reaches Ghostty and cmux about 150 ms after you stop (ntex-look.ghostty, reloaded in place).".into(),
        ]
    }
}

/// The page's line about the last write.
fn applied_line(a: &Applied, to: AppliesTo) -> (bool, String) {
    if !a.errors.is_empty() {
        return (false, a.errors.join(" · "));
    }
    if a.wrote.is_empty() {
        return (true, "Nothing to write: the files already say this.".into());
    }
    let mut parts = vec![format!(
        "Wrote {}",
        a.wrote.iter().map(|p| p.file_name().unwrap_or_default().to_string_lossy().into_owned()).collect::<Vec<_>>().join(" and ")
    )];
    if !a.live {
        return (true, parts.join(" · ") + " (nothing reloaded: a test folder).");
    }
    match a.ghostty_reloaded {
        0 if to != AppliesTo::CmuxOnly => parts.push("Ghostty isn't running (it picks this up when it starts)".into()),
        0 => {}
        1 => parts.push("Ghostty reloaded".into()),
        n => parts.push(format!("{n} Ghosttys reloaded")),
    }
    match &a.cmux_reloaded {
        Some(Ok(())) => parts.push("cmux reloaded".into()),
        Some(Err(e)) => parts.push(format!("cmux: {e}")),
        None => {}
    }
    (true, parts.join(" · ") + ".")
}

