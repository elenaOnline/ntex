//! Page 1: New snippet (UX.md "Page 1"). Owner: the SCREENS builder.
//!
//! Trigger → Output → Example (Try it) → Remark (Checks) → More options →
//! Proof (Generated Lua) → Clear / Save. The draft is rebuilt from the
//! widgets on every change, guessed (`infer::guess`), checked
//! (`infer::check`) and turned into Lua (`codegen::entry`); Try it (250 ms)
//! and the render (600 ms) are debounced and run off the UI thread.
use crate::core::{codegen, infer, inbox};
use crate::model::*;
use crate::model::Context as SnipContext;
use crate::ui::app::{Page, Preview, Studio, UndoItem, local_now, safe};
use crate::ui::screens::shell::{self, GAP};
use crate::ui::widgets::boxes::{BoxKind, LogLine, Section, TBox, Table};
use crate::ui::widgets::code_view::CodeView;
use crate::ui::widgets::controls::{Button, Disclosure, Radio};
use crate::ui::widgets::text_field::{TextField, TextFieldEvent};
use crate::ui::widgets::theme::ActiveTheme as _;
use crate::ui::widgets::token_editor::{TokenEditor, TokenEditorEvent, TokenEditorOffer, TokenSet, TokenToolbar};
use crate::ui::widgets::trigger_field::{TriggerEvent, TriggerField};
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, img,
    prelude::FluentBuilder as _, px,
};
use ntex_studio_io::render::RenderJob;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

const TRY_AFTER: Duration = Duration::from_millis(250);
const RENDER_AFTER: Duration = Duration::from_millis(600);
/// Save without the bridge's check if it has not answered by then.
const CHECK_TIMEOUT: Duration = Duration::from_secs(3);
/// Two blue pulses of the offending box's rule.
const FLASH: Duration = Duration::from_millis(600);
/// The label column of More options (fits "Condition"; with the 12 px gap
/// the Kind row and its *(guessed)* fit the 772 px measure of an 860 px
/// window in both typefaces, and wrap only below about 700 px).
const OPT_LABEL: f32 = 76.0;
/// Where the options' controls start (the label column + the radio's gap).
const OPT_INDENT: f32 = OPT_LABEL + 12.0;

#[derive(Default)]
pub struct TryState {
    pub due: Option<Instant>,
    pub generation: u64,
    pending_gen: u64,
    rx: Option<Receiver<Result<TryResult, String>>>,
    pub result: Option<TryResult>,
    pub error: Option<String>,
    /// The request the table on screen answers.
    pub shown: Option<TryRequest>,
    sent: Option<TryRequest>,
}

impl TryState {
    pub fn running(&self) -> bool {
        self.due.is_some() || self.rx.is_some()
    }
}

pub struct Saving {
    pub draft: Draft,
    rx: Option<Receiver<Result<(), String>>>,
    started: Instant,
}

pub struct SnippetPage {
    pub trigger: Entity<TriggerField>,
    pub output: Entity<TokenEditor>,
    /// Kind::RawLua: the entry's Lua.
    pub raw: Entity<TextField>,
    pub priority: Entity<TextField>,
    pub desc: Entity<TextField>,
    pub eg: Entity<TextField>,
    pub check: Entity<TextField>,
    pub draft: Draft,
    /// Where she was in ntex when she opened the studio.
    pub place: Option<SnipContext>,
    pub open_ft: Option<String>,
    pub findings: Vec<Finding>,
    /// The bridge refused the inbox with this draft in it.
    pub save_error: Option<String>,
    pub lua: String,
    pub options_open: bool,
    on_tab_seen: bool,
    pub try_: TryState,
    pub preview: Preview,
    /// Example: the place she picked by clicking (cycles).
    pub place_override: Option<String>,
    /// (when, which box) for the error flash.
    pub flash: Option<(Instant, &'static str)>,
    /// Bumped per flash, so the box's pulse animation restarts.
    pub flashes: u64,
    pub saving: Option<Saving>,
    last_submit: Option<Instant>,
    /// ⌘⇧V / ⌥1 pressed where the kind does not allow it: offer this kind.
    pub offer: Option<Kind>,
}

fn field(cx: &mut Context<Studio>, f: impl FnOnce(TextField) -> TextField) -> Entity<TextField> {
    cx.new(|cx| f(TextField::new(cx)))
}

impl SnippetPage {
    pub fn new(window: &mut Window, cx: &mut Context<Studio>, subs: &mut Vec<Subscription>) -> SnippetPage {
        let trigger = cx.new(TriggerField::new);
        let output = cx.new(TokenEditor::new);
        let raw = field(cx, |f| f.mono().multiline().placeholder("math(\"ff\", [[\\frac{<>}{<>}]], \"yours\"),"));
        let priority = field(cx, |f| f.mono().placeholder("0"));
        let desc = field(cx, |f| f.placeholder("what it does"));
        let eg = field(cx, |f| f.mono().placeholder("keys"));
        let check = field(cx, |f| f.mono().multiline().placeholder("return true"));

        subs.push(cx.subscribe_in(&trigger, window, |s: &mut Studio, _, ev: &TriggerEvent, window, cx| match ev {
            TriggerEvent::Changed => s.snippet_changed(cx),
            TriggerEvent::Submit => {
                let f = s.snippet.output.read(cx).focus.clone();
                f.focus(window, cx);
                cx.notify();
            }
            TriggerEvent::Cancel => s.escape(window, cx),
        }));
        subs.push(cx.subscribe_in(&output, window, |s: &mut Studio, _, ev: &TokenEditorEvent, window, cx| match ev {
            TokenEditorEvent::Changed => s.snippet_changed(cx),
            TokenEditorEvent::Submit => s.save_snippet(window, cx),
            TokenEditorEvent::Cancel => s.escape(window, cx),
        }));
        subs.push(cx.subscribe_in(&output, window, |s: &mut Studio, _, ev: &TokenEditorOffer, _, cx| match ev {
            TokenEditorOffer::SwitchKind(kind) => {
                s.snippet.offer = Some(*kind);
                cx.notify();
            }
        }));
        for f in [&raw, &priority, &desc, &eg, &check] {
            subs.push(cx.subscribe_in(f, window, |s: &mut Studio, _, ev: &TextFieldEvent, window, cx| match ev {
                TextFieldEvent::Changed => s.snippet_changed(cx),
                TextFieldEvent::Cancel => s.escape(window, cx),
                TextFieldEvent::Submit | TextFieldEvent::Blur => {}
            }));
        }
        // the kit's fields are not tab stops by themselves: make them part of Tab order
        let _ = trigger.read(cx).focus.clone().tab_stop(true);
        let _ = output.read(cx).focus.clone().tab_stop(true);
        for f in [&raw, &priority, &desc, &eg, &check] {
            let _ = f.read(cx).focus.clone().tab_stop(true);
        }
        SnippetPage {
            trigger,
            output,
            raw,
            priority,
            desc,
            eg,
            check,
            draft: Draft::default(),
            place: None,
            open_ft: None,
            findings: Vec::new(),
            save_error: None,
            lua: String::new(),
            options_open: false,
            on_tab_seen: false,
            try_: TryState::default(),
            preview: Preview::new(true, false),
            offer: None,
            place_override: None,
            flash: None,
            flashes: 0,
            saving: None,
            last_submit: None,
        }
    }
}

fn token_set(kind: Kind) -> TokenSet {
    TokenSet::for_kind(kind)
}

fn trigger_mode(kind: Kind) -> &'static str {
    match kind {
        Kind::Pattern => "pattern",
        Kind::Visual => "key",
        _ => "trigger",
    }
}

fn pin(d: &mut Draft, name: &str) {
    if !d.pinned.iter().any(|p| p == name) {
        d.pinned.push(name.to_string());
    }
}

fn pinned(d: &Draft, name: &str) -> bool {
    d.pinned.iter().any(|p| p == name)
}

/// Which box an error belongs to (it flashes, and focus goes there).
fn box_of(code: &str) -> &'static str {
    match code {
        "empty-output" | "no-selection" | "no-capture" | "literal-angle" | "lua-error" => "output",
        _ => "trigger",
    }
}

/// The places Try it can type the trigger, for this draft.
pub fn places(d: &Draft) -> Vec<&'static str> {
    if d.kind == Kind::Visual {
        return vec!["visual"];
    }
    match d.context {
        SnipContext::Math => match d.math_where {
            MathWhere::Any => vec!["inline", "display"],
            MathWhere::Inline => vec!["inline"],
            MathWhere::Display => vec!["display"],
        },
        SnipContext::Text => match d.line {
            LineRule::Anywhere => vec!["text", "text-start", "list-item"],
            LineRule::Start => vec!["text-start"],
            LineRule::AfterText => vec!["text"],
            LineRule::ListItem => vec!["list-item"],
        },
    }
}

/// A Try-it buffer: the caret (‸) as a thin blue bar, a selected default
/// on the selection tint.
fn buffer_el(buffer: &str, selected: Option<&str>, t: &crate::ui::widgets::theme::Theme) -> AnyElement {
    let text = |s: &str| div().child(s.to_string());
    let mut row = div().flex().flex_row().items_center().font(t.mono()).text_size(px(14.)).line_height(px(19.));
    let mut first = true;
    for part in buffer.split('‸') {
        if !first {
            let rest = part;
            match selected.filter(|s| !s.is_empty() && rest.starts_with(*s)) {
                Some(sel) => {
                    row = row.child(div().bg(t.selection).child(sel.to_string()));
                    row = row.child(text(&rest[sel.len()..]));
                }
                None => {
                    row = row.child(crate::ui::widgets::boxes::caret_bar(t, px(14.)));
                    row = row.child(text(rest));
                }
            }
        } else {
            row = row.child(text(part));
        }
        first = false;
    }
    row.into_any_element()
}

/// Where and what to type when `infer::try_plan` is unavailable.
fn fallback_try_plan(d: &Draft) -> TryRequest {
    let mut stops: Vec<u32> = Vec::new();
    for p in &d.output {
        if let Piece::Tab { n, .. } = p
            && *n > 0
            && !stops.contains(n)
        {
            stops.push(*n);
        }
    }
    let keys = match d.kind {
        Kind::Post | Kind::Suffix => format!("x{}", d.trigger),
        Kind::Pattern => d.eg.clone(),
        _ => d.trigger.clone(),
    };
    TryRequest {
        entry: String::new(),
        ft: if d.ft == FileType::Markdown { "markdown".into() } else { "tex".into() },
        place: places(d).first().copied().unwrap_or("inline").to_string(),
        keys,
        tab: d.fires == Fires::Tab,
        fills: ["a", "b", "c", "d", "e", "f", "g", "h", "i"].iter().take(stops.len()).map(|s| s.to_string()).collect(),
    }
}

fn place_line(place: &str, ft: &str) -> String {
    let file = if ft == "markdown" { "an .md file" } else { "a .tex file" };
    match place {
        "inline" => format!("In  $…$  of {file}:"),
        "display" => format!("In  \\[…\\]  of {file}:"),
        "text" => format!("In the text of {file}:"),
        "text-start" => format!("At the start of a line of {file}:"),
        "list-item" => format!("After  \\item  in {file}:"),
        "visual" => format!("With  x+y  selected in  $…$  of {file}:"),
        other => format!("In {other} of {file}:"),
    }
}

impl Studio {
    pub fn snippet_is_empty(&self) -> bool {
        let d = &self.snippet.draft;
        d.trigger.is_empty() && d.output.is_empty() && d.raw_lua.trim().is_empty()
    }

    /// Nothing typed yet: no findings, no Try it.
    fn snippet_pristine(&self) -> bool {
        self.snippet_is_empty()
    }

    /// Re-read the widgets into the draft, guess, check, generate, schedule.
    pub fn snippet_changed(&mut self, cx: &mut Context<Studio>) {
        let now = self.now(cx);
        let p = &mut self.snippet;
        let (trigger, on_tab) = {
            let t = p.trigger.read(cx);
            (t.trigger.clone(), t.on_tab)
        };
        let d = &mut p.draft;
        d.trigger = trigger;
        if on_tab != p.on_tab_seen {
            // Tab in the trigger field: fires on Tab, and she said so
            p.on_tab_seen = on_tab;
            d.fires = if on_tab { Fires::Tab } else { Fires::Auto };
            pin(d, "fires");
        }
        let mut pieces = p.output.read(cx).pieces.clone();
        safe("codegen::renumber", || codegen::renumber(&mut pieces));
        d.output = pieces;
        d.raw_lua = p.raw.read(cx).text.clone();
        d.priority = p.priority.read(cx).text.trim().parse().unwrap_or(0);
        d.desc = p.desc.read(cx).text.clone();
        d.eg = p.eg.read(cx).text.clone();
        d.check = p.check.read(cx).text.clone();
        let place = p.place;
        let before = d.clone();
        if safe("infer::guess", || infer::guess(d, place)).is_none() {
            *d = before;
        }
        p.save_error = None;
        self.sync_snippet_widgets(cx);
        self.snippet_rechecked(cx);
        let p = &mut self.snippet;
        p.try_.generation += 1;
        p.try_.due = Some(now + TRY_AFTER);
        p.preview.schedule(now + RENDER_AFTER);
        cx.notify();
    }

    /// Keep the widgets in step with the guessed draft: the ⇥ keycap in the
    /// trigger field, its mode, the tokens the editor offers.
    fn sync_snippet_widgets(&mut self, cx: &mut Context<Studio>) {
        let p = &mut self.snippet;
        let d = &p.draft;
        let on_tab = d.fires == Fires::Tab;
        let mode = trigger_mode(d.kind);
        p.trigger.update(cx, |t, cx| {
            if t.on_tab != on_tab {
                t.on_tab = on_tab;
                cx.notify();
            }
            if t.mode != mode {
                t.set_mode(mode, cx);
            }
        });
        p.on_tab_seen = on_tab;
        let kind = d.kind;
        let post = kind == Kind::Post;
        p.output.update(cx, |e, cx| {
            if e.allowed != TokenSet::for_kind(kind) || e.post_capture != post {
                e.set_kind(kind, cx);
            }
        });
        if p.offer == Some(kind) {
            p.offer = None;
        }
    }

    /// Re-run the checks and the Lua (the known snippets changed, or the draft).
    pub fn snippet_rechecked(&mut self, cx: &mut Context<Studio>) {
        let was_ready_try = self.snippet.try_.rx.is_none() && self.snippet.try_.result.is_none();
        let d = self.snippet.draft.clone();
        self.snippet.findings = if self.snippet_pristine() {
            Vec::new()
        } else {
            safe("infer::check", || infer::check(&d, &self.known)).unwrap_or_default()
        };
        self.snippet.lua = safe("codegen::entry", || codegen::entry(&d)).unwrap_or_default();
        if was_ready_try && self.bridge_ready() && self.snippet.try_.due.is_none() && !self.snippet_pristine() {
            let now = self.now(cx);
            self.snippet.try_.due = Some(now);
        }
        cx.notify();
    }

    fn snippet_has_error(&self) -> bool {
        self.snippet.findings.iter().any(|f| f.severity == Severity::Error)
    }

    /// Put a draft into the widgets (an inbox entry, a fix, the cleared one).
    /// `pin_all`: an existing entry — its fields are hers, not guesses.
    pub fn load_draft(&mut self, mut d: Draft, pin_all: bool, window: &mut Window, cx: &mut Context<Studio>) {
        let _ = window;
        if pin_all {
            for n in ["kind", "context", "fires", "math_where", "line", "ft"] {
                pin(&mut d, n);
            }
        }
        let p = &mut self.snippet;
        let on_tab = d.fires == Fires::Tab;
        p.trigger.update(cx, |t, cx| t.set(d.trigger.clone(), on_tab, cx));
        p.on_tab_seen = on_tab;
        p.output.update(cx, |e, cx| e.set_pieces(d.output.clone(), cx));
        p.raw.update(cx, |f, cx| f.set_text(d.raw_lua.clone(), cx));
        let prio = if d.priority == 0 { String::new() } else { d.priority.to_string() };
        p.priority.update(cx, |f, cx| f.set_text(prio, cx));
        p.desc.update(cx, |f, cx| f.set_text(d.desc.clone(), cx));
        p.eg.update(cx, |f, cx| f.set_text(d.eg.clone(), cx));
        p.check.update(cx, |f, cx| f.set_text(d.check.clone(), cx));
        if d.priority != 0 || !d.desc.is_empty() || !d.eg.is_empty() || !d.check.is_empty() || d.kind != Kind::Plain {
            p.options_open = true;
        }
        p.place_override = None;
        p.try_.result = None;
        p.try_.error = None;
        p.try_.shown = None;
        p.preview.clear();
        p.draft = d;
        p.flash = None;
        self.snippet_changed(cx);
    }

    /// ⌘N: start over (⌘Z brings the cleared draft back).
    pub fn clear_snippet(&mut self, window: &mut Window, cx: &mut Context<Studio>) {
        if !self.snippet_is_empty() {
            self.undo.push(UndoItem::ClearedDraft(self.snippet.draft.clone()));
            self.toast("Cleared. ⌘Z brings it back.", cx);
        }
        self.snippet.place = None;
        self.snippet.open_ft = None;
        self.load_draft(Draft::default(), false, window, cx);
        self.snippet.trigger.read(cx).focus.clone().focus(window, cx);
    }

    fn set_snippet(&mut self, cx: &mut Context<Studio>, f: impl FnOnce(&mut Draft)) {
        f(&mut self.snippet.draft);
        // radios do not live in a widget: keep the trigger field's ⇥ in step first
        let on_tab = self.snippet.draft.fires == Fires::Tab;
        self.snippet.trigger.update(cx, |t, cx| {
            t.on_tab = on_tab;
            cx.notify();
        });
        self.snippet.on_tab_seen = on_tab;
        self.snippet_changed(cx);
    }

    // -- save ----------------------------------------------------------------

    /// ⌘↵ (UX.md "Save").
    pub fn save_snippet(&mut self, window: &mut Window, cx: &mut Context<Studio>) {
        let now = self.now(cx);
        if self.snippet.saving.is_some() {
            return;
        }
        // the output editor's Submit and the page's ⌘↵ can both arrive for one press
        if self.snippet.last_submit.is_some_and(|t| now.saturating_duration_since(t) < Duration::from_millis(50)) {
            return;
        }
        self.snippet.last_submit = Some(now);
        self.snippet_changed(cx);
        let errors: Vec<&Finding> = self.snippet.findings.iter().filter(|f| f.severity == Severity::Error).collect();
        if self.snippet_is_empty() || !errors.is_empty() {
            let which = errors.first().map(|f| box_of(&f.code)).unwrap_or("trigger");
            if self.snippet_is_empty() {
                // a pristine page shows no findings; say the one that matters
                self.snippet.findings =
                    vec![Finding { severity: Severity::Error, code: "empty-trigger".into(), message: "Trigger is empty.".into(), fix: None }];
            }
            self.flash_box(which, window, cx);
            return;
        }
        let draft = self.snippet.draft.clone();
        if self.bridge_ready() {
            let current = std::fs::read_to_string(self.paths.inbox())
                .ok()
                .or_else(|| safe("inbox::empty_file", inbox::empty_file))
                .unwrap_or_default();
            let (minute, _) = local_now();
            if let Some((text, _)) = safe("inbox::with_draft", || inbox::with_draft(&current, &draft, &minute)) {
                let rx = self.bridge.as_ref().and_then(|b| safe("Bridge::check_file", || b.check_file("inbox.lua", &text)));
                if rx.is_some() {
                    self.snippet.saving = Some(Saving { draft, rx, started: now });
                    cx.notify();
                    return;
                }
            }
        }
        self.commit_snippet(draft, false, window, cx);
    }

    fn flash_box(&mut self, which: &'static str, window: &mut Window, cx: &mut Context<Studio>) {
        self.snippet.flash = Some((self.now(cx), which));
        self.snippet.flashes += 1;
        let f = if which == "output" {
            self.snippet.output.read(cx).focus.clone()
        } else {
            self.snippet.trigger.read(cx).focus.clone()
        };
        f.focus(window, cx);
        cx.notify();
    }

    /// Back up, write, refresh, toast, clear, hide.
    fn commit_snippet(&mut self, draft: Draft, checked: bool, window: &mut Window, cx: &mut Context<Studio>) {
        let name = if draft.kind == Kind::RawLua && draft.trigger.is_empty() { "the Lua entry".to_string() } else { format!("‘{}’", draft.trigger) };
        let editing = draft.id.is_some();
        let inbox_path = self.paths.inbox();
        let backed = self.backup(&format!("the save of {name}"), vec![inbox_path]);
        let (minute, _) = local_now();
        match safe("inbox::save", || inbox::save(&self.paths, &draft, &minute)) {
            Some(Ok(_id)) => {}
            Some(Err(e)) => {
                if backed.is_ok() {
                    self.undo.pop();
                }
                self.toast(format!("! Could not write the inbox: {e}"), cx);
                return;
            }
            None => {
                if backed.is_ok() {
                    self.undo.pop();
                }
                self.toast("! Could not write the inbox.", cx);
                return;
            }
        }
        self.after_files_changed(cx);
        let (verb, prep) = if editing { ("Updated", "in") } else { ("Saved", "to") };
        let mut msg = if checked {
            format!("{verb} {name} {prep} the inbox — live in ntex now.")
        } else {
            format!("{verb} {name} {prep} the inbox. The hidden ntex wasn't ready, so it was not checked first; a broken file leaves the old one active.")
        };
        if backed.is_ok() {
            msg.push_str(" ⌘Z undoes.");
        }
        self.toast(msg, cx);
        self.snippet.place = None;
        self.snippet.open_ft = None;
        self.load_draft(Draft::default(), false, window, cx);
        self.snippet.trigger.read(cx).focus.clone().focus(window, cx);
        if self.prefs.hide_after_save {
            self.hide(cx);
        }
    }

    /// The snippet page's share of the poll loop.
    pub fn pump_snippet(&mut self, now: Instant, window: &mut Window, cx: &mut Context<Studio>) -> bool {
        let mut changed = false;

        // a save waiting for the bridge's check
        if let Some(saving) = &self.snippet.saving {
            let res = match saving.rx.as_ref().map(|rx| rx.try_recv()) {
                // a load error blocks the save; a check that could not run
                // (the bridge failed) saves unchecked, like a timeout
                Some(Ok(Ok(()))) => Some(Ok(true)),
                Some(Ok(Err(e))) if e.starts_with(ntex_studio_io::bridge::UNCHECKED) => Some(Ok(false)),
                Some(Ok(Err(e))) => Some(Err(e)),
                Some(Err(TryRecvError::Empty)) if now.saturating_duration_since(saving.started) < CHECK_TIMEOUT => None,
                _ => Some(Ok(false)), // no answer: save anyway, unchecked
            };
            if let Some(res) = res {
                let saving = self.snippet.saving.take().unwrap();
                match res {
                    Ok(checked) => self.commit_snippet(saving.draft, checked, window, cx),
                    Err(e) => {
                        self.snippet.save_error = Some(e);
                        self.flash_box("output", window, cx);
                    }
                }
                changed = true;
            }
        }

        // Try it
        let t = &mut self.snippet.try_;
        if t.due.is_some_and(|d| now >= d) {
            t.due = None;
            let pristine = self.snippet_is_empty();
            let error = self.snippet.findings.iter().any(|f| f.severity == Severity::Error);
            let ready = self.bridge_state == crate::ui::app::BridgeState::Ready;
            if pristine || error {
                let t = &mut self.snippet.try_;
                t.rx = None;
                if t.result.is_some() || t.error.is_some() {
                    changed = true;
                }
                t.result = None;
                t.error = None;
                t.shown = None;
            } else if ready {
                let d = self.snippet.draft.clone();
                let plan = safe("infer::try_plan", || infer::try_plan(&d)).unwrap_or_else(|| fallback_try_plan(&d));
                {
                    let mut req = plan;
                    req.entry = self.snippet.lua.clone();
                    if let Some(ft) = &self.snippet.open_ft
                        && d.ft == FileType::Both
                    {
                        req.ft = ft.clone();
                    }
                    let allowed = places(&d);
                    if let Some(o) = &self.snippet.place_override
                        && allowed.contains(&o.as_str())
                    {
                        req.place = o.clone();
                    }
                    let rx = self.bridge.as_ref().and_then(|b| safe("Bridge::try_draft", || b.try_draft(req.clone())));
                    let t = &mut self.snippet.try_;
                    t.rx = rx;
                    t.pending_gen = t.generation;
                    t.sent = Some(req);
                    changed = true;
                }
            }
        }
        let t = &mut self.snippet.try_;
        if let Some(rx) = &t.rx {
            match rx.try_recv() {
                Ok(res) => {
                    t.rx = None;
                    // an older draft's answer: a newer one is on its way
                    if t.pending_gen == t.generation || t.due.is_none() {
                        match res {
                            Ok(r) => {
                                t.result = Some(r);
                                t.error = None;
                            }
                            Err(e) => {
                                t.result = None;
                                t.error = Some(e);
                            }
                        }
                        t.shown = t.sent.take();
                    }
                    changed = true;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    t.rx = None;
                    changed = true;
                }
            }
        }

        // the rendered output
        let render_on = self.prefs.render_preview;
        let d = &self.snippet.draft;
        let empty = d.output.is_empty() || d.kind == Kind::RawLua;
        let job = || {
            if !render_on || empty {
                return None;
            }
            let latex = safe("codegen::preview_latex", || codegen::preview_latex(&d.output))?;
            Some(RenderJob::Snippet {
                latex,
                math: d.context == SnipContext::Math,
                display: d.math_where == MathWhere::Display,
            })
        };
        changed |= self.snippet.preview.pump(now, self.renderer.as_ref(), job);

        if let Some((at, _)) = self.snippet.flash
            && now.saturating_duration_since(at) > FLASH
        {
            self.snippet.flash = None;
            changed = true;
        }
        changed
    }

    // -- hints ---------------------------------------------------------------

    pub fn snippet_hints(&self, window: &Window, cx: &App) -> Vec<SharedString> {
        let p = &self.snippet;
        let focused = |e: &gpui::FocusHandle| e.is_focused(window);
        if focused(&p.trigger.read(cx).focus) {
            let first = match trigger_mode(p.draft.kind) {
                "pattern" => "Type a Lua pattern; its captures become %1, %2, … in the output.",
                "key" => "Type the one key that wraps the selection.",
                _ => "Type the trigger exactly as you will in ntex. Tab makes it a Tab shortcut.",
            };
            return vec![first.into(), "Return moves to the output · ⌥⌫ deletes a word · ⌘⌫ everything.".into()];
        }
        if focused(&p.output.read(cx).focus) {
            let mut v: Vec<SharedString> = vec!["Tab inserts a tab stop · ⇧Tab a mirror · ⌘E the exit point.".into()];
            let set = token_set(p.draft.kind);
            if set.selection {
                v.push("⌘⇧V inserts the selection.".into());
            }
            if set.captures > 0 {
                v.push("⌥1 … ⌥9 insert what the trigger captured.".into());
            }
            v.push("⌘↵ saves to the inbox · Esc leaves the editor (on an empty draft, hides the studio).".into());
            return v;
        }
        if focused(&p.raw.read(cx).focus) {
            return vec!["One or more helper calls, each ending in a comma, as in your snippet files.".into()];
        }
        if focused(&p.check.read(cx).focus) {
            return vec!["The body of the condition: return true to let the snippet fire.".into()];
        }
        if focused(&p.eg.read(cx).focus) {
            return vec!["What to type to see it work (patterns need one): Try it types these keys.".into()];
        }
        vec!["⌘↵ saves to the inbox · ⌘N clears · ⌘Z undoes the last save.".into(), "⌘1 ⌘2 ⌘3 switch pages · ⌘W hides the studio.".into()]
    }

    // -- render ----------------------------------------------------------------

    /// The box to pulse, with a fresh id per flash (TBox::flash restarts on a new id).
    fn flash_box_el(&self, which: &str, b: TBox) -> TBox {
        match self.snippet.flash {
            Some((_, w)) if w == which => b.flash(SharedString::from(format!("flash.{which}.{}", self.snippet.flashes))),
            _ => b,
        }
    }

    pub fn render_snippet(&mut self, window: &mut Window, cx: &mut Context<Studio>) -> impl IntoElement {
        let t = cx.theme().clone();
        let now = self.now(cx);
        if self.snippet.preview.opacity(now) < 1.0 {
            window.request_animation_frame();
        }
        let d = self.snippet.draft.clone();
        let this = cx.weak_entity();
        let sel = move |f: fn(&mut Draft, usize)| {
            let this = this.clone();
            move |i: usize, _: &mut Window, cx: &mut App| {
                this.update(cx, |s, cx| s.set_snippet(cx, |d| f(d, i))).ok();
            }
        };

        let title = if d.id.is_some() { "Edit snippet" } else { "New snippet" };

        // Definition 1.1 (Trigger)
        let fires = Radio::new("fires", "Fires", vec!["by itself".into(), "on Tab".into()], (d.fires == Fires::Tab) as usize)
            .guessed(!pinned(&d, "fires"))
            .on_select(sel(|d, i| {
                d.fires = if i == 1 { Fires::Tab } else { Fires::Auto };
                pin(d, "fires");
            }));
        let ctx = Radio::new("context", "In", vec!["maths".into(), "text".into()], (d.context == SnipContext::Text) as usize)
            .guessed(!pinned(&d, "context"))
            .on_select(sel(|d, i| {
                d.context = if i == 1 { SnipContext::Text } else { SnipContext::Math };
                pin(d, "context");
            }));
        let trigger_box = TBox::new(BoxKind::Definition, "Definition 1.1")
            .name("Trigger")
            .child(div().pt(px(4.)).child(self.snippet.trigger.clone()))
            .child(div().flex().flex_row().flex_wrap().gap_x(px(28.)).gap_y(px(4.)).pt(px(8.)).child(fires).child(ctx));

        // Theorem 1.2 (Output)
        let toolbar = div().pt(px(4.)).pb(px(8.)).when(d.kind != Kind::RawLua, |d| d.child(TokenToolbar::new(self.snippet.output.clone())));
        let preview = &self.snippet.preview;
        let rendered: Option<AnyElement> = if !self.prefs.render_preview || d.kind == Kind::RawLua {
            None
        } else if let Some((image, w, h)) = &preview.image {
            Some(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .items_center()
                    .gap(px(10.))
                    .pt(px(8.))
                    .child(shell::slate("Rendered:", cx))
                    // the render is 10 pt type; the page is set at 15 px
                    .child(img(image.clone()).w(px(*w * 1.5)).h(px(*h * 1.5)).opacity(preview.opacity(now)))
                    .into_any_element(),
            )
        } else {
            preview.error.as_ref().map(|e| {
                div()
                    .flex()
                    .justify_end()
                    .pt(px(8.))
                    .child(div().font(t.mono()).text_size(px(13.)).text_color(t.slate).child(e.clone()))
                    .into_any_element()
            })
        };
        let editor: AnyElement = if d.kind == Kind::RawLua {
            div().child(self.snippet.raw.clone()).into_any_element()
        } else {
            div().child(self.snippet.output.clone()).into_any_element()
        };
        let output_box = TBox::new(BoxKind::Theorem, "Theorem 1.2")
            .name("Output")
            .child(toolbar)
            .child(editor)
            .children(rendered);

        // Example 1.3 (Try it in ntex)
        let example = self.render_try(&d, cx).into_any_element();

        // Remark (Checks)
        let mut lines: Vec<AnyElement> = Vec::new();
        if let Some(e) = &self.snippet.save_error {
            lines.push(LogLine { severity: Severity::Error, text: format!("! inbox.lua would not load: {e}").into() }.into_any_element());
        }
        for (i, f) in self.snippet.findings.iter().enumerate() {
            let text: SharedString = match f.severity {
                Severity::Error => format!("! {}", f.message).into(),
                Severity::Warning => format!("Studio Warning: {}", f.message).into(),
                Severity::Note => f.message.clone().into(),
            };
            let fix = f.fix.as_ref().and_then(|fx| fx.draft.clone().map(|d| (fx.label.clone(), d)));
            let line = div()
                .flex()
                .flex_row()
                .items_start()
                .gap(px(10.))
                .child(div().flex_1().child(LogLine { severity: f.severity, text }))
                .when_some(fix, |el, (label, fixed)| {
                    el.child(Button::new(SharedString::from(format!("fix.{i}")), label).small().on_click(cx.listener(
                        move |s, _, window, cx| {
                            s.load_draft(fixed.clone(), false, window, cx);
                        },
                    )))
                });
            lines.push(line.into_any_element());
        }
        if let Some(kind) = self.snippet.offer {
            let (what, label) = match kind {
                Kind::Visual => ("The selection (⌘⇧V) belongs to snippets that wrap a selection.", "Make it wrap a selection"),
                Kind::Post => ("A capture (⌥1) needs a trigger that grabs the letter before it.", "Make it grab the letter"),
                _ => ("Captures (⌥1 … ⌥9) need a pattern trigger.", "Make it a pattern"),
            };
            lines.push(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(px(10.))
                    .child(div().flex_1().child(LogLine::new(Severity::Note, what)))
                    .child(Button::new("offer.switch", label).small().on_click(cx.listener(move |s, _, _, cx| {
                        s.snippet.offer = None;
                        s.set_snippet(cx, |d| {
                            d.kind = kind;
                            pin(d, "kind");
                        });
                    })))
                    .child(Button::new("offer.no", "No").small().on_click(cx.listener(|s, _, _, cx| {
                        s.snippet.offer = None;
                        cx.notify();
                    })))
                    .into_any_element(),
            );
        }
        let remark = (!lines.is_empty()).then(|| {
            TBox::new(BoxKind::Remark, "Remark").name("Checks").child(div().flex().flex_col().gap(px(3.)).pt(px(2.)).children(lines))
        });

        // More options
        let options = self.render_options(&d, cx).into_any_element();

        // Proof (Generated Lua)
        let proof_body: AnyElement = if self.snippet_is_empty() {
            shell::slate("Nothing to generate yet.", cx).into_any_element()
        } else {
            CodeView { code: self.snippet.lua.clone().into(), language: "lua" }.into_any_element()
        };
        let proof = TBox::new(BoxKind::Proof, "Proof").name("Generated Lua").child(div().pt(px(2.)).child(proof_body));

        let saving = self.snippet.saving.is_some();
        let buttons = div()
            .flex()
            .flex_row()
            .justify_end()
            .items_center()
            .gap(px(14.))
            .pt(px(4.))
            .when(saving, |d| d.child(shell::slate("checking…", cx)))
            .child(Button::new("clear", "Clear").keys("⌘N").on_click(cx.listener(|s, _, w, cx| s.clear_snippet(w, cx))))
            .child(
                Button::new("save", if d.id.is_some() { "Save changes" } else { "Save to inbox" })
                    .keys("⌘↵")
                    .primary()
                    .disabled(saving)
                    .on_click(cx.listener(|s, _, w, cx| s.save_snippet(w, cx))),
            );

        div()
            .flex()
            .flex_col()
            .gap(px(GAP))
            .child(Section { number: "1".into(), title: title.into() })
            .child(self.flash_box_el("trigger", trigger_box))
            .child(self.flash_box_el("output", output_box))
            .child(example)
            .children(remark)
            .child(options)
            .child(proof)
            .child(buttons)
    }

    fn render_try(&self, d: &Draft, cx: &mut Context<Studio>) -> impl IntoElement {
        let t = cx.theme().clone();
        let tr = &self.snippet.try_;
        let allowed = places(d);
        let shown_place = tr
            .shown
            .as_ref()
            .map(|r| r.place.clone())
            .or_else(|| self.snippet.place_override.clone())
            .or_else(|| allowed.first().map(|s| s.to_string()))
            .unwrap_or_else(|| "inline".into());
        let ft = tr
            .shown
            .as_ref()
            .map(|r| r.ft.clone())
            .or_else(|| self.snippet.open_ft.clone())
            .unwrap_or_else(|| if d.ft == FileType::Markdown { "markdown".into() } else { "tex".into() });
        let cycle = allowed.len() > 1 && !self.snippet_is_empty();
        let place_el = div()
            .id("try.place")
            .when(cycle, |d| d.cursor_pointer())
            .child(div().flex().flex_row().gap(px(6.)).items_baseline().child(place_line(&shown_place, &ft)).when(cycle, |d| {
                d.child(div().text_size(px(12.)).italic().text_color(t.slate).child("(click for another place)"))
            }))
            .on_click(cx.listener(move |s, _, _, cx| s.cycle_place(cx)));

        let mut body: Vec<AnyElement> = Vec::new();
        let running = tr.running();
        let status: Option<String> = if self.snippet_is_empty() {
            Some("Type a trigger and an output to see ntex expand it.".into())
        } else if self.snippet_has_error() {
            Some("Fix the error below to try it.".into())
        } else {
            match &self.bridge_state {
                crate::ui::app::BridgeState::Failed(e) => Some(format!("ntex could not start here, so it cannot be tried ({e}).")),
                crate::ui::app::BridgeState::Starting if tr.result.is_none() => Some("ntex is starting…".into()),
                _ => None,
            }
        };
        match (&status, &tr.result, &tr.error) {
            (Some(s), _, _) => body.push(shell::slate(s.clone(), cx).into_any_element()),
            (None, Some(r), _) => {
                if let Some(other) = &r.expanded_other {
                    let msg = match (&r.expanded_trigger, &r.expanded_file) {
                        (Some(t), Some(f)) => format!("ntex expanded ‘{t}’ from {f} instead."),
                        _ => format!("ntex expanded {other} instead."),
                    };
                    body.push(shell::slate(msg, cx).into_any_element());
                }
                if !r.expanded {
                    let why = r.reason.as_ref().or(r.error.as_ref()).map(|e| format!(": {e}")).unwrap_or_default();
                    body.push(shell::slate(format!("Nothing expanded{why}."), cx).into_any_element());
                }
                if !r.steps.is_empty() {
                    let mono = |s: &str| div().font(t.mono()).text_size(px(14.)).child(s.to_string()).into_any_element();
                    let rows = r.steps.iter().map(|st| vec![mono(&st.keys), buffer_el(&st.buffer, st.selected.as_deref(), &t)]).collect();
                    body.push(
                        div()
                            .when(running, |d| d.opacity(0.55))
                            .child(Table { header: vec!["You type".into(), "You get".into()], rows, widths: Some(vec![0.3, 0.7]) })
                            .into_any_element(),
                    );
                }
            }
            (None, None, Some(e)) => body.push(LogLine { severity: Severity::Note, text: format!("Could not try it: {e}").into() }.into_any_element()),
            (None, None, None) if running => body.push(shell::slate("trying…", cx).into_any_element()),
            (None, None, None) => body.push(shell::slate("Not tried yet.", cx).into_any_element()),
        }
        let trying = running && tr.result.is_some() && status.is_none();
        TBox::new(BoxKind::Example, "Example 1.3")
            .name("Try it in ntex")
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_baseline()
                    .pt(px(2.))
                    .when(!self.snippet_is_empty(), |d| d.child(place_el))
                    .when(trying, |d| d.child(div().italic().text_size(px(13.)).text_color(t.slate.opacity(0.7)).child("trying…"))),
            )
            .child(div().flex().flex_col().gap(px(4.)).pt(px(4.)).children(body))
    }

    /// Click on the place line: the next allowed place.
    pub fn cycle_place(&mut self, cx: &mut Context<Studio>) {
        let d = &self.snippet.draft;
        let allowed = places(d);
        if allowed.len() < 2 {
            return;
        }
        let cur = self
            .snippet
            .try_
            .shown
            .as_ref()
            .map(|r| r.place.clone())
            .or_else(|| self.snippet.place_override.clone())
            .unwrap_or_else(|| allowed[0].to_string());
        let i = allowed.iter().position(|p| *p == cur).map(|i| (i + 1) % allowed.len()).unwrap_or(0);
        self.snippet.place_override = Some(allowed[i].to_string());
        self.snippet.try_.generation += 1;
        self.snippet.try_.due = Some(self.now(cx));
        cx.notify();
    }

    fn render_options(&self, d: &Draft, cx: &mut Context<Studio>) -> impl IntoElement {
        let t = cx.theme().clone();
        let open = self.snippet.options_open;
        let this = cx.weak_entity();
        let sel = move |f: fn(&mut Draft, usize)| {
            let this = this.clone();
            move |i: usize, _: &mut Window, cx: &mut App| {
                this.update(cx, |s, cx| s.set_snippet(cx, |d| f(d, i))).ok();
            }
        };
        let disclosure = Disclosure {
            id: "more".into(),
            title: "More options.".into(),
            open,
            on_toggle: Some(std::rc::Rc::new({
                let this = cx.weak_entity();
                move |open, _, cx: &mut App| {
                    this.update(cx, |s, cx| {
                        s.snippet.options_open = open;
                        cx.notify();
                    })
                    .ok();
                }
            })),
        };
        let mut col = div().flex().flex_col().child(disclosure);
        if open {
            let kind_i = match d.kind {
                Kind::Plain => 0,
                Kind::Suffix => 1,
                Kind::Post => 2,
                Kind::Pattern => 3,
                Kind::Visual => 4,
                Kind::RawLua => 5,
            };
            let kind = Radio::new(
                "kind",
                "Kind",
                ["plain", "follows a letter", "grabs the letter before", "pattern", "wraps a selection", "Lua"]
                    .into_iter()
                    .map(SharedString::from)
                    .collect(),
                kind_i,
            )
            .label_width(px(OPT_LABEL))
            .guessed(!pinned(d, "kind"))
            .on_select(sel(|d, i| {
                d.kind = [Kind::Plain, Kind::Suffix, Kind::Post, Kind::Pattern, Kind::Visual, Kind::RawLua][i.min(5)];
                pin(d, "kind");
            }));
            let maths = Radio::new(
                "where",
                "Maths",
                vec!["anywhere".into(), "only $…$".into(), "only display".into()],
                match d.math_where {
                    MathWhere::Any => 0,
                    MathWhere::Inline => 1,
                    MathWhere::Display => 2,
                },
            )
            .label_width(px(OPT_LABEL))
            .guessed(!pinned(d, "math_where"))
            .on_select(sel(|d, i| {
                d.math_where = [MathWhere::Any, MathWhere::Inline, MathWhere::Display][i.min(2)];
                pin(d, "math_where");
            }));
            let line = Radio::new(
                "line",
                "Line",
                vec!["anywhere".into(), "at the start".into(), "after text".into(), "in a list item".into()],
                match d.line {
                    LineRule::Anywhere => 0,
                    LineRule::Start => 1,
                    LineRule::AfterText => 2,
                    LineRule::ListItem => 3,
                },
            )
            .label_width(px(OPT_LABEL))
            .guessed(!pinned(d, "line"))
            .on_select(sel(|d, i| {
                d.line = [LineRule::Anywhere, LineRule::Start, LineRule::AfterText, LineRule::ListItem][i.min(3)];
                pin(d, "line");
            }));
            let files = Radio::new(
                "ft",
                "Files",
                vec![".tex and .md".into(), ".tex".into(), ".md".into()],
                match d.ft {
                    FileType::Both => 0,
                    FileType::Tex => 1,
                    FileType::Markdown => 2,
                },
            )
            .label_width(px(OPT_LABEL))
            .guessed(!pinned(d, "ft"))
            .on_select(sel(|d, i| {
                d.ft = [FileType::Both, FileType::Tex, FileType::Markdown][i.min(2)];
                pin(d, "ft");
            }));
            // a label in the options' label column (or at its natural width)
            let opt_label = |label: &'static str, column: bool| {
                div().flex_none().whitespace_nowrap().when(column, |d| d.w(px(OPT_LABEL))).child(shell::label(label, cx))
            };
            let small_field = |label: &'static str, f: &Entity<TextField>, w: f32| {
                div().flex().flex_row().items_baseline().gap(px(8.)).child(opt_label(label, false)).child(div().w(px(w)).child(f.clone()))
            };
            let mono_slate = |s: &'static str| div().font(t.mono()).text_size(px(14.)).text_color(t.slate).child(s);
            let p = &self.snippet;
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .gap(px(6.))
                    .pt(px(6.))
                    .child(kind)
                    .when(d.context == SnipContext::Math, |c| c.child(maths))
                    .when(d.context == SnipContext::Text, |c| c.child(line))
                    .child(files)
                    .child(
                        // Priority in the label column, the fields after it;
                        // the last one ends at the measure's right edge
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .items_baseline()
                            .gap_x(px(12.))
                            .gap_y(px(6.))
                            .child(opt_label("Priority", true))
                            .child(div().w(px(48.)).flex_none().child(p.priority.clone()))
                            .child(div().w(px(12.)))
                            .child(small_field("Description", &p.desc, 220.))
                            .child(div().w(px(12.)))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_baseline()
                                    .gap(px(8.))
                                    .flex_1()
                                    .min_w(px(180.))
                                    .child(opt_label("Example keys", false))
                                    .child(div().flex_1().min_w(px(80.)).child(p.eg.clone())),
                            ),
                    )
                    .child(
                        // Condition: function(…) on the label's line, the body
                        // indented to the controls' column and as wide as the
                        // measure, `end` under `function`
                        div()
                            .flex()
                            .flex_col()
                            .w_full()
                            .gap(px(2.))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_baseline()
                                    .gap(px(12.))
                                    .child(opt_label("Condition", true))
                                    .child(mono_slate("function(line, trigger, captures)")),
                            )
                            .child(div().w_full().pl(px(OPT_INDENT)).child(p.check.clone()))
                            .child(div().pl(px(OPT_INDENT)).child(mono_slate("end"))),
                    ),
            );
        }
        col
    }

    /// An inbox entry, opened for editing (Return on the Inbox page).
    pub fn edit_entry(&mut self, entry: &InboxEntry, window: &mut Window, cx: &mut Context<Studio>) {
        let mut d = entry.draft.clone().unwrap_or_else(|| Draft {
            kind: Kind::RawLua,
            raw_lua: entry.source.trim().to_string(),
            trigger: if entry.trigger == "?" { String::new() } else { entry.trigger.clone() },
            ..Draft::default()
        });
        d.id = Some(entry.id.clone());
        if !self.snippet_is_empty() && self.snippet.draft.id.as_deref() != Some(entry.id.as_str()) {
            self.undo.push(UndoItem::ClearedDraft(self.snippet.draft.clone()));
        }
        self.load_draft(d, true, window, cx);
        self.set_page(Page::Snippet, window, cx);
    }
}
