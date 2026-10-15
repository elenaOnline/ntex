//! Page 4: Library. Every snippet in her files as "you type" → "you get",
//! grouped by file (each file's group title is its category), with a fuzzy
//! search over triggers, outputs, descriptions and categories. Return (or a
//! double click, or Edit) opens the snippet on page 1, and ⌘↵ there writes it
//! back into its own file, in place (`core::library`).
//!
//! "You get" is the output rendered, as page 1 renders it: one tectonic
//! render at a time in the background, cached in state/render-cache, so the
//! first visit fills in over a few minutes and later visits are instant. The
//! LaTeX source shows until (or if) a render exists.
use crate::core::codegen;
use crate::core::library::{self, LibEntry, Library};
use crate::model::*;
use crate::ui::app::{LibDown, LibOpen, LibUp, LIBRARY_CONTEXT, Page, Studio, decode_png, safe};
use crate::ui::screens::shell::{self, GAP};
use crate::ui::screens::snippet::Home;
use crate::ui::widgets::boxes::{LogLine, Section, sup};
use crate::ui::widgets::controls::Button;
use crate::ui::widgets::text_field::{TextField, TextFieldEvent};
use crate::ui::widgets::theme::ActiveTheme as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderImage, ScrollHandle, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, img, prelude::FluentBuilder as _, px, relative,
};
use ntex_studio_io::render::{RenderJob, Rendered, Renderer, SCALE};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant, SystemTime};

/// How often the snippet files' mtimes are checked while the page is open.
const WATCH_EVERY: Duration = Duration::from_secs(1);
/// PNG decodes in flight at once.
const DECODES: usize = 6;
/// The rendered output's size on the page (the render is 10 pt type).
const THUMB_ZOOM: f32 = 1.5;
/// Never taller than this in a row (a matrix, a cases block).
const THUMB_MAX_H: f32 = 64.0;

/// A row's rendered output.
enum Thumb {
    /// Waiting for a render or a decode.
    Pending,
    Ready(Arc<RenderImage>, f32, f32),
    /// TeX refused it (or the decode failed): the row shows its source.
    Failed,
}

pub struct LibraryPage {
    pub search: Entity<TextField>,
    /// The list itself (↑/↓, Return) when the search field doesn't have focus.
    pub focus: FocusHandle,
    pub lib: Library,
    pub error: Option<String>,
    /// The category chip she picked (None: all).
    pub category: Option<String>,
    /// Indices into `lib.entries`, in display order.
    pub shown: Vec<usize>,
    /// Index into `shown`.
    pub row: usize,
    pub list_scroll: ScrollHandle,
    /// (file, mtime) of every snippet file when last read.
    pub(crate) seen: Vec<(String, Option<SystemTime>)>,
    next_watch: Option<Instant>,
    /// Rendered outputs by their cache PNG.
    thumbs: HashMap<PathBuf, Thumb>,
    /// The background renderer (its own, so page 1's renders never cancel
    /// these, nor these page 1's) and the one render in flight.
    renderer: Option<Renderer>,
    rendering: Option<(PathBuf, Receiver<Result<Rendered, String>>)>,
    decoding: Vec<(PathBuf, Receiver<Result<(Arc<RenderImage>, f32, f32), String>>)>,
    /// Scroll the selected row into view on the next frame.
    reveal: bool,
}

impl LibraryPage {
    pub fn new(window: &mut Window, cx: &mut Context<Studio>, subs: &mut Vec<Subscription>) -> LibraryPage {
        let search = cx.new(|cx| TextField::new(cx).placeholder("search: part of a trigger, the LaTeX, a description, a file …"));
        subs.push(cx.subscribe_in(&search, window, |s: &mut Studio, _, ev: &TextFieldEvent, window, cx| match ev {
            TextFieldEvent::Changed => {
                s.library.row = 0;
                s.library_filter(cx);
            }
            TextFieldEvent::Submit => s.library_open(window, cx),
            TextFieldEvent::Cancel => {
                if s.library.search.read(cx).text.is_empty() && s.library.category.is_none() {
                    s.escape(window, cx);
                } else {
                    s.library.search.update(cx, |f, cx| f.set_text("", cx));
                    s.library.category = None;
                    s.library_filter(cx);
                }
            }
            TextFieldEvent::Blur => {}
        }));
        let _ = search.read(cx).focus.clone().tab_stop(true);
        LibraryPage {
            search,
            focus: cx.focus_handle().tab_stop(true),
            lib: Library::default(),
            error: None,
            category: None,
            shown: Vec::new(),
            row: 0,
            list_scroll: ScrollHandle::new(),
            seen: Vec::new(),
            next_watch: None,
            thumbs: HashMap::new(),
            renderer: None,
            rendering: None,
            decoding: Vec::new(),
            reveal: false,
        }
    }

    pub fn selected<'a>(&'a self) -> Option<&'a LibEntry> {
        self.shown.get(self.row).and_then(|&i| self.lib.entries.get(i))
    }
}

/// The render of an entry's output (None: nothing literal to render).
fn job_of(e: &LibEntry) -> Option<RenderJob> {
    let pieces = e.pieces.as_ref()?;
    let latex = codegen::preview_latex(pieces);
    if latex.trim().is_empty() {
        return None;
    }
    // a multi-line body (an environment, an align) only sets in display maths
    let display = !e.text && (latex.contains('\n') || latex.contains("\\begin{"));
    Some(RenderJob::Snippet { latex: latex.trim().to_string(), math: !e.text, display })
}

fn fires_label(e: &LibEntry) -> String {
    let place = if e.text { "text" } else { "maths" };
    match e.fires {
        "on a selection" => "a selection".into(),
        "Lua" => "Lua".into(),
        "by itself" if e.helper == "post" || e.helper == "suffix" => "maths, after a letter".into(),
        f => format!("{place}, {f}"),
    }
}

fn type_label(e: &LibEntry) -> String {
    // post / suffix fire after what they belong to: show it typed after an x
    let t = if matches!(e.helper.as_str(), "post" | "suffix") { format!("x{}", e.trigger) } else { e.trigger.clone() };
    match e.fires {
        "Tab" => format!("{t} ⇥"),
        "on a selection" => format!("sel. {t}"),
        _ => t,
    }
}

/// The source on one short line for a row (the selected row shows it all).
fn short(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let mut out: String = s.chars().take(n).collect();
    out.push('…');
    out
}

impl Studio {
    // -- data ----------------------------------------------------------------

    fn library_mtimes(&self) -> Vec<(String, Option<SystemTime>)> {
        let dir = self.paths.snippets.join("tex");
        let mut v: Vec<(String, Option<SystemTime>)> = std::fs::read_dir(&dir)
            .map(|rd| {
                rd.flatten()
                    .filter(|e| e.file_name().to_string_lossy().ends_with(".lua"))
                    .map(|e| (e.file_name().to_string_lossy().into_owned(), e.metadata().ok().and_then(|m| m.modified().ok())))
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    }

    /// Read every snippet file again (keeps the selection on the same snippet).
    pub fn reload_library(&mut self, cx: &mut Context<Studio>) {
        let keep = self.library.selected().map(|e| (e.file.clone(), e.call.clone(), e.index));
        self.library.seen = self.library_mtimes();
        match safe("library::read", || library::read(&self.paths)) {
            Some(Ok(lib)) => {
                self.library.lib = lib;
                self.library.error = None;
            }
            Some(Err(e)) => self.library.error = Some(e.to_string()),
            None => self.library.error = Some("the snippet reader failed".into()),
        }
        if let Some(c) = &self.library.category {
            if !self.library.lib.entries.iter().any(|e| &e.category == c) {
                self.library.category = None;
            }
        }
        self.library_filter(cx);
        if let Some((file, call, index)) = keep {
            self.library_select(&file, &call, index);
        }
    }

    /// Put the selection on this snippet (same file, same text; else the same
    /// position in that file).
    fn library_select(&mut self, file: &str, call: &str, index: usize) {
        let p = &mut self.library;
        let es = &p.lib.entries;
        let at = p
            .shown
            .iter()
            .position(|&i| es[i].file == file && es[i].call == call)
            .or_else(|| p.shown.iter().position(|&i| es[i].file == file && es[i].index == index));
        if let Some(r) = at {
            p.row = r;
            p.reveal = true;
        }
    }

    /// Recompute `shown` from the query and the category.
    pub fn library_filter(&mut self, cx: &mut Context<Studio>) {
        let q = self.library.search.read(cx).text.clone();
        let p = &mut self.library;
        let ranked = library::search(&p.lib.entries, &q);
        p.shown = match &p.category {
            Some(c) => ranked.into_iter().filter(|&i| &p.lib.entries[i].category == c).collect(),
            None => ranked,
        };
        if p.row >= p.shown.len() {
            p.row = p.shown.len().saturating_sub(1);
        }
        p.reveal = true;
        cx.notify();
    }

    fn library_set_category(&mut self, c: Option<String>, cx: &mut Context<Studio>) {
        self.library.category = c;
        self.library.row = 0;
        self.library_filter(cx);
    }

    pub fn library_move(&mut self, delta: isize, cx: &mut Context<Studio>) {
        let n = self.library.shown.len();
        if n == 0 {
            return;
        }
        let r = (self.library.row as isize + delta).clamp(0, n as isize - 1);
        self.library.row = r as usize;
        self.library.reveal = true;
        cx.notify();
    }

    /// Return / Edit: open the selected snippet on page 1, saving back here.
    pub fn library_open(&mut self, window: &mut Window, cx: &mut Context<Studio>) {
        let Some(e) = self.library.selected().cloned() else { return };
        self.edit_library(&e, window, cx);
    }

    pub fn edit_library(&mut self, e: &LibEntry, window: &mut Window, cx: &mut Context<Studio>) {
        let d = e.draft.clone().unwrap_or_else(|| Draft {
            kind: Kind::RawLua,
            raw_lua: format!("{},", e.call),
            trigger: if e.trigger == "?" { String::new() } else { e.trigger.clone() },
            ..Draft::default()
        });
        if !self.snippet_is_empty() && self.snippet.home.is_none() {
            self.undo.push(crate::ui::app::UndoItem::ClearedDraft(self.snippet.draft.clone()));
        }
        self.load_draft(d, true, window, cx);
        self.snippet.home = Some(Home {
            loc: e.loc(),
            category: e.category.clone(),
            line: e.line,
            trigger: e.draft.as_ref().map(|d| d.trigger.clone()).unwrap_or_else(|| e.trigger.clone()),
        });
        self.snippet_rechecked(cx);
        self.set_page(Page::Snippet, window, cx);
    }

    /// After a save or a cancel on page 1: back to the list, on that snippet.
    pub fn library_after_edit(&mut self, home: &Home, window: &mut Window, cx: &mut Context<Studio>) {
        self.reload_library(cx);
        // after a save the call text changed: find it by position
        self.library_select(&home.loc.file, &home.loc.call, home.loc.index);
        self.set_page(Page::Library, window, cx);
    }

    // -- the poll loop -------------------------------------------------------

    /// The page's share of the poll loop: the mtime watch, the background
    /// renders, the decodes.
    pub fn pump_library(&mut self, now: Instant, cx: &mut Context<Studio>) -> bool {
        if self.page != Page::Library {
            return false;
        }
        let mut changed = false;
        if self.library.next_watch.is_none_or(|t| now >= t) {
            self.library.next_watch = Some(now + WATCH_EVERY);
            if self.library_mtimes() != self.library.seen {
                self.reload_library(cx);
                changed = true;
            }
        }
        if !self.prefs.render_preview {
            return changed;
        }
        if self.library.renderer.is_none() {
            self.library.renderer = safe("Renderer::new", || Renderer::new(&self.paths));
        }
        // borrowed out while the page's state is updated below
        let Some(renderer) = self.library.renderer.take() else { return changed };

        // a finished render: decode it
        if let Some((path, rx)) = &self.library.rendering {
            match rx.try_recv() {
                Ok(Ok(r)) => {
                    let path = path.clone();
                    self.library.decoding.push((path, decode_png(r.png, true, false)));
                    self.library.rendering = None;
                }
                Ok(Err(_)) | Err(TryRecvError::Disconnected) => {
                    let path = path.clone();
                    self.library.thumbs.insert(path, Thumb::Failed);
                    self.library.rendering = None;
                    changed = true;
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        // finished decodes
        let mut still = Vec::new();
        for (path, rx) in std::mem::take(&mut self.library.decoding) {
            match rx.try_recv() {
                Ok(Ok((image, _, _))) => {
                    let s = image.size(0);
                    let (w, h) = (s.width.0 as f32 / SCALE as f32, s.height.0 as f32 / SCALE as f32);
                    self.library.thumbs.insert(path, Thumb::Ready(image, w, h));
                    changed = true;
                }
                Ok(Err(_)) | Err(TryRecvError::Disconnected) => {
                    self.library.thumbs.insert(path, Thumb::Failed);
                    changed = true;
                }
                Err(TryRecvError::Empty) => still.push((path, rx)),
            }
        }
        self.library.decoding = still;

        // what the shown rows still need: cached PNGs are decoded right away
        // (a few at a time), the rest rendered one by one, in display order
        let p = &mut self.library;
        let mut to_render: Option<(PathBuf, RenderJob)> = None;
        for &i in &p.shown {
            if p.decoding.len() >= DECODES && (to_render.is_some() || p.rendering.is_some()) {
                break;
            }
            let Some(job) = job_of(&p.lib.entries[i]) else { continue };
            let path = renderer.cache_path(&job);
            if p.thumbs.contains_key(&path) {
                continue;
            }
            if path.exists() {
                if p.decoding.len() < DECODES {
                    p.thumbs.insert(path.clone(), Thumb::Pending);
                    p.decoding.push((path.clone(), decode_png(path, true, false)));
                }
            } else if to_render.is_none() && p.rendering.is_none() {
                to_render = Some((path, job));
            }
        }
        if let Some((path, job)) = to_render {
            if let Some(rx) = safe("render", || renderer.render(job)) {
                p.thumbs.insert(path.clone(), Thumb::Pending);
                p.rendering = Some((path, rx));
            }
        }
        p.renderer = Some(renderer);
        changed
    }

    /// How many rows have a rendered output on screen (tests).
    pub fn library_thumbs_ready(&self) -> usize {
        let p = &self.library;
        let Some(r) = p.renderer.as_ref() else { return 0 };
        p.shown
            .iter()
            .filter_map(|&i| job_of(&p.lib.entries[i]))
            .filter(|job| matches!(p.thumbs.get(&r.cache_path(job)), Some(Thumb::Ready(..))))
            .count()
    }

    // -- render --------------------------------------------------------------

    pub fn library_hints(&self, window: &Window, cx: &App) -> Vec<SharedString> {
        let p = &self.library;
        let mut v: Vec<SharedString> = Vec::new();
        if p.search.read(cx).focus.is_focused(window) || p.focus.is_focused(window) {
            v.push("Type to search · ↑/↓ choose a snippet · Return edits it (⌘↵ there saves it back into its file).".into());
            v.push("Esc clears the search · ⌘1 ⌘2 ⌘3 ⌘4 switch pages.".into());
        } else {
            v.push("⌘F searches · ⌘1 ⌘2 ⌘3 ⌘4 switch pages · ⌘W hides the studio.".into());
        }
        v
    }

    fn thumb_el(&self, e: &LibEntry, selected: bool, cx: &App) -> AnyElement {
        let t = cx.theme();
        let source = || {
            div()
                .font(t.mono())
                .text_size(px(14.))
                .when(e.pieces.is_none(), |d| d.font(t.serif_italic()).text_color(t.slate))
                .child(if selected { e.output.clone() } else { short(&e.output, 52) })
                .into_any_element()
        };
        let path = job_of(e).and_then(|job| self.library.renderer.as_ref().map(|r| r.cache_path(&job)));
        match path.and_then(|p| self.library.thumbs.get(&p)) {
            Some(Thumb::Ready(image, w, h)) => {
                let (mut w, mut h) = (*w * THUMB_ZOOM, *h * THUMB_ZOOM);
                let max_h = if selected { THUMB_MAX_H * 2.0 } else { THUMB_MAX_H };
                if h > max_h {
                    w *= max_h / h;
                    h = max_h;
                }
                div().max_w_full().overflow_hidden().child(img(image.clone()).w(px(w)).h(px(h))).into_any_element()
            }
            _ => source(),
        }
    }

    pub fn render_library(&mut self, window: &mut Window, cx: &mut Context<Studio>) -> impl IntoElement {
        let t = cx.theme().clone();
        let p = &self.library;
        let total = p.lib.entries.len();
        let files = p.lib.categories().len();
        let title = format!("Library — {total} snippets in {files} files");
        let query = p.search.read(cx).text.clone();
        let searching = !query.trim().is_empty();
        let list_focused = p.focus.is_focused(window) || p.search.read(cx).focus.is_focused(window);

        // the search line
        let search = div()
            .flex()
            .flex_row()
            .items_end()
            .gap(px(12.))
            .child(shell::label("Search", cx))
            .child(div().flex_1().min_w_0().key_context(LIBRARY_SEARCH).child(p.search.clone()))
            .child(shell::slate(
                if searching || p.category.is_some() { format!("{} found", p.shown.len()) } else { String::new() },
                cx,
            ));

        // the categories: All · Accents²⁰ · …
        let mut chips = div().flex().flex_row().flex_wrap().items_start().font(t.caps()).text_size(t.body_size);
        let cats: Vec<(Option<String>, String, usize)> = std::iter::once((None, "All".to_string(), total))
            .chain(p.lib.categories().into_iter().map(|(c, n)| (Some(c.clone()), c, n)))
            .collect();
        let ncats = cats.len();
        for (k, (c, name, n)) in cats.into_iter().enumerate() {
            let current = c == p.category;
            let colour = if current { t.ink } else { t.slate };
            let word = div().when(current, |d| d.font(t.caps()).font_weight(gpui::FontWeight::BOLD)).child(name);
            let cc = c.clone();
            chips = chips.child(
                div()
                    .id(SharedString::from(format!("library.cat.{k}")))
                    .flex()
                    .flex_row()
                    .items_start()
                    .cursor_pointer()
                    .text_color(colour)
                    .hover(|s| s.text_color(t.ink))
                    .child(word)
                    .child(sup(&t, n.to_string()).text_color(colour))
                    .on_click(cx.listener(move |s, _, _, cx| s.library_set_category(cc.clone(), cx))),
            );
            if k + 1 < ncats {
                chips = chips.child(div().font(t.serif()).text_color(t.slate).px(px(5.)).child("·"));
            }
        }

        // the list: booktabs rules, a heading per category (when not searching)
        let widths = [0.03f32, 0.22, 0.55, 0.20];
        let cell = |w: f32| div().w(relative(w)).min_w_0().pr(px(6.));
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut row_child: Vec<usize> = Vec::new(); // shown row → child index
        rows.push(
            div()
                .flex()
                .flex_row()
                .w_full()
                .pt(px(3.))
                .pb(px(3.))
                .border_b(px(0.6))
                .border_color(t.rule)
                .font(t.caps())
                .child(cell(widths[0]))
                .child(cell(widths[1]).child("You type"))
                .child(cell(widths[2]).child("You get"))
                .child(cell(widths[3]).child(if searching { "In" } else { "Fires" }))
                .into_any_element(),
        );
        let mut last_cat: Option<&str> = None;
        let entries = &p.lib.entries;
        for (r, &i) in p.shown.iter().enumerate() {
            let e = &entries[i];
            if !searching && last_cat != Some(e.category.as_str()) {
                last_cat = Some(e.category.as_str());
                rows.push(
                    div()
                        .flex()
                        .flex_row()
                        .items_baseline()
                        .gap(px(8.))
                        .pt(px(if r == 0 { 6. } else { 12. }))
                        .pb(px(2.))
                        .child(div().font(t.serif_bold()).text_color(t.blue).child(e.category.clone()))
                        .child(div().font(t.mono()).text_size(px(12.)).text_color(t.slate).child(e.file.clone()))
                        .into_any_element(),
                );
            }
            let sel = r == p.row;
            row_child.push(rows.len());
            let mark = div().text_color(if list_focused { t.blue } else { t.slate }).child(if sel { "▸" } else { " " });
            let right = if searching { e.category.clone() } else { fires_label(e) };
            let mut row = div()
                .id(SharedString::from(format!("library.row.{r}")))
                .flex()
                .flex_col()
                .w_full()
                .py(px(2.))
                .cursor_pointer()
                .when(sel, |d| d.bg(t.theorem.back))
                .on_click(cx.listener(move |s, ev: &gpui::ClickEvent, w, cx| {
                    s.library.row = r;
                    if ev.click_count() >= 2 {
                        s.library_open(w, cx);
                    } else {
                        s.library.focus.clone().focus(w, cx);
                    }
                    cx.notify();
                }))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .w_full()
                        .child(cell(widths[0]).child(mark))
                        .child(cell(widths[1]).font(t.mono()).text_size(px(15.)).child(type_label(e)))
                        .child(cell(widths[2]).child(self.thumb_el(e, sel, cx)))
                        .child(cell(widths[3]).text_size(px(13.)).text_color(t.slate).child(right)),
                );
            if sel {
                // the selected row opens: the LaTeX, what it is, where it lives, Edit
                let mut about: Vec<String> = Vec::new();
                if !e.desc.is_empty() {
                    about.push(e.desc.clone());
                }
                if !e.eg.is_empty() {
                    about.push(format!("e.g. `{}`", e.eg));
                }
                if searching {
                    about.push(fires_label(e));
                }
                about.push(format!("{}, line {}", e.file, e.line));
                if !e.origin.is_empty() {
                    about.push(format!("origin: {}", e.origin));
                }
                let label = if e.editable() { "Edit" } else { "Edit the Lua" };
                row = row.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .w_full()
                        .pt(px(3.))
                        .pb(px(4.))
                        .child(cell(widths[0]))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .when(e.pieces.is_some(), |d| {
                                    d.child(div().font(t.mono()).text_size(px(13.)).child(e.output.clone()))
                                })
                                .child(shell::slate(about.join(" · "), cx)),
                        )
                        .child(
                            div().flex_none().pl(px(8.)).child(
                                Button::new("library.edit", label)
                                    .keys("↵")
                                    .small()
                                    .on_click(cx.listener(|s, _, w, cx| s.library_open(w, cx))),
                            ),
                        ),
                );
            }
            rows.push(row.into_any_element());
        }
        if p.shown.is_empty() {
            let msg = if total == 0 { "No snippets found in snippets/tex." } else { "Nothing matches. Esc clears the search." };
            rows.push(div().pt(px(6.)).child(shell::slate(msg, cx)).into_any_element());
        }

        if p.reveal {
            if let Some(&child) = row_child.get(p.row) {
                // the heading above the first row of a category comes along
                let target = if child > 0 && !row_child.contains(&(child - 1)) { child - 1 } else { child };
                p.list_scroll.scroll_to_item(target);
            }
            self.library.reveal = false;
        }
        let p = &self.library;

        // the list scrolls by itself; the page keeps the search in view
        let list_h = (f32::from(window.viewport_size().height) - 330.0).max(220.0);
        let list = div()
            .id("library.list")
            .key_context(LIBRARY_CONTEXT)
            .track_focus(&p.focus)
            .on_action(cx.listener(|s, _: &LibUp, _, cx| s.library_move(-1, cx)))
            .on_action(cx.listener(|s, _: &LibDown, _, cx| s.library_move(1, cx)))
            .on_action(cx.listener(|s, _: &LibOpen, w, cx| s.library_open(w, cx)))
            .h(px(list_h))
            .overflow_y_scroll()
            .track_scroll(&p.list_scroll)
            .border_t(px(1.2))
            .border_b(px(1.2))
            .border_color(t.rule)
            .flex()
            .flex_col()
            .children(rows);

        let problems = p.lib.unreadable.iter().map(|u| {
            LogLine { severity: Severity::Warning, text: format!("Studio Warning: {} isn't shown: {}.", u.file, u.why).into() }.into_any_element()
        });
        let error = p.error.as_ref().map(|e| LogLine { severity: Severity::Error, text: format!("! Could not read snippets/tex: {e}").into() });

        div()
            .flex()
            .flex_col()
            .gap(px(GAP * 0.7))
            // ↑/↓/Return while typing in the search reach the list
            .on_action(cx.listener(|s, _: &LibUp, _, cx| s.library_move(-1, cx)))
            .on_action(cx.listener(|s, _: &LibDown, _, cx| s.library_move(1, cx)))
            .on_action(cx.listener(|s, _: &LibOpen, w, cx| s.library_open(w, cx)))
            .child(Section { number: "4".into(), title: title.into() })
            .child(search)
            .child(chips)
            .children(error)
            .children(problems)
            .child(list)
    }
}

/// The key context around the search field: ↑/↓/Return there move and open
/// (app.rs binds them for `LibrarySearch > TextField`, after the kit's keys).
pub const LIBRARY_SEARCH: &str = "LibrarySearch";
