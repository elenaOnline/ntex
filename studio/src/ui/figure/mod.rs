//! Ntex Figures: the figure window (FIGURES.md "The figure window").
//! Owner: FIG-UI.
//!
//! One window per `.fig` path: `ntex-studio --figure /abs/x.fig`, or an
//! OpenRequest with `figure` over the socket. Asking for a path that is
//! already open focuses its window.
//!
//! - `doc`: the source and what the figures crate makes of it (guarded:
//!   pieces still `todo!()` degrade instead of panicking)
//! - `geom`: cm ↔ px, the palette, snapping, a local hit test
//! - `canvas`: the exact render / native draft painter and the mouse
//! - `tools`: V N A B P R
//! - `ask`: the Ask box's state
//! - `view`: the window's root view
//! - direct manipulation (FIGURES.md "Direct manipulation"): `handles`
//!   (where handles sit, what dragging one does), `interact` (picking and
//!   every object command), `inspector` (the properties panel), `menu` (the
//!   right-click menu), `reference` (⌘/)
pub mod ask;
pub mod canvas;
pub mod display;
pub mod doc;
pub mod exact;
pub mod geom;
pub mod handles;
pub mod inspector;
pub mod interact;
pub mod labels;
pub mod menu;
pub mod reference;
pub mod tools;
pub mod view;

use gpui::{App, AppContext as _, Bounds, Global, TitlebarOptions, WindowBounds, WindowHandle, WindowOptions, point, px, size};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use view::{FigureOptions, FigureView};

/// The figure windows' keys (the studio's `app::init` calls this).
pub fn init(cx: &mut App) {
    view::bind_keys(cx);
}

#[derive(Default)]
struct FigureWindows(HashMap<PathBuf, WindowHandle<FigureView>>);
impl Global for FigureWindows {}

/// The path as the registry knows it: absolute, with the folder resolved
/// (the file itself may not exist yet).
pub fn normalize(path: &Path) -> PathBuf {
    let abs = if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(path) };
    match (abs.parent().and_then(|p| p.canonicalize().ok()), abs.file_name()) {
        (Some(dir), Some(name)) => dir.join(name),
        _ => abs,
    }
}

/// Open (or focus) the window for this `.fig`. `quiet`: hidden, unfocused,
/// never activates (NTEX_STUDIO_QUIET).
pub fn open(path: &Path, quiet: bool, cx: &mut App) -> Option<WindowHandle<FigureView>> {
    open_mode(path, crate::ui::launch::Mode::of(quiet, false), cx)
}

/// As `open`, with the launch mode: `Background` shows the window without
/// making it key, never activates the app and orders it behind the others.
pub fn open_mode(path: &Path, mode: crate::ui::launch::Mode, cx: &mut App) -> Option<WindowHandle<FigureView>> {
    use crate::ui::launch;
    let quiet = mode == launch::Mode::Quiet;
    let plan = launch::plan(mode, true);
    let path = normalize(path);
    if let Some(dir) = path.parent()
        && !dir.exists()
    {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Some(h) = cx.try_global::<FigureWindows>().and_then(|w| w.0.get(&path).copied()) {
        let alive = h
            .update(cx, |_, window, _| {
                if plan.activate {
                    window.activate_window();
                }
            })
            .is_ok();
        if alive {
            if plan.activate {
                launch::release_background();
                cx.activate(true);
            }
            return Some(h);
        }
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    // wide enough for the properties panel (it folds away below 1000 px)
    let bounds = Bounds::centered(None, size(px(1180.), px(900.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some(format!("{name} — Ntex Figures").into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(14.), px(14.))),
        }),
        window_min_size: Some(size(px(620.), px(560.))),
        focus: plan.focus,
        show: plan.show,
        ..Default::default()
    };
    let p2 = path.clone();
    let handle = cx
        .open_window(options, move |window, cx| {
            cx.new(|cx| {
                launch::release_on_activation(window, cx);
                FigureView::new(FigureOptions { path: p2, quiet, render: true, build: true }, window, cx)
            })
        })
        .ok()?;
    let _ = handle.update(cx, |_, window, cx| {
        // the red button saves first (the window really closes)
        let h = handle;
        window.on_window_should_close(cx, move |_, cx| {
            let _ = h.update(cx, |v, _, cx| {
                if v.dirty() {
                    // as ⌘W: write and build (a build of older text is
                    // cancelled; this one finishes in the background, and
                    // quitting the studio stops it)
                    v.save(cx);
                }
            });
            true
        });
    });
    if cx.try_global::<FigureWindows>().is_none() {
        cx.set_global(FigureWindows::default());
        // quitting writes every unsaved figure
        cx.on_app_quit(|cx| {
            let handles: Vec<_> = cx.try_global::<FigureWindows>().map(|w| w.0.values().copied().collect()).unwrap_or_default();
            for h in handles {
                let _ = h.update(cx, |v, _, cx| {
                    if v.dirty() {
                        v.write(cx);
                    }
                    v.ask.cancel();
                    // stops a tectonic still rendering (no orphans)
                    v.render.renderer = None;
                });
            }
            // and every PDF build, closed windows' too: each kills and reaps
            // its tectonic; wait (briefly) until they have
            view::cancel_all_builds();
            let executor = cx.background_executor().clone();
            async move {
                let start = std::time::Instant::now();
                while view::builds_running() > 0 && start.elapsed() < std::time::Duration::from_millis(80) {
                    executor.timer(std::time::Duration::from_millis(5)).await;
                }
            }
        })
        .detach();
    }
    cx.global_mut::<FigureWindows>().0.insert(path, handle);
    if plan.activate {
        launch::release_background();
        cx.activate(true);
    }
    if plan.order_back {
        launch::order_back_if_inactive();
    }
    Some(handle)
}

/// How many figure windows are open (smoke logs).
pub fn open_count(cx: &App) -> usize {
    cx.try_global::<FigureWindows>().map_or(0, |w| w.0.len())
}
