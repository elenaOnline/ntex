//! The Library page (page 4), driven offscreen like renders.rs: search, keys,
//! editing a snippet in place in its own file (checked and tried by a real
//! headless ntex), undo, and a hand-written (read-only) entry edited as Lua.
//!
//! `harness = false` (AppKit wants the main thread). No window appears:
//! HeadlessAppContext, Metal offscreen. Every path points at a temp copy of
//! ~/.config/texnvim; her files are only read (and compared before/after).
//! PNGs land in studio/evidence/library/.
//!
//! `cargo test --test library`

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{AppContext as _, HeadlessAppContext, Keystroke, WindowHandle, px, size};
use ntex_studio::ui::app::{BridgeState, Page, Studio, StudioOptions};

struct Win {
    cx: HeadlessAppContext,
    window: WindowHandle<Studio>,
}

impl Win {
    fn open() -> Win {
        let text = gpui_platform::current_platform(true).text_system();
        let mut cx = HeadlessAppContext::with_platform(text, Arc::new(()), gpui_platform::current_headless_renderer);
        cx.update(ntex_studio::ui::app::init);
        let window = cx
            .open_window(size(px(860.), px(960.)), move |window, cx| cx.new(|cx| Studio::new(StudioOptions { quiet: true, bridge: true }, window, cx)))
            .unwrap();
        let mut w = Win { cx, window };
        w.draw();
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
    }
    fn typ(&mut self, text: &str) {
        for ch in text.chars() {
            let k = match ch {
                ' ' => "space".to_string(),
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
    fn tick(&mut self, ms: u64) {
        self.cx.advance_clock(Duration::from_millis(ms));
        self.with(|s, w, cx| s.pump(w, cx));
    }
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
    fn shot(&mut self, name: &str) {
        self.draw();
        let img = self.cx.capture_screenshot(self.window.into()).unwrap();
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("evidence/library");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(format!("{name}.png"));
        img.save(&p).unwrap();
        eprintln!("  wrote {}", p.display());
    }
    fn query(&mut self, q: &str) {
        self.with(|s, w, cx| {
            s.set_page(Page::Library, w, cx);
            s.library.search.update(cx, |f, cx| f.select_all(cx));
        });
        self.key("backspace");
        self.typ(q);
    }
    fn selected(&mut self) -> Option<(String, String)> {
        self.read(|s| s.library.selected().map(|e| (e.trigger.clone(), e.file.clone())))
    }
}

fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        if e.path().is_file() {
            out.insert(e.file_name().to_string_lossy().into_owned(), std::fs::read(e.path()).unwrap());
        }
    }
    out
}

fn changed(a: &BTreeMap<String, Vec<u8>>, b: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    let mut v: Vec<String> = a.keys().chain(b.keys()).filter(|k| a.get(*k) != b.get(*k)).cloned().collect();
    v.dedup();
    v
}

fn main() {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap());
    let real = home.join(".config/texnvim");
    let real_before = files(&real.join("snippets/tex"));
    let root = std::env::temp_dir().join(format!("ntex-studio-library-{}", std::process::id()));
    let config = root.join("texnvim");
    std::fs::create_dir_all(&config).unwrap();
    assert!(std::process::Command::new("rsync")
        .args(["-a", "--exclude", "/studio/target", "--exclude", "/studio/evidence", "--exclude", ".git"])
        .arg(format!("{}/", real.display()))
        .arg(format!("{}/", config.display()))
        .status()
        .unwrap()
        .success());
    // this checkout's bridge script (the copy has the main checkout's)
    std::fs::copy(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bridge/server.lua"), config.join("studio/bridge/server.lua")).unwrap();
    // a warm render cache (read from hers, written only in the copy)
    let state = root.join("state");
    std::fs::create_dir_all(&state).unwrap();
    let _ = std::process::Command::new("cp").arg("-R").arg(home.join(".local/state/ntex-studio/render-cache")).arg(&state).status();
    unsafe {
        std::env::set_var("NTEX_STUDIO_CONFIG", &config);
        std::env::set_var("TEXNVIM_SNIPPETS", config.join("snippets"));
        std::env::set_var("NTEX_STUDIO_STATE", &state);
    }
    let tex = config.join("snippets/tex");
    let mut fails: Vec<String> = Vec::new();
    let mut check = |ok: bool, what: String| {
        eprintln!("  {} {what}", if ok { "ok  " } else { "FAIL" });
        if !ok {
            fails.push(what);
        }
    };

    let mut w = Win::open();
    let ready = w.wait("bridge ready", Duration::from_secs(120), |s| s.bridge_ready() || matches!(s.bridge_state, BridgeState::Failed(_)));
    check(ready && w.read(|s| s.bridge_ready()), "the headless ntex bridge started".into());

    // -- the page ----------------------------------------------------------------
    eprintln!("library: open with ⌘4");
    w.key("cmd-4");
    let (page, n, cats, focused) = w.with(|s, win, cx| {
        (s.page, s.library.lib.entries.len(), s.library.lib.categories().len(), s.library.search.read(cx).focus.is_focused(win))
    });
    check(page == Page::Library, "⌘4 opens the Library".into());
    check(n > 200, format!("{n} snippets listed in {cats} categories"));
    check(focused, "the search field has focus".into());
    // let the thumbnails fill in for a while (rendering the rest in the background)
    w.wait("first rows rendered", Duration::from_secs(90), |s| {
        s.library.shown.iter().take(12).filter(|&&i| s.library.lib.entries[i].pieces.is_some()).count() > 0
            && s.library_thumbs_ready() >= 12
    });
    w.shot("01-all");

    // -- fuzzy search --------------------------------------------------------------
    eprintln!("library: search");
    w.query("integral");
    let first = w.selected();
    check(first.as_ref().is_some_and(|(t, _)| t == "dint"), format!("'integral' finds dint first: {first:?}"));
    w.wait("search rows rendered", Duration::from_secs(30), |s| s.library_thumbs_ready() >= 3);
    w.shot("02-search-integral");
    w.query("frac");
    let n_frac = w.read(|s| s.library.shown.len());
    w.key("down");
    w.key("down");
    let r2 = w.read(|s| s.library.row);
    w.key("up");
    let r1 = w.read(|s| s.library.row);
    check(n_frac > 3 && r2 == 2 && r1 == 1, format!("↓↓↑ in the search field moves the selection ({n_frac} matches, rows {r2} then {r1})"));
    w.key("escape");
    let cleared = w.with(|s, _, cx| s.library.search.read(cx).text.is_empty() && s.page == Page::Library);
    check(cleared, "Esc clears the search".into());
    w.with(|s, _, cx| {
        let c = s.library.lib.categories().iter().find(|(c, _)| c == "Greek").map(|(c, _)| c.clone());
        s.library.category = c;
        s.library_filter(cx);
    });
    let greek = w.read(|s| s.library.shown.iter().all(|&i| s.library.lib.entries[i].category == "Greek") && !s.library.shown.is_empty());
    check(greek, "a category chip shows only that file".into());
    w.shot("03-category-greek");
    w.with(|s, _, cx| {
        s.library.category = None;
        s.library_filter(cx);
    });

    // -- edit in place: ddt's output --------------------------------------------------
    eprintln!("library: edit ddt");
    let before = files(&tex);
    w.query("ddt");
    check(w.selected().is_some_and(|(t, f)| t == "ddt" && f == "calculus.lua"), "'ddt' selects ddt in calculus.lua".into());
    w.key("enter");
    let (page, home) = w.read(|s| (s.page, s.snippet.home.as_ref().map(|h| h.loc.file.clone())));
    check(page == Page::Snippet && home.as_deref() == Some("calculus.lua"), "Return opens it on page 1, editing calculus.lua".into());
    w.with(|s, win, cx| {
        let f = s.snippet.output.read(cx).focus.clone();
        f.focus(win, cx);
    });
    w.key("cmd-a");
    w.typ("\\frac{d}{dx}");
    let found_try = w.wait("try it", Duration::from_secs(30), |s| {
        !s.snippet.try_.running() && s.snippet.try_.result.as_ref().is_some_and(|r| r.steps.iter().any(|st| st.buffer.contains("dx") || st.buffer.contains("dt")) && s.snippet.try_.due.is_none())
            && s.snippet.try_.shown.is_some()
    });
    let tr = w.read(|s| s.snippet.try_.result.clone());
    let hid = tr.as_ref().is_some_and(|r| r.expanded && r.expanded_other.is_none() && r.steps.first().is_some_and(|st| st.buffer.contains(r"\frac{d}{dx}")));
    check(found_try && hid, format!("Try it types ddt and gets the edited version, not the old one: {tr:?}"));
    let findings = w.read(|s| s.snippet.findings.iter().map(|f| f.code.clone()).collect::<Vec<_>>());
    check(!findings.iter().any(|c| c.contains("dup") || c.contains("clash") || c.contains("shadow")), format!("no clash with its own old version: {findings:?}"));
    w.shot("04-editing-ddt");
    w.key("cmd-enter");
    w.wait("saved", Duration::from_secs(10), |s| s.snippet.saving.is_none() && s.page == Page::Library);
    let after = files(&tex);
    let ch = changed(&before, &after);
    check(ch == vec!["calculus.lua".to_string()], format!("only calculus.lua changed: {ch:?}"));
    let old = String::from_utf8(before["calculus.lua"].clone()).unwrap();
    let new = String::from_utf8(after["calculus.lua"].clone()).unwrap();
    let diff: Vec<(&str, &str)> = old.lines().zip(new.lines()).filter(|(a, b)| a != b).collect();
    eprintln!("  diff: {diff:?}");
    let col = |s: &str| s.find("\"default\"");
    check(
        diff.len() == 1 && old.lines().count() == new.lines().count() && diff[0].1.contains(r"[[\frac{d}{dx}]]") && col(diff[0].0) == col(diff[0].1),
        "one line changed, the output literal only, origin column kept".into(),
    );
    let sel = w.selected();
    check(sel.as_ref().is_some_and(|(t, _)| t == "ddt"), format!("back on the Library, on ddt: {sel:?}"));
    let toast = w.read(|s| s.toast.as_ref().map(|t| t.text.to_string()));
    check(toast.as_deref().is_some_and(|t| t.contains("Updated ‘ddt’ in Calculus")), format!("toast: {toast:?}"));
    w.shot("05-after-save");
    w.with(|s, win, cx| s.library.focus.clone().focus(win, cx));
    w.key("cmd-z");
    check(files(&tex) == before, "⌘Z puts calculus.lua back byte-for-byte".into());

    // -- a hand-written entry, edited as Lua --------------------------------------------
    eprintln!("library: pmat as Lua");
    w.tick(1100);
    let before = files(&tex);
    w.query("pmat");
    check(w.selected().is_some_and(|(t, f)| t == "pmat" && f == "environments.lua"), "'pmat' selects the env() entry".into());
    w.key("enter");
    let raw = w.with(|s, _, cx| s.snippet.raw.read(cx).text.clone());
    check(raw.starts_with("env(\"pmat\""), format!("opens as its Lua: {raw}"));
    w.with(|s, _, cx| {
        let t = s.snippet.raw.read(cx).text.replace("\"pmat\"", "\"pmatx\"");
        s.snippet.raw.update(cx, |f, cx| f.set_text(t, cx));
        s.snippet_changed(cx);
    });
    w.key("cmd-enter");
    w.wait("saved", Duration::from_secs(10), |s| s.snippet.saving.is_none() && s.page == Page::Library);
    let after = files(&tex);
    let ch = changed(&before, &after);
    let envs = String::from_utf8(after["environments.lua"].clone()).unwrap();
    check(ch == vec!["environments.lua".to_string()] && envs.contains("env(\"pmatx\""), format!("environments.lua now has pmatx: {ch:?}"));

    // -- a broken edit is refused by the check, nothing written ------------------------------
    eprintln!("library: a broken edit");
    w.tick(1100);
    let before = files(&tex);
    w.query("pmatx");
    w.key("enter");
    w.with(|s, _, cx| {
        s.snippet.raw.update(cx, |f, cx| f.set_text("env(\"pmatx\", nosuchname.field, \"default\"),", cx));
        s.snippet_changed(cx);
    });
    w.key("cmd-enter");
    w.wait("checked", Duration::from_secs(10), |s| s.snippet.saving.is_none());
    let err = w.read(|s| s.snippet.save_error.clone());
    check(err.is_some() && files(&tex) == before, format!("a file that would not load is not written: {err:?}"));
    w.shot("06-broken-edit");
    let st = w.with(|s, win, cx| (s.page, s.snippet.home.is_some(), s.focus.is_focused(win), s.snippet.raw.read(cx).focus.is_focused(win), s.snippet.output.read(cx).focus.is_focused(win)));
    eprintln!("  before esc: {st:?}");
    w.key("escape");
    let st = w.with(|s, win, cx| (s.page, s.snippet.home.is_some(), s.focus.is_focused(win), s.snippet.raw.read(cx).focus.is_focused(win), s.hides));
    eprintln!("  after esc 1: {st:?}");
    w.key("escape");
    let st = w.with(|s, win, _| (s.page, s.snippet.home.is_some(), s.focus.is_focused(win), s.hides));
    eprintln!("  after esc 2: {st:?}");
    let back = w.read(|s| s.page == Page::Library && s.snippet.home.is_none());
    check(back, "Esc, Esc goes back to the Library without saving".into());
    check(files(&tex) == before, "and nothing was written".into());

    // -- her real files were never touched --------------------------------------------------
    check(files(&real.join("snippets/tex")) == real_before, "her real snippet files are byte-for-byte unchanged".into());

    w.with(|s, _, _| s.stop_bridge());
    drop(w);
    if fails.is_empty() {
        let _ = std::fs::remove_dir_all(&root);
        eprintln!("library: all passed");
    } else {
        eprintln!("library: {} failed (temp copy kept at {}):\n  {}", fails.len(), root.display(), fails.join("\n  "));
        std::process::exit(1);
    }
}
