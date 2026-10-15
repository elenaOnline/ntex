//! Offscreen renders of the figure window (FIG-UI), driving the real
//! FigureView through GPUI's HeadlessAppContext with the Metal renderer and
//! the real macOS text system. No window ever appears; nothing activates.
//!
//! `harness = false`: AppKit wants the text system on the main thread.
//!
//! Every file is a temp file: `.fig`s live in a temp folder, the render
//! cache in a temp NTEX_STUDIO_STATE, the theme is a temp copy of
//! templates/tex. PNGs land in studio/evidence/figure/.
//!
//! While FIG-CORE-A/B are still building (`todo!()` in figures/src), the
//! steps that need a missing piece print BLOCKED instead of failing, and the
//! window is fed hand-made figures (laid out by the real `layout` when it is
//! there). `cargo test --test figure_renders`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    AppContext as _, HeadlessAppContext, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    PlatformInput, Point, Pixels, WindowHandle, point, px, size,
};
use ntex_figures::model::*;
use ntex_studio::ui::figure::doc::{self, Doc};
use ntex_studio::ui::figure::handles::HKind;
use ntex_studio::ui::figure::tools::Tool;
use ntex_studio::ui::figure::view::{FigureOptions, FigureView};
use ntex_studio_io::figure_ai::{AskEvent, AskModel};

const W: f32 = 900.0;
const H: f32 = 900.0;

struct Win {
    cx: HeadlessAppContext,
    window: WindowHandle<FigureView>,
}

impl Win {
    fn open(path: &Path, render: bool) -> Win {
        Win::open_sized(path, render, W, H)
    }

    fn open_sized(path: &Path, render: bool, w: f32, h: f32) -> Win {
        let text = gpui_platform::current_platform(true).text_system();
        let mut cx = HeadlessAppContext::with_platform(text, Arc::new(()), gpui_platform::current_headless_renderer);
        cx.update(ntex_studio::ui::app::init);
        let t0 = Instant::now();
        let p = path.to_path_buf();
        let window = cx
            .open_window(size(px(w), px(h)), move |window, cx| {
                cx.new(|cx| FigureView::new(FigureOptions { path: p, quiet: true, render, build: render }, window, cx))
            })
            .unwrap();
        let mut w = Win { cx, window };
        w.draw();
        eprintln!("  first frame: {:.1} ms", ms(t0.elapsed()));
        w.draw();
        w
    }

    fn draw(&mut self) {
        self.cx.run_until_parked();
        self.cx.update_window(self.window.into(), |_, window, cx| window.draw(cx).clear(cx)).unwrap();
        self.cx.run_until_parked();
    }

    fn key(&mut self, keys: &str) {
        self.cx
            .update_window(self.window.into(), |_, window, cx| {
                window.dispatch_keystroke(Keystroke::parse(keys).unwrap(), cx);
            })
            .unwrap();
        self.draw();
        self.settle_fields();
    }

    // Await async replies only in the test harness, never in a widget.
    fn settle_fields(&mut self) {
        let start = Instant::now();
        loop {
            self.tick(2);
            let settled = self.with(|v, _, cx| v.label_field.read(cx).settled()
                && v.fields.values().all(|f| f.entity.read(cx).settled()));
            if settled { break; }
            assert!(start.elapsed() < Duration::from_secs(15), "field reply timed out");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    fn typ(&mut self, text: &str) {
        for ch in text.chars() {
            let k = match ch {
                ' ' => "space".to_string(),
                '\n' => "enter".to_string(),
                c if c.is_ascii_uppercase() => format!("shift-{}->{}", c.to_ascii_lowercase(), c),
                c => c.to_string(),
            };
            self.key(&k);
        }
    }

    fn with<R>(&mut self, f: impl FnOnce(&mut FigureView, &mut gpui::Window, &mut gpui::Context<FigureView>) -> R) -> R {
        let r = self.window.update(&mut self.cx, f).unwrap();
        self.draw();
        r
    }

    fn read<R>(&mut self, f: impl FnOnce(&FigureView) -> R) -> R {
        self.window.update(&mut self.cx, |v, _, _| f(v)).unwrap()
    }

    fn tick(&mut self, ms: u64) {
        self.cx.advance_clock(Duration::from_millis(ms));
        self.with(|v, w, cx| v.pump(w, cx));
    }

    fn wait(&mut self, what: &str, timeout: Duration, cond: impl Fn(&FigureView) -> bool) -> bool {
        let start = Instant::now();
        loop {
            self.tick(50);
            if self.read(&cond) {
                eprintln!("  {what}: {:.1}s", start.elapsed().as_secs_f32());
                return true;
            }
            if start.elapsed() > timeout {
                eprintln!("  {what}: gave up after {:.0}s", timeout.as_secs_f32());
                return false;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn shot(&mut self, name: &str) -> PathBuf {
        self.draw();
        let img = self.cx.capture_screenshot(self.window.into()).unwrap();
        let dir = evidence();
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(format!("{name}.png"));
        img.save(&p).unwrap();
        eprintln!("  wrote {}", p.display());
        p
    }

    fn source(&mut self) -> String {
        self.read(|v| v.doc.source.clone())
    }

    /// A scene point (cm) in window px.
    fn px_of(&mut self, p: Pt) -> Point<Pixels> {
        self.window
            .update(&mut self.cx, |v, _, _| {
                v.sync_view();
                let b = v.canvas_bounds.get().expect("the canvas has been drawn");
                v.view.xf(b).to_px(p)
            })
            .unwrap()
    }

    fn anchor(&mut self, obj: &ObjRef) -> Option<Pt> {
        self.read(|v| v.doc.scene.anchors.iter().find(|(o, _, _)| o == obj).map(|(_, p, _)| *p))
    }

    fn event(&mut self, e: PlatformInput) {
        self.cx
            .update_window(self.window.into(), |_, window, cx| {
                window.dispatch_event(e, cx);
            })
            .unwrap();
    }

    fn down(&mut self, at: Point<Pixels>, clicks: usize) {
        self.event(PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::default(),
            click_count: clicks,
            first_mouse: false,
            pen: None,
        }));
        self.draw();
    }

    fn drag_to(&mut self, at: Point<Pixels>) {
        self.event(PlatformInput::MouseMove(MouseMoveEvent { position: at, pressed_button: Some(MouseButton::Left), modifiers: Modifiers::default(), pen: None }));
        self.draw();
    }

    fn up(&mut self, at: Point<Pixels>) {
        self.event(PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: 1, pen: None }));
        self.draw();
    }

    /// Draw a stroke through these scene points, with the mouse.
    fn stroke(&mut self, pts: &[Pt]) {
        let px: Vec<Point<Pixels>> = pts.iter().map(|p| self.px_of(*p)).collect();
        self.down(px[0], 1);
        for p in &px[1..] {
            self.drag_to(*p);
        }
        self.up(*px.last().unwrap());
    }

    /// Replace the shorthand as if typed: set it, then type one space and
    /// delete it (a real Changed from the field).
    fn type_source(&mut self, src: &str) {
        self.with(|v, w, cx| {
            let f = v.shorthand.read(cx).focus.clone();
            f.focus(w, cx);
            v.shorthand.update(cx, |f, cx| f.set_text(src.to_string(), cx));
        });
        self.key("cmd-down");
        self.key("space");
        self.key("backspace");
    }

    fn focus_canvas(&mut self) {
        self.with(|v, w, cx| v.canvas_focus.clone().focus(w, cx));
    }

    // -- direct manipulation helpers ------------------------------------------------

    /// Where a named control was drawn (window px).
    fn ui(&mut self, key: &str) -> Option<gpui::Bounds<Pixels>> {
        self.read(|v| v.ui_bounds.borrow().get(key).cloned())
    }

    fn down_with(&mut self, at: Point<Pixels>, clicks: usize, button: MouseButton, modifiers: Modifiers) {
        self.event(PlatformInput::MouseDown(MouseDownEvent { button, position: at, modifiers, click_count: clicks, first_mouse: false, pen: None }));
        self.draw();
    }

    fn up_with(&mut self, at: Point<Pixels>, button: MouseButton, modifiers: Modifiers) {
        self.event(PlatformInput::MouseUp(MouseUpEvent { button, position: at, modifiers, click_count: 1, pen: None }));
        self.draw();
    }

    fn click(&mut self, at: Point<Pixels>) {
        self.down(at, 1);
        self.up(at);
    }

    fn click_mods(&mut self, at: Point<Pixels>, modifiers: Modifiers) {
        self.down_with(at, 1, MouseButton::Left, modifiers);
        self.up_with(at, MouseButton::Left, modifiers);
    }

    fn double_click(&mut self, at: Point<Pixels>) {
        self.down(at, 1);
        self.up(at);
        self.down(at, 2);
        self.up(at);
    }

    fn right_click(&mut self, at: Point<Pixels>) {
        self.down_with(at, 1, MouseButton::Right, Modifiers::default());
        self.up_with(at, MouseButton::Right, Modifiers::default());
    }

    /// Click the middle of a named control.
    fn click_ui(&mut self, key: &str) -> bool {
        match self.ui(key) {
            Some(b) => {
                self.click(b.center());
                true
            }
            None => false,
        }
    }

    fn hover_at(&mut self, at: Point<Pixels>) {
        self.event(PlatformInput::MouseMove(MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default(), pen: None }));
        self.draw();
    }

    /// A handle of the selection, in window px.
    fn handle_px(&mut self, want: impl Fn(HKind) -> bool) -> Option<Point<Pixels>> {
        self.read(|v| {
            let b = v.canvas_bounds.get()?;
            let xf = v.view.xf(b);
            v.handle_list(xf.scale).into_iter().find(|h| want(h.kind)).map(|h| xf.to_px(h.at))
        })
    }

    /// Press at `from`, move through `path`, release at the last point: the
    /// input-to-frame time of every move (dispatch + layout + paint).
    fn drag_path(&mut self, from: Point<Pixels>, path: &[Point<Pixels>], modifiers: Modifiers, shot_at: Option<(usize, &str)>) -> Vec<f64> {
        self.down_with(from, 1, MouseButton::Left, modifiers);
        let mut frames = Vec::new();
        for (i, to) in path.iter().enumerate() {
            let t0 = Instant::now();
            self.event(PlatformInput::MouseMove(MouseMoveEvent { position: *to, pressed_button: Some(MouseButton::Left), modifiers, pen: None }));
            self.cx.run_until_parked();
            self.cx.update_window(self.window.into(), |_, window, cx| window.draw(cx).clear(cx)).unwrap();
            frames.push(ms(t0.elapsed()));
            if let Some((k, name)) = shot_at
                && k == i
            {
                self.shot(name);
            }
        }
        self.up_with(*path.last().unwrap_or(&from), MouseButton::Left, modifiers);
        frames
    }

    /// The statement line that starts with `prefix` (e.g. "ngon P ").
    fn line(&mut self, prefix: &str) -> String {
        self.source().lines().find(|l| l.trim_start().starts_with(prefix)).unwrap_or("").to_string()
    }

    /// A label's centre (window px), by its object.
    fn label_px(&mut self, obj: &ObjRef) -> Option<Point<Pixels>> {
        let at = self.read(|v| v.doc.scene.draws.iter().find_map(|d| match d {
            Draw::Label { obj: o, at, .. } if o == obj => Some(*at),
            _ => None,
        }))?;
        Some(self.px_of(at))
    }

    fn shape_frame(&mut self, obj: &ObjRef) -> Option<ntex_studio::ui::figure::handles::Frame> {
        self.read(|v| ntex_studio::ui::figure::handles::frame(&v.doc.figure, &v.doc.scene, obj))
    }

    fn undo_all(&mut self) -> usize {
        self.focus_canvas();
        let mut n = 0;
        while self.read(|v| !v.undo.is_empty()) && n < 400 {
            self.key("cmd-z");
            n += 1;
        }
        n
    }
}

fn evidence() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("evidence/figure")
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn stats(name: &str, mut v: Vec<f64>) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p = |q: f64| v[((v.len() as f64 - 1.0) * q).round() as usize];
    eprintln!("  {name}: n={} median {:.2} ms · p95 {:.2} ms · max {:.2} ms", v.len(), p(0.5), p(0.95), v[v.len() - 1]);
    (p(0.5), p(0.95), v[v.len() - 1])
}

struct TempRoot(PathBuf);
impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

fn temp_root() -> PathBuf {
    std::env::temp_dir().join(format!("ntex-figure-renders-{}", std::process::id()))
}

/// A temp NTEX_STUDIO_CONFIG with a copy of templates/tex, a temp state.
fn setup() -> PathBuf {
    if let Ok(rd) = std::fs::read_dir(std::env::temp_dir()) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if let Some(pid) = name.strip_prefix("ntex-figure-renders-").and_then(|p| p.parse::<i32>().ok())
                && pid != std::process::id() as i32
                && unsafe { kill(pid, 0) } != 0
            {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
    let root = temp_root();
    let _ = std::fs::remove_dir_all(&root);
    let config = root.join("texnvim");
    std::fs::create_dir_all(&config).unwrap();
    let real = PathBuf::from(std::env::var_os("HOME").unwrap()).join(".config/texnvim");
    assert!(std::process::Command::new("rsync")
        .args(["-a", "--exclude", "/studio/", "--exclude", ".git"])
        .arg(format!("{}/", real.display())).arg(format!("{}/", config.display()))
        .status().unwrap().success());
    std::fs::create_dir_all(root.join("figs")).unwrap();
    // SAFETY: single-threaded here, before GPUI starts.
    unsafe {
        std::env::set_var("NTEX_STUDIO_CONFIG", &config);
        std::env::set_var("NTEX_STUDIO_STATE", root.join("state"));
        std::env::set_var("TEXNVIM_SNIPPETS", config.join("snippets"));
    }
    root
}

// -- hand-made figures (while the parser is not built) -------------------------

fn node(name: &str, label: &str, c: f32, r: f32) -> Item {
    Item::Node(Node { name: name.into(), label: label.into(), pos: Some(Pos::Cell(c, r)), color: None, span: Span::default() })
}

fn arrow(from: &str, to: &str, label: Option<&str>, head: Head, line: Line) -> Item {
    Item::Arrow(Arrow {
        from: from.into(),
        to: to.into(),
        label: label.map(|s| s.into()),
        head,
        line,
        bend: 0.0,
        is_loop: false,
        side: LabelSide::Auto,
        label_pos: LabelPos::Middle,
        iso: false,
        thick: false,
        color: None,
        span: Span::default(),
    })
}

const SQUARE: &str = "grid\n  A --f--> B\n  |g       |h\n  v        v\n  C --k--> D\nend\n";

fn square_by_hand() -> Figure {
    Figure {
        items: vec![
            node("A", "A", 0.0, 0.0),
            node("B", "B", 1.0, 0.0),
            node("C", "C", 0.0, 1.0),
            node("D", "D", 1.0, 1.0),
            arrow("A", "B", Some("f"), Head::To, Line::Solid),
            arrow("A", "C", Some("g"), Head::To, Line::Solid),
            arrow("B", "D", Some("h"), Head::To, Line::Solid),
            arrow("C", "D", Some("k"), Head::To, Line::Solid),
        ],
        ..Default::default()
    }
}

/// Each kind once (the look check): a kernel picture, a pentagon, a Cayley graph.
const KINDS: &str = "blob G \"G\" {e, g, g^2, g^3}\nblob K \"\\ker\\varphi\" in G shaded {e, g^2}\nblob H \"H\" {e', h}\nG -φ-> H\ng |-> h ; e |-> e'\nngon P 5 axes rotation at (9,0)\n";

fn kinds_by_hand() -> Figure {
    let el = |n: &str| Element { name: n.into(), label: n.into(), pos: None };
    let blob = |name: &str, label: &str, parent: Option<&str>, shaded: bool, els: Vec<Element>| {
        Item::Blob(Blob {
            name: name.into(),
            label: label.into(),
            elements: els,
            parent: parent.map(|s| s.into()),
            shaded,
            color: None,
            shape: BlobShape::Round,
            size: None,
            pos: None,
            label_place: LabelPlace::Above,
            span: Span::default(),
            angle: 0.0,
        })
    };
    Figure {
        items: vec![
            blob("G", "G", None, false, vec![el("g"), el("g^3")]),
            blob("K", "\\ker\\varphi", Some("G"), true, vec![el("e"), el("g^2")]),
            blob("H", "H", None, false, vec![el("e'"), el("h")]),
            arrow("G", "H", Some("\\varphi"), Head::To, Line::Solid),
            arrow("g", "h", None, Head::MapsTo, Line::Solid),
            arrow("e", "e'", None, Head::MapsTo, Line::Solid),
            Item::NGon(NGon {
                name: "P".into(),
                n: 5,
                labels: (1..=5).map(|i| i.to_string()).collect(),
                all_axes: true,
                axes: vec![],
                rotation: 1,
                turn: 0,
                flip: None,
                radius: None,
                pos: Some(Pos::Abs(9.0, 0.0)),
                span: Span::default(),
                angle: 0.0,
                show_labels: true,
                shaded: true,
                color: None,
                rotation_cw: false,
                rotation_edge: None,
                turn_neg: false,
                rotation_label: None,
                axis_labels: vec![],
            }),
        ],
        ..Default::default()
    }
}

/// The real pipeline when it is there, the hand-made figure otherwise.
fn doc_of(src: &str, by_hand: impl FnOnce() -> Figure) -> (Doc, bool) {
    let d = Doc::new(src);
    if !doc::is_down("parse") && !d.figure.items.is_empty() {
        return (d, true);
    }
    let fig = by_hand();
    let scene = doc::scene_of(&fig);
    (Doc::from_parts(src, fig, scene), false)
}

/// Can canvas edits be written back yet? (format returns text, edit applies)
fn core_a_ready() -> bool {
    let fig = Figure { items: vec![node("A", "A", 0.0, 0.0)], ..Default::default() };
    let format_ok = std::panic::catch_unwind(|| !ntex_figures::format::format(&fig).trim().is_empty()).unwrap_or(false);
    let edit_ok = std::panic::catch_unwind(|| {
        let mut f = fig.clone();
        ntex_figures::edit::apply(&mut f, &Edit::MoveNode { name: "A".into(), pos: Pos::Cell(1.0, 0.0) }).is_ok()
    })
    .unwrap_or(false);
    format_ok && edit_ok
}

fn main() {
    let root = setup();
    let _cleanup = TempRoot(root.clone());
    let figs = root.join("figs");
    let mut failures: Vec<String> = Vec::new();
    let mut blocked: Vec<String> = Vec::new();
    let real_templates = PathBuf::from(std::env::var_os("HOME").unwrap()).join(".config/texnvim/templates/tex");
    doc::quiet_todo_panics();
    let edits_ready = core_a_ready();
    eprintln!("canvas edits (format + edit::apply) ready: {edits_ready}");
    let real_before: Vec<(PathBuf, Vec<u8>)> =
        std::fs::read_dir(&real_templates).unwrap().flatten().map(|e| (e.path(), std::fs::read(e.path()).unwrap_or_default())).collect();
    // XCharter is registered (the draft's labels)
    {
        let text = gpui_platform::current_platform(true).text_system();
        let mut cx = HeadlessAppContext::with_platform(text, Arc::new(()), gpui_platform::current_headless_renderer);
        cx.update(ntex_studio::ui::app::init);
        let names = cx.text_system().all_font_names();
        if !names.iter().any(|n| n == "XCharter") {
            failures.push("XCharter is not registered".into());
        }
    }

    // FIG_ONLY=interact: just the direct-manipulation scenarios (iterating)
    if std::env::var("FIG_ONLY").as_deref() == Ok("interact") {
        interaction(&figs, &mut failures, &mut blocked);
        for b in &blocked {
            eprintln!("BLOCKED: {b}");
        }
        for f in &failures {
            eprintln!("FAIL: {f}");
        }
        let ok = failures.is_empty();
        drop(_cleanup);
        if !ok {
            std::process::exit(1);
        }
        println!("figure_renders (interact): ok");
        return;
    }

    if std::env::var("FIG_ONLY").as_deref() == Ok("snippets") {
        snippet_fields(&figs, &mut failures);
        for failure in &failures { eprintln!("FAIL: {failure}"); }
        assert!(failures.is_empty(), "snippet field acceptance failed");
        println!("figure_renders (snippets): ok");
        return;
    }

    // -- 1. an empty figure: a new file from the template --------------------------
    eprintln!("empty figure (new file)");
    let empty = figs.join("empty.fig");
    let mut w = Win::open(&empty, true);
    let text = std::fs::read_to_string(&empty).unwrap_or_default();
    if text != ntex_studio::ui::figure::view::template_for_path(&empty) || !text.starts_with("% Ntex Figures") {
        failures.push(format!("the new file isn't the template:\n{text}"));
    }
    w.shot("01-empty");
    drop(w);

    // -- 2. the commutative square from the shorthand --------------------------------
    eprintln!("commutative square from shorthand");
    let sq = figs.join("square.fig");
    std::fs::write(&sq, "% a square\n").unwrap();
    let mut w = Win::open(&sq, true);
    w.type_source(&format!("% a square\n{SQUARE}"));
    w.tick(200); // the 150 ms re-read
    let parsed = w.read(|v| !v.doc.figure.items.is_empty());
    if !parsed {
        blocked.push("square from shorthand: parse is not built (hand-made figure used)".into());
        let (d, _) = doc_of(SQUARE, square_by_hand);
        w.with(|v, _, cx| v.load_doc(d, cx));
    }
    w.focus_canvas();
    // a small figure opens fitted to the canvas, at most 200 %
    w.tick(20);
    let zoom = w.read(|v| v.view.zoom);
    if !(zoom > 1.2 && zoom <= 2.0) {
        failures.push(format!("the square should open fitted (zoom in (1.2, 2]), not {zoom}"));
    } else {
        eprintln!("  fitted on open: {:.0} % ✓", zoom * 100.0);
    }
    w.shot("02-square-draft");
    let exact = w.wait("exact render", Duration::from_secs(60), |v| v.showing_exact() || v.render.error.is_some() || doc::is_down("tikz::document"));
    let rerr = w.read(|v| v.render.error.clone());
    if doc::is_down("tikz::document") {
        blocked.push("exact render: tikz::document is not built".into());
    } else if !exact || rerr.is_some() {
        failures.push(format!("the exact render didn't arrive: {rerr:?}"));
    } else {
        w.shot("03-square-exact");
    }

    // -- 3. dragging a node: the draft follows, the source says `at …` ---------------
    eprintln!("drag a node");
    let b = w.anchor(&ObjRef::Node("B".into())).expect("node B is placed");
    let cell = w.read(|v| v.cell());
    let from = w.px_of(b);
    w.down(from, 1);
    // input-to-frame: every mouse move → dispatch + update + layout + paint
    let mut frames = Vec::new();
    let steps = 60;
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let to = w.px_of(Pt { x: b.x + cell.0 * t, y: b.y - 0.2 * (t * 6.0).sin() });
        let t0 = Instant::now();
        w.event(PlatformInput::MouseMove(MouseMoveEvent { position: to, pressed_button: Some(MouseButton::Left), modifiers: Modifiers::default(), pen: None }));
        w.cx.run_until_parked();
        w.cx.update_window(w.window.into(), |_, window, cx| window.draw(cx).clear(cx)).unwrap();
        frames.push(ms(t0.elapsed()));
        if i == steps / 2 + 8 {
            w.shot("04-drag-draft");
        }
    }
    let (med, p95, max) = stats("drag: mouse move → frame built (dispatch + layout + paint)", frames);
    let mut gpu = Vec::new();
    for i in 0..20 {
        let to = w.px_of(Pt { x: b.x + cell.0 + 0.3 * ((i % 2) as f32), y: b.y });
        let t0 = Instant::now();
        w.event(PlatformInput::MouseMove(MouseMoveEvent { position: to, pressed_button: Some(MouseButton::Left), modifiers: Modifiers::default(), pen: None }));
        w.cx.run_until_parked();
        let _ = w.cx.capture_screenshot(w.window.into()).unwrap();
        gpu.push(ms(t0.elapsed()));
    }
    stats("drag: mouse move → pixels read back (adds Metal + GPU readback)", gpu);
    if p95 > 16.0 {
        failures.push(format!("drag input-to-frame p95 {p95:.1} ms > 16 ms (median {med:.1}, max {max:.1})"));
    }
    let dest = w.px_of(Pt { x: b.x + cell.0, y: b.y });
    w.up(dest);
    let src = w.source();
    if !edits_ready {
        blocked.push("drag commit: format / edit::apply are not built (the draft moved; the source can't follow yet)".into());
    } else if !(src.contains("B at 2,0") || src.contains("B at 2, 0")) {
        failures.push(format!("after the drag the source should place B `at 2,0`:\n{src}"));
    } else {
        eprintln!("  source: B at 2,0 ✓");
    }
    w.shot("05-drag-done");
    // ⌘Z puts it back
    w.focus_canvas();
    w.key("cmd-z");
    let back = w.source();
    if edits_ready && back.contains("B at 2,0") {
        failures.push("⌘Z didn't undo the move".into());
    }
    drop(w);

    // -- 4. the arrow tool: A to B ------------------------------------------------------
    eprintln!("arrow tool");
    let ab = figs.join("arrow.fig");
    std::fs::write(&ab, "A at 0,0\nB at 1,0\n").unwrap();
    let mut w = Win::open(&ab, false);
    if w.read(|v| v.doc.figure.items.is_empty()) {
        blocked.push("arrow tool: parse is not built (hand-made nodes)".into());
        let fig = Figure { items: vec![node("A", "A", 0.0, 0.0), node("B", "B", 1.0, 0.0)], ..Default::default() };
        let scene = doc::scene_of(&fig);
        w.with(|v, _, cx| v.load_doc(Doc::from_parts("A at 0,0\nB at 1,0\n", fig, scene), cx));
    }
    w.focus_canvas();
    w.key("a");
    if w.read(|v| v.tool) != Tool::Arrow {
        failures.push("A on the canvas didn't pick the arrow tool".into());
    }
    let a = w.anchor(&ObjRef::Node("A".into())).unwrap();
    let b = w.anchor(&ObjRef::Node("B".into())).unwrap();
    let (pa, pb) = (w.px_of(a), w.px_of(b));
    w.down(pa, 1);
    for i in 1..=8 {
        let t = i as f32 / 10.0;
        w.drag_to(point(pa.x + (pb.x - pa.x) * t, pa.y + (pb.y - pa.y) * t - px(12. * (1.0 - (2.0 * t - 1.0).powi(2)))));
    }
    w.shot("06-arrow-dragging");
    w.up(pb);
    let src = w.source();
    if !edits_ready {
        blocked.push("arrow tool commit: edit::apply / format are not built".into());
    } else if !src.contains("A -> B") {
        failures.push(format!("the arrow tool should write `A -> B`:\n{src}"));
    } else {
        eprintln!("  source: A -> B ✓");
        let editing = w.read(|v| v.label_edit.is_some());
        if !editing {
            failures.push("the label field didn't follow the new arrow".into());
        }
        w.shot("07-arrow-label-field");
        w.typ("$φ$");
        w.key("enter");
        let src = w.source();
        if !src.contains("-\\varphi->") && !src.contains("-φ->") {
            failures.push(format!("typing φ in the arrow's label should give `A -\\varphi-> B`:\n{src}"));
        } else {
            eprintln!("  source: labelled ✓");
        }
        // her spelling: φ stays φ in the shorthand and comes back in the field
        if !src.contains("-φ->") {
            failures.push(format!("typing φ should keep φ in the shorthand (spelling):\n{src}"));
        }
        let idx = w.read(|v| v.doc.figure.items.iter().position(|it| matches!(it, Item::Arrow(a) if a.from == "A" && a.to == "B")));
        if let Some(i) = idx {
            let seeded = w.with(|v, win, cx| {
                v.start_label_edit(ObjRef::Arrow(i), win, cx);
                v.label_field.read(cx).text().to_string()
            });
            if seeded != "φ" {
                failures.push(format!("the label field should show φ as typed, not {seeded:?}"));
            }
            // now \varphi, typed out: it stays \varphi
            w.with(|v, _, cx| v.label_field.update(cx, |f, cx| f.set_text("\\varphi".to_string(), cx)));
            w.key("enter");
            let src = w.source();
            if !src.contains("-\\varphi->") {
                failures.push(format!("typing \\varphi should keep \\varphi in the shorthand:\n{src}"));
            }
            let seeded = w.with(|v, win, cx| {
                v.start_label_edit(ObjRef::Arrow(i), win, cx);
                v.label_field.read(cx).text().to_string()
            });
            if seeded != "\\varphi" {
                failures.push(format!("the label field should show \\varphi as typed, not {seeded:?}"));
            }
            w.key("escape");
            eprintln!("  spelling kept (φ, \\varphi) ✓");
        }
    }
    drop(w);

    // -- 5. rough: a loop becomes a blob; a pentagon becomes `ngon … 5` -----------------
    eprintln!("rough tool");
    let rough = figs.join("rough.fig");
    std::fs::write(&rough, "A at 0,0\n").unwrap();
    let mut w = Win::open(&rough, false);
    if w.read(|v| v.doc.figure.items.is_empty()) {
        let fig = Figure { items: vec![node("A", "A", 0.0, 0.0)], ..Default::default() };
        let scene = doc::scene_of(&fig);
        w.with(|v, _, cx| v.load_doc(Doc::from_parts("A at 0,0\n", fig, scene), cx));
    }
    w.focus_canvas();
    w.key("r");
    // a wobbly loop to the right of A (inside the canvas at the fitted 200 %)
    let lp: Vec<Pt> = (0..=40)
        .map(|i| {
            let t = i as f32 / 40.0 * std::f32::consts::TAU;
            // an uneven wobble, as a hand draws it (five even lobes read as a pentagon)
            let wob = 1.0 + 0.06 * (2.0 * t + 0.3).sin() + 0.025 * (5.0 * t).sin();
            Pt { x: 2.3 + 1.6 * wob * t.cos(), y: 1.1 * wob * t.sin() }
        })
        .collect();
    w.stroke(&lp);
    let ghost = w.read(|v| v.ghost.is_some());
    w.shot("08-rough-loop-ghost");
    w.tick(200);
    let src = w.source();
    if doc::is_down("recognize::recognize") {
        blocked.push("rough loop: recognize is not built".into());
    } else if !ghost {
        failures.push("no ghost after a recognised loop".into());
    }
    if !edits_ready {
        blocked.push("rough loop commit: edit::apply / format are not built".into());
    } else if !src.contains("blob") {
        failures.push(format!("the loop should become a blob:\n{src}"));
    } else {
        eprintln!("  source: a blob ✓");
    }
    w.key("escape");
    w.shot("09-rough-blob");
    // a rough pentagon below
    let mut pent: Vec<Pt> = Vec::new();
    let corners: Vec<Pt> = (0..=5)
        .map(|k| {
            let t = std::f32::consts::FRAC_PI_2 + k as f32 / 5.0 * std::f32::consts::TAU;
            Pt { x: 0.0 + 1.2 * t.cos(), y: -3.6 + 1.2 * t.sin() }
        })
        .collect();
    for w2 in corners.windows(2) {
        for j in 0..8 {
            let t = j as f32 / 8.0;
            let jit = 0.03 * ((j * 7) as f32).sin();
            pent.push(Pt { x: w2[0].x + (w2[1].x - w2[0].x) * t + jit, y: w2[0].y + (w2[1].y - w2[0].y) * t - jit });
        }
    }
    pent.push(corners[0]);
    w.stroke(&pent);
    w.shot("10-rough-pentagon-ghost");
    w.tick(200);
    let src = w.source();
    let ngon_line = src.lines().find(|l| l.trim_start().starts_with("ngon")).map(|s| s.to_string());
    if !edits_ready {
        blocked.push("rough pentagon commit: edit::apply / format are not built".into());
    } else if !ngon_line.as_deref().is_some_and(|l| l.split_whitespace().nth(2) == Some("5")) {
        failures.push(format!("the pentagon should become `ngon … 5`:\n{src}"));
    } else {
        eprintln!("  source: {} ✓", ngon_line.unwrap());
    }
    w.shot("11-rough-pentagon");
    drop(w);

    // -- 6. a parse error ------------------------------------------------------------
    eprintln!("parse error");
    let bad = figs.join("bad.fig");
    std::fs::write(&bad, "A -> B\n").unwrap();
    let mut w = Win::open(&bad, false);
    w.type_source("A -> B\nB ->> \nblob {\nC -f-> A\n");
    w.tick(200);
    let diags = w.read(|v| v.doc.diagnostics.iter().map(|d| d.message.clone()).collect::<Vec<_>>());
    eprintln!("  diagnostics: {diags:?}");
    if doc::is_down("parse") {
        blocked.push("parse error: parse is not built (the note says so)".into());
    } else if !diags.iter().any(|d| d.contains("line 2") || d.contains("line 3")) {
        failures.push(format!("a bad line should be reported as `line N: …`: {diags:?}"));
    }
    w.shot("12-parse-error");
    drop(w);

    // -- 7. the Ask box: running (stub job), done, fast ----------------------------------
    eprintln!("ask box");
    let askf = figs.join("ask.fig");
    std::fs::write(&askf, SQUARE).unwrap();
    let mut w = Win::open(&askf, false);
    if w.read(|v| v.doc.figure.items.is_empty()) {
        let (d, _) = doc_of(SQUARE, square_by_hand);
        w.with(|v, _, cx| v.load_doc(d, cx));
    }
    let before = w.source();
    let (m0, fast0) = w.read(|v| (v.ask.model(), v.ask.fast));
    if m0.model != "claude-sonnet-5-5" || m0.effort != "low" || fast0 {
        failures.push(format!("the Ask default should be Sonnet, low effort, fast off (prefs defaults): {m0:?}, fast {fast0}"));
    }
    w.with(|v, w, cx| {
        let f = v.ask_field.read(cx).focus.clone();
        f.focus(w, cx);
    });
    w.typ("make the diagonal dashed");
    let (tx, rx) = std::sync::mpsc::channel();
    let model = w.read(|v| v.ask.model());
    w.with(|v, _, cx| v.ask_start_with(None, rx, model, cx));
    tx.send(AskEvent::Progress("Asking Sonnet…".into())).unwrap();
    w.tick(16);
    w.cx.advance_clock(Duration::from_millis(1800));
    w.tick(16);
    w.shot("13-ask-running");
    let after = format!("{SQUARE}A --> D\n");
    tx.send(AskEvent::Done { source: after.clone(), summary: "Added a dashed diagonal A → D.".into(), seconds: 3.1 }).unwrap();
    w.tick(16);
    let (src, toast) = w.read(|v| (v.doc.source.clone(), v.toast.as_ref().map(|t| t.text.to_string())));
    if src != after {
        failures.push("the Ask result didn't replace the source".into());
    }
    if toast.as_deref() != Some("Done in 3.1 s — ⌘Z undoes.") {
        failures.push(format!("Ask toast: {toast:?}"));
    }
    w.shot("14-ask-done");
    w.focus_canvas();
    w.key("cmd-z");
    if w.source() != before {
        failures.push("⌘Z after Ask should restore the source in one step".into());
    }
    // the fast toggle
    w.with(|v, _, cx| {
        v.ask.fast = true;
        cx.notify();
    });
    let m: AskModel = w.read(|v| v.ask.model());
    if m.model != "gpt-6-luna" {
        failures.push(format!("fast should ask gpt-6-luna, not {}", m.model));
    }
    w.with(|v, w, cx| {
        let f = v.ask_field.read(cx).focus.clone();
        f.focus(w, cx);
    });
    w.shot("15-ask-fast");
    // the model menu
    w.with(|v, _, cx| {
        v.ask.fast = false;
        v.ask.menu_open = true;
        cx.notify();
    });
    w.shot("16-ask-menu");
    drop(w);

    // -- 8. saving: autosave 1 s after an edit, reload on an outside change, ask on a conflict
    eprintln!("saving and the file on disk");
    let sv = figs.join("save.fig");
    std::fs::write(&sv, "A at 0,0\n").unwrap();
    let mut w = Win::open(&sv, false);
    w.type_source("A at 0,0\nB at 1,0\nA -> B\n");
    w.tick(200);
    let early = std::fs::read_to_string(&sv).unwrap_or_default();
    w.tick(500);
    let mid = std::fs::read_to_string(&sv).unwrap_or_default();
    w.tick(600);
    let late = std::fs::read_to_string(&sv).unwrap_or_default();
    if early.contains("A -> B") || mid.contains("A -> B") || !late.contains("A -> B") {
        failures.push(format!("autosave should write 1 s after the last edit (0.2 s: {}, 0.7 s: {}, 1.3 s: {})", early.contains("A -> B"), mid.contains("A -> B"), late.contains("A -> B")));
    } else {
        eprintln!("  autosaved after 1 s ✓");
    }
    // edited in Neovim, no unsaved edits here: reload
    std::thread::sleep(Duration::from_millis(1100)); // a new mtime
    std::fs::write(&sv, "A at 0,0\nB at 1,0\nA ->> B\n").unwrap();
    w.tick(600);
    if !w.source().contains("A ->> B") {
        failures.push("an outside change with no unsaved edits should reload".into());
    } else {
        eprintln!("  reloaded ✓");
    }
    w.tick(100);
    w.shot("17-reloaded");
    // now with unsaved edits: ask
    w.type_source("A at 0,0\nB at 1,0\nA ->> B\nB -> A\n");
    w.tick(200);
    std::thread::sleep(Duration::from_millis(1100));
    std::fs::write(&sv, "A at 0,0\nB at 1,0\nA >-> B\n").unwrap();
    w.tick(600);
    let conflict = w.read(|v| v.disk.conflict.is_some());
    let kept = w.source().contains("B -> A");
    if !conflict || !kept {
        failures.push(format!("an outside change over unsaved edits should ask (conflict {conflict}, kept {kept})"));
    } else {
        eprintln!("  conflict asked ✓");
    }
    w.focus_canvas();
    w.shot("18-conflict");
    w.with(|v, _, cx| v.resolve_conflict(true, cx));
    if !w.source().contains("A >-> B") {
        failures.push("Reload it should take the file's text".into());
    }
    w.key("cmd-z");
    if !w.source().contains("B -> A") {
        failures.push("⌘Z after Reload it should bring the edits back".into());
    }
    drop(w);

    // -- 9. each kind, for the look ----------------------------------------------------
    eprintln!("kinds");
    let kf = figs.join("kinds.fig");
    std::fs::write(&kf, KINDS).unwrap();
    let mut w = Win::open(&kf, true);
    if w.read(|v| v.doc.figure.items.is_empty()) {
        let (d, _) = doc_of(KINDS, kinds_by_hand);
        w.with(|v, _, cx| v.load_doc(d, cx));
    }
    w.focus_canvas();
    w.shot("19-kinds-draft");
    // the worst case for dragging: a blob (absolute position, so every move
    // lays the whole figure out again, blobs, n-gon and all)
    if let Some(h) = w.read(|v| ntex_studio::ui::figure::geom::center_of(&v.doc.scene, &ObjRef::Blob("H".into()))) {
        let from = w.px_of(h);
        w.down(from, 1);
        let mut frames = Vec::new();
        let mut relayouts = 0;
        for i in 1..=60 {
            let to = point(from.x + px(i as f32 * 2.5), from.y + px((i as f32 * 0.3).sin() * 20.0));
            let before = w.read(|v| v.drag_scene.as_ref().map(|s| Arc::as_ptr(s) as usize));
            let t0 = Instant::now();
            w.event(PlatformInput::MouseMove(MouseMoveEvent { position: to, pressed_button: Some(MouseButton::Left), modifiers: Modifiers::default(), pen: None }));
            w.cx.run_until_parked();
            w.cx.update_window(w.window.into(), |_, window, cx| window.draw(cx).clear(cx)).unwrap();
            frames.push(ms(t0.elapsed()));
            let after = w.read(|v| v.drag_scene.as_ref().map(|s| Arc::as_ptr(s) as usize));
            if before != after {
                relayouts += 1;
            }
        }
        eprintln!("  ({relayouts} of 60 moves laid the figure out again)");
        let (_, p95, _) = stats("drag a blob in the kinds figure: mouse move → frame built", frames);
        if p95 > 16.0 {
            failures.push(format!("blob drag input-to-frame p95 {p95:.1} ms > 16 ms"));
        }
        w.shot("19b-kinds-dragging-blob");
        w.key("escape");
        w.up(from);
    }
    if !doc::is_down("tikz::document") && w.wait("kinds exact", Duration::from_secs(60), |v| v.showing_exact() || v.render.error.is_some()) {
        w.shot("20-kinds-exact");
        w.key("cmd-=");
        w.key("cmd-=");
        w.wait("zoomed render", Duration::from_secs(60), |v| v.showing_exact() && !v.render.busy());
        w.shot("21-kinds-zoomed");
    }
    drop(w);

    // -- 10. direct manipulation: her complaint, step by step ------------------------
    interaction(&figs, &mut failures, &mut blocked);
    snippet_fields(&figs, &mut failures);

    // never the real files
    let real_after: Vec<(PathBuf, Vec<u8>)> =
        std::fs::read_dir(&real_templates).unwrap().flatten().map(|e| (e.path(), std::fs::read(e.path()).unwrap_or_default())).collect();
    if real_before != real_after {
        failures.push("the real templates changed!".into());
    }

    let missing = doc::missing();
    if !missing.is_empty() {
        eprintln!("figures pieces still todo!(): {missing:?}");
    }
    for b in &blocked {
        eprintln!("BLOCKED: {b}");
    }
    if failures.is_empty() {
        println!("figure_renders: ok{}", if blocked.is_empty() { String::new() } else { format!(" ({} blocked on CORE)", blocked.len()) });
    } else {
        for f in &failures {
            eprintln!("FAIL: {f}");
        }
        drop(_cleanup);
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------------------
// 10. direct manipulation (FIGURES.md "Direct manipulation"): her complaint,
// "no way to rotate these shapes or remove labels or change the labels or
// even resize them", played with the mouse and keys, the source checked
// after every step.

const SQUARES: &str = "ngon P 4\nngon Q 4\nngon R 4\nngon S 4\n";
const WIDE: f32 = 1180.0;

fn check(failures: &mut Vec<String>, ok: bool, what: &str, detail: impl FnOnce() -> String) {
    if ok {
        eprintln!("  ✓ {what}");
    } else {
        let d = detail();
        eprintln!("  ✗ {what}: {d}");
        failures.push(format!("{what}: {d}"));
    }
}

fn centre_px(w: &mut Win, obj: &ObjRef) -> Point<Pixels> {
    let c = w.shape_frame(obj).map(|f| f.centre).or_else(|| w.anchor(obj)).expect("the object is drawn");
    w.px_of(c)
}

fn arc(c: Point<Pixels>, r: f32, from_deg: f32, to_deg: f32, steps: usize) -> Vec<Point<Pixels>> {
    (1..=steps)
        .map(|i| {
            let a = (from_deg + (to_deg - from_deg) * i as f32 / steps as f32).to_radians();
            point(c.x + px(r * a.cos()), c.y - px(r * a.sin()))
        })
        .collect()
}

fn line_to(a: Point<Pixels>, b: Point<Pixels>, steps: usize) -> Vec<Point<Pixels>> {
    (1..=steps).map(|i| {
        let t = i as f32 / steps as f32;
        point(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
    }).collect()
}

fn interaction(figs: &Path, failures: &mut Vec<String>, blocked: &mut Vec<String>) {
    let ready = !std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("figures/src/edit.rs")).unwrap_or_default().contains("not implemented yet");
    if !ready {
        blocked.push("interaction: SetItem / MoveObj / Duplicate are not built (FIG-MODEL)".into());
    }
    let mut drag_frames: Vec<(String, Vec<f64>)> = Vec::new();
    let none = Modifiers::default();

    // -- the four squares --------------------------------------------------------
    eprintln!("direct manipulation: four squares");
    let sqf = figs.join("squares.fig");
    std::fs::write(&sqf, SQUARES).unwrap();
    let mut w = Win::open_sized(&sqf, true, WIDE, 900.0);
    w.focus_canvas();
    w.tick(20);
    let start = w.source();
    check(failures, w.read(|v| v.doc.figure.items.iter().filter(|i| matches!(i, Item::NGon(_))).count()) == 4, "four squares parsed", || start.clone());
    let shown = w.with(|v, win, _| v.panel_shown(win));
    check(failures, shown, "the panel shows at 1180 px", || "hidden".into());
    w.shot("interact-01-four-squares");

    let (p, q, r, s4) = (ObjRef::NGon("P".into()), ObjRef::NGon("Q".into()), ObjRef::NGon("R".into()), ObjRef::NGon("S".into()));
    // hover: a faint outline on what a click would take
    let pc = centre_px(&mut w, &p);
    w.hover_at(pc);
    let hov = w.read(|v| v.hover.clone());
    check(failures, hov == Some(p.clone()), "hovering a square outlines it", || format!("{hov:?}"));
    w.shot("interact-02-hover");

    // 1. select P
    w.click(pc);
    let sel = w.read(|v| v.selection.clone());
    check(failures, sel == vec![p.clone()], "a click selects the square", || format!("{sel:?}"));
    w.shot("interact-03-selected");

    // 2. rotate it 45° by the knob
    let knob = w.handle_px(|k| k == HKind::Rotate);
    check(failures, knob.is_some(), "the selected square has a ↻ knob", || "none".into());
    if let Some(knob) = knob {
        let rad = ((f32::from(knob.x - pc.x)).hypot(f32::from(knob.y - pc.y))).max(10.0);
        let path = arc(pc, rad, 90.0, 135.0, 40);
        let f = w.drag_path(knob, &path, none, Some((30, "interact-04-rotating")));
        drag_frames.push(("rotate a square (knob)".into(), f));
        let l = w.line("ngon P ");
        check(failures, l.contains("angle=45"), "dragging the knob 45° writes angle=45", || l.clone());
    }

    // 3. resize by the circumcircle dot: radius 1.4
    let dot = w.handle_px(|k| k == HKind::Radius);
    check(failures, dot.is_some(), "the square has a resize dot", || "none".into());
    if let Some(dot) = dot {
        let pc = centre_px(&mut w, &p);
        let (dx, dy) = (f32::from(dot.x - pc.x), f32::from(dot.y - pc.y));
        let l = dx.hypot(dy).max(1.0);
        let scale = w.read(|v| v.view.xf(v.canvas_bounds.get().unwrap()).scale);
        let target = point(pc.x + px(dx / l * 1.4 * scale), pc.y + px(dy / l * 1.4 * scale));
        let f = w.drag_path(dot, &line_to(dot, target, 30), none, Some((25, "interact-05-resizing")));
        drag_frames.push(("resize a square (dot)".into(), f));
        let l = w.line("ngon P ");
        check(failures, l.contains("radius=1.4"), "dragging the dot to 1.4 cm writes radius=1.4", || l.clone());
    }

    // 4. hide its labels from the panel
    let clicked = w.click_ui("check.ngon.P.labels");
    let l = w.line("ngon P ");
    check(failures, clicked && l.contains("labels=none"), "the panel's Labels box hides them: labels=none", || format!("clicked {clicked}: {l}"));
    w.shot("interact-06-labels-hidden");

    // 5a. rename vertex 1 of Q to `a` with a double-click on the label
    let lq = w.label_px(&ObjRef::NGonVertex("Q".into(), 1));
    if let Some(lq) = lq {
        w.double_click(lq);
        let editing = w.read(|v| v.label_edit.as_ref().map(|l| l.obj.clone()));
        check(failures, editing == Some(ObjRef::NGonVertex("Q".into(), 1)), "a double-click on vertex label 1 edits it in place", || format!("{editing:?}"));
        w.with(|v, _, cx| v.label_field.update(cx, |f, cx| f.select_all(cx)));
        w.typ("$a$");
        w.shot("interact-07-label-field");
        w.key("enter");
        let l = w.line("ngon Q ");
        check(failures, l.contains("a,2,3,4"), "typing a renames vertex 1: labels a,2,3,4", || l.clone());
    } else {
        check(failures, false, "Q's vertex label 1 is drawn", || "no label".into());
    }

    // 5b. … and from the panel's field, on R
    let rc = centre_px(&mut w, &r);
    w.click(rc);
    let field_ok = w.click_ui("field.ngon.R.label.0");
    let focused = w.with(|v, win, cx| v.fields.get("ngon.R.label.0").is_some_and(|f| f.entity.read(cx).focus.is_focused(win)));
    check(failures, field_ok && focused, "a click puts the caret in the panel's vertex-1 field", || format!("clicked {field_ok}, focused {focused}"));
    w.key("cmd-a");
    w.typ("$a$");
    let l = w.line("ngon R ");
    check(failures, l.contains("a,2,3,4"), "typing a in the panel renames R's vertex 1 live", || l.clone());
    w.shot("interact-08-panel-rename");
    w.key("enter");

    // 6. delete one vertex label (S, vertex 2)
    let ls = w.label_px(&ObjRef::NGonVertex("S".into(), 2));
    if let Some(ls) = ls {
        w.click(ls);
        let sel = w.read(|v| v.selection.clone());
        check(failures, sel == vec![ObjRef::NGonVertex("S".into(), 2)], "a click on a vertex label selects just that label", || format!("{sel:?}"));
        w.shot("interact-09-vertex-label");
        w.key("backspace");
        let l = w.line("ngon S ");
        check(failures, l.contains("1,,3,4"), "⌫ on it hides that one label: labels 1,,3,4", || l.clone());
    }

    // 7. ⌘D duplicates, ⌫ deletes
    let pc = centre_px(&mut w, &p);
    w.click(pc);
    w.key("cmd-d");
    let n = w.read(|v| v.doc.figure.items.iter().filter(|i| matches!(i, Item::NGon(_))).count());
    check(failures, n == 5, "⌘D duplicates the square", || format!("{n} n-gons\n{}", w.source()));
    w.shot("interact-10-duplicated");
    w.key("backspace");
    let n = w.read(|v| v.doc.figure.items.iter().filter(|i| matches!(i, Item::NGon(_))).count());
    check(failures, n == 4, "⌫ deletes the copy", || format!("{n} n-gons"));

    // 8. the keys on Q: ] turns, = grows, L labels, → nudges
    let qc = centre_px(&mut w, &q);
    w.click(qc);
    w.key("]");
    let l = w.line("ngon Q ");
    check(failures, l.contains("angle=-15"), "] turns it 15° clockwise", || l.clone());
    w.key("=");
    let l = w.line("ngon Q ");
    check(failures, l.contains("radius=1.2"), "= grows it 10 % (radius=1.2)", || l.clone());
    w.key("l");
    let l = w.line("ngon Q ");
    check(failures, l.contains("labels=none"), "L hides its labels", || l.clone());
    w.key("l");
    w.key("right");
    let l = w.line("ngon Q ");
    check(failures, l.contains(" at "), "→ nudges it (it gets an `at`)", || l.clone());

    // 9. the right-click menu, and Colour ▸
    let rc = centre_px(&mut w, &r);
    w.right_click(rc);
    let open = w.read(|v| v.menu.is_some());
    check(failures, open, "right-click opens the menu", || "no menu".into());
    w.shot("interact-11-menu");
    w.click_ui("menu.Colour");
    w.shot("interact-12-menu-colour");
    let picked = w.click_ui("menu.colour.maroon");
    let l = w.line("ngon R ");
    check(failures, picked && l.contains("color=maroon"), "Colour ▸ maroon colours it", || format!("{picked}: {l}"));

    // 10. ⌘/ the reference
    w.focus_canvas();
    w.key("cmd-/");
    let on = w.read(|v| v.reference_open);
    check(failures, on, "⌘/ opens the reference", || "closed".into());
    w.shot("interact-13-reference");
    w.key("cmd-/");

    // 11. the marquee, ⇧-click, ⌘A, esc
    let b = w.read(|v| v.canvas_bounds.get().unwrap());
    let from = point(b.origin.x + px(8.), b.origin.y + px(8.));
    let qc = centre_px(&mut w, &q);
    let to = point(qc.x + px(12.), b.bottom() - px(12.));
    let f = w.drag_path(from, &line_to(from, to, 20), none, Some((19, "interact-14-marquee")));
    let _ = f;
    let sel = w.read(|v| v.selection.clone());
    check(failures, sel.contains(&p) && sel.contains(&q) && !sel.contains(&r), "a marquee over P and Q selects both", || format!("{sel:?}"));
    w.shot("interact-15-several");
    let sc = centre_px(&mut w, &s4);
    w.click_mods(sc, Modifiers { shift: true, ..Default::default() });
    let sel = w.read(|v| v.selection.clone());
    check(failures, sel.len() == 3 && sel.contains(&s4), "⇧-click adds S", || format!("{sel:?}"));
    w.key("cmd-a");
    let n = w.read(|v| v.selection.len());
    check(failures, n == 4, "⌘A selects all four", || format!("{n}"));
    w.key("escape");
    let n = w.read(|v| v.selection.len());
    check(failures, n == 0, "esc deselects", || format!("{n}"));

    // the exact render after all that sits where the draft does
    if w.wait("exact render (squares)", Duration::from_secs(60), |v| v.showing_exact() || v.render.error.is_some()) {
        w.shot("interact-16-exact");
    }

    // 12. undo all the way back
    let steps = w.undo_all();
    let back = w.source();
    check(failures, back == start, &format!("⌘Z ×{steps} goes back to the start"), || back.clone());
    drop(w);

    // -- an arrow: bend it, swap its label ---------------------------------------------
    eprintln!("direct manipulation: an arrow");
    let af = figs.join("bend.fig");
    let asrc = "A at 0,0\nB at 2,0\nA -f-> B\n";
    std::fs::write(&af, asrc).unwrap();
    let mut w = Win::open_sized(&af, false, WIDE, 900.0);
    w.focus_canvas();
    let i = w.read(|v| v.doc.figure.items.iter().position(|it| matches!(it, Item::Arrow(_)))).unwrap_or(2);
    let mid = w.read(|v| ntex_studio::ui::figure::geom::path_mid(&v.doc.scene, &ObjRef::Arrow(i))).unwrap();
    let mp = w.px_of(mid);
    w.click(mp);
    let sel = w.read(|v| v.selection.clone());
    check(failures, sel == vec![ObjRef::Arrow(i)], "a click on the arrow selects it", || format!("{sel:?}"));
    w.shot("interact-20-arrow");
    if let Some(h) = w.handle_px(|k| k == HKind::Bend) {
        let (a, b) = (w.anchor(&ObjRef::Node("A".into())).unwrap(), w.anchor(&ObjRef::Node("B".into())).unwrap());
        let d = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
        let lift = 0.2936 * d * 0.5; // sin 30°
        let target = w.px_of(Pt { x: (a.x + b.x) / 2.0, y: (a.y + b.y) / 2.0 + lift });
        let f = w.drag_path(h, &line_to(h, target, 30), none, Some((28, "interact-21-bending")));
        drag_frames.push(("bend an arrow".into(), f));
        let l = w.line("A -");
        check(failures, l.contains("bend"), "dragging the middle up bends it (30°)", || l.clone());
    } else {
        check(failures, false, "the arrow has a bend handle", || "none".into());
    }
    if let Some(h) = w.handle_px(|k| k == HKind::ArrowLabel) {
        let a = w.anchor(&ObjRef::Node("A".into())).unwrap();
        let b = w.anchor(&ObjRef::Node("B".into())).unwrap();
        let below = w.px_of(Pt { x: (a.x + b.x) / 2.0, y: (a.y + b.y) / 2.0 - 0.6 });
        w.drag_path(h, &line_to(h, below, 16), none, None);
        let l = w.line("A -");
        check(failures, l.contains("swap"), "dragging its label across the arrow swaps it", || l.clone());
        w.shot("interact-22-arrow-swapped");
    }
    let steps = w.undo_all();
    let back = w.source();
    check(failures, back == asrc, &format!("arrow: ⌘Z ×{steps} goes back"), || back.clone());
    drop(w);

    // -- a blob: resize it, add an element ---------------------------------------------
    eprintln!("direct manipulation: a blob");
    let bf = figs.join("blob.fig");
    let bsrc = "blob G \"G\" {e, g}\n";
    std::fs::write(&bf, bsrc).unwrap();
    let mut w = Win::open_sized(&bf, false, WIDE, 900.0);
    w.focus_canvas();
    let g = ObjRef::Blob("G".into());
    let fr = w.shape_frame(&g).expect("the blob has a frame");
    let inside = w.px_of(Pt { x: fr.centre.x - fr.half.0 * 0.7, y: fr.centre.y - fr.half.1 * 0.45 });
    w.click(inside);
    let sel = w.read(|v| v.selection.clone());
    check(failures, sel == vec![g.clone()], "a click inside the blob selects it", || format!("{sel:?}"));
    let n_handles = w.read(|v| v.handle_list(40.0).len());
    check(failures, n_handles == 9, "the blob has 8 resize handles and a knob", || format!("{n_handles}"));
    w.shot("interact-30-blob");
    if let Some(h) = w.handle_px(|k| k == HKind::Corner(1, -1)) {
        let to = point(h.x + px(70.), h.y + px(40.));
        let f = w.drag_path(h, &line_to(h, to, 30), none, Some((25, "interact-31-blob-resizing")));
        drag_frames.push(("resize a blob (corner)".into(), f));
        let l = w.line("blob G");
        check(failures, l.contains("size="), "dragging a corner writes size=", || l.clone());
    }
    let added = w.click_ui("button.blob.G.add");
    let l = w.line("blob G");
    check(failures, added && l.contains("x}") || l.contains(", x"), "+ element in the panel adds one", || format!("{added}: {l}"));
    w.shot("interact-32-blob-element-added");
    if let Some(e) = w.anchor(&ObjRef::Element("e".into())) {
        let ep = w.px_of(e);
        w.click(ep);
        let sel = w.read(|v| v.selection.clone());
        check(failures, sel == vec![ObjRef::Element("e".into())], "a click on an element selects it", || format!("{sel:?}"));
        w.shot("interact-33-element");
    }
    let steps = w.undo_all();
    let back = w.source();
    check(failures, back == bsrc, &format!("blob: ⌘Z ×{steps} goes back"), || back.clone());
    drop(w);

    // -- a Cayley graph: rotate it, resize it, a second click into a vertex ----------------
    eprintln!("direct manipulation: a Cayley graph");
    let cf = figs.join("cayley.fig");
    let csrc = "cayley D4 <r, s>\n";
    std::fs::write(&cf, csrc).unwrap();
    let mut w = Win::open_sized(&cf, false, WIDE, 900.0);
    w.focus_canvas();
    let cname = w.read(|v| v.doc.figure.items.iter().find_map(|it| if let Item::Cayley(c) = it { Some(c.name.clone()) } else { None })).unwrap_or_default();
    let cg = ObjRef::Cayley(cname.clone());
    let vtx = w.read(|v| v.doc.scene.anchors.iter().find_map(|(o, p, _)| matches!(o, ObjRef::CayleyVertex(..)).then(|| (o.clone(), *p))));
    if let Some((vo, vp)) = vtx {
        let vpx = w.px_of(vp);
        w.click(vpx);
        let sel = w.read(|v| v.selection.clone());
        check(failures, sel == vec![cg.clone()], "a click on a Cayley vertex selects the whole graph", || format!("{sel:?}"));
        w.shot("interact-40-cayley");
        if let Some(knob) = w.handle_px(|k| k == HKind::Rotate) {
            let c = centre_px(&mut w, &cg);
            let rad = f32::from(knob.x - c.x).hypot(f32::from(knob.y - c.y));
            let f = w.drag_path(knob, &arc(c, rad, 90.0, 120.0, 30), none, Some((20, "interact-41-cayley-rotating")));
            drag_frames.push(("rotate a Cayley graph".into(), f));
            let l = w.line("cayley");
            check(failures, l.contains("angle=30"), "turning the knob 30° writes angle=30", || l.clone());
        }
        let has_dot = w.handle_px(|k| k == HKind::Scale).is_some();
        check(failures, has_dot, "D4 has a resize dot", || "none".into());
        if let Some(h) = w.handle_px(|k| k == HKind::Scale) {
            let c = centre_px(&mut w, &cg);
            let to = point(c.x + (h.x - c.x) * 1.3, c.y + (h.y - c.y) * 1.3);
            let f = w.drag_path(h, &line_to(h, to, 20), none, None);
            drag_frames.push(("resize a Cayley graph".into(), f));
            let l = w.line("cayley");
            check(failures, l.contains("radius="), "dragging its dot outward writes radius=", || l.clone());
        }
        // a second click goes into the vertex
        let vp2 = w.read(|v| v.doc.scene.anchors.iter().find_map(|(o, p, _)| (*o == vo).then_some(*p)));
        if let Some(vp2) = vp2 {
            let at = w.px_of(vp2);
            w.click(at);
            let sel = w.read(|v| v.selection.clone());
            check(failures, sel == vec![vo.clone()], "a second click selects the vertex", || format!("{sel:?}"));
            w.shot("interact-42-cayley-vertex");
        }
    }
    let steps = w.undo_all();
    let back = w.source();
    check(failures, back == csrc, &format!("cayley: ⌘Z ×{steps} goes back"), || back.clone());
    drop(w);

    // A4: a set radius would trade its truncated tetrahedron for a circle, so no resize dot
    let a4 = figs.join("a4.fig");
    std::fs::write(&a4, "cayley A4 <a, b>\n").unwrap();
    let mut w = Win::open_sized(&a4, false, WIDE, 900.0);
    w.focus_canvas();
    if let Some(vp) = w.read(|v| v.doc.scene.anchors.iter().find_map(|(o, p, _)| matches!(o, ObjRef::CayleyVertex(..)).then_some(*p))) {
        let at = w.px_of(vp);
        w.click(at);
        let kinds: Vec<HKind> = w.read(|v| v.handle_list(40.0).into_iter().map(|h| h.kind).collect());
        check(failures, kinds == vec![HKind::Rotate], "A4 keeps its layout: a knob, no resize dot", || format!("{kinds:?}"));
        w.shot("interact-43-a4-no-resize");
    }
    drop(w);

    // -- a node and a text: their panels ----------------------------------------------
    eprintln!("direct manipulation: node and text panels");
    let nf = figs.join("node.fig");
    let nsrc = "A at 0,0\nB at 1,0\nA -φ-> B\ntext \"kernel ↦ identity\" at (0,-1.2)\n";
    std::fs::write(&nf, nsrc).unwrap();
    let mut w = Win::open_sized(&nf, false, WIDE, 900.0);
    w.focus_canvas();
    let a = w.anchor(&ObjRef::Node("A".into())).unwrap();
    let ap = w.px_of(a);
    w.click(ap);
    w.shot("interact-50-node");
    let ti = w.read(|v| v.doc.figure.items.iter().position(|it| matches!(it, Item::Text(_))));
    if let Some(ti) = ti
        && let Some(tp) = w.label_px(&ObjRef::Text(ti))
    {
        w.click(tp);
        let sel = w.read(|v| v.selection.clone());
        check(failures, sel == vec![ObjRef::Text(ti)], "a click on the text selects it", || format!("{sel:?}"));
        w.shot("interact-51-text");
    }
    // ↵ on the node edits its label; the panel's field mirrors it
    w.click(ap);
    w.key("enter");
    let editing = w.read(|v| v.label_edit.is_some());
    check(failures, editing, "↵ on a node edits its label", || "no field".into());
    w.key("escape");
    drop(w);

    // -- a narrow window: the panel folds away; ⌘I brings it back without a jump --------
    eprintln!("direct manipulation: narrow window");
    let mut w = Win::open_sized(&sqf, false, 900.0, 900.0);
    w.focus_canvas();
    let shown = w.with(|v, win, _| v.panel_shown(win));
    check(failures, !shown, "below 1000 px the panel folds away", || "shown".into());
    let pc0 = centre_px(&mut w, &p);
    w.shot("interact-60-narrow");
    w.key("cmd-i");
    let shown = w.with(|v, win, _| v.panel_shown(win));
    let pc1 = centre_px(&mut w, &p);
    let jump = f32::from(pc1.x - pc0.x).hypot(f32::from(pc1.y - pc0.y));
    check(failures, shown && jump < 1.0, "⌘I opens it and the figure doesn't move", || format!("shown {shown}, moved {jump:.1} px"));
    w.click(pc1);
    w.shot("interact-61-narrow-panel");
    let pc2 = centre_px(&mut w, &p);
    let jump = f32::from(pc2.x - pc1.x).hypot(f32::from(pc2.y - pc1.y));
    check(failures, jump < 1.0, "selecting doesn't move the figure", || format!("moved {jump:.1} px"));
    drop(w);

    // -- opening a file scrolls the shorthand to its first line that isn't a comment ---------
    eprintln!("direct manipulation: opening at the code");
    let lf = figs.join("long.fig");
    let mut long = String::new();
    for i in 0..30 {
        long.push_str(&format!("% comment line {i}\n"));
    }
    long.push_str("A -> B\n");
    std::fs::write(&lf, &long).unwrap();
    let mut w = Win::open_sized(&lf, false, WIDE, 900.0);
    let y = w.read(|v| f32::from(v.shorthand_scroll.offset().y));
    check(failures, y <= -400.0, "the shorthand box opens at `A -> B`, past 30 comment lines", || format!("offset {y}"));
    w.shot("interact-70-opened-at-code");
    drop(w);

    // -- round 2 (the usability adversary's findings) ---------------------------------------
    eprintln!("direct manipulation: round 2");
    let gf = figs.join("grid2.fig");
    let gsrc = "grid\n  A --f--> B\n  |g       |h\n  v        v\n  C --k--> D\nend\n";
    std::fs::write(&gf, gsrc).unwrap();
    let mut w = Win::open_sized(&gf, false, WIDE, 900.0);
    w.focus_canvas();
    // a double-click on an arrow's label edits it, even once the arrow is selected
    let i = w.read(|v| v.doc.figure.items.iter().position(|it| matches!(it, Item::Arrow(a) if a.from == "A" && a.to == "B"))).unwrap();
    let lp = w.label_px(&ObjRef::Arrow(i)).unwrap();
    w.click(lp);
    w.double_click(lp);
    let editing = w.read(|v| v.label_edit.as_ref().map(|l| l.obj.clone()));
    check(failures, editing == Some(ObjRef::Arrow(i)), "a double-click on a selected arrow's label edits it", || format!("{editing:?}"));
    w.with(|v, _, cx| v.label_field.update(cx, |f, cx| f.select_all(cx)));
    w.typ("$φ$");
    w.key("enter");
    let src = w.source();
    check(failures, src.contains("A --φ--> B") && src.contains("\nend"), "relabelling a grid arrow keeps her ASCII art", || src.clone());
    w.shot("interact-80-grid-relabelled");
    // the arrow menu: Hide label
    let i = w.read(|v| v.doc.figure.items.iter().position(|it| matches!(it, Item::Arrow(a) if a.from == "A" && a.to == "C"))).unwrap();
    let lp = w.label_px(&ObjRef::Arrow(i)).unwrap();
    w.right_click(lp);
    w.shot("interact-81-arrow-menu");
    let hid = w.click_ui("menu.ArrowLabel");
    let src = w.source();
    check(failures, hid && !src.contains("|g") && src.contains("\nend"), "Hide label in an arrow's menu (the grid stays)", || src.clone());
    // dragging an unselected arrow's label across swaps it (a vertical one: art stays)
    w.key("escape");
    w.key("escape");
    let j = w.read(|v| v.doc.figure.items.iter().position(|it| matches!(it, Item::Arrow(a) if a.from == "B" && a.to == "D"))).unwrap();
    let lp = w.label_px(&ObjRef::Arrow(j)).unwrap();
    let (b, d) = (w.anchor(&ObjRef::Node("B".into())).unwrap(), w.anchor(&ObjRef::Node("D".into())).unwrap());
    let left = w.px_of(Pt { x: b.x - 0.6, y: (b.y + d.y) / 2.0 });
    w.drag_path(lp, &line_to(lp, left, 12), Modifiers::default(), None);
    let src = w.source();
    check(failures, src.contains("h|") && src.contains("\nend"), "dragging an unselected arrow's label across swaps it, in the art", || src.clone());
    w.shot("interact-82-grid-swapped");
    // ⌘A ⌫ deletes everything
    w.focus_canvas();
    w.key("cmd-a");
    w.key("backspace");
    let left_over = w.read(|v| v.doc.figure.items.iter().filter(|it| matches!(it, Item::Node(_) | Item::Arrow(_))).count());
    let toast = w.read(|v| v.toast.as_ref().map(|t| t.text.to_string()));
    let src = w.source();
    check(failures, left_over == 0 && !src.contains("grid"), "⌘A ⌫ deletes the whole diagram (and its grid block)", || format!("{left_over} left; toast {toast:?}\n{src}"));
    w.undo_all();
    let back = w.source();
    check(failures, back == gsrc, "grid: ⌘Z goes back to the art", || back.clone());
    drop(w);

    // dragging one auto-placed node leaves the others where they were drawn
    let qf = figs.join("quick.fig");
    std::fs::write(&qf, "X -f-> Z\nY -g-> Z\n").unwrap();
    let mut w = Win::open_sized(&qf, false, WIDE, 900.0);
    w.focus_canvas();
    let (x0, y0) = (w.anchor(&ObjRef::Node("X".into())).unwrap(), w.anchor(&ObjRef::Node("Y".into())).unwrap());
    let z = w.anchor(&ObjRef::Node("Z".into())).unwrap();
    let zp = w.px_of(z);
    let cell = w.read(|v| v.cell());
    let to = w.px_of(Pt { x: z.x + cell.0, y: z.y });
    w.drag_path(zp, &line_to(zp, to, 12), Modifiers::default(), Some((8, "interact-83-dragging-auto-node")));
    let (x1, y1) = (w.anchor(&ObjRef::Node("X".into())).unwrap(), w.anchor(&ObjRef::Node("Y".into())).unwrap());
    check(failures, x0 == x1 && y0 == y1, "dragging Z leaves X and Y where they were", || format!("X {x0:?}→{x1:?}, Y {y0:?}→{y1:?}\n{}", w.source()));
    // the Insert menu on empty canvas: a pentagon, a Cayley graph of Z6, a node
    let b = w.read(|v| v.canvas_bounds.get().unwrap());
    let empty = point(b.origin.x + px(60.), b.bottom() - px(60.));
    w.right_click(empty);
    w.shot("interact-84-insert-menu");
    w.click_ui("menu.InsertNGon");
    w.shot("interact-85-insert-ngon");
    let ok = w.click_ui("menu.ngon.5");
    let sel = w.read(|v| v.selection.clone());
    check(failures, ok && matches!(sel.as_slice(), [ObjRef::NGon(_)]) && w.source().contains("ngon") && w.source().contains(" 5"), "Insert ▸ n-gon ▸ pentagon places and selects it", || format!("{sel:?}\n{}", w.source()));
    let empty2 = point(b.right() - px(80.), b.origin.y + px(80.));
    w.right_click(empty2);
    w.right_click(empty2);
    w.click_ui("menu.InsertCayley");
    w.shot("interact-86-insert-cayley");
    w.key("cmd-a");
    w.typ("6");
    w.key("enter");
    let sel = w.read(|v| v.selection.clone());
    let src = w.source();
    check(failures, src.contains("cayley Z6") && matches!(sel.as_slice(), [ObjRef::Cayley(_)]), "Insert ▸ Cayley graph ▸ ℤ n = 6 ↵ places Z6, selected", || format!("{sel:?}\n{src}"));
    w.shot("interact-87-cayley-inserted");
    // the group changes; the graph stays selected
    let gk = w.read(|v| v.fields.keys().find(|k| k.ends_with(".group")).cloned());
    if let Some(gk) = gk {
        w.click_ui(&format!("field.{gk}"));
        w.key("cmd-a");
        w.typ("D4");
        w.key("enter");
        let sel = w.read(|v| v.selection.clone());
        check(failures, matches!(sel.as_slice(), [ObjRef::Cayley(_)]), "the graph stays selected after its group changes", || format!("{sel:?}"));
    }
    drop(w);

    // Tab walks the vertex labels: in place, then in the panel
    let tf = figs.join("tab.fig");
    std::fs::write(&tf, "ngon P 4\n").unwrap();
    let mut w = Win::open_sized(&tf, false, WIDE, 900.0);
    w.focus_canvas();
    // Tab walks in reading order (top to bottom, then left to right)
    let order = w.read(|v| v.label_order(None));
    let first = match &order[0] { ObjRef::NGonVertex(_, k) => *k, o => panic!("{o:?}") };
    let l1 = w.label_px(&order[0]).unwrap();
    w.double_click(l1);
    for (k, s) in ["a", "b", "c", "d"].iter().enumerate() {
        w.with(|v, _, cx| v.label_field.update(cx, |f, cx| f.select_all(cx)));
        w.typ(s);
        if k < 3 {
            w.key("tab");
        }
    }
    w.shot("interact-88-tab-in-place");
    w.key("enter");
    let l = w.line("ngon P");
    let mut want = vec![String::new(); 4];
    for (o, s) in order.iter().zip(["a", "b", "c", "d"]) {
        if let ObjRef::NGonVertex(_, k) = o { want[*k as usize - 1] = s.to_string(); }
    }
    let _ = first;
    check(failures, l.contains(&want.join(",")), "Tab walks the vertex labels in place, in reading order", || format!("{l} (wanted {want:?})"));
    let pc = centre_px(&mut w, &ObjRef::NGon("P".into()));
    w.click(pc);
    w.click_ui("field.ngon.P.label.0");
    for (k, s) in ["w", "x", "y", "z"].iter().enumerate() {
        w.key("cmd-a");
        w.typ(s);
        if k < 3 {
            w.key("tab");
        }
    }
    w.shot("interact-89-tab-in-panel");
    w.key("enter");
    let l = w.line("ngon P");
    check(failures, l.contains("w,x,y,z"), "Tab walks the panel's vertex fields: w,x,y,z", || l.clone());
    // hovering an axis chip previews it
    if let Some(bb) = w.ui("axis.1–3") {
        w.hover_at(bb.center());
        let prev = w.read(|v| v.drag_scene.is_some());
        check(failures, prev, "hovering axis chip “1–3” previews the axis", || "no preview".into());
        w.shot("interact-90-axis-preview");
    } else {
        check(failures, false, "the axis chips say what they pass through (1–3)", || "no chip".into());
    }
    drop(w);

    // blobs: + element types its name; element to element is ↦
    let kf = figs.join("kernel2.fig");
    std::fs::write(&kf, "blob G {g}\nblob H {e', h}\n").unwrap();
    let mut w = Win::open_sized(&kf, false, WIDE, 900.0);
    w.focus_canvas();
    let hfr = w.shape_frame(&ObjRef::Blob("H".into())).unwrap();
    let inside = w.px_of(Pt { x: hfr.centre.x - hfr.half.0 * 0.7, y: hfr.centre.y - hfr.half.1 * 0.45 });
    w.click(inside);
    w.click_ui("button.blob.H.add");
    let editing = w.read(|v| v.label_edit.is_some());
    w.with(|v, _, cx| v.label_field.update(cx, |f, cx| f.select_all(cx)));
    w.typ("$h'$");
    w.key("enter");
    let l = w.line("blob H");
    check(failures, editing && l.contains("h'") && !l.contains("x"), "+ element: her label is its name (no placeholder left)", || format!("editing {editing}: {l}"));
    w.key("a");
    let (g, h) = (w.anchor(&ObjRef::Element("g".into())).unwrap(), w.anchor(&ObjRef::Element("h".into())).unwrap());
    let (gp, hp) = (w.px_of(g), w.px_of(h));
    w.drag_path(gp, &line_to(gp, hp, 10), Modifiers::default(), None);
    w.key("escape");
    let src = w.source();
    check(failures, src.contains("g |-> h") || src.contains("g ↦ h"), "the arrow tool makes element → element a ↦", || src.clone());
    w.shot("interact-91-maps-to");
    drop(w);

    // -- input to frame during the drags ---------------------------------------------------
    for (name, f) in drag_frames {
        if f.is_empty() {
            continue;
        }
        let (_, p95, max) = stats(&format!("{name}: mouse move → frame built"), f);
        if p95 > 16.0 {
            failures.push(format!("{name}: input-to-frame p95 {p95:.1} ms > 16 ms (max {max:.1})"));
        }
    }
}

/// The actual snippet engine, reached through real double-clicks/keys.
/// No shortcut is simulated in Rust; config/snippets/state are temp copies.
fn snippet_fields(figs: &Path, failures: &mut Vec<String>) {
    // A permanent headless evidence fixture for scoped draft typography.
    let styled_file = figs.join("styled-draft.fig");
    std::fs::write(&styled_file, r#"A "\text{Fun fact}" at 0,2
B "\text{kernel of $\varphi$}" at 0,1
C "\mathrm{im}\varphi" at 0,0
D "\operatorname{Aut}(G)" at 0,-1
E "x_{12}+123" at 0,-2
text "kernel of $\varphi$" at 3,1
ngon Q 4 at 3,-1
"#).unwrap();
    let mut styled = Win::open_sized(&styled_file, false, 1180., 900.);
    styled.with(|v, _, cx| {
        v.selection = vec![ObjRef::NGonVertex("Q".into(), 1)];
        cx.notify();
    });
    styled.shot("styled-labels-draft");
    drop(styled);

    eprintln!("snippet fields: real hidden ntex, offscreen GPUI");
    let file = figs.join("snippet-fields.fig");
    std::fs::write(&file, "A at 0,0\nngon P 4 at 3,0\n").unwrap();
    let mut w = Win::open_sized(&file, false, 1180., 900.);
    let node = ObjRef::Node("A".into());
    let at = w.label_px(&node).unwrap();
    w.double_click(at);
    assert_eq!(w.read(|v| v.label_edit.as_ref().map(|e| e.obj.clone())), Some(node.clone()));
    // Cold fallback echoes and preserves text when the same field upgrades.
    w.key("cmd-a");
    w.typ("Cold text");
    let cold = w.with(|v, _, cx| v.label_field.read(cx).text().to_string());
    check(failures, cold == "Cold text", "fallback accepts ordinary text during startup", || cold.clone());
    w.shot("snippet-fallback");
    let start = Instant::now();
    while !w.with(|v, _, cx| v.label_field.read(cx).ready()) {
        assert!(start.elapsed() < Duration::from_secs(30), "the real snippet field never became ready");
        w.tick(2);
        std::thread::sleep(Duration::from_millis(2));
    }
    let upgraded = w.with(|v, _, cx| v.label_field.read(cx).text().to_string());
    check(failures, upgraded == "Cold text", "live upgrade preserves fallback text", || upgraded.clone());
    w.key("cmd-a");
    // Unpaced burst: optimistic text is visible before any receiver is drained,
    // and Return must wait for every expansion before committing the model.
    w.cx.update_window(w.window.into(), |_, window, cx| {
        for key in ["shift-z->Z", "shift-z->Z"] {
            window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx);
        }
    }).unwrap();
    let echoed = w.with(|v, _, cx| v.label_field.read(cx).text().to_string());
    check(failures, echoed == "ZZ", "printable keys echo before the bridge answers", || echoed.clone());
    w.key("enter");
    let label = w.read(|v| v.doc.figure.items.iter().find_map(|it| match it { Item::Node(n) if n.name == "A" => Some(n.label.clone()), _ => None })).unwrap();
    check(failures, label == r"\mathbb{Z}", "ZZ Return commits mathbb Z (maths is the default)", || format!("{label:?}"));
    w.shot("snippet-mathbb-committed");

    let at = w.label_px(&node).unwrap();
    w.double_click(at);
    w.key("cmd-a");
    check(failures, w.click_ui("label.text"), "the text chip is beside the field", || "no chip".into());
    w.with(|v, window, cx| v.label_field.read(cx).focus.clone().focus(window, cx));
    w.key("cmd-a");
    w.typ("Fun fact");
    w.shot("snippet-text-mode");
    w.key("enter");
    let label = w.read(|v| v.doc.figure.items.iter().find_map(|it| match it { Item::Node(n) if n.name == "A" => Some(n.label.clone()), _ => None })).unwrap();
    check(failures, label == r"\text{Fun fact}", "Fun fact in text mode is a text label", || label.clone());
    w.shot("snippet-text-committed");

    let vertex = ObjRef::NGonVertex("P".into(), 1);
    let at = w.label_px(&vertex).unwrap();
    w.with(|v, _, cx| { v.selection = vec![vertex.clone()]; cx.notify(); });
    let panel_digits = w.with(|v, _, cx| v.fields.get("ngonv.P.1").unwrap().entity.read(cx).text().to_string());
    check(failures, panel_digits == "1", "digit vertex panel has no math delimiters", || panel_digits.clone());
    assert!(w.click_ui("field.ngonv.P.1"));
    w.key("cmd-a"); w.typ("1"); w.key("enter");
    let panel_digits = w.read(|v| v.doc.figure.items.iter().find_map(|it| match it { Item::NGon(g) => g.labels.first().cloned(), _ => None })).unwrap();
    check(failures, panel_digits == "1", "panel commits a digit as maths", || panel_digits.clone());
    w.double_click(at);
    let digits = w.with(|v, _, cx| v.label_field.read(cx).text().to_string());
    check(failures, digits == "1", "digit vertex opens without math delimiters", || digits.clone());
    w.key("enter");
    let digits = w.read(|v| v.doc.figure.items.iter().find_map(|it| match it { Item::NGon(g) => g.labels.first().cloned(), _ => None })).unwrap();
    check(failures, digits == "1", "committing a digit preserves its maths label", || digits.clone());
    w.double_click(at);
    w.key("cmd-a");
    w.typ("//a");
    let state = w.with(|v, _, cx| (v.label_field.read(cx).text().to_string(), v.label_field.read(cx).in_snippet));
    check(failures, state.0 == r"\frac{a}{}" && state.1, "fraction numerator is a live snippet", || format!("{state:?}"));
    check(failures, w.read(|v| v.label_preview.as_ref().is_some_and(|scene| scene.draws.iter().any(|draw|
        matches!(draw, Draw::Label { obj, latex, .. } if obj == &vertex && latex == r"\frac{a}{}")))),
        "typing updates the vertex's canvas draft", || "draft label did not follow the field".into());
    w.shot("snippet-fraction-numerator");
    w.key("tab");
    check(failures, w.read(|v| v.label_edit.as_ref().map(|e| e.obj.clone())) == Some(vertex.clone()), "Tab inside the fraction stays on vertex 1", || "Tab walked the vertex".into());
    w.typ("b");
    w.shot("snippet-fraction-denominator");
    w.key("enter");
    let label = w.read(|v| v.doc.figure.items.iter().find_map(|it| match it { Item::NGon(g) => g.labels.first().cloned(), _ => None })).unwrap();
    check(failures, label == r"\frac{a}{b}", "fraction Return commits a over b", || label.clone());

    let at = w.label_px(&vertex).unwrap();
    w.double_click(at);
    w.key("cmd-a");
    w.typ("plain");
    let next = w.read(|v| v.neighbour_label(&vertex, v.label_edit.as_ref().unwrap().at, 1));
    w.key("tab");
    check(failures, w.read(|v| v.label_edit.as_ref().map(|e| e.obj.clone())) == next && next.is_some() && next != Some(vertex.clone()), "Tab outside a snippet walks to the next label in reading order", || format!("now {:?}, wanted {next:?}", w.read(|v| v.label_edit.as_ref().map(|e| e.obj.clone()))));
    let cur = w.read(|v| v.label_edit.as_ref().map(|e| e.obj.clone())).unwrap();
    w.key("cmd-a");
    w.typ("//");
    w.key("escape");
    let first_esc = w.with(|v, _, cx| v.label_edit.is_some() && !v.label_field.read(cx).in_snippet);
    check(failures, first_esc, "first Esc leaves the snippet and keeps editing", || "field closed or snippet still active".into());
    w.shot("snippet-escaped");
    w.key("escape");
    check(failures, w.read(|v| v.label_edit.is_none()), "second Esc cancels the label edit", || "field still open".into());
    let label = w.read(|v| v.doc.figure.items.iter().find_map(|it| match it { Item::NGon(g) => Some(g.labels.clone()), _ => None })).unwrap();
    let k = match &cur { ObjRef::NGonVertex(_, k) => *k as usize, _ => 0 };
    check(failures, k > 0 && label[k - 1] == k.to_string(), "Esc preserves the original label", || format!("{label:?} at {cur:?}"));

    // A real default-valued tab stop exercises select-mode painting.
    let at = w.label_px(&node).unwrap();
    w.double_click(at);
    // (A is a text label now: the chip puts it back to maths)
    assert!(w.click_ui("label.text"));
    w.with(|v, window, cx| v.label_field.read(cx).focus.clone().focus(window, cx));
    w.key("cmd-a");
    w.typ("dint");
    let selected = w.with(|v, _, cx| v.label_field.read(cx).selection());
    check(failures, !selected.is_empty(), "the integral's default is selected", || format!("{selected:?}"));
    w.shot("snippet-selected-default");
    w.typ("2");
    let text = w.with(|v, _, cx| v.label_field.read(cx).text().to_string());
    check(failures, text.contains("_{2}"), "typing replaces the selected default", || text.clone());
    w.key("escape");
    w.key("escape");

    w.with(|v, _, cx| { v.selection = vec![node.clone()]; cx.notify(); });
    assert!(w.click_ui("chip.node.A.label"), "the panel has the text chip");
    assert!(w.click_ui("field.node.A.label"));
    w.key("cmd-a");
    w.typ("ZZ");
    w.shot("snippet-panel-mathbb");
    w.key("enter");
    let label = w.read(|v| v.doc.figure.items.iter().find_map(|it| match it { Item::Node(n) if n.name == "A" => Some(n.label.clone()), _ => None })).unwrap();
    check(failures, label == r"\mathbb{Z}", "properties label uses the real snippets too", || label.clone());
    assert!(w.click_ui("field.node.A.label"));
    w.key("cmd-a");
    w.typ("//");
    w.key("escape");
    check(failures, w.with(|v, window, cx| v.fields.get("node.A.label").unwrap().entity.read(cx).focus.is_focused(window)),
        "panel first Esc keeps editing", || "panel blurred".into());
    w.key("escape");
    let label = w.read(|v| v.doc.figure.items.iter().find_map(|it| match it { Item::Node(n) if n.name == "A" => Some(n.label.clone()), _ => None })).unwrap();
    check(failures, label == r"\mathbb{Z}", "panel second Esc restores its original label", || label.clone());
    // Native selection and deletion must not delete a second character in
    // ntex, and paste is programmatic (mk/ZZ do not fire).
    let at = w.label_px(&node).unwrap();
    w.double_click(at);
    w.key("cmd-a"); w.typ("abc");
    w.key("cmd-left"); w.key("right"); w.key("shift-right"); w.key("backspace");
    let text = w.with(|v, _, cx| v.label_field.read(cx).text().to_string());
    check(failures, text == "ac", "native selection backspace deletes only its range", || text.clone());
    w.with(|_, _, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string("mkZZ".into())));
    w.key("cmd-a"); w.key("cmd-v");
    let text = w.with(|v, _, cx| v.label_field.read(cx).text().to_string());
    check(failures, text == "mkZZ", "paste does not expand snippets", || text.clone());
    w.key("enter");

    // Free text items store TeX text directly, so their math islands keep
    // $...$ delimiters rather than receiving TikZ's label wrapper.
    w.with(|v, _, cx| {
        let source = format!("{}\ntext \"Fun fact\" at 1,1\n", v.doc.source);
        v.set_source(&source, true, cx);
    });
    let text_obj = w.read(|v| v.doc.figure.items.iter().position(|it| matches!(it, Item::Text(_)))).unwrap();
    let obj = ObjRef::Text(text_obj);
    let at = w.label_px(&obj).unwrap(); w.double_click(at);
    w.key("cmd-a"); w.typ("mkZZ"); w.key("enter");
    let text = w.read(|v| match &v.doc.figure.items[text_obj] { Item::Text(t) => t.text.clone(), _ => unreachable!() });
    check(failures, text == r"$\mathbb{Z}$", "free text items keep a valid maths island", || text.clone());
    let at = w.label_px(&obj).unwrap(); w.double_click(at);
    let text = w.with(|v, _, cx| v.label_field.read(cx).text().to_string());
    check(failures, text == r"$\mathbb{Z}$", "free text maths reopens with its delimiters", || text.clone());
    w.key("escape");

    // Drive the same platform IME handler used by AppKit; composition and
    // commits use field_set, without firing snippets from preedit text.
    let at = w.label_px(&node).unwrap(); w.double_click(at);
    w.key("cmd-a");
    w.with(|v, window, cx| v.label_field.update(cx, |f, cx| {
        use gpui::EntityInputHandler as _;
        f.replace_and_mark_text_in_range(None, "か", Some(1..1), window, cx);
    }));
    w.settle_fields();
    w.with(|v, window, cx| v.label_field.update(cx, |f, cx| {
        use gpui::EntityInputHandler as _;
        f.replace_text_in_range(None, "かな", window, cx);
    }));
    w.settle_fields();
    let text = w.with(|v, _, cx| v.label_field.read(cx).text().to_string());
    check(failures, text == "かな", "IME replaces marked text and preserves UTF-8 cursor offsets", || text.clone());
    w.key("cmd-a"); w.key("backspace");
    let mut echo_ms = Vec::new();
    let mut settled_ms = Vec::new();
    for _ in 0..40 {
        let start = Instant::now();
        w.cx.update_window(w.window.into(), |_, window, cx| {
            window.dispatch_keystroke(Keystroke::parse("q").unwrap(), cx);
        }).unwrap();
        w.draw();
        echo_ms.push(ms(start.elapsed()));
        w.settle_fields();
        settled_ms.push(ms(start.elapsed()));
    }
    echo_ms.sort_by(f64::total_cmp); settled_ms.sort_by(f64::total_cmp);
    eprintln!("  snippet input → echoed frame: median {:.3} / p95 {:.3} / max {:.3} ms",
        echo_ms[20], echo_ms[38], echo_ms[39]);
    eprintln!("  snippet input → reconciled frame (test polling included): median {:.3} / p95 {:.3} / max {:.3} ms",
        settled_ms[20], settled_ms[38], settled_ms[39]);
    w.key("escape");
    w.with(|v, _, cx| {
        let source = format!("{}\nblob H {{a, b}}\n", v.doc.source);
        v.set_source(&source, true, cx);
        v.selection = vec![ObjRef::Blob("H".into())]; cx.notify();
    });
    assert!(w.click_ui("button.blob.H.add"));
    w.key("cmd-a"); w.typ("h'"); w.key("enter");
    let named = w.read(|v| v.doc.figure.items.iter().any(|it| matches!(it, Item::Blob(b)
        if b.name == "H" && b.elements.iter().any(|e| e.name == "h'" && e.label == "h'"))));
    check(failures, named, "a plain first element label preserves its naming behavior", || "placeholder identity remained".into());
    let pids = w.read(|v| v.label_session.pids());
    assert!(!pids.is_empty(), "real ntex processes were started");
    drop(w);
    let deadline = Instant::now() + Duration::from_secs(3);
    for pid in pids {
        while unsafe { kill(pid as i32, 0) } == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_ne!(unsafe { kill(pid as i32, 0) }, 0, "orphan ntex process {pid}");
    }
    eprintln!("  real snippet field assertions complete; ntex processes reaped");
}
