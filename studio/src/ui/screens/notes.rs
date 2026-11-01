//! Page 6: Notes. How a note's PDF looks: the typeface, line spacing, how the
//! theorem-like boxes are drawn, and every color of the notes palette (chips
//! + H/S/V + hex), with saved notes looks to press. Changes are written at
//! once (core::notes: notes-look.json and the two files ntex-base.sty reads)
//! and show at each note's next compile. Owner: the SCREENS builder.
use crate::core::looks;
use crate::core::notes::{self, COLORS, FONTS, NotesLook, STYLE_CHOICES};
use crate::ui::app::Studio;
use crate::ui::screens::shell::{self, GAP};
use crate::ui::widgets::boxes::{Section, aside};
use crate::ui::widgets::controls::{Button, ChannelSlider, Radio};
use crate::ui::widgets::text_field::{TextField, TextFieldEvent};
use crate::ui::widgets::theme::ActiveTheme as _;
use gpui::{
    App, AppContext as _, Context, Entity, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, prelude::FluentBuilder as _, px, rgb,
};

fn colour(c: u32) -> Hsla {
    rgb(c).into()
}

pub struct NotesPage {
    /// Index into `notes::COLORS`.
    pub role: usize,
    pub hex: Entity<TextField>,
    pub name: Entity<TextField>,
    /// The saved notes look last pressed (Rename / Delete act on it).
    pub selected: Option<String>,
}

impl NotesPage {
    pub fn new(window: &mut Window, cx: &mut Context<Studio>, subs: &mut Vec<Subscription>) -> NotesPage {
        let hex = cx.new(|cx| TextField::new(cx).mono().width(px(96.)).placeholder("#rrggbb"));
        subs.push(cx.subscribe_in(&hex, window, |s: &mut Studio, _, ev: &TextFieldEvent, window, cx| match ev {
            TextFieldEvent::Submit | TextFieldEvent::Blur => s.notes_hex_entered(cx),
            TextFieldEvent::Cancel => {
                s.notes_sync_hex(cx);
                s.focus.clone().focus(window, cx);
            }
            TextFieldEvent::Changed => {}
        }));
        let name = cx.new(|cx| TextField::new(cx).width(px(220.)).placeholder("a name for this look"));
        subs.push(cx.subscribe_in(&name, window, |s: &mut Studio, _, ev: &TextFieldEvent, _, cx| {
            if let TextFieldEvent::Submit = ev {
                s.notes_save_as(cx)
            }
        }));
        NotesPage { role: 0, hex, name, selected: None }
    }
}

impl Studio {
    /// The page was opened: notes-look.json and looks.json may have changed.
    pub fn reload_notes(&mut self) {
        self.notes = notes::read(&self.paths);
        self.look_page.looks = looks::read(&self.paths);
    }

    fn notes_role(&self) -> &'static str {
        COLORS[self.notes_page.role.min(COLORS.len() - 1)].0
    }

    pub fn notes_sync_hex(&mut self, cx: &mut Context<Studio>) {
        let c = crate::core::look::hex(self.notes.color(self.notes_role()));
        self.notes_page.hex.update(cx, |f, cx| {
            if f.text != c {
                f.set_text(c, cx);
            }
        });
    }

    /// Change the notes look and write it (and the theme's two files).
    pub fn set_notes(&mut self, change: impl FnOnce(&mut NotesLook), cx: &mut Context<Studio>) {
        let before = self.notes.clone();
        change(&mut self.notes);
        if self.notes == before {
            return;
        }
        if let Err(e) = notes::write(&self.paths, &self.notes) {
            self.notes = before;
            self.toast(format!("! Could not write the notes look: {e}"), cx);
        }
        self.notes_sync_hex(cx);
        cx.notify();
    }

    pub fn notes_hex_entered(&mut self, cx: &mut Context<Studio>) {
        let text = self.notes_page.hex.read(cx).text.clone();
        let role = self.notes_role();
        match crate::core::look::parse_hex(&text) {
            Some(c) => self.set_notes(|n| n.set_color(role, c), cx),
            None => self.toast(format!("! {text} is not a color: write it as #rrggbb."), cx),
        }
        self.notes_sync_hex(cx);
    }

    fn notes_write_looks(&mut self, what: &str, cx: &mut Context<Studio>) {
        match looks::write(&self.paths, &self.look_page.looks) {
            Ok(()) => self.toast(what.to_string(), cx),
            Err(e) => self.toast(format!("! Could not write looks.json: {e}"), cx),
        }
        cx.notify();
    }

    pub fn notes_restore(&mut self, name: &str, cx: &mut Context<Studio>) {
        let Some(saved) = self.look_page.looks.notes_all().into_iter().find(|s| s.name == name) else { return };
        self.notes_page.selected = Some(name.to_string());
        self.notes_page.name.update(cx, |f, cx| f.set_text(name, cx));
        self.set_notes(|n| *n = saved.look.clone(), cx);
        self.toast(format!("Back to “{name}” (each note shows it at its next compile)."), cx);
    }

    pub fn notes_save_as(&mut self, cx: &mut Context<Studio>) {
        let name = self.notes_page.name.read(cx).text.trim().to_string();
        let look = self.notes.clone();
        match self.look_page.looks.save_notes(&name, &look) {
            Ok(()) => {
                self.notes_page.selected = Some(name.clone());
                self.notes_write_looks(&format!("Saved the notes look as “{name}”."), cx);
            }
            Err(e) => self.toast(format!("! {e}"), cx),
        }
    }

    pub fn notes_rename(&mut self, cx: &mut Context<Studio>) {
        let Some(from) = self.notes_page.selected.clone() else { return };
        let to = self.notes_page.name.read(cx).text.trim().to_string();
        match self.look_page.looks.rename_notes(&from, &to) {
            Ok(()) => {
                self.notes_page.selected = Some(to.clone());
                self.notes_write_looks(&format!("Renamed “{from}” to “{to}”."), cx);
            }
            Err(e) => self.toast(format!("! {e}"), cx),
        }
    }

    pub fn notes_delete(&mut self, cx: &mut Context<Studio>) {
        let Some(name) = self.notes_page.selected.clone() else { return };
        match self.look_page.looks.delete_notes(&name) {
            Ok(()) => {
                self.notes_page.selected = None;
                self.notes_write_looks(&format!("Deleted “{name}”."), cx);
            }
            Err(e) => self.toast(format!("! {e}"), cx),
        }
    }

    pub fn notes_hints(&self, _window: &Window, _cx: &App) -> Vec<SharedString> {
        vec![
            "←/→ change the focused setting; Tab moves to the next one.".into(),
            "Each note shows the change at its next compile (save it, or pause typing with live PDF on).".into(),
        ]
    }

    pub fn render_notes(&mut self, _window: &mut Window, cx: &mut Context<Studio>) -> impl IntoElement {
        let t = cx.theme().clone();
        div()
            .flex()
            .flex_col()
            .gap(px(GAP))
            .child(Section::new("6", "Notes look"))
            .child(aside(&t, "How your notes look in the PDF: the typeface, the boxes, the colors. Homework formats that load the notes theme follow too."))
            .child(self.render_notes_saved(cx))
            .child(self.render_notes_type(cx))
            .child(self.render_notes_boxes(cx))
            .child(shell::label("Sample", cx))
            .child(notes_sample(&self.notes, cx))
            .child(self.render_notes_colors(cx))
    }

    fn render_notes_saved(&mut self, cx: &mut Context<Studio>) -> impl IntoElement {
        let t = cx.theme().clone();
        let mut buttons = div().flex().flex_row().flex_wrap().gap(px(8.));
        for (i, saved) in self.look_page.looks.notes_all().iter().enumerate() {
            let name = saved.name.clone();
            let label = if saved.protected { format!("{} ·", saved.name) } else { saved.name.clone() };
            let mut b = Button::new(SharedString::from(format!("notes.saved.{i}")), label).on_click(cx.listener(move |s, _, _, cx| s.notes_restore(&name, cx)));
            if saved.look == self.notes {
                b = b.primary();
            }
            buttons = buttons.child(b);
        }
        let sel = self.notes_page.selected.clone();
        let editable = sel.as_ref().is_some_and(|n| self.look_page.looks.notes.iter().any(|s| &s.name == n && !s.protected));
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(shell::label("Saved notes looks", cx))
            .child(buttons)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .child(self.notes_page.name.clone())
                    .child(Button::new("notes.save-as", "Save current as").on_click(cx.listener(|s, _, _, cx| s.notes_save_as(cx))))
                    .child(Button::new("notes.rename", "Rename").disabled(!editable).on_click(cx.listener(|s, _, _, cx| s.notes_rename(cx))))
                    .child(Button::new("notes.delete", "Delete").disabled(!editable).on_click(cx.listener(|s, _, _, cx| s.notes_delete(cx)))),
            )
            .child(aside(&t, "Press a look to go back to it (doubled: the one you're on). Looks marked · are protected."))
    }

    fn render_notes_type(&mut self, cx: &mut Context<Studio>) -> impl IntoElement {
        let t = cx.theme().clone();
        let this = cx.weak_entity();
        let fonts: Vec<SharedString> = FONTS.iter().map(|(_, l, _)| SharedString::from(*l)).collect();
        let radio = Radio::new("notes.font", "Face", fonts, self.notes.index("font")).label_width(px(130.)).on_select(move |i, _, cx| {
            let v = FONTS[i.min(FONTS.len() - 1)].0;
            this.update(cx, |st, cx| st.set_notes(|n| { n.set("font", v); }, cx)).ok();
        });
        let this = cx.weak_entity();
        let ink = t.ink;
        let spacing = ChannelSlider::new("notes.spacing", "", self.notes.spacing as f32, 150., move |x| {
            let mut c = ink;
            c.a = 0.08 + 0.6 * x;
            c
        })
        .format(|v| format!("{:.2}", v / 100.))
        .on_change(move |v, _, cx| {
            this.update(cx, |st, cx| st.set_notes(|n| n.spacing = (v.round() as u32).clamp(90, 150), cx)).ok();
        });
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(shell::label("Typeface", cx))
            .child(radio)
            .child(div().flex().flex_row().items_center().gap(px(12.)).child(div().w(px(130.)).flex_none().font(t.caps()).child("Line spacing")).child(spacing))
    }

    fn render_notes_boxes(&mut self, cx: &mut Context<Studio>) -> impl IntoElement {
        let t = cx.theme().clone();
        let mut rows = div().flex().flex_col().gap(px(4.));
        for (key, label, options) in STYLE_CHOICES {
            let key = *key;
            let names: Vec<SharedString> = options.iter().map(|(_, l)| SharedString::from(*l)).collect();
            let this = cx.weak_entity();
            rows = rows.child(Radio::new(SharedString::from(format!("notes.{key}")), *label, names, self.notes.index(key)).label_width(px(130.)).on_select(move |i, _, cx| {
                let v = STYLE_CHOICES.iter().find(|(k, ..)| *k == key).map(|(_, _, o)| o[i.min(o.len() - 1)].0);
                if let Some(v) = v {
                    this.update(cx, |st, cx| st.set_notes(|n| { n.set(key, v); }, cx)).ok();
                }
            }));
        }
        let this = cx.weak_entity();
        let rule = colour(self.notes.color("ntexBlueRule"));
        rows = rows.child(
            div().flex().flex_row().items_center().gap(px(12.)).child(div().w(px(130.)).flex_none().font(t.caps()).child("Bar width")).child(
                ChannelSlider::new("notes.bar", "", self.notes.bar as f32, 80., move |x| {
                    let mut c = rule;
                    c.a = 0.15 + 0.85 * x;
                    c
                })
                .format(|v| format!("{:.1} pt", v.round() / 10.))
                .step(5.)
                .on_change(move |v, _, cx| {
                    this.update(cx, |st, cx| st.set_notes(|n| n.bar = v.round() as u32, cx)).ok();
                }),
            ),
        );
        let custom = {
            let d = NotesLook::default();
            (self.notes.bar, &self.notes.fill, &self.notes.frame, &self.notes.corners, &self.notes.heads) != (d.bar, &d.fill, &d.frame, &d.corners, &d.heads)
        };
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(shell::label("Theorem-like boxes", cx))
            .child(rows)
            .child(div().flex().flex_row().child(Button::new("notes.boxes.reset", "Reset boxes").disabled(!custom).on_click(cx.listener(|s, _, _, cx| {
                s.set_notes(|n| {
                    let d = NotesLook::default();
                    n.bar = d.bar;
                    n.fill = d.fill;
                    n.frame = d.frame;
                    n.corners = d.corners;
                    n.heads = d.heads;
                }, cx)
            }))))
    }

    fn render_notes_colors(&mut self, cx: &mut Context<Studio>) -> impl IntoElement {
        let t = cx.theme().clone();
        let mut chips = div().flex().flex_row().flex_wrap().gap_x(px(14.)).gap_y(px(4.));
        for (i, (name, label, _)) in COLORS.iter().enumerate() {
            let on = i == self.notes_page.role;
            chips = chips.child(
                div()
                    .id(SharedString::from(format!("notes.color.{name}")))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(5.))
                    .min_h(px(24.))
                    .cursor_pointer()
                    .child(div().size(px(14.)).bg(colour(self.notes.color(name))).border_1().border_color(if on { t.blue } else { t.rule }))
                    .child(div().text_color(if on { t.blue } else { t.ink }).child(*label))
                    .when(self.notes.edited(name), |d| d.child(div().text_color(t.slate).child("•")))
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.notes_page.role = i;
                        s.notes_sync_hex(cx);
                        cx.notify();
                    })),
            );
        }
        let role = self.notes_role();
        let label = COLORS[self.notes_page.role.min(COLORS.len() - 1)].1;
        let c = self.notes.color(role);
        let (h, sat, v) = crate::core::look::to_hsv(c);
        let this = cx.weak_entity();
        let set = move |f: Box<dyn Fn(f32) -> u32>| {
            let this = this.clone();
            move |x: f32, _: &mut Window, cx: &mut App| {
                let next = f(x);
                this.update(cx, |st, cx| {
                    let role = st.notes_role();
                    st.set_notes(|n| n.set_color(role, next), cx)
                })
                .ok();
            }
        };
        use crate::core::look::from_hsv;
        let hue = ChannelSlider::new("notes.h", "H", h, 360., move |x| colour(from_hsv(x * 360., sat.max(0.35), v.max(0.55))))
            .unit("°")
            .step(2.)
            .on_change(set(Box::new(move |x| from_hsv(x, sat, v))));
        let satur = ChannelSlider::new("notes.s", "S", sat * 100., 100., move |x| colour(from_hsv(h, x, v)))
            .unit("%")
            .on_change(set(Box::new(move |x| from_hsv(h, x / 100., v))));
        let value = ChannelSlider::new("notes.v", "V", v * 100., 100., move |x| colour(from_hsv(h, sat, x)))
            .unit("%")
            .on_change(set(Box::new(move |x| from_hsv(h, sat, x / 100.))));
        let edited = self.notes.edited(role);
        let any = !self.notes.colors.is_empty();
        let base = notes::base_color(role);
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(shell::label("Colors of the notes", cx))
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
                            .child(self.notes_page.hex.clone())
                            .child(shell::slate(if edited { format!("the theme's own: {}", crate::core::look::hex(base)) } else { "the theme's own color".to_string() }, cx)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(10.))
                    .child(Button::new("notes.reset", "Reset this color").disabled(!edited).on_click(cx.listener(move |s, _, _, cx| {
                        s.set_notes(|n| { n.colors.remove(role); }, cx)
                    })))
                    .child(Button::new("notes.reset-all", "Reset all colors").disabled(!any).on_click(cx.listener(|s, _, _, cx| {
                        s.set_notes(|n| n.colors.clear(), cx)
                    }))),
            )
    }
}

/// A theorem, a definition and a proof drawn with the notes look (an
/// approximation: the PDF is the real thing).
fn notes_sample(n: &NotesLook, cx: &App) -> impl IntoElement {
    let t = cx.theme();
    let ink = colour(n.color("ntexInk"));
    let block = |kind: &str, head: &str, body: &str, proof: bool| {
        let (rule, back, headc) = if proof {
            (n.color("ntexProofRule"), n.color("ntexProofBack"), n.color("ntexInk"))
        } else {
            (n.color(&format!("ntex{kind}Rule")), n.color(&format!("ntex{kind}Back")), n.color(&format!("ntex{kind}Head")))
        };
        let bar = if proof { 1.0 } else { n.bar as f32 / 10. * 1.33 };
        let fill = if n.fill == "none" && !proof { 0xffffff } else { back };
        let head_colour = if n.heads == "ink" && !proof { n.color("ntexInk") } else { headc };
        div()
            .flex()
            .flex_row()
            .bg(colour(fill))
            .when(n.frame == "box" && !proof, |d| d.border_1().border_color(colour(rule)))
            .when(n.corners == "rounded" && !proof, |d| d.rounded(px(5.)))
            .child(div().w(px(bar)).flex_none().bg(colour(rule)))
            .child(
                div()
                    .px(px(12.))
                    .py(px(7.))
                    .text_color(ink)
                    .child(div().flex().flex_row().gap(px(6.)).child(div().font(t.serif_bold()).text_color(colour(head_colour)).child(head.to_string())).child(div().when(!proof && kind == "Blue", |d| d.font(t.serif_italic())).child(body.to_string()))),
            )
    };
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .p(px(14.))
        .bg(gpui::white())
        .border_1()
        .border_color(t.rule)
        .child(div().flex().flex_row().gap(px(8.)).font(t.serif_bold()).child(div().text_color(colour(n.color("ntexBlue"))).child("2")).child(div().text_color(ink).child("Quotient groups")))
        .child(block("Blue", "Theorem 2.1.", "If N is normal in G, then G/N is a group.", false))
        .child(block("Teal", "Definition 2.2.", "The kernel of φ is {g ∈ G : φ(g) = e}.", false))
        .child(block("", "Proof.", "Cosets multiply well because N is normal. □", true))
        .child(aside(t, "Box shapes and colors only: the typeface shows in the PDF."))
}
