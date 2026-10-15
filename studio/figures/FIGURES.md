# Ntex Figures: the spec

Diagrams for the author's notes: commutative diagrams, Cayley graphs (often
shorthand, "a visual while working"), blob diagrams of functions,
projections and kernels, and dihedral pictures. She draws with a trackpad, so
**everything is a typed object, never a freehand stroke**. Exactness comes free
from that; "loose" means rough input is cleaned up into objects.

A figure is a small text file, `figures/<name>.fig`, next to the `.tex`. It
compiles to TikZ with her theme (Charter, her palette) into
`figures/<name>.pdf`, and the `.tex` just does `\includegraphics{figures/<name>}`.
Three ways in, one file:

1. **Shorthand.** Type it in the window, or in the `.fig` file in Neovim, where
   saving rebuilds the PDF.
2. **Canvas.** Click, drag and roughly draw; input is snapped and corrected
   into objects.
3. **Ask.** A prompt box where Claude, or GPT-6-Luna in fast mode, rewrites
   the file.

## Files and crates

| Path | Owner | What |
| --- | --- | --- |
| `studio/figures/` (crate `ntex-figures`, lib + bin `ntex-fig`) | FIG-CORE-A, FIG-CORE-B | model, parser, formatter, layout → Scene, TikZ, edits, recogniser, tidy, CLI |
| `templates/tex/ntex-base.sty` (new), `ntex-figure.sty` (new), `ntex-notes.sty` (now requires ntex-base) | FIG-CORE-B | one source of truth for fonts and palette |
| `studio/src/ui/figure/` + wiring in `src/ui/app.rs`, `src/main.rs` | FIG-UI | the figure window |
| `studio/io/src/figure_ai.rs`, `studio/figures/eval/` | FIG-AI | the prompt box backend, and the model evaluation |
| `lua/config/figures.lua`, `.fig` filetype/syntax, `tests/` | FIG-NVIM | Space g, rebuild on save, recompile the .tex when a figure changes |
| `studio/figures/tests/adversary_*.rs`, `studio/figures/verification/` | ADVERSARY | independent acceptance |

`studio/figures/src/model.rs` is shared and **add-only**, like the studio's
model.rs.

## The shorthand (`.fig`)

Line-oriented. `%` starts a comment, to the end of the line, as in LaTeX.
Statements are one per line; `;` also separates statements. Blank lines are
fine. Keywords are lower case. Everything is forgiving: an unknown option is a
warning, not an error, and anything that can be guessed is guessed.

### Names and labels

- A **node** is named by a *token*: a run of characters that isn't whitespace,
  isn't an arrow, and isn't one of `,;{}"`. Examples: `A`, `G`, `G/K`, `Z_n`,
  `H'`, `\ker\varphi`, `\mathbb{Z}`, `φ(G)`. Braces may appear inside a token
  when balanced (`\mathbb{Z}`, `G^{ab}`).
- Its **label** is the token itself, set as LaTeX maths, unless a quoted label
  follows its first mention: `A "\mathbb{Z}/n\mathbb{Z}"`.
- **Unicode input** is accepted everywhere and turned into LaTeX using a fixed
  table (`core::unicode`): Greek letters (φ → `\varphi`, ε → `\varepsilon`,
  …), double-struck letters ℤ ℚ ℝ ℂ ℕ → `\mathbb{…}`, `∘ × ⊗ ⊕ ≅ ⊂ ⊆ ∈ ∉ ∩ ∪
  ∅ ∞ ≤ ≥ ≠ ↦ →`, and sub/superscript digits (x₁ → `x_{1}`, g² → `g^{2}`).
- First mention wins for a node's label; later mentions just refer to it.
- **Your spelling is kept.** The figure stores LaTeX, but whenever the canvas,
  an edit or ⌘T rewrites the file, a label comes back exactly as you typed it:
  `φ` stays `φ`, `\varphi` stays `\varphi`, `ℤ2 × ℤ4` stays `ℤ2 × ℤ4`. A label
  you change in the window is written as you typed the new one.

### Arrows: `FROM <arrow> TO [options]`

| Write | Means | TikZ (cd arrow styles) |
| --- | --- | --- |
| `A -> B` | arrow | `->` |
| `A -f-> B`, `A -"g \circ f"-> B` | labelled (quotes when the label has spaces or dashes) | label `$f$` |
| `A ->> B`, `A -π->> B` | surjection (two heads) | `two heads` |
| `A >-> B`, `A >-ι-> B` | injection (tail) | `tail` |
| `A hook-> B`, `A -ι-hook-> B` | inclusion (hook) | `hook` |
| `A => B`, `A =f=> B` | double | `Rightarrow` |
| `A --> B`, `A --f--> B` | dashed (∃) | `dashed` |
| `A ..> B`, `A .f.> B` | dotted | `dotted` |
| `B <-f- A` | reversed (same as `A -f-> B`) | |
| `A <-> B`, `A <-f-> B` | both ways | `<->` |
| `A = B` | equals (two parallel lines, no head) | `equal` |
| `g |-> h`, `g ↦ h` | maps-to, element to element | `|->` |

Options after `TO`, in any order:
- **shape:** `bend` / `bend-left` / `bend-right` (30°), `bend=40`, `loop`
  (a self-arrow)
- **label:** `swap` (label on the other side), `iso` (adds `≅`: as the label,
  or, when there is one, on the opposite side of the arrow from it)
- **line:** `dashed`, `dotted`, `thick`, `color=<palette name>`
- **position:** `near-start`, `near-end` (label position)

Palette names are `blue violet teal maroon green ochre grey ink` (her
ntex-notes tones). Arrows between blobs are drawn blob to blob, as a big arc
above them; arrows between elements of blobs are drawn point to point.

Two straight arrows between the same two objects (`A -f-> B` and `B -g-> A`)
are drawn side by side, as tikz-cd's `shift left` does: the first one written
on top (or on the left). Several straight arrows into one element's dot arrive
spread around it, so two maps-to heads never stack into what looks like `->>`.

### Placement

- `A at 1,0`: grid cell (column 1, row 0). Rows go down. Cells are 1.9 cm ×
  1.35 cm by default, about what tikz-cd's own spacing gives her nodes (a
  tikz-cd square of single letters measures 1.4 × 1.1 cm between centres, one
  of `G`, `G/K`, `φ(G)` 1.9 × 1.2 cm); `spacing 2.4,1.8` gives more room.
  Fractions are fine: `A at 0.5,1`.
- `A at (3.2,-1)` with parentheses: absolute centimetres, y up.
- Nodes without a position are **auto-placed**:
  - If some nodes are placed, the rest go to the nearest free cells by the
    arrows.
  - Otherwise, a layered layout: a 3-cycle becomes a triangle, a 4-cycle a
    square, chains go left to right, and a branching becomes a fan.
  - Always on integer cells, so the result looks like a hand-made `tikz-cd`.

### Grid blocks (ASCII art)

```
grid
  A --f--> B
  |g       |h
  v        v
  C --k--> D
end
```

- Rows that contain node tokens are node rows. A node's column is found by
  clustering the starting character columns of tokens across all node rows,
  so `A` and `C` share column 0 because they line up.
- **Horizontal arrows** between neighbours in a row:
  - `--f-->` and `-->` are solid, `<--f--` is reversed, `-->>` is a surjection
    and `>-->` an injection.
  - `- - >` is dashed and `..>` dotted.
  - The label is between the dashes.
- **Vertical arrows** live on the lines between two node rows:
  - A column of `|` with a label written straight after it (`|g`), ending in
    `v` (down) or starting with `^` (up).
  - It belongs to the nodes above and below whose token span covers it, or
    is nearest within 2 columns.
  - `‖` or `||` is an equals sign; `:` instead of `|` is dashed.
- **Diagonals:** `\label` (down-right) or `/label` (down-left) on a between
  line connects the nearest nodes in that direction. Diagonals the art can't
  show are written as ordinary arrow statements after `end`, and can
  reference the grid's nodes.
- **A label stays where the art writes it:** above a horizontal arrow (also
  a reversed `<--f--` one), right of a `|g` arrow whether it points down or
  up, left of a `g|` one. (tikz-cd's automatic side would put it below a
  leftward arrow and left of an upward one, so those are stored as `swap`.)
- A grid block gives its nodes integer cells; statements outside the block
  may add arrows and options.

### Blobs (sets, functions, kernels, projections)

```
blob G "G" {e, g, g^2, g^3}
blob K "\ker\varphi" in G shaded {e, g^2}
blob H "H" {e', h}
G -φ-> H
g |-> h ; e |-> e'
```

- `blob NAME ["label"] [{elements}] [in PARENT] [options]`
  - Elements are points: a dot plus a label. An element listed in a child blob
    is also in its parent; don't list it twice.
  - Element names are tokens like nodes, and must be unique in the figure.
- **Options:**
  - `shaded`: a tint of the blob's colour
  - `color=…`
  - `shape=round|tall|wide|bean`
  - `size=w,h` (cm)
  - `at …`
  - `label=above|below|left|right|inside`
- **Layout without positions:** top-level blobs go in a row, left to right,
  in order. Children sit inside their parent and the parent grows to fit.
  Points are spread evenly inside, avoiding child blobs.
- **Shape:** a smooth closed curve through 6 to 8 jittered points around an
  ellipse (deterministic, seeded by the name). So each blob looks hand-drawn,
  but the same blob always looks the same.

### N-gons (dihedral groups)

```
ngon P 4                          % a square on its bottom edge, 1 top-left, then counter-clockwise
ngon P 5 labels a,b,c,d,e axes rotation
ngon Q 6 axis 1 axis 2 rotation=2   % two axes, rotation arrow by 2 steps
```

- **How it sits:** as textbooks draw D4. An even-sided polygon stands on a
  flat bottom edge and vertex 1 is its **top-left** vertex; an odd-sided one
  stands on a flat bottom edge too, with vertex 1 at the **top**. Numbering
  continues counter-clockwise.

- `labels …` (default `1..n`) and `axes` (every reflection axis, dashed).
- `axis k`: the axis through vertex k, or through the midpoint of edge k if n
  is even and k is written `k.5`.
- `rotation[=k]`: an arc arrow labelled `r` or `r^k`. Clockwise (labelled
  `r^{-k}`): `rotation cw`, or `rotation=-k`. The edge it sits on:
  `rotation edge=3` (edge k joins vertices k and k+1, counter-clockwise; with
  `cw`: `rotation cw edge=3`; `edge 3` also reads). The formatter writes
  `rotation cw` for one step, `rotation=-k` for more. A quoted label names
  it: `rotation "ρ"`, `rotation=2 "ρ^2"`, `rotation cw edge=3 "ρ"`.
- `axis k "s"`: the axis through vertex k with a label `s` at its end (outside
  the vertex labels). Several axes: `axis 1 "s" axis 2 "sr"`.
- `turn=k`: draw the polygon after k rotations, so labels move; `turn=-k`
  turns the other way (the same as `turn=n-k`, and written back as `-k`).
  `flip=k`
  reflects it across axis k. Several `ngon`s in a row show a sequence.
- `at …`, `radius=r` (default 1.1 cm, alone or in a row; labels at the
  normal size).
- The rotation arrow sits outside an upper-right edge, between two vertex
  labels (over the top edge for a square).

### Cayley graphs

```
cayley D4 <r, s>                  % known groups
cayley <a, b | a^4, b^2, (ab)^2>  % from a presentation (coset enumeration, ≤ 96 elements)
cayley Z6 <1>                     % additive
cayley D4 <r, s> gens r^-1:maroon,s:hidden   % per generator: ^-1 draws g⁻¹, :colour, :hidden
```

- **Known groups:** `Cn`/`Zn`, `Dn` (order 2n), `V4`/`Z2xZ2`, `Q8`, `S3`,
  `S4`, `A4`, `Zm x Zn`. A product may be spaced and in Unicode:
  `cayley Z2 x Z4`, `cayley ℤ2 × ℤ4`.
- **Generators** may be left out: the group's standard ones are used (`r, s`
  for Dn, `a, b` for a product, `1` for Zn, `i, j` for Q8).
- **Layout:**
  - Automatic: a cycle for cyclic groups, two concentric n-gons (a prism) for
    Dn, a cube or grid for products, a circle otherwise. Two special cases
    keep the picture honest and readable: Q8 `<i, j>` is two concentric
    squares of i-edges with the j-edges straight between them (4 crossings,
    the least possible); A4 `<a, b>` is the truncated tetrahedron, drawn flat
    with e's triangle in the middle and no crossings.
  - Or `layout=circle|prism|grid`.
- **Edges:** each generator gets a colour (blue, maroon, teal, green, in
  order). An involution's edge is drawn once, with no head. A legend is drawn
  if `legend` is given.
- **Vertex names:** `vertex r^2s "x"` gives that vertex (named by its word)
  her own name; repeat for more (`cayley D4 <r, s> vertex r "ρ" vertex s "σ"`).
  Empty or the word itself removes it. Edited in place or in the panel's Name
  field.
- **Vertices:** labelled by shortlex words (`e, a, b, a^2, ab, …`), or
  `labels=none` for dots. **Dn** uses its normal form instead, as her course
  does: `e, r, …, r^{n-1}, s, rs, …, r^{n-1}s` (so `r^5`, not `srs`, in D6).
- **A group that can't be drawn** (unknown, more than 96 elements, infinite
  like `<a, b | a^2, b^2>`) is a diagnostic in the log; nothing about it is
  typeset into the figure.

### Free text and globals

- `text "kernel ↦ identity" at (x,y)`
- `scale 1.2`, `spacing 2.8,2`, `size small|normal|large` (label size).
  A rewrite keeps each global setting where you wrote it.

### Error handling

A line that can't be parsed is reported as `line N: …`, and the rest of the
file still renders. The window and the CLI both show these messages. A figure
never fails to build because of one bad line. An empty figure (nothing to
draw) builds a placeholder PDF, a dashed box saying "empty figure — NAME", so
the `\includegraphics` in the notes still compiles.

Messages come in three kinds: errors, warnings and notes. Notes (like "`A` is
new here, so it became a node.") come on almost every line, so the window's
log, Neovim's diagnostics and `ntex-fig` hide them; `ntex-fig -v` shows them.

## Output

`ntex-fig build figures/x.fig` writes `figures/x.pdf` with tectonic:
- The document is `\documentclass[border=3pt]{standalone}` with
  `\usepackage{ntex-figure}`.
- `ntex-figure.sty` loads `ntex-base.sty` (fonts and palette: the lines moved
  out of ntex-notes.sty, which now requires ntex-base, so Charter changes
  follow everywhere), TikZ, and `\usetikzlibrary{cd,arrows.meta,calc,hobby}`.
- It's built in a temp dir and renamed into place, so a broken build never
  replaces a good PDF.

`ntex-fig check x.fig` prints the diagnostics (`-v` adds the notes).
`ntex-fig tikz x.fig` prints the TikZ, and `--portable` inlines the colours
and styles so you can paste it into a `.tex` for someone without the theme.
`ntex-fig fmt x.fig` writes the canonical form. `ntex-fig new x.fig` writes
the starting file (`figures/template.fig`: two comment lines naming the
figure and the reference, `build::template_for`) if there is no file yet. The
cheat sheet is `figures/reference.md` (`build::REFERENCE`, ⌘/ in the window).

Everything is placed in absolute coordinates computed by `layout` (the
**Scene**), so the canvas and the PDF agree by construction:
- nodes are TikZ nodes at known points, and arrows go between named nodes
- arrow tips and line styles come from `tikz-cd`'s styles (`\usetikzlibrary{cd}`),
  so commutative diagrams look like every algebra textbook

## Rules

Studio rules apply. The same session rules hold:
- **Headless.** No window may appear on screen. GPUI checks use
  HeadlessAppContext offscreen; never run the binary, except in
  `NTEX_STUDIO_QUIET=1` smoke mode.
- **Temp copies only.** Never write her real files in tests.
- **No orphan processes.**
- **Don't commit.**

## The figure window (FIG-UI)

Ntex Studio's process opens one window per `.fig`; asking for the same path
again focuses its window. It has the same LaTeX-set look as the studio
(studio/UX.md "The look"): paper, Latin Modern chrome, `\fbox` buttons,
footnote hints, and the theme's palette.

```
Ntex Figures                                    kernel.fig  ·  saved 14:02
───────────────────────────────────────────────────────────────────────────
[V select] [N node] [A arrow] [B blob] [P polygon] [R rough]  [⌘T tidy]
┌───────────────────────────────── canvas ─────────────────────────────────┐
│  (the figure: the exact PDF render when idle; a native draft while you   │
│   drag; selection handles, cell grid dots in the select/node tools)      │
└──────────────────────────────────────────────────────────────────────────┘
┃ Proof (Shorthand).                                  (the .fig, editable, mono)
┃   grid / A --f--> B / … end                         errors as LaTeX log lines
Ask  ________________________________________  [Sonnet ▾] ☐ fast  ⌘↵
───────────
¹ N click: a node · A drag node→node · R draw roughly: loops → blobs, corners → polygons
```

- **Canvas.** Scene coordinates are scaled to fit, with zoom on ⌘= / ⌘- and
  pinch, and pan by two-finger scroll.
  - On opening (and on a reload from disk) the figure is fitted to the
    canvas, at most 200 %, so a small figure isn't lost in the middle. An
    empty figure is fitted when its first statement arrives. ⌘0 fits again.
  - When idle it shows the exact render (tectonic PNG via the studio's
    Renderer, at 2×).
  - While you drag or type, it switches to a native draft: Scene paths drawn
    with GPUI paths, and labels as `unicode::preview` text in Charter. Fonts:
    bundle XCharter from her tectonic bundle, as WIDGETS did for Latin Modern.
  - The draft and the render share coordinates, so the swap doesn't jump.
- **Tools** (keys V N A B P R when the canvas is focused):
  - **select:** click, ⇧-click, drag to move (snapping to cells for nodes; see
    "Moving, copying, pasting" below for ⌥ ⇧ ⌘ during a drag).
    Edit a label in place with a click on the label of an already selected
    object (Keynote's rule), a double-click, or ↵. The field sits beside the
    label (above it; across the line for an arrow) in a proportional face, and
    the canvas keeps drawing the label as you type. **Maths is the default**:
    the field shows no `$…$` (your snippets still fire: `ZZ`, `//`, Tab stops),
    and a small `text` chip beside it switches that label to text mode
    (`\text{…}`; `mk` makes a maths island inside). Unicode is accepted and
    her spelling is kept (φ stays φ). Tab / ⇧Tab walk every label in reading
    order (top to bottom, then left to right) across nodes, arrows, blobs,
    elements, vertices and the rotation arrow's and axes' labels; ↵ commits,
    Esc reverts. A click on an arrow's label selects the arrow; dragging the
    label across swaps its side; a click alone starts no drag.
  - **node:** a click places a node on the nearest cell and focuses its label.
  - **arrow:** drag from one object to another; the label field follows.
  - **blob:** drag a rectangle to get a blob of that size.
  - **polygon:** a click places a square; the digits 3–9 set n.
  - **rough:** draw freely; `recognize` turns it into a node, arrow, blob
    or n-gon. A ghost of the result flashes for 150 ms, so you see what it
    became.
  - Delete removes the selection. ⌘Z / ⌘⇧Z undo the figure's edits, as
    snapshots of the source.
- **Shorthand box.**
  - A two-way mirror: canvas edits rewrite it with `format`, and typing in it
    re-parses after 150 ms, with the canvas following.
  - Errors and warnings appear under it as TeX log lines; notes are hidden.
  - It uses the studio's TextField in multi-line mono mode.
- **Ask box.**
  - Return sends the instruction (with the current source) to the selected
    model.
  - While it runs: *asking Sonnet…* with a Cancel button.
  - The result replaces the source as one undo step, and the toast reads
    *Done in 3.1 s — ⌘Z undoes.*
  - The model menu lists the evaluated models (`figure_ai::MENU_MODELS`:
    Sonnet, Haiku, Luna, Sol). Its default is prefs.lua's `figure_model`
    (claude-sonnet-5-5) at `figure_effort` (low).
  - The **☐ fast** checkbox (a plain box in the palette, no emoji) switches to
    `figure_fast_model` (gpt-6-luna) at `figure_fast_effort` (low); it starts
    ticked when `figure_fast = true`. The defaults come from
    `figures/eval/REPORT.md`.
- **Saving.**
  - ⌘S writes the `.fig` and builds the PDF; the window stays open.
  - ⌘W saves and closes.
  - Saving is also automatic 1 s after an edit, so ntex's PDF follows along.
  - If the file changes on disk (edited in Neovim), the window reloads it,
    unless it has unsaved edits, in which case it asks.
  - Builds run tectonic in the background with a cancel flag; closing the
    window or quitting the studio kills and reaps it, so no tectonic outlives
    the studio.
- **Opening:** `ntex-studio --figure /abs/path/x.fig`, or OpenRequest.figure
  over the socket. A missing file is created from the starting file
  (`figures/template.fig`, the same one `ntex-fig new` writes): a commented
  cheat sheet, plus one example for each kind, commented out.

## In ntex (FIG-NVIM)

- **Space g** in normal mode, in .tex and .md:
  - **Cursor on a line with `\includegraphics[...]{…figures/NAME…}`:** opens
    that `.fig` (path resolved against the .tex's folder). Subfolders work:
    `figures/sub/deep` opens `figures/sub/deep.fig`.
  - **Otherwise:** asks for a name (default `fig-N`, the next free number),
    creates `figures/NAME.fig` with `ntex-fig new` (or, without the binary,
    the same template built in), and inserts this at the cursor, on its own
    lines (`\centering` + `\includegraphics` inside a `center`, no float, as
    notes want):
    ```
    \begin{center}
      \includegraphics{figures/NAME}
    \end{center}
    ```
  - It then launches the window, and builds the PDF if it's missing.
- **`.fig` files open in ntex** with filetype `ntexfig`: syntax highlighting
  for keywords, arrows, quoted labels, comments and grid blocks. Saving runs
  `ntex-fig build` in the background and shows errors and warnings as Neovim
  diagnostics (notes are hidden).
- **A figure's PDF changing** (window or CLI) triggers a recompile of any
  open `.tex` whose source mentions it, so the live PDF follows. A debounced
  fs_event watch on each open `.tex`'s `figures/` folder does this.
- **Guards:** `TEXNVIM_NO_VIEWER` stops it launching the window in tests,
  just as the studio does.

## Direct manipulation (interaction pass, 2026-10-12)

The author's first hands-on test, verbatim: *"there's seemingly no way for me to
rotate these shapes or remove labels or change the labels or even resize
them? … the human tooling is not where it should be."*

**The rule from now on:** anything the shorthand can say, the mouse and
keyboard can do, with no typing of shorthand required. The text box is a
mirror, not the only way in. Measure against Keynote or Figma for the basic
object handling (select, handles, inspector, right-click menu, nudge).

### New shorthand options (FIG-MODEL)

- **`angle=θ`** on `ngon`, `blob` and `cayley`: rotates the drawing θ degrees
  counter-clockwise. On n-gons the labels rotate with it, unlike `turn`, which
  is the group action and relabels.
- **`labels=none`** on `ngon` hides the vertex labels; they're remembered.
  `labels 1,,3,4`, with an empty entry, hides one.
- **`plain`** on `ngon`: no tinted fill. `color=…` on `ngon` too.
- **Edits:**
  - `SetItem { index, item }` replaces any statement, keeping her spelling
    wherever a label didn't change.
  - `MoveObj { obj, pos }` moves anything that has a position.
  - `Duplicate { obj }` copies an object next to itself, with fresh names.
  - (figure-qol A) `clip::copy_objects` clones any set of objects, shifted,
    with the arrows between them; ⌥-drag, ⌘D, ⌘C and ⌘V all use it.

### Selection

- **Click** selects the object under the pointer. Hit-testing works at every
  level: node, arrow, blob, element, n-gon (the shape), n-gon vertex label,
  Cayley graph (the whole graph; a vertex label inside is selected by a second
  click), and text.
- **⇧-click** adds to the selection. Dragging on empty canvas draws a marquee.
  **⌘A** selects all; **Esc** deselects.
- **Selection look:** a 1.5 px theme-blue outline around the object's bounds
  (a turned n-gon, blob or Cayley graph gets a box that turns with it),
  plus its handles. Hovering shows a faint outline, so she sees what a click
  will hit.

### Moving, copying, pasting (figure-qol A)

One grammar for every drag of the selection:

| Modifier | During a drag |
| --- | --- |
| (none) | the **whole selection** moves together, each object snapping as it would alone: nodes to cells, shapes to half cells, blobs, texts and elements to a millimetre. A blob takes its placed elements and the blobs inside it along. |
| **⌥** (held as the drag starts) | the originals stay and **copies** are dragged (the cursor shows a copy badge and the readout says "copy"). The arrows *between* the copied objects come along, renamed. The whole drag is one undo step; Esc gives the originals back. A copy dropped where it started is no copy. |
| **⇧** | the move is **locked to horizontal or vertical**, whichever has travelled further. |
| **⌘** | **snapping is off** (positions to a millimetre). This is also what ⇧ meant for turning (free angle) and for the node and polygon tools' click (⌘-click puts the node exactly there). |

⌥ no longer means half-cell snapping, and ⇧ no longer means free placement
for shapes; ⌥ in a blob's *resize* drag still resizes from the centre.

- **⌘C / ⌘X** put the selected objects on the system clipboard as `.fig`
  shorthand (names, positions, looks, and the arrows between them), so they
  paste into another figure window or into Neovim. **⌘V** pastes: what this
  window copied lands half a cell right and down, each further paste another
  half cell; shorthand from elsewhere (or a cut pasted back) lands where it
  was written, then steps the same way. Names are kept where they are free,
  fresh otherwise; the pasted objects become the selection. Text that is not
  figure shorthand (it has warnings or errors, or holds no objects) is
  refused with a note, and nothing changes.
- **⌘D** duplicates the selection, carrying the arrows between the
  duplicated objects, offset by the **last ⌥-drag's offset**; with none yet,
  one cell to the right (further if a node is in the way).
- **Distribute** (three or more selected): equal gaps horizontally or
  vertically, the outermost two staying put. Panel: Distribute ▸ across /
  down, next to Align; menu: Distribute horizontally / vertically.

### Handles

All drags snap: angles to 15° (⌘ turns snapping off), sizes to 0.1 cm, and
positions to cells for nodes and half cells for shapes. Each drag is one
undo step.

| Object | Handles |
| --- | --- |
| n-gon | **resize:** a dot on the circumcircle sets the radius. **rotate:** a ↻ knob above the shape sets `angle`. Live readouts follow the pointer ("1.4 cm", "45°"). |
| blob | 8 resize handles set `size` (⌥ resizes from the centre). A rotate knob sets `angle`. |
| Cayley graph | resize sets the radius; rotate sets `angle`. |
| arrow | a midpoint handle sets `bend` (drag sideways), snapping to 0, ±30 and ±45. Its label handle switches to `swap` when dragged across the arrow. |
| node, element, text | drag to move. |

### The properties panel (inspector)

It sits in a margin column on the canvas's right, about 260 px wide; it
collapses below 1000 px window width and ⌘I toggles it. It's set as a
`Definition (Properties).` box in the studio's look, and only shows fields
that apply to the selection. **Every change applies live, as one undo step.**

| Selection | Fields |
| --- | --- |
| n-gon | Sides (stepper 3–12) · Size (field + slider) · Angle (field, ⟲ ⟳ 15° buttons, "point up" / "flat bottom" buttons) · ☐ Labels + an editable list (one field per vertex) · ☐ All axes, or a chip per axis · ☐ Rotation arrow (↺ ↻, steps) · Turn k (⟳ ⟲; negative allowed) / Flip k (the group action) · ☐ Fill · Colour swatches |
| node | Label (with live preview) · Colour · Position (col, row) |
| rotation arrow | Turns (↺ ↻, − k +) · Edge (◂ k ▸; or drag the arrow round the polygon) · Remove |
| arrow | Label · Ends (A → B and a ⇄ Reverse button; ⌘R) · Head (→ ↠ ↣ ↪ ↦ ⇒ = ↔ as a row of glyph buttons) · Line (solid / dashed / dotted) · Bend (slider) · ☐ Swap side · ☐ ≅ · ☐ Thick · Colour |
| blob | Label · Label place · ☐ Shaded · Colour · Shape (round / tall / wide / bean) · Size · Angle · Elements (an editable list: add, rename, remove) |
| element | Label · its blob |
| Cayley graph | Group (field) · Generators · Edges: a row per generator (colour swatches, g / g⁻¹, ☐ shown) · Layout · ☐ Labels · ☐ Legend · Size · Angle |
| text | Text |
| several | Colour · Align (left, centre, top, middle) · Distribute (across, down; three or more) · Delete |
| nothing | the figure: Scale · Spacing · Label size |

### Right-click menu, and keys in select mode

- **Right-click** (or ⌃-click): Edit label · Duplicate · Copy · Cut · Paste · Reverse (arrows, ⌘R) · Rotate 90° ·
  Distribute horizontally / vertically (several) · Hide/Show labels · Colour ▸ · Bring forward / Send back · Delete.
- **Keys:**
  - **Arrow keys:** nudge by one cell (nodes) or 0.1 cm (shapes); ⌥ for a
    fine nudge.
  - **Return:** edit the label (vertex labels too). **⌫:** delete.
  - **⌘R:** reverse the selected arrows (ends swapped, label stays on its
    visual side, bend and near-start/near-end keep the same line); on a
    selected rotation arrow it runs it the other way.
  - **⌘D:** duplicate. **⌘C ⌘X ⌘V:** copy, cut, paste as shorthand. **[ / ]:** rotate by 15°. **− / =:** shrink or grow
    by 10 %.
  - **L:** labels on or off. **⌘I:** panel. **⌘/:** reference.

### Help instead of a cheat sheet in the file

- **The template:** new files start with just two comment lines (what the
  file is, and "⌘/ in the window shows the reference"), not 50 lines of
  comments.
- **⌘/ reference:** toggles a Reference panel with the cheat sheet, shown as
  a Remark box over the shorthand box.
- **Opening a file:** the shorthand box scrolls to the first line that isn't
  a comment.

### Direction and orientation (figure-qol D)

- **Arrow end dots:** a selected arrow shows a dot at each end. Drag one onto
  another node, element, vertex or Cayley vertex (it is outlined while it can
  land) to reattach that end; dropped on nothing, the arrow stays. One undo step.
- **The rotation arrow is a part of its own:** click it (the panel switches to
  its Turns / Edge fields), drag it round the polygon to slide it to the
  nearest edge (`rotation edge=k`), ⌫ removes it, ⌘R or ↺ ↻ turns it round.
  Its default label is `r^{k}` or, clockwise, `r^{-k}`.
- **Model:** `NGon.rotation_cw`, `rotation_edge`, `turn_neg`; `Cayley.gen_styles`
  (`GenStyle`: generator, colour, inverse, hidden); `ObjRef::NGonRotation`.
