//! The Terminal page offscreen: ⌘7 opens it on a temp copy of her Ghostty
//! and cmux configs (never the real ones; nothing is reloaded); clicks on the
//! preview select the option a cell's color comes from; changes are written
//! to ntex-look.ghostty after the debounce and pass `ghostty +validate-config`;
//! "Connect to Ghostty" adds the include; "Applies to" edits cmux's file;
//! themes, padding drags, palette swaps and saved looks work. PNGs land in
//! studio/evidence/terminal/. `cargo test --test terminal`
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    AppContext as _, HeadlessAppContext, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput, Pixels,
    Point, WindowHandle, point, px, size,
};
use ntex_studio::core::ghostty::screen::Target;
use ntex_studio::core::ghostty::{self as g, AppliesTo};
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
    fn event(&mut self, e: PlatformInput) {
        self.cx.update_window(self.window.into(), |_, window, cx| window.dispatch_event(e, cx)).unwrap();
        self.draw();
    }
    fn down(&mut self, at: Point<Pixels>) {
        self.event(PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: 1, first_mouse: false, pen: None }));
    }
    fn move_to(&mut self, at: Point<Pixels>) {
        self.event(PlatformInput::MouseMove(MouseMoveEvent { position: at, pressed_button: Some(MouseButton::Left), modifiers: Modifiers::default(), pen: None }));
    }
    fn up(&mut self, at: Point<Pixels>) {
        self.event(PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: 1, pen: None }));
    }
    fn click(&mut self, at: Point<Pixels>) {
        self.down(at);
        self.up(at);
    }
    /// Run the page's pump until `done` (or `limit`): loads, captures, writes.
    fn pump_until(&mut self, limit: Duration, done: impl Fn(&Studio) -> bool) -> bool {
        let t0 = Instant::now();
        loop {
            self.window.update(&mut self.cx, |s, _, cx| {
                let now = s.now(cx);
                s.pump_terminal(now, cx);
            }).unwrap();
            self.draw();
            if self.window.update(&mut self.cx, |s, _, _| done(s)).unwrap() {
                return true;
            }
            if t0.elapsed() > limit {
                return false;
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    }
    fn shot(&mut self, name: &str) {
        self.draw();
        let img = self.cx.capture_screenshot(self.window.into()).unwrap();
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("evidence/terminal");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(format!("{name}.png"));
        img.save(&p).unwrap();
        eprintln!("  wrote {}", p.display());
    }
    fn sel(&mut self) -> Target {
        self.window.update(&mut self.cx, |s, _, _| s.terminal.selected.clone()).unwrap()
    }
    /// The window point of cell (row, col) of a pane's grid.
    fn cell(&mut self, shell: bool, row: usize, col: usize) -> Point<Pixels> {
        self.window
            .update(&mut self.cx, |s, w, _| {
                let geo = s.terminal.geometry.clone();
                let b = if shell { geo.shell.get() } else { geo.ntex.get() }.expect("grid drawn");
                let look = s.terminal.shown();
                let f = ntex_studio_preview::faces(&look, &s.terminal.defaults, &s.terminal.fonts);
                let m = ntex_studio_preview::metrics(w, &f, &look, &s.terminal.defaults);
                // the shell shows its last rows: `row` counts from its first line
                let row = if shell {
                    let fit = (f32::from(b.size.height) / m.ch).round() as usize;
                    row - g::screen::shell(80).rows.saturating_sub(fit)
                } else {
                    row
                };
                point(b.origin.x + px((col as f32 + 0.5) * m.cw), b.origin.y + px((row as f32 + 0.4) * m.ch))
            })
            .unwrap()
    }
}

use ntex_studio::ui::screens::terminal::preview as ntex_studio_preview;

/// Her Ghostty config as of 2026-10-20, without the include line.
const MAIN: &str = "font-family = JuliaMono\nbackground = #ffffff\nforeground = #000000\ncursor-color = #007acc\ncursor-text = #ffffff\nselection-background = #bfdbfe\nselection-foreground = #000000\npalette = 0=#000000\npalette = 1=#aa3731\npalette = 2=#448c27\npalette = 3=#cb9000\npalette = 4=#325cc0\npalette = 5=#7a3e9d\npalette = 6=#0083b2\npalette = 7=#bbbbbb\npalette = 8=#777777\npalette = 9=#f05050\npalette = 10=#60cb00\npalette = 11=#e09400\npalette = 12=#007acc\npalette = 13=#c040c0\npalette = 14=#00a0c0\npalette = 15=#ffffff\nwindow-padding-x = 4\nwindow-padding-y = 12\nwindow-padding-balance = true\nadjust-cell-height = 10%\nunfocused-split-opacity = 1\nmacos-option-as-alt = true\n";
const CMUX: &str = "# cmux themes start\ntheme = light:GitLab Light,dark:Breadog\n# cmux themes end\nsidebar-font-size = 20\n\nbackground = #ffffff\nforeground = #000000\n";

fn main() {
    let root = std::env::temp_dir().join(format!("ntex-studio-terminal-{}", std::process::id()));
    let config = root.join("texnvim");
    let gdir = root.join("ghostty");
    std::fs::create_dir_all(config.join("snippets/tex")).unwrap();
    std::fs::create_dir_all(config.join("templates/tex")).unwrap();
    std::fs::create_dir_all(&gdir).unwrap();
    std::fs::write(gdir.join("config"), MAIN).unwrap();
    std::fs::write(gdir.join("app.ghostty"), "theme = Breadog\n").unwrap();
    std::fs::write(gdir.join("cmux.ghostty"), CMUX).unwrap();
    // SAFETY: single-threaded here, before any GPUI thread starts.
    unsafe {
        std::env::set_var("NTEX_STUDIO_CONFIG", &config);
        std::env::set_var("TEXNVIM_SNIPPETS", config.join("snippets"));
        std::env::set_var("NTEX_STUDIO_STATE", root.join("state"));
        std::env::set_var("NTEX_STUDIO_GHOSTTY_DIR", &gdir);
        std::env::remove_var("NTEX_STUDIO_GHOSTTY_LIVE");
    }
    let look_file = gdir.join("ntex-look.ghostty");
    let mut failures: Vec<String> = Vec::new();
    let mut check = |ok: bool, what: String| {
        if !ok {
            eprintln!("FAIL {what}");
            failures.push(what);
        }
    };

    let text = gpui_platform::current_platform(true).text_system();
    let mut cx = HeadlessAppContext::with_platform(text, Arc::new(()), gpui_platform::current_headless_renderer);
    cx.update(ntex_studio::ui::app::init);
    let window = cx
        .open_window(size(px(860.), px(3600.)), |window, cx| cx.new(|cx| Studio::new(StudioOptions { quiet: true, bridge: false }, window, cx)))
        .unwrap();
    let mut w = Win { cx, window };
    w.draw();
    w.key("cmd-7");
    check(w.window.update(&mut w.cx, |s, _, _| s.page == Page::Terminal).unwrap(), "⌘7 did not open the Terminal page".into());
    let loaded = w.pump_until(Duration::from_secs(20), |s| s.terminal.loaded);
    check(loaded, "Ghostty's options and themes never loaded".into());
    // a real ntex screen at the preview's size (headless nvim, her config)
    let captured = w.pump_until(Duration::from_secs(20), |s| s.terminal.captured != (0, 0) && s.terminal.ntex.cols == s.terminal.captured.0);
    eprintln!("  ntex captured: {captured}");

    let (bg, cell_h, themes, today) = w
        .window
        .update(&mut w.cx, |s, _, _| {
            let d = s.terminal.defaults.clone();
            (s.terminal.state.look.one("background", &d), s.terminal.state.look.one("adjust-cell-height", &d), s.terminal.themes.len(), s.terminal.saved.iter().any(|x| x.name == "Ghostty today" && x.protected))
        })
        .unwrap();
    check(bg == "#ffffff" && cell_h == "10%", format!("her look did not load: bg {bg}, cell height {cell_h}"));
    check(themes > 400, format!("only {themes} themes"));
    check(today, "“Ghostty today” was not saved as a protected look".into());
    check(!look_file.exists(), "opening the page wrote the look file".into());
    w.shot("01-terminal-her-look");

    // clicks on the preview select what draws the cell
    let p = w.cell(true, 3, 1); // " M" of git status: palette 1
    w.click(p);
    check(w.sel() == Target::key("palette:1"), format!("clicking red git text selected {:?}", w.sel()));
    let p = w.cell(true, 1, 24); // the selected file name in ls
    w.click(p);
    check(matches!(w.sel(), Target::Key(ref k) if k.starts_with("selection-")), format!("clicking the selection selected {:?}", w.sel()));
    let p = w.cell(false, 3, 6); // ntex's text: a fixed color
    w.click(p);
    check(matches!(w.sel(), Target::Fixed(_)), format!("clicking ntex text selected {:?}", w.sel()));
    w.shot("02-terminal-ntex-color-selected");

    // recolor palette 1: written after the debounce, validated by Ghostty
    w.window.update(&mut w.cx, |s, _, cx| {
        s.term_select(Target::key("palette:1"), cx);
        s.term_set_color("palette:1", 0xc41982, cx);
    }).unwrap();
    let wrote = w.pump_until(Duration::from_secs(10), |s| s.terminal.status.is_some());
    let file = std::fs::read_to_string(&look_file).unwrap_or_default();
    check(wrote && file.contains("palette = 1=#c41982"), format!("palette 1 not written:\n{file}"));
    check(file.contains("adjust-cell-height = 10%") && file.contains("theme = \"\""), format!("pinned keys missing:\n{file}"));
    let status = w.window.update(&mut w.cx, |s, _, _| s.terminal.status.clone()).unwrap();
    check(status.as_ref().is_some_and(|(ok, _)| *ok), format!("write status: {status:?}"));
    check(std::fs::read_to_string(gdir.join("cmux.ghostty")).unwrap() == CMUX, "cmux's file changed before Ghostty was connected".into());
    w.shot("03-terminal-palette1-edited");

    // connect: the include line, with a backup
    w.window.update(&mut w.cx, |s, _, cx| s.term_connect(cx)).unwrap();
    let main = std::fs::read_to_string(gdir.join("config")).unwrap();
    check(main.starts_with(MAIN) && main.trim_end().ends_with("config-file = ?ntex-look.ghostty"), format!("include not added:\n{main}"));
    let backups = std::fs::read_dir(&gdir).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with("config.bak-")).count();
    check(backups == 1, format!("{backups} backups of the main config"));

    // a theme as a base; cmux's own bg/fg follow ("Ghostty + cmux")
    w.window.update(&mut w.cx, |s, _, cx| s.term_theme("Breadog", cx)).unwrap();
    w.pump_until(Duration::from_secs(10), |s| s.terminal.idle() && std::fs::read_to_string(&s.terminal.places.look_file()).unwrap_or_default().contains("#f1ebe6"));
    let cm = std::fs::read_to_string(gdir.join("cmux.ghostty")).unwrap();
    check(cm.contains("background = #f1ebe6") && cm.contains("foreground = #362c24") && cm.contains("sidebar-font-size = 20"), format!("cmux not synced:\n{cm}"));
    w.shot("04-terminal-breadog");

    // Ghostty only: cmux keeps Breadog in its block while Ghostty changes
    w.window.update(&mut w.cx, |s, _, cx| {
        s.term_applies(AppliesTo::GhosttyOnly, cx);
        s.term_set_color("background", 0xfafaf5, cx);
    }).unwrap();
    w.pump_until(Duration::from_secs(10), |s| s.terminal.idle() && std::fs::read_to_string(&s.terminal.places.look_file()).unwrap_or_default().contains("#fafaf5"));
    let cm = std::fs::read_to_string(gdir.join("cmux.ghostty")).unwrap();
    let block = g::cmux_block(&cm).unwrap_or_default();
    check(block.contains("background = #f1ebe6") && !block.contains("#fafaf5"), format!("cmux block wrong:\n{cm}"));
    // back to both: the block goes
    w.window.update(&mut w.cx, |s, _, cx| s.term_applies(AppliesTo::Both, cx)).unwrap();
    w.pump_until(Duration::from_secs(10), |s| s.terminal.idle() && !std::fs::read_to_string(&s.terminal.places.cmux).unwrap_or_default().contains(g::CMUX_START));
    let cm = std::fs::read_to_string(gdir.join("cmux.ghostty")).unwrap();
    check(!cm.contains(g::CMUX_START) && cm.contains("background = #fafaf5"), format!("back to both:\n{cm}"));

    // drag the left padding of the ntex pane 10 px right
    let (x0, y0) = w
        .window
        .update(&mut w.cx, |s, _, _| {
            let b = s.terminal.geometry.ntex.get().unwrap();
            (b.origin.x - px(2.), b.origin.y + px(40.))
        })
        .unwrap();
    w.down(point(x0, y0));
    w.move_to(point(x0 + px(5.), y0));
    w.move_to(point(x0 + px(10.), y0));
    w.up(point(x0 + px(10.), y0));
    let pad = w.window.update(&mut w.cx, |s, _, _| s.terminal.state.look.one("window-padding-x", &s.terminal.defaults)).unwrap();
    check(pad == "14,4", format!("dragging the left padding gave {pad}"));

    // drag palette 2 onto 4: they swap
    let (c2, c4) = w.window.update(&mut w.cx, |s, _, _| {
        let c = g::Colors::of(&s.terminal.state.look, &s.terminal.defaults);
        (c.palette[2], c.palette[4])
    }).unwrap();
    w.window.update(&mut w.cx, |s, _, cx| s.term_swap(2, 4, cx)).unwrap();
    let (n2, n4) = w.window.update(&mut w.cx, |s, _, _| {
        let c = g::Colors::of(&s.terminal.state.look, &s.terminal.defaults);
        (c.palette[2], c.palette[4])
    }).unwrap();
    check(n2 == c4 && n4 == c2, "palette swap".into());

    // cursor as a bar, the drawer open on Cursor and Window, an option hovered
    w.window.update(&mut w.cx, |s, _, cx| {
        s.set_term(|l, d| l.put1("cursor-style", "bar", d), cx);
        s.term_select(Target::key("cursor-color"), cx);
        s.terminal.drawer_open = true;
        s.terminal.groups_open.insert("Cursor");
        s.terminal.groups_open.insert("Window");
        s.terminal.hover = Some("window-padding-x".into());
    }).unwrap();
    w.shot("05-terminal-cursor-drawer");
    w.window.update(&mut w.cx, |s, _, _| s.terminal.hover = None).unwrap();

    // save it, go back to Ghostty today (exactly the look it had), press the saved one
    w.window.update(&mut w.cx, |s, _, cx| {
        s.terminal.name.update(cx, |f, cx| f.set_text("Breadog bar", cx));
        s.term_save_as(cx);
        s.term_restore("Ghostty today", cx);
    }).unwrap();
    let back = w.window.update(&mut w.cx, |s, _, _| {
        let today = s.terminal.saved.iter().find(|x| x.name == "Ghostty today").unwrap().look.clone();
        s.terminal.state.look == today
    }).unwrap();
    check(back, "Ghostty today did not restore".into());
    w.pump_until(Duration::from_secs(10), |s| s.terminal.idle() && std::fs::read_to_string(&s.terminal.places.look_file()).unwrap_or_default().contains("background = #ffffff"));
    let file = std::fs::read_to_string(&look_file).unwrap_or_default();
    check(file.contains("palette = 1=#aa3731") && file.contains("window-padding-x = 4"), format!("Ghostty today's file:\n{file}"));
    let looks = std::fs::read_to_string(config.join("looks.json")).unwrap_or_default();
    check(looks.contains("Breadog bar"), "saved look not in looks.json".into());
    w.shot("06-terminal-back-to-today");

    // ⌘-scroll is the font size; the Ghostty default preset; a dark theme
    w.window.update(&mut w.cx, |s, _, cx| {
        s.term_restore("Ghostty default", cx);
    }).unwrap();
    w.shot("07-terminal-ghostty-default");
    w.window.update(&mut w.cx, |s, _, cx| {
        s.term_theme("Catppuccin Mocha", cx);
        s.set_term(|l, d| { l.put1("background-opacity", "0.85", d); l.put1("macos-titlebar-style", "tabs", d); l.put1("unfocused-split-opacity", "0.7", d); }, cx);
        s.term_select(Target::key("background"), cx);
    }).unwrap();
    w.pump_until(Duration::from_secs(10), |s| s.terminal.idle());
    let file = std::fs::read_to_string(&look_file).unwrap_or_default();
    let valid = w.window.update(&mut w.cx, |s, _, _| s.terminal.places.validate(None)).unwrap();
    check(valid.is_ok(), format!("ghostty +validate-config: {valid:?}\n{file}"));
    w.shot("08-terminal-dark-tabs-translucent");

    let _ = std::fs::remove_dir_all(&root);
    if failures.is_empty() {
        eprintln!("terminal: ok");
    } else {
        for f in &failures {
            eprintln!("FAIL {f}");
        }
        std::process::exit(1);
    }
}
