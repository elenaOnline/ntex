# Snippet migration: decisions (2026-09-30)

ntex moved from Obsidian's Latex Suite format (~/.config/latex-suite/snippets.js, read through a converter) to native LuaSnip snippets in ~/.config/texnvim/snippets/tex/*.lua, in the style of Gilles Castel's set. This file lists every decision. The same list, with rendered maths and filters, is the **Migration decisions** section of the reference page (Space ? in ntex).

Every example was typed into the old setup (before) and the new one (now) by a headless ntex: before the old ntex (Latex Suite engine, git tag pre-luasnip), typed headless, 2026-09-30 17:33; now the new ntex (LuaSnip, snippets/tex/*.lua), 2026-09-30 17:35. ‸ marks the cursor. `tests/run.sh decisions` types every “now” again.

Roll back the whole migration: `git -C ~/.config/texnvim checkout pre-luasnip` (and `git -C ~/.config/texnvim checkout main` to come back).

| Section | Entries |
| --- | --- |
| [Needs your call](#needs-your-call) | 5 |
| [Behaviour changes to your snippets](#behaviour-changes-to-your-snippets) | 52 |
| [Kept as is](#kept-as-is) | 132 |
| [New from Castel](#new-from-castel) | 20 |
| [Castel differs, kept yours](#castel-differs-kept-yours) | 43 |
| [Dropped](#dropped) | 33 |
| [Still turned off](#still-turned-off) | 64 |

## Needs your call

Real choices. Each has a recommendation, and what is in place now is that recommendation. Say which way you want it and it's a one-line change.

### Letter triggers no longer fire right after a letter

Trigger: `uu, xx, sub, … (all letter triggers)`

**Decided 2026-09-30:** Keep it off: letter triggers don't fire right after a letter (fire_after_letters = false).

**Now:** A trigger that starts with a letter fires only at the start of a word: `A uu` gives `A \cup`, but `Auu` stays `Auu`. This is Castel's rule, and the main reason `\subset`, `\approx`, `\emptyset` … now type as written.

**Before:** Latex Suite fired them anywhere: `Auu` → `A\cup`, `axx` → `a\times`.

**Alternative:** `fire_after_letters = true` in prefs.lua (`latex = { … }`): letter triggers fire after a letter again (`Auu` → `A\cup`), but still never inside a `\command` name. The mangling test passes with either setting.

**Recommendation:** Keep it off (as now). One space before the trigger is all it takes, and two letters typed together in maths stay two letters. Turn it on if your fingers keep typing `Auu`.

Suffix snippets are not affected: `xsr` → `x^{2}`, `xhat` → `\hat{x}`, `x2` → `x_{2}`, `Adag` → `A^{\dagger}` still work after a letter. Two knock-on effects: after a letter `xp1` and `he4` now act as a subscript (`Axp1` → `Axp_{1}`), and `Acdot` stays `Acdot`.

- .tex: in `$A‸$` type `uu` → before `$A\cup‸$`, now `$Auu‸$`
- .tex: in `$A‸$` type `xx` → before `$A\times‸$`, now `$Axx‸$`
- .tex: in `$A‸$` type `sub` → before `$A\subseteq‸$`, now `$Asub‸$`
- .tex: in `$A‸$` type `sq` → before `$A\sqrt{ ‸ }$`, now `$Asq‸$`
- .tex: in `$A ‸$` type `uu` → before `$A \cup‸$`, now `$A \cup‸$`
- .tex: in `$‸$` type `sr` → before `$^{2}‸$`, now `$^{2}‸$`
- .md: in `$‸$` type `sr` → before `$^{2}‸$`, now `$^{2}‸$`
- .tex: in `$‸$` type `xhat` → before `$\hat{x}‸$`, now `$\hat{x}‸$`
- .md: in `$‸$` type `xhat` → before `$\hat{x}‸$`, now `$\hat{x}‸$`
- .tex: in `$A‸$` type `cdot` → before `$A\cdot‸$`, now `$Acdot‸$`

### mk in .tex files: $…$ or \(…\)?

Trigger: `mk`

**Decided 2026-09-30:** $…$ in .tex too.

**Now:** `mk` gives `$…$` in .tex and .md files, as before.

**Alternative:** `\(…\)` in .tex files: what LaTeX's own documentation prefers (clearer errors when a `$` goes missing).

**Recommendation:** Keep `$…$`. It's what you type everywhere else, what Castel uses, and Backspace-in-empty-`$|$` and Tab-out-of-`$` are built around it. Nothing breaks either way.

To switch: in snippets/tex/text.lua make the mk line `text("mk", function() return vim.bo.filetype == "tex" and [[\(<>\)]] or [[$<>$]] end, "changed")`.

- .tex: in `Let ‸` type `mk` → before `Let $‸$`, now `Let $‸$`
- .tex: in `‸` type `mk` → before `$‸$`, now `$‸$`
- .md: in `‸` type `mk` → before `$‸$`, now `$‸$`

### Auto subscript only after a single letter or a Greek letter

Trigger: `x2 (auto subscript)`

**Decided 2026-09-30:** Single letter or Greek letter only; log2 stays log2 (type log_2).

**Now:** `x2` → `x_{2}`, `ab2` → `ab_{2}`, `\alpha2` → `\alpha_{2}`, but the last letter of any other `\command` no longer gets one: `\leq2` and `\sqrt2` stay as typed.

**Before:** The letter could be the end of a command: `\leq2` → `\leq_{2}`, `\sqrt2` → garbage. It also happened to give `log2` → `\log_{2}` and `sin2` → `\sin_{2}`.

**Alternative:** Allow a subscript after a few named commands as well (`\log`, `\lim`, …): one line each in snippets/tex/subscripts.lua.

**Recommendation:** Keep it (as now). For `\log_{2}` type `log_2`: `_` adds the braces.

- .tex: in `$‸$` type `\leq2` → before `$\leq_{2}‸$`, now `$\leq2‸$`
- .tex: in `$‸$` type `\sqrt2` → before `$\\sqrt{ rt_{2}‸ }$`, now `$\sqrt2‸$`
- .tex: in `$‸$` type `log2` → before `$\log_{2}‸$`, now `$\log2‸$`
- .tex: in `$‸$` type `sin2` → before `$\sin_{2}‸$`, now `$\sin2‸$`
- .tex: in `$‸$` type `log_2` → before `$\log_{2‸}$`, now `$\log_{2‸}$`
- .tex: in `$‸$` type `ab2` → before `$ab_{2}‸$`, now `$ab_{2}‸$`
- .tex: in `$‸$` type `\alpha2` → before `$\alpha_{2}‸$`, now `$\alpha_{2}‸$`

### Castel additions: keep all 20?

Trigger: `20 triggers`

**Decided 2026-09-30:** Keep all 20.

**Now:** 20 snippets from Castel's set are on (see “New from Castel”): `pi`, `zeta`, `star`, `perp`, `lll`, `mcal`, `DD`, `~~`, `<!`, `<>`, `compl`, select-then-`/`, `lr`/`lrb` with Tab, and `table`, `fig`, `desc`, `pac`, `plot`, `bigfun` with Tab at the start of a line in .tex.

**Alternative:** Veto any of them: delete its line (they are marked `"castel"` at the end of the line).

**Recommendation:** Keep them. None fires while you type ordinary words or commands (tested), and the Tab ones only do something when you press Tab right after the word at the start of a line.

- .tex: in `$‸$` type `pi` → before `$pi‸$`, now `$\pi‸$`
- .tex: in `$‸a+b$` type `/` with 3 characters selected → before `$/‸$`, now `$\frac{a+b}{‸}$`
- .tex: in `‸` type `table<Tab>` → before `table ‸`, now `\begin{table}[htpb]⏎	\centering⏎	\caption{caption}⏎	\label{tab:label}⏎	\begin{tabular}{c}⏎	⏎	\end{tabular}⏎\end{table}` (selected: `htpb`)

### Castel's environment snippets for text: add them as Tab snippets?

Trigger: `beg, enum, item, ali (in text)`

**Decided 2026-10-01:** Yes, as Tab-only at the start of a line in .tex: beg, enum, item, ali (text.lua).

**Now:** Not added. Castel's `beg`, `enum`, `item`, `ali` fire by themselves at the start of a line in text, so a line starting with “began”, “items”, “enumerate” or “align” would turn into an environment. Your `beg` works in maths only, as before.

**Alternative:** Add them as Tab snippets (start of a line, .tex only), like `table` and `fig`: `item` Tab → `\begin{itemize} \item … \end{itemize}`.

**Recommendation:** Leave them out for now; say so if you write a lot of lists or align environments in .tex and they go in as Tab snippets.

- .tex: in `‸` type `Let us admit the landmark result: the subset of all sets is a set, and the inner sum is finite.` → before `Let us admit the landmark result: the subset of all sets is a set, and the inner sum is finite.‸`, now `Let us admit the landmark result: the subset of all sets is a set, and the inner sum is finite.‸`


## Behaviour changes to your snippets

Same trigger, same output, but it fires in fewer places. Almost all are the mangling fixes: the trigger used to fire inside a command you were typing.

### No maths snippets inside \ce{…} and \pu{…}

Trigger: `all maths snippets, inside \ce{} and \pu{}`

mhchem has its own syntax there: `H2O`, `A -> B`, `m2`. The auto subscript used to turn `\ce{H2O}` into `\ce{H_{2}O}` and `\pu{3 m2}` into `\pu{3 m_{2}}`. Labels and references (`\label{}`, `\ref{}`, `\eqref{}`, `\cite{}`) are protected the same way.

- .tex: in `$\ce{ ‸ }$` type `H2O` → before `$\ce{ H_{2}O‸ }$`, now `$\ce{ H2O‸ }$`
- .tex: in `$\pu{ ‸ }$` type `3 m2` → before `$\pu{ 3 m_{2}‸ }$`, now `$\pu{ 3 m2‸ }$`
- .tex: in `$\ce{ ‸ }$` type `A -> B` → before `$\ce{ A -> B‸ }$`, now `$\ce{ A -> B‸ }$`
- .tex: in `\begin{equation}⏎x \label{eq:‸}⏎\end{equation}` type `sub` → before `\begin{equation}⏎x \label{eq:sub‸}⏎\end{equation}`, now `\begin{equation}⏎x \label{eq:sub‸}⏎\end{equation}`

### { right after an automatic _{ is skipped

Trigger: `{ after _{`

`_` already opens `_{…}`; typing `x_{i}` as written used to give `x_{{i}}`. Now the second `{` is skipped, so both `x_i` and `x_{i}` give `x_{i}`.

- .tex: in `$‸$` type `x_{i}` → before `$x_{{i}‸}$`, now `$x_{i}‸$`
- .tex: in `$‸$` type `_` → before `$_{‸}$`, now `$_{‸}$`
- .md: in `$‸$` type `_` → before `$_{‸}$`, now `$_{‸}$`

### mk only at the start of a word, and not in comments or `code`

Trigger: `mk`

`Hellomk` used to give `Hello$…$`; inside `` `code` `` in .md and in a `%` comment in .tex it fired too. Now it needs a space (or line start) before it, like the maths triggers.

- .md: in `Hello‸` type `mk` → before `Hello$‸$`, now `Hellomk‸`
- .md: in `ˋ‸ˋ` type `mk` → before `ˋ$‸$ˋ`, now `ˋmk‸ˋ`
- .tex: in `% ‸` type `mk` → before `% $‸$`, now `% mk‸`
- .tex: in `‸` type `mk` → before `$‸$`, now `$‸$`
- .md: in `‸` type `mk` → before `$‸$`, now `$‸$`

### dm in a list item: no longer fires inside words; indented items line up

Trigger: `dm (in a list item)`

In a list item, typing a word containing “dm” (admit, landmark) used to start display maths in the middle of the word. Now `dm` has to start a word. In an indented item (`  - …`) the block now lines up with the item's text (LuaSnip keeps the line's indentation).

- .md: in `- we ‸` type `admit` → before `- we a⏎  $$⏎  	it‸⏎  $$`, now `- we admit‸`
- .md: in `1. a ‸` type `landmark` → before `1. a lan⏎   $$⏎   	ark‸⏎   $$`, now `1. a landmark‸`
- .md: in `  - item ‸` type `dm` → before `  - item ⏎  $$⏎  	‸⏎  $$`, now `  - item ⏎    $$⏎    	‸⏎    $$`
- .tex: in `- item ‸` type `dm` → before `- item ⏎  \[⏎  	‸⏎  \]`, now `- item ⏎  \[⏎  	‸⏎  \]`
- .md: in `- item ‸` type `dm` → before `- item ⏎  $$⏎  	‸⏎  $$`, now `- item ⏎  $$⏎  	‸⏎  $$`

### Snippets live in snippets/tex/*.lua, not in ~/.config/latex-suite

Trigger: `(all)`

One Lua file per topic (sets, greek, operations, accents, subscripts, symbols, calculus, trig, environments, brackets, physics, chemistry, text, visual), one snippet per line, loaded by LuaSnip's from_lua loader and reloaded on save. ntex no longer reads ~/.config/latex-suite at all (Superset still does; it is untouched). Space e picks a file; :SnippetReport (Space r) lists what loaded and any error. A file with an error keeps its previous snippets; at startup the other files still load (before, one typo in snippets.js meant no snippets at all).

- .tex: in `$‸$` type `uu` → before `$\cup‸$`, now `$\cup‸$`
- .md: in `$‸$` type `uu` → before `$\cup‸$`, now `$\cup‸$`
- .tex: in `$‸$` type `//` → before `$\frac{‸}{}$`, now `$\frac{‸}{}$`
- .md: in `$‸$` type `//` → before `$\frac{‸}{}$`, now `$\frac{‸}{}$`

### Latex Suite settings moved into prefs.lua (`latex = { … }`), same values

Trigger: `(features)`

Auto-fraction (and its symbol, breaking characters, excluded places), matrix environments, tabout closing symbols, enlarge triggers, auto-delete $, removing trailing spaces in inline maths: all in prefs.lua now, with the values your Obsidian import had. obsidian-settings.json and settings.json are no longer read. `wordDelimiters` is gone: the word-start rule replaces it. `conceal` in prefs.lua is now simply on/off (there is no shared setting to follow).

- .tex: in `$‸$` type `(a+b)/c` → before `$\frac{a+b}{c‸}$`, now `$\frac{a+b}{c‸}$`
- .tex: in `$\sqrt{a‸}$` type `<Tab><Tab>` → before `$\sqrt{a}$‸`, now `$\sqrt{a}$‸`
- .tex: in `\[⏎\begin{pmatrix}⏎a & b‸⏎\end{pmatrix}⏎\]` type `<CR>c` → before `\[⏎\begin{pmatrix}⏎a & b \\⏎c‸⏎\end{pmatrix}⏎\]`, now `\[⏎\begin{pmatrix}⏎a & b \\⏎c‸⏎\end{pmatrix}⏎\]`
- .tex: in `$‸$` type `(sum` → before `$\left( \sum‸ \right)$`, now `$\left( \sum‸ \right)$`
- .tex: in `x $‸$ y` type `<BS>` → before `x ‸ y`, now `x ‸ y`
- .tex: in `\[⏎‸⏎\]` type `o+` → before `\[⏎\oplus ‸⏎\]`, now `\[⏎\oplus ‸⏎\]`

### Function snippets are Lua; regex triggers are Lua patterns

Trigger: `(engine)`

No JavaScript, no converter, no JSON cache, no jsregexp: `iden3`, the auto subscripts, `\sin` + letter and `dm` in a list are short Lua functions or Lua patterns in the snippet files, visible and editable. Results are identical (see “Kept as is”).

- .tex: in `\[⏎‸⏎\]` type `iden3` → before `\[⏎\begin{pmatrix}⏎1 & 0 & 0 \\⏎0 & 1 & 0 \\⏎0 & 0 & 1⏎\end{pmatrix}‸⏎\]`, now `\[⏎\begin{pmatrix}⏎1 & 0 & 0 \\⏎0 & 1 & 0 \\⏎0 & 0 & 1⏎\end{pmatrix}‸⏎\]`
- .md: in `$$⏎‸⏎$$` type `iden3` → before `$$⏎\begin{pmatrix}⏎1 & 0 & 0 \\⏎0 & 1 & 0 \\⏎0 & 0 & 1⏎\end{pmatrix}‸⏎$$`, now `$$⏎\begin{pmatrix}⏎1 & 0 & 0 \\⏎0 & 1 & 0 \\⏎0 & 0 & 1⏎\end{pmatrix}‸⏎$$`
- .tex: in `$‸$` type `x2` → before `$x_{2}‸$`, now `$x_{2}‸$`
- .md: in `$‸$` type `x2` → before `$x_{2}‸$`, now `$x_{2}‸$`
- .tex: in `$‸$` type `sinx` → before `$\sin x‸$`, now `$\sin x‸$`
- .md: in `$‸$` type `sinx` → before `$\sin x‸$`, now `$\sin x‸$`

### Commands renamed; the old names still work

Trigger: `:LatexSuiteReport …`

:SnippetReport (Space r), :SnippetEdit (Space e, now a picker of the snippet files), :SnippetReload, :Conceal (F2). :LatexSuiteReport, :LatexSuiteEdit, :LatexSuiteReload and :LatexSuiteConceal do the same; delete them from lua/snips/init.lua when you no longer need them.

### phi no longer fires inside \phi, \varphi

Trigger: `phi` · now in greek.lua:12 · yours

Typing `\phi`, `\varphi` as written used to set it off part-way (at “\phi”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. phi itself works as before. `\phi` typed as written now stays `\phi` (it used to become `\\varphi`).

- .tex: in `$‸$` type `\phi` → before `$\\varphi‸$`, now `$\phi‸$`
- .tex: in `$‸$` type `\varphi` → before `$\var\varphi‸$`, now `$\varphi‸$`
- .tex: in `$‸$` type `phi` → before `$\varphi‸$`, now `$\varphi‸$`
- .md: in `$‸$` type `phi` → before `$\varphi‸$`, now `$\varphi‸$`

### imp no longer fires inside \implies, \impliedby

Trigger: `imp` · now in sets.lua:25 · yours

Typing `\implies`, `\impliedby` as written used to set it off part-way (at “\imp”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. imp itself works as before.

- .tex: in `$‸$` type `\implies` → before `$\\implieslies‸$`, now `$\implies‸$`
- .tex: in `$‸$` type `\impliedby` → before `$\\impliesliedby‸$`, now `$\impliedby‸$`
- .tex: in `$‸$` type `imp` → before `$\implies‸$`, now `$\implies‸$`
- .md: in `$‸$` type `imp` → before `$\implies‸$`, now `$\implies‸$`

### iff no longer fires inside \iff

Trigger: `iff` · now in sets.lua:28 · yours

Typing `\iff` as written used to set it off part-way (at “\iff”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. iff itself works as before.

- .tex: in `$‸$` type `\iff` → before `$\\iff‸$`, now `$\iff‸$`
- .tex: in `$‸$` type `iff` → before `$\iff‸$`, now `$\iff‸$`
- .md: in `$‸$` type `iff` → before `$\iff‸$`, now `$\iff‸$`

### sub no longer fires inside \subset, \subseteq, \subsetneq

Trigger: `sub` · now in sets.lua:32 · yours

Typing `\subset`, `\subseteq`, `\subsetneq` as written used to set it off part-way (at “\sub”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. sub itself works as before. `\sub…` commands are the most common victims; `psub` still gives `\subset`.

- .tex: in `$‸$` type `\subset` → before `$\\subseteq\{ ‸ \}$`, now `$\subset‸$`
- .tex: in `$‸$` type `\subseteq` → before `$\\subseteq\{ eq‸ \}$`, now `$\subseteq‸$`
- .tex: in `$‸$` type `\subsetneq` → before `$\\subseteq\{ neq‸ \}$`, now `$\subsetneq‸$`
- .tex: in `$‸$` type `sub` → before `$\subseteq‸$`, now `$\subseteq‸$`
- .md: in `$‸$` type `sub` → before `$\subseteq‸$`, now `$\subseteq‸$`

### neg no longer fires inside \neg

Trigger: `neg` · now in sets.lua:33 · yours

Typing `\neg` as written used to set it off part-way (at “\neg”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. neg itself works as before.

- .tex: in `$‸$` type `\neg` → before `$\\neg‸$`, now `$\neg‸$`
- .tex: in `$‸$` type `neg` → before `$\neg‸$`, now `$\neg‸$`
- .md: in `$‸$` type `neg` → before `$\neg‸$`, now `$\neg‸$`

### and no longer fires inside \land

Trigger: `and` · now in sets.lua:35 · changed

Typing `\land` as written used to set it off part-way (at “\land”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. and itself works as before.

- .tex: in `$‸$` type `\land` → before `$\l\land{}‸$`, now `$\land‸$`
- .tex: in `$‸$` type `and` → before `$\land{}‸$`, now `$\land{}‸$`
- .md: in `$‸$` type `and` → before `$\land{}‸$`, now `$\land{}‸$`

### set no longer fires inside \supset, \supseteq, \setminus, …

Trigger: `set` · now in sets.lua:41 · changed

Typing `\supset`, `\supseteq`, `\setminus`, `\emptyset`, `\overset{!}{=}` as written used to set it off part-way (at “\supset”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. set itself works as before. Also `\setminus`, `\overset`: any command containing “set”.

- .tex: in `$‸$` type `\supset` → before `$\sup\{ ‸ \}$`, now `$\supset‸$`
- .tex: in `$‸$` type `\supseteq` → before `$\sup\{ eq‸ \}$`, now `$\supseteq‸$`
- .tex: in `$‸$` type `\setminus` → before `$\\{ minus‸ \}$`, now `$\setminus‸$`
- .tex: in `$‸$` type `\emptyset` → before `$\empty\{ ‸ \}$`, now `$\emptyset‸$`
- .tex: in `$‸$` type `\overset{!}{=}` → before `$\over\{ {!}{=}‸ \}$`, now `$\overset{!}{=}‸$`
- .tex: in `$‸$` type `set` → before `$\{ ‸ \}$`, now `$\{ ‸ \}$`
- .md: in `$‸$` type `set` → before `$\{ ‸ \}$`, now `$\{ ‸ \}$`

### _ still adds the braces, but x_{i} typed as written no longer gives x_{{i}}

Trigger: `_` · now in operations.lua:16 · default

Typing `\aleph_0`, `x_{i}` as written used to set it off part-way (at “\aleph_”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. _ itself works as before. `\aleph_0` → `\aleph_{0}` still happens on purpose (same output); `x_{i}` typed as written is now fine (see the `{` entry).

- .tex: in `$‸$` type `\aleph_0` → before `$\aleph_{0‸}$`, now `$\aleph_{0‸}$`
- .tex: in `$‸$` type `x_{i}` → before `$x_{{i}‸}$`, now `$x_{i}‸$`
- .tex: in `$‸$` type `_` → before `$_{‸}$`, now `$_{‸}$`
- .md: in `$‸$` type `_` → before `$_{‸}$`, now `$_{‸}$`

### sts no longer fires inside \exists, \nexists

Trigger: `sts` · now in operations.lua:17 · default

Typing `\exists`, `\nexists` as written used to set it off part-way (at “\exists”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. sts itself works as before.

- .tex: in `$‸$` type `\exists` → before `$\exi_\text{‸}$`, now `$\exists‸$`
- .tex: in `$‸$` type `\nexists` → before `$\nexi_\text{‸}$`, now `$\nexists‸$`
- .tex: in `$‸$` type `sts` → before `$_\text{‸}$`, now `$_\text{‸}$`
- .md: in `$‸$` type `sts` → before `$_\text{‸}$`, now `$_\text{‸}$`

### sq no longer fires inside \blacksquare, \square, \sqrt, …

Trigger: `sq` · now in operations.lua:18 · default

Typing `\blacksquare`, `\square`, `\sqrt{x}`, `\sqrt[n]{x}` as written used to set it off part-way (at “\blacksq”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. sq itself works as before.

- .tex: in `$‸$` type `\blacksquare` → before `$\black\sqrt{ uare‸ }$`, now `$\blacksquare‸$`
- .tex: in `$‸$` type `\square` → before `$\\sqrt{ uare‸ }$`, now `$\square‸$`
- .tex: in `$‸$` type `\sqrt{x}` → before `$\\sqrt{ rt{x}‸ }$`, now `$\sqrt{x}‸$`
- .tex: in `$‸$` type `\sqrt[n]{x}` → before `$\\sqrt{ rt[n]{x}‸ }$`, now `$\sqrt[n]{x}‸$`
- .tex: in `$‸$` type `sq` → before `$\sqrt{ ‸ }$`, now `$\sqrt{ ‸ }$`
- .md: in `$‸$` type `sq` → before `$\sqrt{ ‸ }$`, now `$\sqrt{ ‸ }$`

### ee no longer fires inside \vee

Trigger: `ee` · now in operations.lua:20 · yours

Typing `\vee` as written used to set it off part-way (at “\vee”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. ee itself works as before.

- .tex: in `$‸$` type `\vee` → before `$\ve^{ ‸ }$`, now `$\vee‸$`
- .tex: in `$‸$` type `ee` → before `$e^{ ‸ }$`, now `$e^{ ‸ }$`
- .md: in `$‸$` type `ee` → before `$e^{ ‸ }$`, now `$e^{ ‸ }$`

### Re no longer fires inside \Re

Trigger: `Re` · now in operations.lua:26 · default

Typing `\Re` as written used to set it off part-way (at “\Re”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. Re itself works as before.

- .tex: in `$‸$` type `\Re` → before `$\\mathrm{Re}‸$`, now `$\Re‸$`
- .tex: in `$‸$` type `Re` → before `$\mathrm{Re}‸$`, now `$\mathrm{Re}‸$`
- .md: in `$‸$` type `Re` → before `$\mathrm{Re}‸$`, now `$\mathrm{Re}‸$`

### Im no longer fires inside \Im

Trigger: `Im` · now in operations.lua:27 · default

Typing `\Im` as written used to set it off part-way (at “\Im”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. Im itself works as before.

- .tex: in `$‸$` type `\Im` → before `$\\mathrm{Im}‸$`, now `$\Im‸$`
- .tex: in `$‸$` type `Im` → before `$\mathrm{Im}‸$`, now `$\mathrm{Im}‸$`
- .md: in `$‸$` type `Im` → before `$\mathrm{Im}‸$`, now `$\mathrm{Im}‸$`

### bf no longer fires inside \mathbf

Trigger: `bf` · now in operations.lua:28 · default

Typing `\mathbf{v}` as written used to set it off part-way (at “\mathbf”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. bf itself works as before.

- .tex: in `$‸$` type `\mathbf{v}` → before `$\math\mathbf{{v}‸}$`, now `$\mathbf{v}‸$`
- .tex: in `$‸$` type `bf` → before `$\mathbf{‸}$`, now `$\mathbf{‸}$`
- .md: in `$‸$` type `bf` → before `$\mathbf{‸}$`, now `$\mathbf{‸}$`

### rm no longer fires inside \mathrm

Trigger: `rm` · now in operations.lua:29 · default

Typing `\mathrm{d}` as written used to set it off part-way (at “\mathrm”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. rm itself works as before.

- .tex: in `$‸$` type `\mathrm{d}` → before `$\math\mathrm{{d}‸}$`, now `$\mathrm{d}‸$`
- .tex: in `$‸$` type `rm` → before `$\mathrm{‸}$`, now `$\mathrm{‸}$`
- .md: in `$‸$` type `rm` → before `$\mathrm{‸}$`, now `$\mathrm{‸}$`

### det no longer fires inside \widetilde

Trigger: `det` · now in operations.lua:33 · default

Typing `\widetilde{xy}` as written used to set it off part-way (at “\widet”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. det itself works as before.

- .tex: in `$‸$` type `\widetilde{xy}` → before `$\wi\d\tilde{e}{xy}‸$`, now `$\widetilde{xy}‸$`
- .tex: in `$‸$` type `det` → before `$\det‸$`, now `$\det‸$`
- .md: in `$‸$` type `det` → before `$\det‸$`, now `$\det‸$`

### ‹letter›hat (postfix) no longer fires inside \widehat

Trigger: `‹letter›hat (postfix)` · now in accents.lua:26 · default

Typing `\widehat{xy}` as written used to set it off part-way (at “\widehat”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. ‹letter›hat (postfix) itself works as before.

- .tex: in `$‸$` type `\widehat{xy}` → before `$\wid\hat{e}{xy}‸$`, now `$\widehat{xy}‸$`
- .tex: in `$‸$` type `xhat` → before `$\hat{x}‸$`, now `$\hat{x}‸$`
- .md: in `$‸$` type `xhat` → before `$\hat{x}‸$`, now `$\hat{x}‸$`

### ‹letter›bar (postfix) no longer fires inside \hbar

Trigger: `‹letter›bar (postfix)` · now in accents.lua:27 · default

Typing `\hbar` as written used to set it off part-way (at “\hbar”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. ‹letter›bar (postfix) itself works as before.

- .tex: in `$‸$` type `\hbar` → before `$\\bar{h}‸$`, now `$\hbar‸$`
- .tex: in `$‸$` type `xbar` → before `$\bar{x}‸$`, now `$\bar{x}‸$`
- .md: in `$‸$` type `xbar` → before `$\bar{x}‸$`, now `$\bar{x}‸$`

### ‹letter›dot (postfix) no longer fires inside \odot, \ldots, \vdots

Trigger: `‹letter›dot (postfix)` · now in accents.lua:28 · default

Typing `\odot`, `\ldots`, `\vdots` as written used to set it off part-way (at “\odot”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. ‹letter›dot (postfix) itself works as before. The postfix `dot` (xdot → \dot{x}) no longer grabs the end of a command (`\odot`, `\ldots`, `\vdots`), and never turns `cdot` into `\dot{c}`.

- .tex: in `$‸$` type `\odot` → before `$\\dot{o}‸$`, now `$\odot‸$`
- .tex: in `$‸$` type `\ldots` → before `$\\dot{l}s‸$`, now `$\ldots‸$`
- .tex: in `$‸$` type `\vdots` → before `$\\dot{v}s‸$`, now `$\vdots‸$`
- .tex: in `$‸$` type `xdot` → before `$\dot{x}‸$`, now `$\dot{x}‸$`
- .md: in `$‸$` type `xdot` → before `$\dot{x}‸$`, now `$\dot{x}‸$`

### hat no longer fires inside \hat

Trigger: `hat` · now in accents.lua:26 · default

Typing `\hat{x}` as written used to set it off part-way (at “\hat”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. hat itself works as before.

- .tex: in `$‸$` type `\hat{x}` → before `$\\hat{{x}‸}$`, now `$\hat{x}‸$`
- .tex: in `$‸$` type `hat` → before `$\hat{‸}$`, now `$\hat{‸}$`
- .md: in `$‸$` type `hat` → before `$\hat{‸}$`, now `$\hat{‸}$`

### bar no longer fires inside \bar

Trigger: `bar` · now in accents.lua:27 · default

Typing `\bar{x}` as written used to set it off part-way (at “\bar”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. bar itself works as before.

- .tex: in `$‸$` type `\bar{x}` → before `$\\bar{{x}‸}$`, now `$\bar{x}‸$`
- .tex: in `$‸$` type `bar` → before `$\bar{‸}$`, now `$\bar{‸}$`
- .md: in `$‸$` type `bar` → before `$\bar{‸}$`, now `$\bar{‸}$`

### dot no longer fires inside \dot, \dots

Trigger: `dot` · now in accents.lua:28 · default

Typing `\dot{x}`, `\dots` as written used to set it off part-way (at “\dot”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. dot itself works as before.

- .tex: in `$‸$` type `\dot{x}` → before `$\\dot{{x}‸}$`, now `$\dot{x}‸$`
- .tex: in `$‸$` type `\dots` → before `$\\dot{s‸}$`, now `$\dots‸$`
- .tex: in `$‸$` type `dot` → before `$\dot{‸}$`, now `$\dot{‸}$`
- .md: in `$‸$` type `dot` → before `$\dot{‸}$`, now `$\dot{‸}$`

### ddot no longer fires inside \ddot, \ddots

Trigger: `ddot` · now in accents.lua:29 · default

Typing `\ddot{x}`, `\ddots` as written used to set it off part-way (at “\ddot”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. ddot itself works as before.

- .tex: in `$‸$` type `\ddot{x}` → before `$\\ddot{{x}‸}$`, now `$\ddot{x}‸$`
- .tex: in `$‸$` type `\ddots` → before `$\\ddot{s‸}$`, now `$\ddots‸$`
- .tex: in `$‸$` type `ddot` → before `$\ddot{‸}$`, now `$\ddot{‸}$`
- .md: in `$‸$` type `ddot` → before `$\ddot{‸}$`, now `$\ddot{‸}$`

### cdot no longer fires inside \cdot, \cdots

Trigger: `cdot` · now in accents.lua:42 · default

Typing `\cdot`, `\cdots` as written used to set it off part-way (at “\cdot”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. cdot itself works as before.

- .tex: in `$‸$` type `\cdot` → before `$\\cdot‸$`, now `$\cdot‸$`
- .tex: in `$‸$` type `\cdots` → before `$\\cdots‸$`, now `$\cdots‸$`
- .tex: in `$‸$` type `cdot` → before `$\cdot‸$`, now `$\cdot‸$`
- .md: in `$‸$` type `cdot` → before `$\cdot‸$`, now `$\cdot‸$`

### tilde no longer fires inside \tilde

Trigger: `tilde` · now in accents.lua:30 · default

Typing `\tilde{x}` as written used to set it off part-way (at “\tilde”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. tilde itself works as before.

- .tex: in `$‸$` type `\tilde{x}` → before `$\\tilde{{x}‸}$`, now `$\tilde{x}‸$`
- .tex: in `$‸$` type `tilde` → before `$\tilde{‸}$`, now `$\tilde{‸}$`
- .md: in `$‸$` type `tilde` → before `$\tilde{‸}$`, now `$\tilde{‸}$`

### und no longer fires inside \underline, \underbrace, \underset

Trigger: `und` · now in accents.lua:31 · default

Typing `\underline{x}`, `\underbrace{a+b}_{n}`, `\underset{x}{\arg\max}` as written used to set it off part-way (at “\und”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. und itself works as before.

- .tex: in `$‸$` type `\underline{x}` → before `$\\underline{erline{x}‸}$`, now `$\underline{x}‸$`
- .tex: in `$‸$` type `\underbrace{a+b}_{n}` → before `$\\underline{er\bra{ce{a+b}_{{n}‸}}}$`, now `$\underbrace{a+b}_{n}‸$`
- .tex: in `$‸$` type `\underset{x}{\arg\max}` → before `$\\underline{er\{ {x}{\arg\max}‸ \}}$`, now `$\underset{x}{\arg\max}‸$`
- .tex: in `$‸$` type `und` → before `$\underline{‸}$`, now `$\underline{‸}$`
- .md: in `$‸$` type `und` → before `$\underline{‸}$`, now `$\underline{‸}$`

### vec no longer fires inside \vec

Trigger: `vec` · now in accents.lua:32 · default

Typing `\vec{v}` as written used to set it off part-way (at “\vec”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. vec itself works as before.

- .tex: in `$‸$` type `\vec{v}` → before `$\\vec{{v}‸}$`, now `$\vec{v}‸$`
- .tex: in `$‸$` type `vec` → before `$\vec{‸}$`, now `$\vec{‸}$`
- .md: in `$‸$` type `vec` → before `$\vec{‸}$`, now `$\vec{‸}$`

### pmod no longer fires inside \pmod

Trigger: `pmod` · now in accents.lua:46 · default

Typing `\pmod{n}` as written used to set it off part-way (at “\pmod”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. pmod itself works as before.

- .tex: in `$‸$` type `\pmod{n}` → before `$\\pmod{n}‸$`, now `$\pmod{n}‸$`
- .tex: in `$‸$` type `pmod` → before `$\pmod{n}$` (selected: `n`), now `$\pmod{n}$` (selected: `n`)
- .md: in `$‸$` type `pmod` → before `$\pmod{n}$` (selected: `n`), now `$\pmod{n}$` (selected: `n`)

### sum no longer fires inside \sum

Trigger: `sum` · now in symbols.lua:13 · default

Typing `\sum` as written used to set it off part-way (at “\sum”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. sum itself works as before.

- .tex: in `$‸$` type `\sum` → before `$\\sum‸$`, now `$\sum‸$`
- .tex: in `$‸$` type `sum` → before `$\sum‸$`, now `$\sum‸$`
- .md: in `$‸$` type `sum` → before `$\sum‸$`, now `$\sum‸$`

### prod no longer fires inside \prod, \coprod

Trigger: `prod` · now in symbols.lua:14 · default

Typing `\prod`, `\coprod` as written used to set it off part-way (at “\prod”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. prod itself works as before.

- .tex: in `$‸$` type `\prod` → before `$\\prod‸$`, now `$\prod‸$`
- .tex: in `$‸$` type `\coprod` → before `$\co\prod‸$`, now `$\coprod‸$`
- .tex: in `$‸$` type `prod` → before `$\prod‸$`, now `$\prod‸$`
- .md: in `$‸$` type `prod` → before `$\prod‸$`, now `$\prod‸$`

### lim no longer fires inside \lim, \limsup, \liminf

Trigger: `lim` · now in symbols.lua:17 · default

Typing `\lim`, `\limsup`, `\liminf` as written used to set it off part-way (at “\lim”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. lim itself works as before.

- .tex: in `$‸$` type `\lim` → before `$\\lim_{ n \to \infty }$` (selected: `n`), now `$\lim‸$`
- .tex: in `$‸$` type `\limsup` → before `$\\lim_{ sup‸ \to \infty }$`, now `$\limsup‸$`
- .tex: in `$‸$` type `\liminf` → before `$\\lim_{ inf‸ \to \infty }$`, now `$\liminf‸$`
- .tex: in `$‸$` type `lim` → before `$\lim_{ n \to \infty }$` (selected: `n`), now `$\lim_{ n \to \infty }$` (selected: `n`)
- .md: in `$‸$` type `lim` → before `$\lim_{ n \to \infty }$` (selected: `n`), now `$\lim_{ n \to \infty }$` (selected: `n`)

### nabl no longer fires inside \nabla

Trigger: `nabl` · now in symbols.lua:21 · default

Typing `\nabla` as written used to set it off part-way (at “\nabl”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. nabl itself works as before.

- .tex: in `$‸$` type `\nabla` → before `$\\nablaa‸$`, now `$\nabla‸$`
- .tex: in `$‸$` type `nabl` → before `$\nabla‸$`, now `$\nabla‸$`
- .md: in `$‸$` type `nabl` → before `$\nabla‸$`, now `$\nabla‸$`

### para no longer fires inside \parallel

Trigger: `para` · now in symbols.lua:24 · default

Typing `\parallel` as written used to set it off part-way (at “\para”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. para itself works as before.

- .tex: in `$‸$` type `\parallel` → before `$\\parallelllel‸$`, now `$\parallel‸$`
- .tex: in `$‸$` type `para` → before `$\parallel‸$`, now `$\parallel‸$`
- .md: in `$‸$` type `para` → before `$\parallel‸$`, now `$\parallel‸$`

### prop no longer fires inside \propto

Trigger: `prop` · now in symbols.lua:34 · default

Typing `\propto` as written used to set it off part-way (at “\prop”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. prop itself works as before.

- .tex: in `$‸$` type `\propto` → before `$\\proptoto‸$`, now `$\propto‸$`
- .tex: in `$‸$` type `prop` → before `$\propto‸$`, now `$\propto‸$`
- .md: in `$‸$` type `prop` → before `$\propto‸$`, now `$\propto‸$`

### oint no longer fires inside \oint

Trigger: `oint` · now in calculus.lua:19 · default

Typing `\oint` as written used to set it off part-way (at “\oint”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. oint itself works as before.

- .tex: in `$‸$` type `\oint` → before `$\\oint‸$`, now `$\oint‸$`
- .tex: in `$‸$` type `oint` → before `$\oint‸$`, now `$\oint‸$`
- .md: in `$‸$` type `oint` → before `$\oint‸$`, now `$\oint‸$`

### iint no longer fires inside \iint

Trigger: `iint` · now in calculus.lua:20 · default

Typing `\iint` as written used to set it off part-way (at “\iint”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. iint itself works as before.

- .tex: in `$‸$` type `\iint` → before `$\\iint‸$`, now `$\iint‸$`
- .tex: in `$‸$` type `iint` → before `$\iint‸$`, now `$\iint‸$`
- .md: in `$‸$` type `iint` → before `$\iint‸$`, now `$\iint‸$`

### iiint no longer fires inside \iiint

Trigger: `iiint` · now in calculus.lua:21 · default

Typing `\iiint` as written used to set it off part-way (at “\iiint”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. iiint itself works as before.

- .tex: in `$‸$` type `\iiint` → before `$\\iiint‸$`, now `$\iiint‸$`
- .tex: in `$‸$` type `iiint` → before `$\iiint‸$`, now `$\iiint‸$`
- .md: in `$‸$` type `iiint` → before `$\iiint‸$`, now `$\iiint‸$`

### sin, cos, tan, arcsin, … no longer fires inside \arcsin

Trigger: `sin, cos, tan, arcsin, …` · now in trig.lua:18 · default

Typing `\arcsin` as written used to set it off part-way (at “\arcsin”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. sin, cos, tan, arcsin, … itself works as before.

- .tex: in `$‸$` type `\arcsin` → before `$\arc\sin‸$`, now `$\arcsin‸$`
- .tex: in `$‸$` type `sin` → before `$\sin‸$`, now `$\sin‸$`
- .md: in `$‸$` type `sin` → before `$\sin‸$`, now `$\sin‸$`

### dag no longer fires inside \dagger

Trigger: `dag` · now in physics.lua:15 · default

Typing `\dagger` as written used to set it off part-way (at “\dag”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. dag itself works as before.

- .tex: in `$‸$` type `\dagger` → before `$\^{\dagger}ger‸$`, now `$\dagger‸$`
- .tex: in `$‸$` type `dag` → before `$^{\dagger}‸$`, now `$^{\dagger}‸$`
- .md: in `$‸$` type `dag` → before `$^{\dagger}‸$`, now `$^{\dagger}‸$`

### ox no longer fires inside \approx

Trigger: `ox` · now in physics.lua:17 · changed

Typing `\approx` as written used to set it off part-way (at “\approx”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. ox itself works as before.

- .tex: in `$‸$` type `\approx` → before `$\appr\otimes‸$`, now `$\approx‸$`
- .tex: in `$‸$` type `\tox` → before `$\t\otimes‸$`, now `$\tox‸$`
- .tex: in `$‸$` type `ox` → before `$\otimes‸$`, now `$\otimes‸$`
- .md: in `$‸$` type `ox` → before `$\otimes‸$`, now `$\otimes‸$`

### bra no longer fires inside \overbrace

Trigger: `bra` · now in physics.lua:18 · default

Typing `\overbrace{a+b}^{n}` as written used to set it off part-way (at “\overbra”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. bra itself works as before.

- .tex: in `$‸$` type `\overbrace{a+b}^{n}` → before `$\over\bra{ce{a+b}^{n}‸}$`, now `$\overbrace{a+b}^{n}‸$`
- .tex: in `$‸$` type `bra` → before `$\bra{‸}$`, now `$\bra{‸}$`
- .md: in `$‸$` type `bra` → before `$\bra{‸}$`, now `$\bra{‸}$`

### ceil no longer fires inside \lceil

Trigger: `ceil` · now in brackets.lua:15 · default

Typing `\lceil \rceil` as written used to set it off part-way (at “\lceil”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. ceil itself works as before.

- .tex: in `$‸$` type `\lceil \rceil` → before `$\l\lceil  \r\lceil ‸ \rceil \rceil$`, now `$\lceil \rceil‸$`
- .tex: in `$‸$` type `ceil` → before `$\lceil ‸ \rceil$`, now `$\lceil ‸ \rceil$`
- .md: in `$‸$` type `ceil` → before `$\lceil ‸ \rceil$`, now `$\lceil ‸ \rceil$`

### floor no longer fires inside \lfloor

Trigger: `floor` · now in brackets.lua:16 · default

Typing `\lfloor \rfloor` as written used to set it off part-way (at “\lfloor”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. floor itself works as before.

- .tex: in `$‸$` type `\lfloor \rfloor` → before `$\l\lfloor  \r\lfloor ‸ \rfloor \rfloor$`, now `$\lfloor \rfloor‸$`
- .tex: in `$‸$` type `floor` → before `$\lfloor ‸ \rfloor$`, now `$\lfloor ‸ \rfloor$`
- .md: in `$‸$` type `floor` → before `$\lfloor ‸ \rfloor$`, now `$\lfloor ‸ \rfloor$`

### mod no longer fires inside \models, \bmod

Trigger: `mod` · now in brackets.lua:17 · changed

Typing `\models`, `\bmod` as written used to set it off part-way (at “\mod”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. mod itself works as before.

- .tex: in `$‸$` type `\models` → before `$\/els‸$`, now `$\models‸$`
- .tex: in `$‸$` type `\bmod` → before `$\b/‸$`, now `$\bmod‸$`
- .tex: in `$‸$` type `mod` → before `$/‸$`, now `$/‸$`
- .md: in `$‸$` type `mod` → before `$/‸$`, now `$/‸$`


## Kept as is

Typed into the old and the new setup: identical result (text, cursor, tabstops), in .tex and in .md.

| Trigger | Output | Where | Origin | Now in | Note |
| --- | --- | --- | --- | --- | --- |
| `CC` | `\mathcal{C}` | math | changed | sets.lua:13 |  |
| `Nat` | `\mathbb{N}` | math | yours | sets.lua:14 |  |
| `FF` | `\mathbb{F}` | math | yours | sets.lua:15 |  |
| `disj` | `\mathcal{D}` | math | yours | sets.lua:16 |  |
| `pow` | `\mathcal{P}` | math | yours | sets.lua:17 |  |
| `uu` | `\cup` | math | yours | sets.lua:18 |  |
| `UU` | `\bigcup_{$1}$2` | math | yours | sets.lua:19 |  |
| `NN` | `\bigcap_{$1}$2` | math | changed | sets.lua:20 |  |
| `exx` | `\exists` | math | yours | sets.lua:21 |  |
| `fall` | `\forall` | math | yours | sets.lua:22 |  |
| `psub` | `\subset` | math | yours | sets.lua:23 |  |
| `diff` | `\setminus` | math | yours | sets.lua:24 |  |
| `qed` | `\ \blacksquare` | math | yours | sets.lua:26 |  |
| `nn` | `\cap` | math | yours | sets.lua:27 |  |
| `ninn` | `\notin` | math | yours | sets.lua:29 |  |
| `st.` | `\text{ s.t. }` | math | yours | sets.lua:30 |  |
| `odp` | `\langle $1 , $2 \rangle $3` | math | yours | sets.lua:31 |  |
| `lmd` | `\lambda` | math | yours | greek.lua:13 |  |
| `orr` | `\lor{}$1` | math | changed | sets.lua:36 |  |
| `inn` | `\in $1` | math | changed | sets.lua:37 | Castel has the same (tex.snippets line 526) |
| `\\\` | `\setminus{}$1` | math | changed | sets.lua:38 | Castel has the same (tex.snippets line 489) |
| `sup=` | `\supseteq{}$1` | math | changed | sets.lua:39 |  |
| `eset` | `\varnothing{}$1` | math | changed | sets.lua:40 |  |
| `cset` | `\{ $0 : $1\}$2` | math | yours | sets.lua:42 |  |
| `mk` | `$$0$` | text | changed | text.lua:31 |  |
| `dm` | `$$⏎	$0⏎$$` | text | changed | text.lua:32 | The tab inside the block stays a tab character, as in Latex Suite (LuaSnip would turn it into spaces; say if you'd prefer that). |
| `dm after some text on the line` | `⏎$$⏎	$0⏎$$` | text | yours | text.lua:33 |  |
| `beg` | `[[0]]\begin{$0}⏎	$1⏎\end{$0}` | display | yours | environments.lua:27 | The tab inside the block stays a tab character, as in Latex Suite (LuaSnip would turn it into spaces; say if you'd prefer that). |
| `beg` | `[[0]]\begin{$0} $1 \end{$0}` | inline | yours | environments.lua:31 |  |
| `text` | `\text{$0}$1` | math | changed | text.lua:36 |  |
| `"` | `\text{ $0 }$1` | math | changed | text.lua:37 |  |
| `sr` | `^{2}` | math | default | operations.lua:13 |  |
| `cb` | `^{3}` | math | default | operations.lua:14 |  |
| `rd` | `^{$0}$1` | math | changed | operations.lua:15 |  |
| `//` | `\frac{$0}{$1}$2` | math | default | operations.lua:19 | Castel has the same (tex.snippets line 132) |
| `invs` | `^{-1}` | math | default | operations.lua:21 | Castel has the same (tex.snippets line 479) |
| `exp, log or ln` | `[[0]]\[[1]]` | math | default | operations.lua:23 |  |
| `conj` | `^{*}` | math | default | operations.lua:25 |  |
| `trace` | `\mathrm{Tr}` | math | default | operations.lua:34 |  |
| `a letter, then ddot` | `\ddot{[[0]]}` | math | default | accents.lua:29 |  |
| `a letter, then tilde` | `\tilde{[[0]]}` | math | default | accents.lua:30 |  |
| `a letter, then und` | `\underline{[[0]]}` | math | default | accents.lua:31 |  |
| `a letter, then vec` | `\vec{[[0]]}` | math | default | accents.lua:32 |  |
| `a letter, then ,.` | `\mathbf{[[0]]}` | math | default | accents.lua:33 |  |
| `a letter, then .,` | `\mathbf{[[0]]}` | math | default | accents.lua:34 |  |
| `\greek, then ,.` | `\boldsymbol{\[[0]]}` | math | default | accents.lua:35 |  |
| `\greek, then .,` | `\boldsymbol{\[[0]]}` | math | default | accents.lua:36 |  |
| `a letter (or \alpha…), then a digit` | `[[0]]_{[[1]]}` | math | yours | subscripts.lua:32 |  |
| `another digit after x_{2}` | `[[0]]_{[[1]][[2]]}` | math | changed | subscripts.lua:34 |  |
| `a digit after an accent, e.g. \dot{x}` | `\[[0]]{[[1]]}_{[[2]]}` | math | yours | subscripts.lua:36 |  |
| `another digit after \dot{x}_{3}` | `\[[0]]{[[1]]}_{[[2]][[3]]}` | math | yours | subscripts.lua:37 |  |
| `a digit after a double accent, e.g. \dot{\vec{a}}` | `\[[0]]{\[[1]]{[[2]]}}_{[[3]]}` | math | yours | subscripts.lua:38 |  |
| `another digit after \dot{\vec{a}}_{3}` | `\[[0]]{\[[1]]{[[2]]}}_{[[3]][[4]]}` | math | yours | subscripts.lua:39 |  |
| `xjj` | `x_{j}` | math | default | subscripts.lua:41 | Castel has the same (tex.snippets line 368) |
| `xp1` | `x_{n+1}` | math | default | subscripts.lua:42 | Castel has the same (tex.snippets line 378) |
| `ynn` | `y_{n}` | math | default | subscripts.lua:43 | Castel has the same (tex.snippets line 352) |
| `yii` | `y_{i}` | math | default | subscripts.lua:44 | Castel has the same (tex.snippets line 363) |
| `yjj` | `y_{j}` | math | default | subscripts.lua:45 | Castel has the same (tex.snippets line 373) |
| `ooo` | `\infty` | math | default | symbols.lua:12 | Castel has the same (tex.snippets line 320) |
| `\sum` | `\sum_{${0:i}=${1:1}}^{${2:N}} $3` | math, Tab | default | symbols.lua:15 |  |
| `\prod` | `\prod_{${0:i}=${1:1}}^{${2:N}} $3` | math, Tab | default | symbols.lua:16 |  |
| `+-` | `\pm` | math | default | symbols.lua:18 |  |
| `-+` | `\mp` | math | default | symbols.lua:19 |  |
| `...` | `\dots` | math | default | symbols.lua:20 |  |
| `xx` | `\times` | math | default | symbols.lua:22 | Castel has the same (tex.snippets line 428) |
| `**` | `\cdot` | math | default | symbols.lua:23 | Castel has the same (tex.snippets line 433) |
| `===` | `\equiv` | math | default | symbols.lua:26 |  |
| `!=` | `\neq` | math | default | symbols.lua:27 | Castel has the same (tex.snippets line 207) |
| `>=` | `\geq` | math | default | symbols.lua:28 |  |
| `<=` | `\leq` | math | default | symbols.lua:29 |  |
| `>>` | `\gg` | math | default | symbols.lua:30 | Castel has the same (tex.snippets line 493) |
| `<<` | `\ll` | math | default | symbols.lua:31 | Castel has the same (tex.snippets line 497) |
| `simm` | `\sim` | math | default | symbols.lua:32 |  |
| `sim=` | `\simeq` | math | default | symbols.lua:33 |  |
| `<->` | `\leftrightarrow ` | math | default | symbols.lua:36 | Castel has the same (tex.snippets line 469) |
| `->` | `\to` | math | default | symbols.lua:37 | Castel has the same (tex.snippets line 463) |
| `!>` | `\mapsto` | math | default | symbols.lua:38 | Castel has the same (tex.snippets line 474) |
| `=>` | `\implies` | math | default | symbols.lua:39 | Castel has the same (tex.snippets line 96) |
| `=<` | `\impliedby` | math | default | symbols.lua:40 | Castel has the same (tex.snippets line 100) |
| `LL` | `\mathcal{L}` | math | default | symbols.lua:42 |  |
| `HH` | `\mathcal{H}` | math | default | symbols.lua:43 |  |
| `RR` | `\mathbb{R}` | math | default | symbols.lua:44 |  |
| `ZZ` | `\mathbb{Z}` | math | default | symbols.lua:45 |  |
| `par` | `\frac{ \partial ${0:y} }{ \partial ${1:x} } $2` | math, Tab | default | calculus.lua:12 |  |
| `pa, then two letters` | `\frac{ \partial [[0]] }{ \partial [[1]] } ` | math, Tab | default | calculus.lua:13 |  |
| `ddt` | `\frac{d}{dt} ` | math | default | calculus.lua:14 |  |
| `int` | `[[0]]\int` | math | default | calculus.lua:16 |  |
| `\int` | `\int $0 \, d${1:x} $2` | math, Tab | default | calculus.lua:17 |  |
| `dint` | `\int_{${0:0}}^{${1:1}} $2 \, d${3:x} $4` | math | changed | calculus.lua:18 |  |
| `oinf` | `\int_{0}^{\infty} $0 \, d${1:x} $2` | math | default | calculus.lua:22 |  |
| `infi` | `\int_{-\infty}^{\infty} $0 \, d${1:x} $2` | math | default | calculus.lua:23 |  |
| `a letter (not h) right after \sin, \cos, …` | `\[[0]] [[1]]` | math | yours | trig.lua:27 |  |
| `a letter right after \sinh, \cosh, \tanh, \coth` | `\[[0]] [[1]]` | math | yours | trig.lua:28 |  |
| `arcsec, arccsc or arccot` | `\operatorname{[[0]]}$0` | math | default | trig.lua:31 |  |
| `U` | `\underbrace{ ${VISUAL} }_{ $0 }` | select, then key | changed | visual.lua:12 |  |
| `O` | `\overbrace{ ${VISUAL} }^{ $0 }` | select, then key | changed | visual.lua:13 |  |
| `B` | `\underset{ $0 }{ ${VISUAL} }` | select, then key | changed | visual.lua:14 |  |
| `C` | `\cancel{ ${VISUAL} }` | select, then key | changed | visual.lua:15 |  |
| `K` | `\cancelto{ $0 }{ ${VISUAL} }` | select, then key | changed | visual.lua:16 |  |
| `S` | `\sqrt{ ${VISUAL} }` | select, then key | changed | visual.lua:17 |  |
| `kbt` | `k_{B}T` | math | default | physics.lua:11 |  |
| `msun` | `M_{\odot}` | math | default | physics.lua:12 |  |
| `o+` | `\oplus ` | math | default | physics.lua:16 |  |
| `ket` | `\ket{$0} $1` | math | default | physics.lua:19 |  |
| `brk` | `\braket{ $0 | $1 } $2` | math | default | physics.lua:20 |  |
| `outer` | `\ket{${0:\psi}} \bra{${0:\psi}} $1` | math | default | physics.lua:21 |  |
| `pu` | `\pu{ $0 }` | math | default | chemistry.lua:11 |  |
| `cee` | `\ce{ $0 }` | math | default | chemistry.lua:12 |  |
| `he4` | `{}^{4}_{2}He ` | math | default | chemistry.lua:13 |  |
| `he3` | `{}^{3}_{2}He ` | math | default | chemistry.lua:14 |  |
| `iso` | `{}^{${0:4}}_{${1:2}}${2:He}` | math | default | chemistry.lua:15 |  |
| `pmat, bmat, Bmat, vmat or Vmat` | `\begin{[[0]]rix}⏎$0⏎\end{[[0]]rix}` | display | default | environments.lua:33 |  |
| `matrix, cases, align or array` | `\begin{[[0]]}⏎$0⏎\end{[[0]]}` | display | default | environments.lua:39 |  |
| `pmat, bmat, Bmat, vmat or Vmat` | `\begin{[[0]]rix}$0\end{[[0]]rix}` | inline | default | environments.lua:33 |  |
| `matrix, cases, align or array` | `\begin{[[0]]}$0\end{[[0]]}` | inline | default | environments.lua:39 |  |
| `avg` | `\langle $0 \rangle $1` | math | default | brackets.lua:12 |  |
| `norm` | `\lvert $0 \rvert $1` | math | default | brackets.lua:13 |  |
| `Norm` | `\lVert $0 \rVert $1` | math | default | brackets.lua:14 |  |
| `(` | `(${VISUAL})` | select, then key | changed | visual.lua:18 |  |
| `[` | `[${VISUAL}]` | select, then key | changed | visual.lua:19 |  |
| `{` | `{${VISUAL}}` | select, then key | changed | visual.lua:20 |  |
| `(` | `($0)$1` | math | default | brackets.lua:18 |  |
| `{` | `{$0}$1` | math | default | brackets.lua:19 |  |
| `[` | `[$0]$1` | math | default | brackets.lua:20 |  |
| `lr(` | `\left( $0 \right) $1` | math | default | brackets.lua:21 |  |
| `lr{` | `\left\{ $0 \right\} $1` | math | default | brackets.lua:22 |  |
| `lr[` | `\left[ $0 \right] $1` | math | default | brackets.lua:23 |  |
| `lr|` | `\left| $0 \right| $1` | math | default | brackets.lua:24 |  |
| `lra` | `\left< $0 \right> $1` | math | default | brackets.lua:25 | Castel has the same (tex.snippets line 258) |
| `tayl` | `${0:f}(${1:x} + ${2:h}) = ${0:f}(${1:x}) + ${0:f}'(${1:x})${2:h} + ${0:f}''(${1:x}) \frac{${2:h}^{2}}{2!} + \dots$3` | math | default | calculus.lua:25 |  |
| `iden, then a digit` | `(JavaScript function)` | math | default | environments.lua:43 |  |
| `dm at the end of a list item (- …, 1. …)` | `(JavaScript function)` | text | changed | text.lua:34 |  |

Each was typed in a .tex and a .md file in both setups: identical.

## New from Castel

Snippets from Gilles Castel's set that you didn't have, added because they can't fire while you type normally (they pass the mangling and English tests). Each is one line in a snippet file: delete the line to veto it.

### table → table + tabular environment

Trigger: `table` · now in text.lua:41 · Castel's tex.snippets line 54

Added as Tab at the start of a line in .tex. Simplified: the column spec doesn't generate the `&`s.

- .tex: in `‸` type `table<Tab>` → before `table ‸`, now `\begin{table}[htpb]⏎	\centering⏎	\caption{caption}⏎	\label{tab:label}⏎	\begin{tabular}{c}⏎	⏎	\end{tabular}⏎\end{table}` (selected: `htpb`)

### fig → figure environment

Trigger: `fig` · now in text.lua:50 · Castel's tex.snippets line 65

Added as Tab at the start of a line in .tex. Simplified: the label isn't derived from the file name.

- .tex: in `‸` type `fig<Tab>` → before `fig ‸`, now `\begin{figure}[htpb]⏎	\centering⏎	\includegraphics[width=0.8\textwidth]{}⏎	\caption{caption}⏎	\label{fig:label}⏎\end{figure}` (selected: `htpb`)

### desc → description environment

Trigger: `desc` · now in text.lua:57 · Castel's tex.snippets line 86

Added as Tab at the start of a line in .tex.

- .tex: in `‸` type `desc<Tab>` → before `desc  ‸`, now `\begin{description}⏎	\item[‸] ⏎\end{description}`

### pac → \usepackage[options]{package}

Trigger: `pac` · now in text.lua:61 · Castel's tex.snippets line 92

Added as Tab at the start of a line in .tex.

- .tex: in `‸` type `pac<Tab>` → before `pac ‸`, now `\usepackage[options]{package}` (selected: `options`)

### / → select, then /: \frac{selection}{}

Trigger: `/` · now in visual.lua:23 · Castel's tex.snippets line 136

Added to your visual keys.

- .tex: in `$‸a+b$` type `/` with 3 characters selected → before `$/‸$`, now `$\frac{a+b}{‸}$`

### lr → \left( … \right) on Tab

Trigger: `lr` · now in brackets.lua:28 · Castel's tex.snippets line 234

Added (Tab).

- .tex: in `$‸$` type `lr<Tab>` → before `$lr$‸`, now `$\left( ‸ \right)$`

### lrb → \left\{ … \right\} on Tab

Trigger: `lrb` · now in brackets.lua:29 · Castel's tex.snippets line 250

Added (Tab).

- .tex: in `$‸$` type `lrb<Tab>` → before `$lrb$‸`, now `$\left\{ ‸ \right\}$`

### plot → pgfplots figure

Trigger: `plot` · now in text.lua:62 · Castel's tex.snippets line 391

Added as Tab at the start of a line in .tex (needs \usepackage{pgfplots}).

- .tex: in `‸` type `plot<Tab>` → before `plot  ‸`, now `\begin{figure}[‸]⏎	\centering⏎	\begin{tikzpicture}⏎		\begin{axis}[⏎			xmin= -10, xmax= 10,⏎			ymin= -10, ymax = 10,⏎			axis lines = middle,⏎		]⏎			\addplot[domain=-10:10, samples=100]{};⏎		\end{axis}⏎	\end{tikzpicture}⏎	\caption{}⏎	\label{}⏎\end{figure}`

### mcal → \mathcal{…}

Trigger: `mcal` · now in symbols.lua:51 · Castel's tex.snippets line 414

Added.

- .tex: in `$‸$` type `mcal` → before `$mcal‸$`, now `$\mathcal{‸}$`

### lll → \ell

Trigger: `lll` · now in symbols.lua:50 · Castel's tex.snippets line 418

Added.

- .tex: in `$‸$` type `lll` → before `$lll‸$`, now `$\ell‸$`

### star → \star

Trigger: `star` · now in symbols.lua:52 · Castel's tex.snippets line 444

Added (one of the names Castel's rule puts a backslash before).

- .tex: in `$‸$` type `star` → before `$star‸$`, now `$\star‸$`

### perp → \perp

Trigger: `perp` · now in symbols.lua:53 · Castel's tex.snippets line 444

Added (one of the names Castel's rule puts a backslash before).

- .tex: in `$‸$` type `perp` → before `$perp‸$`, now `$\perp‸$`

### pi → \pi

Trigger: `pi` · now in greek.lua:14 · Castel's tex.snippets line 456

Added (one of the names Castel's rule puts a backslash before). \varpi, \epsilon, spin type as written.

- .tex: in `$‸$` type `pi` → before `$pi‸$`, now `$\pi‸$`

### zeta → \zeta

Trigger: `zeta` · now in greek.lua:15 · Castel's tex.snippets line 456

Added (one of the names Castel's rule puts a backslash before).

- .tex: in `$‸$` type `zeta` → before `$zeta‸$`, now `$\zeta‸$`

### compl → ^{c}

Trigger: `compl` · now in operations.lua:30 · Castel's tex.snippets line 484

Added (after a letter: Acompl → A^{c}).

- .tex: in `$A‸$` type `compl` → before `$Acompl‸$`, now `$A^{c}‸$`

### ~~ → \sim

Trigger: `~~` · now in symbols.lua:54 · Castel's tex.snippets line 502

Added (you also have simm).

- .tex: in `$‸$` type `~~` → before `$~~‸$`, now `$\sim‸$`

### <! → \triangleleft (normal subgroup)

Trigger: `<!` · now in symbols.lua:55 · Castel's tex.snippets line 566

Added.

- .tex: in `$‸$` type `<!` → before `$<!‸$`, now `$\triangleleft‸$`

### <> → \diamond

Trigger: `<>` · now in symbols.lua:56 · Castel's tex.snippets line 570

Added.

- .tex: in `$‸$` type `<>` → before `$<>‸$`, now `$\diamond‸$`

### bigfun → align* with a function's domain and rule

Trigger: `bigfun` · now in text.lua:77 · Castel's tex.snippets line 596

Added as Tab at the start of a line in .tex (his fires anywhere by itself).

- .tex: in `‸` type `bigfun<Tab>` → before `bigfun  ‸`, now `\begin{align*}⏎	‸:  &\longrightarrow  \\⏎	 &\longmapsto () = ⏎\end{align*}`

### DD → \mathbb{D}

Trigger: `DD` · now in symbols.lua:57 · Castel's tex.snippets line 640

Added.

- .tex: in `$‸$` type `DD` → before `$DD‸$`, now `$\mathbb{D}‸$`


## Castel differs, kept yours

Castel has a snippet for the same thing (or the same trigger) and it works differently. Yours stays.

### beg: Castel's gives \begin{}…\end{} at the start of a line in text, by itself

Trigger: `beg` · Castel's tex.snippets line 43

Yours works in maths (display: on separate lines; inline: one line). Castel's would fire on “began”/“begin…” at a line start (see Needs your call).

- .tex: in `‸` type `beg` → before `beg‸`, now `beg‸`

### ...: Castel's gives \ldots

Trigger: `...` · Castel's tex.snippets line 50

Yours gives `\dots` (amsmath picks the right dots for the context).

- .tex: in `$‸$` type `...` → before `$\dots‸$`, now `$\dots‸$`

### mk: Castel's gives $…$, plus a space after it unless punctuation follows

Trigger: `mk` · Castel's tex.snippets line 109

Yours: `$…$`, Tab leaves it. No automatic space.

- .tex: in `Let ‸` type `mk` → before `Let $‸$`, now `Let $‸$`

### dm: Castel's gives \[ … .\] wrapping the selection

Trigger: `dm` · Castel's tex.snippets line 118

Yours: a tab-indented block on its own lines; `$$ … $$` in .md; after text or in a list it starts on the next line.

- .tex: in `Intro⏎‸` type `dm` → before `Intro⏎\[⏎	‸⏎\]`, now `Intro⏎\[⏎	‸⏎\]`

### (\d+|…)/: Castel's gives symbol fraction: x/ → \frac{x}{}

Trigger: `(\d+|…)/` · Castel's tex.snippets line 141

Your auto-fraction (the `/` key) does this, including groups in brackets.

- .tex: in `$‸$` type `x/` → before `$\frac{x}{‸}$`, now `$\frac{x}{‸}$`

### …)/: Castel's gives (a+b)/ → \frac{a+b}{}

Trigger: `…)/` · Castel's tex.snippets line 147

Your auto-fraction does this.

- .tex: in `$‸$` type `(a+b)/` → before `$\frac{a+b}{‸}$`, now `$\frac{a+b}{‸}$`

### x1: Castel's gives x_1 (no braces), at a word boundary

Trigger: `x1` · Castel's tex.snippets line 162

Yours: `x_{1}`, also after Greek letters and accents, and more digits extend it.

- .tex: in `$‸$` type `x1` → before `$x_{1}‸$`, now `$x_{1}‸$`

### x_11: Castel's gives x_{11}

Trigger: `x_11` · Castel's tex.snippets line 167

Yours: `x_{2}4` → `x_{24}`.

- .tex: in `$‸$` type `x_11` → before `$x_{11‸}$`, now `$x_{11‸}$`

### ceil: Castel's gives \left\lceil … \right\rceil

Trigger: `ceil` · Castel's tex.snippets line 212

Yours: `\lceil … \rceil` (auto-enlarge adds \left/\right when needed).

- .tex: in `$‸$` type `ceil` → before `$\lceil ‸ \rceil$`, now `$\lceil ‸ \rceil$`

### floor: Castel's gives \left\lfloor … \right\rfloor

Trigger: `floor` · Castel's tex.snippets line 217

Yours: `\lfloor … \rfloor`.

- .tex: in `$‸$` type `floor` → before `$\lfloor ‸ \rfloor$`, now `$\lfloor ‸ \rfloor$`

### pmat: Castel's gives \begin{pmatrix} … \end{pmatrix} on one line

Trigger: `pmat` · Castel's tex.snippets line 221

Yours: on separate lines in display maths, one line inline.

- .tex: in `$‸$` type `pmat` → before `$\begin{pmatrix}‸\end{pmatrix}$`, now `$\begin{pmatrix}‸\end{pmatrix}$`

### bmat: Castel's gives \begin{bmatrix} … \end{bmatrix} on one line

Trigger: `bmat` · Castel's tex.snippets line 225

Yours: on separate lines in display maths, one line inline.

- .tex: in `$‸$` type `bmat` → before `$\begin{bmatrix}‸\end{bmatrix}$`, now `$\begin{bmatrix}‸\end{bmatrix}$`

### lr(: Castel's gives \left( … \right) on Tab

Trigger: `lr(` · Castel's tex.snippets line 238

Yours expands by itself.

- .tex: in `$‸$` type `lr(<Tab>` → before `$\left(  \right)‸$`, now `$\left(  \right)‸$`

### lr|: Castel's gives \left| … \right| on Tab

Trigger: `lr|` · Castel's tex.snippets line 242

Yours expands by itself.

- .tex: in `$‸$` type `lr|<Tab>` → before `$\left|  \right|‸$`, now `$\left|  \right|‸$`

### lr{: Castel's gives \left\{ … \right\} on Tab

Trigger: `lr{` · Castel's tex.snippets line 246

Yours expands by itself.

- .tex: in `$‸$` type `lr{<Tab>` → before `$\left\{  \right\}‸$`, now `$\left\{  \right\}‸$`

### lr[: Castel's gives \left[ … \right] on Tab

Trigger: `lr[` · Castel's tex.snippets line 254

Yours expands by itself.

- .tex: in `$‸$` type `lr[<Tab>` → before `$\left[  \right]‸$`, now `$\left[  \right]‸$`

### conj: Castel's gives \overline{…}

Trigger: `conj` · Castel's tex.snippets line 263

Yours: `^{*}`.

- .tex: in `$z‸$` type `conj` → before `$z^{*}‸$`, now `$z^{*}‸$`

### lim: Castel's gives \lim_{n \to \infty} on Tab

Trigger: `lim` · Castel's tex.snippets line 275

Yours expands by itself, with n and \infty as tabstops.

- .tex: in `$‸$` type `lim<Tab>` → before `$\lim_{ n \to \infty }$` (selected: `\infty`), now `$\lim_{ n \to \infty }$` (selected: `\infty`)

### sq: Castel's gives \sqrt{…}

Trigger: `sq` · Castel's tex.snippets line 292

Yours: `\sqrt{ … }` (spaces inside).

- .tex: in `$‸$` type `sq` → before `$\sqrt{ ‸ }$`, now `$\sqrt{ ‸ }$`

### sr: Castel's gives ^2

Trigger: `sr` · Castel's tex.snippets line 297

Yours: `^{2}`.

- .tex: in `$x‸$` type `sr` → before `$x^{2}‸$`, now `$x^{2}‸$`

### cb: Castel's gives ^3

Trigger: `cb` · Castel's tex.snippets line 302

Yours: `^{3}`.

- .tex: in `$x‸$` type `cb` → before `$x^{3}‸$`, now `$x^{3}‸$`

### td: Castel's gives ^{…}

Trigger: `td` · Castel's tex.snippets line 307

Yours: `rd`.

- .tex: in `$x‸$` type `td` → before `$xtd‸$`, now `$xtd‸$`

### rd: Castel's gives ^{(…)}

Trigger: `rd` · Castel's tex.snippets line 312

Yours: `rd` → `^{…}`.

- .tex: in `$x‸$` type `rd` → before `$x^{‸}$`, now `$x^{‸}$`

### __: Castel's gives _{…}

Trigger: `__` · Castel's tex.snippets line 316

Yours: a single `_`.

- .tex: in `$x‸$` type `__` → before `$x_{_{‸}}$`, now `$x_{_{‸}}$`

### <=: Castel's gives \le

Trigger: `<=` · Castel's tex.snippets line 328

Yours: `\leq`.

- .tex: in `$‸$` type `<=` → before `$\leq‸$`, now `$\leq‸$`

### >=: Castel's gives \ge

Trigger: `>=` · Castel's tex.snippets line 332

Yours: `\geq`.

- .tex: in `$‸$` type `>=` → before `$\geq‸$`, now `$\geq‸$`

### EE: Castel's gives \exists

Trigger: `EE` · Castel's tex.snippets line 337

Yours: `exx`.

- .tex: in `$‸$` type `EE` → before `$EE‸$`, now `$EE‸$`

### AA: Castel's gives \forall

Trigger: `AA` · Castel's tex.snippets line 342

Yours: `fall`.

- .tex: in `$‸$` type `AA` → before `$AA‸$`, now `$AA‸$`

### norm: Castel's gives \|…\|

Trigger: `norm` · Castel's tex.snippets line 438

Yours: `\lvert … \rvert` (and `Norm` → `\lVert … \rVert`).

- .tex: in `$‸$` type `norm` → before `$\lvert ‸ \rvert$`, now `$\lvert ‸ \rvert$`

### sin cos cot csc ln log exp star perp: Castel's gives add the backslash

Trigger: `sin cos cot csc ln log exp star perp` · Castel's tex.snippets line 444

sin cos cot csc ln log exp: same as yours. star and perp: added (see New from Castel).

- .tex: in `$‸$` type `sin` → before `$\sin‸$`, now `$\sin‸$`
- .tex: in `$‸$` type `perp` → before `$perp‸$`, now `$\perp‸$`

### dint: Castel's gives \int_{-\infty}^{\infty}

Trigger: `dint` · Castel's tex.snippets line 450

Yours: `\int_{0}^{1} … \, dx` with tabstops.

- .tex: in `$‸$` type `dint` → before `$\int_{0}^{1}  \, dx$` (selected: `0`), now `$\int_{0}^{1}  \, dx$` (selected: `0`)

### arcsin arccos arctan arccot arccsc arcsec pi zeta int: Castel's gives add the backslash

Trigger: `arcsin arccos arctan arccot arccsc arcsec pi zeta int` · Castel's tex.snippets line 456

arcsin arccos arctan int: same as yours. arccot arccsc arcsec: yours give \operatorname{…} (\arccot is not a LaTeX command). pi and zeta: added (see New from Castel).

- .tex: in `$‸$` type `arccot` → before `$\operatorname{arccot}‸$`, now `$\operatorname{arccot}‸$`
- .tex: in `$‸$` type `pi` → before `$pi‸$`, now `$\pi‸$`

### cc: Castel's gives \subset

Trigger: `cc` · Castel's tex.snippets line 517

Yours: `psub` (\subset) and `sub` (\subseteq).

- .tex: in `$‸$` type `cc` → before `$cc‸$`, now `$cc‸$`

### NN: Castel's gives \N (his macro)

Trigger: `NN` · Castel's tex.snippets line 530

Yours: `NN` → `\bigcap_{}`, `Nat` → `\mathbb{N}`.

- .tex: in `$‸$` type `NN` → before `$\bigcap_{‸}$`, now `$\bigcap_{‸}$`

### Nn: Castel's gives \cap

Trigger: `Nn` · Castel's tex.snippets line 534

Yours: `nn`.

- .tex: in `$‸$` type `Nn` → before `$Nn‸$`, now `$Nn‸$`

### UU: Castel's gives \cup

Trigger: `UU` · Castel's tex.snippets line 538

Yours: `uu` → \cup, `UU` → \bigcup_{}.

- .tex: in `$‸$` type `UU` → before `$\bigcup_{‸}$`, now `$\bigcup_{‸}$`

### OO: Castel's gives \O (his macro)

Trigger: `OO` · Castel's tex.snippets line 550

Yours: `eset` → `\varnothing{}`.

- .tex: in `$‸$` type `OO` → before `$OO‸$`, now `$OO‸$`

### RR: Castel's gives \R (his macro)

Trigger: `RR` · Castel's tex.snippets line 554

Yours: `\mathbb{R}`.

- .tex: in `$‸$` type `RR` → before `$\mathbb{R}‸$`, now `$\mathbb{R}‸$`

### ZZ: Castel's gives \Z (his macro)

Trigger: `ZZ` · Castel's tex.snippets line 562

Yours: `\mathbb{Z}`.

- .tex: in `$‸$` type `ZZ` → before `$\mathbb{Z}‸$`, now `$\mathbb{Z}‸$`

### sts: Castel's gives _\text{…}, not after i

Trigger: `sts` · Castel's tex.snippets line 576

Yours: same output; it can't fire inside any \command now (Castel guards only \exists).

- .tex: in `$x‸$` type `sts` → before `$x_\text{‸}$`, now `$x_\text{‸}$`

### bar: Castel's gives \overline{…}

Trigger: `bar` · Castel's tex.snippets line 609

Yours: `\bar{…}`.

- .tex: in `$‸$` type `bar` → before `$\bar{‸}$`, now `$\bar{‸}$`

### xbar: Castel's gives \overline{x}

Trigger: `xbar` · Castel's tex.snippets line 615

Yours: `\bar{x}`.

- .tex: in `$‸$` type `xbar` → before `$\bar{x}‸$`, now `$\bar{x}‸$`

### HH: Castel's gives \mathbb{H}

Trigger: `HH` · Castel's tex.snippets line 636

Yours: `\mathcal{H}`.

- .tex: in `$‸$` type `HH` → before `$\mathcal{H}‸$`, now `$\mathcal{H}‸$`


## Dropped

Not carried over: Castel's personal snippets, Castel snippets that would collide with yours or with ordinary typing, and two of your snippets that never fired.

### template: Castel's gives basic article template (Dutch babel, Inkscape figure macros)

Trigger: `template` · Castel's tex.snippets line 15 · Castel's personal

Personal: Dutch babel and his Inkscape figure workflow. Your new files start from templates/article.tex.

- .tex: in `‸` type `template<Tab>` → before `template  ‸`, now `template  ‸`

### enum: Castel's gives enumerate, at the start of a line, by itself

Trigger: `enum` · Castel's tex.snippets line 74 · Castel, collides

Would fire on “enumerate…”/“enumeration” at a line start (see Needs your call).

- .tex: in `‸` type `enum` → before `enum‸`, now `enum‸`

### item: Castel's gives itemize, at the start of a line, by itself

Trigger: `item` · Castel's tex.snippets line 80 · Castel, collides

Would fire on “items”/“item” at a line start (see Needs your call).

- .tex: in `‸` type `item` → before `item‸`, now `item‸`

### ali: Castel's gives align* at the start of a line, by itself

Trigger: `ali` · Castel's tex.snippets line 124 · Castel, collides

Would fire on “align…”, “alias”, “alike” at a line start.

- .tex: in `‸` type `ali` → before `ali‸`, now `ali‸`

### sympy: Castel's gives sympy block

Trigger: `sympy` · Castel's tex.snippets line 172 · Castel's personal

Needs Python's sympy; part of Castel's own workflow.

- .tex: in `‸` type `sympy<Tab>` → before `sympy ‸`, now `sympy ‸`

### sympy…sympy: Castel's gives evaluate with sympy

Trigger: `sympy…sympy` · Castel's tex.snippets line 177 · Castel's personal

Needs Python's sympy.

- .tex: in `$‸$` type `sympy 1+1 sympy<Tab>` → before `$sympy 1+1 sympy$‸`, now `$sympy 1+1 sympy$‸`

### math: Castel's gives Mathematica block

Trigger: `math` · Castel's tex.snippets line 189 · Castel's personal

Needs wolframscript.

- .tex: in `‸` type `math<Tab>` → before `math  ‸`, now `math  ‸`

### math…math: Castel's gives evaluate with wolframscript

Trigger: `math…math` · Castel's tex.snippets line 194 · Castel's personal

Needs wolframscript.

- .tex: in `$‸$` type `math 1+1 math<Tab>` → before `$math 1+1 math$‸`, now `$math 1+1 math$‸`

### ==: Castel's gives &= … \\ (align row)

Trigger: `==` · Castel's tex.snippets line 203 · Castel, collides

Would fire before your `===` (\equiv) could.

- .tex: in `$a ‸$` type `== b` → before `$a == b‸$`, now `$a == b‸$`

### (): Castel's gives \left( … \right)

Trigger: `()` · Castel's tex.snippets line 230 · Castel, can't be reached

Your `(` closes itself, so `()` is never two typed characters; use `lr(`.

- .tex: in `$‸$` type `()` → before `$()‸$`, now `$()‸$`

### sum: Castel's gives \sum_{n=1}^{\infty} a_n z^n on Tab

Trigger: `sum` · Castel's tex.snippets line 267 · Castel, can't be reached

Your `sum` → `\sum` fires first; Tab then gives `\sum_{i=1}^{N}`.

- .tex: in `$‸$` type `sum<Tab>` → before `$\sum_{i=1}^{N}$` (selected: `i`), now `$\sum_{i=1}^{N}$` (selected: `i`)

### taylor: Castel's gives Taylor series on Tab

Trigger: `taylor` · Castel's tex.snippets line 271 · Castel, can't be reached

Your `tayl` fires first (Taylor expansion with mirrors).

- .tex: in `$‸$` type `taylor<Tab>` → before `$or(x + h) = or(x) + or'(x)h + or''(x) \frac{h^{2}}{2!} + \dots$` (selected: `x`), now `$or(x + h) = or(x) + or'(x)h + or''(x) \frac{h^{2}}{2!} + \dots$` (selected: `x`)

### limsup: Castel's gives \limsup_{n \to \infty} on Tab

Trigger: `limsup` · Castel's tex.snippets line 279 · Castel, can't be reached

Your `lim` fires at “lim”. Type `\limsup` (now types as written).

- .tex: in `$‸$` type `limsup<Tab>` → before `$\lim_{ sup \to \infty }$` (selected: `\infty`), now `$\lim_{ sup \to \infty }$` (selected: `\infty`)

### prod: Castel's gives \prod_{n=1}^{\infty} on Tab

Trigger: `prod` · Castel's tex.snippets line 283 · Castel, can't be reached

Your `prod` → `\prod` fires first; Tab then gives `\prod_{i=1}^{N}`.

- .tex: in `$‸$` type `prod<Tab>` → before `$\prod_{i=1}^{N}$` (selected: `i`), now `$\prod_{i=1}^{N}$` (selected: `i`)

### part: Castel's gives \frac{\partial V}{\partial x} on Tab

Trigger: `part` · Castel's tex.snippets line 287 · Castel, collides

Your `pa‹y›‹x›` Tab would read “part” as ∂r/∂t.

- .tex: in `$‸$` type `part<Tab>` → before `$\frac{ \partial r }{ \partial t }‸$`, now `$\frac{ \partial r }{ \partial t }‸$`

### rij: Castel's gives (x_n)_{n\in\N} (Dutch: rij = sequence)

Trigger: `rij` · Castel's tex.snippets line 324 · Castel's personal

Uses his \N macro.

- .tex: in `$‸$` type `rij<Tab>` → before `$rij$‸`, now `$rij$‸`

### xnn: Castel's gives x_{n}

Trigger: `xnn` · Castel's tex.snippets line 347 · Castel, kept off like yours

You turned it off.

- .tex: in `$‸$` type `xnn` → before `$x\cap‸$`, now `$xnn‸$`

### xii: Castel's gives x_{i}

Trigger: `xii` · Castel's tex.snippets line 358 · Castel, kept off like yours

You turned it off.

- .tex: in `$‸$` type `xii` → before `$xii‸$`, now `$xii‸$`

### xmm: Castel's gives x_{m}

Trigger: `xmm` · Castel's tex.snippets line 383 · Castel, kept off like yours

Not added: same family as xnn and xii, which you turned off.

- .tex: in `$‸$` type `xmm` → before `$xmm‸$`, now `$xmm‸$`

### R0+: Castel's gives \R_0^+

Trigger: `R0+` · Castel's tex.snippets line 387 · Castel's personal

Uses his \R macro.

- .tex: in `$‸$` type `R0+` → before `$R_{0}+‸$`, now `$R_{0}+‸$`

### nn: Castel's gives TikZ node on Tab

Trigger: `nn` · Castel's tex.snippets line 408 · Castel, collides

Your `nn` is \cap.

- .tex: in `‸` type `nn<Tab>` → before `nn  ‸`, now `nn  ‸`

### ||: Castel's gives \mid

Trigger: `||` · Castel's tex.snippets line 511 · Castel, collides

Would turn `||x||` into ` \mid x \mid`.

- .tex: in `$‸$` type `||x||` → before `$||x||‸$`, now `$||x||‸$`

### notin: Castel's gives \not\in

Trigger: `notin` · Castel's tex.snippets line 521 · Castel, kept off like yours

Latex Suite has the same default; it is off in your file (you use `ninn` → \notin).

- .tex: in `$‸$` type `notin` → before `$notin‸$`, now `$notin‸$`

### uuu: Castel's gives \bigcup_{i \in I}

Trigger: `uuu` · Castel's tex.snippets line 542 · Castel, can't be reached

Your `uu` fires first; use `UU`.

- .tex: in `$‸$` type `uuu` → before `$\cu\pu{ ‸ }$`, now `$\cupu‸$`

### nnn: Castel's gives \bigcap_{i \in I}

Trigger: `nnn` · Castel's tex.snippets line 546 · Castel, can't be reached

Your `nn` fires first; use `NN`.

- .tex: in `$‸$` type `nnn` → before `$\capn‸$`, now `$\capn‸$`

### QQ: Castel's gives \Q (his macro)

Trigger: `QQ` · Castel's tex.snippets line 558 · Castel, kept off like yours

The Latex Suite default QQ → \mathbb{Q} is off in your file.

- .tex: in `$‸$` type `QQ` → before `$QQ‸$`, now `$QQ‸$`

### tt: Castel's gives \text{…}

Trigger: `tt` · Castel's tex.snippets line 581 · Castel, collides

`tt` is also t·t; you have `text` and `"`.

- .tex: in `$‸$` type `tt` → before `$tt‸$`, now `$tt‸$`

### case: Castel's gives cases environment

Trigger: `case` · Castel's tex.snippets line 586 · Castel, collides

Would fire before your `cases`.

- .tex: in `$‸$` type `case` → before `$case‸$`, now `$case‸$`

### SI: Castel's gives \SI{}{} (siunitx)

Trigger: `SI` · Castel's tex.snippets line 592 · Castel, collides

Needs siunitx; you use \pu (mhchem).

- .tex: in `$‸$` type `SI` → before `$SI‸$`, now `$SI‸$`

### cvec: Castel's gives column vector

Trigger: `cvec` · Castel's tex.snippets line 603 · Castel, collides

Your `xvec` postfix turns “cvec” into \vec{c}.

- .tex: in `$‸$` type `cvec` → before `$\vec{c}‸$`, now `$\vec{c}‸$`

### letw: Castel's gives “Let Ω ⊂ ℂ be open.”

Trigger: `letw` · Castel's tex.snippets line 631 · Castel's personal

Text from one of his courses.

- .tex: in `‸` type `letw` → before `letw‸`, now `letw‸`

### CC → \mathbb{C} (second CC in snippets.js, line 230)

Trigger: `CC` · yours, never fired

Never fired: your first CC (line 4, → \mathcal{C}) always won. The live one is kept (sets.lua).

- .tex: in `$‸$` type `CC` → before `$\mathcal{C}‸$`, now `$\mathcal{C}‸$`
- .md: in `$‸$` type `CC` → before `$\mathcal{C}‸$`, now `$\mathcal{C}‸$`

### NN → \mathbb{N} (second NN in snippets.js, line 233)

Trigger: `NN` · yours, never fired

Never fired: your first NN (line 11, → \bigcap_{}) always won. The live one is kept (sets.lua); `Nat` gives \mathbb{N}.

- .tex: in `$‸$` type `NN` → before `$\bigcap_{‸}$`, now `$\bigcap_{‸}$`
- .md: in `$‸$` type `NN` → before `$\bigcap_{‸}$`, now `$\bigcap_{‸}$`


## Still turned off

Latex Suite defaults you had commented out or removed (and your own commented-out ones). They stay off. Any of them can be added back as one line.

| Trigger | What it would do | Status |
| --- | --- | --- |
| `/([^\\\w])beg/` | Latex Suite's own beg (yours replaced it) | not in your file |
| `@a` | @a → \alpha | commented out (snippets.js line 57) |
| `@b` | @b → \beta | commented out (snippets.js line 58) |
| `@g` | @g → \gamma | commented out (snippets.js line 59) |
| `@G` | @G → \Gamma | commented out (snippets.js line 60) |
| `@d` | @d → \delta | commented out (snippets.js line 61) |
| `@D` | @D → \Delta | commented out (snippets.js line 62) |
| `@e` | @e → \epsilon | commented out (snippets.js line 63) |
| `:e` | :e → \varepsilon | commented out (snippets.js line 64) |
| `@z` | @z → \zeta | commented out (snippets.js line 65) |
| `@t` | @t → \theta | commented out (snippets.js line 66) |
| `@T` | @T → \Theta | commented out (snippets.js line 67) |
| `:t` | :t → \vartheta | commented out (snippets.js line 68) |
| `@i` | @i → \iota | commented out (snippets.js line 69) |
| `@k` | @k → \kappa | commented out (snippets.js line 70) |
| `@l` | @l → \lambda | commented out (snippets.js line 71) |
| `@L` | @L → \Lambda | commented out (snippets.js line 72) |
| `@s` | @s → \sigma | commented out (snippets.js line 73) |
| `@S` | @S → \Sigma | commented out (snippets.js line 74) |
| `@u` | @u → \upsilon | commented out (snippets.js line 75) |
| `@U` | @U → \Upsilon | commented out (snippets.js line 76) |
| `@o` | @o → \omega | commented out (snippets.js line 77) |
| `@O` | @O → \Omega | commented out (snippets.js line 78) |
| `ome` | ome → \omega | commented out (snippets.js line 79) |
| `Ome` | Ome → \Omega | commented out (snippets.js line 80) |
| `/\n\s*\"/` | " at the start of a line in maths → \text{ } | not in your file |
| `/(\d)rt/` | 3rt → \sqrt[3]{ } (n-th root) | not in your file |
| `/\bee/` | ee → e^{ } (yours is the plain ee) | not in your file |
| `/(\\?)([A-Za-z]+)(\d)/` | Latex Suite's auto subscript, which also puts a space after other macros (\leq3 → \leq 3); yours replaced it | not in your file |
| `\\(${ACCENT})\{(\\${GREEK}|[A-Za-z])\}(?:_\{(\d+)\})?(\d)` | accent + digit (one rule); yours has it as two rules | not in your file |
| `\\(${ACCENT})\{\\(${ACCENT})\{(\\${GREEK}|[A-Za-z])\}\}(?:_\{(\d+)\})?(\d)` | double accent + digit (one rule); yours has it as two rules | not in your file |
| `xnn` | xnn → x_{n} | commented out (snippets.js line 183) |
| `\xii` | \xii → x_{i} | commented out (snippets.js line 184) |
| `deg` | deg → \degree | not in your file |
| `notin` | notin → \not\in | not in your file |
| `sub=` | sub= → \subseteq | not in your file |
| `/(n?)e\\xi sts/` | e\xi sts → \exists (repairs xi firing inside “exists”) | not in your file |
| `QQ` | QQ → \mathbb{Q} | not in your file |
| `([^\\])(${GREEK})` | alpha → \alpha: a backslash before every Greek name | commented out (snippets.js line 241) |
| `([^\\])(${SYMBOL})` | subset → \subset …: a backslash before common symbol names | commented out (snippets.js line 242) |
| `\\(${GREEK}|${SYMBOL}) sr` | \alpha sr → \alpha^{2} | commented out (snippets.js line 246) |
| `\\(${GREEK}|${SYMBOL}) cb` | \alpha cb → \alpha^{3} | commented out (snippets.js line 247) |
| `\\(${GREEK}|${SYMBOL}) rd` | \alpha rd → \alpha^{ } | commented out (snippets.js line 248) |
| `\\(${GREEK}) hat` | \alpha hat → \hat{\alpha} | commented out (snippets.js line 249) |
| `\\(${GREEK}) dot` | \alpha dot → \dot{\alpha} | commented out (snippets.js line 250) |
| `\\(${GREEK}) bar` | \alpha bar → \bar{\alpha} | commented out (snippets.js line 251) |
| `\\(${GREEK}) vec` | \alpha vec → \vec{\alpha} | commented out (snippets.js line 252) |
| `\\(${GREEK}) tilde` | \alpha tilde → \tilde{\alpha} | commented out (snippets.js line 253) |
| `\\(${GREEK}) und` | \alpha und → \underline{\alpha} | commented out (snippets.js line 254) |
| `/par([0-9])/` | par2 → ∂²y/∂x² (n-th partial derivative) | not in your file |
| `/parn/` | parn → ∂ⁿy/∂xⁿ | not in your file |
| `/\\[A-Za-z]{2,}/` | while typing a \command, stop other snippets (and add a space after a finished one: \tox → \to x) | not in your file |
| `/(?<positive_lookbehind>(?:\n|^)[ \t]*>*)(?<marker>\d+[.)]|[-*+])(?<whitespace>[ \t]+)(?<text>.*)dm/` | Latex Suite's dm in a list (yours replaced it) | not in your file |
| `/(\n|^)[ \t]*(?:> ?)*[ \t]*dm/` | Latex Suite's dm on an empty line, also in > quotes (yours replaced it) | not in your file |
| `--` | -- → – (en dash) in text | commented out (your own) (snippets.js line 52) |
| `–-` | –- → — (em dash) in text | commented out (your own) (snippets.js line 53) |
| `—-` | —- → --- in text | commented out (your own) (snippets.js line 54) |
| `del` | del → \nabla | commented out (your own) (snippets.js line 203) |
| `e\xi sts` | e\xi sts → \exists | commented out (your own) (snippets.js line 226) |
| `\\(${GREEK}|${SYMBOL}|${MORE_SYMBOLS})([A-Za-z])` | a space after \alpha, \in, … when a letter follows | commented out (your own) (snippets.js line 245) |
| `/([^'])\b([B-HJ-Zb-z])\b([\n\s.,?!:'])/` | a lone letter in text becomes maths: x → $x$ | commented out (your own) (snippets.js line 371) |
| `(${GREEK})([\n\s.,?!:'])` | a Greek name in text becomes maths: alpha → $\alpha$ | commented out (your own) (snippets.js line 374) |
| `/([A-Za-z]=\d+)([\n\s.,?!:'])/` | x=2 in text becomes $x=2$ | commented out (your own) (snippets.js line 377) |
| `/([A-Za-z]=[A-Za-z][+-]\d+)([\n\s.,?!:'])/` | x=n+1 in text becomes $x=n+1$ | commented out (your own) (snippets.js line 378) |

Each was typed into both setups: off in both (or one of your snippets answers, the same way).
