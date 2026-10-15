//! Page 2: New block (UX.md "Page 2"). Owner: the SCREENS builder.
//!
//! Name → Looks like (Like / Numbered / Body / Colour, live contrast) →
//! Preview (tectonic specimen) → Snippet → Proof (Generated LaTeX) → Add.
//! "Like" sets numbering, body and colour unless she has touched them (the
//! same pin/guess model as snippets). Then the table of her blocks.
use crate::core::{blocks, colour, inbox};
use crate::model::*;
use crate::ui::app::{Preview, Studio, UndoItem, local_now, safe};
use crate::ui::screens::shell::{self, GAP};
use crate::ui::widgets::boxes::{BoxKind, LogLine, Section, TBox, Table};
use crate::ui::widgets::code_view::CodeView;
use crate::ui::widgets::controls::{Button, Checkbox, HueSlider, Radio, Swatches, keycap};
use crate::ui::widgets::text_field::{TextField, TextFieldEvent};
use crate::ui::widgets::theme::ActiveTheme as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, SharedString, Styled as _,
    Subscription, Window, div, img, prelude::FluentBuilder as _, px,
};
use ntex_studio_io::render::RenderJob;
use std::rc::Rc;
use std::time::{Duration, Instant};

const RENDER_AFTER: Duration = Duration::from_millis(600);
const INK: Rgb = Rgb(0x1E, 0x1E, 0x28);

pub struct BlockPage {
    pub title: Entity<TextField>,
    pub env: Entity<TextField>,
    pub trigger: Entity<TextField>,
    pub draft: BlockDraft,
    /// Fields she set by hand: "numbering", "body", "colour".
    pub pinned: Vec<&'static str>,
    /// The environment no longer follows the title.
    pub env_edited: bool,
    /// The New hue slider's value (kept when she goes back to a palette colour).
    pub hue: f32,
    pub findings: Vec<Finding>,
    pub stanza: String,
    pub preview: Preview,
    /// "Remove “Fun fact”? …" is showing for this environment.
    pub confirm_remove: Option<String>,
    /// The block being edited (its env), if any.
    pub editing: Option<String>,
}

pub fn empty_block() -> BlockDraft {
    BlockDraft {
        title: String::new(),
        env: String::new(),
        base: BlockBase::Remark,
        numbering: Numbering::None,
        body: BodyFont::Upright,
        colour: BlockColour::Palette("Green".into()),
        cref: (String::new(), String::new()),
        trigger: String::new(),
        with_title: true,
    }
}

/// What "Like" implies (ntex-notes.sty): numbering, body, colour.
pub fn like(base: BlockBase) -> (Numbering, BodyFont, &'static str) {
    match base {
        BlockBase::Theorem => (Numbering::WithTheorems, BodyFont::Italic, "Blue"),
        BlockBase::Lemma => (Numbering::WithTheorems, BodyFont::Italic, "Violet"),
        BlockBase::Definition => (Numbering::WithTheorems, BodyFont::Upright, "Teal"),
        BlockBase::Example => (Numbering::WithTheorems, BodyFont::Upright, "Maroon"),
        BlockBase::Remark => (Numbering::None, BodyFont::Upright, "Green"),
    }
}

/// Palette name as shown: "Proof" is the neutral grey.
fn shown_name(n: &str) -> &str {
    if n == "Proof" { "Grey" } else { n }
}

fn colour_name(c: &BlockColour) -> String {
    match c {
        BlockColour::Palette(n) => shown_name(n).to_string(),
        BlockColour::Hue(h) => format!("hue {:.0}°", h),
    }
}

impl BlockPage {
    pub fn new(window: &mut Window, cx: &mut Context<Studio>, subs: &mut Vec<Subscription>) -> BlockPage {
        let title = cx.new(|cx| TextField::new(cx).placeholder("Fun fact"));
        let env = cx.new(|cx| TextField::new(cx).mono().placeholder("funfact"));
        let trigger = cx.new(|cx| TextField::new(cx).mono().placeholder("fun"));
        subs.push(cx.subscribe_in(&title, window, |s: &mut Studio, _, ev: &TextFieldEvent, w, cx| match ev {
            TextFieldEvent::Changed => s.block_changed(cx),
            TextFieldEvent::Submit => {
                let f = s.block.env.read(cx).focus.clone();
                f.focus(w, cx);
            }
            TextFieldEvent::Cancel => s.escape(w, cx),
            TextFieldEvent::Blur => {}
        }));
        subs.push(cx.subscribe_in(&env, window, |s: &mut Studio, _, ev: &TextFieldEvent, w, cx| match ev {
            TextFieldEvent::Changed => {
                let text = s.block.env.read(cx).text.clone();
                let follow = safe("blocks::env_from_title", || blocks::env_from_title(&s.block.draft.title)).unwrap_or_default();
                s.block.env_edited = !text.is_empty() && text != follow;
                s.block_changed(cx)
            }
            TextFieldEvent::Cancel => s.escape(w, cx),
            _ => {}
        }));
        subs.push(cx.subscribe_in(&trigger, window, |s: &mut Studio, _, ev: &TextFieldEvent, w, cx| match ev {
            TextFieldEvent::Changed => s.block_changed(cx),
            TextFieldEvent::Submit => s.save_block(w, cx),
            TextFieldEvent::Cancel => s.escape(w, cx),
            TextFieldEvent::Blur => {}
        }));
        for f in [&title, &env, &trigger] {
            let _ = f.read(cx).focus.clone().tab_stop(true);
        }
        BlockPage {
            title,
            env,
            trigger,
            draft: empty_block(),
            pinned: Vec::new(),
            env_edited: false,
            hue: 145.0,
            findings: Vec::new(),
            stanza: String::new(),
            preview: Preview::new(false, true),
            confirm_remove: None,
            editing: None,
        }
    }
}

impl Studio {
    pub fn block_is_empty(&self) -> bool {
        self.block.draft.title.trim().is_empty() && self.block.draft.trigger.is_empty()
    }

    /// Re-read the fields, apply "Like" to the unpinned ones, check, generate.
    pub fn block_changed(&mut self, cx: &mut Context<Studio>) {
        let now = self.now(cx);
        let b = &mut self.block;
        b.draft.title = b.title.read(cx).text.clone();
        b.draft.trigger = b.trigger.read(cx).text.trim().to_string();
        if !b.env_edited {
            let env = safe("blocks::env_from_title", || blocks::env_from_title(&b.draft.title)).unwrap_or_default();
            if b.env.read(cx).text != env {
                b.env.update(cx, |f, cx| f.set_text(env.clone(), cx));
            }
            b.draft.env = env;
        } else {
            b.draft.env = b.env.read(cx).text.trim().to_string();
        }
        let (numbering, body, col) = like(b.draft.base);
        if !b.pinned.contains(&"numbering") {
            b.draft.numbering = numbering;
        }
        if !b.pinned.contains(&"body") {
            b.draft.body = body;
        }
        if !b.pinned.contains(&"colour") {
            b.draft.colour = BlockColour::Palette(col.into());
        }
        if b.draft.numbering != Numbering::None {
            let lower = b.draft.title.trim().to_lowercase();
            b.draft.cref = (lower.clone(), if lower.is_empty() { String::new() } else { format!("{lower}s") });
        } else {
            b.draft.cref = (String::new(), String::new());
        }
        self.block_rechecked(cx);
        self.block.preview.schedule(now + RENDER_AFTER);
        cx.notify();
    }

    pub fn block_rechecked(&mut self, cx: &mut Context<Studio>) {
        let d = self.block.draft.clone();
        let editing = self.block.editing.clone();
        let existing: Vec<KnownBlock> =
            self.blocks.iter().filter(|k| Some(&k.draft.env) != editing.as_ref()).cloned().collect();
        self.block.findings = if self.block_is_empty() {
            Vec::new()
        } else {
            safe("blocks::check", || blocks::check(&d, &existing, &self.known)).unwrap_or_default()
        };
        self.block.stanza = safe("blocks::stanza", || blocks::stanza(&d)).unwrap_or_default();
        cx.notify();
    }

    fn set_block(&mut self, cx: &mut Context<Studio>, f: impl FnOnce(&mut BlockPage)) {
        f(&mut self.block);
        self.block_changed(cx);
    }

    pub fn load_block(&mut self, d: BlockDraft, window: &mut Window, cx: &mut Context<Studio>) {
        let _ = window;
        let b = &mut self.block;
        b.title.update(cx, |f, cx| f.set_text(d.title.clone(), cx));
        b.env.update(cx, |f, cx| f.set_text(d.env.clone(), cx));
        b.trigger.update(cx, |f, cx| f.set_text(d.trigger.clone(), cx));
        let follow = safe("blocks::env_from_title", || blocks::env_from_title(&d.title)).unwrap_or_default();
        b.env_edited = !d.env.is_empty() && d.env != follow;
        if let BlockColour::Hue(h) = d.colour {
            b.hue = h;
        }
        // a loaded block's looks are hers
        b.pinned = if d.title.is_empty() { Vec::new() } else { vec!["numbering", "body", "colour"] };
        b.draft = d;
        b.confirm_remove = None;
        b.preview.clear();
        self.block_changed(cx);
    }

    pub fn clear_block(&mut self, window: &mut Window, cx: &mut Context<Studio>) {
        if !self.block_is_empty() {
            self.undo.push(UndoItem::ClearedBlock(self.block.draft.clone()));
            self.toast("Cleared. ⌘Z brings it back.", cx);
        }
        self.block.editing = None;
        self.load_block(empty_block(), window, cx);
        self.block.title.read(cx).focus.clone().focus(window, cx);
    }

    /// ⌘↵ on the block page: ntex-blocks.sty + the snippet into the inbox.
    pub fn save_block(&mut self, window: &mut Window, cx: &mut Context<Studio>) {
        self.block_changed(cx);
        if self.block_is_empty() {
            self.block.findings =
                vec![Finding { severity: Severity::Error, code: "empty-title".into(), message: "Title is empty.".into(), fix: None }];
            self.block.title.read(cx).focus.clone().focus(window, cx);
            cx.notify();
            return;
        }
        if self.block.findings.iter().any(|f| f.severity == Severity::Error) {
            self.block.title.read(cx).focus.clone().focus(window, cx);
            cx.notify();
            return;
        }
        let d = self.block.draft.clone();
        let files = vec![self.paths.blocks_sty(), self.paths.notes_sty(), self.paths.inbox()];
        let backed = self.backup(&format!("adding “{}”", d.title), files);
        let saved: Result<(), String> = match safe("blocks::save", || blocks::save(&self.paths, &d)) {
            Some(Ok(())) => Ok(()),
            Some(Err(e)) => Err(e.to_string()),
            None => Err("the block writer is not built yet".into()),
        };
        if let Err(e) = saved {
            if backed.is_ok() {
                self.undo.pop();
            }
            self.toast(format!("! Could not add “{}”: {e}", d.title), cx);
            return;
        }
        let mut snippet_note = String::new();
        if let Some(Some(snip)) = safe("blocks::snippet", || blocks::snippet(&d)) {
            let (minute, _) = local_now();
            match safe("inbox::save", || inbox::save(&self.paths, &snip, &minute)) {
                Some(Ok(_)) => snippet_note = format!("; snippet ‘{}’ ⇥ is in the inbox", d.trigger),
                _ => snippet_note = format!("; the snippet ‘{}’ could not be written to the inbox", d.trigger),
            }
        }
        self.after_files_changed(cx);
        let verb = if self.block.editing.as_deref() == Some(d.env.as_str()) { "Updated" } else { "Added" };
        let undo = if backed.is_ok() { " ⌘Z undoes." } else { "" };
        self.toast(format!("{verb} “{}” — \\begin{{{}}} works in every new compile{snippet_note}.{undo}", d.title, d.env), cx);
        self.block.editing = None;
        self.load_block(empty_block(), window, cx);
    }

    fn remove_block(&mut self, env: String, cx: &mut Context<Studio>) {
        let title = self.blocks.iter().find(|b| b.draft.env == env).map(|b| b.draft.title.clone()).unwrap_or(env.clone());
        let backed = self.backup(&format!("removing “{title}”"), vec![self.paths.blocks_sty()]);
        match safe("blocks::remove", || blocks::remove(&self.paths, &env)) {
            Some(Ok(())) => {
                self.block.confirm_remove = None;
                self.after_files_changed(cx);
                let undo = if backed.is_ok() { " ⌘Z undoes." } else { "" };
                self.toast(format!("Removed “{title}” from your template.{undo}"), cx);
            }
            other => {
                if backed.is_ok() {
                    self.undo.pop();
                }
                let e = match other {
                    Some(Err(e)) => e.to_string(),
                    _ => "the block writer is not built yet".into(),
                };
                self.toast(format!("! Could not remove “{title}”: {e}"), cx);
            }
        }
    }

    pub fn pump_block(&mut self, now: Instant, _cx: &mut Context<Studio>) -> bool {
        let render_on = self.prefs.render_preview;
        let d = &self.block.draft;
        let empty = d.title.trim().is_empty();
        let job = || {
            if !render_on || empty {
                return None;
            }
            let tex = safe("blocks::specimen", || blocks::specimen(d))?;
            Some(RenderJob::Document { tex })
        };
        self.block.preview.pump(now, self.renderer.as_ref(), job)
    }

    pub fn block_hints(&self, window: &Window, cx: &App) -> Vec<SharedString> {
        let b = &self.block;
        if b.title.read(cx).focus.is_focused(window) {
            return vec!["The title as printed, e.g. Fun fact. The environment follows it until you edit it.".into()];
        }
        if b.env.read(cx).focus.is_focused(window) {
            return vec!["Letters only: \\begin{funfact} … \\end{funfact}.".into()];
        }
        if b.trigger.read(cx).focus.is_focused(window) {
            return vec!["A Tab shortcut at the start of a line types the block. Empty: no snippet.".into()];
        }
        vec!["⌘↵ adds the block to your template · ⌘N clears · ⌘Z undoes.".into(), "⌘1 ⌘2 ⌘3 switch pages.".into()]
    }

    pub fn render_block(&mut self, window: &mut Window, cx: &mut Context<Studio>) -> impl IntoElement {
        let t = cx.theme().clone();
        let now = self.now(cx);
        if self.block.preview.opacity(now) < 1.0 {
            window.request_animation_frame();
        }
        let d = self.block.draft.clone();
        let this = cx.weak_entity();
        let sel = move |f: fn(&mut BlockPage, usize)| {
            let this = this.clone();
            move |i: usize, _: &mut Window, cx: &mut App| {
                this.update(cx, |s, cx| s.set_block(cx, |b| f(b, i))).ok();
            }
        };
        let title_shown = if d.title.trim().is_empty() { "Fun fact".to_string() } else { d.title.trim().to_string() };

        // Definition 2.1 (Name)
        let name_box = TBox::new(BoxKind::Definition, "Definition 2.1").name("Name").child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_x(px(28.))
                .gap_y(px(6.))
                .pt(px(4.))
                .child(
                    div().flex().flex_row().items_baseline().gap(px(8.)).child(shell::label("Title", cx)).child(div().w(px(220.)).child(self.block.title.clone())),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_baseline()
                        .gap(px(8.))
                        .child(shell::label("Environment", cx))
                        .child(div().w(px(150.)).child(self.block.env.clone()))
                        .when(!self.block.env_edited && !d.env.is_empty(), |d| d.child(shell::slate("(follows the title)", cx))),
                ),
        );

        // Theorem 2.2 (Looks like)
        let bases = [BlockBase::Theorem, BlockBase::Lemma, BlockBase::Definition, BlockBase::Example, BlockBase::Remark];
        let like_r = Radio::new(
            "like",
            "Like",
            vec!["theorem".into(), "lemma".into(), "definition".into(), "example".into(), "remark".into()],
            bases.iter().position(|b| *b == d.base).unwrap_or(4),
        )
        .on_select(sel(|b, i| {
            b.draft.base = [BlockBase::Theorem, BlockBase::Lemma, BlockBase::Definition, BlockBase::Example, BlockBase::Remark][i.min(4)];
        }));
        let numbered = Radio::new(
            "numbered",
            "Numbered",
            vec!["no".into(), format!("with theorems ({title_shown} 1.4)").into(), format!("on its own ({title_shown} 1)").into()],
            match d.numbering {
                Numbering::None => 0,
                Numbering::WithTheorems => 1,
                Numbering::Own => 2,
            },
        )
        .guessed(!self.block.pinned.contains(&"numbering"))
        .on_select(sel(|b, i| {
            b.draft.numbering = [Numbering::None, Numbering::WithTheorems, Numbering::Own][i.min(2)];
            if !b.pinned.contains(&"numbering") {
                b.pinned.push("numbering");
            }
        }));
        let body = Radio::new("body", "Body", vec!["italic".into(), "upright".into()], (d.body == BodyFont::Upright) as usize)
            .guessed(!self.block.pinned.contains(&"body"))
            .on_select(sel(|b, i| {
                b.draft.body = if i == 0 { BodyFont::Italic } else { BodyFont::Upright };
                if !b.pinned.contains(&"body") {
                    b.pinned.push("body");
                }
            }));
        let palette = safe("colour::palette", colour::palette).unwrap_or_default();
        let hue_tones = safe("colour::tones_for_hue", || colour::tones_for_hue(self.block.hue));
        let mut items: Vec<(SharedString, Tones)> =
            palette.iter().map(|(n, tones)| (SharedString::from(shown_name(n).to_string()), *tones)).collect();
        if let Some(ht) = hue_tones {
            items.push(("New hue…".into(), ht));
        }
        let names: Vec<String> = palette.iter().map(|(n, _)| n.to_string()).collect();
        let selected = match &d.colour {
            BlockColour::Palette(n) => names.iter().position(|x| x == n),
            BlockColour::Hue(_) => Some(names.len()),
        };
        let swatches = Swatches {
            id: "swatches".into(),
            items,
            selected,
            on_select: Some(Rc::new({
                let this = cx.weak_entity();
                move |i, _, cx: &mut App| {
                    let names = names.clone();
                    this.update(cx, |s, cx| {
                        s.set_block(cx, |b| {
                            b.draft.colour = match names.get(i) {
                                Some(n) => BlockColour::Palette(n.clone()),
                                None => BlockColour::Hue(b.hue),
                            };
                            if !b.pinned.contains(&"colour") {
                                b.pinned.push("colour");
                            }
                        })
                    })
                    .ok();
                }
            })),
        };
        let is_hue = matches!(d.colour, BlockColour::Hue(_));
        let slider = is_hue.then(|| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(10.))
                .child(
                    div().w(px(260.)).child(HueSlider {
                        id: "hue".into(),
                        hue: self.block.hue,
                        on_change: Some(Rc::new({
                            let this = cx.weak_entity();
                            move |h, _, cx: &mut App| {
                                this.update(cx, |s, cx| {
                                    s.set_block(cx, |b| {
                                        b.hue = h;
                                        b.draft.colour = BlockColour::Hue(h);
                                    })
                                })
                                .ok();
                            }
                        })),
                    }),
                )
        });
        let tones = safe("colour::tones", || colour::tones(&d.colour));
        let contrast = tones.and_then(|tn| {
            let body = safe("colour::contrast", || colour::contrast(INK, tn.back))?;
            let head = safe("colour::contrast", || colour::contrast(tn.head, tn.back))?;
            Some((body, head))
        });
        let contrast_el = contrast.map(|(body, head)| {
            let ok = body >= 7.0 && head >= 4.5;
            div()
                .flex()
                .flex_row()
                .gap(px(10.))
                .items_baseline()
                .child(shell::label("Contrast", cx))
                .child(div().child(format!("body {body:.1} : 1 · head {head:.1} : 1")))
                .child(if ok {
                    div().text_color(t.remark.head).child("✓").into_any_element()
                } else {
                    div().italic().text_color(t.slate).child("below the bar (body 7 : 1, head 4.5 : 1) — readable, but quieter than the theme").into_any_element()
                })
        });
        let looks_box = TBox::new(BoxKind::Theorem, "Theorem 2.2").name("Looks like").child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .pt(px(4.))
                .child(like_r)
                .child(numbered)
                .child(body)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .gap(px(10.))
                        .child(div().pt(px(2.)).child(shell::label("Colour", cx)))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.))
                                .child(swatches)
                                .when(!self.block.pinned.contains(&"colour"), |d| d.child(shell::slate("(guessed from Like)", cx)))
                                .children(slider)
                                .children(contrast_el),
                        ),
                ),
        );

        // Example 2.3 (Preview)
        let pv = &self.block.preview;
        let preview_el: AnyElement = if !self.prefs.render_preview {
            shell::slate("Rendered previews are off (prefs.lua: render_preview).", cx).into_any_element()
        } else if let Some((image, w, h)) = &pv.image {
            // 10 pt type set at the page's 15 px, within the measure
            let scale = (1.5f32).min(720.0 / w.max(1.0));
            img(image.clone()).w(px(*w * scale)).h(px(*h * scale)).opacity(pv.opacity(now)).into_any_element()
        } else if let Some(e) = &pv.error {
            div().font(t.mono()).text_size(px(13.)).text_color(t.slate).child(e.clone()).into_any_element()
        } else if d.title.trim().is_empty() {
            shell::slate("Give it a title to see it set in your theme.", cx).into_any_element()
        } else {
            shell::slate("setting it…", cx).into_any_element()
        };
        let preview_box = TBox::new(BoxKind::Example, "Example 2.3").name("Preview").child(div().pt(px(4.)).child(preview_el));

        // Lemma 2.4 (Snippet)
        let with_title = d.with_title;
        let begin = if d.env.is_empty() { "…".to_string() } else { d.env.clone() };
        let shape = if with_title {
            format!("→ \\begin{{{begin}}}[‸] … \\end{{{begin}}}")
        } else {
            format!("→ \\begin{{{begin}}} ‸ … \\end{{{begin}}}")
        };
        let snippet_box = TBox::new(BoxKind::Lemma, "Lemma 2.4").name("Snippet").child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .pt(px(4.))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .items_baseline()
                        .gap_x(px(10.))
                        .gap_y(px(4.))
                        .child(shell::label("Trigger", cx))
                        .child(div().w(px(90.)).child(self.block.trigger.clone()))
                        .child(keycap("⇥"))
                        .child(shell::slate("(Tab at the start of a line, in .tex)", cx))
                        .child(div().w(px(18.)))
                        .child(
                            Checkbox::new("with_title", "asks for a title", with_title).on_toggle({
                                let this = cx.weak_entity();
                                move |on, _, cx: &mut App| {
                                    this.update(cx, |s, cx| s.set_block(cx, |b| b.draft.with_title = on)).ok();
                                }
                            }),
                        ),
                )
                .when(!d.trigger.is_empty(), |c| c.child(crate::ui::widgets::boxes::caret_text(&t, &shape, px(14.), px(19.)))),
        );

        // findings
        let lines: Vec<AnyElement> = self
            .block
            .findings
            .iter()
            .map(|f| {
                let text: SharedString = match f.severity {
                    Severity::Error => format!("! {}", f.message).into(),
                    Severity::Warning => format!("Studio Warning: {}", f.message).into(),
                    Severity::Note => f.message.clone().into(),
                };
                LogLine { severity: f.severity, text }.into_any_element()
            })
            .collect();
        let remark = (!lines.is_empty())
            .then(|| TBox::new(BoxKind::Remark, "Remark").name("Checks").child(div().flex().flex_col().gap(px(3.)).pt(px(2.)).children(lines)));

        let proof = TBox::new(BoxKind::Proof, "Proof")
            .name("Generated LaTeX")
            .child(div().pt(px(2.)).child(if self.block_is_empty() {
                shell::slate("Nothing to generate yet.", cx).into_any_element()
            } else {
                CodeView { code: self.block.stanza.clone().into(), language: "latex" }.into_any_element()
            }));

        let label = if self.block.editing.is_some() { "Update my template" } else { "Add to my template" };
        let buttons = div()
            .flex()
            .flex_row()
            .justify_end()
            .gap(px(14.))
            .child(Button::new("block.clear", "Clear").keys("⌘N").on_click(cx.listener(|s, _, w, cx| s.clear_block(w, cx))))
            .child(Button::new("block.save", label).keys("⌘↵").primary().on_click(cx.listener(|s, _, w, cx| s.save_block(w, cx))));

        let section_no = if self.block.editing.is_some() { "Edit block" } else { "New block" };
        div()
            .flex()
            .flex_col()
            .gap(px(GAP))
            .child(Section { number: "2".into(), title: section_no.into() })
            .child(name_box)
            .child(looks_box)
            .child(preview_box)
            .child(snippet_box)
            .children(remark)
            .child(proof)
            .child(buttons)
            .child(self.render_your_blocks(cx))
    }

    fn render_your_blocks(&self, cx: &mut Context<Studio>) -> impl IntoElement {
        let t = cx.theme().clone();
        let mut col = div().flex().flex_col().gap(px(6.)).pt(px(10.)).child(
            div().font_weight(gpui::FontWeight::BOLD).child("Your blocks"),
        );
        if self.blocks.is_empty() {
            return col.child(shell::slate("None yet. Blocks you add appear here, and in every new compile.", cx));
        }
        let mono = |s: String| div().font(t.mono()).text_size(px(14.)).child(s).into_any_element();
        let rows: Vec<Vec<AnyElement>> = self
            .blocks
            .iter()
            .enumerate()
            .map(|(i, k)| {
                let env = k.draft.env.clone();
                let env2 = env.clone();
                let d = k.draft.clone();
                vec![
                    div().child(k.draft.title.clone()).into_any_element(),
                    mono(k.draft.env.clone()),
                    div().child(colour_name(&k.draft.colour)).into_any_element(),
                    if k.draft.trigger.is_empty() {
                        div().text_color(t.slate).child("—").into_any_element()
                    } else {
                        mono(format!("{} ⇥", k.draft.trigger))
                    },
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(6.))
                        .child(Button::new(SharedString::from(format!("block.edit.{i}")), "Edit").on_click(cx.listener(
                            move |s, _, w, cx| {
                                s.block.editing = Some(env.clone());
                                s.load_block(d.clone(), w, cx);
                                s.scroll.set_offset(gpui::point(px(0.), px(0.)));
                            },
                        )))
                        .child(Button::new(SharedString::from(format!("block.remove.{i}")), "Remove").on_click(cx.listener(
                            move |s, _, _, cx| {
                                s.block.confirm_remove = Some(env2.clone());
                                cx.notify();
                            },
                        )))
                        .into_any_element(),
                ]
            })
            .collect();
        col = col.child(Table {
            header: vec!["Block".into(), "Environment".into(), "Colour".into(), "Snippet".into(), "".into()],
            rows,
            widths: Some(vec![0.22, 0.2, 0.16, 0.14, 0.28]),
        });
        if let Some(env) = &self.block.confirm_remove {
            let title = self.blocks.iter().find(|b| &b.draft.env == env).map(|b| b.draft.title.clone()).unwrap_or(env.clone());
            let env_r = env.clone();
            col = col.child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_baseline()
                    .gap(px(10.))
                    .child(div().child(crate::ui::widgets::boxes::prose(
                        &t,
                        &format!("Remove “{title}”? Documents using \\begin{{{env}}} will stop compiling."),
                        t.serif(),
                        t.ink,
                    )))
                    .child(Button::new("block.remove.yes", "Remove").on_click(cx.listener(move |s, _, _, cx| s.remove_block(env_r.clone(), cx))))
                    .child(Button::new("block.remove.no", "Keep").on_click(cx.listener(|s, _, _, cx| {
                        s.block.confirm_remove = None;
                        cx.notify();
                    }))),
            );
        }
        col
    }
}
