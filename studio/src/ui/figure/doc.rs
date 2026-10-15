//! The figure document: the `.fig` source and what the figures crate makes
//! of it (Figure, diagnostics, Scene). No GPUI here.
//!
//! The source is the truth for typing and Ask; the Figure is the truth for
//! canvas edits (an edit is applied to the Figure, `format` writes it back,
//! and the result is re-read so spans stay right).
//!
//! Every call into `ntex_figures` goes through [`core`]: a panic (a
//! `todo!()` while FIG-CORE-A/B are still building, or a bug) becomes `None`
//! instead of taking the window down. A piece that is `todo!()` stays down
//! for the life of the process, so it is not called again.
use ntex_figures::model::*;
use ntex_figures::{edit, format, layout, parse};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, OnceLock};

fn down() -> &'static Mutex<BTreeSet<&'static str>> {
    static DOWN: OnceLock<Mutex<BTreeSet<&'static str>>> = OnceLock::new();
    DOWN.get_or_init(|| Mutex::new(BTreeSet::new()))
}

fn panic_text(e: &(dyn std::any::Any + Send)) -> String {
    e.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| e.downcast_ref::<String>().cloned()).unwrap_or_default()
}

/// A panic from a stub that hasn't been written yet.
pub fn is_todo(msg: &str) -> bool {
    msg.contains("not yet implemented") || msg.contains("FIG-CORE") || msg.contains("FIG-AI")
}

/// Quiet the default panic message for the stubs' `todo!()`s (they are
/// expected while the other builders work); everything else still prints.
pub fn quiet_todo_panics() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let msg = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_default();
            if !is_todo(&msg) {
                prev(info);
            }
        }));
    });
}

/// Call into the figures crate (or FIG-AI); `None` if it panicked.
pub fn core<T>(name: &'static str, f: impl FnOnce() -> T) -> Option<T> {
    if down().lock().map(|d| d.contains(name)).unwrap_or(false) {
        return None;
    }
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(v) => Some(v),
        Err(e) => {
            let msg = panic_text(&*e);
            if is_todo(&msg) {
                if let Ok(mut d) = down().lock()
                    && d.insert(name)
                {
                    eprintln!("ntex-studio: figures: {name} is not built yet");
                }
            } else {
                eprintln!("ntex-studio: figures: {name} failed: {msg}");
            }
            None
        }
    }
}

/// The pieces that are still `todo!()` (seen so far).
pub fn missing() -> Vec<&'static str> {
    down().lock().map(|d| d.iter().copied().collect()).unwrap_or_default()
}

pub fn is_down(name: &str) -> bool {
    down().lock().map(|d| d.contains(name)).unwrap_or(false)
}

#[derive(Clone, Debug, Default)]
pub struct Doc {
    pub source: String,
    pub figure: Figure,
    pub diagnostics: Vec<Diagnostic>,
    pub scene: Arc<Scene>,
    /// Bumped whenever the scene changes (renders carry it; stale ones are dropped).
    pub generation: u64,
}

impl Doc {
    pub fn new(source: &str) -> Doc {
        let mut d = Doc::default();
        d.set_source(source);
        d
    }

    /// A document with a hand-made figure and scene (tests, and the window
    /// while the parser or layout are not built yet).
    pub fn from_parts(source: &str, figure: Figure, scene: Scene) -> Doc {
        Doc { source: source.to_string(), figure, diagnostics: Vec::new(), scene: Arc::new(scene), generation: 1 }
    }

    /// Re-read the source (typing, Ask, undo, reload).
    pub fn set_source(&mut self, src: &str) {
        self.source = src.to_string();
        match core("parse", || parse::parse(src)) {
            Some((fig, diags)) => {
                self.figure = fig;
                self.diagnostics = diags;
            }
            None => {
                // (the window says which pieces are missing)
                self.figure = Figure::default();
                self.diagnostics.clear();
            }
        }
        self.relayout();
    }

    pub fn relayout(&mut self) {
        self.scene = Arc::new(scene_of(&self.figure));
        self.generation += 1;
    }

    /// After a canvas edit: format the figure, then re-read it (fresh spans).
    /// Err when the formatter is missing (the edit can't be written back).
    pub fn set_figure(&mut self, fig: Figure) -> Result<(), String> {
        let src = core("format", || format::format(&fig)).ok_or("the formatter is not built yet")?;
        // never trade a figure for an empty file (a formatter stub, or a bug)
        let has_items = fig.items.iter().any(|it| !matches!(it, Item::Comment { .. }));
        if has_items && src.trim().is_empty() {
            return Err("the formatter returned nothing (not built yet?)".into());
        }
        self.source = src.clone();
        match core("parse", || parse::parse(&src)) {
            Some((f, diags)) => {
                self.figure = f;
                self.diagnostics = diags;
            }
            None => self.figure = fig,
        }
        self.relayout();
        Ok(())
    }

    /// Apply canvas edits to a copy of the figure; the names they made.
    pub fn applied(&self, edits: &[Edit]) -> Result<(Figure, Vec<Option<String>>), String> {
        let mut fig = self.figure.clone();
        let mut names = Vec::new();
        for e in edits {
            names.push(apply(&mut fig, e)?);
        }
        Ok((fig, names))
    }

    /// Is there anything to draw?
    pub fn is_empty(&self) -> bool {
        self.scene.draws.is_empty()
    }

    pub fn errors(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Error).count()
    }
}

/// layout, or an empty scene.
pub fn scene_of(fig: &Figure) -> Scene {
    core("layout", || layout::layout(fig)).unwrap_or_default()
}

/// edit::apply, guarded.
pub fn apply(fig: &mut Figure, e: &Edit) -> Result<Option<String>, String> {
    match core("edit", || edit::apply(fig, e)) {
        Some(r) => r,
        None => Err("canvas edits are not built yet".into()),
    }
}

pub fn fresh_name(fig: &Figure) -> String {
    if let Some(n) = core("fresh_name", || edit::fresh_name(fig)) {
        return n;
    }
    // the same idea as edit::fresh_name: A, B, …, Z, A1, …
    let used = names(fig);
    for i in 0.. {
        for c in 'A'..='Z' {
            let n = if i == 0 { c.to_string() } else { format!("{c}{i}") };
            if !used.contains(&n) {
                return n;
            }
        }
    }
    unreachable!()
}

/// Every object name in the figure.
pub fn names(fig: &Figure) -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    for it in &fig.items {
        match it {
            Item::Node(n) => {
                s.insert(n.name.clone());
            }
            Item::Blob(b) => {
                s.insert(b.name.clone());
                for e in &b.elements {
                    s.insert(e.name.clone());
                }
            }
            Item::NGon(g) => {
                s.insert(g.name.clone());
            }
            Item::Cayley(c) => {
                s.insert(c.name.clone());
            }
            _ => {}
        }
    }
    s
}

/// A preview of LaTeX as plain text (unicode::preview, or the source minus
/// backslashes while it is not built).
pub fn preview(latex: &str) -> String {
    if let Some(s) = core("unicode::preview", || ntex_figures::unicode::preview(latex)) {
        return s;
    }
    latex.replace(['\\', '{', '}', '$'], "")
}

/// Styled draft runs for maths labels and free text with maths islands.
pub fn preview_runs(latex: &str, math: bool) -> Vec<(String, bool)> {
    core("unicode::preview_runs", || {
        if math { ntex_figures::unicode::preview_runs(latex) }
        else { ntex_figures::unicode::preview_text_runs(latex) }
    }).unwrap_or_else(|| vec![(preview(latex), false)])
}

/// Unicode input → LaTeX (unicode::to_latex, or as typed).
pub fn to_latex(s: &str) -> String {
    core("unicode::to_latex", || ntex_figures::unicode::to_latex(s)).unwrap_or_else(|| s.to_string())
}
