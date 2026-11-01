//! The Look page offscreen: ⌘5 opens it, ←/→ on a setting writes look.json
//! (in a temp config, never the real one), the sample follows the palette.
//! PNGs land in studio/evidence/look/. `cargo test --test look`
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
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("evidence/look");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(format!("{name}.png"));
        img.save(&p).unwrap();
        eprintln!("  wrote {}", p.display());
    }
}

fn main() {
    let root = std::env::temp_dir().join(format!("ntex-studio-look-{}", std::process::id()));
    let config = root.join("texnvim");
    std::fs::create_dir_all(config.join("snippets/tex")).unwrap();
    std::fs::create_dir_all(config.join("templates/tex")).unwrap();
    // SAFETY: single-threaded here, before any GPUI thread starts.
    unsafe {
        std::env::set_var("NTEX_STUDIO_CONFIG", &config);
        std::env::set_var("TEXNVIM_SNIPPETS", config.join("snippets"));
        std::env::set_var("NTEX_STUDIO_STATE", root.join("state"));
    }
    let look = config.join("look.json");
    let mut failures: Vec<String> = Vec::new();

    let text = gpui_platform::current_platform(true).text_system();
    let mut cx = HeadlessAppContext::with_platform(text, Arc::new(()), gpui_platform::current_headless_renderer);
    cx.update(ntex_studio::ui::app::init);
    let window = cx
        .open_window(size(px(860.), px(1900.)), |window, cx| cx.new(|cx| Studio::new(StudioOptions { quiet: true, bridge: false }, window, cx)))
        .unwrap();
    let mut w = Win { cx, window };
    w.draw();

    w.key("cmd-5");
    let on_look = w.window.update(&mut w.cx, |s, _, _| s.page == Page::Look).unwrap();
    if !on_look {
        failures.push("⌘5 did not open the Look page".into());
    }
    w.shot("01-look-defaults");
    if look.exists() {
        failures.push("opening the page wrote look.json".into());
    }

    // Tab until → changes the palette (the palette radio), then → again: Code Dark
    for _ in 0..20 {
        w.key("tab");
        w.key("right");
        if std::fs::read_to_string(&look).unwrap_or_default().contains("\"palette\": \"code\"") {
            break;
        }
    }
    w.key("right");
    let written = std::fs::read_to_string(&look).unwrap_or_default();
    eprintln!("  look.json: {}", written.replace('\n', " "));
    if !written.contains("\"palette\": \"code-dark\"") {
        failures.push(format!("→ → on Palette did not write code-dark: {written}"));
    }
    w.shot("02-look-code-dark");

    // Tab to the Standard row, → : the first standard theme
    w.key("tab");
    w.key("right");
    let written = std::fs::read_to_string(&look).unwrap_or_default();
    if !written.contains("\"palette\": \"vscode-light\"") {
        failures.push(format!("→ on Standard did not write vscode-light: {written}"));
    }
    w.window.update(&mut w.cx, |s, _, cx| s.set_look(|l| { l.set("palette", "gruvbox-dark"); }, cx)).unwrap();
    w.shot("02b-look-gruvbox-dark");
    w.window.update(&mut w.cx, |s, _, cx| s.set_look(|l| { l.set("palette", "solarized-light"); }, cx)).unwrap();
    w.shot("02c-look-solarized-light");
    w.window.update(&mut w.cx, |s, _, cx| s.set_look(|l| { l.set("palette", "newpaper-light"); }, cx)).unwrap();
    w.shot("02d-look-newpaper-light");

    // colors: pick Math, type a hex, Return → an edit on this palette only
    w.window.update(&mut w.cx, |s, _, cx| s.set_look(|l| { l.set("palette", "slate"); }, cx)).unwrap();
    w.window
        .update(&mut w.cx, |s, window, cx| {
            s.look_page.role = 2; // math
            s.look_sync_hex(cx);
            let f = s.look_page.hex.read(cx).focus.clone();
            f.focus(window, cx);
            s.look_page.hex.update(cx, |f, cx| f.select_all(cx));
        })
        .unwrap();
    w.draw();
    // (the # is optional; synthetic shift-3 types a 3 here)
    for k in ["c", "c", "2", "2", "6", "6", "enter"] {
        w.key(k);
    }
    let field = w.window.update(&mut w.cx, |s, _, cx| s.look_page.hex.read(cx).text.clone()).unwrap();
    eprintln!("  hex field: {field}");
    let written = std::fs::read_to_string(&look).unwrap_or_default();
    eprintln!("  look.json: {}", written.replace('\n', " "));
    if !written.contains(r##""slate":{"math":"#cc2266"}"##) && !written.contains("\"math\": \"#cc2266\"") {
        failures.push(format!("hex entry did not write the math color: {written}"));
    }
    // → on the S slider (focused by Tab from the hex field? not guaranteed): drive it directly
    w.window.update(&mut w.cx, |s, _, cx| s.set_look(|l| l.set_color("Definition", 0x1f8a70), cx)).unwrap();
    w.shot("04-look-colors-edited");
    w.window.update(&mut w.cx, |s, _, cx| s.set_look(|l| l.reset_colors(), cx)).unwrap();
    if std::fs::read_to_string(&look).unwrap_or_default().contains("colors") {
        failures.push("Reset all left color edits in look.json".into());
    }

    // saved looks: save the current one, change things, press it to go back
    w.window.update(&mut w.cx, |s, _, cx| {
        s.set_look(|l| { l.set("palette", "alabaster"); l.set_color("math", 0xc41982); }, cx);
        s.look_page.name.update(cx, |f, cx| f.set_text("Test look", cx));
        s.look_save_as(cx);
    }).unwrap();
    let saved = std::fs::read_to_string(config.join("looks.json")).unwrap_or_default();
    if !saved.contains("Test look") || !saved.contains("#c41982") {
        failures.push(format!("Save current as did not write looks.json: {saved}"));
    }
    w.window.update(&mut w.cx, |s, _, cx| s.look_restore("Slate (default)", cx)).unwrap();
    let now = w.window.update(&mut w.cx, |s, _, _| (s.look.palette.clone(), s.look.colors.get("alabaster").cloned())).unwrap();
    if now.0 != "slate" {
        failures.push(format!("pressing Slate (default) did not go back to Slate: {now:?}"));
    }
    w.shot("05-look-saved-looks");
    w.window.update(&mut w.cx, |s, _, cx| s.look_restore("Test look", cx)).unwrap();
    let back = w.window.update(&mut w.cx, |s, _, _| (s.look.palette.clone(), s.look.color("math"))).unwrap();
    if back != ("alabaster".to_string(), 0xc41982) {
        failures.push(format!("pressing Test look did not restore it: {back:?}"));
    }
    w.window.update(&mut w.cx, |s, _, cx| s.look_delete(cx)).unwrap();
    if std::fs::read_to_string(config.join("looks.json")).unwrap_or_default().contains("Test look") {
        failures.push("Delete left the look in looks.json".into());
    }
    w.window.update(&mut w.cx, |s, _, cx| { s.look_restore("Slate (default)", cx); s.look_delete(cx); }).unwrap();

    // structure: a glyph and a shade, written under "structure"; reset drops it
    w.window.update(&mut w.cx, |s, _, cx| {
        s.set_look(|l| { l.structure.set("rule_glyph", "━"); l.structure.set("bar_glyph", "┃"); l.structure.set_number("begin_shade", 0); }, cx);
    }).unwrap();
    let written = std::fs::read_to_string(&look).unwrap_or_default();
    if !written.contains("\"rule_glyph\": \"━\"") || !written.contains("\"begin_shade\": 0") {
        failures.push(format!("structure settings not written: {written}"));
    }
    w.shot("06-look-structure");
    w.window.update(&mut w.cx, |s, _, cx| s.set_look(|l| l.structure = Default::default(), cx)).unwrap();
    if std::fs::read_to_string(&look).unwrap_or_default().contains("structure") {
        failures.push("Reset structure left settings in look.json".into());
    }

    // a change made elsewhere (:NtexLook) shows when the page is opened again
    std::fs::write(&look, r#"{"palette":"code","sections":"band","kinds":"color","lines":"name","guides":false,"ink":"all"}"#).unwrap();
    w.key("cmd-1");
    w.key("cmd-5");
    let palette = w.window.update(&mut w.cx, |s, _, _| s.look.palette.clone()).unwrap();
    if palette != "code" {
        failures.push(format!("reopening the page did not read look.json: {palette}"));
    }
    w.shot("03-look-code-band");

    let _ = std::fs::remove_dir_all(&root);
    if failures.is_empty() {
        eprintln!("look: ok");
    } else {
        for f in &failures {
            eprintln!("FAIL {f}");
        }
        std::process::exit(1);
    }
}
