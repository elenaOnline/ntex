//! The studio's state and root view. Owner: the SCREENS builder.
//!
//! One root entity, [`Studio`], owns everything: paths, prefs, the bridge,
//! the renderer, the inbox, the blocks, the undo stack, the toast and the
//! three pages' state (`screens::{snippet, block, inbox}`; each page module
//! adds an `impl Studio` block with its rendering and behaviour).
//!
//! Slow work never runs on the UI thread. The bridge, the renderer and the
//! sorter hand back `std::sync::mpsc` receivers; [`Studio::pump`] drains them
//! (`try_recv`, never blocking) from a `cx.spawn_in` loop on a short timer.
//! Every request carries a generation number; a result for an older draft is
//! dropped. Debounces and the toast use the executor's clock, so the
//! offscreen tests can drive time with `advance_clock` and call `pump`
//! themselves.
use crate::model::*;
use crate::model::Context as SnipContext;
use crate::ui::screens::{block::BlockPage, inbox::InboxPage, library::LibraryPage, shell, snippet::SnippetPage};
use gpui::{
    App, AppContext as _, Bounds, Context, FocusHandle, Focusable, Global, InteractiveElement as _, IntoElement,
    KeyBinding, ParentElement as _, Pixels, Render, RenderImage, ScrollHandle, SharedString, Styled as _, Subscription,
    Task, TitlebarOptions, Window, WindowBounds, WindowHandle, WindowOptions, actions, div, point, px, size,
};
use ntex_studio_io::bridge::Bridge;
use ntex_studio_io::render::{RenderJob, Rendered, Renderer};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant, SystemTime};

actions!(
    studio,
    [
        /// ⌘1
        PageSnippet,
        /// ⌘2
        PageBlock,
        /// ⌘3
        PageInbox,
        /// ⌘4
        PageLibrary,
        /// ⌘F: search the library
        LibSearch,
        /// ↑ in the library (its list or its search field)
        LibUp,
        /// ↓ in the library
        LibDown,
        /// Return in the library: edit the selected snippet
        LibOpen,
        /// ⌘↵
        Save,
        /// ⌘N
        Clear,
        /// ⌘Z outside a text field: undo the last save, delete or sort
        Undo,
        /// ⌘⇧Z outside a text field
        Redo,
        /// ⌘⇧S
        Sort,
        /// ⌘W
        Hide,
        /// Esc outside a field: hide on an empty draft, else blur
        Escape,
        /// ⌘Q
        Quit,
        /// Tab outside the trigger field and the output editor
        FocusNext,
        /// ⇧Tab outside the trigger field and the output editor
        FocusPrev,
        InboxUp,
        InboxDown,
        InboxEdit,
        InboxDelete,
    ]
);

pub const KEY_CONTEXT: &str = "Studio";
pub const INBOX_CONTEXT: &str = "InboxTable";
pub const LIBRARY_CONTEXT: &str = "LibraryList";

/// How often the poll loop drains the receivers.
const POLL: Duration = Duration::from_millis(16);
/// The inbox file's mtime is checked this often (hand edits, sorting).
const MTIME_EVERY: Duration = Duration::from_secs(1);
/// A toast stays at full slate ink this long …
pub const TOAST_FOR: Duration = Duration::from_millis(3500);
/// … then fades out this quickly (a half-faded note is unreadable: keep it short).
pub const TOAST_FADE: Duration = Duration::from_millis(400);

/// Start everything inside a GPUI app: widgets::init and the studio's keys.
pub fn init(cx: &mut gpui::App) {
    crate::ui::widgets::init(cx);
    crate::ui::figure::init(cx);
    let s = Some(KEY_CONTEXT);
    let t = Some(INBOX_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("cmd-1", PageSnippet, s),
        KeyBinding::new("cmd-2", PageBlock, s),
        KeyBinding::new("cmd-3", PageInbox, s),
        KeyBinding::new("cmd-enter", Save, s),
        KeyBinding::new("cmd-n", Clear, s),
        KeyBinding::new("cmd-z", Undo, s),
        KeyBinding::new("cmd-shift-z", Redo, s),
        KeyBinding::new("cmd-shift-s", Sort, s),
        KeyBinding::new("cmd-w", Hide, s),
        KeyBinding::new("escape", Escape, s),
        KeyBinding::new("cmd-q", Quit, s),
        KeyBinding::new("tab", FocusNext, s),
        KeyBinding::new("shift-tab", FocusPrev, s),
        KeyBinding::new("up", InboxUp, t),
        KeyBinding::new("down", InboxDown, t),
        KeyBinding::new("enter", InboxEdit, t),
        KeyBinding::new("backspace", InboxDelete, t),
        KeyBinding::new("delete", InboxDelete, t),
        // ⌘↵ is the page's save everywhere, also inside the kit's fields
        // (bound after the kit, so it wins over their Submit at the same depth)
        KeyBinding::new("cmd-enter", Save, Some("NtexEdit")),
        KeyBinding::new("cmd-4", PageLibrary, s),
        KeyBinding::new("cmd-f", LibSearch, s),
        KeyBinding::new("up", LibUp, Some(LIBRARY_CONTEXT)),
        KeyBinding::new("down", LibDown, Some(LIBRARY_CONTEXT)),
        KeyBinding::new("enter", LibOpen, Some(LIBRARY_CONTEXT)),
        // in the search field (bound after the kit, so they win over its ↑/↓/Return)
        KeyBinding::new("up", LibUp, Some("LibrarySearch > TextField")),
        KeyBinding::new("down", LibDown, Some("LibrarySearch > TextField")),
        KeyBinding::new("enter", LibOpen, Some("LibrarySearch > TextField")),
    ]);
}

/// Run `f`, turning a panic into `None` (and a line on stderr). Core and io
/// are pure logic and processes written by other builders; a bug in one of
/// them must not take the window down mid-lecture.
pub fn safe<T>(what: &str, f: impl FnOnce() -> T) -> Option<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(v) => Some(v),
        Err(e) => {
            let msg = e
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| e.downcast_ref::<String>().cloned())
                .unwrap_or_default();
            eprintln!("ntex-studio: {what} failed: {msg}");
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Snippet,
    Block,
    Inbox,
    Library,
}

impl Page {
    pub fn from_name(s: &str) -> Option<Page> {
        match s {
            "snippet" => Some(Page::Snippet),
            "block" => Some(Page::Block),
            "inbox" => Some(Page::Inbox),
            "library" => Some(Page::Library),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum BridgeState {
    /// Started; no `list` answer yet.
    Starting,
    /// Answered `list` once: checks and Try it work.
    Ready,
    /// Could not start (nvim missing, …): the studio works without it.
    Failed(String),
}

/// One step back for ⌘Z outside a field.
#[derive(Clone, Debug)]
pub enum UndoItem {
    /// Files as they were before a save, delete, block or sort.
    Files { label: String, backup: PathBuf, files: Vec<PathBuf> },
    /// ⌘N cleared this snippet draft.
    ClearedDraft(Draft),
    /// ⌘N cleared this block draft.
    ClearedBlock(BlockDraft),
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub id: u64,
    pub text: SharedString,
    pub at: Instant,
    /// The page it belongs to: switching pages drops it.
    pub page: Page,
}

impl Toast {
    /// Full ink for `TOAST_FOR`, then a linear fade over `TOAST_FADE`.
    pub fn opacity(&self, now: Instant) -> f32 {
        let age = now.saturating_duration_since(self.at);
        if age <= TOAST_FOR {
            1.0
        } else {
            (1.0 - (age - TOAST_FOR).as_secs_f32() / TOAST_FADE.as_secs_f32()).clamp(0.0, 1.0)
        }
    }
}

/// A rendered preview (snippet output or block specimen): debounced, one
/// render in flight, stale results dropped, the PNG decoded off the UI
/// thread, faded in.
#[derive(Default)]
pub struct Preview {
    /// When to start the next render.
    pub due: Option<Instant>,
    /// Bumped by every edit.
    pub generation: u64,
    /// The generation the render in flight was made for.
    pending_gen: u64,
    rx: Option<Receiver<Result<Rendered, String>>>,
    decode_rx: Option<Receiver<Result<(Arc<RenderImage>, f32, f32), String>>>,
    decoding: Option<Rendered>,
    /// The image on screen and its size in points.
    pub image: Option<(Arc<RenderImage>, f32, f32)>,
    /// A TeX error ("! Missing $ inserted."), shown as one slate log line.
    pub error: Option<String>,
    pub shown_at: Option<Instant>,
    /// White → transparent (a snippet render sits on a tinted box).
    pub knock_out_white: bool,
    /// Trim the white page around the content (a specimen document).
    pub crop: bool,
}

impl Preview {
    /// `knock_out_white`: for a render on a tinted box; `crop`: for a page.
    pub fn new(knock_out_white: bool, crop: bool) -> Preview {
        Preview { knock_out_white, crop, ..Preview::default() }
    }

    pub fn schedule(&mut self, at: Instant) {
        self.generation += 1;
        self.due = Some(at);
    }

    pub fn clear(&mut self) {
        self.generation += 1;
        self.due = None;
        self.rx = None;
        self.decode_rx = None;
        self.image = None;
        self.error = None;
    }

    pub fn busy(&self) -> bool {
        self.due.is_some() || self.rx.is_some() || self.decode_rx.is_some()
    }

    /// Start a due render, collect a finished one. `job`: None = nothing to
    /// render (renders off, or nothing to show). Returns whether anything changed.
    pub fn pump(&mut self, now: Instant, renderer: Option<&Renderer>, job: impl FnOnce() -> Option<RenderJob>) -> bool {
        let mut changed = false;
        if self.due.is_some_and(|d| now >= d) {
            self.due = None;
            match (renderer, job()) {
                (Some(r), Some(job)) => {
                    self.rx = safe("render", || r.render(job));
                    self.pending_gen = self.generation;
                }
                _ => {
                    self.rx = None;
                    if self.image.is_some() || self.error.is_some() {
                        changed = true;
                    }
                    self.image = None;
                    self.error = None;
                }
            }
        }
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(res) => {
                    self.rx = None;
                    if self.pending_gen == self.generation || self.image.is_none() {
                        match res {
                            Ok(rendered) => {
                                self.decode_rx = Some(decode_png(rendered.png.clone(), self.knock_out_white, self.crop));
                                self.decoding = Some(rendered);
                            }
                            Err(e) => {
                                self.error = Some(e.lines().next().unwrap_or("").to_string());
                                self.image = None;
                                changed = true;
                            }
                        }
                    }
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.rx = None,
            }
        }
        if let Some(rx) = &self.decode_rx {
            match rx.try_recv() {
                Ok(res) => {
                    self.decode_rx = None;
                    let r = self.decoding.take();
                    match (res, r) {
                        (Ok((img, fx, fy)), Some(r)) => {
                            self.image = Some((img, r.width * fx, r.height * fy));
                            self.error = None;
                            self.shown_at = Some(now);
                        }
                        (Err(e), _) => self.error = Some(e),
                        _ => {}
                    }
                    changed = true;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.decode_rx = None,
            }
        }
        changed
    }

    /// 0…1 for the 150 ms fade-in.
    pub fn opacity(&self, now: Instant) -> f32 {
        match self.shown_at {
            Some(t) => (now.saturating_duration_since(t).as_secs_f32() / 0.150).clamp(0.0, 1.0),
            None => 1.0,
        }
    }
}

/// PNG → GPUI image (BGRA), on a thread. `knock_out`: white becomes
/// transparent (colour-to-alpha against white, so ink and tints composite
/// back exactly over white and sit naturally on a tinted box). `crop`: trim
/// the white page to the content plus a margin. Also returns the fraction
/// of the width and height kept, so the size in points can follow.
pub fn decode_png(path: PathBuf, knock_out: bool, crop: bool) -> Receiver<Result<(Arc<RenderImage>, f32, f32), String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let res = (|| -> Result<(Arc<RenderImage>, f32, f32), String> {
            let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let mut img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
                .map_err(|e| e.to_string())?
                .into_rgba8();
            let (w0, h0) = img.dimensions();
            let (mut fx, mut fy) = (1.0, 1.0);
            if crop {
                let ink = |p: &image::Rgba<u8>| p.0[3] > 0 && (p.0[0] < 245 || p.0[1] < 245 || p.0[2] < 245);
                let (mut x0, mut y0, mut x1, mut y1) = (w0, h0, 0, 0);
                for (x, y, p) in img.enumerate_pixels() {
                    if ink(p) {
                        x0 = x0.min(x);
                        y0 = y0.min(y);
                        x1 = x1.max(x);
                        y1 = y1.max(y);
                    }
                }
                if x1 > x0 && y1 > y0 {
                    let m = 24; // 12 pt at 2×
                    let (x0, y0) = (x0.saturating_sub(m), y0.saturating_sub(m));
                    let (x1, y1) = ((x1 + m).min(w0 - 1), (y1 + m).min(h0 - 1));
                    img = image::imageops::crop_imm(&img, x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image();
                    fx = img.width() as f32 / w0 as f32;
                    fy = img.height() as f32 / h0 as f32;
                }
            }
            for p in img.pixels_mut() {
                let [r, g, b, a] = p.0;
                if knock_out {
                    let alpha = (255 - r.min(g).min(b)) as f32 / 255.0;
                    if alpha <= 0.0 {
                        p.0 = [0, 0, 0, 0];
                        continue;
                    }
                    let un = |c: u8| (255.0 - (255.0 - c as f32) / alpha).clamp(0.0, 255.0) as u8;
                    let na = (alpha * a as f32).round() as u8;
                    // BGRA
                    p.0 = [un(b), un(g), un(r), na];
                } else {
                    p.0 = [b, g, r, a];
                }
            }
            Ok((Arc::new(RenderImage::new(vec![image::Frame::new(img)])), fx, fy))
        })();
        let _ = tx.send(res);
    });
    rx
}

/// Local time, `2026-10-07T14:32` (and with seconds for backup folders).
pub fn local_now() -> (String, String) {
    #[repr(C)]
    struct Tm {
        sec: i32,
        min: i32,
        hour: i32,
        mday: i32,
        mon: i32,
        year: i32,
        wday: i32,
        yday: i32,
        isdst: i32,
        gmtoff: std::ffi::c_long,
        zone: *mut std::ffi::c_char,
    }
    unsafe extern "C" {
        fn localtime_r(t: *const i64, out: *mut Tm) -> *mut Tm;
    }
    let secs = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let mut tm = Tm { sec: 0, min: 0, hour: 0, mday: 1, mon: 0, year: 70, wday: 0, yday: 0, isdst: 0, gmtoff: 0, zone: std::ptr::null_mut() };
    // SAFETY: localtime_r writes only into `tm`.
    unsafe { localtime_r(&secs, &mut tm) };
    let minute = format!("{:04}-{:02}-{:02}T{:02}:{:02}", tm.year + 1900, tm.mon + 1, tm.mday, tm.hour, tm.min);
    static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let stamp = format!("{}.{:02}-{}", minute.replace(':', "."), tm.sec, n);
    (minute, stamp)
}

pub struct StudioOptions {
    /// Tests: never hide, activate or quit the app (count requests instead).
    pub quiet: bool,
    /// Start the headless ntex bridge (tests of pure layout may skip it).
    pub bridge: bool,
}

impl Default for StudioOptions {
    fn default() -> Self {
        StudioOptions { quiet: false, bridge: true }
    }
}

pub struct Studio {
    pub paths: Paths,
    pub prefs: StudioPrefs,
    pub quiet: bool,
    pub page: Page,
    /// The page itself (nothing focused): page keys land here.
    pub focus: FocusHandle,
    pub scroll: ScrollHandle,
    pub bridge: Option<Bridge>,
    pub bridge_state: BridgeState,
    /// Every snippet loaded in ntex (empty until the bridge answers).
    pub known: Vec<KnownSnippet>,
    known_rx: Option<Receiver<Result<Vec<KnownSnippet>, String>>>,
    known_retry: Option<Instant>,
    pub renderer: Option<Renderer>,
    pub inbox: Vec<InboxEntry>,
    pub inbox_error: Option<String>,
    inbox_mtime: Option<SystemTime>,
    next_mtime_check: Option<Instant>,
    pub blocks: Vec<KnownBlock>,
    pub undo: Vec<UndoItem>,
    pub redo: Vec<UndoItem>,
    pub toast: Option<Toast>,
    pub snippet: SnippetPage,
    pub block: BlockPage,
    pub inbox_page: InboxPage,
    pub library: LibraryPage,
    pub open_rx: Option<Receiver<OpenRequest>>,
    /// Hide requests (quiet mode counts them instead of hiding).
    pub hides: u32,
    /// Frame changed: written to state/window.json on the next pump.
    bounds_dirty: Option<Bounds<Pixels>>,
    _poll: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl Studio {
    pub fn new(opts: StudioOptions, window: &mut Window, cx: &mut Context<Studio>) -> Studio {
        // the English word list (≈150 ms in debug) loads off the UI thread, so
        // the first keystroke doesn't pay for it
        crate::core::words::preload_in_background();
        let paths = Paths::from_env();
        let prefs = safe("prefs::read", || crate::core::prefs::read(&paths)).unwrap_or_default();
        crate::ui::widgets::theme::set_typeface(&prefs.typeface, cx);
        let focus = cx.focus_handle();
        let mut subs = Vec::new();
        let snippet = SnippetPage::new(window, cx, &mut subs);
        let block = BlockPage::new(window, cx, &mut subs);
        let inbox_page = InboxPage::new(cx);
        let library = LibraryPage::new(window, cx, &mut subs);
        let renderer = safe("Renderer::new", || Renderer::new(&paths));
        let mut studio = Studio {
            paths,
            prefs,
            quiet: opts.quiet,
            page: Page::Snippet,
            focus,
            scroll: ScrollHandle::new(),
            bridge: None,
            bridge_state: BridgeState::Starting,
            known: Vec::new(),
            known_rx: None,
            known_retry: None,
            renderer,
            inbox: Vec::new(),
            inbox_error: None,
            inbox_mtime: None,
            next_mtime_check: None,
            blocks: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            toast: None,
            snippet,
            block,
            inbox_page,
            library,
            open_rx: None,
            hides: 0,
            bounds_dirty: None,
            _poll: None,
            _subs: subs,
        };
        studio.reload_inbox();
        studio.reload_blocks();
        if opts.bridge {
            studio.start_bridge();
        } else {
            studio.bridge_state = BridgeState::Failed("the bridge is off".into());
        }
        studio.snippet_changed(cx);
        studio.block_changed(cx);
        // the poll loop: drains every receiver, runs debounces, the toast, the mtime watch
        studio._poll = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                if this.update_in(cx, |s, window, cx| s.pump(window, cx)).is_err() {
                    break;
                }
            }
        }));
        studio.snippet.trigger.read(cx).focus.clone().focus(window, cx);
        studio
    }

    pub fn now(&self, cx: &App) -> Instant {
        cx.background_executor().now()
    }

    // -- the bridge --------------------------------------------------------

    fn start_bridge(&mut self) {
        match safe("Bridge::start", || Bridge::start(&self.paths)) {
            Some(Ok(b)) => {
                self.bridge = Some(b);
                self.bridge_state = BridgeState::Starting;
                self.refresh_known();
            }
            Some(Err(e)) => self.bridge_state = BridgeState::Failed(e.to_string()),
            None => self.bridge_state = BridgeState::Failed("the bridge is not built yet".into()),
        }
    }

    pub fn bridge_ready(&self) -> bool {
        self.bridge_state == BridgeState::Ready
    }

    /// Ask the bridge for every loaded snippet (after a save, a sort, a hand edit).
    pub fn refresh_known(&mut self) {
        if let Some(b) = &self.bridge {
            self.known_rx = safe("Bridge::list", || b.list());
        }
    }

    /// The snippet files changed on disk: re-copy them into the bridge, re-list.
    pub fn files_changed(&mut self) {
        if let Some(b) = &self.bridge {
            safe("Bridge::refresh", || b.refresh());
        }
        self.refresh_known();
    }

    pub fn stop_bridge(&mut self) {
        if let Some(mut b) = self.bridge.take() {
            safe("Bridge::stop", || b.stop());
        }
        self.inbox_page.cancel_job();
    }

    // -- files -------------------------------------------------------------

    pub fn reload_inbox(&mut self) {
        self.inbox_mtime = mtime(&self.paths.inbox());
        match safe("inbox::read", || crate::core::inbox::read(&self.paths)) {
            Some(Ok(entries)) => {
                self.inbox = entries;
                self.inbox_error = None;
            }
            Some(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                self.inbox.clear();
                self.inbox_error = None;
            }
            Some(Err(e)) => self.inbox_error = Some(e.to_string()),
            None => self.inbox_error = Some("The inbox reader is not built yet.".into()),
        }
        let n = self.inbox.len();
        if self.inbox_page.row >= n {
            self.inbox_page.row = n.saturating_sub(1);
        }
    }

    pub fn reload_blocks(&mut self) {
        match safe("blocks::read", || crate::core::blocks::read(&self.paths)) {
            Some(Ok(b)) => self.blocks = b,
            Some(Err(_)) | None => self.blocks.clear(),
        }
    }

    // -- the poll loop -----------------------------------------------------

    /// Drain every receiver and run what is due. Never blocks. The poll loop
    /// calls it every 16 ms; tests call it directly.
    pub fn pump(&mut self, window: &mut Window, cx: &mut Context<Studio>) {
        let now = self.now(cx);
        let mut changed = false;

        // the bridge's list of known snippets
        if let Some(rx) = &self.known_rx {
            match rx.try_recv() {
                Ok(Ok(list)) => {
                    self.known = list;
                    self.known_rx = None;
                    self.known_retry = None;
                    self.bridge_state = BridgeState::Ready;
                    self.snippet_rechecked(cx);
                    self.block_rechecked(cx);
                    changed = true;
                }
                Ok(Err(e)) => {
                    self.known_rx = None;
                    if self.bridge_state != BridgeState::Ready {
                        // not up yet: ask again in a moment
                        self.known_retry = Some(now + Duration::from_secs(1));
                    }
                    let _ = e;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.known_rx = None;
                    if self.bridge_state != BridgeState::Ready {
                        self.known_retry = Some(now + Duration::from_secs(2));
                    }
                }
            }
        }
        if self.known_retry.is_some_and(|t| now >= t) {
            self.known_retry = None;
            self.refresh_known();
        }

        // later invocations (Space n)
        let mut requests = Vec::new();
        if let Some(rx) = &self.open_rx {
            while let Ok(req) = rx.try_recv() {
                requests.push(req);
            }
        }
        for req in requests {
            self.open_request(req, window, cx);
            changed = true;
        }

        // the inbox on disk (hand edits, sorting, ntex)
        if self.next_mtime_check.is_none_or(|t| now >= t) {
            self.next_mtime_check = Some(now + MTIME_EVERY);
            let m = mtime(&self.paths.inbox());
            if m != self.inbox_mtime {
                self.reload_inbox();
                self.files_changed();
                changed = true;
            }
        }

        changed |= self.pump_snippet(now, window, cx);
        changed |= self.pump_block(now, cx);
        changed |= self.pump_inbox(now, cx);
        changed |= self.pump_library(now, cx);

        if let Some(t) = &self.toast {
            let age = now.saturating_duration_since(t.at);
            if age >= TOAST_FOR + TOAST_FADE {
                self.toast = None;
                changed = true;
            } else if age >= TOAST_FOR {
                changed = true; // fading
            }
        }

        if let Some(b) = self.bounds_dirty.take() {
            save_frame(&self.paths, b);
        }

        if changed {
            cx.notify();
        }
    }

    // -- shared behaviour ----------------------------------------------------

    pub fn toast(&mut self, text: impl Into<SharedString>, cx: &mut Context<Studio>) {
        let id = self.toast.as_ref().map_or(0, |t| t.id + 1);
        self.toast = Some(Toast { id, text: text.into(), at: self.now(cx), page: self.page });
        cx.notify();
    }

    /// Back up `files` (for ⌘Z) before changing them.
    pub fn backup(&mut self, label: &str, files: Vec<PathBuf>) -> Result<(), String> {
        let (_, stamp) = local_now();
        match safe("apply::backup", || crate::core::apply::backup(&self.paths, &files, &stamp)) {
            Some(Ok(backup)) => {
                self.undo.push(UndoItem::Files { label: label.to_string(), backup, files });
                self.redo.clear();
                Ok(())
            }
            Some(Err(e)) => Err(e.to_string()),
            None => Err("backups are not built yet".into()),
        }
    }

    pub fn hide(&mut self, cx: &mut Context<Studio>) {
        self.hides += 1;
        if !self.quiet {
            cx.hide();
        }
    }

    pub fn set_page(&mut self, page: Page, window: &mut Window, cx: &mut Context<Studio>) {
        if self.page != page {
            self.page = page;
            self.scroll.set_offset(point(px(0.), px(0.)));
            // a toast is about the page it was made on
            if self.toast.as_ref().is_some_and(|t| t.page != page) {
                self.toast = None;
            }
        }
        match page {
            Page::Snippet => self.snippet.trigger.read(cx).focus.clone().focus(window, cx),
            Page::Block => self.block.title.read(cx).focus.clone().focus(window, cx),
            Page::Inbox => self.inbox_page.focus.clone().focus(window, cx),
            Page::Library => {
                if self.library.lib.entries.is_empty() || self.library.seen.is_empty() {
                    self.reload_library(cx);
                }
                self.library.search.read(cx).focus.clone().focus(window, cx);
            }
        }
        cx.notify();
    }

    /// Space n / a second `ntex-studio`: page, prefill, focus.
    pub fn open_request(&mut self, mut req: OpenRequest, window: &mut Window, cx: &mut Context<Studio>) {
        // FIG-UI: `--figure PATH` opens (or focuses) that figure's own window;
        // a request with nothing else leaves the studio's window alone
        if let Some(fig) = req.figure.take().filter(|f| !f.is_empty()) {
            let mode = crate::ui::launch::Mode::of(self.quiet, req.background);
            cx.defer(move |cx| {
                crate::ui::figure::open_mode(std::path::Path::new(&fig), mode, cx);
            });
            if req.page.is_none() && req.output.is_none() && req.context.is_none() && req.ft.is_none() {
                return;
            }
        }
        let page = req.page.as_deref().and_then(Page::from_name);
        let place = match req.context.as_deref() {
            Some("math") => Some(SnipContext::Math),
            Some("text") => Some(SnipContext::Text),
            _ => None,
        };
        if req.output.is_some() || place.is_some() || req.ft.is_some() {
            self.snippet.place = place.or(self.snippet.place);
            if let Some(ft) = &req.ft {
                self.snippet.open_ft = Some(ft.clone());
            }
        }
        let prefill = req.output.is_some();
        if let Some(text) = req.output.filter(|t| !t.is_empty()) {
            let mut d = Draft::default();
            d.output = vec![Piece::Text(text)];
            self.load_draft(d, false, window, cx);
        } else if place.is_some() {
            self.snippet_changed(cx);
        }
        self.set_page(page.unwrap_or(if prefill { Page::Snippet } else { self.page }), window, cx);
        let plan = crate::ui::launch::plan(crate::ui::launch::Mode::of(self.quiet, req.background), true);
        if plan.activate {
            // a studio launched in the background becomes a normal app now
            crate::ui::launch::release_background();
            cx.activate(true);
            window.activate_window();
        }
    }

    pub fn undo_last(&mut self, window: &mut Window, cx: &mut Context<Studio>) {
        let Some(item) = self.undo.pop() else {
            self.toast("Nothing to undo.", cx);
            return;
        };
        match item {
            UndoItem::Files { label, backup, files } => {
                // keep the current state for ⌘⇧Z
                let (_, stamp) = local_now();
                let redo = safe("apply::backup", || crate::core::apply::backup(&self.paths, &files, &stamp))
                    .and_then(|r| r.ok());
                match safe("apply::undo", || crate::core::apply::undo(&backup)) {
                    Some(Ok(())) => {
                        if let Some(r) = redo {
                            self.redo.push(UndoItem::Files { label: label.clone(), backup: r, files });
                        }
                        self.after_files_changed(cx);
                        self.inbox_page.undone(&label);
                        self.toast(format!("Undid {label}."), cx);
                    }
                    Some(Err(e)) => self.toast(format!("! Could not undo {label}: {e}"), cx),
                    None => self.toast(format!("! Could not undo {label}."), cx),
                }
            }
            UndoItem::ClearedDraft(d) => {
                self.load_draft(d, false, window, cx);
                self.set_page(Page::Snippet, window, cx);
                self.toast("Restored the cleared snippet.", cx);
            }
            UndoItem::ClearedBlock(b) => {
                self.load_block(b, window, cx);
                self.set_page(Page::Block, window, cx);
                self.toast("Restored the cleared block.", cx);
            }
        }
    }

    pub fn redo_last(&mut self, cx: &mut Context<Studio>) {
        let Some(UndoItem::Files { label, backup, files }) = self.redo.pop() else {
            return;
        };
        let (_, stamp) = local_now();
        let back = safe("apply::backup", || crate::core::apply::backup(&self.paths, &files, &stamp)).and_then(|r| r.ok());
        if let Some(Ok(())) = safe("apply::undo", || crate::core::apply::undo(&backup)) {
            if let Some(b) = back {
                self.undo.push(UndoItem::Files { label: label.clone(), backup: b, files });
            }
            self.after_files_changed(cx);
            self.toast(format!("Redid {label}."), cx);
        }
    }

    /// The inbox / blocks changed on disk because of us.
    pub fn after_files_changed(&mut self, cx: &mut Context<Studio>) {
        self.reload_inbox();
        self.reload_blocks();
        self.files_changed();
        if self.page == Page::Library || !self.library.lib.entries.is_empty() {
            self.reload_library(cx);
        }
        self.snippet_rechecked(cx);
        cx.notify();
    }

    /// Footnotes for whatever has the keyboard.
    pub fn hints(&self, window: &Window, cx: &App) -> Vec<SharedString> {
        match self.page {
            Page::Snippet => self.snippet_hints(window, cx),
            Page::Block => self.block_hints(window, cx),
            Page::Inbox => self.inbox_hints(window, cx),
            Page::Library => self.library_hints(window, cx),
        }
    }

    // -- frame -------------------------------------------------------------

    pub fn watch_frame(&mut self, window: &mut Window, cx: &mut Context<Studio>) {
        let sub = cx.observe_window_bounds(window, |s, window, _| {
            if let WindowBounds::Windowed(b) = window.window_bounds() {
                s.bounds_dirty = Some(b);
            }
        });
        self._subs.push(sub);
    }
}

fn mtime(p: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

impl Focusable for Studio {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Studio {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.page {
            Page::Snippet => self.render_snippet(window, cx).into_any_element(),
            Page::Block => self.render_block(window, cx).into_any_element(),
            Page::Inbox => self.render_inbox(window, cx).into_any_element(),
            Page::Library => self.render_library(window, cx).into_any_element(),
        };
        let page = shell::frame(self, body, window, cx);
        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .size_full()
            .on_action(cx.listener(|s, _: &PageSnippet, w, cx| s.set_page(Page::Snippet, w, cx)))
            .on_action(cx.listener(|s, _: &PageBlock, w, cx| s.set_page(Page::Block, w, cx)))
            .on_action(cx.listener(|s, _: &PageInbox, w, cx| s.set_page(Page::Inbox, w, cx)))
            .on_action(cx.listener(|s, _: &PageLibrary, w, cx| s.set_page(Page::Library, w, cx)))
            .on_action(cx.listener(|s, _: &LibSearch, w, cx| {
                s.set_page(Page::Library, w, cx);
                s.library.search.update(cx, |f, cx| f.select_all(cx));
            }))
            .on_action(cx.listener(|s, _: &Save, w, cx| match s.page {
                Page::Snippet => s.save_snippet(w, cx),
                Page::Block => s.save_block(w, cx),
                Page::Inbox | Page::Library => {}
            }))
            .on_action(cx.listener(|s, _: &Clear, w, cx| match s.page {
                Page::Snippet => s.clear_snippet(w, cx),
                Page::Block => s.clear_block(w, cx),
                Page::Inbox | Page::Library => {}
            }))
            .on_action(cx.listener(|s, _: &Undo, w, cx| s.undo_last(w, cx)))
            .on_action(cx.listener(|s, _: &Redo, _, cx| s.redo_last(cx)))
            .on_action(cx.listener(|s, _: &Sort, w, cx| {
                s.set_page(Page::Inbox, w, cx);
                s.start_sort(cx)
            }))
            .on_action(cx.listener(|s, _: &Hide, _, cx| s.hide(cx)))
            .on_action(cx.listener(|s, _: &Escape, w, cx| s.escape(w, cx)))
            .on_action(cx.listener(|s, _: &Quit, _, cx| {
                s.stop_bridge();
                if !s.quiet {
                    cx.quit();
                }
            }))
            .on_action(cx.listener(|_, _: &FocusNext, w, cx| w.focus_next(cx)))
            .on_action(cx.listener(|_, _: &FocusPrev, w, cx| w.focus_prev(cx)))
            .child(page)
    }
}

impl Studio {
    /// Esc: on an empty draft hide the window (the app keeps running);
    /// otherwise leave the field (the draft stays).
    pub fn escape(&mut self, window: &mut Window, cx: &mut Context<Studio>) {
        let empty = match self.page {
            Page::Snippet => self.snippet_is_empty(),
            Page::Block => self.block_is_empty(),
            Page::Inbox | Page::Library => true,
        };
        let in_field = !self.focus.is_focused(window);
        // editing a Library snippet: Esc leaves the field it is in, then goes
        // back to the list (a field that isn't drawn doesn't count)
        if self.page == Page::Snippet && self.snippet.home.is_some() {
            let p = &self.snippet;
            let raw = p.draft.kind == Kind::RawLua;
            let mut fields = vec![p.trigger.read(cx).focus.clone(), p.priority.read(cx).focus.clone(), p.desc.read(cx).focus.clone()];
            fields.push(p.eg.read(cx).focus.clone());
            fields.push(p.check.read(cx).focus.clone());
            fields.push(if raw { p.raw.read(cx).focus.clone() } else { p.output.read(cx).focus.clone() });
            if fields.iter().any(|f| f.is_focused(window)) {
                self.focus.clone().focus(window, cx);
                cx.notify();
            } else {
                self.cancel_home(window, cx);
            }
            return;
        }
        if empty || !in_field {
            self.hide(cx);
        } else {
            self.focus.clone().focus(window, cx);
            cx.notify();
        }
    }
}

// ---------------------------------------------------------------------------
// the window (main.rs) and later requests

struct StudioWindow(WindowHandle<Studio>);
impl Global for StudioWindow {}

#[derive(serde::Serialize, serde::Deserialize)]
struct Frame {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

fn frame_file(paths: &Paths) -> PathBuf {
    paths.state.join("window.json")
}

fn save_frame(paths: &Paths, b: Bounds<Pixels>) {
    let f = Frame { x: b.origin.x.into(), y: b.origin.y.into(), w: b.size.width.into(), h: b.size.height.into() };
    let _ = std::fs::create_dir_all(&paths.state);
    if let Ok(s) = serde_json::to_string(&f) {
        let tmp = frame_file(paths).with_extension("json.tmp");
        if std::fs::write(&tmp, s).is_ok() {
            let _ = std::fs::rename(tmp, frame_file(paths));
        }
    }
}

fn load_frame(paths: &Paths) -> Option<Bounds<Pixels>> {
    let s = std::fs::read_to_string(frame_file(paths)).ok()?;
    let f: Frame = serde_json::from_str(&s).ok()?;
    (f.w >= 620.0 && f.h >= 560.0).then(|| Bounds::new(point(px(f.x), px(f.y)), size(px(f.w), px(f.h))))
}

/// When the process started (main.rs sets it), for first-frame timings.
pub static STARTED: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// The studio's one window (UX.md: 860 × 960, min 620 × 560, transparent
/// title bar, frame remembered in state/window.json).
pub fn open_window(cx: &mut App, first: OpenRequest, requests: Option<Receiver<OpenRequest>>) -> Option<WindowHandle<Studio>> {
    open_window_with(cx, first, requests, false)
}

/// `quiet` (NTEX_STUDIO_QUIET): the window is created hidden and unfocused,
/// and the studio never activates, hides or quits the app by itself.
pub fn open_window_with(
    cx: &mut App,
    first: OpenRequest,
    requests: Option<Receiver<OpenRequest>>,
    quiet: bool,
) -> Option<WindowHandle<Studio>> {
    // FIG-UI: launched for a figure only, the studio's own window starts
    // hidden (the next Space n shows it)
    let figure_only = first.figure.is_some() && first.page.is_none() && first.output.is_none();
    let plan = crate::ui::launch::plan(crate::ui::launch::Mode::of(quiet, first.background), !figure_only);
    let paths = Paths::from_env();
    let bounds = load_frame(&paths).unwrap_or_else(|| Bounds::centered(None, size(px(860.), px(960.)), cx));
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("Ntex Studio".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(14.), px(14.))),
        }),
        window_min_size: Some(size(px(620.), px(560.))),
        focus: plan.focus,
        show: plan.show,
        ..Default::default()
    };
    let handle = cx
        .open_window(options, |window, cx| {
            cx.new(|cx| {
                let t_new = Instant::now();
                let mut s = Studio::new(StudioOptions { quiet, bridge: true }, window, cx);
                if quiet {
                    eprintln!("ntex-studio: Studio::new {:.1} ms", t_new.elapsed().as_secs_f64() * 1000.0);
                }
                s.open_rx = requests;
                s.watch_frame(window, cx);
                crate::ui::launch::release_on_activation(window, cx);
                s
            })
        })
        .ok()?;
    handle
        .update(cx, |s, window, cx| {
            // the red button hides: the app keeps running, the next Space n is instant
            window.on_window_should_close(cx, |_, cx| {
                cx.hide();
                false
            });
            s.open_request(first, window, cx);
        })
        .ok();
    if plan.order_back {
        crate::ui::launch::order_back_if_inactive();
    }
    cx.set_global(StudioWindow(handle));
    cx.on_app_quit(|cx| {
        if let Some(w) = cx.try_global::<StudioWindow>().map(|w| w.0) {
            let _ = w.update(cx, |s, _, _| s.stop_bridge());
        }
        async {}
    })
    .detach();
    Some(handle)
}

/// Forward a request to the open studio (if any).
pub fn open_request(req: OpenRequest, cx: &mut App) {
    if let Some(w) = cx.try_global::<StudioWindow>().map(|w| w.0) {
        let _ = w.update(cx, |s, window, cx| s.open_request(req, window, cx));
    }
}
