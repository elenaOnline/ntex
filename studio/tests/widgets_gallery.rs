//! WIDGETS: every widget in every state, rendered offscreen (HeadlessAppContext
//! + the Metal headless renderer, real fonts), saved to evidence/widgets/,
//! plus the token editor and trigger field driven with synthetic keys.
//!
//! `harness = false`: AppKit wants the text system on the main thread, which
//! libtest's workers are not. Nothing opens a window on screen.
//!
//!     cargo test --test widgets_gallery
//!
//! `NTEX_GALLERY_ONLY=keys` skips the renders.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, AppContext as _, ClipboardItem, Context, Entity, FocusHandle, HeadlessAppContext, IntoElement,
    KeyBinding, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement,
    PlatformInput, Point, Render, SharedString, Styled, Window, WindowHandle, actions, div, point, prelude::*, px,
    size,
};
use ntex_studio::model::{Piece, Rgb, Severity, Tones};
use ntex_studio::ui::widgets::{
    self,
    boxes::{BoxKind, Footnotes, LogLine, MarginNote, RunningHead, Section, TBox, Table, caps_label},
    code_view::CodeView,
    controls::{Button, Checkbox, Disclosure, HueSlider, Keycap, Radio, Swatches, Visual},
    text_field::TextField,
    theme::{ActiveTheme as _, Theme},
    token_editor::{TokenEditor, TokenEditorEvent, TokenSet, TokenToolbar},
    trigger_field::{TriggerEvent, TriggerField},
};

actions!(gallery_app, [AppTab, AppF, AppSpace, AppSave, AppNext]);

type Scene = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

struct Stage {
    scene: Scene,
    log: Rc<RefCell<Vec<String>>>,
}

impl Render for Stage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.theme().clone();
        let log = self.log.clone();
        let (l1, l2, l3, l4) = (log.clone(), log.clone(), log.clone(), log.clone());
        div()
            .key_context("GalleryApp")
            .size_full()
            .bg(t.paper)
            .text_color(t.ink)
            .font(t.serif())
            .text_size(t.body_size)
            .on_action(move |_: &AppTab, _, _| log.borrow_mut().push("app:tab".into()))
            .on_action(move |_: &AppF, _, _| l1.borrow_mut().push("app:f".into()))
            .on_action(move |_: &AppSpace, _, _| l2.borrow_mut().push("app:space".into()))
            .on_action(move |_: &AppSave, _, _| l3.borrow_mut().push("app:save".into()))
            .on_action(move |_: &AppNext, _, _| l4.borrow_mut().push("app:next".into()))
            .child((self.scene)(window, cx))
    }
}

struct Win {
    cx: HeadlessAppContext,
    window: WindowHandle<Stage>,
    log: Rc<RefCell<Vec<String>>>,
}

fn text_system() -> Arc<dyn gpui::PlatformTextSystem> {
    gpui_platform::current_platform(true).text_system()
}

fn app() -> HeadlessAppContext {
    let mut cx = HeadlessAppContext::with_platform(text_system(), Arc::new(()), gpui_platform::current_headless_renderer);
    cx.update(|cx| {
        widgets::init(cx);
        // app bindings the fields must beat (plain keys, Tab) or yield to (⌘↵)
        cx.bind_keys([
            KeyBinding::new("tab", AppTab, Some("GalleryApp")),
            KeyBinding::new("f", AppF, Some("GalleryApp")),
            KeyBinding::new("space", AppSpace, Some("GalleryApp")),
            KeyBinding::new("cmd-enter", AppSave, Some("GalleryApp")),
            KeyBinding::new("cmd-1", AppNext, Some("GalleryApp")),
        ]);
    });
    cx
}

impl Win {
    fn open(mut cx: HeadlessAppContext, w: f32, h: f32, scene: Scene) -> Win {
        let log: Rc<RefCell<Vec<String>>> = Rc::default();
        let l = log.clone();
        let window = cx.open_window(size(px(w), px(h)), move |_, cx| cx.new(|_| Stage { scene, log: l })).unwrap();
        let mut win = Win { cx, window, log };
        win.draw();
        win.draw();
        win
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
    fn typ(&mut self, s: &str) {
        for c in s.chars() {
            let k = match c {
                ' ' => "space".to_string(),
                c if c.is_ascii_uppercase() => format!("shift-{}", c.to_ascii_lowercase()),
                c => c.to_string(),
            };
            self.key(&k);
        }
    }
    fn focus(&mut self, f: &FocusHandle) {
        let f = f.clone();
        self.cx.update_window(self.window.into(), |_, window, cx| window.focus(&f, cx)).unwrap();
        self.draw();
    }
    fn input(&mut self, e: PlatformInput) {
        self.cx.update_window(self.window.into(), |_, window, cx| window.dispatch_event(e, cx)).unwrap();
        self.draw();
    }
    fn mouse(&mut self, at: Point<gpui::Pixels>, down: Option<bool>, clicks: usize) {
        let modifiers = Modifiers::default();
        self.input(match down {
            Some(true) => PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: at,
                modifiers,
                click_count: clicks,
                first_mouse: false,
                pen: None,
            }),
            Some(false) => PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position: at,
                modifiers,
                click_count: clicks,
                pen: None,
            }),
            None => PlatformInput::MouseMove(MouseMoveEvent { position: at, pressed_button: None, modifiers, pen: None }),
        });
    }
    fn shot(&mut self, name: &str) -> PathBuf {
        self.draw();
        let img = self.cx.capture_screenshot(self.window.into()).unwrap();
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("evidence/widgets");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.png"));
        img.save(&path).unwrap();
        println!("  wrote {}", path.display());
        path
    }
    fn update<R>(&mut self, f: impl FnOnce(&mut Window, &mut App) -> R) -> R {
        let r = self.cx.update_window(self.window.into(), |_, window, cx| f(window, cx)).unwrap();
        self.draw();
        r
    }
}

fn new_entity<T: 'static>(cx: &mut HeadlessAppContext, f: impl FnOnce(&mut Context<T>) -> T) -> Entity<T> {
    cx.update(|cx| cx.new(f))
}

fn tab(n: u32) -> Piece {
    Piece::Tab { n, default: String::new() }
}
fn tabd(n: u32, d: &str) -> Piece {
    Piece::Tab { n, default: d.into() }
}
fn txt(s: &str) -> Piece {
    Piece::Text(s.into())
}

fn page(t: &Theme) -> gpui::Div {
    div().flex().flex_col().gap(t.box_skip).px(t.margin_x).pt(px(16.)).pb(px(16.)).w_full()
}

fn rgb(c: u32) -> Rgb {
    Rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}
fn tones(r: u32, b: u32, h: u32) -> Tones {
    Tones { rule: rgb(r), back: rgb(b), head: rgb(h) }
}

// ---------------------------------------------------------------------------
// renders

fn render_type(typeface: &'static str, name: &str) {
    let mut cx = app();
    cx.update(|cx| widgets::theme::set_typeface(typeface, cx));
    let names = cx.text_system().all_font_names();
    for fam in widgets::fonts::FAMILIES {
        assert!(names.iter().any(|n| n == fam), "{fam} is not registered (have: {:?})", names.iter().filter(|n| n.contains("Latin") || n.contains("LM") || n.contains("Gyre")).collect::<Vec<_>>());
    }
    let scene: Scene = Rc::new(|_, cx| {
        let t = cx.theme().clone();
        let row = |label: &str, el: gpui::Div| {
            div().flex().flex_row().gap(px(16.)).child(div().w(px(120.)).font(t.mono()).text_size(px(11.)).text_color(t.slate).child(label.to_string())).child(el)
        };
        page(&t)
            .child(Section::new("0", format!("Type specimen ({})", t.typeface)))
            .child(row("serif", div().font(t.serif()).child("The quick brown fox jumps over the lazy dog — fi fl ff 1234567890")))
            .child(row("italic", div().font(t.serif_italic()).child("Studio Warning: ‘ff’ starts 12 English words (offer, effect, …).")))
            .child(row("bold", div().font(t.serif_bold()).child("Definition 1.1 Theorem 1.2 Lemma 2.4")))
            .child(row("caps", div().font(t.caps()).child("Trigger · Fires · Where · Snippet · Block · Inbox")))
            .child(row("mono 14", div().font(t.mono()).text_size(t.mono_size).child("math(\"ff\", [[\\frac{<>}{<>}]], \"yours\"),")))
            .child(row("mono 26", div().font(t.mono()).text_size(t.trigger_size).child("ff \\to")))
            .child(row("math", div().font(t.math_font()).child("● ○ □ ⊠ ▸ ▾ ⇥ ⇧ ⌘ ↵ ∑ ∫ α β")))
            .into_any_element()
    });
    let mut w = Win::open(cx, 860., 420., scene);
    w.shot(name);
}

fn render_boxes() {
    let cx = app();
    let scene: Scene = Rc::new(|_, cx| {
        let t = cx.theme().clone();
        let lua = "  -- studio: id=ff-1432 made=2026-10-07T14:32\n  math(\"ff\", [[\\frac{<>}{<1:x>}<0>%V%1 %% %<]], \"yours\"),";
        div()
            .flex()
            .flex_col()
            .w_full()
            .child(
                div().px(t.margin_x).pt(px(14.)).child(
                    RunningHead::new("head", "Ntex Studio").page("Snippet", None).page("Block", None).page("Inbox", Some(3)).current(0),
                ),
            )
            .child(
                page(&t)
                    .child(Section::new("1", "New snippet"))
                    .child(TBox::new(BoxKind::Definition, "Definition 1.1").name("Trigger").child(div().font(t.mono()).text_size(t.trigger_size).child("ff")))
                    .child(TBox::new(BoxKind::Theorem, "Theorem 1.2").name("Output").child("Body text in Latin Modern Roman, 15 px."))
                    .child(
                        TBox::new(BoxKind::Example, "Example 1.3").name("Try it in ntex").child(
                            Table::new(vec!["You type".into(), "You get".into()])
                                .row(vec![div().font(t.mono()).child("ff").into_any_element(), ntex_studio::ui::widgets::boxes::caret_text(&t, "\\frac{‸}{}", px(14.), px(19.)).into_any_element()])
                                .row(vec![div().font(t.mono()).child("a ⇥").into_any_element(), ntex_studio::ui::widgets::boxes::caret_text(&t, "\\frac{a}{‸}", px(14.), px(19.)).into_any_element()])
                                .row(vec![div().font(t.mono()).child("b ⇥").into_any_element(), ntex_studio::ui::widgets::boxes::caret_text(&t, "\\frac{a}{b}‸", px(14.), px(19.)).into_any_element()]),
                        ),
                    )
                    .child(
                        TBox::new(BoxKind::Remark, "Remark").name("Checks")
                            .child(LogLine::new(Severity::Warning, "‘ff’ is already a snippet: \\frac{…} in operations.lua (yours)."))
                            .child(LogLine::new(Severity::Error, "Output is empty."))
                            .child(LogLine::new(Severity::Note, "Guessed maths from the trigger.")),
                    )
                    .child(TBox::new(BoxKind::Lemma, "Lemma 2.4").name("Snippet").italic_body().child("An italic theorem-style body."))
                    .child(TBox::new(BoxKind::Proof, "Proof").name("Generated Lua").child(CodeView::new(lua, "lua")))
                    .child(TBox::new(BoxKind::Definition, "Definition 2.1").name("Run-in").run_in().child("The body continues on the head's line, as amsthm sets it."))
                    .child(div().flex().flex_row().justify_end().child(MarginNote::new("toast-1", "Saved ‘ff’ to the inbox — live in ntex now. ⌘Z undoes.")))
                    .child(Footnotes::new(vec![
                        "Type the trigger exactly as you will in ntex. Tab makes it expand on Tab.".into(),
                        "Tab inserts a tab stop · ⇧Tab a mirror · ⌘E the exit point.".into(),
                    ])),
            )
            .into_any_element()
    });
    let mut w = Win::open(cx, 860., 1020., scene);
    // the margin note at full opacity (past its 150 ms fade-in; animations run on the wall clock)
    std::thread::sleep(Duration::from_millis(400));
    w.draw();
    w.shot("boxes");
}

fn render_controls() {
    let cx = app();
    let scene: Scene = Rc::new(|_, cx| {
        let t = cx.theme().clone();
        let row = |label: &str| div().flex().flex_row().flex_wrap().items_center().gap(px(18.)).child(div().w(px(110.)).font(t.mono()).text_size(px(11.)).text_color(t.slate).child(label.to_string()));
        let palette = vec![
            (SharedString::from("Blue"), tones(0x1E47B8, 0xE0E7FB, 0x153079)),
            ("Violet".into(), tones(0x661EB8, 0xECE0FB, 0x441579)),
            ("Teal".into(), tones(0x1C829C, 0xE2F4F8, 0x0F596B)),
            ("Maroon".into(), tones(0xA92D4A, 0xF8EAED, 0x79152D)),
            ("Green".into(), tones(0x238B45, 0xE5F5EA, 0x145D2C)),
            ("Grey".into(), tones(0x8C8C8C, 0xF4F4F4, 0x1E1E28)),
        ];
        page(&t)
            .child(Section::new("2", "Controls"))
            .child(
                row("buttons")
                    .child(Button::new("b1", "Clear").keys("⌘N"))
                    .child(Button::new("b2", "Save to inbox").keys("⌘↵").primary())
                    .child(Button::new("b3", "Hover").visual(Visual::Hover))
                    .child(Button::new("b4", "Pressed").visual(Visual::Press))
                    .child(Button::new("b5", "Disabled").keys("⌘⇧S").disabled(true))
                    .child(Button::new("b6", "Focused").visual(Visual::Focus))
                    .child(Button::new("b7", "Use priority 1").small()),
            )
            .child(row("radio").child(Radio::new("r1", "Fires", vec!["by itself".into(), "on Tab".into()], 0).guessed(true)))
            .child(row("radio focus").child(Radio::new("r2", "In", vec!["maths".into(), "text".into()], 1).visual(Visual::Focus)))
            .child(row("radio off").child(Radio::new("r3", "Line", vec!["anywhere".into(), "at the start".into(), "after text".into()], 0).disabled_options(vec![2])))
            .child(row("checkbox").child(Checkbox::new("c1", "asks for a title", true)).child(Checkbox::new("c2", "numbered", false)).child(Checkbox::new("c3", "focused", false).visual(Visual::Focus)))
            .child(row("keycaps").child(Keycap::new("⌘↵")).child(Keycap::new("⇧⇥")).child(Keycap::new("⌥1")).child(Keycap::new("⇥").size(px(18.))))
            .child(row("swatches").child(Swatches::new("sw", palette).selected(Some(4))))
            .child(row("hue").child(HueSlider::new("hue", 145.)))
            .child(row("disclosure").child(Disclosure::new("d1", "More options.", false)).child(Disclosure::new("d2", "More options.", true)))
            .child(row("caps label").child(caps_label(&t, "Priority")).child(caps_label(&t, "Description")))
            .into_any_element()
    });
    let mut w = Win::open(cx, 860., 560., scene);
    w.shot("controls");
}

fn render_fields() {
    let mut cx = app();
    let tf_empty = new_entity(&mut cx, |cx| TextField::new(cx).placeholder("Fun fact"));
    let tf_full = new_entity(&mut cx, |cx| {
        let mut f = TextField::new(cx);
        f.set_text("Fun fact", cx);
        f
    });
    let tf_focus = new_entity(&mut cx, |cx| {
        let mut f = TextField::new(cx).mono();
        f.set_text("funfact", cx);
        f
    });
    let tf_multi = new_entity(&mut cx, |cx| {
        let mut f = TextField::new(cx).mono().multiline();
        f.set_text("return line:match(\"^%s*$\") ~= nil\n\tand trigger ~= \"\"", cx);
        f
    });
    let tr_empty = new_entity(&mut cx, TriggerField::new);
    let tr_tab = new_entity(&mut cx, |cx| {
        let mut f = TriggerField::new(cx);
        f.set("ff", true, cx);
        f
    });
    let ed = new_entity(&mut cx, |cx| {
        let mut e = TokenEditor::new(cx);
        e.set_kind(ntex_studio::model::Kind::Post, cx);
        e.set_pieces(
            vec![
                txt("\\frac{"),
                tab(1),
                txt("}{"),
                tabd(2, "x"),
                txt("} + "),
                tab(1),
                txt("\n\t\\hat{"),
                Piece::Capture(1),
                txt("} "),
                Piece::Selection,
                txt(" "),
                Piece::Exit,
            ],
            cx,
        );
        e
    });
    let ed_empty = new_entity(&mut cx, TokenEditor::new);
    let (a, b, c, d, e1, e2, e3, e4) = (
        tf_empty.clone(),
        tf_full.clone(),
        tf_focus.clone(),
        tf_multi.clone(),
        tr_empty.clone(),
        tr_tab.clone(),
        ed.clone(),
        ed_empty.clone(),
    );
    let scene: Scene = Rc::new(move |_, cx| {
        let t = cx.theme().clone();
        let lab = |s: &str| div().w(px(150.)).flex_none().font(t.mono()).text_size(px(11.)).text_color(t.slate).child(s.to_string());
        let row = |s: &str, el: AnyElement| div().flex().flex_row().items_end().gap(px(14.)).child(lab(s)).child(div().flex_1().child(el));
        page(&t)
            .child(Section::new("3", "Fields"))
            .child(row("text, empty", div().w(px(260.)).child(a.clone()).into_any_element()))
            .child(row("text, filled", div().w(px(260.)).child(b.clone()).into_any_element()))
            .child(row("mono, focused, sel", div().w(px(260.)).child(c.clone()).into_any_element()))
            .child(row("multi-line mono", d.clone().into_any_element()))
            .child(row("trigger, empty", e1.clone().into_any_element()))
            .child(row("trigger ff ⇥ focused", e2.clone().into_any_element()))
            .child(row("toolbar (post)", TokenToolbar::new(e3.clone()).into_any_element()))
            .child(row("token editor", e3.clone().into_any_element()))
            .child(row("token editor, empty", e4.clone().into_any_element()))
            .into_any_element()
    });
    let mut w = Win::open(cx, 860., 640., scene);
    // a focused mono field with a selection
    let f = tf_focus.read_with(&w.cx, |f, _| f.focus.clone());
    w.focus(&f);
    w.key("cmd-a");
    w.key("shift-left");
    w.shot("fields-text-focused");
    // the focused trigger with ⇥
    let f = tr_tab.read_with(&w.cx, |f, _| f.focus.clone());
    w.focus(&f);
    w.shot("fields-trigger-focused");
    // the editor focused, hovering a chip: the tooltip
    let f = ed.read_with(&w.cx, |e, _| e.focus.clone());
    w.focus(&f);
    let chips = ed.read_with(&w.cx, |e, _| e.chip_rects());
    assert!(chips.len() >= 6, "chips laid out: {chips:?}");
    let (_, b) = chips[2]; // the mirror of 1
    let at = point(b.left() + b.size.width / 2., b.top() + b.size.height / 2.);
    w.mouse(at, None, 0);
    w.cx.advance_clock(Duration::from_millis(500));
    w.draw();
    let tip = ed.read_with(&w.cx, |e, _| e.hovered_tooltip());
    assert_eq!(tip.as_deref(), Some("Mirror of tab stop 1 — typed in 2 places"));
    w.shot("fields-editor-tooltip");
    // double-click the default chip: edit it in place
    let (_, b) = chips[1];
    let at = point(b.left() + b.size.width / 2., b.top() + b.size.height / 2.);
    w.mouse(at, Some(true), 2);
    w.mouse(at, Some(false), 2);
    assert!(ed.read_with(&w.cx, |e, _| e.editing_default()));
    w.typ("y_1");
    w.shot("fields-editor-editing-default");
    w.key("enter");
    let p = ed.read_with(&w.cx, |e, _| e.pieces().to_vec());
    assert_eq!(p[3], tabd(2, "y_1"), "{p:?}");
    drop((tf_empty, tf_full, tf_focus, tf_multi, tr_empty, tr_tab, ed, ed_empty));
}

fn render_page(typeface: &'static str, name: &str) {
    let mut cx = app();
    cx.update(|cx| widgets::theme::set_typeface(typeface, cx));
    let trig = new_entity(&mut cx, |cx| {
        let mut f = TriggerField::new(cx);
        f.set("ff", false, cx);
        f
    });
    let ed = new_entity(&mut cx, |cx| {
        let mut e = TokenEditor::new(cx);
        e.set_pieces(vec![txt("\\frac{"), tab(1), txt("}{"), tab(2), txt("}")], cx);
        e
    });
    let (tr, e) = (trig.clone(), ed.clone());
    let scene: Scene = Rc::new(move |_, cx| {
        let t = cx.theme().clone();
        let lua = "  math(\"ff\", [[\\frac{<>}{<>}]], \"yours\"),";
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(div().px(t.margin_x).pt(px(14.)).child(RunningHead::new("head", "Ntex Studio").page("Snippet", None).page("Block", None).page("Inbox", Some(3)).current(0)))
            .child(
                page(&t)
                    .pt(t.margin_top)
                    .child(Section::new("1", "New snippet"))
                    .child(
                        TBox::new(BoxKind::Definition, "Definition 1.1").name("Trigger")
                            .child(tr.clone())
                            .child(div().flex().flex_row().gap(px(28.))
                                .child(Radio::new("fires", "Fires", vec!["by itself".into(), "on Tab".into()], 0).guessed(true))
                                .child(Radio::new("in", "In", vec!["maths".into(), "text".into()], 0).guessed(true))),
                    )
                    .child(TBox::new(BoxKind::Theorem, "Theorem 1.2").name("Output").child(TokenToolbar::new(e.clone())).child(e.clone()))
                    .child(
                        TBox::new(BoxKind::Example, "Example 1.3").name("Try it in ntex")
                            .child(div().child("In $…$ of a .tex file:"))
                            .child(
                                Table::new(vec!["You type".into(), "You get".into()])
                                    .row(vec![div().font(t.mono()).child("ff").into_any_element(), ntex_studio::ui::widgets::boxes::caret_text(&t, "\\frac{‸}{}", px(14.), px(19.)).into_any_element()])
                                    .row(vec![div().font(t.mono()).child("a ⇥").into_any_element(), ntex_studio::ui::widgets::boxes::caret_text(&t, "\\frac{a}{‸}", px(14.), px(19.)).into_any_element()])
                                    .row(vec![div().font(t.mono()).child("b ⇥").into_any_element(), ntex_studio::ui::widgets::boxes::caret_text(&t, "\\frac{a}{b}‸", px(14.), px(19.)).into_any_element()]),
                            ),
                    )
                    .child(Disclosure::new("more", "More options.", false))
                    .child(TBox::new(BoxKind::Proof, "Proof").name("Generated Lua").child(CodeView::new(lua, "lua")))
                    .child(div().flex().flex_row().justify_end().gap(px(16.)).child(Button::new("clear", "Clear").keys("⌘N")).child(Button::new("save", "Save to inbox").keys("⌘↵").primary()))
                    .child(div().h(px(30.)))
                    .child(Footnotes::new(vec!["Type the trigger exactly as you will in ntex. Tab makes it expand on Tab.".into()])),
            )
            .into_any_element()
    });
    let mut w = Win::open(cx, 860., 960., scene);
    let f = trig.read_with(&w.cx, |f, _| f.focus.clone());
    w.focus(&f);
    w.shot(name);
    drop((trig, ed));
}

// ---------------------------------------------------------------------------
// keys

fn editor_window() -> (Win, Entity<TokenEditor>, Entity<TriggerField>, Rc<RefCell<Vec<String>>>) {
    let mut cx = app();
    let ed = new_entity(&mut cx, TokenEditor::new);
    let tr = new_entity(&mut cx, TriggerField::new);
    let events: Rc<RefCell<Vec<String>>> = Rc::default();
    let (ev1, ev2) = (events.clone(), events.clone());
    cx.update(|cx| {
        cx.subscribe(&ed, move |_, e: &TokenEditorEvent, _| {
            ev1.borrow_mut().push(
                match e {
                    TokenEditorEvent::Changed => "ed:changed",
                    TokenEditorEvent::Submit => "ed:submit",
                    TokenEditorEvent::Cancel => "ed:cancel",
                }
                .into(),
            )
        })
        .detach();
        cx.subscribe(&tr, move |_, e: &TriggerEvent, _| {
            ev2.borrow_mut().push(
                match e {
                    TriggerEvent::Changed => "tr:changed",
                    TriggerEvent::Submit => "tr:submit",
                    TriggerEvent::Cancel => "tr:cancel",
                }
                .into(),
            )
        })
        .detach();
    });
    let (e, t) = (ed.clone(), tr.clone());
    let scene: Scene = Rc::new(move |_, cx| {
        let th = cx.theme().clone();
        page(&th).child(t.clone()).child(e.clone()).into_any_element()
    });
    let w = Win::open(cx, 760., 900., scene);
    (w, ed, tr, events)
}

fn pieces(w: &Win, ed: &Entity<TokenEditor>) -> Vec<Piece> {
    ed.read_with(&w.cx, |e, _| e.pieces().to_vec())
}

fn keys_token_editor() {
    let (mut w, ed, _tr, events) = editor_window();
    let f = ed.read_with(&w.cx, |e, _| e.focus.clone());
    w.focus(&f);

    // \frac{ Tab }{ Tab }
    w.typ("\\frac{");
    w.key("tab");
    w.typ("}{");
    w.key("tab");
    w.typ("}");
    assert_eq!(pieces(&w, &ed), vec![txt("\\frac{"), tab(1), txt("}{"), tab(2), txt("}")]);
    assert!(w.log.borrow().is_empty(), "the app saw keys meant for the editor: {:?}", w.log.borrow());

    // ⇧Tab: a mirror of the last tab stop before the caret (2); again: cycles to 1
    w.key("shift-tab");
    assert_eq!(pieces(&w, &ed).last(), Some(&tab(2)));
    w.key("shift-tab");
    assert_eq!(pieces(&w, &ed), vec![txt("\\frac{"), tab(1), txt("}{"), tab(2), txt("}"), tab(1)]);

    // ⌘E: the exit point; ⌘E again elsewhere moves it
    w.key("cmd-e");
    assert_eq!(pieces(&w, &ed).last(), Some(&Piece::Exit));
    w.key("left");
    w.key("left");
    w.key("cmd-e");
    let p = pieces(&w, &ed);
    assert_eq!(p.iter().filter(|p| **p == Piece::Exit).count(), 1, "{p:?}");
    assert_eq!(p, vec![txt("\\frac{"), tab(1), txt("}{"), tab(2), txt("}"), Piece::Exit, tab(1)]);

    // backspace over a chip deletes the whole chip
    w.key("backspace");
    assert_eq!(pieces(&w, &ed), vec![txt("\\frac{"), tab(1), txt("}{"), tab(2), txt("}"), tab(1)]);
    w.key("cmd-right");
    w.key("backspace");
    assert_eq!(pieces(&w, &ed), vec![txt("\\frac{"), tab(1), txt("}{"), tab(2), txt("}")]);

    // a selection becomes the new tab stop's default; renumbered in reading order
    w.key("cmd-left");
    w.key("right");
    w.key("shift-right");
    w.key("shift-right");
    w.key("shift-right");
    w.key("shift-right");
    w.key("tab");
    assert_eq!(pieces(&w, &ed), vec![txt("\\"), tabd(1, "frac"), txt("{"), tab(2), txt("}{"), tab(3), txt("}")]);
    w.key("cmd-z");
    assert_eq!(pieces(&w, &ed), vec![txt("\\frac{"), tab(1), txt("}{"), tab(2), txt("}")]);
    w.key("cmd-shift-z");
    assert_eq!(pieces(&w, &ed)[1], tabd(1, "frac"));

    // Return auto-indents
    w.update(|_, cx| ed.update(cx, |e, cx| e.set_pieces(vec![txt("\\begin{x}\n\t\\item ")], cx)));
    w.key("cmd-down");
    w.key("enter");
    w.typ("y");
    assert_eq!(pieces(&w, &ed), vec![txt("\\begin{x}\n\t\\item \n\ty")]);

    // copy writes helper syntax
    w.update(|_, cx| ed.update(cx, |e, cx| e.set_pieces(vec![txt("\\frac{"), tab(1), txt("}{"), tabd(2, "x"), txt("}"), Piece::Exit], cx)));
    w.key("cmd-a");
    w.key("cmd-c");
    let clip = w.cx.update(|cx| cx.read_from_clipboard().and_then(|c| c.text()));
    assert_eq!(clip.as_deref(), Some("\\frac{<>}{<:x>}<0>"));

    // paste VS Code syntax into an empty editor: chips
    w.update(|_, cx| ed.update(cx, |e, cx| e.set_pieces(Vec::new(), cx)));
    w.cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string("\\frac{$1}{$2}$0".into())));
    w.key("cmd-v");
    assert_eq!(pieces(&w, &ed), vec![txt("\\frac{"), tab(1), txt("}{"), tab(2), txt("}"), Piece::Exit]);
    // pasted again after it: new tab stops (3, 4), and only the new exit point
    w.key("cmd-v");
    let p = pieces(&w, &ed);
    assert_eq!(p.iter().filter(|p| **p == Piece::Exit).count(), 1, "{p:?}");
    assert!(p.contains(&tab(4)), "{p:?}");

    // captures/selection only when allowed
    w.update(|_, cx| ed.update(cx, |e, cx| e.set_pieces(Vec::new(), cx)));
    w.key("alt-1");
    assert_eq!(pieces(&w, &ed), vec![]);
    w.update(|_, cx| ed.update(cx, |e, cx| e.set_allowed(TokenSet { captures: 1, selection: true }, cx)));
    w.key("alt-1");
    w.key("cmd-shift-v");
    assert_eq!(pieces(&w, &ed), vec![Piece::Capture(1), Piece::Selection]);

    // ⌘↵ submits (the editor's, not the app's); Esc cancels
    events.borrow_mut().clear();
    w.key("cmd-enter");
    w.key("escape");
    assert_eq!(*events.borrow(), vec!["ed:submit".to_string(), "ed:cancel".to_string()]);
    assert!(w.log.borrow().is_empty(), "{:?}", w.log.borrow());
    println!("  token editor keys: ok");
    drop((ed, _tr));
}

fn keys_trigger() {
    let (mut w, ed, tr, events) = editor_window();
    let f = tr.read_with(&w.cx, |t, _| t.focus.clone());
    w.focus(&f);
    let trig = |w: &Win| tr.read_with(&w.cx, |t, _| (t.trigger.clone(), t.on_tab));

    // the app binds f, space and tab: the field still takes them literally
    w.typ("ff");
    w.key("space");
    w.key("\\");
    w.typ("to1(");
    assert_eq!(trig(&w), ("ff \\to1(".to_string(), false));
    w.key("tab");
    assert_eq!(trig(&w), ("ff \\to1(".to_string(), true));
    assert!(w.log.borrow().is_empty(), "the app stole keys: {:?}", w.log.borrow());

    // ⌥⌫ deletes a word
    w.key("alt-backspace");
    assert_eq!(trig(&w).0, "ff \\to1");
    w.key("alt-backspace");
    assert_eq!(trig(&w).0, "ff \\");
    w.key("alt-backspace");
    assert_eq!(trig(&w).0, "ff ");
    // ⌘-chords pass through: ⌘1 and ⌘↵ reach the app
    w.key("cmd-1");
    w.key("cmd-enter");
    assert_eq!(*w.log.borrow(), vec!["app:next".to_string(), "app:save".to_string()]);
    // Return / ↓ submit, Esc cancels
    events.borrow_mut().clear();
    w.key("enter");
    w.key("down");
    w.key("escape");
    assert_eq!(*events.borrow(), vec!["tr:submit".to_string(), "tr:submit".to_string(), "tr:cancel".to_string()]);
    // ⌘⌫ deletes everything; ⌘Z brings it back
    w.key("cmd-backspace");
    assert_eq!(trig(&w).0, "");
    w.key("cmd-z");
    assert_eq!(trig(&w).0, "ff ");
    // "key" mode: one key
    w.update(|_, cx| tr.update(cx, |t, cx| t.set_mode("key", cx)));
    w.typ("S");
    assert_eq!(trig(&w).0, "S");
    println!("  trigger keys: ok");
    drop((ed, tr));
}

fn keys_text_field() {
    let mut cx = app();
    let tf = new_entity(&mut cx, TextField::new);
    let t2 = tf.clone();
    let scene: Scene = Rc::new(move |_, cx| page(cx.theme()).child(t2.clone()).into_any_element());
    let mut w = Win::open(cx, 600., 200., scene);
    let f = tf.read_with(&w.cx, |t, _| t.focus.clone());
    w.focus(&f);
    w.typ("Fun fact");
    w.key("alt-left");
    w.key("alt-shift-right");
    w.key("cmd-x");
    assert_eq!(tf.read_with(&w.cx, |t, _| t.text.clone()), "Fun ");
    w.key("cmd-v");
    w.key("cmd-v");
    assert_eq!(tf.read_with(&w.cx, |t, _| t.text.clone()), "Fun factfact");
    w.key("cmd-z");
    w.key("cmd-z");
    w.key("cmd-z");
    assert_eq!(tf.read_with(&w.cx, |t, _| t.text.clone()), "Fun fact");
    // Tab is the page's
    w.key("tab");
    assert_eq!(*w.log.borrow(), vec!["app:tab".to_string()]);
    println!("  text field keys: ok");
    drop(tf);
}

/// Input-to-frame on a 30-line output: a key dispatched, the editor
/// re-laid-out and the frame drawn (the GPU readback isn't part of it).
fn keystroke_cost() {
    let (mut w, ed, _tr, _) = editor_window();
    let mut p = Vec::new();
    for i in 0..30 {
        p.push(txt(&format!("\t\\frac{{\\partial^{{{i}}} f_{{{i}}}}}{{\\partial x^{{{i}}}}} + ")));
        p.push(tab(i as u32 + 1));
        p.push(txt(" = \\int_0^1 g(t)\\,dt"));
        if i % 3 == 0 {
            p.push(tab(1));
        }
        p.push(txt("\n"));
    }
    w.update(|_, cx| ed.update(cx, |e, cx| e.set_pieces(p, cx)));
    let f = ed.read_with(&w.cx, |e, _| e.focus.clone());
    w.focus(&f);
    w.key("cmd-up");
    for _ in 0..10 {
        w.key("down");
    }
    w.key("cmd-right");
    let mut times = Vec::new();
    for i in 0..60 {
        let k = match i % 6 {
            0 => "tab",
            1 => "backspace",
            _ => "x",
        };
        let start = Instant::now();
        w.key(k);
        times.push(start.elapsed());
    }
    times.sort();
    let mean = times.iter().sum::<Duration>() / times.len() as u32;
    let p95 = times[(times.len() * 95) / 100];
    let max = *times.last().unwrap();
    println!("  keystroke on a 30-line output (key + layout + frame): mean {mean:.2?}, p95 {p95:.2?}, max {max:.2?}");
    drop((ed, _tr));
    assert!(p95 < Duration::from_millis(16), "p95 {p95:?} ≥ 16 ms");
}

fn main() {
    let only = std::env::var("NTEX_GALLERY_ONLY").unwrap_or_default();
    if only != "keys" {
        println!("renders:");
        render_type("Computer Modern", "type-latin-modern");
        render_type("Palatino", "type-pagella");
        render_boxes();
        render_controls();
        render_fields();
        render_page("Computer Modern", "page-snippet");
        render_page("Palatino", "page-snippet-palatino");
    }
    println!("keys:");
    keys_token_editor();
    keys_trigger();
    keys_text_field();
    keystroke_cost();
    println!("widgets_gallery: all ok");
}
