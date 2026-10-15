//! USABILITY ADVERSARY: the author's 25 tasks (figures/verification/usability.md),
//! attempted in the real figure window, offscreen, with synthetic mouse and
//! keys only. Targets are found the way she finds them: canvas objects where
//! the Scene draws them, handles where they are drawn, panel controls by
//! their laid-out bounds (`ui_bounds`). No internal command is called to do
//! a task; the harness only reads state to report on it.
//!
//! `harness = false` (AppKit wants the main thread). No window appears:
//! HeadlessAppContext, Metal offscreen. Temp NTEX_STUDIO_CONFIG / STATE and
//! temp .fig copies; nothing of hers is written. PNGs land in
//! studio/evidence/usability/, results in evidence/usability/results.txt.
//!
//! `cargo test --test usability` (set USABILITY_ONLY=N1,D2 to run some).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    AppContext as _, Bounds, HeadlessAppContext, Keystroke, Modifiers, ModifiersChangedEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput, Pixels,
    Point, WindowHandle, point, px, size,
};
use ntex_figures::model::*;
use ntex_studio::ui::figure::doc;
use ntex_studio::ui::figure::geom;
use ntex_studio::ui::figure::handles::HKind;
use ntex_studio::ui::figure::view::{FigureOptions, FigureView};

/// The real window's default size (figure::open).
const W: f32 = 1180.0;
const H: f32 = 900.0;
/// Below this a trackpad target is too small (px, either side).
const MIN_TARGET: f32 = 24.0;

// -- one task's record -------------------------------------------------------------

#[derive(Default)]
struct Rec {
    id: String,
    words: String,
    ideal: usize,
    steps: Vec<String>,
    issues: Vec<String>,
    notes: Vec<String>,
    lat: Vec<(String, Vec<f64>)>,
    ok: bool,
    diff: String,
    undo: String,
    shots: Vec<String>,
}

struct Win {
    cx: HeadlessAppContext,
    window: WindowHandle<FigureView>,
    rec: Rec,
    shot_n: usize,
}

impl Win {
    fn open(path: &Path) -> Win {
        let text = gpui_platform::current_platform(true).text_system();
        let mut cx = HeadlessAppContext::with_platform(text, Arc::new(()), gpui_platform::current_headless_renderer);
        cx.update(ntex_studio::ui::app::init);
        let p = path.to_path_buf();
        let window = cx
            .open_window(size(px(W), px(H)), move |window, cx| {
                cx.new(|cx| FigureView::new(FigureOptions { path: p, quiet: true, render: false, build: false }, window, cx))
            })
            .unwrap();
        let mut w = Win { cx, window, rec: Rec::default(), shot_n: 0 };
        w.draw();
        w.draw();
        w.focus_canvas();
        w.tick(20);
        w
    }

    /// One frame. The bounds map is cleared first so a control that is no
    /// longer drawn can't be "found".
    fn draw(&mut self) {
        self.cx.run_until_parked();
        let _ = self.window.update(&mut self.cx, |v, _, _| v.ui_bounds.borrow_mut().clear());
        self.cx.update_window(self.window.into(), |_, window, cx| window.draw(cx).clear(cx)).unwrap();
        self.cx.run_until_parked();
    }

    fn read<R>(&mut self, f: impl FnOnce(&FigureView) -> R) -> R {
        self.window.update(&mut self.cx, |v, _, _| f(v)).unwrap()
    }

    fn tick(&mut self, ms: u64) {
        self.cx.advance_clock(Duration::from_millis(ms));
        let _ = self.window.update(&mut self.cx, |v, w, cx| v.pump(w, cx));
        self.draw();
    }

    fn source(&mut self) -> String {
        self.read(|v| v.doc.source.clone())
    }

    fn focus_canvas(&mut self) {
        let _ = self.window.update(&mut self.cx, |v, w, cx| v.canvas_focus.clone().focus(w, cx));
        self.draw();
    }

    fn event(&mut self, e: PlatformInput) {
        self.cx
            .update_window(self.window.into(), |_, window, cx| {
                window.dispatch_event(e, cx);
            })
            .unwrap();
    }

    fn shot(&mut self, what: &str) {
        self.draw();
        self.shot_n += 1;
        let name = format!("{}-{:02}-{}", self.rec.id, self.shot_n, what);
        let img = self.cx.capture_screenshot(self.window.into()).unwrap();
        let dir = evidence();
        std::fs::create_dir_all(&dir).unwrap();
        img.save(dir.join(format!("{name}.png"))).unwrap();
        self.rec.shots.push(name);
    }

    // -- where things are (what she sees) -------------------------------------------

    fn xf_px(&mut self, p: Pt) -> Point<Pixels> {
        self.read(|v| {
            let b = v.canvas_bounds.get().expect("canvas drawn");
            v.view.xf(b).to_px(p)
        })
    }
    fn px_per_cm(&mut self) -> f32 {
        self.read(|v| {
            let b = v.canvas_bounds.get().unwrap();
            v.view.xf(b).scale
        })
    }
    fn centre(&mut self, obj: &ObjRef) -> Option<Point<Pixels>> {
        let c = self.read(|v| geom::center_of(&v.doc.scene, obj))?;
        Some(self.xf_px(c))
    }
    fn label(&mut self, obj: &ObjRef) -> Option<Point<Pixels>> {
        let at = self.read(|v| {
            v.doc.scene.draws.iter().find_map(|d| match d {
                Draw::Label { obj: o, at, .. } if o == obj => Some(*at),
                _ => None,
            })
        })?;
        Some(self.xf_px(at))
    }
    fn dot(&mut self, obj: &ObjRef) -> Option<Point<Pixels>> {
        let at = self.read(|v| {
            v.doc.scene.draws.iter().find_map(|d| match d {
                Draw::Dot { obj: o, at, .. } if o == obj => Some(*at),
                _ => None,
            })
        })?;
        Some(self.xf_px(at))
    }
    /// A point on an arrow's line, `t` of the way along, nudged `off` px to its left.
    fn on_arrow(&mut self, i: usize, t: f32, off: f32) -> Option<Point<Pixels>> {
        let pts = self.read(|v| {
            v.doc.scene.draws.iter().find_map(|d| match d {
                Draw::Path { obj: ObjRef::Arrow(k), start, segs, .. } if *k == i => Some(geom::flatten(*start, segs)),
                _ => None,
            })
        })?;
        let total: f32 = pts.windows(2).map(|w| geom::dist(w[0], w[1])).sum();
        let mut acc = 0.0;
        for w2 in pts.windows(2) {
            let l = geom::dist(w2[0], w2[1]);
            if acc + l >= total * t && l > 0.0 {
                let s = (total * t - acc) / l;
                let p = Pt { x: w2[0].x + s * (w2[1].x - w2[0].x), y: w2[0].y + s * (w2[1].y - w2[0].y) };
                let a = self.xf_px(w2[0]);
                let b = self.xf_px(w2[1]);
                let (dx, dy) = (f32::from(b.x - a.x), f32::from(b.y - a.y));
                let n = dx.hypot(dy).max(1e-3);
                let c = self.xf_px(p);
                return Some(point(c.x + px(-dy / n * off), c.y + px(dx / n * off)));
            }
            acc += l;
        }
        None
    }
    fn arrow_index(&mut self, from: &str, to: &str) -> Option<usize> {
        self.read(|v| v.doc.figure.items.iter().position(|it| matches!(it, Item::Arrow(a) if a.from == from && a.to == to)))
    }
    fn handle(&mut self, want: impl Fn(HKind) -> bool) -> Option<Point<Pixels>> {
        self.read(|v| {
            let b = v.canvas_bounds.get()?;
            let xf = v.view.xf(b);
            v.handle_list(xf.scale).into_iter().find(|h| want(h.kind)).map(|h| xf.to_px(h.at))
        })
    }
    fn ui(&mut self, key: &str) -> Option<Bounds<Pixels>> {
        self.read(|v| v.ui_bounds.borrow().get(key).cloned())
    }
    fn ui_keys(&mut self) -> Vec<String> {
        let mut k: Vec<String> = self.read(|v| v.ui_bounds.borrow().keys().cloned().collect());
        k.sort();
        k
    }
    fn selection(&mut self) -> Vec<ObjRef> {
        self.read(|v| v.selection.clone())
    }
    fn line(&mut self, prefix: &str) -> String {
        self.source().lines().find(|l| l.trim_start().starts_with(prefix)).unwrap_or("").to_string()
    }

    // -- her gestures (each one is a step) ---------------------------------------------

    fn step(&mut self, s: impl Into<String>) {
        self.rec.steps.push(s.into());
    }
    fn issue(&mut self, s: impl Into<String>) {
        let s = s.into();
        if !self.rec.issues.contains(&s) {
            self.rec.issues.push(s);
        }
    }
    fn note(&mut self, s: impl Into<String>) {
        self.rec.notes.push(s.into());
    }

    fn mouse(&mut self, at: Point<Pixels>, button: MouseButton, clicks: usize, m: Modifiers) {
        self.event(PlatformInput::MouseDown(MouseDownEvent { button, position: at, modifiers: m, click_count: clicks, first_mouse: false, pen: None }));
        self.draw();
        self.event(PlatformInput::MouseUp(MouseUpEvent { button, position: at, modifiers: m, click_count: clicks, pen: None }));
        self.draw();
    }
    fn hover(&mut self, at: Point<Pixels>) {
        self.event(PlatformInput::MouseMove(MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default(), pen: None }));
        self.draw();
    }
    fn click(&mut self, at: Point<Pixels>, what: &str) {
        self.hover(at);
        self.mouse(at, MouseButton::Left, 1, Modifiers::default());
        self.step(format!("click {what}"));
    }
    fn dclick(&mut self, at: Point<Pixels>, what: &str) {
        self.hover(at);
        self.mouse(at, MouseButton::Left, 1, Modifiers::default());
        self.mouse(at, MouseButton::Left, 2, Modifiers::default());
        self.step(format!("double-click {what}"));
    }
    fn right(&mut self, at: Point<Pixels>, what: &str) {
        self.hover(at);
        self.mouse(at, MouseButton::Right, 1, Modifiers::default());
        self.step(format!("right-click {what}"));
    }
    fn key(&mut self, k: &str, what: &str) {
        self.key_raw(k);
        self.step(format!("{k} ({what})"));
    }
    fn key_raw(&mut self, k: &str) {
        self.cx
            .update_window(self.window.into(), |_, window, cx| {
                window.dispatch_keystroke(Keystroke::parse(k).unwrap(), cx);
            })
            .unwrap();
        self.draw();
        let start = Instant::now();
        loop {
            self.tick(2);
            let settled = self.window.update(&mut self.cx, |v, _, cx| v.label_field.read(cx).settled()
                && v.fields.values().all(|f| f.entity.read(cx).settled())).unwrap();
            if settled { break; }
            assert!(start.elapsed() < Duration::from_secs(15), "field reply timed out");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    /// Type a label (content, not shorthand): one step.
    fn typ(&mut self, text: &str) {
        for ch in text.chars() {
            let k = match ch {
                ' ' => "space".to_string(),
                // a capital as the keyboard sends it: ⇧ + the key, typing the capital
                c if c.is_ascii_uppercase() => format!("shift-{}->{}", c.to_ascii_lowercase(), c),
                c => c.to_string(),
            };
            self.key_raw(&k);
        }
        self.step(format!("type “{text}”"));
    }
    /// Click a panel/menu control by its laid-out bounds; flags small targets.
    /// Scroll the properties panel the way a user would (the wheel over it)
    /// until the control is inside its visible part.
    fn panel_into_view(&mut self, key: &str) {
        for _ in 0..30 {
            let (Some(b), Some(p)) = (self.ui(key), self.ui("panel.box")) else { return };
            let pinned = self.ui("panel.figure.toggle");
            // the pinned Figure head sits at the bottom of the panel: it is already reachable
            if key == "panel.figure.toggle" && b.top() >= p.top() && b.bottom() <= p.bottom() {
                return;
            }
            let pinned_top = pinned.filter(|h| h.bottom() <= p.bottom() + px(1.) && h.top() > p.top() + px(100.)).map(|h| h.top()).unwrap_or(p.bottom());
            let (c, top, bottom) = (b.center().y, p.top() + px(30.), pinned_top - px(12.));
            let dy = if c < top { 120.0 } else if c > bottom { -120.0 } else { return };
            let at = point(p.center().x, p.top() + px(40.));
            self.event(PlatformInput::ScrollWheel(gpui::ScrollWheelEvent { position: at, delta: gpui::ScrollDelta::Pixels(point(px(0.), px(dy))), modifiers: Modifiers::default(), touch_phase: gpui::TouchPhase::Moved }));
            self.draw();
        }
    }
    fn click_ui(&mut self, key: &str, what: &str) -> bool {
        self.panel_into_view(key);
        match self.ui(key) {
            Some(b) => {
                let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
                if w < MIN_TARGET || h < MIN_TARGET {
                    self.issue(format!("“{what}” is a {w:.0}×{h:.0} px target (< {MIN_TARGET:.0})"));
                }
                self.click(b.center(), what);
                true
            }
            None => {
                let keys = self.ui_keys();
                self.issue(format!("no “{what}” control drawn (looked for `{key}`)"));
                self.note(format!("controls drawn: {}", keys.join(" ")));
                false
            }
        }
    }
    /// Find a control whose key starts with `prefix` and contains `has`.
    fn find_ui(&mut self, prefix: &str, has: &str) -> Option<String> {
        self.ui_keys().into_iter().find(|k| k.starts_with(prefix) && k.contains(has))
    }
    /// A drag along `path`, timing each move → frame; shots at the middle.
    fn drag(&mut self, from: Point<Pixels>, path: &[Point<Pixels>], m: Modifiers, what: &str, shot: Option<&str>) -> Vec<f64> {
        self.hover(from);
        self.event(PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: from, modifiers: m, click_count: 1, first_mouse: false, pen: None }));
        self.draw();
        let mut frames = Vec::new();
        let mid = path.len() * 2 / 3;
        for (i, to) in path.iter().enumerate() {
            let t0 = Instant::now();
            self.event(PlatformInput::MouseMove(MouseMoveEvent { position: *to, pressed_button: Some(MouseButton::Left), modifiers: m, pen: None }));
            self.cx.run_until_parked();
            self.cx.update_window(self.window.into(), |_, window, cx| window.draw(cx).clear(cx)).unwrap();
            frames.push(ms(t0.elapsed()));
            if i == mid
                && let Some(s) = shot
            {
                self.shot(s);
            }
        }
        let end = *path.last().unwrap_or(&from);
        self.event(PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: end, modifiers: m, click_count: 1, pen: None }));
        self.draw();
        self.step(format!("drag {what}"));
        self.rec.lat.push((what.to_string(), frames.clone()));
        frames
    }

    // -- probes (not steps) ---------------------------------------------------------------

    /// How far (px) from `at`, along (dx, dy), hovering still finds `want`.
    fn reach(&mut self, at: Point<Pixels>, dir: (f32, f32), want: &dyn Fn(&FigureView) -> bool) -> f32 {
        let mut last = -1.0;
        for d in 0..=30 {
            let p = point(at.x + px(dir.0 * d as f32), at.y + px(dir.1 * d as f32));
            self.hover(p);
            if self.read(|v| want(v)) {
                last = d as f32;
            } else if last >= 0.0 {
                break;
            }
        }
        self.hover(point(px(5.), px(5.)));
        last
    }
    /// The handle's grab radius (px), by hovering outwards from it.
    fn grab_radius(&mut self, at: Point<Pixels>, kind: HKind) -> f32 {
        let r = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)]
            .iter()
            .map(|d| self.reach(at, *d, &|v: &FigureView| v.hover_handle == Some(kind)))
            .fold(f32::MAX, f32::min);
        r
    }
}

macro_rules! note {
    ($w:expr, $($t:tt)*) => {{ let s = format!($($t)*); $w.note(s); }};
}
macro_rules! issue {
    ($w:expr, $($t:tt)*) => {{ let s = format!($($t)*); $w.issue(s); }};
}

fn evidence() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("evidence/usability")
}
fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}
fn pct(v: &[f64], q: f64) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if s.is_empty() {
        return 0.0;
    }
    s[((s.len() as f64 - 1.0) * q).round() as usize]
}
fn line_to(a: Point<Pixels>, b: Point<Pixels>, n: usize) -> Vec<Point<Pixels>> {
    (1..=n).map(|i| {
        let t = i as f32 / n as f32;
        point(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
    }).collect()
}
fn arc(c: Point<Pixels>, r: f32, from_deg: f32, to_deg: f32, n: usize) -> Vec<Point<Pixels>> {
    // degrees counter-clockwise on screen (y down)
    (1..=n).map(|i| {
        let a = (from_deg + (to_deg - from_deg) * i as f32 / n as f32).to_radians();
        point(c.x + px(r * a.cos()), c.y - px(r * a.sin()))
    }).collect()
}
fn angle_px(c: Point<Pixels>, p: Point<Pixels>) -> f32 {
    f32::from(c.y - p.y).atan2(f32::from(p.x - c.x)).to_degrees()
}
fn diff(a: &str, b: &str) -> String {
    let al: Vec<&str> = a.lines().collect();
    let bl: Vec<&str> = b.lines().collect();
    let mut out = Vec::new();
    for l in &al {
        if !bl.contains(l) {
            out.push(format!("- {l}"));
        }
    }
    for l in &bl {
        if !al.contains(l) {
            out.push(format!("+ {l}"));
        }
    }
    out.join("\n")
}

struct TempRoot(PathBuf);
impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn setup() -> PathBuf {
    let root = std::env::temp_dir().join(format!("ntex-usability-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let config = root.join("texnvim");
    std::fs::create_dir_all(&config).unwrap();
    let real = PathBuf::from(std::env::var_os("HOME").unwrap()).join(".config/texnvim");
    assert!(std::process::Command::new("rsync")
        .args(["-a", "--exclude", "/studio/", "--exclude", ".git"])
        .arg(format!("{}/", real.display())).arg(format!("{}/", config.display()))
        .status().unwrap().success());
    std::fs::create_dir_all(root.join("figs")).unwrap();
    // SAFETY: before GPUI starts, single-threaded.
    unsafe {
        std::env::set_var("NTEX_STUDIO_CONFIG", root.join("texnvim"));
        std::env::set_var("NTEX_STUDIO_STATE", root.join("state"));
        std::env::set_var("TEXNVIM_SNIPPETS", config.join("snippets"));
        std::env::set_var("TEXNVIM_NO_VIEWER", "1");
    }
    root
}

// -- the task runner ----------------------------------------------------------------

struct Run {
    figs: PathBuf,
    out: Vec<Rec>,
    only: Option<Vec<String>>,
}

impl Run {
    fn task(&mut self, id: &str, words: &str, ideal: usize, src: &str, body: impl FnOnce(&mut Win) -> bool) {
        if let Some(o) = &self.only
            && !o.iter().any(|x| x == id)
        {
            return;
        }
        eprintln!("== {id}: {words}");
        let path = self.figs.join(format!("{id}.fig"));
        std::fs::write(&path, src).unwrap();
        let mut w = Win::open(&path);
        w.rec.id = id.to_string();
        w.rec.words = words.to_string();
        w.rec.ideal = ideal;
        let s0 = w.source();
        let u0 = w.read(|v| v.undo.len());
        w.shot("start");
        let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(&mut w))).unwrap_or_else(|_| {
            eprintln!("  (task body panicked)");
            false
        });
        // close a label field or menu she may have left open, as Esc would
        w.rec.ok = ok;
        let s1 = w.source();
        w.rec.diff = diff(&s0, &s1);
        // G1: undo back to the start, then redo
        w.focus_canvas();
        let k = w.read(|v| v.undo.len()).saturating_sub(u0);
        for _ in 0..k {
            w.key_raw("cmd-z");
        }
        let back = w.source() == s0;
        for _ in 0..k {
            w.key_raw("cmd-shift-z");
        }
        let fwd = w.source() == s1;
        w.rec.undo = format!("{k} undo step(s); ⌘Z×{k} restores: {back}; ⌘⇧Z×{k} redoes: {fwd}");
        if !back {
            w.issue("⌘Z doesn't put the source back as it was".to_string());
        }
        if s0 != s1 && k == 0 {
            w.issue("the change made no undo step".to_string());
        }
        for (what, f) in &w.rec.lat {
            if !f.is_empty() {
                let p95 = pct(f, 0.95);
                if p95 > 16.0 {
                    w.rec.issues.push(format!("drag “{what}”: input→frame p95 {p95:.1} ms > 16"));
                }
            }
        }
        let r = std::mem::take(&mut w.rec);
        eprintln!("  ok={} steps={} (ideal {}) issues={}", r.ok, r.steps.len(), r.ideal, r.issues.len());
        self.out.push(r);
        drop(w);
    }
}

// -- figures -------------------------------------------------------------------------------

const SQUARE: &str = "grid\n  A --f--> B\n  |g       |h\n  v        v\n  C --k--> D\nend\n";
const KERNEL: &str = "% The kernel, shaded, inside G; everything in it goes to the identity.\nblob G \"G\" {g, gk}\nblob K \"\\ker\\varphi\" in G shaded {e, k}\nblob H \"H\" {e', h}\nG -φ-> H\ne |-> e' ; k |-> e' ; g |-> h ; gk |-> h\n";
const QUICK: &str = "% Quick shorthand while working: no positions at all.\nN hook-> G ; G ->> G/N ; G -φ-> H\n";

fn ngon_of(w: &mut Win, name: &str) -> Option<NGon> {
    w.read(|v| v.doc.figure.items.iter().find_map(|it| match it {
        Item::NGon(g) if g.name == name => Some(g.clone()),
        _ => None,
    }))
}
fn first_ngon(w: &mut Win) -> Option<NGon> {
    w.read(|v| v.doc.figure.items.iter().find_map(|it| if let Item::NGon(g) = it { Some(g.clone()) } else { None }))
}
fn cayley_of(w: &mut Win) -> Option<Cayley> {
    w.read(|v| v.doc.figure.items.iter().find_map(|it| if let Item::Cayley(c) = it { Some(c.clone()) } else { None }))
}
fn blob_of(w: &mut Win, name: &str) -> Option<Blob> {
    w.read(|v| v.doc.figure.items.iter().find_map(|it| match it {
        Item::Blob(b) if b.name == name => Some(b.clone()),
        _ => None,
    }))
}
fn arrow_at(w: &mut Win, i: usize) -> Option<Arrow> {
    w.read(|v| match v.doc.figure.items.get(i) {
        Some(Item::Arrow(a)) => Some(a.clone()),
        _ => None,
    })
}
/// Where the canvas has room (px): its centre.
fn canvas_mid(w: &mut Win) -> Point<Pixels> {
    w.read(|v| v.canvas_bounds.get().unwrap().center())
}
/// The vertex at the top of a Cayley graph (largest y), and its word.
fn top_vertex(w: &mut Win, cname: &str) -> Option<String> {
    let cname = cname.to_string();
    w.read(move |v| {
        v.doc.scene.anchors.iter().filter_map(|(o, p, _)| match o {
            ObjRef::CayleyVertex(n, word) if *n == cname => Some((p.y, word.clone())),
            _ => None,
        }).max_by(|a, b| a.0.partial_cmp(&b.0).unwrap()).map(|(_, w)| w)
    })
}
fn vertex_pt(w: &mut Win, cname: &str, word: &str) -> Option<Pt> {
    let (c, wd) = (cname.to_string(), word.to_string());
    w.read(move |v| v.doc.scene.anchors.iter().find_map(|(o, p, _)| match o {
        ObjRef::CayleyVertex(n, x) if *n == c && *x == wd => Some(*p),
        _ => None,
    }))
}

fn main() {
    let root = setup();
    let _cleanup = TempRoot(root.clone());
    doc::quiet_todo_panics();
    let only = std::env::var("USABILITY_ONLY").ok().map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
    if only.is_none() {
        let _ = std::fs::remove_dir_all(evidence());
    }
    let mut run = Run { figs: root.join("figs"), out: Vec::new(), only };
    let real_templates = PathBuf::from(std::env::var_os("HOME").unwrap()).join(".config/texnvim/templates/tex");
    let real_before: Vec<(PathBuf, Vec<u8>)> = std::fs::read_dir(&real_templates).unwrap().flatten().map(|e| (e.path(), std::fs::read(e.path()).unwrap_or_default())).collect();
    let none = Modifiers::default();

    // ===================================================== n-gons
    run.task("N1", "draw D4 and show the reflection across the diagonal", 3, "% d4\n", |w| {
        w.key("p", "polygon tool");
        let c = canvas_mid(w);
        w.click(c, "on the canvas");
        w.shot("placed");
        let Some(g) = first_ngon(w) else {
            w.issue("the polygon tool's click placed nothing");
            return false;
        };
        if g.n != 4 {
            note!(w, "the polygon tool placed a {}-gon", g.n);
        }
        let sel = w.selection();
        if sel != vec![ObjRef::NGon(g.name.clone())] {
            issue!(w, "the new square isn't selected after placing it (selection {sel:?}), so the panel doesn't show it");
            w.key("v", "select tool");
            let c = w.centre(&ObjRef::NGon(g.name.clone())).unwrap();
            w.click(c, "the square");
        }
        // the axis through vertex 1 (and 3): the diagonal
        let keys = w.ui_keys();
        let axis_keys: Vec<String> = keys.iter().filter(|k| k.starts_with("axis.")).cloned().collect();
        note!(w, "axis chips: {axis_keys:?}");
        if axis_keys.iter().all(|k| !k.contains('–') && !k.contains('-')) {
            w.issue("axis chips are bare numbers (1, 2, 1.5, 2.5): nothing says 1 is the diagonal through 1 and 3");
        }
        // hover preview?
        if let Some(b) = w.ui("axis.1") {
            let before = w.read(|v| v.doc.scene.draws.len());
            w.hover(b.center());
            let after = w.read(|v| v.doc.scene.draws.len());
            let prev = w.read(|v| v.drag_scene.is_some());
            if before == after && !prev {
                w.note("hovering an axis chip previews nothing on the canvas");
            }
        }
        w.click_ui("axis.1", "axis chip 1");
        w.shot("axis");
        let g = first_ngon(w).unwrap();
        g.axes.contains(&2)
    });

    run.task("N2", "make the square sit flat", 2, "ngon P 4 angle=30\n", |w| {
        let c = w.centre(&ObjRef::NGon("P".into())).unwrap();
        w.click(c, "the tilted square");
        w.shot("selected");
        let key = w.find_ui("button.", "flat").unwrap_or_default();
        w.click_ui(&key, "flat bottom");
        w.shot("flat");
        let g = ngon_of(w, "P").unwrap();
        if g.angle.rem_euclid(90.0).abs() > 0.01 {
            issue!(w, "“flat bottom” left angle={}", g.angle);
            return false;
        }
        // and on a pentagon, rotated: does flat mean edge at the bottom?
        true
    });

    run.task("N2b", "make the pentagon sit flat again (odd n)", 2, "ngon P 5 angle=20\n", |w| {
        let c = w.centre(&ObjRef::NGon("P".into())).unwrap();
        w.click(c, "the tilted pentagon");
        let key = w.find_ui("button.", "flat").unwrap_or_default();
        w.click_ui(&key, "flat bottom");
        w.shot("flat");
        let g = ngon_of(w, "P").unwrap();
        // the default pentagon already sits on an edge: flat = angle 0 mod 72
        let ok = (g.angle.rem_euclid(72.0)).abs() < 0.01 || (72.0 - g.angle.rem_euclid(72.0)).abs() < 0.01;
        if !ok {
            issue!(w, "“flat bottom” on a pentagon gave angle={} (not an edge at the bottom)", g.angle);
        }
        ok
    });

    run.task("N3", "hide the vertex numbers", 2, "% D4 acting on the square: every reflection axis, and the rotation r.\nngon P 4 axes rotation\n", |w| {
        let c = w.centre(&ObjRef::NGon("P".into())).unwrap();
        w.click(c, "the square");
        w.shot("selected");
        let key = w.find_ui("check.", "label").unwrap_or_default();
        w.click_ui(&key, "Labels checkbox");
        w.shot("hidden");
        let g = ngon_of(w, "P").unwrap();
        if g.show_labels {
            w.issue("unticking Labels didn't hide them");
            return false;
        }
        // tick again: remembered
        let key = w.find_ui("check.", "label").unwrap_or_default();
        w.click_ui(&key, "Labels checkbox again");
        let g2 = ngon_of(w, "P").unwrap();
        if !g2.show_labels || g2.labels != vec!["1", "2", "3", "4"] {
            issue!(w, "ticking Labels back didn't restore 1–4: {:?}", g2.labels);
        }
        // the menu route
        w.right(c, "the square (menu)");
        w.shot("menu");
        let mk = if w.ui("menu.Labels").is_some() { "menu.Labels".to_string() } else { String::new() };
        note!(w, "menu entries: {:?}", w.ui_keys().into_iter().filter(|k| k.starts_with("menu.")).collect::<Vec<_>>());
        if !mk.is_empty() {
            w.click_ui(&mk, "Hide labels (menu)");
        } else {
            w.key_raw("escape");
        }
        w.shot("menu-hidden");
        !ngon_of(w, "P").unwrap().show_labels
    });

    run.task("N4", "label the vertices a b c d", 6, "ngon P 4\n", |w| {
        let l1 = w.label(&ObjRef::NGonVertex("P".into(), 1)).unwrap();
        w.dclick(l1, "vertex label 1");
        w.shot("editing-1");
        let editing = w.read(|v| v.label_edit.as_ref().map(|e| e.obj.clone()));
        if editing.is_none() {
            w.issue("double-clicking a vertex label doesn't edit it");
        }
        w.key_raw("cmd-a");
        w.typ("$a$");
        w.key("tab", "next vertex?");
        let ed2 = w.read(|v| v.label_edit.as_ref().map(|e| e.obj.clone()));
        w.shot("after-tab");
        let tab_walks = ed2 == Some(ObjRef::NGonVertex("P".into(), 2));
        if !tab_walks {
            issue!(w, "Tab in a vertex label doesn't go to the next vertex (now editing {ed2:?})");
            w.note("falling back: one double-click per vertex");
            if ed2.is_some() {
                w.key_raw("escape");
            }
            let g = ngon_of(w, "P").unwrap();
            if g.labels.first().map(|s| s.as_str()) != Some("a") {
                // the Tab may have thrown the first one away; Return it again
                let l1 = w.label(&ObjRef::NGonVertex("P".into(), 1)).unwrap();
                w.dclick(l1, "vertex label 1 again");
                w.key_raw("cmd-a");
                w.typ("$a$");
                w.key("enter", "set");
            }
            for (k, s) in [(2u32, "b"), (3, "c"), (4, "d")] {
                if let Some(lp) = w.label(&ObjRef::NGonVertex("P".into(), k)) {
                    w.dclick(lp, &format!("vertex label {k}"));
                    w.key_raw("cmd-a");
                    w.typ(&format!("${s}$"));
                    w.key("enter", "set");
                }
            }
        } else {
            for s in ["b", "c", "d"] {
                w.key_raw("cmd-a");
                w.typ(&format!("${s}$"));
                if s != "d" {
                    w.key("tab", "next");
                }
            }
            w.key("enter", "set");
        }
        w.shot("labelled");
        let g = ngon_of(w, "P").unwrap();
        note!(w, "labels now {:?}; line `{}`", g.labels, w.line("ngon"));
        g.labels == vec!["a", "b", "c", "d"]
    });

    run.task("N4b", "label the vertices a b c d, in the panel", 6, "ngon P 4\n", |w| {
        let c = w.centre(&ObjRef::NGon("P".into())).unwrap();
        w.click(c, "the square");
        let fields: Vec<String> = w.ui_keys().into_iter().filter(|k| k.starts_with("field.") && k.contains("label")).collect();
        note!(w, "label fields: {fields:?}");
        let Some(f1) = fields.first().cloned() else {
            w.issue("no per-vertex label fields in the panel");
            return false;
        };
        let b = w.ui(&f1).unwrap();
        if f32::from(b.size.width) < 50.0 {
            issue!(w, "vertex label fields are {:.0} px wide: `\\ker\\varphi`-length labels won't fit", f32::from(b.size.width));
        }
        w.click_ui(&f1, "label field 1");
        w.key_raw("cmd-a");
        w.typ("$a$");
        w.key("tab", "next field");
        w.key_raw("cmd-a");
        w.typ("$b$");
        w.key("tab", "next field");
        w.key_raw("cmd-a");
        w.typ("$c$");
        w.key("tab", "next field");
        w.key_raw("cmd-a");
        w.typ("$d$");
        w.key("enter", "done");
        w.shot("labelled");
        let g = ngon_of(w, "P").unwrap();
        note!(w, "labels now {:?}", g.labels);
        if g.labels != vec!["a", "b", "c", "d"] {
            issue!(w, "Tab doesn't walk the panel's vertex fields (labels {:?})", g.labels);
        }
        g.labels == vec!["a", "b", "c", "d"]
    });

    run.task("N5", "copy this pentagon and turn it once", 3, "ngon P 5\n", |w| {
        let c = w.centre(&ObjRef::NGon("P".into())).unwrap();
        w.click(c, "the pentagon");
        w.key("cmd-d", "duplicate");
        w.shot("duplicated");
        let names: Vec<String> = w.read(|v| v.doc.figure.items.iter().filter_map(|it| if let Item::NGon(g) = it { Some(g.name.clone()) } else { None }).collect());
        if names.len() != 2 {
            w.issue("⌘D made no copy");
            return false;
        }
        let copy = names[1].clone();
        let sel = w.selection();
        if sel != vec![ObjRef::NGon(copy.clone())] {
            issue!(w, "after ⌘D the copy isn't the selection ({sel:?})");
            let cc = w.centre(&ObjRef::NGon(copy.clone())).unwrap();
            w.click(cc, "the copy");
        }
        // overlap / visible?
        let (a, b) = (w.centre(&ObjRef::NGon("P".into())).unwrap(), w.centre(&ObjRef::NGon(copy.clone())).unwrap());
        let gap = f32::from(b.x - a.x).hypot(f32::from(b.y - a.y));
        let cb = w.read(|v| v.canvas_bounds.get().unwrap());
        if !cb.contains(&b) {
            w.issue("the copy lands outside the visible canvas");
        }
        note!(w, "copy is {gap:.0} px from the original");
        let turn = w.find_ui("button.", "turn").filter(|k| k.contains("plus") || k.contains("+") || k.ends_with("up") || k.contains("inc"));
        let tk = turn.or_else(|| w.find_ui("", "turn")).unwrap_or_default();
        note!(w, "turn controls: {:?}", w.ui_keys().into_iter().filter(|k| k.contains("turn")).collect::<Vec<_>>());
        w.click_ui(&tk, "Turn +");
        w.shot("turned");
        let g = ngon_of(w, &copy).unwrap();
        let p = ngon_of(w, "P").unwrap();
        if p.turn != 0 {
            w.issue("Turn changed the original too");
        }
        g.turn == 1
    });

    run.task("N6", "make the hexagon a bit bigger and stand it on a vertex", 3, "ngon P 6\n", |w| {
        let c = w.centre(&ObjRef::NGon("P".into())).unwrap();
        w.click(c, "the hexagon");
        w.shot("selected");
        let Some(dot) = w.handle(|k| k == HKind::Radius) else {
            w.issue("no radius handle");
            return false;
        };
        let gr = w.grab_radius(dot, HKind::Radius);
        note!(w, "radius dot grab radius ≈ {gr:.0} px");
        if gr * 2.0 < MIN_TARGET {
            issue!(w, "radius dot: grab area {:.0} px across (< {MIN_TARGET:.0})", gr * 2.0 + 1.0);
        }
        let r0 = ngon_of(w, "P").unwrap().radius.unwrap_or(1.1);
        let ppcm = w.px_per_cm();
        let (dx, dy) = (f32::from(dot.x - c.x), f32::from(dot.y - c.y));
        let l = dx.hypot(dy);
        let target = point(dot.x + px(dx / l * 0.3 * ppcm), dot.y + px(dy / l * 0.3 * ppcm));
        w.drag(dot, &line_to(dot, target, 30), none, "the radius dot outwards", Some("resizing"));
        let r1 = ngon_of(w, "P").unwrap().radius.unwrap_or(1.1);
        note!(w, "radius {r0} → {r1}; line `{}`", w.line("ngon"));
        let Some(knob) = w.handle(|k| k == HKind::Rotate) else {
            w.issue("no rotate knob");
            return false;
        };
        let gk = w.grab_radius(knob, HKind::Rotate);
        note!(w, "rotate knob grab radius ≈ {gk:.0} px");
        if gk * 2.0 < MIN_TARGET {
            issue!(w, "rotate knob: grab area {:.0} px across (< {MIN_TARGET:.0})", gk * 2.0 + 1.0);
        }
        // the knob must not sit on vertex 1's label
        if let Some(l1) = w.label(&ObjRef::NGonVertex("P".into(), 1)) {
            let d = f32::from(l1.x - knob.x).hypot(f32::from(l1.y - knob.y));
            if d < 20.0 {
                issue!(w, "the rotate knob is {d:.0} px from vertex label 1");
            }
        }
        let c = w.centre(&ObjRef::NGon("P".into())).unwrap();
        let rad = f32::from(knob.x - c.x).hypot(f32::from(knob.y - c.y));
        let a0 = angle_px(c, knob);
        w.drag(knob, &arc(c, rad, a0, a0 + 31.0, 30), none, "the ↻ knob 30°", Some("rotating"));
        let g = ngon_of(w, "P").unwrap();
        note!(w, "angle {}; line `{}`", g.angle, w.line("ngon"));
        r1 > r0 && (g.angle.rem_euclid(60.0) - 30.0).abs() < 0.01
    });

    run.task("N7", "actually make it a pentagon, no fill", 3, "ngon P 6 labels a,b,c,d,e,f\n", |w| {
        let c = w.centre(&ObjRef::NGon("P".into())).unwrap();
        w.click(c, "the hexagon");
        let minus = w.ui_keys().into_iter().find(|k| k.contains("sides") && (k.contains("minus") || k.contains("dec") || k.contains("−") || k.contains("-")));
        note!(w, "sides controls: {:?}", w.ui_keys().into_iter().filter(|k| k.contains("sides")).collect::<Vec<_>>());
        w.click_ui(&minus.unwrap_or_default(), "Sides −");
        let fill = w.find_ui("check.", "fill").unwrap_or_default();
        w.click_ui(&fill, "Fill checkbox");
        w.shot("pentagon-plain");
        let g = ngon_of(w, "P").unwrap();
        note!(w, "line `{}`", w.line("ngon"));
        if g.n == 5 && g.labels.len() != 5 {
            issue!(w, "after Sides − the labels are {:?}", g.labels);
        }
        if g.n == 5 && g.labels.first().map(|s| s.as_str()) != Some("a") {
            w.issue("changing the sides threw away her vertex labels");
        }
        g.n == 5 && !g.shaded
    });

    // ===================================================== Cayley graphs
    run.task("C1", "rotate the cayley graph so e is on top (it was turned earlier)", 2, "cayley D4 <r, s> legend angle=40\n", |w| {
        let cg = cayley_of(w).unwrap();
        let cref = ObjRef::Cayley(cg.name.clone());
        let top0 = top_vertex(w, &cg.name);
        note!(w, "top vertex at start: {top0:?}");
        let e = vertex_pt(w, &cg.name, "e").unwrap();
        let ep = w.xf_px(e);
        w.click(ep, "the graph (on e)");
        let sel = w.selection();
        if sel != vec![cref.clone()] {
            issue!(w, "a click on the graph selected {sel:?}, not the graph");
        }
        w.shot("selected");
        let Some(knob) = w.handle(|k| k == HKind::Rotate) else {
            w.issue("no rotate knob on the Cayley graph");
            return false;
        };
        let c = w.centre(&cref).unwrap();
        // the pointer goes where she wants e: work out the turn from e's angle
        let ev = vertex_pt(w, &cg.name, "e").unwrap();
        let evp = w.xf_px(ev);
        let ea = angle_px(c, evp);
        let turn = 90.0 - ea;
        let turn = if turn > 180.0 { turn - 360.0 } else if turn < -180.0 { turn + 360.0 } else { turn };
        let ka = angle_px(c, knob);
        let rad = f32::from(knob.x - c.x).hypot(f32::from(knob.y - c.y));
        note!(w, "e sits at {ea:.0}°; needs a {turn:.0}° turn");
        w.drag(knob, &arc(c, rad, ka, ka + turn, 40), none, "the ↻ knob until e is on top", Some("rotating"));
        let top = top_vertex(w, &cg.name);
        w.shot("rotated");
        let c2 = cayley_of(w).unwrap();
        note!(w, "top vertex now {top:?}; angle {}; line `{}`", c2.angle, w.line("cayley"));
        let menu_put_top = {
            let e = vertex_pt(w, &cg.name, "e").unwrap();
            let ep = w.xf_px(e);
            w.hover(ep);
            w.mouse(ep, MouseButton::Right, 1, Modifiers::default());
            let k = w.ui_keys().into_iter().filter(|k| k.starts_with("menu.")).collect::<Vec<_>>();
            w.shot("menu-on-vertex");
            w.key_raw("escape");
            k
        };
        note!(w, "menu on a vertex: {menu_put_top:?}");
        top.as_deref() == Some("e")
    });

    run.task("C2", "make the Cayley graph smaller", 2, "cayley D4 <r, s> legend\n", |w| {
        let cg = cayley_of(w).unwrap();
        let cref = ObjRef::Cayley(cg.name.clone());
        let c = w.centre(&cref).unwrap();
        let e = vertex_pt(w, &cg.name, "e").unwrap();
        let ep = w.xf_px(e);
        w.click(ep, "the graph");
        let Some(dot) = w.handle(|k| k == HKind::Scale) else {
            w.issue("no resize dot on the Cayley graph");
            return false;
        };
        let gr = w.grab_radius(dot, HKind::Scale);
        if gr * 2.0 < MIN_TARGET {
            issue!(w, "Cayley resize dot: grab area {:.0} px across", gr * 2.0 + 1.0);
        }
        let target = point(c.x + (dot.x - c.x) * 0.7, c.y + (dot.y - c.y) * 0.7);
        w.drag(dot, &line_to(dot, target, 30), none, "the resize dot inwards", Some("shrinking"));
        w.shot("smaller");
        let c2 = cayley_of(w).unwrap();
        note!(w, "line `{}`", w.line("cayley"));
        c2.radius.is_some_and(|r| cg.radius.is_none_or(|r0| r < r0))
    });

    run.task("C3", "hide the element names, show the legend", 3, "cayley D4 <r, s>\n", |w| {
        let cg = cayley_of(w).unwrap();
        let e = vertex_pt(w, &cg.name, "e").unwrap();
        let ep = w.xf_px(e);
        w.click(ep, "the graph");
        w.shot("selected");
        let lk = w.find_ui("check.", "label").unwrap_or_default();
        w.click_ui(&lk, "Labels checkbox");
        let gk = w.find_ui("check.", "legend").unwrap_or_default();
        w.click_ui(&gk, "Legend checkbox");
        w.shot("done");
        let c2 = cayley_of(w).unwrap();
        note!(w, "line `{}`", w.line("cayley"));
        !c2.labels && c2.legend
    });

    run.task("C4", "make it D5 instead", 3, "cayley D4 <r, s> legend\n", |w| {
        let cg = cayley_of(w).unwrap();
        let e = vertex_pt(w, &cg.name, "e").unwrap();
        let ep = w.xf_px(e);
        w.click(ep, "the graph");
        let gk = w.find_ui("field.", "group").unwrap_or_default();
        w.click_ui(&gk, "Group field");
        w.key_raw("cmd-a");
        w.typ("D5");
        w.key("enter", "set");
        w.tick(200);
        w.shot("d5");
        let line = w.line("cayley");
        note!(w, "line `{line}`");
        // a bad name next
        let gk = w.find_ui("field.", "group").unwrap_or_default();
        if w.ui(&gk).is_none() {
            note!(w, "after Return the Group field is gone (selection {:?})", w.read(|v| v.selection.clone()));
            w.issue("changing the group deselects the graph: the panel she was typing in vanishes on Return");
            let cg = cayley_of(w).unwrap();
            if let Some(e) = vertex_pt(w, &cg.name, "e") {
                let ep = w.xf_px(e);
                w.hover(ep);
                w.mouse(ep, MouseButton::Left, 1, Modifiers::default());
            }
        }
        let gk = w.find_ui("field.", "group").unwrap_or_default();
        if let Some(b) = w.ui(&gk) {
            w.hover(b.center());
            w.mouse(b.center(), MouseButton::Left, 1, Modifiers::default());
            w.key_raw("cmd-a");
            for ch in ["D"] {
                w.key_raw(ch);
            }
            w.key_raw("enter");
            w.tick(200);
            w.shot("bad-group");
            let diags = w.read(|v| v.doc.diagnostics.iter().map(|d| d.message.clone()).collect::<Vec<_>>());
            let drawn = w.read(|v| v.doc.scene.anchors.iter().any(|(o, _, _)| matches!(o, ObjRef::CayleyVertex(..))));
            note!(w, "after typing “D”: diagnostics {diags:?}, graph still drawn: {drawn}");
            if !drawn {
                w.issue("a half-typed group name (“D”) wipes the graph while she types");
            }
            w.key_raw("cmd-a");
            w.key_raw("D");
            w.key_raw("5");
            w.key_raw("enter");
            w.tick(200);
        }
        line.contains("D5")
    });

    run.task("C5", "quick Cayley sketch of Z6 (from nothing)", 4, "% sketch\n", |w| {
        let c = canvas_mid(w);
        w.right(c, "empty canvas");
        w.shot("menu-empty");
        let keys: Vec<String> = w.ui_keys().into_iter().filter(|k| k.starts_with("menu.")).collect();
        note!(w, "menu on empty canvas: {keys:?}");
        w.key_raw("escape");
        let insert = keys.iter().find(|k| k.to_lowercase().contains("cayley") || k.to_lowercase().contains("insert")).cloned();
        if insert.is_none() {
            w.issue("no way to insert a Cayley graph without typing shorthand or asking: no toolbar tool, nothing in the right-click menu");
            return false;
        }
        // (FIG-INTERACT, round 2) the route itself: right-click · Cayley graph ▸ · Z_n: 6 · Return
        let Some(cay) = keys.iter().find(|k| k.to_lowercase().contains("cayley")).cloned() else { return false };
        w.right(c, "empty canvas (again)");
        w.click_ui(&cay, "Cayley graph ▸");
        w.shot("cayley-menu");
        w.key_raw("cmd-a");
        w.typ("6");
        w.key("enter", "insert");
        w.shot("inserted");
        let c = cayley_of(w);
        note!(w, "source:\n{}", w.source());
        match c {
            Some(c) => {
                let sel = w.selection();
                if sel != vec![ObjRef::Cayley(c.name.clone())] {
                    issue!(w, "the new graph isn't selected ({sel:?})");
                }
                matches!(&c.group, GroupSpec::Named(g) if g == "Z6")
            }
            None => false,
        }
    });

    // ===================================================== blobs
    run.task("B1", "make the kernel blob bigger", 2, KERNEL, |w| {
        let k = ObjRef::Blob("K".into());
        // a point inside K, away from its elements: between centre and the outline
        let kc = w.read(|v| geom::center_of(&v.doc.scene, &ObjRef::Blob("K".into()))).unwrap();
        let e = w.read(|v| v.doc.scene.anchors.iter().find(|(o, _, _)| *o == ObjRef::Element("e".into())).map(|(_, p, _)| *p)).unwrap();
        let kk = w.read(|v| v.doc.scene.anchors.iter().find(|(o, _, _)| *o == ObjRef::Element("k".into())).map(|(_, p, _)| *p)).unwrap();
        let spot = Pt { x: kc.x - (e.x + kk.x - 2.0 * kc.x).signum() * 0.25, y: kc.y };
        let sp = w.xf_px(spot);
        w.hover(sp);
        let hov = w.read(|v| v.hover.clone());
        w.shot("hover-in-K");
        if hov != Some(k.clone()) {
            note!(w, "hovering inside K shows {hov:?}");
        }
        w.click(sp, "inside K");
        let sel = w.selection();
        if sel != vec![k.clone()] {
            issue!(w, "a click inside K selected {sel:?}");
            return false;
        }
        w.shot("K-selected");
        let Some(corner) = w.handle(|h| h == HKind::Corner(1, -1)).or_else(|| w.handle(|h| matches!(h, HKind::Corner(1, _)))) else {
            w.issue("no corner handle on K");
            return false;
        };
        let gr = w.grab_radius(corner, HKind::Corner(1, -1));
        note!(w, "blob corner grab radius ≈ {gr:.0} px");
        if gr >= 0.0 && gr * 2.0 < MIN_TARGET {
            issue!(w, "blob corner handle: grab area {:.0} px across (< {MIN_TARGET:.0})", gr * 2.0 + 1.0);
        }
        let ppcm = w.px_per_cm();
        let to = point(corner.x + px(0.5 * ppcm), corner.y + px(0.4 * ppcm));
        w.drag(corner, &line_to(corner, to, 30), none, "K's corner out", Some("resizing"));
        w.shot("bigger");
        let kb = blob_of(w, "K").unwrap();
        note!(w, "K line `{}`; G line `{}`", w.line("blob K"), w.line("blob G"));
        // is K still inside G? compare outline boxes
        let (gr_, kr) = w.read(|v| (geom::extent_of(&v.doc.scene, &ObjRef::Blob("G".into())), geom::extent_of(&v.doc.scene, &ObjRef::Blob("K".into()))));
        if let (Some(g), Some(k)) = (gr_, kr)
            && (k.min.x < g.min.x || k.max.x > g.max.x || k.min.y < g.min.y || k.max.y > g.max.y)
        {
            w.issue("after growing K it pokes out of G");
        }
        kb.size.is_some()
    });

    run.task("B2", "add an element to H", 3, KERNEL, |w| {
        let hc = w.centre(&ObjRef::Blob("H".into())).unwrap();
        // inside H but off its elements: below centre
        let ppcm = w.px_per_cm();
        let mut sp = point(hc.x, hc.y + px(0.55 * ppcm));
        'scan: for dy in [0.55f32, 0.7, 0.4, -0.55, -0.7, 0.0] {
            for dx in [0.0f32, 0.3, -0.3] {
                let q = point(hc.x + px(dx * ppcm), hc.y + px(dy * ppcm));
                w.hover(q);
                if w.read(|v| v.hover.clone()) == Some(ObjRef::Blob("H".into())) {
                    sp = q;
                    break 'scan;
                }
            }
        }
        w.click(sp, "inside H (where the hover outlines H)");
        let sel = w.selection();
        if sel != vec![ObjRef::Blob("H".into())] {
            note!(w, "the click inside H selected {sel:?}");
            let lp = w.label(&ObjRef::Blob("H".into())).unwrap();
            w.click(lp, "H's label");
        }
        w.shot("H-selected");
        let keys = w.ui_keys();
        note!(w, "element controls: {:?}", keys.iter().filter(|k| k.contains("elem") || k.contains("add")).collect::<Vec<_>>());
        let add = keys.iter().find(|k| k.contains("add")).cloned().unwrap_or_default();
        w.click_ui(&add, "+ (add element)");
        w.shot("added");
        let focused_new = w.read(|v| v.label_edit.is_some());
        let b = blob_of(w, "H").unwrap();
        note!(w, "H now {:?}; editing a label: {focused_new}", b.elements.iter().map(|e| e.name.clone()).collect::<Vec<_>>());
        if b.elements.len() < 3 {
            return false;
        }
        // name it h'
        let newest = b.elements.last().unwrap().name.clone();
        let fkeys: Vec<String> = w.ui_keys().into_iter().filter(|k| k.starts_with("field.") && k.contains(&newest)).collect();
        if focused_new {
            w.key_raw("cmd-a");
            w.typ("$h'$");
            w.key("enter", "set");
        } else if let Some(fk) = fkeys.first().cloned() {
            w.click_ui(&fk, "the new element's field");
            w.key_raw("cmd-a");
            w.typ("$h'$");
            w.key("enter", "set");
        } else {
            w.issue("the new element's name isn't ready to type: no field focused, none found");
        }
        w.shot("named");
        let b = blob_of(w, "H").unwrap();
        note!(w, "H elements {:?} labels {:?}", b.elements.iter().map(|e| e.name.clone()).collect::<Vec<_>>(), b.elements.iter().map(|e| e.label.clone()).collect::<Vec<_>>());
        b.elements.iter().any(|e| e.label == "h'" || e.name == "h'")
    });

    run.task("B3", "rename g to g^{-1}", 3, KERNEL, |w| {
        let gl = w.label(&ObjRef::Element("g".into())).unwrap();
        w.hover(gl);
        let hov = w.read(|v| v.hover.clone());
        if hov != Some(ObjRef::Element("g".into())) {
            note!(w, "hovering g's label shows {hov:?}");
        }
        w.dclick(gl, "the label g");
        w.shot("editing");
        if w.read(|v| v.label_edit.is_none()) {
            w.issue("double-clicking an element's label doesn't edit it");
            return false;
        }
        w.key_raw("cmd-a");
        w.typ("$g^{-1}$");
        w.key("enter", "set");
        w.shot("renamed");
        let src = w.source();
        note!(w, "source:\n{src}");
        let arrows_ok = w.read(|v| v.doc.diagnostics.iter().all(|d| !d.message.contains("unknown")));
        if !arrows_ok {
            w.issue("renaming the label broke an arrow that names g");
        }
        src.contains("g^{-1}")
    });

    run.task("B4", "send g to h", 2, "blob G \"G\" {g, gk}\nblob K \"\\ker\\varphi\" in G shaded {e, k}\nblob H \"H\" {e', h}\nG -φ-> H\ne |-> e' ; k |-> e' ; gk |-> h\n", |w| {
        w.key("a", "arrow tool");
        let g = w.dot(&ObjRef::Element("g".into())).unwrap();
        let h = w.dot(&ObjRef::Element("h".into())).unwrap();
        w.drag(g, &line_to(g, h, 30), none, "from g's dot to h's dot", Some("dragging"));
        w.shot("drawn");
        w.key_raw("escape");
        let i = w.arrow_index("g", "h");
        let src = w.source();
        note!(w, "source:\n{src}");
        match i.and_then(|i| arrow_at(w, i)) {
            Some(a) => {
                if a.head != Head::MapsTo {
                    issue!(w, "element → element defaulted to {:?}, not ↦", a.head);
                }
                true
            }
            None => {
                w.issue("the drag from g to h made no g→h arrow");
                false
            }
        }
    });

    run.task("B5", "unshade the kernel and make it maroon", 3, KERNEL, |w| {
        let lp = w.label(&ObjRef::Blob("K".into())).unwrap();
        w.click(lp, "K's label");
        if w.selection() != vec![ObjRef::Blob("K".into())] {
            w.issue("clicking K's label didn't select K");
        }
        let sk = w.find_ui("check.", "shad").unwrap_or_default();
        w.click_ui(&sk, "Shaded checkbox");
        w.click_ui("colour.maroon", "maroon swatch");
        w.shot("done");
        let k = blob_of(w, "K").unwrap();
        !k.shaded && k.color.as_deref() == Some("maroon")
    });

    run.task("B6", "get rid of the label on G", 3, KERNEL, |w| {
        let lp = w.label(&ObjRef::Blob("G".into())).unwrap();
        w.click(lp, "G's label");
        w.shot("selected");
        let fk = w.find_ui("field.", "label").unwrap_or_default();
        w.click_ui(&fk, "Label field");
        w.key_raw("cmd-a");
        w.key("backspace", "clear");
        w.key("enter", "done");
        w.tick(200);
        w.shot("cleared");
        let g = blob_of(w, "G").unwrap();
        let diags = w.read(|v| v.doc.diagnostics.iter().map(|d| d.message.clone()).collect::<Vec<_>>());
        note!(w, "G line `{}`; diagnostics {diags:?}", w.line("blob G"));
        g.label.is_empty() && !w.read(|v| v.doc.scene.draws.iter().any(|d| matches!(d, Draw::Label { obj: ObjRef::Blob(n), .. } if n == "G")))
    });

    run.task("B6b", "put H's label inside", 2, KERNEL, |w| {
        let lp = w.label(&ObjRef::Blob("H".into())).unwrap();
        w.click(lp, "H's label");
        let k = w.find_ui("", "inside").unwrap_or_default();
        note!(w, "place controls: {:?}", w.ui_keys().into_iter().filter(|k| k.contains("place")).collect::<Vec<_>>());
        w.click_ui(&k, "Place: inside");
        w.shot("inside");
        blob_of(w, "H").unwrap().label_place == LabelPlace::Inside
    });

    // ===================================================== commutative diagrams
    run.task("D1", "bend this arrow so it doesn't cross", 2, "A at 0,0\nB at 1,0\nC at 2,0\nA -> B\nB -> C\nA -f-> C\n", |w| {
        let i = w.arrow_index("A", "C").unwrap();
        // the long arrow lies on top of A→B and B→C: she aims at the line, then at the label
        let on = w.on_arrow(i, 0.2, 0.0).unwrap();
        w.click(on, "the long arrow f (on its line)");
        let sel = w.selection();
        if sel != vec![ObjRef::Arrow(i)] {
            note!(w, "a click on the overlapped line selected {sel:?} (fair: A→B is under it)");
            let lp = w.label(&ObjRef::Arrow(i)).unwrap();
            w.click(lp, "the label f");
            if w.selection() != vec![ObjRef::Arrow(i)] {
                w.issue("can't select the arrow that crosses: neither its line nor its label picks it");
                return false;
            }
        }
        w.shot("selected");
        let Some(hb) = w.handle(|k| k == HKind::Bend) else {
            w.issue("no bend handle");
            return false;
        };
        if let Some(lh) = w.handle(|k| k == HKind::ArrowLabel) {
            let d = f32::from(lh.x - hb.x).hypot(f32::from(lh.y - hb.y));
            note!(w, "bend handle is {d:.0} px from the label handle");
            if d < 24.0 {
                issue!(w, "bend handle and label handle are {d:.0} px apart: easy to grab the wrong one");
            }
        }
        let gr = w.grab_radius(hb, HKind::Bend);
        if gr >= 0.0 && gr * 2.0 < MIN_TARGET {
            issue!(w, "bend handle: grab area {:.0} px across", gr * 2.0 + 1.0);
        }
        let ppcm = w.px_per_cm();
        let to = point(hb.x, hb.y - px(0.7 * ppcm));
        w.drag(hb, &line_to(hb, to, 30), none, "the bend handle up", Some("bending"));
        w.shot("bent");
        let a = arrow_at(w, i).unwrap();
        note!(w, "bend {}; line `{}`", a.bend, w.line("A -f"));
        a.bend.abs() > 1.0
    });

    run.task("D2", "make that arrow a surjection", 2, QUICK, |w| {
        let i = w.arrow_index("G", "H").unwrap();
        // the hit band, on a lone straight arrow, on the side away from its label
        let lp = w.label(&ObjRef::Arrow(i)).unwrap();
        let on = w.on_arrow(i, 0.3, 0.0).unwrap();
        let away = { let o1 = w.on_arrow(i, 0.3, 1.0).unwrap(); let d1 = f32::from(o1.x - lp.x).hypot(f32::from(o1.y - lp.y)); let d0 = f32::from(on.x - lp.x).hypot(f32::from(on.y - lp.y)); if d1 > d0 { 1.0 } else { -1.0 } };
        let o1 = w.on_arrow(i, 0.3, away).unwrap();
        let dir = (f32::from(o1.x - on.x), f32::from(o1.y - on.y));
        let band = w.reach(on, dir, &move |v: &FigureView| v.hover == Some(ObjRef::Arrow(i)));
        note!(w, "arrow hit band ≈ {band:.0} px off the line (away from the label)");
        if band < 6.0 {
            issue!(w, "a straight arrow is only hit within {band:.0} px of its line");
        }
        w.click(on, "the arrow φ");
        if w.selection() != vec![ObjRef::Arrow(i)] {
            issue!(w, "clicking on the line of G→H selected {:?}", w.selection());
            w.click(lp, "the label φ");
        }
        w.shot("selected");
        let heads: Vec<String> = w.ui_keys().into_iter().filter(|k| k.starts_with("head.")).collect();
        note!(w, "head buttons: {heads:?}");
        w.click_ui("head.TwoHeads", "↠");
        w.shot("surjection");
        let line = w.source();
        if !line.contains("-φ->>") {
            w.issue("her spelling φ wasn't kept on the arrow");
        }
        arrow_at(w, i).unwrap().head == Head::TwoHeads
    });

    run.task("D3", "change f to φ", 3, SQUARE, |w| {
        let i = w.arrow_index("A", "B").unwrap();
        let lp = w.label(&ObjRef::Arrow(i)).unwrap();
        w.dclick(lp, "the label f");
        w.shot("after-double-click");
        if w.read(|v| v.label_edit.is_none()) {
            let why = w.read(|v| v.gesture.is_some());
            issue!(w, "double-clicking an arrow's label doesn't edit it (the second press lands on the label's own swap handle{})", if why { "; a handle drag started" } else { "" });
            w.key("enter", "edit label (the footnote's ↵)");
            if w.read(|v| v.label_edit.is_none()) {
                w.issue("Return on a selected arrow doesn't edit its label either");
                return false;
            }
        }
        w.shot("editing");
        w.key_raw("cmd-a");
        w.typ("$φ$");
        w.key("enter", "set");
        w.shot("phi");
        let src = w.source();
        note!(w, "source:\n{src}");
        if !src.contains("grid") {
            w.issue("relabelling one arrow turned her `grid … end` ASCII block into plain statements");
        }
        src.contains("φ")
    });

    run.task("D3b", "remove the label on f", 2, SQUARE, |w| {
        let i = w.arrow_index("A", "B").unwrap();
        let lp = w.label(&ObjRef::Arrow(i)).unwrap();
        w.right(lp, "the label f");
        let keys: Vec<String> = w.ui_keys().into_iter().filter(|k| k.starts_with("menu.")).collect();
        note!(w, "menu: {keys:?}");
        w.shot("menu");
        // (FIG-INTERACT, round 2: the arrow's menu now has Hide label / Add label)
        if keys.iter().any(|k| k == "menu.ArrowLabel") {
            w.click_ui("menu.ArrowLabel", "Hide label (menu)");
            w.shot("removed");
            let a = arrow_at(w, i);
            note!(w, "source:\n{}", w.source());
            return a.is_some_and(|a| a.label.as_deref().is_none_or(|l| l.is_empty())) && w.read(|v| v.doc.diagnostics.iter().all(|d| d.severity != Severity::Error));
        }
        w.key_raw("escape");
        w.issue("an arrow's right-click menu has no Hide/Remove label (and its “Hide labels” entry is greyed out)");
        // the panel route: the arrow is selected now
        if w.selection() != vec![ObjRef::Arrow(i)] {
            w.click(lp, "the label f");
        }
        let fk = w.find_ui("field.", "label").unwrap_or_default();
        w.click_ui(&fk, "Label field");
        w.key_raw("cmd-a");
        w.key("backspace", "clear");
        w.key("enter", "done");
        w.shot("removed");
        let a = arrow_at(w, i);
        note!(w, "source:\n{}", w.source());
        a.is_some_and(|a| a.label.as_deref().is_none_or(|l| l.is_empty())) && w.read(|v| v.doc.diagnostics.iter().all(|d| d.severity != Severity::Error))
    });

    run.task("D4", "put g's label on the other side", 2, SQUARE, |w| {
        let i = w.arrow_index("A", "C").unwrap();
        let lp = w.label(&ObjRef::Arrow(i)).unwrap();
        let on = w.on_arrow(i, 0.5, 0.0).unwrap();
        let across = point(on.x + (on.x - lp.x), on.y + (on.y - lp.y));
        w.drag(lp, &line_to(lp, across, 20), none, "the label g across the arrow (not selected yet)", Some("dragging-label"));
        let side = arrow_at(w, i).unwrap().side;
        if side == LabelSide::Auto {
            w.issue("dragging an unselected arrow's label across it does nothing; it has to be selected first");
            let lp2 = w.label(&ObjRef::Arrow(i)).unwrap();
            if w.selection() != vec![ObjRef::Arrow(i)] {
                w.click(lp2, "the label g");
            }
            let on = w.on_arrow(i, 0.5, 0.0).unwrap();
            let across = point(on.x + (on.x - lp2.x), on.y + (on.y - lp2.y));
            w.drag(lp2, &line_to(lp2, across, 20), none, "the selected label across", Some("dragging-selected-label"));
        }
        w.shot("after");
        let a = arrow_at(w, i).unwrap();
        note!(w, "side {:?}; source:\n{}", a.side, w.source());
        if !w.source().contains("grid") {
            w.issue("swapping one label turned her `grid … end` ASCII block into plain statements");
        }
        a.side != LabelSide::Auto
    });

    run.task("D5", "turn this sketch into a pullback square", 12, "X -f-> Z\nY -g-> Z\n", |w| {
        w.shot("sketch");
        let cell = w.read(|v| v.cell());
        let ppcm = w.px_per_cm();
        let (cw, ch) = (cell.0 * ppcm, cell.1 * ppcm);
        let x0 = w.centre(&ObjRef::Node("X".into())).unwrap();
        let y0 = w.centre(&ObjRef::Node("Y".into())).unwrap();
        let z0 = w.centre(&ObjRef::Node("Z".into())).unwrap();
        note!(w, "auto-placed (px): X ({:.0},{:.0}) Y ({:.0},{:.0}) Z ({:.0},{:.0})", f32::from(x0.x), f32::from(x0.y), f32::from(y0.x), f32::from(y0.y), f32::from(z0.x), f32::from(z0.y));
        // she wants the pullback layout: P(1,1) X(2,1) Y(1,2) Z(2,2), W(0,0).
        // drag each of Z, X, Y onto its cell; re-check after every drag
        // (others may move), at most 3 rounds
        let cellpx = |w: &mut Win, c: f32, r: f32| -> Point<Pixels> { let cc = w.read(|v| v.cell()); w.xf_px(Pt { x: c * cc.0, y: -r * cc.1 }) };
        let _ = (cw, ch, x0, y0, z0);
        let mut drags = 0;
        for _round in 0..3 {
            let mut all = true;
            for (n, c, r) in [("Z", 2.0, 2.0), ("X", 2.0, 1.0), ("Y", 1.0, 2.0)] {
                let at = w.centre(&ObjRef::Node(n.into())).unwrap();
                let want = cellpx(w, c, r);
                if (f32::from(at.x - want.x)).abs() > 3.0 || (f32::from(at.y - want.y)).abs() > 3.0 {
                    all = false;
                    let before: Vec<(String, Point<Pixels>)> = ["X", "Y", "Z"].iter().filter(|m| **m != n).map(|m| (m.to_string(), w.centre(&ObjRef::Node((*m).into())).unwrap())).collect();
                    w.drag(at, &line_to(at, want, 20), none, &format!("{n} to its cell"), if drags == 0 { Some("first-move") } else { None });
                    drags += 1;
                    for (m, p0) in before {
                        let p1 = w.centre(&ObjRef::Node(m.clone().into())).unwrap();
                        if (f32::from(p1.x - p0.x)).abs() > 3.0 || (f32::from(p1.y - p0.y)).abs() > 3.0 {
                            issue!(w, "dragging one auto-placed node re-flows the others: {m} jumped when {n} was moved");
                        }
                    }
                }
            }
            if all {
                break;
            }
        }
        note!(w, "{drags} drags to lay X, Y, Z out as a pullback; source:\n{}", w.source());
        w.shot("laid-out");
        let p_at = cellpx(w, 1.0, 1.0);
        let x1 = w.centre(&ObjRef::Node("X".into())).unwrap();
        let (cw, ch) = (0.0f32, 0.0f32);
        let _ = (cw, ch);
        w.key("n", "node tool");
        let n0 = w.read(|v| v.doc.figure.items.iter().filter(|i| matches!(i, Item::Node(_))).count());
        w.click(p_at, "the empty corner (P)");
        w.shot("node-placed");
        let n1 = w.read(|v| v.doc.figure.items.iter().filter(|i| matches!(i, Item::Node(_))).count());
        if n1 != n0 + 1 {
            issue!(w, "the node tool's click in the empty corner placed no node (nodes {n0} → {n1})");
            return false;
        }
        let editing = w.read(|v| v.label_edit.is_some());
        if !editing {
            w.issue("the node tool's click doesn't open the label field");
            w.key("enter", "edit label");
        }
        w.key_raw("cmd-a");
        w.typ("$P$");
        w.key("enter", "set");
        let x2 = w.centre(&ObjRef::Node("X".into())).unwrap();
        if (f32::from(x2.x - x1.x)).abs() > 2.0 || (f32::from(x2.y - x1.y)).abs() > 2.0 {
            w.issue("adding a node made the other nodes jump");
        }
        let pname = w.read(|v| v.doc.figure.items.iter().filter_map(|it| if let Item::Node(n) = it { Some(n.name.clone()) } else { None }).last()).unwrap_or_default();
        note!(w, "new node `{pname}`; source:\n{}", w.source());
        w.key("a", "arrow tool");
        for (to, what) in [("X", "P to X"), ("Y", "P to Y")] {
            let a = w.centre(&ObjRef::Node(pname.clone())).unwrap();
            let b = w.centre(&ObjRef::Node(to.into())).unwrap();
            w.drag(a, &line_to(a, b, 20), none, what, None);
            if w.read(|v| v.label_edit.is_some()) {
                w.key("escape", "no label");
            }
        }
        if w.read(|v| v.tool) != ntex_studio::ui::figure::tools::Tool::Arrow {
            w.issue("the arrow tool doesn't stay on between arrows");
        }
        w.shot("legs");
        let w_at = cellpx(w, 0.0, 0.0);
        w.key("n", "node tool");
        w.click(w_at, "up-left of P (W)");
        if w.read(|v| v.label_edit.is_none()) {
            w.key("enter", "edit label");
        }
        w.key_raw("cmd-a");
        w.typ("$W$");
        w.key("enter", "set");
        let wname = w.read(|v| v.doc.figure.items.iter().filter_map(|it| if let Item::Node(n) = it { Some(n.name.clone()) } else { None }).last()).unwrap_or_default();
        w.key("a", "arrow tool");
        let a = w.centre(&ObjRef::Node(wname.clone())).unwrap();
        let b = w.centre(&ObjRef::Node(pname.clone())).unwrap();
        w.drag(a, &line_to(a, b, 20), none, "W to P", None);
        if w.read(|v| v.label_edit.is_some()) {
            w.typ("$\\exists!$");
            w.key("enter", "set");
        }
        let i = w.arrow_index(&wname, &pname);
        w.key("v", "select tool");
        if let Some(i) = i {
            let on = w.on_arrow(i, 0.3, 0.0).unwrap();
            w.click(on, "the W→P arrow");
            let dk = w.find_ui("", "dashed").unwrap_or_default();
            w.click_ui(&dk, "Line: dashed");
        }
        w.shot("pullback");
        let src = w.source();
        note!(w, "source:\n{src}");
        i.and_then(|i| arrow_at(w, i)).is_some_and(|a| a.line == Line::Dashed) && w.arrow_index(&pname, "X").is_some() && w.arrow_index(&pname, "Y").is_some()
    });

    run.task("D6", "delete this square (the n-gon)", 2, "ngon P 4 axes rotation\n", |w| {
        let c = w.centre(&ObjRef::NGon("P".into())).unwrap();
        w.click(c, "the square");
        w.key("backspace", "delete");
        w.shot("deleted");
        first_ngon(w).is_none()
    });

    run.task("D6b", "delete this square (the commutative one, a grid block)", 2, SQUARE, |w| {
        let cb = w.read(|v| v.canvas_bounds.get().unwrap());
        let from = point(cb.origin.x + px(12.), cb.origin.y + px(12.));
        let to = point(cb.origin.x + cb.size.width - px(12.), cb.origin.y + cb.size.height - px(12.));
        w.drag(from, &line_to(from, to, 20), none, "a marquee around it", Some("marquee"));
        let n = w.selection().len();
        note!(w, "marquee took {n} objects");
        w.key("backspace", "delete");
        w.shot("deleted");
        let toast = w.read(|v| v.toast.as_ref().map(|t| t.text.to_string()));
        note!(w, "toast: {toast:?}");
        if toast.as_deref().is_some_and(|t| t.contains("Couldn't")) {
            issue!(w, "deleting the whole selected diagram fails: {}", toast.unwrap());
        }
        w.read(|v| v.doc.figure.items.iter().all(|it| !matches!(it, Item::Node(_) | Item::Arrow(_))))
    });

    run.task("D6c", "delete this diagram (⌘A ⌫, statements)", 2, QUICK, |w| {
        w.key("cmd-a", "select all");
        w.key("backspace", "delete");
        w.shot("deleted");
        let toast = w.read(|v| v.toast.as_ref().map(|t| t.text.to_string()));
        note!(w, "toast: {toast:?}; source {:?}", w.source());
        if toast.as_deref().is_some_and(|t| t.contains("Couldn't")) {
            issue!(w, "⌘A ⌫ fails: {}", toast.unwrap());
        }
        w.read(|v| v.doc.figure.items.iter().all(|it| !matches!(it, Item::Node(_) | Item::Arrow(_))))
    });

    run.task("D6d", "delete one corner of the square", 2, SQUARE, |w| {
        let c = w.centre(&ObjRef::Node("D".into())).unwrap();
        w.click(c, "D");
        w.key("backspace", "delete");
        w.shot("deleted");
        let toast = w.read(|v| v.toast.as_ref().map(|t| t.text.to_string()));
        let diags = w.read(|v| v.doc.diagnostics.iter().filter(|d| d.severity != Severity::Note).map(|d| d.message.clone()).collect::<Vec<_>>());
        note!(w, "toast {toast:?}; diagnostics {diags:?}; source:\n{}", w.source());
        if !diags.is_empty() {
            issue!(w, "deleting D leaves diagnostics: {diags:?}");
        }
        w.read(|v| !v.doc.figure.items.iter().any(|it| matches!(it, Item::Node(n) if n.name == "D")))
    });

    // ===================================================== undo (with a slider and a typed field)
    run.task("G1", "undo that (a Size slider drag, then a typed panel field)", 1, "ngon P 5\n", |w| {
        let c = w.centre(&ObjRef::NGon("P".into())).unwrap();
        w.click(c, "the pentagon");
        let sk = w.find_ui("slider.", "radius").unwrap_or_default();
        let Some(b) = w.ui(&sk) else {
            w.issue("no Size slider");
            return false;
        };
        let u0 = w.read(|v| v.undo.len());
        let s0 = w.source();
        let from = point(b.origin.x + b.size.width * 0.2, b.center().y);
        let to = point(b.origin.x + b.size.width * 0.8, b.center().y);
        // the slider's thumb sits where the value is: press on the track
        w.drag(from, &line_to(from, to, 25), none, "the Size slider", Some("slider"));
        let u1 = w.read(|v| v.undo.len());
        note!(w, "the slider drag made {} undo step(s)", u1 - u0);
        if u1 - u0 > 1 {
            issue!(w, "one slider drag made {} undo steps", u1 - u0);
        }
        w.key("cmd-z", "undo");
        if w.source() != s0 {
            w.issue("one ⌘Z after a slider drag doesn't restore the size");
        }
        let still = w.selection();
        if still != vec![ObjRef::NGon("P".into())] {
            note!(w, "after ⌘Z the selection is {still:?}");
        }
        // the angle field, typed
        let ak = w.find_ui("field.", "angle").unwrap_or_default();
        let u2 = w.read(|v| v.undo.len());
        let s2 = w.source();
        w.click_ui(&ak, "Angle field");
        w.key_raw("cmd-a");
        w.typ("45");
        w.key("enter", "set");
        let u3 = w.read(|v| v.undo.len());
        note!(w, "typing 45 in Angle made {} undo step(s)", u3 - u2);
        if u3 - u2 > 1 {
            issue!(w, "typing “45” in the Angle field made {} undo steps", u3 - u2);
        }
        w.focus_canvas();
        w.key("cmd-z", "undo");
        let back = w.source() == s2;
        if !back {
            w.issue("one ⌘Z after typing in a panel field doesn't restore it");
        }
        w.shot("undone");
        back
    });

    // ===================================================== every control's size, per panel
    run.task("Z1", "(probe) every panel control under 24 px, per selection kind", 0, "ngon P 4\nA at 4,0\nB at 5,0\nA -f-> B\nblob G \"G\" {g, h} at (0,-4)\ncayley Z4 <1> at (6,-4)\n", |w| {
        let targets: Vec<(&str, ObjRef)> = vec![("n-gon", ObjRef::NGon("P".into())), ("node", ObjRef::Node("A".into())), ("blob", ObjRef::Blob("G".into()))];
        for (kind, o) in targets {
            let Some(c) = (if kind == "blob" { w.label(&o) } else { w.centre(&o) }) else { continue };
            w.hover(c);
            w.mouse(c, MouseButton::Left, 1, Modifiers::default());
            let small: Vec<String> = w.read(|v| v.ui_bounds.borrow().iter().filter(|(_, b)| f32::from(b.size.width) < MIN_TARGET || f32::from(b.size.height) < MIN_TARGET).map(|(k, b)| format!("{k} {:.0}×{:.0}", f32::from(b.size.width), f32::from(b.size.height))).collect());
            let mut small = small;
            small.sort();
            note!(w, "{kind}: {}", small.join(", "));
        }
        if let Some(i) = w.arrow_index("A", "B") {
            let lp = w.label(&ObjRef::Arrow(i)).unwrap();
            w.hover(lp);
            w.mouse(lp, MouseButton::Left, 1, Modifiers::default());
            let mut small: Vec<String> = w.read(|v| v.ui_bounds.borrow().iter().filter(|(_, b)| f32::from(b.size.width) < MIN_TARGET || f32::from(b.size.height) < MIN_TARGET).map(|(k, b)| format!("{k} {:.0}×{:.0}", f32::from(b.size.width), f32::from(b.size.height))).collect());
            small.sort();
            note!(w, "arrow: {}", small.join(", "));
        }
        w.shot("probe");
        true
    });


    // ===================================================== figure-qol D: direction and orientation
    // a point on the rotation arrow (px)
    fn rot_pt(w: &mut Win, name: &str, t: f32) -> Option<Point<Pixels>> {
        let n = name.to_string();
        let pts = w.read(move |v| v.doc.scene.draws.iter().find_map(|d| match d {
            Draw::Path { obj: ObjRef::NGonRotation(m), start, segs, .. } if *m == n => Some(geom::flatten(*start, segs)),
            _ => None,
        }))?;
        let q = pts[((pts.len() - 1) as f32 * t).round() as usize];
        Some(w.xf_px(q))
    }
    fn dist_px(a: Point<Pixels>, b: Point<Pixels>) -> f32 {
        f32::from(a.x - b.x).hypot(f32::from(a.y - b.y))
    }
    const REV: &str = "A at 0,0\nB at 2,0\nC at 2,-2\nA -φ-hook-> B bend-left near-start\nA -g-> C\n";

    run.task("QD1", "reverse this arrow with ⌘R", 2, REV, |w| {
        let i = w.arrow_index("A", "B").unwrap();
        let p = w.on_arrow(i, 0.55, 0.0).unwrap();
        w.click(p, "the A → B arrow");
        w.key("cmd-r", "reverse");
        w.shot("reversed");
        let Some(a) = w.arrow_index("B", "A").and_then(|k| arrow_at(w, k)) else { return false };
        let src = w.source();
        note!(w, "source:\n{src}");
        let sel = w.selection();
        if !matches!(sel.as_slice(), [ObjRef::Arrow(_)]) {
            issue!(w, "the arrow isn't selected after reversing: {sel:?}");
        }
        a.head == Head::Hook && a.side == LabelSide::Swap && a.bend == -30.0 && a.label_pos == LabelPos::NearEnd && src.contains("φ")
    });

    run.task("QD2", "reverse this arrow from the right-click menu", 3, REV, |w| {
        let i = w.arrow_index("A", "C").unwrap();
        let p = w.on_arrow(i, 0.5, 0.0).unwrap();
        w.right(p, "the A → C arrow");
        w.shot("menu");
        if !w.click_ui("menu.Reverse", "Reverse") {
            return false;
        }
        w.shot("reversed");
        w.arrow_index("C", "A").is_some() && w.arrow_index("A", "C").is_none()
    });

    run.task("QD3", "reverse this arrow with the ⇄ button in the panel", 3, REV, |w| {
        let i = w.arrow_index("A", "B").unwrap();
        let p = w.on_arrow(i, 0.55, 0.0).unwrap();
        w.click(p, "the arrow");
        let key = w.find_ui("button.arrow.", "reverse").unwrap_or_default();
        if !w.click_ui(&key, "⇄ Reverse") {
            return false;
        }
        w.shot("reversed");
        let ok1 = w.arrow_index("B", "A").is_some();
        // and back again
        let key = w.find_ui("button.arrow.", "reverse").unwrap_or_default();
        w.click_ui(&key, "⇄ Reverse");
        ok1 && w.arrow_index("A", "B").is_some() && w.source().contains("φ")
    });

    run.task("QD4", "the rotation arrow goes the other way (↻ in the panel)", 3, "ngon P 4 axes rotation\n", |w| {
        let Some(p) = rot_pt(w, "P", 0.5) else { return false };
        w.click(p, "the rotation arrow");
        let sel = w.selection();
        if sel != vec![ObjRef::NGonRotation("P".into())] {
            issue!(w, "clicking the rotation arrow selects {sel:?}");
            return false;
        }
        w.shot("selected");
        if !w.click_ui("ngon.P.rot.cw", "↻") {
            return false;
        }
        w.shot("clockwise");
        let g = first_ngon(w).unwrap();
        let label = w.read(|v| v.doc.scene.draws.iter().find_map(|d| match d { Draw::Label { obj: ObjRef::NGonRotation(_), latex, .. } => Some(latex.clone()), _ => None }));
        note!(w, "line: {}; label {label:?}", w.line("ngon"));
        // ⌘R runs it back
        w.focus_canvas();
        w.key("cmd-r", "reverse");
        let back = first_ngon(w).unwrap();
        g.rotation_cw && label.as_deref() == Some("r^{-1}") && !back.rotation_cw && w.line("ngon") == "ngon P 4 axes rotation"
    });

    run.task("QD5", "slide the rotation arrow round to another edge", 1, "ngon P 5 axes rotation\n", |w| {
        let c = w.centre(&ObjRef::NGon("P".into())).unwrap();
        let Some(p) = rot_pt(w, "P", 0.5) else { return false };
        let r = dist_px(c, p);
        let from_deg = angle_px(c, p);
        let want = ntex_figures::layout::rotation_edge_angle(5, Some(2));
        w.shot("before");
        w.drag(p, &arc(c, r, from_deg, from_deg + ((want - from_deg + 540.0).rem_euclid(360.0) - 180.0), 14), none, "the rotation arrow round to edge 2", Some("sliding"));
        w.shot("moved");
        let g = first_ngon(w).unwrap();
        note!(w, "line: {}", w.line("ngon"));
        let sel = w.selection();
        g.rotation_edge == Some(2) && w.line("ngon").contains("edge=2") && sel == vec![ObjRef::NGonRotation("P".into())]
    });

    run.task("QD6", "turn the square the other way (⟳) and back", 3, "ngon P 4\n", |w| {
        let c = w.centre(&ObjRef::NGon("P".into())).unwrap();
        w.click(c, "the square");
        if !w.click_ui("button.ngon.P.turn-", "⟳") {
            return false;
        }
        let g = first_ngon(w).unwrap();
        w.shot("turn-minus-one");
        let line = w.line("ngon");
        let ok1 = g.turn == 3 && g.turn_neg && line == "ngon P 4 turn=-1";
        w.click_ui("button.ngon.P.turn-", "⟳");
        let g2 = first_ngon(w).unwrap();
        w.click_ui("button.ngon.P.turn+", "⟲");
        w.click_ui("button.ngon.P.turn+", "⟲");
        w.click_ui("button.ngon.P.turn+", "⟲");
        let g3 = first_ngon(w).unwrap();
        note!(w, "after ⟳⟳: {}; after ⟲⟲⟲: {}", g2.turn, g3.turn);
        ok1 && g2.turn == 2 && g2.turn_neg && g3.turn == 1 && !g3.turn_neg && w.line("ngon") == "ngon P 4 turn=1"
    });

    run.task("QD7", "draw only r, backwards (r⁻¹) in maroon, and hide s, on this Cayley graph", 5, "cayley D4 <r, s> legend\n", |w| {
        let cg = cayley_of(w).unwrap();
        let top = top_vertex(w, &cg.name).unwrap();
        let q = vertex_pt(w, &cg.name, &top).unwrap();
        let q = w.xf_px(q);
        w.click(q, "the graph");
        w.shot("rows");
        let (sel, keys) = (w.selection(), w.ui_keys());
        note!(w, "selection {sel:?}; controls {}", keys.join(" "));
        let ok_rows = w.ui("cayley.D4.gen0.inv").is_some() && w.ui("check.cayley.D4.gen1.shown").is_some();
        if !ok_rows {
            issue!(w, "no per-generator rows");
            return false;
        }
        w.click_ui("cayley.D4.gen0.colour.maroon", "r: maroon");
        w.click_ui("cayley.D4.gen0.inv", "r⁻¹");
        w.click_ui("check.cayley.D4.gen1.shown", "s: shown off");
        w.shot("styled");
        let c = cayley_of(w).unwrap();
        note!(w, "line: {}", w.line("cayley"));
        let inv_row = w.ui("cayley.D4.gen1.inv");
        if inv_row.is_some() {
            issue!(w, "an involution offers g⁻¹");
        }
        w.line("cayley") == "cayley D4 <r, s> gens r^-1:maroon,s:hidden legend"
            && c.gen_styles.len() == 2
    });

    run.task("QD8", "move this arrow's head from B to C (drag its end dot)", 2, "A at 0,0\nB at 2,0\nC at 2,-2\nA -f-> B\n", |w| {
        let i = w.arrow_index("A", "B").unwrap();
        let p = w.on_arrow(i, 0.5, 0.0).unwrap();
        w.click(p, "the arrow");
        let Some(head) = w.handle(|k| k == HKind::ArrowEnd(true)) else {
            issue!(w, "no dot at the arrow's head");
            return false;
        };
        let tail = w.handle(|k| k == HKind::ArrowEnd(false));
        if tail.is_none() {
            issue!(w, "no dot at the arrow's tail");
        }
        w.shot("dots");
        let cpx = w.centre(&ObjRef::Node("C".into())).unwrap();
        // by hand, to look at the drop target mid-drag
        w.hover(head);
        w.event(PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: head, modifiers: none, click_count: 1, first_mouse: false, pen: None }));
        w.draw();
        for q in line_to(head, cpx, 10) {
            w.event(PlatformInput::MouseMove(MouseMoveEvent { position: q, pressed_button: Some(MouseButton::Left), modifiers: none, pen: None }));
            w.draw();
        }
        let hov = w.read(|v| v.hover.clone());
        w.shot("over-C");
        w.event(PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: cpx, modifiers: none, click_count: 1, pen: None }));
        w.draw();
        w.step("drag the head dot onto C");
        if hov != Some(ObjRef::Node("C".into())) {
            issue!(w, "C isn't highlighted while the end is over it: {hov:?}");
        }
        w.shot("moved");
        let ok1 = w.arrow_index("A", "C").is_some() && w.arrow_index("A", "B").is_none();
        // the tail onto B
        let i = w.arrow_index("A", "C").unwrap();
        let p = w.on_arrow(i, 0.5, 0.0).unwrap();
        w.click(p, "the arrow");
        let tail = w.handle(|k| k == HKind::ArrowEnd(false)).unwrap();
        let bpx = w.centre(&ObjRef::Node("B".into())).unwrap();
        w.drag(tail, &line_to(tail, bpx, 10), none, "the tail dot onto B", None);
        ok1 && w.arrow_index("B", "C").is_some()
    });

    run.task("QD9", "drop the end on nothing: the arrow stays", 2, "A at 0,0\nB at 2,0\nA -f-> B\n", |w| {
        let i = w.arrow_index("A", "B").unwrap();
        let p = w.on_arrow(i, 0.5, 0.0).unwrap();
        w.click(p, "the arrow");
        let head = w.handle(|k| k == HKind::ArrowEnd(true)).unwrap();
        let s0 = w.source();
        let away = point(head.x + px(10.), head.y + px(150.));
        w.drag(head, &line_to(head, away, 8), none, "the head dot to empty canvas", Some("tether"));
        w.source() == s0
    });

    // ===================================================== class C: editing labels after placing them
    fn node_label(w: &mut Win, n: &str) -> String {
        w.read(|v| v.doc.figure.items.iter().find_map(|it| match it { Item::Node(x) if x.name == n => Some(x.label.clone()), _ => None })).unwrap_or_default()
    }
    fn editing(w: &mut Win) -> Option<ObjRef> {
        w.read(|v| v.label_edit.as_ref().map(|e| e.obj.clone()))
    }
    fn field_text(w: &mut Win) -> (String, bool) {
        w.window.update(&mut w.cx, |v, _, cx| (v.label_field.read(cx).text().to_string(), v.label_field.read(cx).math())).unwrap()
    }
    const QC_SRC: &str = "A \"\\mathbb{Z}\" at 0,0\nB \"b\" at 3,0\nA -f-> B\n";

    run.task("QC1", "edit a placed label: no $…$, a text chip, a click on the selected label", 4, QC_SRC, |w| {
        let a = ObjRef::Node("A".into());
        let c = w.centre(&a).unwrap();
        w.click(c, "node A (selects it)");
        if editing(w).is_some() {
            w.issue("the first click on an unselected node opened its field");
        }
        w.click(c, "node A again (its label, already selected)");
        let Some(o) = editing(w) else {
            w.issue("a click on the label of the selected node doesn't edit it");
            return false;
        };
        let (text, math) = field_text(w);
        if text.contains('$') || !math || text != r"\mathbb{Z}" {
            issue!(w, "the field shows {text:?} (maths {math}); expected `\\mathbb{{Z}}` with no $");
        }
        // beside the label (above it), not over it
        let chip = w.ui("label.text");
        let lp = w.label(&o).unwrap();
        match chip {
            Some(b) if f32::from(b.center().y) < f32::from(lp.y) - 8.0 => {}
            Some(b) => issue!(w, "the field isn't above the label: chip y {:.0}, label y {:.0}", f32::from(b.center().y), f32::from(lp.y)),
            None => w.issue("no `text` chip beside the field"),
        }
        w.shot("editing-above");
        // the canvas keeps drawing the label as typed
        w.key_raw("cmd-a");
        w.typ("Q");
        let live = w.read(|v| v.label_preview.as_ref().is_some_and(|s| s.draws.iter().any(|d| matches!(d, Draw::Label { obj, latex, .. } if *obj == o && latex == "Q"))));
        if !live {
            w.issue("the canvas isn't drawing the label live as typed");
        }
        w.key("escape", "revert");
        if node_label(w, "A") != r"\mathbb{Z}" || editing(w).is_some() {
            w.issue("Esc doesn't revert and close");
        }
        // text mode through the chip
        w.click(c, "node A again");
        w.click_ui("label.text", "the text chip");
        w.key_raw("cmd-a");
        w.typ("Fun fact");
        w.shot("text-mode");
        w.key("enter", "commit");
        let l = node_label(w, "A");
        note!(w, "A is now {l:?}");
        if l != r"\text{Fun fact}" {
            issue!(w, "text mode committed {l:?}");
        }
        // reopening shows the text without its wrapper, chip on
        w.click(c, "node A (selected)");
        let (text, math) = field_text(w);
        if text != "Fun fact" || math {
            issue!(w, "a text label reopens as {text:?} (maths {math})");
        }
        w.key("escape", "close");
        node_label(w, "A") == r"\text{Fun fact}"
    });

    run.task("QC2", "snippets and spelling still work with the $ hidden", 5, QC_SRC, |w| {
        let b = ObjRef::Node("B".into());
        let c = w.centre(&b).unwrap();
        w.dclick(c, "node B");
        // her snippets come from the real hidden ntex: wait for it, as a warm window has
        let start = Instant::now();
        while !w.window.update(&mut w.cx, |v, _, cx| v.label_field.read(cx).ready()).unwrap() {
            assert!(start.elapsed() < Duration::from_secs(30), "ntex never became ready");
            w.tick(2);
            std::thread::sleep(Duration::from_millis(2));
        }
        w.key_raw("cmd-a");
        w.typ("//a");
        w.key("tab", "to the denominator");
        w.typ("b");
        let (t, _) = field_text(w);
        w.shot("fraction");
        w.key("enter", "commit");
        let l = node_label(w, "B");
        if l != r"\frac{a}{b}" {
            issue!(w, "// gave {l:?} (field showed {t:?})");
        }
        w.click(c, "node B (selected)");
        w.key_raw("cmd-a");
        w.typ("φ");
        w.key("enter", "commit");
        let src = w.source();
        if !src.contains("φ") {
            issue!(w, "φ was not kept as typed:\n{src}");
        }
        w.click(c, "node B (selected)");
        let (t, _) = field_text(w);
        if t != "φ" {
            issue!(w, "φ reopens as {t:?}");
        }
        w.key("escape", "close");
        // the panel too: no $, a text chip
        let has_chip = w.ui("chip.node.B.label").is_some();
        if !has_chip {
            w.issue("the panel's Label field has no text chip");
        }
        let ptxt = w.window.update(&mut w.cx, |v, _, cx| v.fields.get("node.B.label").map(|f| f.entity.read(cx).text().to_string())).unwrap();
        if ptxt.as_deref() != Some("φ") {
            issue!(w, "the panel's Label field shows {ptxt:?}");
        }
        w.shot("panel");
        has_chip && src.contains("φ") && l == r"\frac{a}{b}"
    });

    run.task("QC3", "an arrow's label: a click selects, a drag swaps, the field is across the line", 5, QC_SRC, |w| {
        let i = w.arrow_index("A", "B").unwrap();
        let o = ObjRef::Arrow(i);
        let lp = w.label(&o).unwrap();
        let s0 = w.source();
        w.click(lp, "the label f");
        if w.selection() != vec![o.clone()] {
            issue!(w, "a click on an arrow's label selected {:?}", w.selection());
        }
        if w.source() != s0 || editing(w).is_some() {
            w.issue("a click alone on the label started something (a swap or an edit)");
        }
        // the swap drag still works, from the selected arrow
        let across = point(lp.x, lp.y + px(40.));
        w.drag(lp, &line_to(lp, across, 12), none, "the label across the line", Some("swapping"));
        let swapped = w.source() != s0;
        if !swapped {
            w.issue("dragging the label across no longer swaps its side");
        }
        // undo the swap, then edit with a click on the selected label
        w.focus_canvas();
        w.key_raw("cmd-z");
        let lp = w.label(&o).unwrap();
        w.click(lp, "the label f (arrow selected)");
        if editing(w) != Some(o.clone()) {
            w.issue("a click on the selected arrow's label doesn't edit it");
            return false;
        }
        let line = w.on_arrow(i, 0.5, 0.0).unwrap();
        let chip = w.ui("label.text").map(|b| b.center());
        match chip {
            // the label sits above the line (smaller y): the field is below it
            Some(c) if (f32::from(c.y) - f32::from(line.y)) * (f32::from(lp.y) - f32::from(line.y)) < 0.0 => {}
            Some(c) => issue!(w, "the field isn't across the line: field y {:.0}, line y {:.0}, label y {:.0}", f32::from(c.y), f32::from(line.y), f32::from(lp.y)),
            None => w.issue("no chip"),
        }
        w.shot("field-across");
        w.key_raw("cmd-a");
        w.typ("g");
        w.key("enter", "commit");
        let a = arrow_at(w, i).unwrap();
        a.label.as_deref() == Some("g") && swapped
    });

    const QC_WALK: &str = "A \"a\" at 0,0\nB \"b\" at 3,0\nC \"c\" at 0,-2\nA -f-> B\nngon P 4 at 6,-1 labels p,q,r,s\n";
    run.task("QC4", "Tab and ⇧Tab walk every label in reading order", 6, QC_WALK, |w| {
        let expect = w.read(|v| v.label_order(None));
        note!(w, "reading order: {expect:?}");
        // independent of the implementation: each step goes down a line or right along one
        let ys: Vec<(f32, f32)> = expect.iter().map(|o| w.read(|v| v.label_spot(o)).map(|p| (p.x, p.y)).unwrap()).collect();
        for pair in ys.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if !(b.1 < a.1 - 0.05 || (b.1 - a.1).abs() <= 0.5 && b.0 >= a.0 - 0.01) {
                issue!(w, "not reading order: {a:?} then {b:?}");
            }
        }
        // top row first (A, f, B and the polygon's top labels by x), …
        let kinds: Vec<&str> = expect.iter().map(|o| match o { ObjRef::Node(_) => "node", ObjRef::Arrow(_) => "arrow", ObjRef::NGonVertex(..) => "vertex", _ => "other" }).collect();
        if !(kinds.contains(&"node") && kinds.contains(&"arrow") && kinds.contains(&"vertex")) {
            issue!(w, "the walk doesn't cover nodes, arrows and vertices: {kinds:?}");
        }
        let first = expect[0].clone();
        let c = w.label(&first).unwrap();
        w.dclick(c, "the first label");
        let mut seen = vec![editing(w).unwrap()];
        for _ in 1..expect.len() {
            w.key("tab", "next label");
            match editing(w) {
                Some(o) => seen.push(o),
                None => {
                    w.issue("Tab left editing before the walk ended");
                    break;
                }
            }
        }
        w.shot("walking");
        if seen != expect {
            issue!(w, "Tab walked {seen:?}, expected {expect:?}");
        }
        w.key("tab", "wraps");
        if editing(w) != Some(first.clone()) {
            w.issue("Tab doesn't wrap to the first label");
        }
        w.key("shift-tab", "back");
        if editing(w) != expect.last().cloned() {
            w.issue("⇧Tab doesn't go back to the last label");
        }
        // Return commits where it stands
        w.key_raw("cmd-a");
        w.typ("z");
        w.key("enter", "commit");
        let n = w.read(|v| v.doc.figure.items.iter().any(|it| matches!(it, Item::NGon(g) if g.labels.iter().any(|l| l == "z"))));
        if !n || editing(w).is_some() {
            w.issue("Return didn't commit the last label");
        }
        seen == expect && n
    });

    run.task("QC5", "name a Cayley vertex", 4, "cayley Z4 <1> at 0,0\n", |w| {
        let top = top_vertex(w, "Z4").unwrap();
        let vp = vertex_pt(w, "Z4", &top).unwrap();
        let p = w.xf_px(vp);
        w.click(p, "the graph");
        w.click(p, "the vertex (now selected)");
        w.click(p, "the selected vertex's name");
        let want = ObjRef::CayleyVertex("Z4".into(), top.clone());
        if editing(w) != Some(want) {
            issue!(w, "editing {:?}", editing(w));
            return false;
        }
        w.key_raw("cmd-a");
        w.typ("x");
        w.shot("vertex-name");
        w.key("enter", "commit");
        let src = w.source();
        note!(w, "line `{}`", w.line("cayley"));
        w.shot("renamed");
        let drawn = w.read(|v| v.doc.scene.draws.iter().any(|d| matches!(d, Draw::Label { obj: ObjRef::CayleyVertex(_, x), latex, .. } if *x == top && latex == "x")));
        src.contains("vertex") && drawn
    });

    run.task("QC6", "name the rotation arrow and an axis", 7, "ngon P 4 axes rotation\n", |w| {
        let c = w.centre(&ObjRef::NGon("P".into())).unwrap();
        w.click(c, "the polygon");
        // the rotation label r
        let (rp, ap) = w.read(|v| {
            let pt = |o: &ObjRef| v.doc.scene.draws.iter().find_map(|d| match d { Draw::Label { obj, at, .. } if obj == o => Some(*at), _ => None });
            (pt(&ObjRef::NGonRotation("P".into())), pt(&ObjRef::NGonAxis("P".into(), 2)))
        });
        let rp = rp.unwrap();
        let rp = w.xf_px(rp);
        // the rotation arrow is a part of its own (class D): the first click on
        // its r selects it, a second click (Keynote's rule) edits the label
        w.click(rp, "the rotation arrow's r (selects the arrow)");
        if w.read(|v| v.selection.clone()) != vec![ObjRef::NGonRotation("P".into())] {
            issue!(w, "selection {:?}", w.read(|v| v.selection.clone()));
            return false;
        }
        w.click(rp, "the rotation arrow's r again (arrow selected)");
        if editing(w) != Some(ObjRef::NGonRotationLabel("P".into())) {
            issue!(w, "editing {:?}", editing(w));
            return false;
        }
        w.key_raw("cmd-a");
        w.typ("ρ");
        w.shot("rotation-name");
        w.key("enter", "commit");
        let l = w.line("ngon");
        note!(w, "line `{l}`");
        let rot_ok = l.contains("\"ρ\"");
        // an axis has no label until it is given one: from the panel's Names row
        // (the rotation arrow is still selected: back to the polygon's panel)
        let _ = ap;
        let c = w.centre(&ObjRef::NGon("P".into())).unwrap();
        w.click(c, "the polygon");
        let nk = w.find_ui("field.", "axis2").unwrap_or_default();
        if !w.click_ui(&nk, "the axis 1 name field") {
            return false;
        }
        w.typ("s");
        w.key("enter", "commit");
        let l = w.line("ngon");
        note!(w, "line `{l}`");
        let ap = w.read(|v| v.doc.scene.draws.iter().find_map(|d| match d { Draw::Label { obj: ObjRef::NGonAxis(_, 2), at, .. } => Some(*at), _ => None }));
        let Some(ap) = ap else {
            w.issue("the axis label isn't drawn");
            return false;
        };
        let ap = w.xf_px(ap);
        w.shot("axis-name-set");
        // in place: a click on the axis's label of the selected polygon
        w.click(ap, "the axis's s");
        let in_place = editing(w) == Some(ObjRef::NGonAxis("P".into(), 2));
        if !in_place {
            issue!(w, "clicking the axis's label edits {:?}", editing(w));
        } else {
            w.key_raw("cmd-a");
            w.typ("t");
            w.key("enter", "commit");
        }
        w.shot("axis-name");
        let l = w.line("ngon");
        rot_ok && in_place && l.contains("axis 1 \"t\"")
    });


    // ===================================================== class A: the object-handling grammar
    let alt = Modifiers { alt: true, ..Default::default() };
    let shift = Modifiers { shift: true, ..Default::default() };
    let cmd = Modifiers { platform: true, ..Default::default() };
    fn node_pos(w: &mut Win, n: &str) -> Option<Pos> {
        w.read(|v| v.doc.figure.items.iter().find_map(|it| match it { Item::Node(x) if x.name == n => x.pos, _ => None }))
    }
    fn nodes_n(w: &mut Win) -> usize {
        w.read(|v| v.doc.figure.items.iter().filter(|it| matches!(it, Item::Node(_))).count())
    }
    fn arrows_n(w: &mut Win) -> usize {
        w.read(|v| v.doc.figure.items.iter().filter(|it| matches!(it, Item::Arrow(_))).count())
    }
    fn undo_n(w: &mut Win) -> usize {
        w.read(|v| v.undo.len())
    }
    fn clip_text(w: &mut Win) -> Option<String> {
        w.cx.update(|cx| cx.read_from_clipboard().and_then(|c| c.text()))
    }
    fn clip_set(w: &mut Win, t: &str) {
        let t = t.to_string();
        w.cx.update(|cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(t)));
    }
    fn near(a: Option<Pos>, c: f32, r: f32) -> bool {
        matches!(a, Some(Pos::Cell(x, y)) if (x - c).abs() < 0.02 && (y - r).abs() < 0.02)
    }
    fn cell_px(w: &mut Win) -> (f32, f32) {
        let ppc = w.px_per_cm();
        let c = w.read(|v| v.cell());
        (c.0 * ppc, c.1 * ppc)
    }
    fn toast_text(w: &mut Win) -> String {
        w.read(|v| v.toast.as_ref().map(|t| t.text.to_string()).unwrap_or_default())
    }
    fn shift_click(w: &mut Win, at: Point<Pixels>, what: &str) {
        w.hover(at);
        w.mouse(at, MouseButton::Left, 1, Modifiers { shift: true, ..Default::default() });
        w.step(format!("⇧-click {what}"));
    }
    const QA_PAIR: &str = "A at 0,0\nB at 1,0\nC at 4,0\nA -f-> B\n";
    static QA_CLIP: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

    run.task("QA1", "⌥-drag: leave the originals and drag copies, arrows between them too", 4, QA_PAIR, |w| {
        let (a, b) = (ObjRef::Node("A".into()), ObjRef::Node("B".into()));
        let ca = w.centre(&a).unwrap();
        w.click(ca, "node A");
        let cb = w.centre(&b).unwrap();
        shift_click(w, cb, "node B");
        let u0 = undo_n(w);
        let (_, ch) = cell_px(w);
        let to = point(ca.x, ca.y + px(ch));
        w.drag(ca, &line_to(ca, to, 8), alt, "A, with ⌥, one row down", Some("copying"));
        w.shot("copied");
        let sel = w.selection();
        let (na, nb) = (node_pos(w, "A"), node_pos(w, "B"));
        let src = w.source();
        note!(w, "source:\n{src}");
        let copies: Vec<String> = sel.iter().filter_map(|o| if let ObjRef::Node(n) = o { Some(n.clone()) } else { None }).collect();
        if undo_n(w) != u0 + 1 {
            issue!(w, "the copy drag took {} undo steps", undo_n(w) - u0);
        }
        if copies.len() != 2 {
            issue!(w, "the copies aren't the selection: {sel:?}");
            return false;
        }
        let (p0, p1) = (node_pos(w, &copies[0]), node_pos(w, &copies[1]));
        let ok_arrow = w.arrow_index(&copies[0], &copies[1]).is_some() || w.arrow_index(&copies[1], &copies[0]).is_some();
        near(na, 0.0, 0.0) && near(nb, 1.0, 0.0) && nodes_n(w) == 5 && arrows_n(w) == 2 && ok_arrow
            && ((near(p0, 0.0, 1.0) && near(p1, 1.0, 1.0)) || (near(p0, 1.0, 1.0) && near(p1, 0.0, 1.0)))
    });

    run.task("QA2", "drag one of the selection: the whole selection moves", 3, "A at 0,0\nB at 2,0\nC at 0,1\nD at 4,3\nA -> B\n", |w| {
        w.focus_canvas();
        w.key("cmd-a", "select all");
        w.shot("all-selected");
        let b = w.centre(&ObjRef::Node("B".into())).unwrap();
        let (cw, _) = cell_px(w);
        w.drag(b, &line_to(b, point(b.x + px(cw), b.y), 8), none, "B one cell right", Some("moving"));
        w.shot("moved");
        let (a, b2, c, d) = (node_pos(w, "A"), node_pos(w, "B"), node_pos(w, "C"), node_pos(w, "D"));
        note!(w, "A {a:?} B {b2:?} C {c:?} D {d:?}");
        near(a, 1.0, 0.0) && near(b2, 3.0, 0.0) && near(c, 1.0, 1.0) && near(d, 5.0, 3.0)
    });

    run.task("QA3", "⇧-drag locks to horizontal or vertical", 3, "A at 0,0\nB at 3,0\nngon P 5 at (1,-3)\n", |w| {
        let a = w.centre(&ObjRef::Node("A".into())).unwrap();
        let (cw, ch) = cell_px(w);
        // more across than down: only across
        w.drag(a, &line_to(a, point(a.x + px(2.0 * cw), a.y + px(0.4 * ch)), 8), shift, "A across, a little down, with ⇧", Some("locked"));
        let first = node_pos(w, "A");
        // more down than across: only down
        let b = w.centre(&ObjRef::Node("B".into())).unwrap();
        w.drag(b, &line_to(b, point(b.x + px(0.4 * cw), b.y + px(2.0 * ch)), 8), shift, "B down, a little across, with ⇧", None);
        let second = node_pos(w, "B");
        // a shape locks too
        let p = w.centre(&ObjRef::NGon("P".into())).unwrap();
        w.drag(p, &line_to(p, point(p.x + px(0.3 * cw), p.y + px(1.0 * ch)), 8), shift, "the polygon, mostly down, with ⇧", None);
        let pg = first_ngon(w).unwrap();
        note!(w, "A {first:?} B {second:?} P {:?}", pg.pos);
        let before = Pos::Abs(1.0, -3.0);
        near(first, 2.0, 0.0) && near(second, 3.0, 2.0) && matches!(pg.pos, Some(Pos::Cell(..)) | Some(Pos::Abs(..))) && pg.pos != Some(before)
            && match pg.pos { Some(Pos::Abs(x, _)) => (x - 1.0).abs() < 0.11, Some(Pos::Cell(c, _)) => (c - 1.0 / 1.9).abs() < 0.06, None => false }
    });

    run.task("QA4", "⌘ during a drag turns snapping off", 3, "A at 0,0\nB at 3,0\nngon P 5 at (1,-3)\n", |w| {
        let a = w.centre(&ObjRef::Node("A".into())).unwrap();
        let (cw, ch) = cell_px(w);
        // without ⌘: on a cell
        let b = w.centre(&ObjRef::Node("B".into())).unwrap();
        w.drag(b, &line_to(b, point(b.x + px(0.37 * cw), b.y + px(0.21 * ch)), 8), none, "B a bit, snapping", None);
        let snapped = node_pos(w, "B");
        // with ⌘: where it was dropped
        w.drag(a, &line_to(a, point(a.x + px(0.37 * cw), a.y + px(0.21 * ch)), 8), cmd, "A the same bit, with ⌘", Some("free"));
        let free = node_pos(w, "A");
        let g0 = first_ngon(w).unwrap();
        let p = w.centre(&ObjRef::NGon("P".into())).unwrap();
        w.drag(p, &line_to(p, point(p.x + px(0.13 * cw), p.y + px(0.0)), 8), cmd, "the polygon a hair, with ⌘", None);
        let g1 = first_ngon(w).unwrap();
        note!(w, "B {snapped:?}; A {free:?}; P {:?} → {:?}", g0.pos, g1.pos);
        let moved_free = matches!(free, Some(Pos::Abs(x, y)) if (x - 0.37 * 1.9).abs() < 0.12 && (y + 0.21 * 1.35).abs() < 0.12 && x.abs() > 0.05);
        let p_free = matches!((g0.pos, g1.pos), (Some(Pos::Abs(x0, _)), Some(Pos::Abs(x1, _))) if (x1 - x0 - 0.13 * 1.9).abs() < 0.12 && x1 != x0);
        near(snapped, 3.0, 0.0) && moved_free && p_free
    });

    run.task("QA4b", "⌘ during a turn drag: any angle, not 15° steps", 3, "ngon P 5 at (1,-1)\n", |w| {
        w.key("v", "select tool");
        let c = w.centre(&ObjRef::NGon("P".into())).unwrap();
        w.click(c, "the polygon");
        let Some(k) = w.handle(|k| k == HKind::Rotate) else { return false };
        let g = angle_px(c, k);
        w.drag(k, &arc(c, f32::from(k.y - c.y).hypot(f32::from(k.x - c.x)), g, g + 22.0, 10), none, "the knob about 22°", None);
        let snapped = first_ngon(w).unwrap().angle;
        let Some(k) = w.handle(|k| k == HKind::Rotate) else { return false };
        let g = angle_px(c, k);
        w.drag(k, &arc(c, f32::from(k.y - c.y).hypot(f32::from(k.x - c.x)), g, g + 22.0, 10), cmd, "the knob about 22° more, with ⌘", Some("free-turn"));
        let free = first_ngon(w).unwrap().angle;
        note!(w, "angle after snapped turn {snapped}, after free turn {free}");
        (snapped / 15.0).fract().abs() < 1e-3 && snapped != 0.0 && (free / 15.0).fract().abs() > 0.05
    });

    run.task("QA5", "⌘C then ⌘V twice: shorthand on the clipboard, pasted offset, repeating", 5, "A at 0,0\nB at 2,0\nC at 4,0\nA -f-> B\nB -g-> C\n", |w| {
        let ca = w.centre(&ObjRef::Node("A".into())).unwrap();
        w.click(ca, "node A");
        let cb = w.centre(&ObjRef::Node("B".into())).unwrap();
        shift_click(w, cb, "node B");
        w.key("cmd-c", "copy");
        let Some(text) = clip_text(w) else {
            w.issue("⌘C left nothing on the clipboard");
            return false;
        };
        note!(w, "clipboard:\n{text}");
        *QA_CLIP.lock().unwrap() = text.clone();
        let shorthand_ok = text.contains("-f->") && text.contains('A') && !text.contains("-g->") && !text.contains('C');
        if !shorthand_ok {
            issue!(w, "the clipboard isn't the copied shorthand: {text:?}");
        }
        let u0 = undo_n(w);
        w.key("cmd-v", "paste");
        w.shot("pasted");
        let sel = w.selection();
        let n1 = nodes_n(w);
        let first: Vec<String> = sel.iter().filter_map(|o| if let ObjRef::Node(n) = o { Some(n.clone()) } else { None }).collect();
        if first.len() != 2 || n1 != 5 {
            issue!(w, "pasting selects {sel:?}; {n1} nodes");
            return false;
        }
        let p1 = (node_pos(w, &first[0]), node_pos(w, &first[1]));
        w.key("cmd-v", "paste again");
        w.shot("pasted-again");
        let second: Vec<String> = w.selection().iter().filter_map(|o| if let ObjRef::Node(n) = o { Some(n.clone()) } else { None }).collect();
        let p2 = (node_pos(w, &second[0]), node_pos(w, &second[1]));
        note!(w, "first paste {p1:?}, second {p2:?}; undo steps {}", undo_n(w) - u0);
        let steps = undo_n(w) - u0;
        // A at (0,0) → a cell right and down (nodes stay on the grid: review U3), then another
        let pa = |p: (Option<Pos>, Option<Pos>), c: f32, r: f32| near(p.0, c, r) || near(p.1, c, r);
        shorthand_ok && steps == 2 && nodes_n(w) == 7 && arrows_n(w) == 4 && pa(p1, 1.0, 1.0) && pa(p1, 3.0, 1.0) && pa(p2, 2.0, 2.0) && pa(p2, 4.0, 2.0)
    });

    run.task("QA6", "paste shorthand from another window or Neovim, and refuse text that isn't", 3, "A at 0,0\nB at 2,0\nA -f-> B\n", |w| {
        // what ⌘C put on the clipboard in QA5's window, as another window would find it
        let theirs = QA_CLIP.lock().unwrap().clone();
        let text = if theirs.is_empty() { "X at 0,0\nY at 2,0\nX -h-> Y\n".to_string() } else { theirs };
        clip_set(w, &text);
        w.focus_canvas();
        let s0 = w.source();
        w.key("cmd-v", "paste");
        w.shot("pasted-foreign");
        let n = nodes_n(w);
        let sel = w.selection();
        note!(w, "after paste: {} nodes, selection {sel:?}", n);
        let pasted = n == 4 && sel.len() >= 2;
        // text that isn't shorthand: refused, nothing changes
        clip_set(w, "Dear Bob, thanks for the notes. See you at 5.");
        let s1 = w.source();
        let u1 = undo_n(w);
        w.key("cmd-v", "paste prose");
        let refused = w.source() == s1 && undo_n(w) == u1;
        let t = toast_text(w);
        note!(w, "toast: {t}");
        w.shot("refused");
        if !refused {
            w.issue("pasting prose changed the figure");
        }
        if !t.contains("Nothing to paste") {
            issue!(w, "no note about the refused paste: {t:?}");
        }
        // and some real shorthand typed elsewhere
        clip_set(w, "P ->> Q\nP \"φ\" at 5,0\n");
        w.key("cmd-v", "paste");
        let src = w.source();
        note!(w, "{src}");
        pasted && refused && s0 != s1 && src.contains("φ")
    });

    run.task("QA7", "⌘D repeats the last ⌥-drag offset and carries the arrows", 6, QA_PAIR, |w| {
        let (a, b) = (ObjRef::Node("A".into()), ObjRef::Node("B".into()));
        // fresh: one cell right, further when something is there
        let ca = w.centre(&a).unwrap();
        w.click(ca, "node A");
        let cb = w.centre(&b).unwrap();
        shift_click(w, cb, "node B");
        w.key("cmd-d", "duplicate");
        let sel1: Vec<String> = w.selection().iter().filter_map(|o| if let ObjRef::Node(n) = o { Some(n.clone()) } else { None }).collect();
        if sel1.len() != 2 {
            return false;
        }
        let d1 = (node_pos(w, &sel1[0]), node_pos(w, &sel1[1]));
        let arrows1 = arrows_n(w);
        note!(w, "first ⌘D: {d1:?}, {arrows1} arrows");
        w.shot("duplicated");
        // an ⌥-drag, then ⌘D repeats its offset
        let (_, ch) = cell_px(w);
        let ca = w.centre(&ObjRef::Node(sel1[0].clone())).unwrap();
        let to = point(ca.x, ca.y + px(2.0 * ch));
        w.drag(ca, &line_to(ca, to, 8), alt, "the copy two rows down, with ⌥", None);
        let sel2: Vec<String> = w.selection().iter().filter_map(|o| if let ObjRef::Node(n) = o { Some(n.clone()) } else { None }).collect();
        let at2 = sel2.first().and_then(|n| node_pos(w, n));
        w.key("cmd-d", "duplicate again");
        w.shot("repeated");
        let sel3: Vec<String> = w.selection().iter().filter_map(|o| if let ObjRef::Node(n) = o { Some(n.clone()) } else { None }).collect();
        let at3 = sel3.first().and_then(|n| node_pos(w, n));
        note!(w, "⌥-drag copy at {at2:?}, ⌘D at {at3:?}; {} arrows", arrows_n(w));
        let on_cell = |p: Option<Pos>| match p { Some(Pos::Cell(c, r)) => Some((c, r)), _ => None };
        let (Some((c2, r2)), Some((c3, r3))) = (on_cell(at2), on_cell(at3)) else { return false };
        // the first duplicate went right past B (two cells), carrying its arrow
        let first_ok = near(d1.0, 2.0, 0.0) && near(d1.1, 3.0, 0.0) && arrows1 == 2;
        first_ok && (c3 - c2).abs() < 0.02 && (r3 - r2 - 2.0).abs() < 0.02 && arrows_n(w) == 4
    });

    run.task("QA8", "distribute three or more evenly, from the panel", 4, "A at 0,0\nB at 1,0\nC at 5,0\n", |w| {
        w.focus_canvas();
        w.key("cmd-a", "select all");
        w.shot("selected");
        if !w.click_ui("button.distribute.h", "Distribute across") {
            return false;
        }
        w.shot("distributed");
        let (xa, xb, xc) = {
            let g = |w: &mut Win, n: &str| w.read(|v| geom::center_of(&v.doc.scene, &ObjRef::Node(n.into()))).map(|p| p.x);
            (g(w, "A"), g(w, "B"), g(w, "C"))
        };
        let (Some(xa), Some(xb), Some(xc)) = (xa, xb, xc) else { return false };
        note!(w, "centres {xa:.2} {xb:.2} {xc:.2} cm");
        // the gaps between their edges are equal (the nodes' widths differ a little)
        let g = w.read(|v| {
            let r = |n: &str| v.doc.scene.anchors.iter().find(|(o, _, _)| *o == ObjRef::Node(n.into())).map(|x| x.2.clone());
            (r("A"), r("B"), r("C"))
        });
        let (Some(ra), Some(rb), Some(rc)) = g else { return false };
        let (g1, g2) = (rb.min.x - ra.max.x, rc.min.x - rb.max.x);
        note!(w, "gaps {g1:.3} {g2:.3}");
        (g1 - g2).abs() < 0.03 && (xa - 0.0).abs() < 0.01 && (xc - 5.0 * 1.9).abs() < 0.01
    });

    run.task("QA8b", "distribute vertically, from the right-click menu", 4, "A at 0,0\nB at 0,1\nC at 0,5\n", |w| {
        w.focus_canvas();
        w.key("cmd-a", "select all");
        let a = w.centre(&ObjRef::Node("A".into())).unwrap();
        w.right(a, "node A");
        w.shot("menu");
        let keys = w.ui_keys();
        note!(w, "menu: {:?}", keys.iter().filter(|k| k.starts_with("menu.")).collect::<Vec<_>>());
        if !w.click_ui("menu.DistributeV", "Distribute vertically") {
            return false;
        }
        w.shot("distributed");
        let g = w.read(|v| {
            let r = |n: &str| v.doc.scene.anchors.iter().find(|(o, _, _)| *o == ObjRef::Node(n.into())).map(|x| x.2.clone());
            (r("A"), r("B"), r("C"))
        });
        let (Some(ra), Some(rb), Some(rc)) = g else { return false };
        let (g1, g2) = (ra.min.y - rb.max.y, rb.min.y - rc.max.y);
        note!(w, "gaps {g1:.3} {g2:.3}");
        (g1 - g2).abs() < 0.03
    });

    run.task("QA9", "a turned shape's selection outline turns with it", 2, "blob G size=4,2 angle=30 at (1,0)\nblob H size=3,1.5 at (7,0)\n", |w| {
        use ntex_studio::ui::figure::handles::turned_outline;
        let c = w.centre(&ObjRef::Blob("G".into())).unwrap();
        w.click(c, "the turned blob");
        w.shot("turned-selected");
        let t = w.read(|v| turned_outline(&v.doc.scene, &ObjRef::Blob("G".into())));
        let upright = w.read(|v| turned_outline(&v.doc.scene, &ObjRef::Blob("H".into())));
        note!(w, "turned {t:?}; the upright blob: {upright:?}");
        let Some(t) = t else {
            w.issue("a blob at 30° has an upright selection box");
            return false;
        };
        let corners = t.corners(0.0);
        let slanted = (corners[0].y - corners[1].y).abs() > 0.3;
        // and it hugs the blob: the corners sit about the blob's own half extents
        (t.angle - 30.0).abs() < 0.5 && slanted && upright.is_none() && t.half.x > 1.9 && t.half.y > 0.9
    });

    run.task("QA10", "Esc during a copy drag drops the copies; a copy dropped where it started is none", 3, QA_PAIR, |w| {
        let a = ObjRef::Node("A".into());
        let ca = w.centre(&a).unwrap();
        w.click(ca, "node A");
        let s0 = w.source();
        let u0 = undo_n(w);
        let (_, ch) = cell_px(w);
        w.hover(ca);
        w.event(PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: ca, modifiers: alt, click_count: 1, first_mouse: false, pen: None }));
        w.draw();
        for to in line_to(ca, point(ca.x, ca.y + px(ch)), 6) {
            w.event(PlatformInput::MouseMove(MouseMoveEvent { position: to, pressed_button: Some(MouseButton::Left), modifiers: alt, pen: None }));
            w.draw();
        }
        let during = w.read(|v| v.drag_figure.as_ref().map(|f| f.items.iter().filter(|it| matches!(it, Item::Node(_))).count()));
        let sel_during = w.selection();
        w.shot("mid-copy");
        w.key("escape", "cancel the drag");
        let sel_after = w.selection();
        w.event(PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: ca, modifiers: alt, click_count: 1, pen: None }));
        w.draw();
        let cancelled = w.source() == s0 && undo_n(w) == u0 && sel_after == vec![a.clone()];
        note!(w, "nodes in the draft {during:?}; selection during {sel_during:?}, after Esc {sel_after:?}");
        // out and back: no copy
        let out = point(ca.x, ca.y + px(ch));
        let mut path = line_to(ca, out, 5);
        path.extend(line_to(out, ca, 5));
        w.drag(ca, &path, alt, "A out and back, with ⌥", None);
        let back_none = w.source() == s0 && undo_n(w) == u0 && w.selection() == vec![a];
        during == Some(4) && cancelled && back_none
    });

    run.task("QA11", "a dragged blob takes its placed elements along; ⌥ copies them with it", 4, "blob G {e at (0,0), g at (1.5,0)} size=4,2 at (0.75,0)\n", |w| {
        let c = w.centre(&ObjRef::Blob("G".into())).unwrap();
        w.click(c, "the blob");
        let ppc = w.px_per_cm();
        let elems = |w: &mut Win, b: &str| -> Vec<Option<Pos>> {
            let b = b.to_string();
            w.read(move |v| v.doc.figure.items.iter().find_map(|it| match it { Item::Blob(x) if x.name == b => Some(x.elements.iter().map(|e| e.pos).collect()), _ => None }).unwrap_or_default())
        };
        let before = elems(w, "G");
        w.drag(c, &line_to(c, point(c.x + px(2.0 * ppc), c.y), 8), none, "the blob 2 cm right", Some("blob-moving"));
        let after = elems(w, "G");
        note!(w, "elements {before:?} → {after:?}");
        let shifted = match (&before[..], &after[..]) {
            ([Some(Pos::Abs(a0, _)), Some(Pos::Abs(b0, _))], [Some(Pos::Abs(a1, _)), Some(Pos::Abs(b1, _))]) => (a1 - a0 - 2.0).abs() < 0.11 && (b1 - b0 - 2.0).abs() < 0.11,
            _ => false,
        };
        // ⌥-drag: a copy of the blob with its own elements
        let c2 = w.centre(&ObjRef::Blob("G".into())).unwrap();
        let sel0 = w.selection();
        w.drag(c2, &line_to(c2, point(c2.x, c2.y + px(2.5 * ppc)), 8), alt, "the blob 2.5 cm down, with ⌥", Some("blob-copy"));
        let blobs: Vec<String> = w.read(|v| v.doc.figure.items.iter().filter_map(|it| if let Item::Blob(b) = it { Some(b.name.clone()) } else { None }).collect());
        let sel = w.selection();
        note!(w, "blobs {blobs:?}; selection {sel:?} (was {sel0:?})");
        let Some(ObjRef::Blob(copy)) = sel.first().cloned() else { return false };
        let ce = elems(w, &copy);
        let orig = elems(w, "G");
        note!(w, "copy {copy} elements {ce:?}; original {orig:?}");
        let moved_down = match (&orig[..], &ce[..]) {
            ([Some(Pos::Abs(_, y0)), _], [Some(Pos::Abs(_, y1)), _]) => (y0 - y1 - 2.5).abs() < 0.11,
            _ => false,
        };
        shifted && blobs.len() == 2 && copy != "G" && orig == after && moved_down
    });

    run.task("QA12", "dragging at scale 0.7 follows the pointer (model cm = drawn cm ÷ scale)", 2, "scale 0.7\ntext \"t\" at (2,0)\n", |w| {
        let t = ObjRef::Text(w.read(|v| v.doc.figure.items.iter().position(|it| matches!(it, Item::Text(_)))).unwrap());
        let c = w.centre(&t).unwrap();
        let ppc = w.px_per_cm();
        w.drag(c, &line_to(c, point(c.x + px(1.4 * ppc), c.y), 8), none, "the text 1.4 cm (drawn) right", None);
        let pos = w.read(|v| v.doc.figure.items.iter().find_map(|it| if let Item::Text(t) = it { Some(t.pos) } else { None }));
        note!(w, "text now at {pos:?}");
        matches!(pos, Some(Pos::Abs(x, _)) if (x - 4.0).abs() < 0.11)
    });

    run.task("QA13", "the footnotes name ⌥ ⇧ ⌘ for a drag", 2, QA_PAIR, |w| {
        let ca = w.centre(&ObjRef::Node("A".into())).unwrap();
        w.click(ca, "node A");
        let h = w.window.update(&mut w.cx, |v, window, cx| v.hints(window, cx).iter().map(|s| s.to_string()).collect::<Vec<_>>().join(" | ")).unwrap();
        note!(w, "hints: {h}");
        w.shot("footnote");
        h.contains('⌥') && h.contains('⇧') && h.contains('⌘') && h.contains("⌘C")
    });

    // ============================================ figure-qol B: how big the figure is on the page
    fn cmd_key(w: &mut Win, on: bool) {
        let modifiers = if on { Modifiers::command() } else { Modifiers::default() };
        w.event(PlatformInput::ModifiersChanged(ModifiersChangedEvent { modifiers, capslock: Default::default() }));
        w.draw();
    }
    fn set_dpi(w: &mut Win, px_per_cm: f32) {
        let _ = w.window.update(&mut w.cx, |v, _, _| v.px_per_cm_override = Some(px_per_cm));
        w.draw();
    }
    fn readout(w: &mut Win) -> Option<String> {
        w.read(|v| v.page_readout())
    }
    fn page_cm(w: &mut Win) -> (f32, f32) {
        w.read(|v| ntex_figures::pagesize::page_size(&v.doc.scene))
    }
    fn scale_of(w: &mut Win) -> f32 {
        w.read(|v| v.doc.figure.globals.scale)
    }
    fn state_text(w: &mut Win) -> String {
        w.read(|v| v.canvas_state_text())
    }
    const QB_SRC: &str = "A at 0,0\nB at 3,0\nA -f-> B\n";

    run.task("QB1", "see how big the figure is on the page, live, and find the Figure section", 3, QB_SRC, |w| {
        let r0 = readout(w).unwrap_or_default();
        let (pw, ph) = page_cm(w);
        let want = format!("{pw:.1} × {ph:.1} cm on the page");
        note!(w, "header reads {r0:?}");
        if r0 != want {
            issue!(w, "the header says {r0:?}; the page is {want:?}");
        }
        w.shot("header-readout");
        // nothing selected: the Figure section is open, with its fields
        for k in ["panel.figure.toggle", "field.fig.width", "field.fig.scale", "field.fig.spacing.x", "field.fig.spacing.y", "slider.fig.scale", "panel.size.small"] {
            if w.ui(k).is_none() {
                issue!(w, "nothing selected, but `{k}` isn't drawn");
            }
        }
        // select a node: the Figure section folds under its section
        let a = w.centre(&ObjRef::Node("A".into())).unwrap();
        w.click(a, "node A");
        if w.ui("field.fig.width").is_some() {
            w.issue("the Figure section stays open under a selection");
        }
        if w.ui("panel.figure.toggle").is_none() {
            w.issue("no Figure section head beneath the node's properties");
        }
        w.shot("folded-under-node");
        // opening it from the head
        w.click_ui("panel.figure.toggle", "the Figure head");
        let open = w.ui("field.fig.width").is_some();
        if !open {
            w.issue("the Figure head doesn't open the section");
        }
        w.shot("opened-under-node");
        // the readout is live: dragging B further away widens it, before the mouse is up
        let b = w.centre(&ObjRef::Node("B".into())).unwrap();
        let to = point(b.x + px(120.), b.y);
        w.hover(b);
        w.event(PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: b, modifiers: Modifiers::default(), click_count: 1, first_mouse: false, pen: None }));
        w.draw();
        for q in line_to(b, to, 10) {
            w.event(PlatformInput::MouseMove(MouseMoveEvent { position: q, pressed_button: Some(MouseButton::Left), modifiers: Modifiers::default(), pen: None }));
            w.draw();
        }
        let mid = readout(w).unwrap_or_default();
        w.shot("live-while-dragging");
        w.event(PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: to, modifiers: Modifiers::default(), click_count: 1, pen: None }));
        w.draw();
        w.step("drag B out");
        if mid == r0 {
            issue!(w, "the readout didn't change while B was dragged away ({mid:?})");
        }
        let after = readout(w).unwrap_or_default();
        if after != mid {
            issue!(w, "mid-drag the header said {mid:?}, after the drop {after:?}");
        }
        r0 == want && open && mid != r0
    });

    run.task("QB2", "make the figure 8 cm wide on the page by typing it", 3, QB_SRC, |w| {
        let w0 = page_cm(w).0;
        let ok = w.click_ui("field.fig.width", "the Width field");
        if !ok {
            return false;
        }
        w.key_raw("cmd-a");
        w.typ("8");
        w.key("enter", "commit");
        let (pw, _) = page_cm(w);
        let s = scale_of(w);
        note!(w, "was {w0:.2} cm; now {pw:.2} cm at scale {s}");
        if (pw - 8.0).abs() > 0.05 {
            issue!(w, "typing 8 gives a page {pw:.2} cm wide");
        }
        w.shot("width-typed");
        // a longer one is still a single step: 12.5
        let u0 = w.read(|v| v.undo.len());
        w.click_ui("field.fig.width", "the Width field");
        w.key_raw("cmd-a");
        w.typ("12.5");
        w.key("enter", "commit");
        let (pw2, _) = page_cm(w);
        let steps = w.read(|v| v.undo.len()) - u0;
        if (pw2 - 12.5).abs() > 0.05 {
            issue!(w, "typing 12.5 gives {pw2:.2} cm");
        }
        if steps != 1 {
            issue!(w, "typing 12.5 made {steps} undo steps");
        }
        // the header and the Scale field follow
        let r = readout(w).unwrap_or_default();
        let sf = w.window.update(&mut w.cx, |v, _, cx| v.fields.get("fig.scale").map(|f| f.entity.read(cx).text().to_string())).unwrap();
        note!(w, "header {r:?}; Scale field {sf:?}");
        if !r.starts_with("12.5") {
            issue!(w, "the header says {r:?}");
        }
        w.shot("width-12-5");
        // a width the labels alone exceed changes nothing
        let s_before = scale_of(w);
        w.click_ui("field.fig.width", "the Width field");
        w.key_raw("cmd-a");
        w.typ("0.6");
        w.key("enter", "commit");
        if (scale_of(w) - s_before).abs() > 1e-6 {
            issue!(w, "0.6 cm (narrower than the labels) changed the scale to {}", scale_of(w));
        }
        (pw - 8.0).abs() <= 0.05 && (pw2 - 12.5).abs() <= 0.05 && steps == 1
    });

    run.task("QB3", "scale it with the slider and the Scale field", 3, QB_SRC, |w| {
        let s0 = scale_of(w);
        let Some(b) = w.ui("slider.fig.scale") else {
            w.issue("no Scale slider in the Figure section");
            return false;
        };
        let u0 = w.read(|v| v.undo.len());
        let from = point(b.origin.x + b.size.width * 0.2, b.center().y);
        let to = point(b.origin.x + b.size.width * 0.7, b.center().y);
        w.drag(from, &line_to(from, to, 20), none, "the Scale slider", Some("slider"));
        let (s1, steps) = (scale_of(w), w.read(|v| v.undo.len()) - u0);
        note!(w, "scale {s0} → {s1} in {steps} step(s)");
        if (s1 - s0).abs() < 0.05 {
            issue!(w, "the slider didn't change the scale ({s0} → {s1})");
        }
        if steps != 1 {
            issue!(w, "one slider drag made {steps} undo steps");
        }
        w.shot("after-slider");
        w.click_ui("field.fig.scale", "the Scale field");
        w.key_raw("cmd-a");
        w.typ("1.5");
        w.key("enter", "commit");
        let s2 = scale_of(w);
        if (s2 - 1.5).abs() > 1e-4 {
            issue!(w, "typing 1.5 gives scale {s2}");
        }
        // the width field follows
        let wf = w.window.update(&mut w.cx, |v, _, cx| v.fields.get("fig.width").map(|f| f.entity.read(cx).text().to_string())).unwrap();
        let (pw, _) = page_cm(w);
        note!(w, "Width field {wf:?}, page {pw:.2}");
        if wf.as_deref() != Some(format!("{pw:.1}").as_str()) {
            issue!(w, "the Width field says {wf:?} but the page is {pw:.1}");
        }
        (s1 - s0).abs() >= 0.05 && steps == 1 && (s2 - 1.5).abs() < 1e-4
    });

    run.task("QB4", "⌘1 shows it at its real size on this display; ⌘0 fits again", 2, QB_SRC, |w| {
        // a known display: 40 logical px to the cm
        set_dpi(w, 40.0);
        let fit = w.px_per_cm();
        w.key("cmd-1", "actual size");
        let at = w.px_per_cm();
        let st = state_text(w);
        note!(w, "fit {fit:.1} px/cm; ⌘1 {at:.2} px/cm; corner says {st:?}");
        if (at - 40.0).abs() > 0.2 {
            issue!(w, "⌘1 shows {at:.2} px per cm, not 40");
        }
        if !st.contains("actual size") {
            issue!(w, "the canvas state says {st:?}");
        }
        // 1 cm of the figure measures 40 px on screen (A to B is 3 cells)
        let (a, b) = (w.centre(&ObjRef::Node("A".into())).unwrap(), w.centre(&ObjRef::Node("B".into())).unwrap());
        let cell = w.read(|v| v.doc.scene.cell.0);
        let measured = f32::from(b.x - a.x) / (3.0 * cell);
        if (measured - 40.0).abs() > 0.3 {
            issue!(w, "A to B is {:.1} px; at 40 px/cm it should be {:.1}", f32::from(b.x - a.x), 3.0 * cell * 40.0);
        }
        w.shot("actual-size");
        // the panel folding away (⌘I) doesn't change the size it is shown at
        w.key("cmd-i", "hide the panel");
        let still = w.px_per_cm();
        if (still - 40.0).abs() > 0.3 {
            issue!(w, "hiding the panel changed the size to {still:.2} px/cm");
        }
        w.key("cmd-i", "show the panel");
        // the figure scale changes what is on screen but not the cm: a wider page, still 40 px per cm
        w.key("cmd-0", "fit");
        let back = w.px_per_cm();
        let st2 = state_text(w);
        note!(w, "⌘0: {back:.1} px/cm, {st2:?}");
        if (back - 40.0).abs() < 1.0 && (fit - 40.0).abs() > 1.0 || st2.contains("actual size") {
            w.issue("⌘0 doesn't fit again");
        }
        // 2×: her trackpad zoom steps off actual size; the readout stops saying so
        w.key("cmd-1", "actual size");
        w.key("cmd-=", "zoom in");
        let st3 = state_text(w);
        if st3.contains("actual size") {
            w.issue("zoomed in, the corner still says actual size");
        }
        (at - 40.0).abs() <= 0.2 && st.contains("actual size") && !st2.contains("actual size") && !st3.contains("actual size")
    });

    run.task("QB5", "hold ⌘: a page frame appears round the figure, not during a drag", 2, QB_SRC, |w| {
        let none_at_rest = w.read(|v| v.page_frame().is_none());
        if !none_at_rest {
            w.issue("a page frame without ⌘");
        }
        cmd_key(w, true);
        let frame = w.read(|v| v.page_frame());
        let Some(f) = frame else {
            w.issue("⌘ held, but no page frame");
            return false;
        };
        let (pw, ph) = page_cm(w);
        let (fw, fh) = (f.max.x - f.min.x, f.max.y - f.min.y);
        note!(w, "frame {fw:.2} × {fh:.2} cm; the page {pw:.2} × {ph:.2}");
        if (fw - pw).abs() > 0.01 || (fh - ph).abs() > 0.01 {
            issue!(w, "the frame is {fw:.2} × {fh:.2}, the page {pw:.2} × {ph:.2}");
        }
        w.shot("page-frame");
        cmd_key(w, false);
        if w.read(|v| v.page_frame().is_some()) {
            w.issue("the frame stays after ⌘ is released");
        }
        // a drag in progress: no frame
        let a = w.centre(&ObjRef::Node("A".into())).unwrap();
        w.hover(a);
        w.event(PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: a, modifiers: Modifiers::default(), click_count: 1, first_mouse: false, pen: None }));
        w.draw();
        let to = point(a.x + px(40.), a.y + px(30.));
        for q in line_to(a, to, 6) {
            w.event(PlatformInput::MouseMove(MouseMoveEvent { position: q, pressed_button: Some(MouseButton::Left), modifiers: Modifiers::default(), pen: None }));
            w.draw();
        }
        cmd_key(w, true);
        let during = w.read(|v| v.page_frame().is_some());
        w.event(PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: to, modifiers: Modifiers::default(), click_count: 1, pen: None }));
        w.draw();
        cmd_key(w, false);
        if during {
            w.issue("the page frame is drawn while an object is being dragged");
        }
        none_at_rest && !during
    });

    run.task("QB6", "⌘-drag a corner of the page frame to scale the whole figure", 2, QB_SRC, |w| {
        let (w0, h0) = page_cm(w);
        let s0 = scale_of(w);
        let u0 = w.read(|v| v.undo.len());
        cmd_key(w, true);
        let r = w.read(|v| v.page_frame()).unwrap();
        // the bottom-right corner (y is up: min y); the top-left stays
        let corner = w.xf_px(Pt { x: r.max.x, y: r.min.y });
        let anchor0 = w.xf_px(Pt { x: r.min.x, y: r.max.y });
        // a press on a corner without moving changes nothing
        w.mouse(corner, MouseButton::Left, 1, Modifiers::command());
        if scale_of(w) != s0 || w.read(|v| v.undo.len()) != u0 {
            w.issue("a click on the corner changed the figure");
        }
        let cmd = Modifiers::command();
        w.hover(corner);
        w.event(PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: corner, modifiers: cmd, click_count: 1, first_mouse: false, pen: None }));
        w.draw();
        let to = point(corner.x + px(120.), corner.y + px(60.));
        let path = line_to(corner, to, 12);
        for q in &path {
            w.event(PlatformInput::MouseMove(MouseMoveEvent { position: *q, pressed_button: Some(MouseButton::Left), modifiers: cmd, pen: None }));
            w.draw();
        }
        let live = w.read(|v| v.readout.clone().map(|(_, t)| t)).unwrap_or_default();
        let hdr = readout(w).unwrap_or_default();
        note!(w, "live readout {live:?}; header {hdr:?}");
        if !live.contains("cm") || !live.contains("scale") {
            issue!(w, "no live width readout while dragging the corner: {live:?}");
        }
        if scale_of(w) != s0 {
            w.issue("the scale was committed before the mouse came up");
        }
        w.shot("dragging-corner");
        w.event(PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: to, modifiers: cmd, click_count: 1, pen: None }));
        w.draw();
        w.step("⌘-drag the page corner");
        cmd_key(w, false);
        let (w1, h1) = page_cm(w);
        let s1 = scale_of(w);
        let steps = w.read(|v| v.undo.len()) - u0;
        note!(w, "page {w0:.2}×{h0:.2} → {w1:.2}×{h1:.2}; scale {s0} → {s1}; {steps} step(s)");
        if s1 <= s0 + 0.05 {
            issue!(w, "dragging out didn't enlarge the figure ({s0} → {s1})");
        }
        if steps != 1 {
            issue!(w, "the corner drag made {steps} undo steps");
        }
        // the readout agreed with what was set
        if !hdr.starts_with(&format!("{w1:.1}")) {
            issue!(w, "the header said {hdr:?} mid-drag but the page ended {w1:.2} cm wide");
        }
        // the opposite corner stayed where it was on screen
        let r1 = w.read(|v| ntex_studio::ui::figure::geom::page_rect(&v.doc.scene));
        let anchor1 = w.xf_px(Pt { x: r1.min.x, y: r1.max.y });
        let moved = f32::from(anchor1.x - anchor0.x).hypot(f32::from(anchor1.y - anchor0.y));
        if moved > 1.5 {
            issue!(w, "the top-left corner moved {moved:.1} px on screen");
        }
        w.shot("after-corner-drag");
        // Esc mid-drag puts it all back
        cmd_key(w, true);
        let r = w.read(|v| v.page_frame()).unwrap();
        let corner = w.xf_px(Pt { x: r.max.x, y: r.min.y });
        let (sb, tb) = (scale_of(w), w.read(|v| v.view.target.clone()));
        w.hover(corner);
        w.event(PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: corner, modifiers: cmd, click_count: 1, first_mouse: false, pen: None }));
        w.draw();
        for q in line_to(corner, point(corner.x - px(60.), corner.y - px(30.)), 6) {
            w.event(PlatformInput::MouseMove(MouseMoveEvent { position: q, pressed_button: Some(MouseButton::Left), modifiers: cmd, pen: None }));
            w.draw();
        }
        w.focus_canvas();
        w.key_raw("escape");
        w.event(PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: corner, modifiers: cmd, click_count: 1, pen: None }));
        w.draw();
        cmd_key(w, false);
        let (sa, ta) = (scale_of(w), w.read(|v| v.view.target.clone()));
        if sa != sb || ta != tb {
            issue!(w, "Esc mid-drag left scale {sb} → {sa} / view moved");
        }
        s1 > s0 + 0.05 && steps == 1 && moved <= 1.5 && sa == sb && ta == tb
    });

    // ===================================================== figure-qol E: what the shorthand can say, the mouse can too
    fn label_at(w: &mut Win, o: &ObjRef) -> Option<Pt> {
        let o = o.clone();
        w.read(move |v| v.doc.scene.draws.iter().find_map(|d| match d { Draw::Label { obj, at, .. } if *obj == o => Some(*at), _ => None }))
    }
    const QE_SRC: &str = "A at 0,0\nB at 3,0\nC at 3,-2\nA -f-> B\nA -g-> C\n";

    run.task("QE1", "put an arrow's label near its end (panel)", 3, QE_SRC, |w| {
        let i = w.arrow_index("A", "B").unwrap();
        let o = ObjRef::Arrow(i);
        let before = label_at(w, &o).unwrap();
        let p = w.on_arrow(i, 0.5, 0.0).unwrap();
        w.click(p, "the A → B arrow");
        w.shot("selected");
        if !w.click_ui("panel.labelpos.end", "Label at: end") && !w.click_ui("panel.labelpos.2", "Label at: end") {
            return false;
        }
        w.shot("near-end");
        let after = label_at(w, &o).unwrap();
        let a = arrow_at(w, i).unwrap();
        note!(w, "label x {:.2} → {:.2}; line `{}`", before.x, after.x, w.line("A -f"));
        let mid = a.label_pos == LabelPos::NearEnd && after.x > before.x + 0.3 && w.line("A -f").contains("near-end");
        // and near the start
        w.click_ui("panel.labelpos.start", "Label at: start");
        let s = label_at(w, &o).unwrap();
        mid && s.x < before.x - 0.3 && w.line("A -f").contains("near-start")
    });

    run.task("QE2", "add a loop arrow on a node (panel button and menu)", 3, "A at 0,0\nB at 3,0\nA -f-> B\n", |w| {
        let c = w.centre(&ObjRef::Node("A".into())).unwrap();
        w.click(c, "node A");
        w.shot("selected");
        if !w.click_ui("button.node.A.loop", "Add loop arrow") {
            return false;
        }
        w.shot("looped");
        let n = w.read(|v| v.doc.figure.items.iter().filter(|it| matches!(it, Item::Arrow(a) if a.from == "A" && a.to == "A" && a.is_loop)).count());
        // leave the label field (Esc), then the menu route on B
        w.key_raw("escape");
        w.focus_canvas();
        let c = w.centre(&ObjRef::Node("B".into())).unwrap();
        w.right(c, "node B");
        w.shot("menu");
        w.click_ui("menu.Loop", "Add loop arrow (menu)");
        w.key_raw("escape");
        let m = w.read(|v| v.doc.figure.items.iter().filter(|it| matches!(it, Item::Arrow(a) if a.from == "B" && a.to == "B" && a.is_loop)).count());
        note!(w, "source:\n{}", w.source());
        n == 1 && m == 1
    });

    run.task("QE3", "the ≅ and which side it sits on", 4, "A at 0,0\nB at 3,0\nA -f-> B\n", |w| {
        let i = w.arrow_index("A", "B").unwrap();
        let p = w.on_arrow(i, 0.5, 0.0).unwrap();
        w.click(p, "the arrow");
        let iso = w.find_ui("check.arrow.", "iso").unwrap_or_default();
        if !w.click_ui(&iso, "≅") {
            return false;
        }
        let cong = |w: &mut Win| w.read(|v| v.doc.scene.draws.iter().find_map(|d| match d { Draw::Label { obj: ObjRef::Arrow(_), at, latex, .. } if latex == "\\cong" => Some(at.y), _ => None }));
        let f = |w: &mut Win| w.read(|v| v.doc.scene.draws.iter().find_map(|d| match d { Draw::Label { obj: ObjRef::Arrow(_), at, latex, .. } if latex == "f" => Some(at.y), _ => None }));
        let (c0, f0) = (cong(w).unwrap(), f(w).unwrap());
        w.shot("iso");
        let swap = w.find_ui("check.arrow.", "swap").unwrap_or_default();
        w.click_ui(&swap, "Swap side");
        let (c1, f1) = (cong(w).unwrap(), f(w).unwrap());
        w.shot("swapped");
        note!(w, "≅ y {c0:.2} → {c1:.2}; f y {f0:.2} → {f1:.2}; `{}`", w.line("A -f"));
        // ≅ is across the line from f, and swapping flips both
        (c0 - 0.0) * (f0 - 0.0) < 0.0 && (c1 - 0.0) * (f1 - 0.0) < 0.0 && (c0 > 0.0) != (c1 > 0.0) && w.line("A -f").contains("swap") && w.line("A -f").contains("iso")
    });

    run.task("QE4", "colour and size of a text (panel)", 4, "A at 0,0\ntext \"kernel\" at (0,-2)\n", |w| {
        let c = w.centre(&ObjRef::Text(1)).unwrap();
        w.click(c, "the text");
        w.shot("selected");
        if !w.click_ui("colour.teal", "teal") {
            return false;
        }
        if !w.click_ui("panel.textsize.large", "size large") {
            return false;
        }
        w.shot("teal-large");
        let l = w.line("text");
        note!(w, "line `{l}`");
        let (col, sz) = w.read(|v| v.doc.scene.draws.iter().find_map(|d| match d { Draw::Label { obj: ObjRef::Text(_), color, size, .. } => Some((color.clone(), *size)), _ => None }).unwrap());
        l.contains("color=teal") && l.contains("size=large") && col == "teal" && sz > 10.5
    });

    run.task("QE5", "set the head of two arrows at once (mixed)", 5, "A at 0,0\nB at 3,0\nC at 0,-2\nA -f-> B\nA ->> C\n", |w| {
        let (i, j) = (w.arrow_index("A", "B").unwrap(), w.arrow_index("A", "C").unwrap());
        let p = w.on_arrow(i, 0.5, 0.0).unwrap();
        w.click(p, "the A → B arrow");
        let q = w.on_arrow(j, 0.5, 0.0).unwrap();
        shift_click(w, q, "the A → C arrow");
        w.shot("two-selected");
        if w.selection().len() != 2 {
            issue!(w, "selection {:?}", w.selection());
            return false;
        }
        // the panel says the heads differ: neither is picked
        let keys = w.ui_keys();
        note!(w, "panel controls: {}", keys.iter().filter(|k| k.starts_with("many.")).cloned().collect::<Vec<_>>().join(" "));
        if w.ui("many.head.To").is_none() {
            w.issue("no shared Head row for two arrows");
            return false;
        }
        w.click_ui("many.head.Hook", "↪ for both");
        w.click_ui("many.line.dashed", "dashed for both");
        w.shot("both-hooked");
        let (a, b) = (arrow_at(w, i).unwrap(), arrow_at(w, j).unwrap());
        w.focus_canvas();
        // one undo step per click
        a.head == Head::Hook && b.head == Head::Hook && a.line == Line::Dashed && b.line == Line::Dashed
    });

    run.task("QE6", "auto-place a node from the panel", 3, "A at 0,0\nB at 3,0\nC at 6,-2\nA -f-> B\nB -g-> C\n", |w| {
        let c = w.centre(&ObjRef::Node("C".into())).unwrap();
        w.click(c, "node C");
        w.shot("selected");
        if !w.click_ui("button.node.C.autoplace", "Auto-place") {
            return false;
        }
        w.shot("auto-placed");
        let l = w.line("C");
        note!(w, "line `{l}`");
        let placed = w.read(|v| v.doc.scene.anchors.iter().any(|(o, _, _)| *o == ObjRef::Node("C".into())));
        !l.contains(" at ") && placed
    });

    run.task("QE7", "auto-place shapes: menu, and several at once", 4, "blob G \"G\" {a, b} at (0,0)\nblob H \"H\" {c} at (4,0)\nngon P 4 at 8,0\n", |w| {
        let g = w.label(&ObjRef::Blob("G".into())).unwrap();
        w.right(g, "blob G");
        w.shot("menu");
        w.click_ui("menu.AutoPlace", "Auto-place (menu)");
        let l = w.line("blob G");
        let one = !l.contains(" at ");
        // now H and P together
        w.focus_canvas();
        let h = w.label(&ObjRef::Blob("H".into())).unwrap();
        w.click(h, "blob H");
        let p = w.centre(&ObjRef::NGon("P".into())).unwrap();
        shift_click(w, p, "the polygon");
        w.shot("two");
        w.click_ui("button.many.autoplace", "Auto-place (both)");
        note!(w, "source:\n{}", w.source());
        one && !w.line("blob H").contains(" at ") && !w.line("ngon").contains(" at ")
    });

    run.task("QE8", "type where a blob and a text sit; switch cells ↔ cm", 5, "blob G \"G\" {a, b} at (0,0)\ntext \"hi\" at (2,-2)\n", |w| {
        let g = w.label(&ObjRef::Blob("G".into())).unwrap();
        w.click(g, "blob G");
        w.shot("blob-panel");
        if !w.click_ui("field.blob.G.col", "At x") {
            return false;
        }
        w.key_raw("cmd-a");
        w.typ("1.5");
        w.key("enter", "commit");
        let l = w.line("blob G");
        note!(w, "line `{l}`");
        let ok1 = l.contains("(1.5,0)");
        // text: switch the units
        let t = w.centre(&ObjRef::Text(1)).unwrap();
        w.click(t, "the text");
        w.click_ui("button.txtpos.1.unit", "cm → cells");
        let l2 = w.line("text");
        note!(w, "text line `{l2}`");
        w.shot("text-cells");
        ok1 && l2.contains("at ") && !l2.contains("at (")
    });

    run.task("QE9", "put a blob inside another (panel)", 3, "blob G \"G\" {a, b}\nblob K \"K\" {c}\n", |w| {
        let k = w.label(&ObjRef::Blob("K".into())).unwrap();
        w.click(k, "blob K");
        w.shot("selected");
        if !w.click_ui("panel.parent.G", "Inside: G") {
            return false;
        }
        w.shot("inside");
        let b = blob_of(w, "K").unwrap();
        note!(w, "line `{}`", w.line("blob K"));
        b.parent.as_deref() == Some("G") && w.line("blob K").contains(" in G")
    });

    run.task("QE10", "shared fields of two n-gons and two blobs", 6, "ngon P 4 at 0,0 plain\nngon Q 5 at 4,0\nblob G \"G\" {a} at (0,-4)\nblob H \"H\" {b} at (4,-4) shaded\n", |w| {
        let p = w.centre(&ObjRef::NGon("P".into())).unwrap();
        w.click(p, "square P");
        let q = w.centre(&ObjRef::NGon("Q".into())).unwrap();
        shift_click(w, q, "pentagon Q");
        w.shot("two-ngons");
        let fill = w.find_ui("check.many.ngon", "fill").unwrap_or_default();
        if !w.click_ui(&fill, "Fill (mixed: sets both)") {
            return false;
        }
        let both = [ngon_of(w, "P").unwrap().shaded, ngon_of(w, "Q").unwrap().shaded];
        w.shot("both-filled");
        // two blobs
        w.focus_canvas();
        let g = w.label(&ObjRef::Blob("G".into())).unwrap();
        w.click(g, "blob G");
        let h = w.label(&ObjRef::Blob("H".into())).unwrap();
        shift_click(w, h, "blob H");
        let sh = w.find_ui("check.many.blob", "shaded").unwrap_or_default();
        w.click_ui(&sh, "Shaded (mixed)");
        w.click_ui("many.shape.wide", "wide for both");
        w.shot("blobs");
        let (bg, bh) = (blob_of(w, "G").unwrap(), blob_of(w, "H").unwrap());
        note!(w, "source:\n{}", w.source());
        both == [true, true] && bg.shaded && bh.shaded && bg.shape == BlobShape::Wide && bh.shape == BlobShape::Wide
    });

    run.task("QE11", "loop arrow with the arrow tool: drag a node onto itself", 2, "A at 0,0\nB at 3,0\nA -f-> B\n", |w| {
        w.key("a", "arrow tool");
        let c = w.centre(&ObjRef::Node("A".into())).unwrap();
        let out = point(c.x + px(40.), c.y + px(10.));
        w.drag(c, &[out, c], none, "A out and back onto A", Some("looping"));
        w.key_raw("escape");
        w.shot("looped");
        note!(w, "source:\n{}", w.source());
        w.read(|v| v.doc.figure.items.iter().any(|it| matches!(it, Item::Arrow(a) if a.from == "A" && a.to == "A" && a.is_loop)))
    });

    // ============================================ independent review of figure-qol (R*)
    // Each R task fails on figure-qol as reviewed and should pass once the
    // finding it names is fixed (see the review report).
    fn drawn_cm(w: &mut Win, names: &[&str]) -> Vec<(String, f32, f32)> {
        w.read(|v| {
            names
                .iter()
                .map(|n| {
                    let p = geom::center_of(&v.doc.scene, &ObjRef::Node(n.to_string())).unwrap_or(Pt { x: f32::NAN, y: f32::NAN });
                    (n.to_string(), (p.x * 100.0).round() / 100.0, (p.y * 100.0).round() / 100.0)
                })
                .collect()
        })
    }

    // F1: copying auto-placed nodes re-flows the originals. The move path pins
    // the others where they are drawn; the copy paths (⌥-drag, ⌘D, ⌘V) don't,
    // so the new pinned copies push every auto-placed original along.
    run.task("R1", "⌥-drag a copy of A in a figure with no positions: A, B, C stay where they are", 2, "A -> B\nB -> C\n", |w| {
        let before = drawn_cm(w, &["A", "B", "C"]);
        let a = w.centre(&ObjRef::Node("A".into())).unwrap();
        w.click(a, "node A");
        let (_, ch) = cell_px(w);
        let to = point(a.x, a.y + px(ch));
        w.drag(a, &line_to(a, to, 8), alt, "a copy of A one cell down", Some("copying"));
        w.shot("copied");
        let after = drawn_cm(w, &["A", "B", "C"]);
        let n = nodes_n(w);
        note!(w, "nodes {n}; drawn before {before:?}; after {after:?}\n{}", w.source());
        if before != after {
            issue!(w, "the originals moved when a copy was dropped: {before:?} → {after:?}");
        }
        n == 4 && before == after
    });

    run.task("R2", "⌘D on A in a figure with no positions: A, B, C stay where they are", 2, "A -> B\nB -> C\n", |w| {
        let before = drawn_cm(w, &["A", "B", "C"]);
        let a = w.centre(&ObjRef::Node("A".into())).unwrap();
        w.click(a, "node A");
        w.key("cmd-d", "duplicate");
        w.shot("duplicated");
        let after = drawn_cm(w, &["A", "B", "C"]);
        let n = nodes_n(w);
        note!(w, "nodes {n}; drawn before {before:?}; after {after:?}\n{}", w.source());
        if before != after {
            issue!(w, "⌘D moved the originals: {before:?} → {after:?}");
        }
        n == 4 && before == after
    });

    // F3: B's ⌘-press on a page-frame corner is taken before anything else, in
    // every tool, within 12 px. When zoomed out, a node at a corner of the figure
    // lies inside that grip, so A's ⌘-drag (move without snapping) scales the
    // whole figure instead of moving the node.
    run.task("R3", "⌘-drag the corner node A a little, free of the grid: A moves, the figure's scale doesn't", 1, "A at 0,0\nB at 20,0\nC at 0,8\nD at 20,8\nA -> B\nC -> D\n", |w| {
        let s0 = w.read(|v| v.doc.figure.globals.scale);
        let mut tries = 0;
        let (mut a, mut d) = (point(px(0.), px(0.)), f32::MAX);
        while tries < 6 {
            a = w.centre(&ObjRef::Node("A".into())).unwrap();
            let r = w.read(|v| geom::page_rect(&v.doc.scene));
            let corner = w.xf_px(Pt { x: r.min.x, y: r.max.y });
            d = f32::from(a.x - corner.x).hypot(f32::from(a.y - corner.y));
            if d < 10.0 {
                break;
            }
            w.key_raw("cmd--");
            tries += 1;
        }
        note!(w, "node A's centre is {d:.1} px from the page frame's corner at {:.1} px/cm", w.px_per_cm());
        w.shot("zoomed-out");
        let to = point(a.x + px(30.), a.y + px(18.));
        w.drag(a, &line_to(a, to, 8), cmd, "A with ⌘ (no snapping)", Some("dragging"));
        let s1 = w.read(|v| v.doc.figure.globals.scale);
        let pa = node_pos(w, "A");
        note!(w, "scale {s0} → {s1}; A at {pa:?}");
        if s1 != s0 {
            issue!(w, "⌘-dragging node A scaled the whole figure ({s0} → {s1})");
        }
        s1 == s0 && !matches!(pa, Some(Pos::Cell(x, y)) if x == 0.0 && y == 0.0)
    });

    // ---------------------------------------------------------------- review fixes (worker 2)
    fn rect_px(w: &mut Win, r: &Rect) -> Bounds<Pixels> {
        let (a, b) = (w.xf_px(r.min), w.xf_px(r.max));
        Bounds::from_corners(point(a.x.min(b.x), a.y.min(b.y)), point(a.x.max(b.x), a.y.max(b.y)))
    }
    fn panel_field(w: &mut Win, key: &str) -> Option<String> {
        w.window.update(&mut w.cx, |v, _, cx| v.fields.get(key).map(|f| f.entity.read(cx).text().to_string())).unwrap()
    }

    // U4: a long panel scrolls, the folded Figure head stays reachable, controls under the cut are found by scrolling
    run.task("QG1", "a long properties panel scrolls; the Figure head stays reachable", 4, "A at 0,0\nB at 3,0\nC at 0,-2\nA -f-> B\nA ->> C\n", |w| {
        let (i, j) = (w.arrow_index("A", "B").unwrap(), w.arrow_index("A", "C").unwrap());
        let p = w.on_arrow(i, 0.5, 0.0).unwrap();
        w.click(p, "the A → B arrow");
        let q = w.on_arrow(j, 0.5, 0.0).unwrap();
        shift_click(w, q, "the A → C arrow");
        w.shot("panel-cut-with-figure-head");
        let over = w.read(|v| f32::from(v.panel_scroll.max_offset().y));
        let (pb, head) = (w.ui("panel.box"), w.ui("panel.figure.toggle"));
        note!(w, "panel overflows by {over:.0} px; panel {pb:?}; head {head:?}");
        let (Some(pb), Some(head)) = (pb, head) else { return false };
        if over < 1.0 {
            w.issue("the panel doesn't overflow here, so nothing is tested");
            return false;
        }
        if head.bottom() > pb.bottom() + px(1.) || head.top() < pb.top() {
            issue!(w, "the Figure head {head:?} is outside the panel {pb:?}");
        }
        // a control under the cut: scroll to it as a user would, then click it
        let b0: Vec<f32> = w.read(|v| v.doc.figure.items.iter().filter_map(|it| if let Item::Arrow(a) = it { Some(a.bend) } else { None }).collect());
        if !w.click_ui("many.bend.right", "Bend right (under the cut)") {
            return false;
        }
        w.shot("scrolled-to-bend");
        let b1: Vec<f32> = w.read(|v| v.doc.figure.items.iter().filter_map(|it| if let Item::Arrow(a) = it { Some(a.bend) } else { None }).collect());
        note!(w, "bends {b0:?} → {b1:?}");
        // and the head still opens the Figure section
        if !w.click_ui("panel.figure.toggle", "the Figure head") {
            return false;
        }
        w.shot("figure-section-open");
        let open = w.ui("field.fig.width").is_some();
        b0 != b1 && open
    });

    // U6: the label field clears what it edits
    run.task("QG2", "the label field doesn't sit on the loop or the node's selection box", 3, "A at 0,0\nB at 3,0\nA -f-> B\n", |w| {
        let c = w.centre(&ObjRef::Node("A".into())).unwrap();
        w.click(c, "node A");
        w.click(c, "node A again (its label)");
        let sel = w.read(|v| geom::extent_of(&v.doc.scene, &ObjRef::Node("A".into())).map(|_| ())).is_some();
        let (field, outline) = {
            let f = w.ui("label.field");
            let o = w.read(|v| ntex_studio::ui::figure::handles::outline(&v.doc.scene, &ObjRef::Node("A".into())));
            (f, o)
        };
        w.shot("node-field");
        let (Some(field), Some(outline)) = (field, outline) else {
            issue!(w, "no field ({sel}) or outline");
            return false;
        };
        let ob = rect_px(w, &outline);
        note!(w, "field {field:?}; selection box {ob:?}");
        let mut ok = !field.intersects(&ob);
        if !ok {
            w.issue("the label field overlaps the node's selection box");
        }
        w.key_raw("escape");
        w.focus_canvas();
        w.click(c, "node A");
        if !w.click_ui("button.node.A.loop", "Add loop arrow") {
            return false;
        }
        w.shot("loop-field");
        let loop_ext = w.read(|v| {
            let i = v.doc.figure.items.iter().position(|it| matches!(it, Item::Arrow(a) if a.is_loop))?;
            geom::extent_of(&v.doc.scene, &ObjRef::Arrow(i))
        });
        let (Some(field), Some(ext)) = (w.ui("label.field"), loop_ext) else {
            w.issue("no label field or loop extent after adding the loop");
            return false;
        };
        let lb = rect_px(w, &ext);
        note!(w, "field {field:?}; loop {lb:?}");
        if field.intersects(&lb) {
            w.issue("the label field overlaps the loop");
            ok = false;
        }
        ok
    });

    // U9: a text item shows its text (with the chip), not \text{…}
    run.task("QG3", "a text item's panel shows the text, not \\text{...}", 2, "A at 0,0\ntext \"kernel\" at (0,-2)\n", |w| {
        let c = w.centre(&ObjRef::Text(1)).unwrap();
        w.click(c, "the text");
        w.shot("text-panel");
        let shown = panel_field(w, "text.1").unwrap_or_default();
        let chip = w.ui("chip.text.1").is_some();
        note!(w, "field shows {shown:?}; chip {chip}");
        shown == "kernel" && chip
    });

    // U10: counter-clockwise left, clockwise right, in the Turn row
    run.task("QG4", "the Turn row reads ⟲ n ⟳ like Angle and Rotation", 2, "ngon P 5\n", |w| {
        let c = w.centre(&ObjRef::NGon("P".into())).unwrap();
        w.click(c, "the pentagon");
        w.shot("turn-row");
        let (Some(l), Some(r)) = (w.ui("button.ngon.P.turn+"), w.ui("button.ngon.P.turn-")) else { return false };
        note!(w, "⟲ at x {:.0}, ⟳ at x {:.0}", f32::from(l.center().x), f32::from(r.center().x));
        l.center().x < r.center().x
    });

    // U11: a double-click on a Cayley vertex names it
    run.task("QG5", "double-click a Cayley vertex to name it", 2, "cayley D4 <r, s>\n", |w| {
        let cg = cayley_of(w).unwrap();
        let top = top_vertex(w, &cg.name).unwrap();
        let q = vertex_pt(w, &cg.name, &top).unwrap();
        let q = w.xf_px(q);
        w.dclick(q, "a vertex");
        let want = ObjRef::CayleyVertex(cg.name.clone(), top.clone());
        let editing = w.read(|v| v.label_edit.as_ref().map(|e| e.obj.clone()));
        w.shot("field-open");
        if editing.as_ref() != Some(&want) {
            issue!(w, "double-click opened {editing:?}, wanted {want:?}");
            return false;
        }
        w.typ("x");
        w.key("enter", "commit");
        w.shot("named");
        let drawn = w.read(|v| v.doc.scene.draws.iter().any(|d| matches!(d, Draw::Label { obj: ObjRef::CayleyVertex(_, x), latex, .. } if *x == top && latex == "x")));
        note!(w, "line: {}", w.line("cayley"));
        drawn
    });

    // U12: the default name is a placeholder, not text
    run.task("QG6", "the rotation arrow's Name shows its default as a placeholder", 3, "ngon P 4 axes rotation\n", |w| {
        let Some(p) = rot_pt(w, "P", 0.5) else { return false };
        w.click(p, "the rotation arrow");
        if !w.click_ui("ngon.P.rot.cw", "↻") {
            return false;
        }
        w.shot("name-placeholder");
        let shown = panel_field(w, "ngon.P.rot.label");
        let g = first_ngon(w).unwrap();
        note!(w, "Name field holds {shown:?}; rotation_label {:?}", g.rotation_label);
        shown.as_deref() == Some("") && g.rotation_label.is_none()
    });

    // U13: a width that can't be had says why
    run.task("QG7", "typing a width the labels can't fit says so", 3, "A \"a long label here\" at 0,0\nB \"another long one\" at 1,0\nA -f-> B\n", |w| {
        let s0 = scale_of(w);
        if !w.click_ui("field.fig.width", "the Width field") {
            return false;
        }
        w.key_raw("cmd-a");
        w.typ("0.5");
        w.shot("too-narrow");
        let note = w.read(|v| v.width_note.clone());
        let s1 = scale_of(w);
        note!(w, "note {note:?}; scale {s0} → {s1}");
        let said = note.as_deref().is_some_and(|n| n.contains("labels alone"));
        // a width that works clears it
        w.key_raw("cmd-a");
        w.typ("8");
        w.key("enter", "commit");
        let gone = w.read(|v| v.width_note.is_none());
        said && gone
    });

    // U15b: a loop has no reverse; the readout stays off the labels
    run.task("QG8", "a loop's panel has no reverse button", 3, "A at 0,0\nB at 3,0\nA -f-> B\n", |w| {
        let c = w.centre(&ObjRef::Node("A".into())).unwrap();
        w.click(c, "node A");
        if !w.click_ui("button.node.A.loop", "Add loop arrow") {
            return false;
        }
        w.key_raw("escape");
        w.focus_canvas();
        let Some(i) = w.read(|v| v.doc.figure.items.iter().position(|it| matches!(it, Item::Arrow(a) if a.is_loop))) else { return false };
        let _ = w.window.update(&mut w.cx, |v, _, cx| {
            v.selection = vec![ObjRef::Arrow(i)];
            cx.notify();
        });
        w.draw();
        w.shot("loop-panel");
        let rev = w.ui(&format!("button.arrow.{i}.reverse")).is_some();
        let keys = w.ui_keys();
        note!(w, "reverse drawn: {rev}; controls {keys:?}");
        !rev
    });

    // F11: the turn steppers treat an old positive `turn` ≥ n (still accepted,
    // "unchanged") as out of range and jump to 0: ⟲ on turn=7 of a pentagon
    // (= turn 2) gives turn 0, not 3.
    run.task("R4", "⟲ on a pentagon written `turn=7` (the same as turn=2) steps to turn 3", 2, "ngon P 5 turn=7\n", |w| {
        let c = w.centre(&ObjRef::NGon("P".into())).unwrap();
        w.click(c, "the pentagon");
        if !w.click_ui("button.ngon.P.turn+", "⟲") {
            return false;
        }
        let g = ngon_of(w, "P").unwrap();
        note!(w, "turn after ⟲: {} (neg {}); `{}`", g.turn, g.turn_neg, w.line("ngon"));
        let k = if g.turn_neg { (g.turn % 5) as i32 - 5 } else { (g.turn % 5) as i32 };
        k.rem_euclid(5) == 3
    });

    // ============================================ fixes after the review (QF*)
    fn blank_px(w: &mut Win) -> Point<Pixels> {
        w.read(|v| {
            let b = v.canvas_bounds.get().unwrap();
            point(b.left() + px(14.), b.bottom() - px(14.))
        })
    }

    // U1: ⇧ decides on release. Dragging takes the object (and the selection)
    // along on one axis; a ⇧-click without moving toggles it.
    run.task("QF1", "⇧-drag moves the pressed object (and the selection) along a line, bystanders stay; ⇧-click toggles", 6, "A at 0,0\nB at 3,0\nngon P 5 at (1,-3)\n", |w| {
        let (cw, ch) = cell_px(w);
        let p0 = first_ngon(w).unwrap().pos;
        // an unselected A, ⇧-dragged: A alone moves, along one axis
        let a = w.centre(&ObjRef::Node("A".into())).unwrap();
        w.drag(a, &line_to(a, point(a.x + px(2.0 * cw), a.y + px(0.4 * ch)), 8), shift, "A across with ⇧", Some("locked"));
        let (pa, pb) = (node_pos(w, "A"), node_pos(w, "B"));
        let sel1 = w.selection();
        note!(w, "A {pa:?} B {pb:?}; selection {sel1:?}");
        let alone = near(pa, 2.0, 0.0) && near(pb, 3.0, 0.0) && first_ngon(w).unwrap().pos == p0;
        if !alone {
            issue!(w, "a ⇧-drag of one node moved a bystander or didn't lock: A {pa:?}, B {pb:?}");
        }
        // pick A and B, then ⇧-drag A down: the selected A is not toggled off, both move
        let blank = blank_px(w);
        w.click(blank, "empty canvas");
        let a = w.centre(&ObjRef::Node("A".into())).unwrap();
        w.click(a, "node A");
        let b = w.centre(&ObjRef::Node("B".into())).unwrap();
        shift_click(w, b, "node B");
        let a = w.centre(&ObjRef::Node("A".into())).unwrap();
        let u0 = undo_n(w);
        w.drag(a, &line_to(a, point(a.x + px(0.3 * cw), a.y + px(1.0 * ch)), 8), shift, "the selected A down with ⇧", Some("group"));
        let (qa, qb) = (node_pos(w, "A"), node_pos(w, "B"));
        let sel2 = w.selection();
        let steps = undo_n(w) - u0;
        note!(w, "A {qa:?} B {qb:?}; selection {sel2:?}; {steps} undo step(s)");
        let group = near(qa, 2.0, 1.0) && near(qb, 3.0, 1.0) && sel2.len() == 2 && steps == 1 && first_ngon(w).unwrap().pos == p0;
        if !group {
            issue!(w, "⇧-dragging a selected node didn't drag the selection: A {qa:?} B {qb:?} {sel2:?}");
        }
        // ⇧-click (no drag) on B takes it out of the selection
        let b = w.centre(&ObjRef::Node("B".into())).unwrap();
        shift_click(w, b, "node B");
        let sel3 = w.selection();
        note!(w, "after ⇧-click on B: {sel3:?}");
        w.shot("toggled");
        alone && group && sel3 == vec![ObjRef::Node("A".into())]
    });

    // U2 / U3: a paste never lands exactly on what it was copied from; nodes
    // step a whole cell (on the grid), shapes half a cell.
    run.task("QF2", "⌘V of what was copied, and of the same shorthand from elsewhere, never lands on the originals", 4, "A at 0,0\nB at 2,0\nA -f-> B\n", |w| {
        w.key("cmd-a", "select all");
        w.key("cmd-c", "copy");
        w.key("cmd-v", "paste");
        w.shot("pasted-own");
        let own: Vec<Option<Pos>> = w.selection().iter().filter_map(|o| if let ObjRef::Node(n) = o { Some(node_pos(w, n)) } else { None }).collect();
        // the same shorthand written elsewhere (Neovim, another window)
        clip_set(w, "A at 0,0\nB at 2,0\nA -f-> B\n");
        w.key("cmd-v", "paste the same text from elsewhere");
        w.shot("pasted-foreign");
        let there: Vec<Option<Pos>> = w.selection().iter().filter_map(|o| if let ObjRef::Node(n) = o { Some(node_pos(w, n)) } else { None }).collect();
        // text that lands on nothing lands where it was written
        clip_set(w, "Z at 9,9\n");
        w.key("cmd-v", "paste somewhere empty");
        let z = w.selection().iter().find_map(|o| if let ObjRef::Node(n) = o { node_pos(w, n) } else { None });
        note!(w, "own {own:?}; foreign {there:?}; empty spot {z:?}\n{}", w.source());
        let all: Vec<Option<Pos>> = w.read(|v| v.doc.figure.items.iter().filter_map(|it| if let Item::Node(n) = it { Some(n.pos) } else { None }).collect());
        let mut dup = false;
        for (i, p) in all.iter().enumerate() {
            dup |= all[i + 1..].contains(p);
        }
        if dup {
            issue!(w, "two nodes sit on the same cell: {all:?}");
        }
        !dup && own.iter().any(|p| near(*p, 1.0, 1.0)) && there.iter().any(|p| near(*p, 2.0, 2.0)) && near(z, 9.0, 9.0)
    });

    run.task("QF3", "⌘V steps a cell for a node and half a cell for a shape", 4, "A at 0,0\nngon P 5 at (4,-1)\n", |w| {
        let a = w.centre(&ObjRef::Node("A".into())).unwrap();
        w.click(a, "node A");
        w.key("cmd-c", "copy");
        w.key("cmd-v", "paste");
        let np = w.selection().iter().find_map(|o| if let ObjRef::Node(n) = o { node_pos(w, n) } else { None });
        let blank = blank_px(w);
        w.click(blank, "empty canvas");
        let c = w.centre(&ObjRef::NGon("P".into())).unwrap();
        w.click(c, "the pentagon");
        w.key("cmd-c", "copy");
        w.key("cmd-v", "paste");
        w.shot("pasted");
        let ps: Vec<NGon> = w.read(|v| v.doc.figure.items.iter().filter_map(|it| if let Item::NGon(g) = it { Some(g.clone()) } else { None }).collect());
        note!(w, "pasted node {np:?}; pentagons {:?}", ps.iter().map(|g| g.pos).collect::<Vec<_>>());
        let step = match (ps.first().and_then(|g| g.pos), ps.get(1).and_then(|g| g.pos)) {
            (Some(Pos::Abs(x0, y0)), Some(Pos::Abs(x1, y1))) => Some(((x1 - x0).abs(), (y1 - y0).abs())),
            _ => None,
        };
        near(np, 1.0, 1.0) && step.is_some_and(|(dx, dy)| (dx - 0.95).abs() < 0.03 && (dy - 0.675).abs() < 0.03)
    });

    // C12: an element can't be on the clipboard alone, and says why
    run.task("QF4", "⌘C on a single point says why it can't, and ⌘D / ⌥-drag add another point", 3, "blob G {g, k}\n", |w| {
        let g = w.centre(&ObjRef::Element("g".into())).unwrap();
        w.click(g, "the point g");
        let sel = w.selection();
        let before = clip_text(w);
        w.key("cmd-c", "copy");
        let t = toast_text(w);
        w.shot("explained");
        let after = clip_text(w);
        note!(w, "selection {sel:?}; toast {t:?}");
        w.key("cmd-d", "duplicate the point");
        let n = blob_of(w, "G").map(|b| b.elements.len()).unwrap_or(0);
        note!(w, "elements after ⌘D: {n}\n{}", w.source());
        sel == vec![ObjRef::Element("g".into())] && t.contains("point") && t.contains("blob") && before == after && n == 3
    });

    // U7: the ⌘-corner drag keeps the figure, and its readout, on the canvas
    run.task("QF5", "⌘-drag the page corner far off the canvas: the figure and its readout stay in view", 2, QB_SRC, |w| {
        cmd_key(w, true);
        let cmd = Modifiers::command();
        let r = w.read(|v| v.page_frame()).unwrap();
        let corner = w.xf_px(Pt { x: r.max.x, y: r.min.y });
        let bounds = w.read(|v| v.canvas_bounds.get().unwrap());
        w.hover(corner);
        w.event(PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: corner, modifiers: cmd, click_count: 1, first_mouse: false, pen: None }));
        w.draw();
        // way past the canvas's lower right
        let to = point(bounds.right() + px(300.), bounds.bottom() + px(300.));
        let mut inside = true;
        for q in line_to(corner, to, 16) {
            w.event(PlatformInput::MouseMove(MouseMoveEvent { position: q, pressed_button: Some(MouseButton::Left), modifiers: cmd, pen: None }));
            w.draw();
            let (rect, per) = w.read(|v| {
                let b = v.canvas_bounds.get().unwrap();
                let xf = v.view.xf(b);
                (v.drag_scene.as_ref().map(|s| geom::page_rect(s)), xf)
            });
            if let Some(rc) = rect {
                let c = per.to_px(Pt { x: rc.max.x, y: rc.min.y });
                inside &= c.x <= bounds.right() && c.y <= bounds.bottom() && c.x >= bounds.left() && c.y >= bounds.top();
            }
        }
        let live = w.read(|v| v.readout.clone().map(|(_, t)| t)).unwrap_or_default();
        w.shot("dragged-far");
        w.event(PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: to, modifiers: cmd, click_count: 1, pen: None }));
        w.draw();
        cmd_key(w, false);
        let s1 = scale_of(w);
        let (c1, bnds) = {
            let r = w.read(|v| geom::page_rect(&v.doc.scene));
            (w.xf_px(Pt { x: r.max.x, y: r.min.y }), bounds)
        };
        note!(w, "readout {live:?}; scale {s1}; corner now at {c1:?} in {bnds:?}");
        w.shot("dropped");
        if !inside {
            issue!(w, "the dragged corner left the canvas");
        }
        inside && s1 > 1.0 && c1.x <= bnds.right() && c1.y <= bnds.bottom() && live.contains("scale")
    });

    // U15a: ⌘0 keeps the selection's handles on the canvas
    run.task("QF6", "⌘0 with a tall blob selected: its rotate knob is on the canvas", 2, "blob G size=2,16 at (0,0)\n", |w| {
        let c = w.centre(&ObjRef::Blob("G".into())).unwrap();
        w.click(c, "the blob");
        w.key("cmd-0", "fit");
        w.shot("fitted");
        let bounds = w.read(|v| v.canvas_bounds.get().unwrap());
        let Some(k) = w.handle(|k| k == HKind::Rotate) else { return false };
        let ok = k.y >= bounds.top() + px(8.) && k.y <= bounds.bottom() - px(8.) && k.x >= bounds.left() && k.x <= bounds.right();
        note!(w, "rotate knob at {k:?} in {bounds:?}");
        ok
    });

    // ============================================ QH1: no footnote is cut off
    // Every footnote, in every state the figure window can be in, is one line
    // at the default width (W = 1180): measured from the laid-out rows (a
    // line that doesn't fit wraps, and the footer is pushed off the window).
    run.task("QH1", "every footnote fits on one line at the default window width", 1, "A at 0,0\nB at 3,0\nA -f-> B\nblob G \"G\" {e, g} at 0,-4\nngon P 4 rotation at 5,-4\ncayley D4 <r, s> at 7,2\ntext \"hi\" at (0,2)\n", |w| {
        fn hints_now(w: &mut Win) -> Vec<(String, f32, f32)> {
            w.draw();
            let n = w.read(|v| v.ui_bounds.borrow().keys().filter(|k| k.starts_with("footnote.")).count());
            let text: Vec<String> = w.read(|_| Vec::new());
            let _ = text;
            let hs = w.window.update(&mut w.cx, |v, window, cx| v.hints(window, cx).iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap();
            let mut out = Vec::new();
            for (i, h) in hs.into_iter().enumerate() {
                if let (Some(b), Some(t)) = (w.ui(&format!("footnote.{i}")), w.ui(&format!("footnote-text.{i}"))) {
                    // (text, row height, how far past the row's right edge the text runs)
                    out.push((h, f32::from(b.size.height), f32::from(t.right() - b.right()).max(0.0) + f32::from(b.bottom()) * 0.0));
                }
            }
            let _ = n;
            out
        }
        let mut seen: Vec<(String, String, f32, f32)> = Vec::new();
        let record = |w: &mut Win, state: &str, seen: &mut Vec<(String, String, f32, f32)>| {
            for (h, hgt, bottom) in hints_now(w) {
                seen.push((state.to_string(), h, hgt, bottom));
            }
        };
        let set_sel = |w: &mut Win, sel: Vec<ObjRef>| {
            let _ = w.window.update(&mut w.cx, |v, _, cx| {
                v.selection = sel;
                cx.notify();
            });
            w.draw();
        };
        w.focus_canvas();
        for (k, name) in [("v", "tool select"), ("n", "tool node"), ("a", "tool arrow"), ("b", "tool blob"), ("p", "tool polygon"), ("r", "tool rough")] {
            w.key_raw(k);
            record(w, name, &mut seen);
        }
        w.key_raw("v");
        w.shot("footnotes-select-tool");
        let sels: Vec<(&str, Vec<ObjRef>)> = vec![
            ("node", vec![ObjRef::Node("A".into())]),
            ("arrow", vec![ObjRef::Arrow(w.arrow_index("A", "B").unwrap_or(0))]),
            ("blob", vec![ObjRef::Blob("G".into())]),
            ("element", vec![ObjRef::Element("G.e".into())]),
            ("ngon", vec![ObjRef::NGon("P".into())]),
            ("ngon vertex", vec![ObjRef::NGonVertex("P".into(), 1)]),
            ("rotation arrow", vec![ObjRef::NGonRotation("P".into())]),
            ("cayley", vec![ObjRef::Cayley("D4".into())]),
            ("cayley vertex", vec![ObjRef::CayleyVertex("D4".into(), "r".into())]),
            ("text", vec![ObjRef::Text(w.read(|v| v.doc.figure.items.iter().position(|it| matches!(it, Item::Text(_))).unwrap_or(0)))]),
            ("several", vec![ObjRef::Node("A".into()), ObjRef::Node("B".into())]),
        ];
        for (name, sel) in sels {
            set_sel(w, sel);
            record(w, &format!("selected {name}"), &mut seen);
        }
        // the label field (↵ on a node), the shorthand, the ask box, a panel field
        set_sel(w, vec![ObjRef::Node("A".into())]);
        w.key_raw("enter");
        record(w, "label field", &mut seen);
        w.key_raw("escape");
        set_sel(w, vec![]);
        let _ = w.window.update(&mut w.cx, |v, window, cx| v.shorthand.read(cx).focus.clone().focus(window, cx));
        w.draw();
        record(w, "shorthand focused", &mut seen);
        let _ = w.window.update(&mut w.cx, |v, window, cx| v.ask_field.read(cx).focus.clone().focus(window, cx));
        w.draw();
        record(w, "ask focused", &mut seen);
        w.focus_canvas();
        set_sel(w, vec![ObjRef::NGon("P".into())]);
        let f = w.ui_keys().into_iter().find(|k| k.starts_with("field."));
        if let Some(f) = f
            && w.click_ui(&f, "a panel field")
        {
            record(w, "panel field focused", &mut seen);
        }
        w.focus_canvas();
        set_sel(w, vec![ObjRef::Node("A".into())]);
        w.shot("footnotes-node-selected");
        let base = seen.iter().map(|s| s.2).fold(f32::MAX, f32::min);
        let bad: Vec<String> = seen.iter().filter(|s| s.2 > base * 1.4 || s.3 > 0.5).map(|s| format!("[{}] {:.0}px tall, runs {:.0}px past the edge: {}", s.0, s.2, s.3, s.1)).collect();
        note!(w, "{} footnotes measured in {} states; one line is {base:.1} px; widest string {} chars", seen.len(), seen.iter().map(|s| s.0.clone()).collect::<std::collections::BTreeSet<_>>().len(), seen.iter().map(|s| s.1.chars().count()).max().unwrap_or(0));
        for b in &bad {
            issue!(w, "footnote wraps: {b}");
        }
        !seen.is_empty() && bad.is_empty()
    });

    // ---------------------------------------------------------------- report
    let mut out = String::new();
    for r in &run.out {
        let lat: Vec<String> = r.lat.iter().filter(|(_, f)| !f.is_empty()).map(|(w, f)| format!("{w}: median {:.1} / p95 {:.1} / max {:.1} ms", pct(f, 0.5), pct(f, 0.95), pct(f, 1.0))).collect();
        out.push_str(&format!(
            "## {} — {}\nresult: {} · steps {} (ideal {})\nsteps: {}\nissues:\n{}\nnotes:\n{}\nlatency: {}\nundo: {}\ndiff:\n{}\nshots: {}\n\n",
            r.id,
            r.words,
            if r.ok { "done" } else { "NOT DONE" },
            r.steps.len(),
            r.ideal,
            r.steps.join(" · "),
            r.issues.iter().map(|s| format!("  - {s}")).collect::<Vec<_>>().join("\n"),
            r.notes.iter().map(|s| format!("  - {s}")).collect::<Vec<_>>().join("\n"),
            lat.join("; "),
            r.undo,
            r.diff,
            r.shots.join(", ")
        ));
    }
    std::fs::create_dir_all(evidence()).unwrap();
    std::fs::write(evidence().join(if run.only.is_some() { "results-partial.txt" } else { "results.txt" }), &out).unwrap();
    eprintln!("{out}");
    let real_after: Vec<(PathBuf, Vec<u8>)> = std::fs::read_dir(&real_templates).unwrap().flatten().map(|e| (e.path(), std::fs::read(e.path()).unwrap_or_default())).collect();
    if real_before != real_after {
        eprintln!("FAIL: the real templates changed!");
        std::process::exit(1);
    }
    let done = run.out.iter().filter(|r| r.ok).count();
    println!("usability: {done}/{} tasks done (see evidence/usability/results.txt)", run.out.len());
    // the review's R tasks are regression tests: a NOT DONE one fails the run
    let review_failed: Vec<&str> = run.out.iter().filter(|r| r.id.starts_with('R') && !r.ok).map(|r| r.id.as_str()).collect();
    if !review_failed.is_empty() {
        eprintln!("FAIL: review tasks not done: {}", review_failed.join(", "));
        std::process::exit(1);
    }
}
