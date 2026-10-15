# Your snippets

One file per topic in `tex/`, one snippet per line. They are ordinary
[LuaSnip](https://github.com/L3MON4D3/LuaSnip) snippets, in the style of
[Gilles Castel's set](https://github.com/gillescastel/latex-snippets), written
with a few short helpers so each file reads like a table. `.tex` and `.md`
files use the same snippets.

| File | What's in it |
| --- | --- |
| `tex/sets.lua` | ∪ ∩ ⊆ ∈ ∃ ∀ ⟹ … (`uu`, `nn`, `sub`, `inn`, `exx`, `fall`, `imp`, …) |
| `tex/greek.lua` | `phi` → \varphi, `lmd` → \lambda, `pi`, `zeta` |
| `tex/operations.lua` | `sr` `cb` `rd` `_` `sq` `//` `ee` `invs` `conj`, `exp` `log` `ln` `det` … |
| `tex/accents.lua` | `xhat` → \hat{x}, `hat`, `bar`, `dot`, `vec`, `v,.` → \mathbf{v} … |
| `tex/subscripts.lua` | `x2` → x_{2}, `\alpha3`, `x24` → x_{24}, `xjj`, `xp1` … |
| `tex/symbols.lua` | `ooo` ∞, `sum`, `lim`, `->`, `<=`, `xx` ×, `RR` ℝ … |
| `tex/calculus.lua` | `par`, `ddt`, `int`, `dint`, `oinf`, `tayl` … |
| `tex/trig.lua` | `sin` → \sin, then `\sinx` → \sin x … |
| `tex/environments.lua` | `beg`, `pmat`, `cases`, `align`, `iden3` … |
| `tex/brackets.lua` | `(` pairs, `avg`, `norm`, `ceil`, `lr(` … |
| `tex/physics.lua` | `kbt`, `dag`, `o+`, `ox`, `bra`, `ket` … |
| `tex/chemistry.lua` | `cee` → \ce{}, `pu`, `iso` … |
| `tex/text.lua` | `mk`, `dm`, `text`, `"`, and `table` / `fig` / … with Tab |
| `tex/visual.lua` | select, then `S` `C` `U` `(` `/` … |
| `tex/inbox.lua` | snippets made in Ntex Studio, waiting to be sorted |

`DECISIONS.md` lists every decision of the move from Latex Suite (2026-09-30);
the reference page (Space ? in ntex) shows the same, with rendered maths.

## How a line reads

```lua
  math("uu",       [[\cup]],                       "yours"),
  math("//",       [[\frac{<>}{<>}]],     "default"),
  math("dint",      [[\int_{<:0>}^{<:1>} <> \, d<:x> ]],                  "changed", { desc = "Definite integral" }),
```

These three are copied from `tex/sets.lua`, `tex/operations.lua` and
`tex/calculus.lua`. Read them as: typing `uu` in maths gives `\cup`, by itself.
`<>` is where the cursor goes; Tab jumps to the next `<>`, and after the last
one Tab leaves the snippet. `<:0>` is a tabstop that starts out holding `0`
(selected, so typing replaces it).

The output is in `[[double brackets]]` so backslashes stay single (`[[\cup]]`
is `\cup`). If the output itself contains `]]`, use `[=[ … ]=]` or a normal
`"…"` string with doubled backslashes.

The last field says where a snippet came from: `"yours"` (your own),
`"changed"` (a Latex Suite default you edited), `"default"` (Latex Suite's,
as shipped), `"castel"` (added from Castel's set), `"new"`. It is only a label
(the reference page shows it); anything works.

## The helpers

| Helper | Fires | Example |
| --- | --- | --- |
| `math(trig, out, origin)` | in maths, by itself, at the start of a word | `math("uu", [[\cup]], "yours")` |
| `math_tab(trig, out, origin)` | in maths, when you press Tab after it | `math_tab("par", …)` |
| `suffix(trig, out, origin)` | in maths, right after what it belongs to | `suffix("sr", [[^{2}]], …)`: `xsr` → `x^{2}` |
| `post(trig, out, origin)` | in maths, grabs the letter before it as `%1` | `post("hat", [[\hat{%1}]], …)`: `xhat` → `\hat{x}` |
| `pat(pattern, out, origin)` | in maths, a Lua pattern trigger; captures are `%1`, `%2` | `pat("(%a)(%d)", [[%1_{%2}]], …)`: `x2` → `x_{2}` |
| `env(trig, name, origin)` | in maths: `\begin{name}…\end{name}`, on separate lines in display maths, one line inline | `env("pmat", "pmatrix", "default")` |
| `text(trig, out, origin)` | in text (outside maths), by itself, at the start of a word | `text("mk", [[$<>$]], "changed")` |
| `vis(key, out, origin)` | select text in maths, type the key; `%V` is the selection | `vis("S", [[\sqrt{ %V }]], "changed")` |

In the output:

| Write | Means |
| --- | --- |
| `<>` | a tabstop |
| `<:text>` | a tabstop holding `text` |
| `<1>`, `<1:text>` | a numbered tabstop; the same number twice is one tabstop typed in two places (`\begin{<1>} … \end{<1>}`) |
| `<0>` | where the last Tab leaves you (default: the end) |
| `%1`, `%2` | text captured by `post` / `pat` |
| `%V` | the selection (`vis`) |
| `%%` | a literal `%` |
| `%<`, `%>` | a literal `<` or `>` where it would read as a tabstop: `%<%>` gives `<>` |

A trailing space in the output is dropped in inline maths (`o+` gives
`\oplus` in `$…$`, `\oplus ` in display maths); a tab stays a tab.

Options, as a table after the origin, all optional:
`where = "inline"` or `"display"` (maths only in that kind),
`line = "start"` / `"after text"` / `"list item"` (text snippets: what's on the
line before the trigger), `tab = true` (`pat` / `text`: needs Tab),
`ft = "tex"` (only in .tex files), `priority = 1` (higher wins when two
triggers match; `-1` loses), `check = function(line, trigger, captures) … end`
(an extra condition), `eg = "x2"` (the example the reference page types for a
pattern), `desc = "…"`.

The output can also be a function that returns it, for outputs that depend on
what was typed: see `identity` in `tex/environments.lua` (`iden3`) and the
three `dm` versions in `tex/text.lua`.

## When a snippet fires (and when it doesn't)

- **Maths or text.** `math`… only in maths: `$…$`, `\(…\)`, `\[…\]`,
  equation/align/… environments, `$$…$$` and ```` ```math ```` in .md. Never
  inside `\text{…}`, `\label{…}`, `\ref{…}`, `\begin{…}`, `\ce{…}`, `\pu{…}`,
  or a `%` comment. `text` only outside maths (and not in comments or `` `code` ``).
- **Start of a word.** A trigger that starts with a letter never fires right
  after a letter or a backslash: `\subset`, `\approx`, `\emptyset` type as
  written, and `A uu` gives `A \cup` while `Auu` stays `Auu`.
  (`fire_after_letters = true` in prefs.lua, `latex = { … }`, lets them fire
  after a letter again, but still never inside a `\command`.)
- **Suffixes** (`suffix`, `post`, the auto subscript) follow a letter on
  purpose, but never the last letter of a `\command`: `\hbar`, `\leq2` stay.

## Add a snippet

1. Space e in ntex (or `:SnippetEdit`), pick the file.
2. Copy a line that is close to what you want and change it, e.g.
   `math("QQ", [[\mathbb{Q}]], "yours"),`
3. Save. The file reloads at once; try it.

If the file has an error, ntex says so in one line, the previous version of
that file stays active, and Space r (`:SnippetReport`) shows the error. A new
file in `tex/` is picked up when saved too; it has to end with
`return h.group("Title", { … })` like the others.

## Test

```sh
~/.config/texnvim/tests/run.sh            # everything, headless, a few minutes
~/.config/texnvim/tests/run.sh mangling   # 234 commands typed as written stay as written
~/.config/texnvim/tests/run.sh mine      # your snippets in .md and .tex
```

The tests copy this folder first, so they never change it. A new snippet that
fires inside a command shows up in `mangling`; one that fires in ordinary
words shows up in `english`.

## Roll back

Everything before the move to LuaSnip is the git tag `pre-luasnip`:

```sh
git -C ~/.config/texnvim checkout pre-luasnip   # the old Latex Suite setup (reads ~/.config/latex-suite)
git -C ~/.config/texnvim checkout main          # back to this
```
