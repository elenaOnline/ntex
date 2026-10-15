//! The figure window's root view (FIGURES.md "The figure window").
//!
//! One [`FigureView`] per `.fig`. It owns the document ([`Doc`]), the
//! canvas state (tool, view, selection, gesture), the shorthand box (a
//! two-way mirror of the source), the Ask box, the exact render, saving and
//! the watch on the file.
//!
//! As in the studio, nothing slow runs on the UI thread: the render
//! (tectonic), the PDF build and the Ask job report through mpsc receivers
//! that [`FigureView::pump`] drains every 16 ms; debounces use the
//! executor's clock, so the offscreen tests drive time with `advance_clock`.
use super::ask::{self, AskState};
use super::canvas::{self, Exact, Frame, Handlers, Overlay};
use super::doc::{self, Doc, core};
use super::geom::{self, View};
use super::tools::{Gesture, Tool};
use crate::model::Paths;
use crate::ui::app::{TOAST_FADE, TOAST_FOR, decode_png, local_now};
use crate::ui::screens::shell::{MARGIN, TITLEBAR};
use crate::ui::widgets::boxes::{BoxKind, Footnotes, LogLine, MarginNote, TBox, caps_label};
use crate::ui::widgets::controls::{Button, Checkbox, Keycap, Visual};
use crate::ui::widgets::text_field::{TextField, TextFieldEvent};
use crate::ui::widgets::snippet_field::{SnippetField, SnippetFieldEvent, Session};
use crate::ui::widgets::theme::{ActiveTheme as _, Theme};
use gpui::{
    AnyElement, App, AppContext as _, Bounds, Context, Entity, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, Pixels, Render, RenderImage, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, Window, actions, deferred, div,
    prelude::FluentBuilder as _, px,
};
use ntex_figures::model::*;
use ntex_studio_io::figure_ai::AskJob;
use super::exact::{ExactRenderer, Raster};
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant, SystemTime};

actions!(
    figure,
    [
        ToolSelect,
        ToolNode,
        ToolArrow,
        ToolBlob,
        ToolPolygon,
        ToolRough,
        /// ⌘T
        Tidy,
        /// ⌫ on the canvas
        DeleteSelection,
        /// ⌘Z outside a text field
        FigUndo,
        /// ⌘⇧Z outside a text field
        FigRedo,
        /// ⌘S
        FigSave,
        /// ⌘W
        SaveClose,
        ZoomIn,
        ZoomOut,
        ZoomFit,
        /// Esc on the canvas
        CanvasEscape,
        /// ⌘A on the canvas
        SelectAll,
        /// ⌘D
        Duplicate,
        /// ⌘C, ⌘X, ⌘V on the canvas: the objects as `.fig` shorthand on the clipboard
        CopySel,
        CutSel,
        PasteSel,
        /// ⌘R: reverse the selected arrows (a rotation arrow: its direction)
        Reverse,
        /// ⌘I: the properties panel
        TogglePanel,
        /// ⌘/: the shorthand's reference
        ToggleReference,
        /// Tab in a label field: the next vertex's / element's label
        NextLabel,
        /// ⇧Tab in a label field
        PrevLabel,
    ]
);

/// The key context of a label field (in place, or the panel's per-vertex
/// and per-element fields): Tab walks the labels.
pub const LABEL_CONTEXT: &str = "FigureLabelField";

pub const KEY_CONTEXT: &str = "Figure";
pub const CANVAS_CONTEXT: &str = "FigureCanvas";

const POLL: Duration = Duration::from_millis(16);
/// The shorthand is re-read this long after the last keystroke.
pub const REPARSE_AFTER: Duration = Duration::from_millis(150);
/// The exact render starts this long after the last change.
pub const RENDER_AFTER: Duration = Duration::from_millis(350);
/// Autosave (and the PDF build) this long after the last edit.
pub const AUTOSAVE_AFTER: Duration = Duration::from_secs(1);
/// The file on disk is checked this often.
const DISK_EVERY: Duration = Duration::from_millis(500);
const UNDO_MAX: usize = 200;

pub fn bind_keys(cx: &mut App) {
    use gpui::KeyBinding;
    let f = Some(KEY_CONTEXT);
    let c = Some(CANVAS_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("v", ToolSelect, c),
        KeyBinding::new("n", ToolNode, c),
        KeyBinding::new("a", ToolArrow, c),
        KeyBinding::new("b", ToolBlob, c),
        KeyBinding::new("p", ToolPolygon, c),
        KeyBinding::new("r", ToolRough, c),
        KeyBinding::new("backspace", DeleteSelection, c),
        KeyBinding::new("delete", DeleteSelection, c),
        KeyBinding::new("escape", CanvasEscape, c),
        KeyBinding::new("cmd-a", SelectAll, c),
        KeyBinding::new("cmd-d", Duplicate, c),
        KeyBinding::new("cmd-c", CopySel, c),
        KeyBinding::new("cmd-x", CutSel, c),
        KeyBinding::new("cmd-v", PasteSel, c),
        KeyBinding::new("cmd-r", Reverse, c),
        KeyBinding::new("cmd-i", TogglePanel, f),
        KeyBinding::new("cmd-/", ToggleReference, f),
        KeyBinding::new("tab", NextLabel, Some(LABEL_CONTEXT)),
        KeyBinding::new("shift-tab", PrevLabel, Some(LABEL_CONTEXT)),
        KeyBinding::new("cmd-t", Tidy, f),
        KeyBinding::new("cmd-z", FigUndo, f),
        KeyBinding::new("cmd-shift-z", FigRedo, f),
        KeyBinding::new("cmd-s", FigSave, f),
        KeyBinding::new("cmd-w", SaveClose, f),
        KeyBinding::new("cmd-=", ZoomIn, f),
        KeyBinding::new("cmd-+", ZoomIn, f),
        KeyBinding::new("cmd-shift-=", ZoomIn, f),
        KeyBinding::new("cmd--", ZoomOut, f),
        KeyBinding::new("cmd-0", ZoomFit, f),
    ]);
}

pub struct FigureOptions {
    pub path: PathBuf,
    /// Tests and the smoke mode: never activate, never close the app.
    pub quiet: bool,
    /// Start exact renders (tectonic). Tests of the draft may turn it off.
    pub render: bool,
    /// Build the PDF on save.
    pub build: bool,
}

/// A label being edited in place.
#[derive(Clone, Debug)]
pub struct LabelEdit {
    pub obj: ObjRef,
    /// Where the label is (cm, centre); the field sits beside it.
    pub at: Pt,
    pub before: String,
    /// Was the label maths when the field opened? (the `text` chip flips it)
    pub before_math: bool,
    /// An arrow's label: the middle of its line, the field goes across it.
    pub away: Option<Pt>,
}

#[derive(Clone, Debug)]
pub struct FigToast {
    pub id: u64,
    pub text: SharedString,
    pub at: Instant,
}

impl FigToast {
    pub fn opacity(&self, now: Instant) -> f32 {
        let age = now.saturating_duration_since(self.at);
        if age <= TOAST_FOR { 1.0 } else { (1.0 - (age - TOAST_FOR).as_secs_f32() / TOAST_FADE.as_secs_f32()).clamp(0.0, 1.0) }
    }
}

/// The exact render: debounced, one in flight, stale results dropped.
#[derive(Default)]
pub struct RenderState {
    pub renderer: Option<ExactRenderer>,
    pub due: Option<Instant>,
    rx: Option<(u64, Rect, Receiver<Result<Raster, String>>)>,
    decode: Option<(u64, Rect, Raster, Receiver<Result<(Arc<RenderImage>, f32, f32), String>>)>,
    /// (generation, the image)
    pub exact: Option<(u64, Exact)>,
    /// Pixels per pt of the image on screen (a zoom past it renders again).
    pub scale: f64,
    /// The TeX error of the last render ("! Undefined control sequence.").
    pub error: Option<String>,
    /// How many renders finished (tests).
    pub finished: u32,
}

impl RenderState {
    pub fn busy(&self) -> bool {
        self.due.is_some() || self.rx.is_some() || self.decode.is_some()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum BuildStatus {
    Idle,
    Building,
    Built(String),
    Failed(String),
}

/// The file on disk.
pub struct DiskState {
    /// The text on disk as far as we know (what we read or wrote last).
    pub text: Option<String>,
    pub mtime: Option<SystemTime>,
    /// "14:02"
    pub saved_at: Option<String>,
    pub autosave_due: Option<Instant>,
    next_check: Option<Instant>,
    /// Changed on disk while we had unsaved edits: their text (ask).
    pub conflict: Option<String>,
    pub write_error: Option<String>,
    build_rx: Option<Receiver<Option<Result<ntex_figures::build::Built, String>>>>,
    pub build: BuildStatus,
    /// Saves done (tests).
    pub saves: u32,
}

pub struct FigureView {
    pub path: PathBuf,
    pub paths: Paths,
    pub quiet: bool,
    pub build_on_save: bool,
    pub doc: Doc,
    pub tool: Tool,
    pub view: View,
    pub selection: Vec<ObjRef>,
    pub gesture: Option<Gesture>,
    /// The draft while dragging (the moved figure, laid out).
    pub drag_scene: Option<Arc<Scene>>,
    pub drag_figure: Option<Figure>,
    pub hover: Option<ObjRef>,
    /// The recognised shape and when it was made (the 150 ms flash).
    pub ghost: Option<(Vec<Vec<Pt>>, Instant)>,
    pub last_ngon: Option<String>,
    pub label_edit: Option<LabelEdit>,
    /// (class C) The press began on the label of the selected object: a
    /// release without a drag edits it.
    pub click_edit: Option<ObjRef>,
    /// (figure-qol A) The clipboard's bookkeeping: the last copy drag's offset, what this window copied or pasted.
    pub clip: super::interact::ClipState,
    pub focus: FocusHandle,
    pub canvas_focus: FocusHandle,
    pub shorthand: Entity<TextField>,
    pub ask_field: Entity<TextField>,
    pub label_field: Entity<SnippetField>,
    pub label_session: Session,
    pub label_preview: Option<Arc<Scene>>,
    pub ask: AskState,
    pub reparse_due: Option<Instant>,
    pub render: RenderState,
    pub undo: Vec<String>,
    pub redo: Vec<String>,
    pub disk: DiskState,
    pub toast: Option<FigToast>,
    /// ⌘W in quiet mode (tests count instead of closing).
    pub closes: u32,
    pub canvas_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// The canvas bounds the view was last drawn for: when they change (the
    /// panel opens, a log line appears) the figure stays where it is on screen.
    pub last_canvas: Option<Bounds<Pixels>>,
    // -- direct manipulation (interact.rs, handles.rs, inspector.rs, menu.rs)
    /// The run of changes the last undo step covers (typing in one panel
    /// field, a slider drag, held arrow keys): the same group again adds no step.
    pub undo_group: Option<String>,
    /// ⌘I: Some(shown) once she has toggled it; None = by the window's width.
    pub panel_open: Option<bool>,
    /// ⌘/: the Reference panel.
    pub reference_open: bool,
    pub menu: Option<super::menu::Menu>,
    /// The panel's text fields, by what they edit.
    pub fields: std::collections::HashMap<String, super::inspector::PanelField>,
    /// Where named controls were drawn (the offscreen tests click there).
    pub ui_bounds: Rc<std::cell::RefCell<std::collections::HashMap<String, Bounds<Pixels>>>>,
    /// The shorthand box's scroll (opening scrolls to the first line that isn't a comment).
    pub shorthand_scroll: gpui::ScrollHandle,
    pub scroll_to_code: bool,
    /// The live readout of a handle drag: (where, "45°").
    pub readout: Option<(Pt, String)>,
    /// The handle under the pointer (the cursor shows what it does).
    pub hover_handle: Option<super::handles::HKind>,
    /// The panel's label fields in Tab order (rebuilt every frame).
    pub tab_order: Vec<String>,
    /// The Insert ▸ Cayley graph ▸ Z_n field.
    pub zn_field: Entity<TextField>,
    /// Elements made by “+ element” with a placeholder name: the first
    /// label she gives one becomes its name too.
    pub fresh_elements: Vec<String>,
    /// Fit the figure to the canvas (capped at 200 %) as soon as the canvas
    /// has bounds: set on open and on a reload, never by her own zooming.
    pub fit_pending: bool,
    /// Set when the window closes or the studio quits: a PDF build in flight
    /// kills its tectonic (no tectonic outlives the studio).
    pub build_cancel: Arc<AtomicBool>,
    _poll: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

/// The starting file for a new figure: the shared `figures/template.fig`
/// (also `ntex-fig new`), embedded at build time (build.rs).
pub const TEMPLATE: &str = ntex_figures::build::TEMPLATE;

/// The starting file for this `.fig` (NAME filled in, as `ntex-fig new` does).
pub fn template_for_path(path: &std::path::Path) -> String {
    ntex_figures::build::template_for(&ntex_figures::build::figure_name(path))
}

fn hhmm() -> String {
    let (minute, _) = local_now();
    minute.split('T').nth(1).unwrap_or(&minute).to_string()
}

fn mtime(p: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// Write atomically (temp file + rename), so Neovim never reads half a file.
fn write_atomic(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension(format!("fig.{}.tmp", std::process::id()));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

impl FigureView {
    pub fn new(opts: FigureOptions, window: &mut Window, cx: &mut Context<FigureView>) -> FigureView {
        doc::quiet_todo_panics();
        let paths = Paths::from_env();
        // a missing file starts from the template (and is written at once)
        let (text, on_disk) = match std::fs::read_to_string(&opts.path) {
            Ok(t) => (t, true),
            Err(_) => (template_for_path(&opts.path), false),
        };
        let shorthand = cx.new(|cx| TextField::new(cx).mono().multiline().placeholder("A -> B"));
        let ask_field = cx.new(|cx| TextField::new(cx).placeholder("say what to change: “make the square commute with a dashed diagonal”"));
        let label_session = Session::new(paths.clone());
        let label_field = cx.new(|cx| SnippetField::new(cx).proportional().with_session(label_session.clone()));
        let zn_field = cx.new(|cx| TextField::new(cx).mono().width(px(40.)).placeholder("6"));
        let mut subs = Vec::new();
        subs.push(cx.subscribe_in(&shorthand, window, |v: &mut FigureView, _, ev: &TextFieldEvent, window, cx| match ev {
            TextFieldEvent::Changed => {
                v.reparse_due = Some(v.now(cx) + REPARSE_AFTER);
                cx.notify();
            }
            TextFieldEvent::Cancel => {
                v.canvas_focus.clone().focus(window, cx);
                cx.notify();
            }
            TextFieldEvent::Submit | TextFieldEvent::Blur => {}
        }));
        subs.push(cx.subscribe_in(&ask_field, window, |v: &mut FigureView, _, ev: &TextFieldEvent, window, cx| match ev {
            TextFieldEvent::Submit => v.ask_submit(window, cx),
            TextFieldEvent::Cancel => {
                if v.ask.busy() {
                    v.ask_cancel(cx);
                } else {
                    v.canvas_focus.clone().focus(window, cx);
                }
                cx.notify();
            }
            TextFieldEvent::Changed | TextFieldEvent::Blur => {}
        }));
        subs.push(cx.subscribe_in(&label_field, window, |v: &mut FigureView, _, ev: &SnippetFieldEvent, window, cx| match ev {
            SnippetFieldEvent::Submit => v.commit_label(window, cx),
            SnippetFieldEvent::Blur => {
                if v.label_edit.is_some() {
                    v.commit_label(window, cx);
                }
            }
            SnippetFieldEvent::Cancel => {
                v.label_edit = None;
                v.label_preview = None;
                v.canvas_focus.clone().focus(window, cx);
                cx.notify();
            }
            SnippetFieldEvent::Changed => v.preview_label(cx),
            SnippetFieldEvent::Walk(dir) => v.next_label(*dir, window, cx),
        }));
        subs.push(cx.subscribe_in(&zn_field, window, |v: &mut FigureView, f, ev: &TextFieldEvent, window, cx| match ev {
            TextFieldEvent::Submit => {
                let n = f.read(cx).text().trim().parse::<u32>().ok().filter(|n| (1..=48).contains(n)).unwrap_or(6);
                v.insert_cayley(&format!("Z{n}"), window, cx);
            }
            TextFieldEvent::Cancel => {
                v.menu = None;
                v.canvas_focus.clone().focus(window, cx);
                cx.notify();
            }
            _ => {}
        }));
        let renderer = opts.render.then(|| ExactRenderer::new(&paths.templates_tex, &paths.state));
        // the Ask models (prefs.lua, read fresh for each window)
        let prefs = crate::ui::app::safe("prefs::read", || crate::core::prefs::read(&paths)).unwrap_or_default();
        let doc = Doc::new(&text);
        let mut view = View::default();
        view.fit(&doc.scene);
        let mut v = FigureView {
            path: opts.path.clone(),
            paths,
            quiet: opts.quiet,
            build_on_save: opts.build,
            doc,
            tool: Tool::Select,
            view,
            selection: Vec::new(),
            gesture: None,
            drag_scene: None,
            drag_figure: None,
            hover: None,
            ghost: None,
            last_ngon: None,
            label_edit: None,
            click_edit: None,
            clip: Default::default(),
            focus: cx.focus_handle(),
            canvas_focus: cx.focus_handle(),
            shorthand,
            ask_field,
            label_field,
            label_session,
            label_preview: None,
            ask: AskState::from_prefs(&prefs),
            reparse_due: None,
            render: RenderState { renderer, ..Default::default() },
            undo: Vec::new(),
            redo: Vec::new(),
            disk: DiskState {
                text: on_disk.then(|| text.clone()),
                mtime: mtime(&opts.path),
                saved_at: None,
                autosave_due: None,
                next_check: None,
                conflict: None,
                write_error: None,
                build_rx: None,
                build: BuildStatus::Idle,
                saves: 0,
            },
            toast: None,
            closes: 0,
            canvas_bounds: Rc::new(Cell::new(None)),
            last_canvas: None,
            undo_group: None,
            panel_open: None,
            reference_open: false,
            menu: None,
            fields: Default::default(),
            ui_bounds: Default::default(),
            shorthand_scroll: gpui::ScrollHandle::new(),
            scroll_to_code: true,
            readout: None,
            hover_handle: None,
            tab_order: Vec::new(),
            zn_field,
            fresh_elements: Vec::new(),
            fit_pending: true,
            build_cancel: Arc::new(AtomicBool::new(false)),
            _poll: None,
            _subs: subs,
        };
        v.shorthand.update(cx, |f, cx| f.set_text(text.clone(), cx));
        if !on_disk {
            // the new file exists from the start (Neovim's Space g opens it too)
            v.write(cx);
        }
        let now = v.now(cx);
        v.render.due = Some(now);
        v._poll = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                if this.update_in(cx, |v, window, cx| v.pump(window, cx)).is_err() {
                    break;
                }
            }
        }));
        v.canvas_focus.clone().focus(window, cx);
        v
    }

    /// A view over a hand-made document (tests; and nothing else).
    pub fn load_doc(&mut self, doc: Doc, cx: &mut Context<Self>) {
        self.doc = doc;
        self.view.fit(&self.doc.scene);
        self.fit_pending = true;
        let src = self.doc.source.clone();
        self.shorthand.update(cx, |f, cx| f.set_text(src, cx));
        self.schedule_render(cx);
        cx.notify();
    }

    pub fn now(&self, cx: &App) -> Instant {
        cx.background_executor().now()
    }

    pub fn file_name(&self) -> String {
        self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }

    pub fn dirty(&self) -> bool {
        self.disk.text.as_deref() != Some(self.doc.source.as_str())
    }

    pub fn toast(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        let id = self.toast.as_ref().map_or(0, |t| t.id + 1);
        self.toast = Some(FigToast { id, text: text.into(), at: self.now(cx) });
        cx.notify();
    }

    // -- changes -------------------------------------------------------------

    fn push_undo(&mut self, before: String) {
        self.undo_group = None;
        if self.undo.last() != Some(&before) {
            self.undo.push(before);
            if self.undo.len() > UNDO_MAX {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
    }

    /// Something changed the document: render, autosave, keep it in view.
    fn changed(&mut self, cx: &mut Context<Self>) {
        let now = self.now(cx);
        self.schedule_render(cx);
        self.disk.autosave_due = Some(now + AUTOSAVE_AFTER);
        // the selection may name objects that are gone
        let scene = self.doc.scene.clone();
        self.selection.retain(|o| scene.anchors.iter().any(|(a, _, _)| a == o) || scene.draws.iter().any(|d| draw_obj(d) == Some(o)));
        // only when nothing of the figure is left in view: refitting on every
        // edit that crosses the edge made the canvas jump under her hand
        if let Some(b) = self.canvas_bounds.get()
            && !self.view.touches(&scene.bounds, b)
        {
            self.view.fit_to(&scene, b);
            self.last_canvas = Some(b);
        }
        cx.notify();
    }

    fn schedule_render(&mut self, cx: &mut Context<Self>) {
        self.render.due = Some(self.now(cx) + RENDER_AFTER);
    }

    /// Replace the source (Ask, undo, reload): one undo step.
    pub fn set_source(&mut self, src: &str, undoable: bool, cx: &mut Context<Self>) {
        if undoable {
            let before = self.doc.source.clone();
            self.push_undo(before);
        }
        self.doc.set_source(src);
        let s = src.to_string();
        self.shorthand.update(cx, |f, cx| f.set_text(s, cx));
        self.reparse_due = None;
        self.changed(cx);
    }

    /// A canvas edit: the figure becomes the truth, the shorthand follows.
    pub fn commit_figure(&mut self, fig: Figure, what: &str, cx: &mut Context<Self>) -> bool {
        self.commit_grouped(fig, what, None, cx)
    }

    /// A canvas edit as part of a run (`group`): the first change of a run
    /// is an undo step, the rest of the run joins it.
    pub fn commit_grouped(&mut self, fig: Figure, what: &str, group: Option<String>, cx: &mut Context<Self>) -> bool {
        // drawing on the canvas: the view stays where she is drawing
        self.fit_pending = false;
        if self.gesture.is_none() {
            // (a hover preview's draft is out of date now)
            self.drag_scene = None;
            self.drag_figure = None;
        }
        let before = self.doc.source.clone();
        match self.doc.set_figure(fig) {
            Ok(()) => {
                if self.doc.source != before {
                    let joins = group.is_some() && group == self.undo_group;
                    if joins {
                        self.redo.clear();
                    } else {
                        self.push_undo(before);
                    }
                    self.undo_group = group;
                }
                let s = self.doc.source.clone();
                self.shorthand.update(cx, |f, cx| f.set_text(s, cx));
                self.reparse_due = None;
                self.changed(cx);
                true
            }
            Err(e) => {
                self.toast(format!("! Couldn't {what}: {e}"), cx);
                false
            }
        }
    }

    /// Apply canvas edits and commit; the names they made.
    pub fn edit_and_commit(&mut self, edits: Vec<Edit>, what: &str, _window: &mut Window, cx: &mut Context<Self>) -> Option<Vec<Option<String>>> {
        match self.doc.applied(&edits) {
            Ok((fig, names)) => self.commit_figure(fig, what, cx).then_some(names),
            Err(e) => {
                self.toast(format!("! Couldn't add {what}: {e}"), cx);
                None
            }
        }
    }

    pub fn fig_undo(&mut self, cx: &mut Context<Self>) {
        let Some(prev) = self.undo.pop() else {
            self.toast("Nothing to undo.", cx);
            return;
        };
        let cur = self.doc.source.clone();
        self.redo.push(cur);
        self.set_source(&prev, false, cx);
    }

    pub fn fig_redo(&mut self, cx: &mut Context<Self>) {
        let Some(next) = self.redo.pop() else { return };
        let cur = self.doc.source.clone();
        self.undo.push(cur);
        self.set_source(&next, false, cx);
    }

    pub fn tidy(&mut self, cx: &mut Context<Self>) {
        let mut fig = self.doc.figure.clone();
        match core("layout::tidy", || ntex_figures::layout::tidy(&mut fig)) {
            Some(edits) if edits.is_empty() => self.toast("Already tidy.", cx),
            Some(edits) => {
                if self.commit_figure(fig, "tidy", cx) {
                    self.toast(format!("Tidied ({} change{}) — ⌘Z undoes.", edits.len(), if edits.len() == 1 { "" } else { "s" }), cx);
                }
            }
            None => self.toast("! Tidy is not built yet.", cx),
        }
    }

    // -- labels ----------------------------------------------------------------

    /// Double-click / ↵ / a click on the label of a selected object / a new
    /// object: edit its label in place (the field sits beside the label).
    pub fn start_label_edit(&mut self, obj: ObjRef, window: &mut Window, cx: &mut Context<Self>) {
        // a polygon's label is its vertices'; ↵ on one starts at vertex 1
        let obj = match obj {
            ObjRef::NGon(n) => ObjRef::NGonVertex(n, 1),
            // the rotation arrow (selected as its own part) edits its label
            ObjRef::NGonRotation(n) => ObjRef::NGonRotationLabel(n),
            o => o,
        };
        self.menu = None;
        let scene = self.doc.scene.clone();
        if matches!(obj, ObjRef::Cayley(_) | ObjRef::CayleyEdge(..) | ObjRef::NGon(_)) {
            return;
        }
        let spot = self.label_spot(&obj);
        // as she typed it (φ stays φ, \varphi stays \varphi), else the LaTeX
        let vertex_label = match &obj {
            ObjRef::NGonVertex(n, k) => super::handles::item_index(&self.doc.figure, &obj).and_then(|i| match &self.doc.figure.items[i] {
                Item::NGon(g) => {
                    let j = super::handles::label_index_at(g, *k);
                    let latex = g.labels.get(j).cloned().unwrap_or_default();
                    Some(ntex_figures::spelling::raw(&self.doc.figure, &format!("ngon:{n}.{}", j + 1), &latex))
                }
                _ => None,
            }),
            _ => None,
        };
        let drawn = scene.draws.iter().find_map(|d| match d {
            Draw::Label { obj: o, latex, .. } if Some(o) == Some(&obj) => Some(latex.clone()),
            _ => None,
        });
        let current = vertex_label
            .or_else(|| core("spelling::as_typed", || ntex_figures::spelling::as_typed(&self.doc.figure, &obj)).flatten())
            .or_else(|| self.label_text(&obj))
            .or(drawn)
            .unwrap_or_default();
        let mut away = None;
        let at = spot
            .or_else(|| match obj {
                // where tikz-cd puts a new label: left of the direction of travel
                ObjRef::Arrow(_) => geom::path_mid(&scene, &obj).map(|m| {
                    let (dx, dy) = geom::path_dir(&scene, &obj).unwrap_or((1.0, 0.0));
                    Pt { x: m.x - dy * 0.32, y: m.y + dx * 0.32 }
                }),
                _ => geom::center_of(&scene, &obj),
            })
            .unwrap_or_default();
        if matches!(obj, ObjRef::Arrow(_)) {
            away = geom::path_mid(&scene, &obj);
        }
        let (current, math) = self.label_start(&obj, &current);
        self.label_preview = None;
        self.label_edit = Some(LabelEdit { obj, at, before: current.clone(), before_math: math, away });
        self.label_field.update(cx, |f, cx| {
            f.set_math(math, cx);
            f.set_text(current, cx);
            f.select_all(cx);
        });
        self.label_field.read(cx).focus.clone().focus(window, cx);
        cx.notify();
    }

    /// The label as written in the model.
    fn label_text(&self, obj: &ObjRef) -> Option<String> {
        for (i, it) in self.doc.figure.items.iter().enumerate() {
            match (it, obj) {
                (Item::Node(n), ObjRef::Node(m)) if &n.name == m => return Some(n.label.clone()),
                (Item::Arrow(a), ObjRef::Arrow(k)) if i == *k => return Some(a.label.clone().unwrap_or_default()),
                (Item::Blob(b), ObjRef::Blob(m)) if &b.name == m => return Some(b.label.clone()),
                (Item::Blob(b), ObjRef::Element(m)) => {
                    if let Some(e) = b.elements.iter().find(|e| &e.name == m) {
                        return Some(e.label.clone());
                    }
                }
                (Item::Text(t), ObjRef::Text(k)) if i == *k => return Some(t.text.clone()),
                _ => {}
            }
        }
        None
    }

    pub fn commit_label(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(le) = self.label_edit.take() else { return };
        self.label_preview = None;
        let typed = self.label_field.read(cx).text().trim().to_string();
        let math = self.label_field.read(cx).math();
        let label = self.field_label(&le.obj, cx);
        if self.label_field.read(cx).focus.is_focused(window) {
            self.canvas_focus.clone().focus(window, cx);
        }
        if typed == le.before && math == le.before_math {
            cx.notify();
            return;
        }
        // the text as typed: edit.rs makes it LaTeX and keeps her spelling,
        // so the shorthand shows φ if she typed φ
        self.set_label(&le.obj, &label, None, cx);
        cx.notify();
    }

    // -- the Ask box --------------------------------------------------------------

    pub fn ask_submit(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.ask.busy() {
            return;
        }
        let instruction = self.ask_field.read(cx).text().trim().to_string();
        if instruction.is_empty() {
            return;
        }
        let model = self.ask.model();
        let source = self.doc.source.clone();
        match core("AskJob::start", || AskJob::start(&source, &instruction, &model)) {
            Some(Ok((job, rx))) => self.ask_start_with(Some(job), rx, model, cx),
            Some(Err(e)) => {
                self.ask.error = Some(format!("Couldn't ask {}: {e}", ask::name(&model)));
                cx.notify();
            }
            None => {
                self.ask.error = Some("The Ask backend is not built yet.".into());
                cx.notify();
            }
        }
    }

    /// Run an ask from this receiver (the real job, or a stub in tests).
    pub fn ask_start_with(&mut self, job: Option<AskJob>, rx: Receiver<ntex_studio_io::figure_ai::AskEvent>, model: ask::AskModel, cx: &mut Context<Self>) {
        self.ask.error = None;
        self.ask.menu_open = false;
        self.ask.running = Some(ask::Running { job, rx, started: self.now(cx), model, progress: None });
        cx.notify();
    }

    pub fn ask_cancel(&mut self, cx: &mut Context<Self>) {
        self.ask.cancel();
        self.toast("Cancelled; the figure is as it was.", cx);
    }

    // -- saving -------------------------------------------------------------------

    /// Write the .fig (no build).
    pub fn write(&mut self, cx: &mut Context<Self>) -> bool {
        let text = self.doc.source.clone();
        match write_atomic(&self.path, &text) {
            Ok(()) => {
                self.disk.text = Some(text);
                self.disk.mtime = mtime(&self.path);
                self.disk.saved_at = Some(hhmm());
                self.disk.write_error = None;
                self.disk.autosave_due = None;
                self.disk.saves += 1;
                cx.notify();
                true
            }
            Err(e) => {
                self.disk.write_error = Some(format!("{}: {e}", self.path.display()));
                cx.notify();
                false
            }
        }
    }

    /// ⌘S / autosave: write, then build the PDF in the background.
    pub fn save(&mut self, cx: &mut Context<Self>) {
        self.disk.conflict = None;
        if !self.write(cx) {
            return;
        }
        if !self.build_on_save || self.doc.is_empty() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let path = self.path.clone();
        let templates = self.paths.templates_tex.clone();
        // a newer save supersedes a build still running (it would write an older PDF)
        self.build_cancel.store(true, Ordering::SeqCst);
        let cancel = Arc::new(AtomicBool::new(false));
        self.build_cancel = cancel.clone();
        builds_started(&cancel);
        std::thread::Builder::new()
            .name("ntex-fig-build".into())
            .spawn(move || {
                let r = core("build::build", || ntex_figures::build::build_cancellable(&path, &templates, &cancel));
                builds_finished(&cancel);
                let _ = tx.send(r);
            })
            .ok();
        self.disk.build_rx = Some(rx);
        self.disk.build = BuildStatus::Building;
    }

    // -- the poll loop --------------------------------------------------------------

    /// Drain every receiver and run what is due. Never blocks.
    pub fn pump(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = self.now(cx);
        let mut changed = self.apply_pending_fit();
        self.label_field.clone().update(cx, |f, cx| f.pump(window, cx));
        let fields: Vec<_> = self.fields.values().map(|f| f.entity.clone()).collect();
        for field in fields { field.update(cx, |f, cx| f.pump(window, cx)); }

        // typing in the shorthand: re-read after 150 ms
        if self.reparse_due.is_some_and(|t| now >= t) {
            self.reparse_due = None;
            let text = self.shorthand.read(cx).text().to_string();
            if text != self.doc.source {
                let before = self.doc.source.clone();
                self.push_undo(before);
                self.doc.set_source(&text);
                self.changed(cx);
            }
            changed = true;
        }

        changed |= self.pump_render(now, window, cx);

        if let Some((_, at)) = &self.ghost {
            if now.saturating_duration_since(*at) >= super::tools::GHOST {
                self.ghost = None;
            }
            changed = true;
        }

        // the Ask job
        match self.ask.pump() {
            ask::Outcome::Nothing => {}
            ask::Outcome::Progress => changed = true,
            ask::Outcome::Done { source, summary, seconds } => {
                self.set_source(&source, true, cx);
                let what = if summary.trim().is_empty() { String::new() } else { format!("{} ", summary.trim().trim_end_matches('.')) };
                let _ = what;
                self.toast(format!("Done in {seconds:.1} s — ⌘Z undoes."), cx);
                changed = true;
            }
            ask::Outcome::Failed(e) => {
                self.ask.error = Some(e);
                changed = true;
            }
        }

        // autosave (and the PDF build)
        if self.disk.autosave_due.is_some_and(|t| now >= t) && self.gesture.is_none() {
            self.disk.autosave_due = None;
            if self.dirty() && self.disk.conflict.is_none() {
                self.save(cx);
                changed = true;
            }
        }
        if let Some(rx) = &self.disk.build_rx {
            match rx.try_recv() {
                Ok(r) => {
                    self.disk.build_rx = None;
                    self.disk.build = match r {
                        Some(Ok(_)) => BuildStatus::Built(hhmm()),
                        Some(Err(e)) if e == ntex_figures::build::CANCELLED => BuildStatus::Idle,
                        Some(Err(e)) => BuildStatus::Failed(e.lines().next().unwrap_or("").to_string()),
                        None => BuildStatus::Failed("the PDF builder is not built yet".into()),
                    };
                    changed = true;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.disk.build_rx = None,
            }
        }

        // the file on disk (edited in Neovim)
        if self.disk.next_check.is_none_or(|t| now >= t) {
            self.disk.next_check = Some(now + DISK_EVERY);
            changed |= self.check_disk(cx);
        }

        if let Some(t) = &self.toast {
            let age = now.saturating_duration_since(t.at);
            if age >= TOAST_FOR + TOAST_FADE {
                self.toast = None;
                changed = true;
            } else if age >= TOAST_FOR {
                changed = true;
            }
        }
        if changed {
            cx.notify();
        }
    }

    /// Pixels per pt the exact render needs to be sharp at this zoom.
    fn wanted_scale(&self, window: &Window) -> f64 {
        let px_per_cm = self.canvas_bounds.get().map(|b| self.view.xf(b).scale).unwrap_or(geom::MAX_FIT);
        let dev = f64::from(px_per_cm / geom::PT_PER_CM) * f64::from(window.scale_factor());
        dev.ceil().clamp(2.0, 8.0)
    }

    fn pump_render(&mut self, now: Instant, window: &mut Window, _cx: &mut Context<Self>) -> bool {
        let mut changed = false;
        let wanted = self.wanted_scale(window);
        // zoomed in past the image's resolution: render it again, sharper
        if !self.render.busy()
            && self.render.exact.as_ref().is_some_and(|(g, _)| *g == self.doc.generation)
            && wanted > self.render.scale
        {
            self.render.due = Some(now);
        }
        let r = &mut self.render;
        if r.due.is_some_and(|d| now >= d) {
            r.due = None;
            let generation = self.doc.generation;
            let tex = if self.doc.is_empty() { None } else { core("tikz::document", || ntex_figures::tikz::document(&self.doc.figure, &self.doc.scene)) };
            match (&r.renderer, tex) {
                (Some(renderer), Some(tex)) => {
                    r.rx = Some((generation, self.doc.scene.bounds.clone(), renderer.render(tex, wanted)));
                }
                _ => {
                    r.rx = None;
                    if r.exact.is_some() {
                        r.exact = None;
                        changed = true;
                    }
                }
            }
        }
        if let Some((generation, bounds, rx)) = &r.rx {
            match rx.try_recv() {
                Ok(res) => {
                    let (generation, bounds) = (*generation, bounds.clone());
                    r.rx = None;
                    match res {
                        Ok(raster) => {
                            // white → transparent, so the cell dots show under the ink
                            let drx = decode_png(raster.png.clone(), true, false);
                            r.decode = Some((generation, bounds, raster, drx));
                        }
                        Err(e) if e == "superseded" => {}
                        Err(e) => {
                            r.error = Some(e.lines().next().unwrap_or("").to_string());
                            r.finished += 1;
                            changed = true;
                        }
                    }
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => r.rx = None,
            }
        }
        if let Some((generation, bounds, raster, rx)) = &r.decode {
            match rx.try_recv() {
                Ok(res) => {
                    if let Ok((image, _, _)) = res {
                        r.exact = Some((*generation, Exact { image, w_pt: raster.w_pt, h_pt: raster.h_pt, bounds: bounds.clone() }));
                        r.scale = raster.scale;
                        r.error = None;
                    }
                    r.decode = None;
                    r.finished += 1;
                    changed = true;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => r.decode = None,
            }
        }
        changed
    }

    fn check_disk(&mut self, cx: &mut Context<Self>) -> bool {
        let m = mtime(&self.path);
        if m.is_none() || m == self.disk.mtime {
            return false;
        }
        self.disk.mtime = m;
        let Ok(text) = std::fs::read_to_string(&self.path) else { return false };
        if Some(&text) == self.disk.text.as_ref() || text == self.doc.source {
            self.disk.text = Some(text);
            return false;
        }
        if self.dirty() {
            self.disk.conflict = Some(text);
        } else {
            self.disk.text = Some(text.clone());
            self.set_source(&text, true, cx);
            self.disk.autosave_due = None;
            self.zoom_fit(cx);
            self.scroll_to_code = true;
            self.toast(format!("Reloaded {}: it changed on disk. ⌘Z goes back.", self.file_name()), cx);
        }
        true
    }

    /// The conflict notice's buttons.
    pub fn resolve_conflict(&mut self, take_theirs: bool, cx: &mut Context<Self>) {
        let Some(theirs) = self.disk.conflict.take() else { return };
        if take_theirs {
            self.disk.text = Some(theirs.clone());
            self.set_source(&theirs, true, cx);
            self.disk.autosave_due = None;
            self.zoom_fit(cx);
            self.toast(format!("Reloaded {} — ⌘Z brings your edits back.", self.file_name()), cx);
        } else {
            self.save(cx);
            self.toast(format!("Kept yours; {} on disk is overwritten.", self.file_name()), cx);
        }
    }

    pub fn save_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.label_edit.is_some() {
            self.commit_label(window, cx);
        }
        if self.dirty() || self.disk.text.is_none() {
            self.save(cx);
        }
        self.closes += 1;
        if !self.quiet {
            window.remove_window();
        }
    }

    // -- zoom -------------------------------------------------------------------

    pub fn zoom_by(&mut self, factor: f32, cx: &mut Context<Self>) {
        self.fit_pending = false;
        if let Some(b) = self.canvas_bounds.get() {
            self.view.zoom_at(factor, b.center(), b);
        } else {
            self.view.zoom = (self.view.zoom * factor).clamp(0.2, 8.0);
        }
        cx.notify();
    }

    /// ⌘0, and the fit on open/reload: the whole figure, centred, at most 200 %.
    pub fn zoom_fit(&mut self, cx: &mut Context<Self>) {
        let scene = self.doc.scene.clone();
        match self.canvas_bounds.get() {
            Some(b) => {
                self.view.fit_to(&scene, b);
                self.fit_pending = false;
                self.last_canvas = Some(b);
            }
            // not laid out yet: fit on the first frame
            None => self.fit_pending = true,
        }
        cx.notify();
    }

    /// The canvas was drawn at new bounds: keep the figure where it was on
    /// screen (the canvas paints that way already; the view adopts it here).
    pub fn sync_view(&mut self) {
        let now = self.canvas_bounds.get();
        if !self.fit_pending
            && let (Some(b), Some(l)) = (now, self.last_canvas)
            && b != l
        {
            self.view.keep(l, b);
        }
        if !self.fit_pending {
            self.last_canvas = now;
        }
    }

    /// The pending fit, once the canvas has bounds and the figure has some
    /// (an empty figure waits for its first statement from the shorthand).
    fn apply_pending_fit(&mut self) -> bool {
        if !self.fit_pending {
            return false;
        }
        let Some(b) = self.canvas_bounds.get() else { return false };
        if geom::is_empty_rect(&self.doc.scene.bounds) {
            return false;
        }
        let scene = self.doc.scene.clone();
        self.view.fit_to(&scene, b);
        self.fit_pending = false;
        self.last_canvas = Some(b);
        true
    }

    // -- what the canvas shows -------------------------------------------------------

    /// Is the exact render on screen (rather than the native draft)?
    pub fn showing_exact(&self) -> bool {
        self.exact_now().is_some()
    }

    fn exact_now(&self) -> Option<&Exact> {
        let (generation, ex) = self.render.exact.as_ref()?;
        let idle = self.gesture.is_none() && self.drag_scene.is_none() && self.reparse_due.is_none() && self.label_edit.is_none();
        (idle && *generation == self.doc.generation).then_some(ex)
    }

    pub fn frame(&self, cx: &App) -> Frame {
        let now = self.now(cx);
        let scene = self.label_preview.clone().or_else(|| self.drag_scene.clone()).unwrap_or_else(|| self.doc.scene.clone());
        let overlay = match &self.gesture {
            Some(Gesture::Arrow { a, b, .. }) => Overlay::Line(*a, *b),
            Some(Gesture::Rect { a, b }) => Overlay::Rect(*a, *b),
            Some(Gesture::Rough { pts }) => Overlay::Stroke(pts.clone()),
            Some(Gesture::Marquee { a, b, .. }) if geom::dist(*a, *b) > 0.0 => Overlay::Marquee(*a, *b),
            // a dragged arrow end with nothing under it: a tether from the other end
            Some(Gesture::Handle { start, moved: true, last: None, .. }) if matches!(start.handle.kind, super::handles::HKind::ArrowEnd(_)) => {
                match (start.handle.kind, start.ends, &self.readout) {
                    (super::handles::HKind::ArrowEnd(is_to), Some((a, b)), Some((p, _))) => Overlay::Tether(if is_to { a } else { b }, *p),
                    _ => Overlay::None,
                }
            }
            _ => Overlay::None,
        };
        let px_per_cm = self.canvas_bounds.get().map(|b| self.view.xf(b).scale).unwrap_or(geom::MAX_FIT);
        let handles = if self.tool == Tool::Select { self.handle_list(px_per_cm) } else { Vec::new() };
        let cursor = match (&self.gesture, self.hover_handle) {
            (Some(Gesture::Handle { start, .. }), _) => Some(super::canvas::cursor_for(start.handle.kind, true)),
            (Some(Gesture::Move(m)), _) if m.moved => Some(if m.copied { gpui::CursorStyle::DragCopy } else { gpui::CursorStyle::ClosedHand }),
            (None, Some(k)) => Some(super::canvas::cursor_for(k, false)),
            _ => None,
        };
        let ghost = self.ghost.as_ref().map(|(o, at)| {
            let t = now.saturating_duration_since(*at).as_secs_f32() / super::tools::GHOST.as_secs_f32();
            (o.clone(), (1.0 - t).clamp(0.0, 1.0) * 0.9)
        });
        Frame {
            scene,
            view: self.view.clone(),
            exact: self.exact_now().cloned(),
            grid: self.tool.shows_grid().then(|| self.cell()),
            selection: self.selection.clone(),
            hover: self.hover.clone(),
            overlay,
            ghost,
            // (class C) the label stays drawn, live as typed, beside the field
            editing: None,
            empty_hint: Some("An empty figure. N places a node; or type the shorthand below.".into()),
            fit: self.fit_pending,
            prev_bounds: self.last_canvas,
            ring: match self.selection.as_slice() {
                [o @ ObjRef::NGon(_)] if self.tool == Tool::Select => {
                    let fig = self.drag_figure.as_ref().unwrap_or(&self.doc.figure);
                    let scene = self.label_preview.clone().or_else(|| self.drag_scene.clone()).unwrap_or_else(|| self.doc.scene.clone());
                    super::handles::frame(fig, &scene, o).map(|f| (f.centre, f.radius))
                }
                _ => None,
            },
            handles,
            readout: self.readout.clone(),
            cursor,
            drop_target: matches!(&self.gesture, Some(Gesture::Handle { start, .. }) if matches!(start.handle.kind, super::handles::HKind::ArrowEnd(_))),
        }
    }

    /// Footnotes for whatever has the keyboard.
    pub fn hints(&self, window: &Window, cx: &App) -> Vec<SharedString> {
        let keys: SharedString = "⌘S saves and builds the PDF · ⌘Z / ⌘⇧Z undo, redo · ⌘= ⌘- zoom, ⌘0 fits · ⌘T tidies · ⌘W saves and closes".into();
        if self.label_field.read(cx).focus.is_focused(window) {
            return vec![self.label_field.read(cx).hint().into()];
        }
        if self.shorthand.read(cx).focus.is_focused(window) {
            return vec!["The shorthand is the figure: the canvas follows 150 ms after you stop typing · esc goes to the canvas".into(), keys];
        }
        if self.ask_field.read(cx).focus.is_focused(window) {
            let m = ask::name(&self.ask.model());
            let other = if self.ask.fast {
                let normal = self.ask.models.get(self.ask.selected).map(ask::name).unwrap_or_default();
                format!("untick fast to ask {normal}")
            } else {
                format!("tick fast to ask {}", ask::name(&self.ask.fast_model))
            };
            return vec![format!("↵ asks {m} to rewrite the figure; the answer replaces it as one ⌘Z · {other}").into(), keys];
        }
        if let Some(f) = self.fields.values().find(|f| f.entity.read(cx).focus.is_focused(window) && f.is_label) {
            return vec![f.entity.read(cx).hint().into(), keys];
        }
        if self.fields.values().any(|f| f.entity.read(cx).focus.is_focused(window)) {
            return vec!["Every key applies at once; the field is one ⌘Z · ↵ or esc goes back to the canvas".into(), keys];
        }
        if self.tool == Tool::Select && !self.selection.is_empty() {
            return vec![self.selection_hint().into(), "⌥-drag copies · ⌘D duplicates · ⌘C ⌘X ⌘V · L labels · ⌘I the panel · ⌘/ the reference · right-click for more".into()];
        }
        vec![self.tool.hint().into(), keys]
    }

    /// The keys for what is selected (a footnote).
    fn selection_hint(&self) -> String {
        let shape = self.selection.iter().any(|o| matches!(super::interact::owner(o), ObjRef::NGon(_) | ObjRef::Blob(_) | ObjRef::Cayley(_)));
        match self.selection.as_slice() {
            [ObjRef::NGonVertex(..)] => "↵ edits the vertex's label · ⌫ hides it · the polygon's handles are a click inside it".into(),
            [ObjRef::Arrow(_)] => "drag the middle handle to bend (snaps at 0, 30°, 45°) · drag the label across to swap sides · drag an end dot onto another object to reattach it · ⌘R reverses · click the label again or ↵ edits it · ⌫ deletes".into(),
            [ObjRef::NGonRotation(_)] => "drag the arrow round to another edge · ⌘R runs it the other way (↺ ↻) · ↵ names it · ⌫ removes it".into(),
            [ObjRef::CayleyVertex(..)] => "a vertex of the graph · esc, then a click, takes the whole graph".into(),
            _ if shape => "drag to move (⌥ copies, ⇧ keeps a line, ⌘ goes free) · drag ↻ to turn (15° steps, ⌘ free) and the dots to resize · [ ] turn 15° · − = size · arrows nudge (⌥ finer) · ⌫ deletes".into(),
            _ => "drag to move (⌥ copies, ⇧ keeps a line, ⌘ goes free) · arrows nudge (⌥ finer) · click the label again or ↵ edits it · ⌫ deletes".into(),
        }
    }
}

// -- PDF builds in flight (every window's), so quitting can stop them ----------

fn running_builds() -> &'static std::sync::Mutex<Vec<Arc<AtomicBool>>> {
    static RUNNING: std::sync::OnceLock<std::sync::Mutex<Vec<Arc<AtomicBool>>>> = std::sync::OnceLock::new();
    RUNNING.get_or_init(Default::default)
}

fn builds_started(flag: &Arc<AtomicBool>) {
    if let Ok(mut v) = running_builds().lock() {
        v.push(flag.clone());
    }
}

fn builds_finished(flag: &Arc<AtomicBool>) {
    if let Ok(mut v) = running_builds().lock() {
        v.retain(|f| !Arc::ptr_eq(f, flag));
    }
}

/// How many PDF builds (tectonic) are still running.
pub fn builds_running() -> usize {
    running_builds().lock().map(|v| v.len()).unwrap_or(0)
}

/// Stop every PDF build (the studio is quitting): each kills and reaps its
/// tectonic within a few ms; `builds_running` reaches 0 when they have.
pub fn cancel_all_builds() {
    if let Ok(v) = running_builds().lock() {
        for f in v.iter() {
            f.store(true, Ordering::SeqCst);
        }
    }
}

fn draw_obj(d: &Draw) -> Option<&ObjRef> {
    match d {
        Draw::Label { obj, .. } | Draw::Path { obj, .. } | Draw::Dot { obj, .. } => Some(obj),
    }
}

/// Labels no canvas edit covers (text), and every label while `edit` is not
/// built: `typed` made LaTeX here. A text keeps her spelling as a hint.
pub(crate) fn set_label_direct(fig: &mut Figure, obj: &ObjRef, typed: &str) -> bool {
    if matches!(obj, ObjRef::NGonAxis(..) | ObjRef::NGonRotationLabel(_) | ObjRef::CayleyVertex(..)) {
        return FigureView::set_label_part_direct(fig, obj, typed);
    }
    let latex = doc::to_latex(typed);
    let latex = latex.as_str();
    let mut text_at = None;
    for (i, it) in fig.items.iter_mut().enumerate() {
        match (it, obj) {
            (Item::Node(n), ObjRef::Node(m)) if &n.name == m && !latex.is_empty() => {
                n.label = latex.to_string();
                return true;
            }
            (Item::Arrow(a), ObjRef::Arrow(k)) if i == *k => {
                a.label = (!latex.is_empty()).then(|| latex.to_string());
                return true;
            }
            (Item::Blob(b), ObjRef::Blob(m)) if &b.name == m => {
                b.label = latex.to_string();
                return true;
            }
            (Item::Blob(b), ObjRef::Element(m)) => {
                if let Some(e) = b.elements.iter_mut().find(|e| &e.name == m) {
                    e.label = latex.to_string();
                    return true;
                }
            }
            (Item::NGon(g), ObjRef::NGonVertex(name, vertex)) if &g.name == name => {
                let j = super::handles::label_index_at(g, *vertex);
                if let Some(label) = g.labels.get_mut(j) {
                    *label = latex.to_string();
                    return true;
                }
            }
            (Item::Text(_), ObjRef::Text(k)) if i == *k => {
                text_at = Some(i);
                break;
            }
            _ => {}
        }
    }
    let Some(i) = text_at else { return false };
    let text = ntex_figures::labeltext::text_item_from_field(typed);
    if let Some(Item::Text(t)) = fig.items.get_mut(i) {
        t.text = text.clone();
    }
    // the hints are keyed by (site, LaTeX); "text" is one key for every text
    let raw = typed.to_string();
    fig.hints.spelling.retain(|s| !(s.key == "text" && s.latex == text));
    if raw != text && !raw.is_empty() {
        fig.hints.spelling.push(Spelling { key: "text".into(), latex: text, raw });
    }
    let _ = core("spelling::normalize", || ntex_figures::spelling::normalize(fig));
    true
}

impl Focusable for FigureView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

// ---------------------------------------------------------------------------
// rendering

impl FigureView {
    fn render_head(&self, t: &Theme) -> impl IntoElement {
        let status: String = if self.disk.conflict.is_some() {
            "changed on disk".into()
        } else if self.disk.write_error.is_some() {
            "not saved".into()
        } else if self.dirty() {
            "edited".into()
        } else {
            let saved = self.disk.saved_at.as_ref().map(|s| format!("saved {s}")).unwrap_or_else(|| "opened".into());
            match &self.disk.build {
                BuildStatus::Building => format!("{saved} · building the PDF…"),
                BuildStatus::Failed(_) => format!("{saved} · the PDF didn't build"),
                _ => saved,
            }
        };
        div().pt(px(TITLEBAR)).px(px(MARGIN)).child(
            div()
                .flex()
                .flex_row()
                .w_full()
                .justify_between()
                .items_end()
                .pb(px(3.))
                .border_b_1()
                .border_color(t.head_rule)
                .line_height(t.line(t.body_size))
                .child(div().font(t.serif_italic()).text_size(t.body_size).text_color(t.slate).child("Ntex Figures"))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_end()
                        .gap(px(8.))
                        .child(div().font(t.mono()).text_size(t.mono_size).text_color(t.ink).child(self.file_name()))
                        .child(div().font(t.serif()).text_color(t.slate).child("·"))
                        .child(div().font(t.serif_italic()).text_size(t.body_size).text_color(t.slate).child(status)),
                ),
        )
    }

    fn render_toolbar(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let mut row = div().flex().flex_row().flex_wrap().items_center().gap(px(8.));
        for tool in Tool::ALL {
            let mut b = Button::new(SharedString::from(format!("tool.{}", tool.name())), tool.name())
                .keys(tool.key())
                .on_click(cx.listener(move |v, _, window, cx| {
                    v.set_tool(tool, cx);
                    v.canvas_focus.clone().focus(window, cx);
                }));
            if self.tool == tool {
                b = b.visual(Visual::Hover);
            }
            row = row.child(b);
        }
        row = row.child(div().w(px(8.))).child(Button::new("tidy", "tidy").keys("⌘T").on_click(cx.listener(|v, _, _, cx| v.tidy(cx))));
        let _ = t;
        row
    }

    /// What the canvas shows, in its corner: *exact* / *draft* / *rendering…*, and the zoom.
    fn canvas_state(&self) -> String {
        let state = if self.gesture.is_some() || self.drag_scene.is_some() {
            "draft"
        } else if self.showing_exact() {
            "exact render"
        } else if self.render.busy() && !self.doc.is_empty() {
            "draft · rendering…"
        } else {
            "draft"
        };
        format!("{state} · {:.0} %", self.view.zoom * 100.0)
    }

    fn render_canvas(&self, t: &Theme, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let frame = self.frame(cx);
        let down = cx.listener(|v, ev: &(gpui::MouseDownEvent, geom::Xf), window, cx| v.canvas_down(&ev.0, ev.1, window, cx));
        let moved = cx.listener(|v, ev: &(gpui::MouseMoveEvent, geom::Xf), window, cx| v.canvas_moved(&ev.0, ev.1, window, cx));
        let up = cx.listener(|v, ev: &(gpui::MouseUpEvent, geom::Xf), window, cx| v.canvas_up(&ev.0, ev.1, window, cx));
        let scroll = cx.listener(|v, ev: &(gpui::ScrollWheelEvent, Bounds<Pixels>), _window, cx| {
            // her own zoom and pan win over a fit still waiting
            v.fit_pending = false;
            let d = ev.0.delta.pixel_delta(px(16.));
            if ev.0.modifiers.platform || ev.0.modifiers.control {
                // ⌘-scroll zooms, like a pinch
                let f = (1.0 - f32::from(d.y) / 200.0).clamp(0.5, 2.0);
                v.view.zoom_at(f, ev.0.position, ev.1);
            } else {
                v.view.pan.0 += f32::from(d.x);
                v.view.pan.1 += f32::from(d.y);
            }
            cx.notify();
        });
        let pinch = cx.listener(|v, ev: &(gpui::PinchEvent, Bounds<Pixels>), _window, cx| {
            v.fit_pending = false;
            v.view.zoom_at(1.0 + ev.0.delta, ev.0.position, ev.1);
            cx.notify();
        });
        let handlers = Handlers {
            down: Box::new(move |e, xf, w, cx| down(&(e.clone(), xf), w, cx)),
            moved: Box::new(move |e, xf, w, cx| moved(&(e.clone(), xf), w, cx)),
            up: Box::new(move |e, xf, w, cx| up(&(e.clone(), xf), w, cx)),
            scroll: Box::new(move |e, b, w, cx| scroll(&(e.clone(), b), w, cx)),
            pinch: Box::new(move |e, b, w, cx| pinch(&(e.clone(), b), w, cx)),
        };
        // the label field, beside the label it edits (above it; across the line for an arrow)
        let label = self.label_edit.as_ref().and_then(|le| {
            let b = self.canvas_bounds.get()?;
            let xf = self.view.xf(b);
            let is_text = matches!(le.obj, ObjRef::Text(_));
            let math = self.label_field.read(cx).math();
            // as wide as what is typed (it grows), so the figure around it stays visible
            let chars = self.label_field.read(cx).text().chars().count().max(2) as f32;
            let chip_w = if is_text { 0.0 } else { 46.0 };
            let max_w = (f32::from(b.size.width) - 12.).max(72.);
            let (fw, h) = ((chars * 8.0 + 26.0).clamp(72.0, max_w - chip_w), 28.0);
            let w = fw + chip_w;
            let c = self.field_centre(le.at, le.away, &xf, h);
            let c = gpui::point(c.x - b.origin.x, c.y - b.origin.y);
            let left = (c.x - px(w / 2.)).max(px(6.)).min(b.size.width - px(w + 6.));
            let mut top = c.y - px(h / 2.);
            // no room above the label: below it
            if top < px(6.) && le.away.is_none() {
                let p = xf.to_px(le.at);
                top = p.y - b.origin.y + px(h * 0.5 + 6.);
            }
            let top = top.max(px(6.)).min(b.size.height - px(h + 6.));
            let chip = (!is_text).then(|| {
                super::inspector::chip(t, "label.text", "text", !math, false)
                    .h(px(28.))
                    .on_click(cx.listener(|v, _, window, cx| v.toggle_label_mode(window, cx)))
                    .child(super::menu::record_bounds(self.ui_bounds.clone(), "label.text".to_string()))
                    .into_any_element()
            });
            Some(
                div()
                    .key_context(LABEL_CONTEXT)
                    .absolute()
                    .left(left)
                    .top(top)
                    .w(px(w))
                    .occlude()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.))
                    .bg(Theme::alpha(t.paper, 0.94))
                    .border_1()
                    .border_color(Theme::alpha(t.blue, 0.5))
                    .px(px(4.))
                    .child(div().flex_1().min_w_0().child(self.label_field.clone()))
                    .children(chip),
            )
        });
        let focused = self.canvas_focus.is_focused(window);
        div()
            .id("canvas")
            .key_context(CANVAS_CONTEXT)
            .track_focus(&self.canvas_focus)
            .relative()
            .flex_1()
            .min_w(px(200.))
            .h_full()
            .border_1()
            .border_color(if focused { Theme::alpha(t.blue, 0.55) } else { Theme::alpha(t.slate, 0.35) })
            .overflow_hidden()
            .on_action(cx.listener(|v, _: &ToolSelect, _, cx| v.set_tool(Tool::Select, cx)))
            .on_action(cx.listener(|v, _: &ToolNode, _, cx| v.set_tool(Tool::Node, cx)))
            .on_action(cx.listener(|v, _: &ToolArrow, _, cx| v.set_tool(Tool::Arrow, cx)))
            .on_action(cx.listener(|v, _: &ToolBlob, _, cx| v.set_tool(Tool::Blob, cx)))
            .on_action(cx.listener(|v, _: &ToolPolygon, _, cx| v.set_tool(Tool::Polygon, cx)))
            .on_action(cx.listener(|v, _: &ToolRough, _, cx| v.set_tool(Tool::Rough, cx)))
            .on_action(cx.listener(|v, _: &DeleteSelection, window, cx| v.delete_selected(window, cx)))
            .on_action(cx.listener(|v, _: &CanvasEscape, _, cx| {
                v.drag_scene = None;
                v.drag_figure = None;
                v.readout = None;
                v.hover = None;
                if v.menu.take().is_some() {
                    // just the menu
                } else if let Some(g) = v.gesture.take() {
                    // (a drag of copies: the originals are what is selected again)
                    if let Gesture::Move(m) = g
                        && m.copied
                    {
                        v.selection = m.sel0.clone();
                    }
                } else {
                    // a Cayley vertex steps out to its graph, then nothing
                    match v.selection.as_slice() {
                        [ObjRef::CayleyVertex(c, _)] => v.selection = vec![ObjRef::Cayley(c.clone())],
                        _ => v.selection.clear(),
                    }
                }
                cx.notify();
            }))
            .on_action(cx.listener(|v, _: &SelectAll, _, cx| v.select_all(cx)))
            .on_action(cx.listener(|v, _: &Duplicate, _, cx| {
                v.duplicate_selection(cx);
            }))
            .on_action(cx.listener(|v, _: &CopySel, _, cx| {
                v.copy_selection(false, cx);
            }))
            .on_action(cx.listener(|v, _: &CutSel, _, cx| {
                v.copy_selection(true, cx);
            }))
            .on_action(cx.listener(|v, _: &PasteSel, _, cx| {
                v.paste(cx);
            }))
            .on_action(cx.listener(|v, _: &Reverse, _, cx| {
                v.reverse_selection(cx);
            }))
            .on_key_down(cx.listener(|v, ev: &KeyDownEvent, window, cx| {
                let k = &ev.keystroke;
                if k.modifiers.platform || k.modifiers.control {
                    return;
                }
                let alt = k.modifiers.alt;
                let done = match k.key.as_str() {
                    "left" => v.nudge(-1.0, 0.0, alt, cx),
                    "right" => v.nudge(1.0, 0.0, alt, cx),
                    "up" => v.nudge(0.0, 1.0, alt, cx),
                    "down" => v.nudge(0.0, -1.0, alt, cx),
                    _ if alt => false,
                    "[" => v.rotate_by(15.0, cx),
                    "]" => v.rotate_by(-15.0, cx),
                    "-" => v.scale_by(1.0 / 1.1, cx),
                    "=" => v.scale_by(1.1, cx),
                    "l" => v.toggle_labels(cx),
                    "enter" => match v.selection.first().cloned() {
                        Some(o) if v.selection.len() == 1 => {
                            v.start_label_edit(o, window, cx);
                            true
                        }
                        _ => false,
                    },
                    key => match key.parse::<u32>() {
                        Ok(n) if (3..=9).contains(&n) => v.set_sides(n, cx),
                        _ => false,
                    },
                };
                if done {
                    cx.stop_propagation();
                }
            }))
            .child(canvas::element(frame, handlers, self.canvas_bounds.clone()))
            .child(
                div()
                    .absolute()
                    .right(px(4.))
                    .bottom(px(2.))
                    .px(px(4.))
                    .bg(Theme::alpha(t.paper, 0.9))
                    .font(t.serif_italic())
                    .text_size(t.small_size)
                    .text_color(Theme::alpha(t.slate, 0.8))
                    .child(self.canvas_state()),
            )
            .children(label)
            .into_any_element()
    }

    fn render_shorthand(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let mut lines: Vec<AnyElement> = Vec::new();
        if let Some(theirs) = &self.disk.conflict {
            let _ = theirs;
            lines.push(
                TBox::new(BoxKind::Remark, "Remark")
                    .run_in()
                    .child(div().child(format!("{} changed on disk while you had unsaved edits.", self.file_name())))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.))
                            .pt(px(4.))
                            .child(Button::new("conflict.theirs", "Reload it").small().on_click(cx.listener(|v, _, _, cx| v.resolve_conflict(true, cx))))
                            .child(Button::new("conflict.mine", "Keep mine").small().on_click(cx.listener(|v, _, _, cx| v.resolve_conflict(false, cx)))),
                    )
                    .into_any_element(),
            );
        }
        let mut log: Vec<(crate::model::Severity, String)> = Vec::new();
        // errors and warnings; the parser's notes ("`A` is new here, so it
        // became a node.") come on almost every line, so they stay hidden
        for d in &self.doc.diagnostics {
            let sev = match d.severity {
                Severity::Error => crate::model::Severity::Error,
                Severity::Warning => crate::model::Severity::Warning,
                Severity::Note => continue,
            };
            log.push((sev, d.message.clone()));
        }
        if let Some(e) = &self.render.error {
            log.push((crate::model::Severity::Error, e.clone()));
        }
        if let BuildStatus::Failed(e) = &self.disk.build {
            log.push((crate::model::Severity::Error, format!("{}: {e}", self.file_name().replace(".fig", ".pdf"))));
        }
        if let Some(e) = &self.disk.write_error {
            log.push((crate::model::Severity::Error, format!("Couldn't save {e}")));
        }
        // errors first, then warnings, then notes (in file order within each)
        log.sort_by_key(|(sev, _)| match sev {
            crate::model::Severity::Error => 0,
            crate::model::Severity::Warning => 1,
            crate::model::Severity::Note => 2,
        });
        let missing = doc::missing();
        if !missing.is_empty() {
            log.push((crate::model::Severity::Note, format!("Not built yet: {}.", missing.join(", "))));
        }
        let shown = log.len().min(5);
        let more = log.len() - shown;
        let field = div()
            .id("shorthand-scroll")
            .track_scroll(&self.shorthand_scroll)
            // nine 20 px lines: the box never cuts a line in half
            .max_h(px(182.))
            .overflow_y_scroll()
            .pt(px(2.))
            .child(self.shorthand.clone());
        let mut proof = TBox::new(BoxKind::Proof, "Proof").name("Shorthand").child(field);
        if !log.is_empty() {
            proof = proof.child(
                div()
                    .flex()
                    .flex_col()
                    .pt(px(6.))
                    .children(log.into_iter().take(shown).map(|(s, m)| LogLine::new(s, m).into_any_element()))
                    .when(more > 0, |d| d.child(div().font(t.serif_italic()).text_color(t.slate).child(format!("… and {more} more")))),
            );
        }
        // ⌘/: the reference, over the shorthand box (growing up over the
        // canvas, so nothing else moves)
        let reference = self.reference_open.then(|| {
            let close = Button::new("reference.close", "Close").small().keys("⌘/").on_click(cx.listener(|v, _, window, cx| {
                v.reference_open = false;
                v.shorthand.read(cx).focus.clone().focus(window, cx);
                cx.notify();
            }));
            deferred(
                div()
                    .absolute()
                    .left(px(MARGIN))
                    .right(px(MARGIN))
                    .bottom_0()
                    .bg(t.paper)
                    .border_t_1()
                    .border_color(Theme::alpha(t.ink, 0.6))
                    .pt(px(6.))
                    .child(
                        div()
                            .id("reference-scroll")
                            .max_h(px(470.))
                            .overflow_y_scroll()
                            .child(super::reference::render(t).child(div().pt(px(6.)).flex().flex_row().justify_end().child(close))),
                    ),
            )
            .with_priority(2)
        });
        div().relative().px(px(MARGIN)).pt(px(12.)).flex().flex_col().gap(px(8.)).children(lines).child(proof).children(reference)
    }

    fn render_ask(&self, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.ask.model();
        let name = ask::name(&model);
        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .child(caps_label(t, "Ask").flex_none())
            .child(div().flex_1().min_w(px(120.)).child(self.ask_field.clone()));
        if let Some(r) = &self.ask.running {
            let secs = self.now(cx).saturating_duration_since(r.started).as_secs_f32();
            let what = format!("asking {}… {:.0} s", ask::name(&r.model), secs);
            row = row
                .child(div().font(t.serif_italic()).text_color(t.slate).child(what))
                .child(Button::new("ask.cancel", "Cancel").keys("esc").on_click(cx.listener(|v, _, _, cx| v.ask_cancel(cx))));
        } else {
            let menu_label = if self.ask.fast { self.ask.models.get(self.ask.selected).map(ask::name).unwrap_or_default() } else { name.clone() };
            let mut menu_button = Button::new("ask.model", format!("{menu_label} ▾")).on_click(cx.listener(|v, _, _, cx| {
                v.ask.menu_open = !v.ask.menu_open;
                cx.notify();
            }));
            if self.ask.fast {
                menu_button = menu_button.disabled(true);
            }
            // ☐ fast / ☒ fast: the fast model (prefs' figure_fast_model) instead of the menu's
            let me = cx.entity().downgrade();
            let fast = Checkbox::new("ask.fast", "fast", self.ask.fast).on_toggle(move |on, _, cx| {
                let _ = me.update(cx, |v, cx| {
                    v.ask.fast = on;
                    v.ask.menu_open = false;
                    cx.notify();
                });
            });
            let menu = self.ask.menu_open.then(|| {
                let mut list = div().flex().flex_col().bg(t.paper).border_1().border_color(t.rule).py(px(4.)).min_w(px(180.));
                for (i, m) in self.ask.models.iter().enumerate() {
                    let on = i == self.ask.selected;
                    list = list.child(
                        div()
                            .id(SharedString::from(format!("ask.model.{i}")))
                            .flex()
                            .flex_row()
                            .gap(px(8.))
                            .px(px(10.))
                            .min_h(px(24.))
                            .items_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(t.theorem.back))
                            .child(div().font(t.math_font()).text_color(if on { t.ink } else { t.slate }).child(if on { "●" } else { "○" }))
                            .child(div().child(ask::name(m)))
                            .child(div().font(t.mono()).text_size(t.small_size).text_color(t.slate).child(m.model.clone()))
                            .on_click(cx.listener(move |v, _, _, cx| {
                                v.ask.selected = i;
                                v.ask.menu_open = false;
                                cx.notify();
                            })),
                    );
                }
                // opens upward, right-aligned with its button (inside the page margin)
                deferred(div().absolute().right_0().bottom(px(36.)).child(list)).with_priority(1)
            });
            row = row.child(div().relative().child(menu_button).children(menu)).child(fast).child(Keycap::new("↵"));
        }
        let status = if let Some(r) = &self.ask.running {
            r.progress.clone().filter(|p| !p.starts_with("Asking")).map(|p| div().pt(px(4.)).font(t.serif_italic()).text_color(t.slate).child(p).into_any_element())
        } else {
            self.ask.error.as_ref().map(|e| div().pt(px(4.)).child(LogLine::new(crate::model::Severity::Error, e.clone())).into_any_element())
        };
        div().px(px(MARGIN)).pt(px(10.)).child(row).children(status)
    }

    fn render_footer(&self, t: &Theme, window: &Window, cx: &App) -> impl IntoElement {
        let notes = self.hints(window, cx);
        let now = self.now(cx);
        let toast = self.toast.as_ref().map(|toast| {
            MarginNote::new(SharedString::from(format!("figtoast.{}", toast.id)), toast.text.clone())
                .duration(TOAST_FOR + TOAST_FADE)
                .opacity(toast.opacity(now))
        });
        div()
            .flex()
            .flex_col()
            .px(px(MARGIN))
            .pt(px(8.))
            .pb(px(14.))
            .bg(t.paper)
            .child(div().w_full().h(t.line(t.small_size)).mb(px(6.)).flex().flex_row().justify_end().children(toast.map(|note| div().max_w(px(640.)).child(note))))
            .child(Footnotes::new(notes))
    }
}

impl Render for FigureView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme().clone();
        self.apply_pending_fit();
        self.sync_view();
        if self.ghost.is_some() || self.ask.busy() {
            window.request_animation_frame();
        }
        if self.scroll_to_code {
            self.scroll_to_code = false;
            let line = super::reference::first_code_line(&self.doc.source) as f32;
            let lh = f32::from(t.line(t.mono_size).max(px(20.)));
            self.shorthand_scroll.set_offset(gpui::point(px(0.), px(-(line * lh))));
        }
        let head = self.render_head(&t).into_any_element();
        let toolbar = div().px(px(MARGIN)).pt(px(14.)).child(self.render_toolbar(&t, cx)).into_any_element();
        let canvas = self.render_canvas(&t, window, cx);
        let panel = self.panel_shown(window).then(|| self.render_panel(&t, window, cx));
        let canvas = div()
            .flex()
            .flex_row()
            .flex_1()
            .min_h(px(200.))
            .px(px(MARGIN))
            .mt(px(10.))
            .gap(px(16.))
            .child(canvas)
            .children(panel)
            .into_any_element();
        let menu = self.render_menu(&t, window, cx);
        let shorthand = self.render_shorthand(&t, cx).into_any_element();
        let ask = self.render_ask(&t, cx).into_any_element();
        let footer = self.render_footer(&t, window, cx).into_any_element();
        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(t.paper)
            .text_color(t.ink)
            .font(t.serif())
            .text_size(t.body_size)
            .line_height(t.line(t.body_size))
            .on_action(cx.listener(|v, _: &Tidy, _, cx| v.tidy(cx)))
            .on_action(cx.listener(|v, _: &FigUndo, _, cx| v.fig_undo(cx)))
            .on_action(cx.listener(|v, _: &FigRedo, _, cx| v.fig_redo(cx)))
            .on_action(cx.listener(|v, _: &FigSave, window, cx| {
                if v.label_edit.is_some() {
                    v.commit_label(window, cx);
                }
                v.save(cx);
                if v.disk.write_error.is_none() {
                    let f = v.file_name();
                    v.toast(format!("Saved {f}; building the PDF."), cx);
                }
            }))
            .on_action(cx.listener(|v, _: &SaveClose, window, cx| v.save_close(window, cx)))
            .on_action(cx.listener(|v, _: &ZoomIn, _, cx| v.zoom_by(1.25, cx)))
            .on_action(cx.listener(|v, _: &ZoomOut, _, cx| v.zoom_by(0.8, cx)))
            .on_action(cx.listener(|v, _: &ZoomFit, _, cx| v.zoom_fit(cx)))
            .on_action(cx.listener(|v, _: &TogglePanel, window, cx| {
                let shown = v.panel_shown(window);
                v.panel_open = Some(!shown);
                cx.notify();
            }))
            .on_action(cx.listener(|v, _: &NextLabel, window, cx| v.next_label(1, window, cx)))
            .on_action(cx.listener(|v, _: &PrevLabel, window, cx| v.next_label(-1, window, cx)))
            .on_action(cx.listener(|v, _: &ToggleReference, _, cx| {
                v.reference_open = !v.reference_open;
                cx.notify();
            }))
            // ⌘↵ inside a field (the kit binds it to the studio's Save): ask
            .on_action(cx.listener(|v, _: &crate::ui::app::Save, window, cx| {
                if v.ask_field.read(cx).focus.is_focused(window) {
                    v.ask_submit(window, cx);
                }
            }))
            .child(head)
            .child(toolbar)
            .child(canvas)
            .child(shorthand)
            .child(ask)
            .child(footer)
            .children(menu)
    }
}
