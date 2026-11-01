//! Page 5: Look. The editor look ntex paints with (the palette, and how
//! sections, blocks and nesting show), as radio groups and a checkbox. Every
//! change is written to look.json at once (core::look); every open ntex
//! watches that file and repaints. Owner: the SCREENS builder.
use crate::core::look::{self, Look, SETTINGS};
use crate::ui::app::Studio;
use crate::ui::screens::shell::{self, GAP};
use crate::ui::widgets::boxes::{Rich, Section, aside};
use crate::ui::widgets::controls::{Button, ChannelSlider, Checkbox, Radio};
use crate::ui::widgets::text_field::{TextField, TextFieldEvent};
use crate::ui::widgets::theme::{ActiveTheme as _, Theme};
use gpui::{
    App, AppContext as _, Context, Entity, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, prelude::FluentBuilder as _, px, rgb,
};

/// The Look page's own state: which color the HSV editor has, and its hex field.
pub struct LookPage {
    /// Index into `look::ROLES`.
    pub role: usize,
    pub hex: Entity<TextField>,
    /// Saved looks (looks.json).
    pub looks: crate::core::looks::Looks,
    /// The name field for Save as / Rename.
    pub name: Entity<TextField>,
    /// The saved look last pressed (Rename / Delete act on it).
    pub selected: Option<String>,
}

impl LookPage {
    pub fn new(window: &mut Window, cx: &mut Context<Studio>, subs: &mut Vec<Subscription>) -> LookPage {
        let hex = cx.new(|cx| TextField::new(cx).mono().width(px(96.)).placeholder("#rrggbb"));
        subs.push(cx.subscribe_in(&hex, window, |s: &mut Studio, _, ev: &TextFieldEvent, window, cx| match ev {
            TextFieldEvent::Submit | TextFieldEvent::Blur => s.look_hex_entered(cx),
            TextFieldEvent::Cancel => {
                s.look_sync_hex(cx);
                s.focus.clone().focus(window, cx);
            }
            TextFieldEvent::Changed => {}
        }));
        let name = cx.new(|cx| TextField::new(cx).width(px(220.)).placeholder("a name for this look"));
        subs.push(cx.subscribe_in(&name, window, |s: &mut Studio, _, ev: &TextFieldEvent, _, cx| {
            if let TextFieldEvent::Submit = ev {
                s.look_save_as(cx)
            }
        }));
        LookPage { role: 0, hex, looks: Default::default(), name, selected: None }
    }
}

fn colour(c: u32) -> Hsla {
    rgb(c).into()
}

/// t of b into a (as palettes.lua's mix).
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |x: u32, s: u32| ((x >> s) & 0xff) as f32;
    let f = |s: u32| ((ch(a, s) + (ch(b, s) - ch(a, s)) * t).round() as u32) << s;
    f(16) | f(8) | f(0)
}

impl Studio {
    /// The page was opened: look.json may have changed (:NtexLook in ntex, a hand edit).
    pub fn reload_look(&mut self) {
        self.look = look::read(&self.paths);
        self.look_page.looks = crate::core::looks::read(&self.paths);
    }

    fn write_looks(&mut self, what: &str, cx: &mut Context<Studio>) {
        match crate::core::looks::write(&self.paths, &self.look_page.looks) {
            Ok(()) => self.toast(what.to_string(), cx),
            Err(e) => self.toast(format!("! Could not write looks.json: {e}"), cx),
        }
        cx.notify();
    }

    /// Press a saved look: everything goes back to it.
    pub fn look_restore(&mut self, name: &str, cx: &mut Context<Studio>) {
        let Some(saved) = self.look_page.looks.editor_all().into_iter().find(|s| s.name == name) else { return };
        self.look_page.selected = Some(name.to_string());
        self.look_page.name.update(cx, |f, cx| f.set_text(name, cx));
        self.set_look(|l| *l = crate::core::looks::restore(l, &saved.look), cx);
        self.toast(format!("Back to “{name}”."), cx);
    }

    pub fn look_save_as(&mut self, cx: &mut Context<Studio>) {
        let name = self.look_page.name.read(cx).text.trim().to_string();
        let look = self.look.clone();
        match self.look_page.looks.save_editor(&name, &look) {
            Ok(()) => {
                self.look_page.selected = Some(name.clone());
                self.write_looks(&format!("Saved the look as “{name}”."), cx);
            }
            Err(e) => self.toast(format!("! {e}"), cx),
        }
    }

    pub fn look_rename(&mut self, cx: &mut Context<Studio>) {
        let Some(from) = self.look_page.selected.clone() else { return };
        let to = self.look_page.name.read(cx).text.trim().to_string();
        match self.look_page.looks.rename_editor(&from, &to) {
            Ok(()) => {
                self.look_page.selected = Some(to.clone());
                self.write_looks(&format!("Renamed “{from}” to “{to}”."), cx);
            }
            Err(e) => self.toast(format!("! {e}"), cx),
        }
    }

    pub fn look_delete(&mut self, cx: &mut Context<Studio>) {
        let Some(name) = self.look_page.selected.clone() else { return };
        match self.look_page.looks.delete_editor(&name) {
            Ok(()) => {
                self.look_page.selected = None;
                self.write_looks(&format!("Deleted “{name}”."), cx);
            }
            Err(e) => self.toast(format!("! {e}"), cx),
        }
    }

    fn render_saved(&mut self, cx: &mut Context<Studio>) -> impl IntoElement {
        let t = cx.theme().clone();
        let all = self.look_page.looks.editor_all();
        let mut buttons = div().flex().flex_row().flex_wrap().gap(px(8.));
        for (i, saved) in all.iter().enumerate() {
            let name = saved.name.clone();
            let label = if saved.protected { format!("{} ·", saved.name) } else { saved.name.clone() };
            let mut b = Button::new(SharedString::from(format!("look.saved.{i}")), label)
                .on_click(cx.listener(move |s, _, _, cx| s.look_restore(&name, cx)));
            if crate::core::looks::matches(&self.look, &saved.look) {
                b = b.primary();
            }
            buttons = buttons.child(b);
        }
        let sel = self.look_page.selected.clone();
        let editable = sel.as_ref().is_some_and(|n| self.look_page.looks.editor.iter().any(|s| &s.name == n && !s.protected));
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(shell::label("Saved looks", cx))
            .child(buttons)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .child(self.look_page.name.clone())
                    .child(Button::new("look.save-as", "Save current as").on_click(cx.listener(|s, _, _, cx| s.look_save_as(cx))))
                    .child(Button::new("look.rename", "Rename").disabled(!editable).on_click(cx.listener(|s, _, _, cx| s.look_rename(cx))))
                    .child(Button::new("look.delete", "Delete").disabled(!editable).on_click(cx.listener(|s, _, _, cx| s.look_delete(cx)))),
            )
            .child(aside(&t, "Press a look to go back to it (the doubled box is the one you're on). Looks marked · are protected: they can't be renamed, overwritten or deleted. Saved looks live in looks.json."))
    }

    fn look_role(&self) -> &'static str {
        look::ROLES[self.look_page.role.min(look::ROLES.len() - 1)].0
    }

    /// The hex field shows the selected color (unless she is typing in it).
    pub fn look_sync_hex(&mut self, cx: &mut Context<Studio>) {
        let c = look::hex(self.look.color(self.look_role()));
        self.look_page.hex.update(cx, |f, cx| {
            if f.text != c {
                f.set_text(c, cx);
            }
        });
    }

    /// Return (or leaving the field) in the hex field: a valid color is applied.
    pub fn look_hex_entered(&mut self, cx: &mut Context<Studio>) {
        let text = self.look_page.hex.read(cx).text.clone();
        let role = self.look_role();
        match look::parse_hex(&text) {
            Some(c) => self.set_look(|l| l.set_color(role, c), cx),
            None => self.toast(format!("! {text} is not a color: write it as #rrggbb."), cx),
        }
        self.look_sync_hex(cx);
    }

    fn look_select(&mut self, i: usize, cx: &mut Context<Studio>) {
        self.look_page.role = i;
        self.look_sync_hex(cx);
        cx.notify();
    }

    /// Change one setting and write look.json.
    pub fn set_look(&mut self, change: impl FnOnce(&mut Look), cx: &mut Context<Studio>) {
        let before = self.look.clone();
        change(&mut self.look);
        if self.look == before {
            return;
        }
        if let Err(e) = look::write(&self.paths, &self.look) {
            self.look = before;
            self.toast(format!("! Could not write look.json: {e}"), cx);
        }
        self.look_sync_hex(cx);
        cx.notify();
    }

    pub fn look_hints(&self, _window: &Window, _cx: &App) -> Vec<SharedString> {
        vec![
            "←/→ change the focused setting; Tab moves to the next one.".into(),
            "Every open ntex repaints at once (look.json). In ntex itself: :NtexLook.".into(),
        ]
    }

    pub fn render_look(&mut self, _window: &mut Window, cx: &mut Context<Studio>) -> impl IntoElement {
        let t = cx.theme().clone();
        let mut controls = div().flex().flex_col().gap(px(4.));
        // the palette as two rows: hers, then the standard themes to compare
        let pal = self.look.index("palette");
        for (label, range) in [("Palette", 0..look::CUSTOM), ("Standard", look::CUSTOM..look::PALETTES.len())] {
            let options: Vec<SharedString> = look::PALETTES[range.clone()].iter().map(|(_, l)| SharedString::from(*l)).collect();
            let start = range.start;
            let selected = if range.contains(&pal) { pal - start } else { usize::MAX };
            let this = cx.weak_entity();
            controls = controls.child(
                Radio::new(SharedString::from(format!("look.palette.{label}")), label, options, selected)
                    .label_width(px(96.))
                    .on_select(move |i, _, cx| {
                        let v = look::PALETTES[(start + i).min(look::PALETTES.len() - 1)].0;
                        this.update(cx, |st, cx| st.set_look(|l| { l.set("palette", v); }, cx)).ok();
                    }),
            );
        }
        let standard = look::is_standard(&self.look.palette);
        for s in SETTINGS.iter().filter(|s| s.key != "palette") {
            let key = s.key;
            let options: Vec<SharedString> = s.options.iter().map(|(_, l)| SharedString::from(*l)).collect();
            let this = cx.weak_entity();
            controls = controls.child(
                Radio::new(SharedString::from(format!("look.{key}")), s.label, options, self.look.index(key))
                    .label_width(px(96.))
                    .disabled_options(if key == "ink" && standard { (0..s.options.len()).collect() } else { Vec::new() })
                    .on_select(move |i, _, cx| {
                        let value = SETTINGS.iter().find(|s| s.key == key).map(|s| s.options[i.min(s.options.len() - 1)].0);
                        if let Some(v) = value {
                            this.update(cx, |st, cx| st.set_look(|l| { l.set(key, v); }, cx)).ok();
                        }
                    }),
            );
            if key == "lines" {
                let this = cx.weak_entity();
                controls = controls.child(
                    div().flex().flex_row().gap(px(12.)).child(div().w(px(96.)).flex_none().font(t.caps()).child("Guides")).child(
                        Checkbox::new("look.guides", "Nesting lines", self.look.guides).on_toggle(move |on, _, cx| {
                            this.update(cx, |st, cx| st.set_look(|l| l.guides = on, cx)).ok();
                        }),
                    ),
                );
            }
        }
        div()
            .flex()
            .flex_col()
            .gap(px(GAP))
            .child(Section::new("5", "Editor look"))
            .child(aside(&t, "How ntex colors your .tex files while you type (not the PDF)."))
            .child(self.render_saved(cx))
            .child(controls)
            .child(self.render_structure(cx))
            .child(shell::label("Sample", cx))
            .child(sample(&t, &self.look))
            .child(aside(&t, palette_note(&self.look.palette)))
            .child(self.render_colors(cx))
    }

    /// The fine structure settings: glyphs, which rules and lines show, shades.
    fn render_structure(&mut self, cx: &mut Context<Studio>) -> impl IntoElement {
        let mut rows = div().flex().flex_col().gap(px(4.));
        for (key, label, options) in look::STRUCTURE_CHOICES {
            let key = *key;
            let names: Vec<SharedString> = options.iter().map(|(_, l)| SharedString::from(*l)).collect();
            let this = cx.weak_entity();
            rows = rows.child(
                Radio::new(SharedString::from(format!("look.st.{key}")), *label, names, self.look.structure.index(key))
                    .label_width(px(150.))
                    .on_select(move |i, _, cx| {
                        let v = look::STRUCTURE_CHOICES.iter().find(|(k, ..)| *k == key).map(|(_, _, o)| o[i.min(o.len() - 1)].0);
                        if let Some(v) = v {
                            this.update(cx, |st, cx| st.set_look(|l| { l.structure.set(key, v); }, cx)).ok();
                        }
                    }),
            );
        }
        let this = cx.weak_entity();
        let t = cx.theme().clone();
        rows = rows.child(
            div().flex().flex_row().gap(px(12.)).child(div().w(px(150.)).flex_none().font(t.caps()).child("Block titles")).child(
                Checkbox::new("look.st.title_bold", "Bold", self.look.structure.title_bold).on_toggle(move |on, _, cx| {
                    this.update(cx, |st, cx| st.set_look(|l| l.structure.title_bold = on, cx)).ok();
                }),
            ),
        );
        for (key, label, lo, hi, unit) in look::STRUCTURE_SLIDERS {
            let (key, lo) = (*key, *lo);
            let this = cx.weak_entity();
            let ink = t.ink;
            let paper = t.paper;
            rows = rows.child(
                div().flex().flex_row().items_center().gap(px(12.)).child(div().w(px(150.)).flex_none().font(t.caps()).child(*label)).child(
                    ChannelSlider::new(SharedString::from(format!("look.st.{key}")), "", self.look.structure.number(key) as f32, *hi as f32, move |x| {
                        let mut c = ink;
                        c.a = 0.08 + 0.6 * x;
                        let _ = paper;
                        c
                    })
                    .unit(unit)
                    .on_change(move |v, _, cx| {
                        this.update(cx, |st, cx| st.set_look(|l| l.structure.set_number(key, (v.round() as u32).max(lo)), cx)).ok();
                    }),
                ),
            );
        }
        let custom = !self.look.structure.is_default();
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(shell::label("Editor structure", cx))
            .child(rows)
            .child(div().flex().flex_row().child(Button::new("look.st.reset", "Reset structure").disabled(!custom).on_click(cx.listener(|s, _, _, cx| {
                s.set_look(|l| l.structure = Default::default(), cx)
            }))))
    }

    /// Every color of the palette as a chip; the chosen one gets H, S, V
    /// sliders and a hex field. Edits belong to this palette only.
    fn render_colors(&mut self, cx: &mut Context<Studio>) -> impl IntoElement {
        let t = cx.theme().clone();
        let pal_label = look::PALETTES.iter().find(|(p, _)| *p == self.look.palette).map(|(_, l)| *l).unwrap_or("");
        let mut chips = div().flex().flex_row().flex_wrap().gap_x(px(14.)).gap_y(px(4.));
        for (i, (role, label)) in look::ROLES.iter().enumerate() {
            let on = i == self.look_page.role;
            let c = self.look.color(role);
            chips = chips.child(
                div()
                    .id(SharedString::from(format!("look.color.{role}")))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(5.))
                    .min_h(px(24.))
                    .cursor_pointer()
                    .child(div().size(px(14.)).bg(colour(c)).border_1().border_color(if on { t.blue } else { t.rule }))
                    .child(div().text_color(if on { t.blue } else { t.ink }).child(*label))
                    .when(self.look.edited(role), |d| d.child(div().text_color(t.slate).child("•")))
                    .on_click(cx.listener(move |s, _, _, cx| s.look_select(i, cx))),
            );
        }
        let role = self.look_role();
        let label = look::ROLES[self.look_page.role.min(look::ROLES.len() - 1)].1;
        let c = self.look.color(role);
        let (h, sat, v) = look::to_hsv(c);
        let this = cx.weak_entity();
        let set = move |f: Box<dyn Fn(f32, f32, f32) -> u32>| {
            let this = this.clone();
            move |x: f32, _: &mut Window, cx: &mut App| {
                let next = f(x, 0.0, 0.0);
                this.update(cx, |st, cx| {
                    let role = st.look_role();
                    st.set_look(|l| l.set_color(role, next), cx)
                })
                .ok();
            }
        };
        let hue = ChannelSlider::new("look.h", "H", h, 360., move |x| colour(look::from_hsv(x * 360., sat.max(0.35), v.max(0.55))))
            .unit("°")
            .step(2.)
            .on_change(set(Box::new(move |x, _, _| look::from_hsv(x, sat, v))));
        let satur = ChannelSlider::new("look.s", "S", sat * 100., 100., move |x| colour(look::from_hsv(h, x, v)))
            .unit("%")
            .on_change(set(Box::new(move |x, _, _| look::from_hsv(h, x / 100., v))));
        let value = ChannelSlider::new("look.v", "V", v * 100., 100., move |x| colour(look::from_hsv(h, sat, x)))
            .unit("%")
            .on_change(set(Box::new(move |x, _, _| look::from_hsv(h, sat, x / 100.))));
        let base = look::base_color(&self.look.palette, role);
        let edited = self.look.edited(role);
        let any = self.look.colors.contains_key(&self.look.palette);
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(shell::label(format!("Colors of {pal_label}"), cx))
            .child(chips)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(18.))
                    .items_start()
                    .child(div().size(px(64.)).flex_none().bg(colour(c)).border_1().border_color(t.rule))
                    .child(div().flex().flex_col().child(hue).child(satur).child(value))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(div().child(label))
                            .child(self.look_page.hex.clone())
                            .child(shell::slate(if edited { format!("{pal_label}'s own: {}", look::hex(base)) } else { format!("{pal_label}'s own color") }, cx)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(10.))
                    .child(Button::new("look.reset", "Reset this color").disabled(!edited).on_click(cx.listener(move |s, _, _, cx| {
                        s.set_look(|l| l.reset_color(role), cx)
                    })))
                    .child(Button::new("look.reset-all", format!("Reset all of {pal_label}")).disabled(!any).on_click(cx.listener(|s, _, _, cx| {
                        s.set_look(|l| l.reset_colors(), cx)
                    }))),
            )
    }
}

fn palette_note(palette: &str) -> &'static str {
    if look::is_standard(palette) {
        return "A standard theme, as its Neovim port draws LaTeX, with your structure settings on top (block colors come from the theme's own keyword, string, number, comment and type colors). Black ink doesn't apply. cmux's background and text color follow while ntex runs; a light palette of yours gives the white back.";
    }
    match palette {
        "code" => "Code: strong roles, as in a code theme. Blue math, magenta math commands, red markup, gray italic comments.",
        "code-dark" => "Code Dark: Code on a dark page. The terminal's background follows while ntex runs; a light palette gives the white back.",
        "alabaster" => "Alabaster: the look before, kept for reference.",
        _ => "Slate: one cool blue-gray for all the markup, a near-black indigo for math, ink only for what prints.",
    }
}

/// Three lines of a note in the palette's colors, with the settings applied.
fn sample(t: &Theme, l: &Look) -> impl IntoElement {
    let s = l.swatch();
    let dark = s.dark();
    let markup = if l.ink == "all" && !s.standard { s.ink } else { s.markup };
    let kinds = l.kinds != "none";
    let whole = l.lines == "whole" && kinds;
    let def = if kinds { s.definition } else if s.standard { s.env } else { markup };
    let f = &l.structure;
    let cmd = if whole { mix(def, if dark { s.bg } else { 0x000000 }, f.begin_shade as f32 / 100. * if dark { 0.79 } else { 1. }) } else { markup };
    let delim = if whole { mix(s.quiet, def, f.brace_tint as f32 / 100.) } else { s.quiet };
    let mono = t.mono();
    let mut bold = t.mono();
    bold.weight = gpui::FontWeight::BOLD;
    let mut ital = t.mono();
    ital.style = gpui::FontStyle::Italic;
    let rule = l.sections == "rule";
    let band = l.sections == "band";
    let bar = |on: bool| {
        div().w(px(10.)).flex_none().text_color(colour(s.definition)).child(if on && l.kinds == "bar" { SharedString::from(f.bar_glyph.clone()) } else { " ".into() })
    };
    let guide = |on: bool| {
        div().w(px(18.)).flex_none().text_color(colour(mix(s.definition, s.bg, (f.guide_fade as f32 - if dark { 7. } else { 0. }).min(95.) / 100.))).child(if on && l.guides { SharedString::from(f.guide_glyph.clone()) } else { " ".into() })
    };
    let line = |gutter: gpui::Div, body: gpui::StyledText| div().flex().flex_row().child(gutter).child(body);
    let heading = Rich::new()
        .span("\\section{", mono.clone(), colour(markup))
        .span("Kernels", bold.clone(), colour(s.ink))
        .span("}", mono.clone(), colour(s.quiet))
        .span(if f.numbers == "end" && l.sections != "none" { "  §1" } else { "" }, mono.clone(), colour(mix(s.ink, s.bg, 0.5)))
        .build();
    let begin = Rich::new()
        .span("\\begin", mono.clone(), colour(cmd))
        .span("{", mono.clone(), colour(delim))
        .span("definition", mono.clone(), colour(def))
        .span("}[", mono.clone(), colour(delim))
        .span("Kernel", if kinds && f.title_bold { bold.clone() } else { mono.clone() }, colour(if kinds { def } else { s.ink }))
        .span("]", mono.clone(), colour(delim))
        .build();
    let body = Rich::new()
        .span("The ", mono.clone(), colour(s.ink))
        .span("\\emph", mono.clone(), colour(markup))
        .span("{", mono.clone(), colour(s.quiet))
        .span("kernel", mono.clone(), colour(s.ink))
        .span("} ", mono.clone(), colour(s.quiet))
        .span("$", mono.clone(), colour(s.quiet))
        .span("x ", mono.clone(), colour(s.math))
        .span("\\in", mono.clone(), colour(s.mathcmd))
        .span(" G", mono.clone(), colour(s.math))
        .span("$ ", mono.clone(), colour(s.quiet))
        .span("% check", if s.comment_italic { ital.clone() } else { mono.clone() }, colour(s.comment))
        .build();
    let end = Rich::new()
        .span("\\end", mono.clone(), colour(cmd))
        .span("{", mono.clone(), colour(delim))
        .span("definition", mono.clone(), colour(def))
        .span("}", mono.clone(), colour(delim))
        .build();
    let rule_colour = mix(s.ink, s.bg, 0.75);
    let mut page = div()
        .flex()
        .flex_col()
        .px(px(12.))
        .py(px(10.))
        .bg(colour(s.bg))
        .border_1()
        .border_color(t.rule)
        .text_size(px(14.))
        .line_height(px(20.));
    if rule {
        page = page.child(div().overflow_hidden().whitespace_nowrap().font(t.mono()).text_color(colour(rule_colour)).child(f.rule_glyph.repeat(140)));
    }
    let head_row = line(bar(false), heading);
    page = page.child(if band { head_row.bg(colour(mix(s.bg, s.definition, if dark { 0.12 } else { 0.07 }))) } else { head_row });
    page.child(line(bar(true), begin))
        .child(line(div().flex().flex_row().child(bar(true)).child(guide(true)), body))
        .child(line(bar(true), end))
}
