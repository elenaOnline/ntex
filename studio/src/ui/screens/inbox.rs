//! Page 3: Inbox (UX.md "Page 3"). Owner: the SCREENS builder.
//!
//! A booktabs table of what is waiting (↑/↓ row focus, Return edits, ⌫
//! deletes, undoable), and "Sort into my files": the sorter (io::agent)
//! streams its progress into a Proof log; its plan is applied by
//! core::apply on a background thread, every touched file checked by a
//! one-shot bridge; one ⌘Z puts everything back.
use crate::core::{apply, codegen, inbox};
use crate::model::*;
use crate::ui::app::{InboxDelete, InboxDown, InboxEdit, InboxUp, INBOX_CONTEXT, Studio, local_now, safe};
use crate::ui::screens::shell::{self, GAP};
use crate::ui::widgets::boxes::{BoxKind, LogLine, Section, TBox, Table, aside, prose};
use crate::ui::widgets::controls::Button;
use crate::ui::widgets::theme::ActiveTheme as _;
use gpui::{
    AnyElement, App, Context, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};
use ntex_studio_io::agent::SortJob;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

const SORT_TIMEOUT: Duration = Duration::from_secs(180);
const SORT_LABEL: &str = "the sort";
/// Log lines kept on screen while sorting.
const LOG_SHOWN: usize = 14;

#[derive(Clone, Debug, PartialEq)]
pub enum SortStatus {
    Idle,
    Sorting,
    Applying,
    Done,
    Failed,
    Cancelled,
}

pub struct InboxPage {
    /// The table: ↑/↓, Return, ⌫.
    pub focus: FocusHandle,
    pub row: usize,
    job: Option<SortJob>,
    rx: Option<Receiver<AgentEvent>>,
    apply_rx: Option<Receiver<Result<SortOutcome, String>>>,
    /// (severity, text): progress is a Note (italic slate), errors "! …".
    pub log: Vec<(Severity, String)>,
    pub plan: Option<SortPlan>,
    pub outcome: Option<SortOutcome>,
    pub status: SortStatus,
    /// id → trigger, as the entries were when the sort started.
    triggers: HashMap<String, String>,
    started: Option<Instant>,
}

impl InboxPage {
    pub fn new(cx: &mut Context<Studio>) -> InboxPage {
        InboxPage {
            focus: cx.focus_handle().tab_stop(true),
            row: 0,
            job: None,
            rx: None,
            apply_rx: None,
            log: Vec::new(),
            plan: None,
            outcome: None,
            status: SortStatus::Idle,
            triggers: HashMap::new(),
            started: None,
        }
    }

    pub fn busy(&self) -> bool {
        matches!(self.status, SortStatus::Sorting | SortStatus::Applying)
    }

    pub fn cancel_job(&mut self) {
        if let Some(mut job) = self.job.take() {
            safe("SortJob::cancel", || job.cancel());
        }
        self.rx = None;
    }

    /// ⌘Z undid something; if it was the sort, say so in the log.
    pub fn undone(&mut self, label: &str) {
        if label == SORT_LABEL && self.status == SortStatus::Done {
            self.log.push((Severity::Note, "Undone: the snippets are back in the inbox.".into()));
            self.status = SortStatus::Idle;
            self.outcome = None;
        }
    }
}

/// "claude-haiku-5-5" → "Haiku 5.5".
pub fn model_name(m: &str) -> String {
    let m = m.strip_prefix("claude-").unwrap_or(m);
    let mut parts = m.split('-');
    let name = parts.next().unwrap_or("");
    let mut cap = name.chars();
    let name = match cap.next() {
        Some(c) => c.to_uppercase().collect::<String>() + cap.as_str(),
        None => String::new(),
    };
    let ver: Vec<&str> = parts.filter(|p| p.chars().all(|c| c.is_ascii_digit()) && p.len() <= 2).collect();
    if ver.is_empty() { name } else { format!("{name} {}", ver.join(".")) }
}

fn fires_of(e: &InboxEntry) -> &'static str {
    match &e.draft {
        Some(d) if d.kind == Kind::Visual => "on a selection",
        Some(d) if d.fires == Fires::Tab => "Tab",
        Some(_) => "by itself",
        None => "Lua",
    }
}

fn trigger_of(e: &InboxEntry) -> String {
    match &e.draft {
        Some(d) if d.fires == Fires::Tab => format!("{} ⇥", e.trigger),
        _ => e.trigger.clone(),
    }
}

fn output_of(e: &InboxEntry) -> String {
    let s = match &e.draft {
        Some(d) if d.kind != Kind::RawLua => safe("codegen::plain", || codegen::plain(&d.output)).unwrap_or_default(),
        _ => e.source.trim().lines().next().unwrap_or("").to_string(),
    };
    let s = s.replace('\n', " ⏎ ").replace('\t', " ");
    let mut out: String = s.chars().take(42).collect();
    if s.chars().count() > 42 {
        out.push('…');
    }
    out
}

fn made_of(e: &InboxEntry) -> String {
    e.made.get(11..16).unwrap_or(&e.made).to_string()
}

impl Studio {
    fn inbox_move(&mut self, delta: isize, cx: &mut Context<Studio>) {
        let n = self.inbox.len();
        if n == 0 {
            return;
        }
        let r = (self.inbox_page.row as isize + delta).clamp(0, n as isize - 1);
        self.inbox_page.row = r as usize;
        cx.notify();
    }

    fn inbox_edit(&mut self, window: &mut Window, cx: &mut Context<Studio>) {
        if let Some(e) = self.inbox.get(self.inbox_page.row).cloned() {
            self.edit_entry(&e, window, cx);
        }
    }

    /// ⌫ on a row: delete it (⌘Z undoes).
    pub fn inbox_delete(&mut self, cx: &mut Context<Studio>) {
        let Some(e) = self.inbox.get(self.inbox_page.row).cloned() else { return };
        let backed = self.backup(&format!("deleting ‘{}’", e.trigger), vec![self.paths.inbox()]);
        match safe("inbox::remove", || inbox::remove(&self.paths, std::slice::from_ref(&e.id))) {
            Some(Ok(())) => {
                self.after_files_changed(cx);
                let undo = if backed.is_ok() { " ⌘Z undoes." } else { "" };
                self.toast(format!("Deleted ‘{}’ from the inbox.{undo}", e.trigger), cx);
            }
            other => {
                if backed.is_ok() {
                    self.undo.pop();
                }
                let why = match other {
                    Some(Err(e)) => e.to_string(),
                    _ => "the inbox writer is not built yet".into(),
                };
                self.toast(format!("! Could not delete ‘{}’: {why}", e.trigger), cx);
            }
        }
    }

    /// ⌘⇧S / the Sort button.
    pub fn start_sort(&mut self, cx: &mut Context<Studio>) {
        if self.inbox_page.busy() {
            return;
        }
        if self.inbox.is_empty() {
            self.toast("Nothing to sort: the inbox is empty.", cx);
            return;
        }
        let p = &mut self.inbox_page;
        p.log.clear();
        p.plan = None;
        p.outcome = None;
        p.triggers = self.inbox.iter().map(|e| (e.id.clone(), trigger_of(e))).collect();
        match safe("SortJob::start", || SortJob::start(&self.paths, &self.prefs, &self.inbox)) {
            Some(Ok((job, rx))) => {
                p.job = Some(job);
                p.rx = Some(rx);
                p.status = SortStatus::Sorting;
                p.started = Some(cx.background_executor().now());
                p.log.push((Severity::Note, format!("Asking {} where each snippet belongs …", model_name(&self.prefs.sort_model))));
            }
            Some(Err(e)) => {
                p.status = SortStatus::Failed;
                p.log.push((Severity::Error, format!("! Could not start the sorter: {e}")));
            }
            None => {
                p.status = SortStatus::Failed;
                p.log.push((Severity::Error, "! The sorter is not built yet.".into()));
            }
        }
        cx.notify();
    }

    fn cancel_sort(&mut self, cx: &mut Context<Studio>) {
        if self.inbox_page.status == SortStatus::Sorting {
            self.inbox_page.cancel_job();
            self.inbox_page.status = SortStatus::Cancelled;
            self.inbox_page.log.push((Severity::Note, "Cancelled. Nothing was moved.".into()));
            cx.notify();
        }
    }

    pub fn pump_inbox(&mut self, now: Instant, cx: &mut Context<Studio>) -> bool {
        let mut changed = false;
        let mut plan_in: Option<SortPlan> = None;
        if let Some(rx) = &self.inbox_page.rx {
            loop {
                match rx.try_recv() {
                    Ok(AgentEvent::Progress(line)) => {
                        let line = line.trim().to_string();
                        if !line.is_empty() {
                            self.inbox_page.log.push((Severity::Note, line));
                        }
                        changed = true;
                    }
                    Ok(AgentEvent::Plan(plan)) => {
                        plan_in = Some(plan);
                        break;
                    }
                    Ok(AgentEvent::Failed(e)) => {
                        self.inbox_page.status = SortStatus::Failed;
                        self.inbox_page.log.push((Severity::Error, format!("! {e}")));
                        self.inbox_page.rx = None;
                        self.inbox_page.job = None;
                        changed = true;
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        if self.inbox_page.status == SortStatus::Sorting {
                            self.inbox_page.status = SortStatus::Failed;
                            self.inbox_page.log.push((Severity::Error, "! The sorter stopped without a plan.".into()));
                        }
                        self.inbox_page.rx = None;
                        self.inbox_page.job = None;
                        changed = true;
                        break;
                    }
                }
            }
        }
        if self.inbox_page.status == SortStatus::Sorting
            && self.inbox_page.started.is_some_and(|t| now.saturating_duration_since(t) > SORT_TIMEOUT)
        {
            self.inbox_page.cancel_job();
            self.inbox_page.status = SortStatus::Failed;
            self.inbox_page.log.push((Severity::Error, "! The sorter took more than 3 minutes; stopped it. Nothing was moved.".into()));
            changed = true;
        }
        if let Some(plan) = plan_in {
            self.apply_plan(plan);
            changed = true;
        }
        if let Some(rx) = &self.inbox_page.apply_rx {
            match rx.try_recv() {
                Ok(res) => {
                    self.inbox_page.apply_rx = None;
                    self.finish_sort(res, cx);
                    changed = true;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.inbox_page.apply_rx = None;
                    self.finish_sort(Err("the mover stopped".into()), cx);
                    changed = true;
                }
            }
        }
        changed
    }

    /// The plan is in: apply it on a thread (backup, move, check each file).
    fn apply_plan(&mut self, plan: SortPlan) {
        let p = &mut self.inbox_page;
        p.rx = None;
        p.job = None;
        p.status = SortStatus::Applying;
        let n = plan.moves.len();
        p.log.push((Severity::Note, format!("Moving {n} snippet{} and checking every file …", if n == 1 { "" } else { "s" })));
        p.plan = Some(plan.clone());
        let paths = self.paths.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        p.apply_rx = Some(rx);
        std::thread::spawn(move || {
            let (_, stamp) = local_now();
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut check = |path: &std::path::Path, text: &str| {
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    ntex_studio_io::bridge::check_file_once(&paths, &name, text)
                };
                apply::apply(&paths, &plan, &stamp, &mut check)
            }));
            let res = match res {
                Ok(Ok(outcome)) => Ok(outcome),
                Ok(Err(e)) => Err(e.to_string()),
                Err(_) => Err("the mover is not built yet".into()),
            };
            let _ = tx.send(res);
        });
    }

    fn finish_sort(&mut self, res: Result<SortOutcome, String>, cx: &mut Context<Studio>) {
        match res {
            Ok(outcome) => {
                let mut files: Vec<PathBuf> = vec![self.paths.inbox()];
                for (_, f) in &outcome.moved {
                    let p = self.paths.snippets.join("tex").join(f);
                    if !files.contains(&p) {
                        files.push(p);
                    }
                }
                self.undo.push(crate::ui::app::UndoItem::Files { label: SORT_LABEL.into(), backup: outcome.backup.clone(), files });
                self.redo.clear();
                let n = outcome.moved.len();
                let p = &mut self.inbox_page;
                for (id, why) in &outcome.kept {
                    let t = p.triggers.get(id).cloned().unwrap_or(id.clone());
                    p.log.push((Severity::Warning, format!("‘{t}’ stayed in the inbox: {why}")));
                }
                p.log.push((Severity::Note, format!("Moved {n} snippet{}.", if n == 1 { "" } else { "s" })));
                p.status = SortStatus::Done;
                p.outcome = Some(outcome);
                self.after_files_changed(cx);
            }
            Err(e) => {
                let p = &mut self.inbox_page;
                p.status = SortStatus::Failed;
                p.log.push((Severity::Error, format!("! {e}")));
                p.log.push((Severity::Note, "Everything was restored.".into()));
                self.reload_inbox();
            }
        }
    }

    pub fn inbox_hints(&self, window: &Window, _cx: &App) -> Vec<SharedString> {
        let mut v: Vec<SharedString> = Vec::new();
        if self.inbox_page.focus.is_focused(window) && !self.inbox.is_empty() {
            v.push("↑/↓ choose a snippet · Return edits it · ⌫ deletes it (⌘Z undoes).".into());
        }
        v.push("⌘⇧S sorts the inbox into your files; one ⌘Z puts everything back.".into());
        v
    }

    pub fn render_inbox(&mut self, window: &mut Window, cx: &mut Context<Studio>) -> impl IntoElement {
        let t = cx.theme().clone();
        let n = self.inbox.len();
        let title = match n {
            0 => "Inbox".to_string(),
            1 => "Inbox — 1 snippet waiting".to_string(),
            n => format!("Inbox — {n} snippets waiting"),
        };
        let focused = self.inbox_page.focus.is_focused(window);
        let mono = |s: String| div().font(t.mono()).text_size(px(14.)).child(s);

        let table: AnyElement = if let Some(err) = &self.inbox_error {
            LogLine { severity: Severity::Error, text: format!("! Could not read inbox.lua: {err}").into() }.into_any_element()
        } else if n == 0 {
            div()
                .italic()
                .text_color(t.slate)
                .pt(px(6.))
                .child("Nothing waiting. New snippets land here first; sort them into your files when you have a minute.")
                .into_any_element()
        } else {
            let rows: Vec<Vec<AnyElement>> = self
                .inbox
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    let sel = i == self.inbox_page.row;
                    let mark = div()
                        .id(SharedString::from(format!("inbox.row.{i}")))
                        .w(px(12.))
                        .text_color(if focused { t.blue } else { t.slate })
                        .child(if sel { "▸" } else { " " })
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.inbox_page.row = i;
                            s.inbox_page.focus.clone().focus(w, cx);
                            cx.notify();
                        }));
                    let cell = |el: gpui::Div, id: &str| {
                        el.id(SharedString::from(format!("inbox.{id}.{i}"))).cursor_pointer().on_click(cx.listener(
                            move |s, ev: &gpui::ClickEvent, w, cx| {
                                s.inbox_page.row = i;
                                s.inbox_page.focus.clone().focus(w, cx);
                                if ev.click_count() >= 2 {
                                    s.inbox_edit(w, cx);
                                }
                                cx.notify();
                            },
                        ))
                    };
                    vec![
                        mark.into_any_element(),
                        cell(mono(trigger_of(e)), "trigger").into_any_element(),
                        cell(mono(output_of(e)), "output").into_any_element(),
                        cell(div().child(fires_of(e)), "fires").into_any_element(),
                        cell(div().child(made_of(e)), "made").into_any_element(),
                    ]
                })
                .collect();
            Table {
                header: vec!["".into(), "Trigger".into(), "Output".into(), "Fires".into(), "Made".into()],
                rows,
                widths: Some(vec![0.03, 0.17, 0.5, 0.16, 0.14]),
            }
            .into_any_element()
        };
        let table = div()
            .id("inbox.table")
            .key_context(INBOX_CONTEXT)
            .track_focus(&self.inbox_page.focus)
            .on_action(cx.listener(|s, _: &InboxUp, _, cx| s.inbox_move(-1, cx)))
            .on_action(cx.listener(|s, _: &InboxDown, _, cx| s.inbox_move(1, cx)))
            .on_action(cx.listener(|s, _: &InboxEdit, w, cx| s.inbox_edit(w, cx)))
            .on_action(cx.listener(|s, _: &InboxDelete, _, cx| s.inbox_delete(cx)))
            .child(table);

        let busy = self.inbox_page.busy();
        let sorting = self.inbox_page.status == SortStatus::Sorting;
        let buttons = div()
            .flex()
            .flex_row()
            .justify_end()
            .items_center()
            .gap(px(14.))
            .when(sorting, |d| d.child(Button::new("sort.cancel", "Cancel").on_click(cx.listener(|s, _, _, cx| s.cancel_sort(cx)))))
            .child(
                Button::new("sort", "Sort into my files")
                    .keys("⌘⇧S")
                    .primary()
                    .disabled(busy || n == 0)
                    .on_click(cx.listener(|s, _, _, cx| s.start_sort(cx))),
            )
            .child(
                div()
                    .italic()
                    .text_color(t.slate)
                    .text_size(px(13.))
                    .child(format!("{} · {}", model_name(&self.prefs.sort_model), self.prefs.sort_effort)),
            );

        let proof = (!self.inbox_page.log.is_empty() || self.inbox_page.plan.is_some()).then(|| self.render_sort_log(cx));

        div()
            .flex()
            .flex_col()
            .gap(px(GAP))
            .child(Section { number: "3".into(), title: title.into() })
            .child(table)
            .child(buttons)
            .children(proof)
    }

    fn render_sort_log(&self, cx: &mut Context<Studio>) -> impl IntoElement {
        let t = cx.theme().clone();
        let p = &self.inbox_page;
        let skip = p.log.len().saturating_sub(LOG_SHOWN);
        let mut lines: Vec<AnyElement> = Vec::new();
        if skip > 0 {
            lines.push(shell::slate(format!("… {skip} earlier lines"), cx).into_any_element());
        }
        // progress first, then the plan, then the outcome lines
        let (outcome_lines, progress): (Vec<_>, Vec<_>) = p.log.iter().skip(skip).partition(|(sev, text)| {
            *sev != Severity::Note || text.starts_with("Moved ") || text.starts_with("Everything") || text.starts_with("Undone")
        });
        for (_, text) in progress {
            lines.push(shell::slate(text.clone(), cx).into_any_element());
        }
        if let Some(plan) = &p.plan {
            let mono = |s: String| div().font(t.mono()).text_size(px(14.)).child(s);
            let rows = plan.moves.iter().map(|m| {
                let trig = p
                    .triggers
                    .get(&m.id)
                    .cloned()
                    .or_else(|| self.inbox.iter().find(|e| e.id == m.id).map(trigger_of))
                    .unwrap_or(m.id.clone());
                let target = match (&m.after_trigger, &m.new_title) {
                    (_, Some(title)) => format!("→ {} (new: {title})", m.file),
                    (Some(after), None) => format!("→ {}, after {after}", m.file),
                    (None, None) => format!("→ {}", m.file),
                };
                div()
                    .flex()
                    .flex_row()
                    .gap(px(14.))
                    .child(div().w(px(90.)).flex_none().child(mono(trig)))
                    .child(div().w(px(260.)).flex_none().child(mono(target)))
                    .child(div().flex_1().min_w_0().child(aside(&t, m.reason.clone())))
                    .into_any_element()
            });
            lines.extend(rows);
            if !plan.notes.trim().is_empty() {
                lines.push(shell::slate(plan.notes.trim().to_string(), cx).into_any_element());
            }
        }
        for (sev, text) in outcome_lines {
            let line = if *sev == Severity::Note {
                div().child(prose(&t, text, t.serif(), t.ink)).into_any_element()
            } else {
                LogLine { severity: *sev, text: text.clone().into() }.into_any_element()
            };
            lines.push(line);
        }
        let done = p.status == SortStatus::Done && p.outcome.is_some();
        TBox::new(BoxKind::Proof, "Proof").name("Sorting").child(
            div()
                .flex()
                .flex_col()
                .gap(px(3.))
                .pt(px(2.))
                .children(lines)
                .when(done, |d| {
                    d.child(div().flex().flex_row().pt(px(4.)).child(
                        Button::new("sort.undo", "Undo").keys("⌘Z").on_click(cx.listener(|s, _, w, cx| s.undo_last(w, cx))),
                    ))
                }),
        )
    }
}
