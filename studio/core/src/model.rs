//! The shared shapes. Every module reads and writes these; nothing else is
//! shared between the builders' folders.
//!
//! RULE: add-only. A builder may add a type, a field with a `#[serde(default)]`,
//! or an enum variant, and says so in its report. Nobody renames or removes
//! anything here, and nobody changes a signature in another folder's stubs.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// snippets

/// Which helper of `lua/snips/helpers.lua` a draft becomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Kind {
    /// `math` / `math_tab` / `text` (+ `{ tab = true }`): a plain trigger.
    #[default]
    Plain,
    /// `suffix`: maths, meant to follow a letter (`xsr` → `x^{2}`).
    Suffix,
    /// `post`: maths, grabs the letter before the trigger as `%1` (`xhat` → `\hat{x}`).
    Post,
    /// `pat`: maths (or text with Tab), a Lua pattern trigger; captures `%1`, `%2`, …
    Pattern,
    /// `vis`: select text in maths, type the key; `%V` is the selection.
    Visual,
    /// The whole entry is hand-written Lua (the escape hatch for anything else).
    RawLua,
}

/// Where a snippet fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Context {
    #[default]
    Math,
    Text,
}

/// By itself (autosnippet) or on Tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Fires {
    #[default]
    Auto,
    Tab,
}

/// `opts.where` (maths only).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MathWhere {
    #[default]
    Any,
    Inline,
    Display,
}

/// `opts.line` (text only): what is on the line before the trigger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LineRule {
    #[default]
    Anywhere,
    /// `line = "start"`
    Start,
    /// `line = "after text"`
    AfterText,
    /// `line = "list item"`
    ListItem,
}

/// `opts.ft`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FileType {
    #[default]
    Both,
    Tex,
    Markdown,
}

/// One piece of a snippet's output, as the token editor holds it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Piece {
    /// Literal LaTeX. May contain newlines and tabs (a tab stays a tab).
    Text(String),
    /// A tab stop. Tab stops with the same `n` are one tab stop typed in
    /// several places (a mirror: "multicursor"). `default` is the text it
    /// starts out holding (selected, so typing replaces it); "" = empty.
    /// Only the first occurrence of an `n` carries the default.
    Tab { n: u32, default: String },
    /// `<0>`: where the last Tab leaves the cursor (default: the end).
    Exit,
    /// `%1` … `%9`: text captured by a `post` / `pat` trigger.
    Capture(u8),
    /// `%V`: the selected text (`vis`).
    Selection,
}

/// A snippet being made (or edited) in the studio.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Draft {
    /// The inbox entry this draft edits; `None` for a new one.
    #[serde(default)]
    pub id: Option<String>,
    /// What she types. For `Kind::Pattern`, a Lua pattern; for `Kind::Visual`, the key.
    pub trigger: String,
    pub kind: Kind,
    pub context: Context,
    pub fires: Fires,
    #[serde(default)]
    pub math_where: MathWhere,
    #[serde(default)]
    pub line: LineRule,
    #[serde(default)]
    pub ft: FileType,
    /// `opts.priority`; 0 = not written.
    #[serde(default)]
    pub priority: i32,
    /// `opts.desc`; "" = not written.
    #[serde(default)]
    pub desc: String,
    /// `opts.eg` (example keys for the reference page; patterns need one); "" = not written.
    #[serde(default)]
    pub eg: String,
    /// `opts.check`: the *body* of `function(line, trigger, captures) … end`; "" = none.
    #[serde(default)]
    pub check: String,
    /// The origin label; the studio writes "yours".
    pub origin: String,
    pub output: Vec<Piece>,
    /// `Kind::RawLua` only: the entry's Lua, verbatim (one or more `h.…(…)` calls, comma-terminated).
    #[serde(default)]
    pub raw_lua: String,
    /// Which fields she set by hand (the rest are the studio's guesses and
    /// may be re-guessed as she types). Names: "kind", "context", "fires",
    /// "math_where", "line", "ft".
    #[serde(default)]
    pub pinned: Vec<String>,
}

impl Default for Draft {
    fn default() -> Self {
        Draft {
            id: None,
            trigger: String::new(),
            kind: Kind::Plain,
            context: Context::Math,
            fires: Fires::Auto,
            math_where: MathWhere::Any,
            line: LineRule::Anywhere,
            ft: FileType::Both,
            priority: 0,
            desc: String::new(),
            eg: String::new(),
            check: String::new(),
            origin: "yours".into(),
            output: Vec::new(),
            raw_lua: String::new(),
            pinned: Vec::new(),
        }
    }
}

/// One snippet in `snippets/tex/inbox.lua`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InboxEntry {
    /// From the entry's `-- studio: id=… made=…` marker line.
    pub id: String,
    /// ISO local time, `2026-10-07T14:32`.
    pub made: String,
    /// The entry's Lua, marker line excluded, exactly as in the file.
    pub source: String,
    /// The draft it parses back to; `None` if hand-edited past what the parser
    /// understands (then it opens as `Kind::RawLua`).
    pub draft: Option<Draft>,
    /// For lists: the trigger as written (or "?").
    pub trigger: String,
    /// 1-based line of the marker in inbox.lua.
    pub line: usize,
}

/// A snippet already loaded in ntex (from the bridge's `list`, i.e.
/// `require("snips").list()` — the `snip.ntex` metadata).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KnownSnippet {
    pub trigger: String,
    #[serde(default)]
    pub pattern: Option<String>,
    /// "math" | "suffix" | "post" | "pattern" | "text" | "visual"
    pub kind: String,
    /// "math" | "inline" | "display" | "text"
    #[serde(rename = "where")]
    pub where_: String,
    /// "auto" | "tab" | "visual"
    pub how: String,
    #[serde(default)]
    pub body: Option<String>,
    pub file: String,
    #[serde(default)]
    pub line: Option<usize>,
    #[serde(default)]
    pub origin: String,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub line_rule: Option<String>,
    #[serde(default)]
    pub ft: Option<String>,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub desc: Option<String>,
}

/// How bad a finding is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    /// Saving is refused until it is fixed ("! Trigger is empty.").
    Error,
    /// Saving is allowed; shown in the Remark box ("Studio Warning: …").
    Warning,
    /// A quiet note (a guess the studio made, an escape it applied).
    Note,
}

/// One thing the checks found about a draft or a block.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub severity: Severity,
    /// Stable id for tests and for "fix" buttons: "empty-trigger",
    /// "duplicate", "shadows", "shadowed-by", "english-words", "no-selection",
    /// "no-capture", "literal-angle", "lua-error", "env-exists", …
    pub code: String,
    /// One sentence, plain English, ready to show.
    pub message: String,
    /// A one-click fix, if there is one: the draft (or block) after applying it.
    #[serde(default)]
    pub fix: Option<Fix>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fix {
    /// Button label: "Make it Tab-only", "Use priority 1", …
    pub label: String,
    #[serde(default)]
    pub draft: Option<Draft>,
    #[serde(default)]
    pub block: Option<BlockDraft>,
}

// ---------------------------------------------------------------------------
// trying a draft in a real (headless) ntex

/// Where the bridge types the trigger.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TryRequest {
    /// The draft's Lua entry (codegen output), loaded beside all her snippets.
    pub entry: String,
    /// "tex" | "markdown"
    pub ft: String,
    /// "inline" ($…$) | "display" (\[…\]) | "text" | "text-start" (line start)
    /// | "list-item" (`\item `) | "visual" (the selection is "x+y", then the key)
    pub place: String,
    /// What to type to fire it: the trigger, or the example (`eg`) for patterns,
    /// and for post/suffix a letter first ("x" + trigger).
    pub keys: String,
    /// Press Tab after the keys (Tab snippets).
    pub tab: bool,
    /// Text typed into each tab stop in turn, with Tab between ("a", "b", …).
    pub fills: Vec<String>,
}

/// What happened, step by step. `buffer` marks the cursor with "‸" and holds
/// only the interesting line(s) (the math or text around the cursor).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct TryStep {
    /// What was typed for this step, for display: "ff", "a ⇥", "⇥".
    pub keys: String,
    pub buffer: String,
    /// (BRIDGE) The text selected after this step (a tab stop holding a
    /// default, in select mode); "‸" in `buffer` then marks where it starts.
    #[serde(default)]
    pub selected: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct TryResult {
    /// Did a snippet expand at all on the first step?
    pub expanded: bool,
    /// Which snippet expanded (trigger + file), if it was not the draft.
    #[serde(default)]
    pub expanded_other: Option<String>,
    pub steps: Vec<TryStep>,
    #[serde(default)]
    pub error: Option<String>,
    /// (BRIDGE) The trigger of the snippet that expanded (the draft's or
    /// another), as written in its file.
    #[serde(default)]
    pub expanded_trigger: Option<String>,
    /// (BRIDGE) Its file, e.g. "operations.lua" ("zz_studio_try.lua" = the draft).
    #[serde(default)]
    pub expanded_file: Option<String>,
    /// (BRIDGE) Nothing expanded: why, if the bridge can tell ("the cursor
    /// wasn't in maths", "it only fires at the start of a line", …).
    #[serde(default)]
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// blocks (new theorem-like environments in the notes theme)

/// The existing kinds a new block can look like (sets the defaults for
/// numbering and body font; colour is separate).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BlockBase {
    Theorem,
    Lemma,
    Definition,
    Example,
    #[default]
    Remark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Numbering {
    /// `\newtheorem*` (like remark)
    #[default]
    None,
    /// shares the theorem counter: "Fun fact 1.4"
    WithTheorems,
    /// its own counter, per section: "Fun fact 1.1"
    Own,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BodyFont {
    Italic,
    #[default]
    Upright,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// A block's three tones, as in ntex-notes.sty: the bar, the fill, the head.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tones {
    pub rule: Rgb,
    pub back: Rgb,
    pub head: Rgb,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum BlockColour {
    /// One of the theme's palette names: "Blue", "Violet", "Teal", "Maroon",
    /// "Green" (reuses `ntex<Name>Rule/Back/Head`), or "Proof" (the neutral grey).
    Palette(String),
    /// A new hue (degrees), turned into three tones matched to the palette's
    /// lightness and saturation (`core::colour::tones_for_hue`).
    Hue(f32),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockDraft {
    /// As printed: "Fun fact".
    pub title: String,
    /// The environment: "funfact" (letters only).
    pub env: String,
    pub base: BlockBase,
    pub numbering: Numbering,
    pub body: BodyFont,
    pub colour: BlockColour,
    /// `\crefname` singular/plural, e.g. "fun fact" / "fun facts" (numbered only).
    #[serde(default)]
    pub cref: (String, String),
    /// The snippet that types it: trigger + Tab at a line start in .tex; "" = none.
    #[serde(default)]
    pub trigger: String,
    /// The snippet asks for a title: `\begin{funfact}[<>]`.
    #[serde(default)]
    pub with_title: bool,
}

/// A block already in `templates/tex/ntex-blocks.sty` (made by the studio).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KnownBlock {
    pub draft: BlockDraft,
    /// The stanza's text in the file.
    pub source: String,
}

// ---------------------------------------------------------------------------
// sorting the inbox (Haiku agent)

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SortMove {
    /// Inbox entry id.
    pub id: String,
    /// Target file name in snippets/tex/, e.g. "operations.lua". May be a new
    /// file, then `new_title` is set.
    pub file: String,
    /// Insert after the snippet with this trigger in the target file
    /// (keeps related snippets together); `None`: at the end of the list.
    #[serde(default)]
    pub after_trigger: Option<String>,
    /// For a new file: its group title ("Probability").
    #[serde(default)]
    pub new_title: Option<String>,
    /// One short sentence, shown in the plan.
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SortPlan {
    pub moves: Vec<SortMove>,
    /// Anything the agent wants to say (kept short).
    #[serde(default)]
    pub notes: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SortOutcome {
    /// (entry id, file it went to)
    pub moved: Vec<(String, String)>,
    /// Entries that stayed in the inbox, with why.
    pub kept: Vec<(String, String)>,
    /// Folder with the files as they were before; `core::apply::undo` restores it.
    pub backup: PathBuf,
}

/// What the agent runner reports while it works.
#[derive(Clone, Debug, PartialEq)]
pub enum AgentEvent {
    /// A line of progress ("Reading operations.lua", a thought), for the log.
    Progress(String),
    Plan(SortPlan),
    Failed(String),
}

// ---------------------------------------------------------------------------
// where things live

/// Every path the studio touches. Tests point `NTEX_STUDIO_CONFIG` at a temp
/// copy of ~/.config/texnvim, so nothing real is ever written by a test.
#[derive(Clone, Debug, PartialEq)]
pub struct Paths {
    /// ~/.config/texnvim (or $NTEX_STUDIO_CONFIG)
    pub config: PathBuf,
    /// config/snippets (or $TEXNVIM_SNIPPETS)
    pub snippets: PathBuf,
    /// config/templates/tex
    pub templates_tex: PathBuf,
    /// ~/.local/state/ntex-studio (or $NTEX_STUDIO_STATE): backups, socket, log
    pub state: PathBuf,
}

impl Paths {
    pub fn inbox(&self) -> PathBuf {
        self.snippets.join("tex").join("inbox.lua")
    }
    pub fn blocks_sty(&self) -> PathBuf {
        self.templates_tex.join("ntex-blocks.sty")
    }
    pub fn notes_sty(&self) -> PathBuf {
        self.templates_tex.join("ntex-notes.sty")
    }
    pub fn prefs(&self) -> PathBuf {
        self.config.join("prefs.lua")
    }
    pub fn socket(&self) -> PathBuf {
        self.state.join("studio.sock")
    }

    pub fn from_env() -> Paths {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"));
        let config = std::env::var_os("NTEX_STUDIO_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config/texnvim"));
        let snippets = std::env::var_os("TEXNVIM_SNIPPETS")
            .map(PathBuf::from)
            .unwrap_or_else(|| config.join("snippets"));
        let state = std::env::var_os("NTEX_STUDIO_STATE")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state/ntex-studio"));
        Paths { templates_tex: config.join("templates/tex"), config, snippets, state }
    }
}

/// The studio's own settings: the `studio = { … }` table in prefs.lua
/// (`core::prefs::read`). Missing keys keep these defaults.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StudioPrefs {
    /// Hide the window after ⌘↵ saves, so focus goes back to ntex.
    pub hide_after_save: bool,
    /// "Computer Modern" | "Palatino"
    pub typeface: String,
    /// The sorting agent.
    pub sort_model: String,
    pub sort_effort: String,
    /// Render the output with tectonic (a second or so, in the background).
    pub render_preview: bool,
    /// The figure window's Ask box: the normal model and effort. "claude-…"
    /// models run through `claude -p`, the others ("gpt-…") through `codex
    /// exec` (io::figure_ai::ask_model). Defaults from figures/eval/REPORT.md.
    #[serde(default = "default_figure_model")]
    pub figure_model: String,
    #[serde(default = "default_figure_effort")]
    pub figure_effort: String,
    /// The ⚡ fast model and effort.
    #[serde(default = "default_figure_fast_model")]
    pub figure_fast_model: String,
    #[serde(default = "default_figure_fast_effort")]
    pub figure_fast_effort: String,
    /// Whether ⚡ fast is on when a figure window opens.
    #[serde(default = "default_figure_fast")]
    pub figure_fast: bool,
}

fn default_figure_model() -> String {
    "claude-sonnet-5-5".into()
}
fn default_figure_effort() -> String {
    "low".into()
}
fn default_figure_fast_model() -> String {
    "gpt-6-luna".into()
}
fn default_figure_fast_effort() -> String {
    "low".into()
}
fn default_figure_fast() -> bool {
    false
}

impl Default for StudioPrefs {
    fn default() -> Self {
        StudioPrefs {
            hide_after_save: true,
            typeface: "Computer Modern".into(),
            sort_model: "claude-haiku-5-5".into(),
            sort_effort: "high".into(),
            render_preview: true,
            figure_model: default_figure_model(),
            figure_effort: default_figure_effort(),
            figure_fast_model: default_figure_fast_model(),
            figure_fast_effort: default_figure_fast_effort(),
            figure_fast: default_figure_fast(),
        }
    }
}

/// What a second `ntex-studio` invocation (or ntex's Space n) asks the
/// running one to do (single instance, over `Paths::socket`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct OpenRequest {
    /// "snippet" | "block" | "inbox"; None = keep the current page
    #[serde(default)]
    pub page: Option<String>,
    /// Prefill the output with this text (a selection from ntex).
    #[serde(default)]
    pub output: Option<String>,
    /// Where the cursor was in ntex: "math" | "text"
    #[serde(default)]
    pub context: Option<String>,
    /// "tex" | "markdown"
    #[serde(default)]
    pub ft: Option<String>,
    /// Open (or focus) the figure window for this .fig file (absolute path).
    #[serde(default)]
    pub figure: Option<String>,
}
