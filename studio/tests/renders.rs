//! Offscreen renders of every page, driving the real Studio view through
//! GPUI's HeadlessAppContext with the Metal renderer and the real macOS text
//! system. No window ever appears; nothing activates the app.
//!
//! `harness = false`: AppKit wants the text system on the main thread, which
//! libtest's workers are not.
//!
//! Every path points at a temp copy of ~/.config/texnvim
//! (`NTEX_STUDIO_CONFIG`, `TEXNVIM_SNIPPETS`, `NTEX_STUDIO_STATE`); the real
//! files are never written. PNGs land in studio/evidence/screens/.
//!
//! `cargo test --test renders` (add `NTEX_RENDERS_SORT=1` to also run a real
//! sort with the claude CLI on the temp copy).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{AppContext as _, HeadlessAppContext, Keystroke, WindowHandle, px, size};
use ntex_studio::model::*;
use ntex_studio::ui::app::{BridgeState, Page, Studio, StudioOptions};

const W: f32 = 860.0;
const H: f32 = 960.0;

struct Win {
    cx: HeadlessAppContext,
    window: WindowHandle<Studio>,
}

impl Win {
    fn open(width: f32, height: f32, bridge: bool) -> Win {
        let text = gpui_platform::current_platform(true).text_system();
        let mut cx = HeadlessAppContext::with_platform(text, Arc::new(()), gpui_platform::current_headless_renderer);
        cx.update(ntex_studio::ui::app::init);
        let t0 = Instant::now();
        let window = cx
            .open_window(size(px(width), px(height)), move |window, cx| {
                cx.new(|cx| Studio::new(StudioOptions { quiet: true, bridge }, window, cx))
            })
            .unwrap();
        let mut w = Win { cx, window };
        w.draw();
        eprintln!("  first frame: {:.1} ms (Studio::new + layout + paint, bridge spawned)", ms(t0.elapsed()));
        for _ in 0..2 {
            w.draw();
        }
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
    }

    /// Type text literally, one key at a time.
    fn typ(&mut self, text: &str) {
        for ch in text.chars() {
            let k = match ch {
                ' ' => "space".to_string(),
                '\n' => "enter".to_string(),
                '\t' => "tab".to_string(),
                c => c.to_string(),
            };
            self.key(&k);
        }
    }

    fn with<R>(&mut self, f: impl FnOnce(&mut Studio, &mut gpui::Window, &mut gpui::Context<Studio>) -> R) -> R {
        let r = self.window.update(&mut self.cx, f).unwrap();
        self.draw();
        r
    }

    fn read<R>(&mut self, f: impl FnOnce(&Studio) -> R) -> R {
        self.window.update(&mut self.cx, |s, _, _| f(s)).unwrap()
    }

    /// Advance the app's clock and drain every receiver.
    fn tick(&mut self, ms: u64) {
        self.cx.advance_clock(Duration::from_millis(ms));
        self.with(|s, w, cx| s.pump(w, cx));
    }

    /// Tick until `cond` holds or `timeout` (real time) passes.
    fn wait(&mut self, what: &str, timeout: Duration, cond: impl Fn(&Studio) -> bool) -> bool {
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

    fn focus_output(&mut self) {
        self.with(|s, w, cx| {
            let f = s.snippet.output.read(cx).focus.clone();
            f.focus(w, cx);
        });
    }

    fn focus_page(&mut self) {
        self.with(|s, w, cx| s.focus.clone().focus(w, cx));
    }
}

fn evidence() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("evidence/screens")
}

/// The temp root of this run; removed on drop (and before a failing exit).
struct TempRoot(PathBuf);

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp_root() -> PathBuf {
    std::env::temp_dir().join(format!("ntex-studio-renders-{}", std::process::id()))
}

/// Every snippet file under `tex/`, by name (for byte-for-byte comparisons).
fn snippet_files(config: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    if let Ok(rd) = std::fs::read_dir(config.join("snippets/tex")) {
        for e in rd.flatten() {
            if e.path().is_file() {
                out.insert(e.file_name().to_string_lossy().into_owned(), std::fs::read(e.path()).unwrap_or_default());
            }
        }
    }
    out
}

/// A private copy of ~/.config/texnvim (without the studio's build).
fn temp_config() -> PathBuf {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap());
    let real = home.join(".config/texnvim");
    let root = temp_root();
    let _ = std::fs::remove_dir_all(&root);
    let config = root.join("texnvim");
    std::fs::create_dir_all(&config).unwrap();
    let ok = std::process::Command::new("rsync")
        .args(["-a", "--exclude", "/studio/target", "--exclude", "/studio/evidence", "--exclude", ".git"])
        .arg(format!("{}/", real.display()))
        .arg(format!("{}/", config.display()))
        .status()
        .unwrap()
        .success();
    assert!(ok, "rsync of the config failed");
    // start from an empty inbox
    let _ = std::fs::remove_file(config.join("snippets/tex/inbox.lua"));
    // SAFETY: single-threaded here, before any GPUI or bridge thread starts.
    unsafe {
        std::env::set_var("NTEX_STUDIO_CONFIG", &config);
        std::env::set_var("TEXNVIM_SNIPPETS", config.join("snippets"));
        std::env::set_var("NTEX_STUDIO_STATE", root.join("state"));
    }
    config
}

fn inbox_text(config: &Path) -> String {
    std::fs::read_to_string(config.join("snippets/tex/inbox.lua")).unwrap_or_default()
}

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn stats(name: &str, mut v: Vec<f64>) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p = |q: f64| v[((v.len() as f64 - 1.0) * q).round() as usize];
    eprintln!("  {name}: n={} median {:.2} ms · p95 {:.2} ms · max {:.2} ms", v.len(), p(0.5), p(0.95), v[v.len() - 1]);
}

fn main() {
    let only = std::env::args().nth(1).filter(|a| !a.starts_with('-'));
    // leftovers of runs that were killed before they could clean up
    if let Ok(rd) = std::fs::read_dir(std::env::temp_dir()) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if let Some(pid) = name.strip_prefix("ntex-studio-renders-").and_then(|p| p.parse::<i32>().ok())
                && pid != std::process::id() as i32
                && unsafe { kill(pid, 0) } != 0
            {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
    let config = temp_config();
    let _cleanup = TempRoot(temp_root());
    let real_inbox = PathBuf::from(std::env::var_os("HOME").unwrap()).join(".config/texnvim/snippets/tex/inbox.lua");
    let real_before = std::fs::read(&real_inbox).ok();
    eprintln!("renders: config copy at {}", config.display());
    let mut failures: Vec<String> = Vec::new();

    let mut w = Win::open(W, H, true);

    // -- the empty page, first frame ------------------------------------------
    eprintln!("snippet: empty");
    let focused = w.read(|s| s.page == Page::Snippet);
    assert!(focused);
    w.shot("01-snippet-empty");

    // -- the empty inbox ------------------------------------------------------
    w.key("cmd-3");
    w.shot("02-inbox-empty");
    w.key("cmd-1");

    // -- wait for the bridge (the save is checked by it when it is up) ---------
    let ready = w.wait("bridge ready", Duration::from_secs(120), |s| {
        s.bridge_ready() || matches!(s.bridge_state, BridgeState::Failed(_))
    });
    let state = w.read(|s| s.bridge_state.clone());
    eprintln!("  bridge: {state:?}");
    if !ready || state != BridgeState::Ready {
        failures.push(format!("bridge not ready: {state:?}"));
    }

    // -- type a snippet end to end ---------------------------------------------
    eprintln!("snippet: typed end to end");
    w.with(|s, w, cx| {
        let f = s.snippet.trigger.read(cx).focus.clone();
        f.focus(w, cx);
    });
    w.typ("ff");
    w.key("enter");
    let in_output = w.with(|s, w, cx| s.snippet.output.read(cx).focus.is_focused(w));
    if !in_output {
        failures.push("Return in the trigger field did not move focus to the output".into());
    }
    w.typ("\\frac{");
    w.key("tab");
    w.typ("}{");
    w.key("tab");
    w.typ("}");
    let lua = w.read(|s| s.snippet.lua.clone());
    eprintln!("  lua: {lua}");
    w.shot("03-snippet-typed");
    w.key("cmd-enter");
    w.wait("save", Duration::from_secs(10), |s| s.snippet.saving.is_none() && s.snippet_is_empty());
    let text = inbox_text(&config);
    let want = r#"math("ff", [[\frac{<>}{<>}]], "yours"),"#;
    if text.contains(want) {
        eprintln!("  inbox.lua holds {want}");
    } else {
        failures.push(format!("inbox.lua lacks {want}:\n{text}"));
    }
    let hides = w.read(|s| s.hides);
    if hides != 1 {
        failures.push(format!("expected one hide after save (prefs.hide_after_save), got {hides}"));
    }
    w.tick(200);
    w.shot("04-snippet-saved-toast");
    // the toast: full slate ink for 3.5 s, then gone after a 400 ms fade
    let a = w.read(|s| s.toast.as_ref().map(|t| t.opacity(t.at + Duration::from_millis(3400))));
    let b = w.read(|s| s.toast.as_ref().map(|t| t.opacity(t.at + Duration::from_millis(3700))));
    let c = w.read(|s| s.toast.as_ref().map(|t| t.opacity(t.at + Duration::from_millis(3950))));
    eprintln!("  toast opacity at 3.4 s {a:?} · 3.7 s {b:?} · 3.95 s {c:?}");
    if a != Some(1.0) || c != Some(0.0) {
        failures.push(format!("toast fade: {a:?} {b:?} {c:?}"));
    }

    // ⌘Z on the page undoes the save, ⌘⇧Z redoes it
    w.focus_page();
    w.key("cmd-z");
    let undone = !inbox_text(&config).contains(want);
    w.key("cmd-shift-z");
    let redone = inbox_text(&config).contains(want);
    if !(undone && redone) {
        failures.push(format!("page undo/redo of the save: undone={undone} redone={redone}"));
    }
    w.tick(1100); // the mtime watch and the bridge's list catch up
    w.tick(3000); // and the "Redid" toast is gone before the next shots

    // -- findings: a duplicate trigger, no output yet ---------------------------
    eprintln!("snippet: findings");
    w.with(|s, w, cx| s.set_page(Page::Snippet, w, cx));
    w.wait("known snippets include the inbox", Duration::from_secs(20), |s| {
        s.known.iter().any(|k| k.trigger == "ff" && k.file.contains("inbox"))
    });
    w.typ("ff");
    w.key("enter");
    w.tick(50);
    let findings = w.read(|s| s.snippet.findings.iter().map(|f| f.code.clone()).collect::<Vec<_>>());
    eprintln!("  findings: {findings:?}");
    w.shot("05-snippet-findings");

    // -- filled: Try it table and the rendered output ---------------------------
    eprintln!("snippet: try it + render");
    w.typ("\\sqrt{");
    w.key("tab");
    w.typ("}");
    w.wait("try it", Duration::from_secs(30), |s| s.snippet.try_.result.is_some() && !s.snippet.try_.running());
    w.wait("render", Duration::from_secs(90), |s| {
        s.snippet.preview.image.is_some() || s.snippet.preview.error.is_some() || !s.prefs.render_preview
    });
    w.tick(300);
    let tr = w.read(|s| s.snippet.try_.result.clone());
    eprintln!("  try: {tr:?}");
    let rendered = w.read(|s| (s.snippet.preview.image.as_ref().map(|i| (i.1, i.2)), s.snippet.preview.error.clone()));
    eprintln!("  render: {rendered:?}");
    w.shot("06-snippet-try");

    // -- input-to-frame in the output editor -------------------------------------
    eprintln!("input-to-frame (output editor)");
    w.focus_output();
    let mut draw_only = Vec::new();
    for i in 0..60 {
        let key = if i % 2 == 0 { "x" } else { "backspace" };
        let t0 = Instant::now();
        w.cx.update_window(w.window.into(), |_, window, cx| {
            window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx);
        })
        .unwrap();
        w.cx.run_until_parked();
        w.cx.update_window(w.window.into(), |_, window, cx| window.draw(cx).clear(cx)).unwrap();
        draw_only.push(ms(t0.elapsed()));
    }
    stats("keystroke → frame built (dispatch + update + layout + paint)", draw_only.clone());
    let mut with_gpu = Vec::new();
    for i in 0..20 {
        let key = if i % 2 == 0 { "x" } else { "backspace" };
        let t0 = Instant::now();
        w.cx.update_window(w.window.into(), |_, window, cx| {
            window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx);
        })
        .unwrap();
        w.cx.run_until_parked();
        let _ = w.cx.capture_screenshot(w.window.into()).unwrap();
        with_gpu.push(ms(t0.elapsed()));
    }
    stats("keystroke → pixels read back (adds Metal render + GPU readback)", with_gpu);
    let mut trigger_keys = Vec::new();
    w.with(|s, w, cx| {
        let f = s.snippet.trigger.read(cx).focus.clone();
        f.focus(w, cx);
    });
    for i in 0..40 {
        let key = if i % 2 == 0 { "q" } else { "backspace" };
        let t0 = Instant::now();
        w.cx.update_window(w.window.into(), |_, window, cx| {
            window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx);
        })
        .unwrap();
        w.cx.run_until_parked();
        w.cx.update_window(w.window.into(), |_, window, cx| window.draw(cx).clear(cx)).unwrap();
        trigger_keys.push(ms(t0.elapsed()));
    }
    stats("trigger field keystroke → frame built", trigger_keys);
    w.tick(700);

    // -- more options open ------------------------------------------------------
    w.with(|s, _, cx| {
        s.snippet.options_open = true;
        cx.notify();
    });
    w.with(|s, _, _| s.scroll.set_offset(gpui::point(px(0.), px(-420.))));
    w.shot("07-snippet-options");
    w.with(|s, _, _| s.scroll.set_offset(gpui::point(px(0.), px(0.))));

    // -- Tab from the page moves between controls ---------------------------------
    w.focus_page();
    w.key("tab");
    let moved = w.with(|s, w, cx| !s.focus.is_focused(w) && w.focused(cx).is_some());
    if !moved {
        failures.push("Tab on the page did not move focus to a control".into());
    }

    // -- the error flash: save with no output ------------------------------------
    w.key("cmd-n");
    w.typ("zz");
    w.key("cmd-enter");
    w.tick(16);
    w.shot("08-snippet-error-flash");
    w.key("cmd-n");

    // -- block page: a new hue --------------------------------------------------
    eprintln!("block: new hue");
    w.key("cmd-2");
    w.typ("Fun fact");
    w.with(|s, w, cx| {
        let f = s.block.trigger.read(cx).focus.clone();
        f.focus(w, cx);
    });
    w.typ("fun");
    w.with(|s, _, cx| {
        s.block.hue = 145.0;
        s.block.draft.colour = BlockColour::Hue(145.0);
        if !s.block.pinned.contains(&"colour") {
            s.block.pinned.push("colour");
        }
        s.block_changed(cx);
    });
    w.wait("block specimen", Duration::from_secs(90), |s| {
        s.block.preview.image.is_some() || s.block.preview.error.is_some() || !s.prefs.render_preview
    });
    w.tick(300);
    let env = w.read(|s| (s.block.draft.env.clone(), s.block.findings.iter().map(|f| f.code.clone()).collect::<Vec<_>>()));
    eprintln!("  block: {env:?}");
    w.shot("09-block-new-hue");
    w.key("cmd-enter");
    w.tick(100);
    let sty = std::fs::read_to_string(config.join("templates/tex/ntex-blocks.sty")).unwrap_or_default();
    if !sty.contains("funfact") {
        failures.push("ntex-blocks.sty lacks funfact after saving the block".into());
    }
    w.focus_page();
    w.shot("10-block-saved");
    let on_block = w.read(|s| s.toast.is_some());
    w.key("cmd-1");
    let after_switch = w.read(|s| s.toast.is_some());
    if !on_block || after_switch {
        failures.push(format!("the block toast should show on the block page only: on block {on_block}, after ⌘1 {after_switch}"));
    }
    w.key("cmd-2");

    // -- the inbox with entries ---------------------------------------------------
    eprintln!("inbox: entries");
    w.key("cmd-3");
    w.tick(1100);
    let n = w.read(|s| s.inbox.len());
    eprintln!("  {n} entries");
    w.key("down");
    w.shot("11-inbox-entries");

    // -- the sorting log -----------------------------------------------------------
    if std::env::var_os("NTEX_RENDERS_SORT").is_some() {
        eprintln!("inbox: real sort");
        let before_sort = snippet_files(&config);
        w.key("cmd-shift-s");
        w.wait("sort", Duration::from_secs(200), |s| !s.inbox_page.busy());
        let (status, plan, log) = w.read(|s| (s.inbox_page.status.clone(), s.inbox_page.plan.clone(), s.inbox_page.log.clone()));
        eprintln!("  status: {status:?}");
        for (sev, line) in &log {
            eprintln!("  log {sev:?}: {line}");
        }
        if let Some(plan) = &plan {
            for m in &plan.moves {
                eprintln!("  plan: {} → {} after {:?} (new title {:?}): {}", m.id, m.file, m.after_trigger, m.new_title, m.reason);
            }
            if !plan.notes.is_empty() {
                eprintln!("  plan notes: {}", plan.notes);
            }
        }
        w.shot("12-inbox-sorted");
        let after_sort = snippet_files(&config);
        let touched: Vec<&String> = after_sort.keys().filter(|k| before_sort.get(*k) != after_sort.get(*k)).collect();
        eprintln!("  files changed by the sort: {touched:?}");
        if status != ntex_studio::ui::screens::inbox::SortStatus::Done {
            failures.push(format!("the sort did not finish: {status:?}"));
        }
        // one ⌘Z on the page puts every file back, byte for byte
        w.focus_page();
        w.key("cmd-z");
        w.tick(1100);
        let undone = snippet_files(&config);
        if undone == before_sort {
            eprintln!("  undo: all {} snippet files byte-identical to before the sort", undone.len());
        } else {
            let diff: Vec<&String> = before_sort.keys().chain(undone.keys()).filter(|k| before_sort.get(*k) != undone.get(*k)).collect();
            failures.push(format!("undo of the sort left differences in {diff:?}"));
        }
        w.with(|s, _, cx| {
            s.inbox_page.status = ntex_studio::ui::screens::inbox::SortStatus::Idle;
            s.inbox_page.log.clear();
            s.inbox_page.plan = None;
            cx.notify();
        });
    } else {
        // a sort in progress, as the log shows it (no agent is run)
        let ids: Vec<(String, String)> = w.read(|s| s.inbox.iter().map(|e| (e.id.clone(), e.trigger.clone())).collect());
        w.with(|s, _, cx| {
            use ntex_studio::ui::screens::inbox::SortStatus;
            let p = &mut s.inbox_page;
            p.log = vec![
                (Severity::Note, "Asking Haiku 5.5 where each snippet belongs …".into()),
                (Severity::Note, "reading operations.lua …".into()),
                (Severity::Note, "reading environments.lua …".into()),
            ];
            p.plan = Some(SortPlan {
                moves: ids
                    .iter()
                    .map(|(id, t)| SortMove {
                        id: id.clone(),
                        file: if t == "fun" { "environments.lua".into() } else { "operations.lua".into() },
                        after_trigger: Some(if t == "fun" { "beg".into() } else { "//".into() }),
                        new_title: None,
                        reason: if t == "fun" { "the other Tab environments".into() } else { "fractions live there".into() },
                    })
                    .collect(),
                notes: String::new(),
            });
            p.status = SortStatus::Sorting;
            cx.notify();
        });
        w.shot("12-inbox-sorting");
        w.with(|s, _, cx| {
            s.inbox_page.status = ntex_studio::ui::screens::inbox::SortStatus::Idle;
            s.inbox_page.log.clear();
            s.inbox_page.plan = None;
            cx.notify();
        });
    }

    // -- the inbox: ⌫ deletes, ⌘Z restores ---------------------------------------
    let before = w.read(|s| s.inbox.len());
    w.with(|s, w, cx| s.inbox_page.focus.clone().focus(w, cx));
    w.key("backspace");
    let after = w.read(|s| s.inbox.len());
    w.focus_page();
    w.key("cmd-z");
    let restored = w.read(|s| s.inbox.len());
    if !(after + 1 == before && restored == before) {
        failures.push(format!("delete/undo on the inbox: {before} → {after} → {restored}"));
    }

    // -- Return on a row opens it on page 1 with its id ----------------------------
    w.with(|s, w, cx| {
        s.set_page(Page::Inbox, w, cx);
        s.inbox_page.row = 0;
        s.inbox_page.focus.clone().focus(w, cx)
    });
    let dbg = w.read(|s| (s.inbox.iter().map(|e| (e.id.clone(), e.trigger.clone(), e.draft.is_some())).collect::<Vec<_>>(), s.page, s.inbox_page.row));
    eprintln!("  before Return: {dbg:?}");
    w.key("enter");
    let editing = w.read(|s| (s.page, s.snippet.draft.id.clone()));
    if editing.0 != Page::Snippet || editing.1.is_none() {
        failures.push(format!("Return on an inbox row did not open it for editing: {editing:?}"));
    }
    w.shot("13-snippet-editing-entry");

    drop(w);

    // -- Palatino ------------------------------------------------------------------
    if only.as_deref() != Some("fast") {
        let prefs = config.join("prefs.lua");
        let original = std::fs::read_to_string(&prefs).unwrap_or_default();
        let palatino = format!("{original}\nstudio = {{ typeface = \"Palatino\", render_preview = false }}\n");
        std::fs::write(&prefs, palatino).unwrap();
        let mut p = Win::open(W, H, false);
        p.typ("ff");
        p.key("enter");
        p.typ("\\frac{");
        p.key("tab");
        p.typ("}{");
        p.key("tab");
        p.typ("}");
        p.tick(50);
        p.shot("14-snippet-palatino");
        p.with(|s, _, cx| {
            s.snippet.options_open = true;
            cx.notify();
        });
        p.with(|s, _, _| s.scroll.set_offset(gpui::point(px(0.), px(-560.))));
        p.shot("15-snippet-options-palatino");
        p.key("cmd-2");
        p.typ("Fun fact");
        p.with(|s, w, cx| {
            let f = s.block.trigger.read(cx).focus.clone();
            f.focus(w, cx);
        });
        p.typ("fun");
        p.tick(50);
        p.shot("16-block-palatino");
        drop(p);
        std::fs::write(&prefs, original).unwrap();
    }

    // never the real files
    let real_after = std::fs::read(&real_inbox).ok();
    if real_before != real_after {
        failures.push("the real inbox.lua changed!".into());
    }

    if failures.is_empty() {
        println!("renders: ok");
    } else {
        for f in &failures {
            eprintln!("FAIL: {f}");
        }
        drop(_cleanup);
        std::process::exit(1);
    }
}
