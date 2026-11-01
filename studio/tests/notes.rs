//! The Notes page offscreen: ⌘6 opens it; the typeface, box style and colors
//! write notes-look.json and the theme's two generated files (in a temp
//! config, never the real one); the defaults write none. PNGs land in
//! studio/evidence/notes/. `cargo test --test notes`
use std::path::PathBuf;
use std::sync::Arc;

use gpui::{AppContext as _, HeadlessAppContext, Keystroke, WindowHandle, px, size};
use ntex_studio::ui::app::{Page, Studio, StudioOptions};

struct Win {
    cx: HeadlessAppContext,
    window: WindowHandle<Studio>,
}

impl Win {
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
    fn shot(&mut self, name: &str) {
        self.draw();
        let img = self.cx.capture_screenshot(self.window.into()).unwrap();
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("evidence/notes");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(format!("{name}.png"));
        img.save(&p).unwrap();
        eprintln!("  wrote {}", p.display());
    }
}

fn main() {
    let root = std::env::temp_dir().join(format!("ntex-studio-notes-{}", std::process::id()));
    let config = root.join("texnvim");
    let tpl = config.join("templates/tex");
    std::fs::create_dir_all(config.join("snippets/tex")).unwrap();
    std::fs::create_dir_all(&tpl).unwrap();
    // SAFETY: single-threaded here, before any GPUI thread starts.
    unsafe {
        std::env::set_var("NTEX_STUDIO_CONFIG", &config);
        std::env::set_var("TEXNVIM_SNIPPETS", config.join("snippets"));
        std::env::set_var("NTEX_STUDIO_STATE", root.join("state"));
    }
    let fonts = tpl.join("ntex-notes-fonts.tex");
    let lookf = tpl.join("ntex-notes-look.tex");
    let mut failures: Vec<String> = Vec::new();

    let text = gpui_platform::current_platform(true).text_system();
    let mut cx = HeadlessAppContext::with_platform(text, Arc::new(()), gpui_platform::current_headless_renderer);
    cx.update(ntex_studio::ui::app::init);
    let window = cx
        .open_window(size(px(860.), px(1700.)), |window, cx| cx.new(|cx| Studio::new(StudioOptions { quiet: true, bridge: false }, window, cx)))
        .unwrap();
    let mut w = Win { cx, window };
    w.draw();
    w.key("cmd-6");
    if !w.window.update(&mut w.cx, |s, _, _| s.page == Page::Notes).unwrap() {
        failures.push("⌘6 did not open the Notes page".into());
    }
    w.shot("01-notes-defaults");
    if fonts.exists() || lookf.exists() {
        failures.push("opening the page wrote theme files".into());
    }

    w.window.update(&mut w.cx, |s, _, cx| {
        s.set_notes(|n| { n.set("font", "palatino"); n.set("corners", "rounded"); n.set("frame", "box"); n.set_color("ntexBlueRule", 0xc41982); n.bar = 15; }, cx);
    }).unwrap();
    let f = std::fs::read_to_string(&fonts).unwrap_or_default();
    let l = std::fs::read_to_string(&lookf).unwrap_or_default();
    if !f.contains("newpxtext") || !l.contains("C41982") || !l.contains("arc=4pt") {
        failures.push(format!("theme files not written:\n{f}\n{l}"));
    }
    w.shot("02-notes-edited");

    // save it, go back to the theme default (files go), press it again
    w.window.update(&mut w.cx, |s, _, cx| {
        s.notes_page.name.update(cx, |f, cx| f.set_text("Pink rounded", cx));
        s.notes_save_as(cx);
        s.notes_restore("Theme default (Charter)", cx);
    }).unwrap();
    if fonts.exists() || lookf.exists() {
        failures.push("Theme default left theme files behind".into());
    }
    w.window.update(&mut w.cx, |s, _, cx| s.notes_restore("Pink rounded", cx)).unwrap();
    if !std::fs::read_to_string(&lookf).unwrap_or_default().contains("C41982") {
        failures.push("pressing the saved notes look did not bring it back".into());
    }

    let _ = std::fs::remove_dir_all(&root);
    if failures.is_empty() {
        eprintln!("notes: ok");
    } else {
        for f in &failures {
            eprintln!("FAIL {f}");
        }
        std::process::exit(1);
    }
}
