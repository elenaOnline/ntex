# ntex manual

> Written as the author's own manual, so "your snippets" and "your Latex Suite
> triggers" mean the collection in `snippets/tex/`. Paths assume the repo is
> cloned to `~/.config/texnvim`. The front page is [../README.md](../README.md).

A separate Neovim setup for fast live LaTeX: lecture notes, problem sets, scratch
work. Snippets are native LuaSnip, in the style of Gilles Castel's set, with
your Latex Suite triggers carried over: `mk`, `dm`, `//`, `x2`, `uu`, `odp` …
behave the way your fingers expect, and commands like `\subset` or `\approx`
type as written. (Until 2026-09-30 it read Obsidian's `snippets.js`; see
[Snippets](#snippets) and `snippets/DECISIONS.md`.)

It never touches a normal Neovim setup (`~/.config/nvim`): everything lives in the
folders listed under [Uninstall](#uninstall).

---

## Start

In cmux (or any terminal):

```sh
ntex lecture-03.tex        # any .tex file, anywhere on disk
ntex notes.md              # markdown with $…$ and $$…$$ works too
ntex                       # then type :Scratch and Enter for a throwaway .tex
```

A new or empty `.tex` file starts from `templates/article.tex` (article class,
amsmath/amssymb/amsthm, theorem/lemma/definition/… environments). Edit that file
to change the template.

You are in **normal mode** when Neovim opens. Press `i` to start typing
(**insert mode**, the bottom line says `-- INSERT --`). Press `Esc` to stop.

## The Vim you need (cheat sheet)

| Keys | What it does |
| --- | --- |
| `i` | start typing (insert mode) at the cursor; `A` = at the end of the line |
| `Esc` | stop typing (back to normal mode) |
| `Ctrl-s` | save (works while typing). `:w` Enter also saves |
| `:q` Enter | close. `:q!` Enter = close without saving |
| `Ctrl-z` | undo, even while typing. In normal mode: `u` undo, `Ctrl-r` redo |
| `/word` Enter | find "word"; `n` next match |
| `:e path/to/file.tex` Enter | open another file |
| `dd` / `yy` / `p` | (normal mode) cut line / copy line / paste |
| `v`, then arrows | select text; `Ctrl-g` turns it into a *typing* selection (see visual snippets) |
| `gg` / `G` | jump to top / bottom |
| `:Scratch` Enter | new timestamped scratch .tex in `~/scratch` (`:Scratch ~/somewhere` for another folder) |
| Space `r` | snippet report (what loaded, file by file; load errors) |
| Space `z` | fix the word under the cursor (Harper's suggestions; see [IDE features](#ide-features)) |
| Space `f` / `o` / `b` | find a file in this folder / recent files / open files |
| Space `d` · `]d` `[d` · Space `x` | this line's errors in full · next / previous error · all of them |

The mouse works: click to move, drag to select, scroll.

**Mac keys, as in any Mac text field** (insert and normal mode; also on the `:` line):
`Option-Left` / `Option-Right` jump a word, `Option-Backspace` deletes the word
before the cursor. Word rules follow macOS: `\`, `{`, `$`, spaces separate
words; `don't` and `3.14` are one word; line breaks are crossed.
`⌘C` / `⌘X` copy / cut the selection (mouse drag, Shift-arrows, `v`) to the
Mac clipboard, and `⌘V` pastes, in any mode; pasting over a selection leaves
the clipboard as it was. Vim's own keys (`y`, `d`, `p`, `x` …) are unchanged.
cmux 0.64.x swallows `⌘C` when the terminal itself has nothing selected; add
`keybind = super+c=unbind` to
`~/Library/Application Support/com.cmuxterm.app/config.ghostty` (cmux-only, so
Ghostty.app keeps its `⌘C`) and run `cmux reload-config`. Not needed from cmux
0.65.1, which passes `⌘C` on to Neovim itself.
`⌘Z` undoes and `⌘⇧Z` redoes, in any mode; in insert mode you stay in insert
mode and `⌘Z` takes back what you typed since the last break; with a selection,
the selection goes away first. Vim's own `u` and `Ctrl-R` are unchanged.
(`lua/config/macos_keys.lua`)

## Maths keys (insert mode)

| Keys | What happens |
| --- | --- |
| type a trigger | auto snippets expand as you type (`mk` → `$…$`, `x2` → `x_{2}`, `//` → `\frac{}{}` …); letter triggers only at the start of a word (`A uu` → `A \cup`) |
| `Tab` | 1. expand a Tab snippet (e.g. `\sum` Tab) 2. jump to the next tabstop 3. in a matrix: step out of a bracket on this line, else insert ` & ` 4. step past the next `)`/`}`/`]`/`\rangle`… or out of `$…$` 5. a normal Tab |
| `Shift-Tab` | previous tabstop |
| `/` in math | auto-fraction: `a/` → `\frac{a}{…}`; `(a+b)/` → `\frac{a+b}{…}` |
| `)` `]` `}` | if the same bracket is right after the cursor, step over it |
| `{` right after `_{` | skipped (`_` already opened the brace), so `x_{i}` typed as written works |
| `Enter` | in a matrix/cases/align: ` \\` and a new line; otherwise a new line |
| `Shift-Enter` | in a matrix: leave the row / the environment |
| `Backspace` | in an empty `$|$`: delete both dollars |
| select + type `(` `[` `{` `S` `C` `K` `U` `O` `B` `/` | wrap the selection (visual snippets) — see below |
| `Ctrl-Space` | show completions (`\ref{`, `\cite{`, your `\newcommand`s via texlab) |
| `↓`/`↑` or `Ctrl-n`/`Ctrl-p`, then `Enter` | pick a completion. `Enter` only accepts after you picked one, so it never eats a matrix newline |
| `F2` | conceal on/off: `\alpha` shown as α, etc. (on by default — set in prefs.lua) |

The completion menu opens by itself only after a backslash, inside `\ref{` /
`\cite{` / `\begin{` …, and for file paths. `Tab` never accepts a completion — it
belongs to snippets.

**Visual snippets.** Select text with the mouse or `Shift`+arrows (that gives a
*typing* selection), then type `S` → `\sqrt{ … }`, `(` → `( … )`, `C` → `\cancel{…}`,
`U` → `\underbrace{…}_{…}`, `/` → `\frac{…}{}`. The result stays selected, so you can chain them.
From a `v` selection press `Ctrl-g` first. Outside math the key just replaces the
selection as usual.

## Snippets

```
~/.config/texnvim/snippets/
  tex/*.lua        your snippets: one file per topic, one snippet per line
  README.md        how a line reads, all the helpers, how to add one
  DECISIONS.md     every decision of the move from Latex Suite (2026-09-30)
```

They are ordinary LuaSnip snippets, loaded with LuaSnip's `from_lua` loader;
.md files get the same ones. A line reads trigger, output, origin. Three,
copied from the files (`tex/sets.lua`, `tex/operations.lua`, `tex/calculus.lua`):

```lua
  math("uu",       [[\cup]],                       "yours"),
  math("//",       [[\frac{<>}{<>}]],     "default"),
  math("dint",      [[\int_{<:0>}^{<:1>} <> \, d<:x> ]],                  "changed", { desc = "Definite integral" }),
```

`<>` is a tabstop (Tab jumps to the next, then out), `<:0>` a tabstop holding
`0`. `math` fires by itself in maths at the start of a word; `math_tab` needs
Tab; `suffix`, `post` (`xhat` → `\hat{x}`), `pat` (Lua patterns: `x2` →
`x_{2}`), `env`, `text` and `vis` (select, then a key) are the others —
`snippets/README.md` explains each.

- **Make one on the fly:** Space `n` opens Ntex Studio (`studio/`, a small
  native app): type the trigger, Return, the output (Tab for a tab stop), ⌘↵.
  It lands in `snippets/tex/inbox.lua` and works in every open ntex at once.
  In visual mode Space `n` starts from the selection; Space `N` is the inbox,
  where Haiku sorts the new ones into your files. Build it once:
  `~/.config/texnvim/studio/build.sh`.
- **Add or change one:** Space `e` (`:SnippetEdit`) picks a file; copy a line
  close to what you want, change it, save. That file reloads at once (also when
  another program saves it). A file with an error keeps its previous snippets;
  Space `r` (`:SnippetReport`) shows the error and every loaded snippet.
- **No mangling:** a trigger never fires inside a `\command` or a word, nor
  inside `\text{}`, `\label{}`, `\ce{}`, a `%` comment. 234 common commands
  typed as written come out unchanged (68 didn't with Latex Suite). Letter
  triggers fire at the start of a word: `A uu` → `A \cup`, `Auu` stays.
  `fire_after_letters = true` in prefs.lua (`latex = { … }`) lets them fire after
  a letter again.
- **Test:** `~/.config/texnvim/tests/run.sh` (all suites; see [Tests](#tests)).
- **Settings** (auto-fraction, tabout, matrix keys, …): `latex = { … }` in
  prefs.lua, below.
- `~/.config/latex-suite/` is no longer read by ntex.

## Compile and view

- **Live PDF:** when you pause typing (0.8 s), the file saves itself and
  recompiles (tectonic; a `.pdf` appears next to the `.tex`). With Skim's *Check
  for file changes* on, the PDF updates as you work. A compile that fails
  mid-equation keeps the last good PDF, and the error list stays closed — `\le`
  shows it. Turn off or change the pause in `prefs.lua`. `Ctrl-s` still saves
  (and compiles) right away.
- `\lv` (backslash, l, v in normal mode) shows the PDF **in a cmux pane** to the
  right (`pdf_viewer` in `prefs.lua`): `"tdf"` (default — a terminal PDF viewer
  that reloads itself and keeps your page; `j`/`k` or arrows to move, `q` to
  close), `"browser"` (cmux browser split, sharp and selectable, reloaded after
  each compile) or `"skim"`. Outside cmux, or if cmux refuses, it uses Skim.
- `\ls` opens **Skim** at the cursor (SyncTeX). Skim only ever opens when you
  ask. The first time, macOS asks whether the terminal may control Skim — click *OK*.
- `\ll` compile now · `\le` show errors · `\lc` delete build files · `\lt` table of contents
- Let Skim refresh the PDF by itself: Skim → Settings… (⌘,) → *Sync* → tick
  *Check for file changes* (and *Reload automatically* if offered).
- Optional, click in Skim to jump to the source: Skim → Settings… → *Sync* →
  *Preset*: **Custom**, *Command*: `/usr/bin/env`, *Arguments*:
  `NVIM_APPNAME=texnvim /opt/homebrew/bin/nvim --headless -c "VimtexInverseSearch %line '%file'"`.
  Then ⌘⇧-click in the PDF.

## IDE features

Errors where they happen, grammar and spelling, Copilot ghost text and a file
finder. Each has a switch in `prefs.lua` (below); the code is in `lua/ide/`.

### Errors where they happen

After every compile (the live-PDF pause, `Ctrl-s`, `\ll`) the tectonic log is
read and each error lands **on its line**: a red underline under the exact
command, `E` in the line-number column, and the message, shortened, at the end
of the line. Warnings (an undefined `\ref`, …) are amber with `W`. A clean
compile clears them. `\input` files get theirs too.

| Keys | |
| --- | --- |
| Space `d` | this line's messages in full, in a small float (moving closes it) |
| `]d` / `[d` | next / previous error, warning or grammar note; its message opens on arrival |
| Space `x` | every error, warning and grammar note in a list with a preview; Enter jumps |

- **While you type**, an error on the line you are typing in waits until you
  press `Esc`, since live PDF compiles mid-formula. Errors elsewhere show at once.
- **texlab** checks as you type (duplicate labels, undefined `\ref`s, unmatched
  braces) without compiling; its notes appear when you leave insert mode.
- TeX stops at the first error, so you see one compile error at a time.
- Overfull / underfull `\hbox` warnings are hidden; `box_warnings = true`
  shows them as faint hints.
- Why the note sits at the end of the line and not as a full message under it:
  a LaTeX paragraph is often one long wrapped line, and a block of text opening
  under the cursor line would push the screen down every time the cursor lands
  on a flagged line. The end-of-line note never moves text; Space `d` gives the
  whole message.
- This also fixes an old blind spot: VimTeX's own error list (`\le`) reads
  pdflatex's log format, so it never saw tectonic's "Undefined control
  sequence" or "Missing $", and called those compiles successful.

### Grammar and spelling (Harper)

[Harper](https://writewithharper.com) (`brew install harper`, runs on your Mac
only) checks .tex and .md as you write: quiet blue underlines, no signs.
Vim's own spell check is off where Harper runs, so nothing is underlined twice.

- Space `z` on an underlined word: Harper's fixes (replace with …, add to your
  dictionary, ignore); Enter takes the first. Without Harper it does what it
  did before (Vim's first suggestion). `z=` still lists Vim's suggestions.
  Space `z` moved to Harper because its fixes cover grammar too ("a apple",
  "teh"), and *add to dictionary* is one of them.
- Maths is never checked: Harper itself skips `$…$` and maths environments,
  and ntex drops anything it flags inside `\[…\]`, `\(…\)` and `$$…$$` (Harper
  reads those as prose). Commands, labels, `\cref` and environment names are
  not flagged.
- `harper_dialect = "British"` (or Australian, Canadian, Indian) in prefs.lua.
  Your dictionary: `~/Library/Application Support/harper-ls/dictionary.txt`.

### Copilot ghost text

GitHub Copilot suggests the rest of a sentence or formula in grey italics while
you type in .tex and .md files. Neovim's built-in inline completion and GitHub's
own server do the work (no plugin).

| Keys (insert mode) | |
| --- | --- |
| `Alt-l` (Option-l) | accept the suggestion |
| `Alt-]` / `Alt-[` | next / previous suggestion |
| Space `a` (normal mode) | Copilot on / off; says which |

`Tab` never accepts a suggestion: it stays with snippets and tabout.

**Sign in once** (nothing appears until you do; until then ntex says
`Copilot: run :LspCopilotSignIn` once per session):

1. `ntex notes.tex` (any .tex or .md file; the command exists only there).
2. Type `:LspCopilotSignIn` and Enter. Neovim copies a one-time code (like
   `ABCD-1234`) to the clipboard and asks *Open the browser to complete the
   sign-in process?*. Press `y`.
3. The browser opens **https://github.com/login/device**. Sign in to GitHub if
   asked, paste the code (⌘V), click **Continue**, then **Authorize**.
4. Back in ntex: `Signed in as <you>.` Done; it is remembered (in
   `~/.config/github-copilot/`).

**Which plan.** Copilot needs Copilot on your GitHub account:
- *Copilot Free*: no card, a monthly allowance of completions (2,000 when this
  was written). Ghost text stops for the rest of the month once it is used up.
- *Student plan*: verified students get full Copilot free through GitHub
  Education (apply at https://education.github.com/pack with your university
  email; approval can take a few days), with no completion limit.

Either way, turn it on at https://github.com/settings/copilot, which also shows
what you have.

**Privacy.** While Copilot is on, the text around the cursor in the file you
are editing is sent to GitHub to get suggestions. Space `a` turns it off for the
session (the server stops, so nothing is sent); `copilot_at_start = false`
starts ntex with it off; `copilot = false` never starts it. ntex turns the
editor's usage telemetry off. Whether GitHub may keep and use your snippets
for product improvement is a setting at https://github.com/settings/copilot.

**Sign out:** `:LspCopilotSignOut` in a .tex / .md file.

**Update the server** (installed only for ntex, version 1.551.2):
`npm install --prefix ~/.local/share/texnvim/copilot @github/copilot-language-server@latest`.

### File finder

| Keys | |
| --- | --- |
| Space `f` | files in this file's folder |
| Space `F` | files anywhere under `~` (not `~/Library`, `.git`, `node_modules`) |
| Space `o` | recent files |
| Space `/` | search the text of the files in this file's folder |
| Space `b` | open files |
| Space `x` | errors, warnings, grammar notes |

Type to filter, Enter opens, Esc closes; `Ctrl-v` / `Ctrl-s` open in a split.
It is snacks.nvim's picker in Modus colours: a small list at the top, no
icons, no preview (text search and the error list get a preview). Needs `fd`
and `rg` (`brew install fd ripgrep`).

## Your ntex preferences (`prefs.lua`)

`~/.config/texnvim/prefs.lua` — edit, save, restart `ntex`:

| Key | Default | |
| --- | --- | --- |
| `images` | `false` | rendered math images in the text (below) |
| `conceal` | `true` | pretty symbols at start. F2 toggles |
| `live_pdf` | `true` | save + compile when you pause typing |
| `live_pdf_delay_ms` | `800` | how long a pause |
| `pdf_viewer` | `"tdf"` | where `\lv` shows the PDF: `"tdf"`, `"browser"`, `"skim"` |
| `conceal_scripts` | `true` | also conceal `_{i}` → ᵢ; `false` keeps sub/superscripts as typed |
| `compile_errors` | `true` | compile errors on their line after every compile |
| `box_warnings` | `false` | overfull / underfull `\hbox` as faint hints |
| `harper` | `true` | Harper grammar and spelling (replaces Vim's spell check in .tex / .md) |
| `harper_dialect` | `"American"` | `"British"`, `"Australian"`, `"Canadian"`, `"Indian"` |
| `copilot` | `true` | Copilot ghost text at all (`false`: never started) |
| `copilot_at_start` | `true` | on when ntex starts; Space `a` switches |
| `latex = { … }` | your Latex Suite values | maths typing: `autofraction`, `tabout`, `matrix_envs`, `enlarge_triggers`, `auto_delete_dollar`, `remove_snippet_whitespace`, `fire_after_letters`, … (each explained in prefs.lua) |

**Font:** JuliaMono, set for cmux in `~/.config/ghostty/config` (proper-size
sub/superscripts and maths symbols for conceal).

**Colours:** Modus (`lua/config/theme.lua`) — high contrast (WCAG AAA), light or
dark following the terminal. Concealed symbols use the math colour instead of
the usual pale grey. Change colours in `on_highlights` there.

## Rendered math in the terminal (snacks.nvim) — off by default

Turn on with `images = true` in `prefs.lua` (or once: `TEXNVIM_IMAGES=1 ntex file.tex`).
In cmux and Ghostty, math is then drawn as images right in the text (kitty graphics
protocol + unicode placeholders). The line you are on always shows the LaTeX
source; move off it to see the rendering. The first formula takes a few seconds
(tectonic warms up); later ones are quick and cached.

- Needs: `tectonic` (or `pdflatex`), ImageMagick (`magick`) and **Ghostscript
  (`gs`)** — ImageMagick uses gs to turn the PDF into a PNG.
- The `ntex` launcher tells snacks it is running in a Ghostty-class terminal
  (`SNACKS_GHOSTTY=true`) when it sees cmux/Ghostty, because snacks otherwise
  relies on the terminal's XTVERSION reply.
- Inside tmux, images are off (cmux doesn't pass kitty graphics through tmux).
- One-off override: `TEXNVIM_IMAGES=0` / `TEXNVIM_IMAGES=1` before `ntex`.

## Reference page

`Space ?` (or `:Reference`) opens an all-in-one reference: the daily loop,
**every snippet with a verified example** (typed into a headless ntex, so it
shows what really comes out), the **migration decisions** (every change from
Latex Suite, before and after, filterable; “needs your call” marked), maths
typing features, Vim keys (the ntex / VimTeX
ones read from the live keymap), LaTeX style standards, a symbol finder
("how do I type ⊆?"), settings and files, commands, troubleshooting. One global
search: press `/` on the page. In cmux it opens in a browser split to the right
and the cursor stays in Neovim; elsewhere in your default browser. Works offline
(KaTeX is bundled in `reference/vendor/katex`).

- It rebuilds by itself in the background when you save a snippet file (you
  only hear about it if that fails);
  `:Reference` rebuilds first if anything else changed, `:Reference!` always.
- By hand: `node ~/.config/texnvim/reference/build.mjs` (`--full` re-types every
  example). Examples that didn't expand as their text suggests are listed in
  `reference/build-report.txt`, along with contrast numbers.
- Words live in `reference/content/*.mjs` (Vim keys, symbols, the LaTeX guide,
  troubleshooting): edit, then `:Reference`. Examples for pattern snippets are
  the `eg = "…"` on their line in the snippet files.
- Without Neovim: `open ~/.config/texnvim/reference/index.html`.

## Commands

| Command | |
| --- | --- |
| `:SnippetReport` | loaded snippets per file, load errors (Space `r`) |
| `:SnippetEdit` | pick a snippet file to open (Space `e`) |
| `:SnippetReload` | reload every snippet file (saving one already does) |
| `:SnippetStudio [page]` | Ntex Studio: `snippet` (Space `n`), `block`, `inbox` (Space `N`) |
| `:Figure [name]` | Ntex Figures: same as Space `g`; with a name, make/open `figures/NAME.fig` and include it |
| `:Conceal` | same as F2 |
| `:LatexSuiteReport` … | the old names of the four above, still working |
| `:Scratch [dir]` | new scratch .tex |
| `:Reference` | the reference page (`Space ?`); `!` = rebuild first |
| `:LspCopilotSignIn` / `:LspCopilotSignOut` | Copilot sign in / out (in a .tex / .md file) |
| `:checkhealth vim.lsp` | which language servers run (texlab, harper_ls, copilot) |
| `:Lazy` | plugin manager (`:Lazy restore` = back to the versions in `lazy-lock.json`) |
| `:checkhealth snacks luasnip` | self-check (`:checkhealth vimtex` asks macOS about Skim, which may show a permission prompt) |

## Tests

```sh
~/.config/texnvim/tests/run.sh             # everything (a few minutes), no windows open
~/.config/texnvim/tests/run.sh mine       # just your snippets
```

Suites: `mine` (your snippets in .md and .tex, macOS keys, hot reload),
`defaults` (the Latex Suite defaults you kept, and the ones that stay off),
`mangling` (the 234 symbol-finder commands typed as written come out
unchanged, except an allowlist in `reference/content/symbols.mjs`), `english`
(ordinary text, also inside `\text{}`, stays as typed), `decisions` (every
“now” example in `snippets/decisions.json` typed again), `perf`
(per-keystroke cost), `system` (health, maths rendering to PNG, template,
:Scratch, conceal, texlab completion; compiling only with
`TEXNVIM_TEST_COMPILE=1`, since a compile can make VimTeX talk to Skim),
`ide` (compile errors land on the right line for an undefined command, a
missing `$`, an unbalanced `{`, a missing `\end{align}`, an undefined `\ref`
and an `\input` file, and clear after a fix; texlab and Harper notes; Copilot
unauthenticated: server, keys, toggle, Tab still a snippet key; the six finder
keys open and close. It compiles with tectonic), `figures` (Space `g` finds
the figure on an `\includegraphics` line or makes a new one, `.fig` syntax,
`ntex-fig check` messages as diagnostics from a fake `ntex-fig`, a figure's
PDF changing recompiles the `.tex`; never launches the window, never
compiles), `reference` (the reference
page builds and lists every snippet; written to the temp folder). Every suite
runs with `TEXNVIM_NO_VIEWER=1`: no Skim, no cmux pane, no browser, no Ntex Studio. They copy the snippets folder to a temp folder first and never
modify `~/.config/texnvim/snippets`.

Switches for headless runs (tests, the reference build, Ntex Studio's bridge),
set in the environment before `nvim`: `TEXNVIM_NO_VIEWER=1` (no viewer, ever),
`TEXNVIM_NO_COMPILE=1` (nothing compiles), `TEXNVIM_NO_REFERENCE=1` (no
reference rebuilds), `TEXNVIM_NO_LSP=1` (no language servers: texlab,
harper-ls and Copilot never start), `TEXNVIM_NO_WATCH=1` (snippet files reload
only when asked, not when another program saves them), `TEXNVIM_SNIPPETS=DIR`
(another snippets folder).

## Files

```
~/.local/bin/ntex                 launcher (NVIM_APPNAME=texnvim)
~/.local/bin/ntex-studio          Ntex Studio launcher (the release build in studio/target)
~/.local/bin/ntex-fig              Ntex Figures CLI launcher: build | check | tikz | fmt a .fig
~/.local/share/texnvim/copilot/   GitHub's Copilot language server (npm, for ntex only)
~/.config/texnvim/
  prefs.lua                       YOUR ntex preferences (images, conceal, live PDF)
  init.lua                        loads everything below
  lua/config/options.lua          wrap, spell, conceal defaults, mouse, clipboard
  lua/config/keymaps.lua          Ctrl-s, Ctrl-z, Space shortcuts
  lua/config/theme.lua            colours (Modus + readable conceal)
  lua/config/macos_keys.lua       Option-Left/Right/Backspace like macOS
  lua/config/livepdf.lua          save + compile when you pause typing
  lua/config/pdfview.lua          \\lv: PDF in a cmux pane (tdf / browser) or Skim
  lua/config/commands.lua         :Scratch, new-file template
  lua/config/reference.lua        :Reference / Space ?: build + open the reference page
  lua/config/studio.lua           :SnippetStudio / Space n, Space N: launch Ntex Studio
  lua/config/figures.lua          :Figure / Space g: Ntex Figures (.fig next to the .tex, built to PDF,
                                  rebuilt on save, the live PDF follows); syntax/ + ftplugin/ntexfig
  lua/config/lsp.lua              texlab
  lua/config/ide.lua              IDE features: loads lua/ide/, Space z
  lua/ide/                        diagnostics.lua (how errors look, Space d), compile_errors.lua
                                  (tectonic log → errors), harper.lua, copilot.lua, finder.lua
  lua/plugins/core.lua            plugins: VimTeX, LuaSnip, blink.cmp, treesitter, snacks, nvim-lspconfig
  lua/snips/                      snippets and maths typing
    init.lua        LuaSnip setup, loading + hot reload of snippets/, commands
    helpers.lua     the snippet vocabulary (math, post, pat, env, vis, …)
    cond.lua        when a snippet may fire (word starts, \commands, maths/text)
    context.lua     math / text / \text{} / comment detection (VimTeX for .tex)
    mdmath.lua      $ / $$ scanner for .md
    features.lua    Tab, Enter, auto-fraction, tabout, matrix keys, enlarge brackets
    visual.lua      visual snippets in select mode
    settings.lua    prefs.lua latex = { … } → behaviour
    conceal.lua     F2 conceal (.tex via VimTeX, .md own)
    report.lua      :SnippetReport
  snippets/                       your snippets (tex/*.lua), README.md, DECISIONS.md, decisions.json
  tools/conceal_maps.json         symbol tables for .md conceal (from Latex Suite)
  tools/migration/                how DECISIONS.md was made (old snippets snapshot, results typed into the old setup)
  templates/article.tex           template for new .tex files
  templates/tex/                  the notes theme (ntex-notes.sty) + ntex-blocks.sty (blocks made in Ntex Studio)
  studio/                         Ntex Studio: the app (Rust), its headless-ntex bridge (bridge/server.lua), build.sh
  tests/                          run.sh + driver
  reference/                      the reference page: build.mjs, verify.lua (headless typing),
                                  content/*.mjs (the words), client/ (CSS/JS), vendor/katex/
  lazy-lock.json                  exact plugin versions
```

## Uninstall

```sh
rm -rf ~/.config/texnvim ~/.local/share/texnvim ~/.local/state/texnvim ~/.cache/texnvim
rm ~/.local/bin/ntex
```

That's all of it (`~/.local/share/texnvim` includes the Copilot server).
`brew uninstall harper fd ripgrep` removes the tools the IDE features use.
