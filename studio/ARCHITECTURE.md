# Ntex Studio: architecture

A small native app (GPUI) for making ntex snippets, Tab shortcuts and new
theorem-like blocks on the fly, mid-lecture. It writes the same Lua the author
writes by hand (`lua/snips/helpers.lua`, `snippets/README.md`) into
`snippets/tex/inbox.lua`. ntex already hot-reloads every file in
`snippets/tex/` (`lua/snips/init.lua`: fs_event + BufWritePost), so **a saved
snippet works in every open ntex at once**. No restart needed.

UX.md has every control, key and pixel. This file covers modules, data flow,
ownership and the rules.

## The loop it serves

    in ntex: Space n  (or ⌘-Tab to the studio)
      → studio comes forward, focus in the trigger field
      → type the trigger (literally: every key is the trigger), Return
      → type the output; Tab = a tab stop, ⇧Tab = a mirror, ⌘E = exit point
      → (the studio guesses maths/text, auto/Tab; checks conflicts; tries it
         in a real headless ntex; renders the output)
      → ⌘↵: saved to inbox.lua, window hides, back in ntex, snippet live

Later, on the Inbox page: **Sort into my files** runs Haiku 5.5 (high effort)
to choose a topic file for each inbox entry. The studio then moves the entries
itself: it backs up first, checks the result, and lets you undo.

## Modules and owners

| Folder | Owner | What |
| --- | --- | --- |
| `core/src/model.rs` | architect | shared shapes. **Add-only.** |
| `core/` (crate ntex-studio-core; `cargo test -p ntex-studio-core`) | CORE | pure logic: codegen, Lua reading, inbox file, guesses + checks, English words, blocks (.sty), colour, applying a sort plan, prefs. No GPUI, no processes. |
| `io/` (crate ntex-studio-io; `cargo test -p ntex-studio-io`) | BRIDGE | processes: headless ntex bridge, tectonic renders, the claude sorter, single instance |
| `bridge/` (Lua) | BRIDGE | `server.lua`, the headless ntex side of the bridge |
| `../lua/config/studio.lua`, `../lua/snips/helpers.lua` (escape only), `../templates/tex/` (ntex-blocks.sty + one `\RequirePackage` line), `../snippets/tex/inbox.lua`, `~/.local/bin/ntex-studio`, `../tests/cases.lua` (+ studio cases) | BRIDGE | the ntex side |
| `src/ui/widgets/` | WIDGETS | the LaTeX-looking kit: theme, fonts, boxes, controls, text field, trigger field, token editor, code view |
| `src/ui/screens/`, `src/ui/app.rs`, `src/main.rs`, `tests/renders.rs` | SCREENS | the pages, state, keys, window, offscreen renders |
| `tests/adversary_*.rs`, `verification/` | ADVERSARY | independent acceptance |

A builder edits only its own folders. The public signatures in the stubs are
the contract: keep them and add to them freely. If one must change, say so in
your report and leave the old one working.
`Cargo.toml`: add the dependencies you need and list them in your report.

## Workspace

Three crates so builders never break each other's builds: `core` (no deps but
serde), `io` (depends on core), and the app at the root (GPUI; depends on both,
re-exports them as `ntex_studio::core`, `::io`, `::model`). WIDGETS and SCREENS
share the app crate: keep it compiling after every edit (stub with `todo!()`
rather than leave it broken), because the other one builds it too.
All three share `studio/target` (cargo's lock serialises builds; that's fine).

## Data flow

    TriggerField ─┐
    TokenEditor ──┼─► Draft ──► infer::guess (un-pinned fields)
    options ──────┘      │
                         ├─► codegen::entry ──► the Proof box (live, every keystroke)
                         ├─► infer::check(known) ──► Findings (Remark box; errors block ⌘↵)
                         ├─(debounced 250 ms)─► Bridge::try_draft ──► Example box steps
                         └─(debounced 600 ms)─► Renderer::render(preview_latex) ──► image
    ⌘↵ ─► inbox::with_draft ─► Bridge::check_file ─ok─► apply::backup + inbox::save
           ─► Bridge::refresh, inbox reread ─► hide window (prefs.hide_after_save)

Everything slow (bridge, renders, agent) runs on background threads and
reports through `std::sync::mpsc` receivers. The UI polls them from
`cx.spawn` tasks (or uses `cx.background_spawn` + `await`) and never blocks
the UI thread. Results older than the current draft are dropped: each request
carries a generation number.

## The Lua the studio writes

Exactly the style of her files, one helper call per entry:

```lua
  -- studio: id=ff-1432 made=2026-10-07T14:32
  math("ff", [[\frac{<>}{<>}]], "yours"),
```

- Helper by kind/context/fires:
  - Plain: `math` (Math+Auto), `math_tab` (Math+Tab), `text` (Text+Auto),
    `text(…, { tab = true })` (Text+Tab).
  - Suffix: `suffix`. Post: `post`. Pattern: `pat` (plus `{ tab = true }`
    when Tab). Visual: `vis`.
  - Text pattern snippets: `pat` is maths-only in helpers.lua, so a Text +
    Pattern draft is an Error finding ("Pattern triggers work in maths only")
    unless written as RawLua.
- Opts table only when needed, keys in this order: `where`, `line`, `tab`,
  `ft`, `priority`, `eg`, `desc`, `check`. For example
  `{ tab = true, line = "start", ft = "tex", desc = "…" }`.
- `check` is written `check = function(line, trigger, captures) <body> end`.
- Bodies use `[[…]]` (`lua_string`). Tab stops are unnumbered `<>`/`<:x>`
  when each appears once and in order. Otherwise every tab stop is numbered
  (`<1>`, `<1:x>`). An exit point is `<0>`.
- Literal `%` → `%%`. A literal `<…>` that would read as a tab stop (`<>`,
  `<3>`, `<:x>`) → `%<…%>`. That is a new escape BRIDGE adds to
  `helpers.parse`: `%<` = literal `<`, `%>` = literal `>`. README.md and
  reference/build.mjs learn it too.
- Trailing whitespace in the output is meaningful (dropped in inline maths by
  helpers.lua). Keep it exactly.
- inbox.lua starts with a header comment ("Snippets made in Ntex Studio,
  waiting to be sorted. Safe to edit by hand.") and
  `local h = require("snips.helpers")`. It also binds every helper locally,
  `local math, math_tab, text, suffix, post, pat, vis = h.math, …`, so entries
  read like the topic files. It ends with `return h.group("Inbox", { … })`.

## Bridge protocol (JSON lines)

The server is `nvim --clean --headless -l studio/bridge/server.lua`. It starts
the real config the way `tests/driver.lua` does: a child
`nvim --embed --headless -n -i NONE`, typed into over RPC, with this
environment:

- `NVIM_APPNAME=texnvim`
- `TEXNVIM_SNIPPETS=<private copy of snippets/>`
- `TEXNVIM_NO_VIEWER=1`, `TEXNVIM_NO_COMPILE=1`, `TEXNVIM_NO_REFERENCE=1`
- no language servers: harper, copilot and texlab must not start. Find or add
  the env switch, and test for orphaned servers.

Requests and responses (one JSON object per line):

    → {"id":1,"op":"list"}                        ← {"id":1,"ok":true,"snippets":[KnownSnippet…]}
    → {"id":2,"op":"check","name":"inbox.lua","text":"…"}  ← {"id":2,"ok":true} | {"id":2,"ok":false,"error":"inbox.lua:12: …"}
    → {"id":3,"op":"try", …TryRequest}            ← {"id":3,"ok":true,"result":TryResult}
    → {"id":4,"op":"refresh"}                     ← {"id":4,"ok":true}
    → {"id":5,"op":"quit"}
    ← {"event":"restart","reason":"…"}   (unprompted: a try hit a Lua error, the
                                          child may be corrupt; the server exits
                                          and the client starts a fresh one,
                                          retrying any request in flight once)

`try`: write the entry to `<copy>/tex/zz_studio_try.lua` and reload it, so it
sits beside all her real snippets and shadowing is real. Open a scratch buffer
of `ft`, set up `place`, then type `keys` key by key (each key fully
processed, as the driver does). Press Tab if `tab`, then type each fill
followed by Tab. Record a step after each. Report which snippet expanded, from
LuaSnip's `ls.session.current_nodes` / `get_active_snip` metadata (`snip.ntex`
and `_source.file`). Afterwards, delete the try file and reload.

## Renders

`io::render`: tectonic with `-Z search-path=<templates/tex>`. The snippet
preview uses `\documentclass[varwidth,border=2pt]{standalone}` with
amsmath/amssymb/mathtools/newpxtext+newpxmath (matching her notes) and a blue
`\square` for tab stops. Block specimens use the full theme. PDF → PNG at 2×
in-process (CoreGraphics `CGPDFDocument` → `CGBitmapContext`, via
objc2-core-graphics or core-graphics) into
`state/render-cache/<hash>.png`. Typical cost is about a second, so it's
debounced and cached, and a stale result is dropped.

## The sorter

`claude -p --model <prefs.sort_model> --effort <prefs.sort_effort>`, with:

- `--output-format stream-json --verbose --json-schema <plan_schema>`
- `--allowedTools Read Grep Glob`
- cwd = the snippets folder

It never gets the user's email or anything outside the snippets folder. Use
`--bare`/no-hooks if it works without losing auth. Check `claude --help`.

The prompt gives:

- every inbox entry (id + Lua);
- each topic file (name, its first comment line, its group title, its triggers);
- the house rules: keep related snippets together, prefer existing files,
  propose a new file only for a clearly new topic.

The answer is a `SortPlan`. The studio applies it itself (`core::apply`):
backup, cut, paste and fix the helper locals, then check every touched file
with the bridge. All or nothing; Undo restores the backup. Time out after 3
minutes, and allow Cancel.

## Blocks

`templates/tex/ntex-blocks.sty` always exists. It is empty apart from its
header, and ntex-notes.sty does `\RequirePackage{ntex-blocks}` just before
`\endinput`. Each block is one stanza between marker lines:

```latex
% ── block: funfact ── made in Ntex Studio 2026-10-07 ──
\definecolor{ntexFunfactRule}{HTML}{…} … Back … Head
\theoremstyle{ntexupright}
\newtheorem*{funfact}{Fun fact}
\tcolorboxenvironment{funfact}{ntexbox=Funfact}
% ── end: funfact ──
```

The snippet for a block (`blocks::snippet`) goes to the inbox like any other.
It is a Text snippet, fired by Tab at a line start, in .tex only:
`\begin{funfact}[<>]` (with a title) or `\begin{funfact}`, then
`\t<>`, then `\end{funfact}`. It matches Castel's `beg`.

The article template comment lists the blocks: SCREENS doesn't touch it, and
the comment in `ntex-notes.sty` says custom blocks live in ntex-blocks.sty.

## Rules for every builder

- **Headless only.** No test, probe or check may show a window, activate the
  app, or open Skim, Preview, a browser or any viewer. GPUI checks use
  `HeadlessAppContext` with `gpui_platform::current_headless_renderer`
  (offscreen Metal).
  **Never run the `ntex-studio` binary itself in automated checks.** The
  hands-on pass is done by a person.
- **Never touch her real files in tests.** Point `NTEX_STUDIO_CONFIG` (and
  `TEXNVIM_SNIPPETS`) at a temp copy of `~/.config/texnvim`, and point
  `NTEX_STUDIO_STATE` at a temp dir.
- The ntex test suite (`~/.config/texnvim/tests/run.sh`, headless) must still
  pass after your change. Run the suites you could affect. Always use
  `TEXNVIM_NO_VIEWER=1`, which run.sh sets.
- No orphan processes: every nvim/tectonic/claude child is killed on drop, on
  quit and on panic.
- Don't commit. The architect commits.
- Reference source for GPUI: the patched Zed copy Cargo checks out from
  github.com/elenaOnline/gpui-patched (under `~/.cargo/git/checkouts/`).
  GPUI's examples/input.rs shows a complete IME text field.
