# Ntex Figures — usability, from the author's side

The usability adversary's file for the direct-manipulation pass (FIGURES.md,
"Direct manipulation"). Phase 1 was written before reading the new UI code: it
says what each task *should* take. Phase 2 (below the line, filled in after
FIG-INTERACT landed) is what the real window took, driven offscreen with
synthetic mouse and keys through `studio/tests/usability.rs`.

## How to read the tasks

She has a **trackpad**, which shapes every cost:

- A **click** is cheap. A **drag** costs more: she has to hold the pad, so a
  small or moving target is worse on a drag than on a click. A **two-finger
  click** (right-click) is cheap and natural on a Mac trackpad.
- A **key chord** is cheap once she knows it. It is never the only way to do
  something she would do once a week.
- **Typing a label** (`a`, `\ker\varphi`, `g^{-1}`) is fine: that's content.
  **Typing shorthand** (`angle=45`, `labels=none`, `->>`) is not: that's the
  thing the pass is meant to make unnecessary.

**Step counting.** One click, one drag, one key chord, or one typed label (the
whole string) each count as 1. Moving the pointer and hovering are free.

**Discoverable** means she would find it in the first minute of looking,
without the reference (⌘/) or this file, by one of:

- **H** a visible handle on the selected object (and its live readout);
- **P** a labelled field or button in the properties panel;
- **M** an entry in the right-click menu;
- **F** a footnote hint that appears for the current selection or tool;
- **V** hover feedback (outline, cursor, tooltip) that says what a click hits.

A task whose only route is a bare key (`L`, `[`, `=`) is **not**
discoverable unless a footnote or the menu names the key.

**Pass bar for phase 2.**

- **success**: done with mouse and keyboard only, in at most ideal + 1
  steps, through a discoverable route, and every intermediate frame reads
  sensibly;
- **friction**: done, but more than ideal + 1 steps, or only through an
  undiscoverable route, or with a bad intermediate frame;
- **fail**: impossible without typing shorthand or the Ask box, or the result
  is wrong (the wrong thing changed, undo doesn't restore it, the figure
  breaks).

Every task ends with **⌘Z**: one undo must put the source back exactly as it
was before the task's last gesture (spelling included), and the canvas must
follow.

## The 25 tasks

Starting figures are the shipped examples (`figures/examples/`) unless a task
says otherwise. "Her words" is how she'd say it, mid-lecture.

### N-gons (dihedral)

**N1. "draw D4 and show the reflection across the diagonal"** (empty file)

- Ideal (3): `P` (or click POLYGON) · click on the canvas → a square ·
  in the panel, click the axis chip **1** (the axis through vertex 1 and 3,
  the diagonal).
- Discoverable: P (tool button), P (panel lists axes by vertex, with a hover
  preview of each axis on the canvas). A square drawn by the polygon tool
  should already be selected so the panel is showing.
- Watch: does the click with the polygon tool leave the new square
  selected? Are axis chips labelled by what they go through ("1–3",
  "edge 1.5") rather than a bare index?

**N2. "make the square sit flat"** (07-d4-square rotated to `angle=30`)

- Ideal (2): click the square · click **flat bottom** in the panel.
  Alternative (2): right-click → *Rotate…*, or drag the ↻ knob to 0 (it
  snaps at 0, so it's one drag).
- Discoverable: P (a named button), H (the knob's "0°" readout).
- Watch: "flat bottom" must work for every n: odd n-gons too, where flat
  means the edge at the bottom, not angle 0.

**N3. "hide the vertex numbers"** (07-d4-square)

- Ideal (2): click the square · untick **Labels** in the panel. Or
  right-click → *Hide labels* (2), or `L` (2).
- Discoverable: P, M. `L` alone is not.
- Watch: ticking it back brings the old labels back (they're remembered);
  the axes and rotation arrow don't move when labels go.

**N4. "label the vertices a b c d"** (07-d4-square)

- Ideal (6): double-click the vertex label 1 · type `a` · Tab · `b` · Tab ·
  `c` · Tab · `d` · Return. Counted as: double-click, 4 typed labels,
  Return = 6. Or in the panel's label list, the same with Tab between
  fields.
- Discoverable: V (hovering a vertex label outlines it), F ("Tab: next
  vertex"), P (one field per vertex, in vertex order).
- Watch: does Tab walk vertex to vertex in numbering order? Does an empty
  field hide that one label (not make a `,,` that confuses the parser)?
  Does Esc cancel only the field she's in?

**N5. "copy this pentagon and turn it once"** (08-d5-pentagon)

- Ideal (3): click the pentagon · ⌘D (or right-click → *Duplicate*) · in
  the copy's panel, **Turn** +1.
- Discoverable: M (Duplicate), P (Turn, named as the group action, not to be
  confused with Angle).
- Watch: the copy lands beside the original, not on top of it and not off
  the visible canvas; the copy is what's selected afterwards; Turn relabels
  (1→2…), Angle doesn't. Do "Turn" and "Angle" say clearly which is which?

**N6. "make the hexagon a bit bigger and stand it on a vertex"** (`ngon P 6`)

- Ideal (2): click it · drag the radius dot outwards (readout "1.4 cm").
  Then 1 more: drag the ↻ knob 30° (readout "30°"). Total 3.
- Discoverable: H (both handles visible on selection, each with a cursor
  that says resize/rotate), P (Size slider, Angle field).
- Watch: radius dot and rotate knob at least 24 px hit targets at the
  default zoom; the knob isn't hidden under the vertex 1 label or the
  rotation arrow; the readout doesn't cover the shape's own labels.

**N7. "actually make it a pentagon, no fill"** (`ngon P 6`)

- Ideal (3): click it · Sides stepper − · untick **Fill**.
- Discoverable: P.
- Watch: changing n keeps her custom labels sensible (extend/trim, don't
  wipe); axes chosen by number that no longer exist are dropped, not errors.

### Cayley graphs

**C1. "rotate the cayley graph so e is on top"** (10-cayley-d4)

- Ideal (2): click the graph · drag the ↻ knob until e is at the top (the
  15° snap lands it). Better still (2): right-click e → *Put on top*.
- Discoverable: H (knob + "°" readout), M.
- Watch: does a click on the graph select the whole graph (not a vertex)?
  Do labels stay upright as it turns? Does the legend stay put?

**C2. "make the Cayley graph smaller"** (10-cayley-d4)

- Ideal (2): click · drag the resize dot inwards. Or `−` (2).
- Discoverable: H, P (Size).
- Watch: shrinking can't overlap vertex labels into an unreadable pile
  without warning; the legend doesn't shrink with it.

**C3. "hide the element names, show the legend"** (10-cayley-d4)

- Ideal (3): click · untick **Labels** · tick **Legend**.
- Discoverable: P, M (Hide labels).

**C4. "make it D5 instead"** (10-cayley-d4)

- Ideal (3): click · in **Group**, select the text and type `D5` · Return.
  (Typing a group name is content, like a label.)
- Discoverable: P.
- Watch: a bad group name ("D") shows a readable message by the field and
  doesn't wipe the graph.

**C5. "quick Cayley sketch of Z6"** (empty file)

- Ideal (≤4): there's no Cayley tool in the toolbar, so the ideal is a
  menu: right-click on empty canvas → *Insert ▸ Cayley graph* · click
  where · Group `Z6` · Return.
- Discoverable: M (or a toolbar entry). If the only routes are shorthand or
  Ask, this is a **fail** by the pass rule (anything the shorthand can say).

### Blobs (functions, kernels, projections)

**B1. "make the kernel blob bigger"** (06-kernel)

- Ideal (2): click K (inside G) · drag a corner handle out. ⌥ to grow from
  the centre.
- Discoverable: H, V (hover shows that the click hits K, not G).
- Watch: the first click inside K selects K, not G (innermost wins);
  G grows to keep K inside, or K is clamped, but K never pokes out of G;
  the elements e, k move with K.

**B2. "add an element to H"** (06-kernel)

- Ideal (3): click H · **+** in the Elements list · type `h'` + Return.
- Discoverable: P. (Better still: double-click on empty space inside a
  blob adds a point there; V/F would say so.)
- Watch: a duplicate name (`e'` again) is refused with a message, not a
  silently broken figure; the new dot lands inside H.

**B3. "rename g to g^{-1}"** (06-kernel)

- Ideal (3): double-click the label `g` · type `g^{-1}` · Return.
- Discoverable: V (hovering the element label outlines it), F.
- Watch: the maps-to arrow from g still connects (name vs. label: renaming
  the *label* must not break the arrow that refers to the *name*).

**B4. "send g to h"** (06-kernel, with `g |-> h` removed)

- Ideal (2): `A` (arrow tool) · drag from the dot g to the dot h. Element
  to element should default to ↦.
- Discoverable: tool button, V (the dot lights up as a drop target).
- Watch: small dots (≈ 4 px) as drag start points on a trackpad; the arrow
  shouldn't attach to the enclosing blob instead.

**B5. "unshade the kernel and make it maroon"** (06-kernel)

- Ideal (3): click K · untick **Shaded** · click the maroon swatch.
- Discoverable: P, M (Colour ▸).

**B6. "get rid of the label on G" / "put H's label inside"** (06-kernel)

- Ideal (3): click G · clear the Label field (select-all, ⌫) · Return.
  For H: click H · Label place → *inside* (2).
- Discoverable: P, M (Hide labels on a blob).
- Watch: an empty label is a legal state that round-trips (`blob G ""`),
  not a parse warning.

### Commutative diagrams

**D1. "bend this arrow so it doesn't cross"** (03-pullback, the `W -a-> X`
arrow straightened to cross P)

- Ideal (2): click the arrow · drag its midpoint handle sideways (snaps at
  ±30, ±45).
- Discoverable: H (handle at the midpoint on selection, with a readout),
  P (Bend slider).
- Watch: clicking a 0.6 px line on a trackpad: the hit band must be ≥ 8 px
  either side; the midpoint handle must not sit on top of the arrow's own
  label (which has its own handle).

**D2. "make that arrow a surjection"** (14-quick, the `G -φ-> H` arrow)

- Ideal (2): click the arrow · click **↠** in the panel's Head row.
- Discoverable: P (glyphs, which she reads instantly).
- Watch: the label φ is kept; the source keeps her spelling (`-φ->>`, not
  `-\varphi->>`).

**D3. "change f to φ" / "remove the label on f"** (02-square-grid)

- Ideal (3): double-click the label f · type `φ` · Return. To remove:
  double-click f · ⌫ · Return (3), or right-click the arrow →
  *Hide label* (2).
- Discoverable: V, F, M.
- Watch: removing the label inside a `grid` block rewrites the art
  sensibly; the arrow survives.

**D4. "put g's label on the other side"** (02-square-grid)

- Ideal (2): drag the label across the arrow (it flips to `swap`). Or
  click the arrow · tick **Swap side** (2).
- Discoverable: H, P.

**D5. "turn this sketch into a pullback square"** (start: `X -f-> Z ; Y -g-> Z`
auto-placed, a quick sketch)

- Ideal (≈10): N, click top-left cell, type `P` label (3) · A, drag P→X,
  drag P→Y (3) · N, click above-left, type `W` (2… shared tool switch
  counted once) · A, drag W→P · click the new arrow, **dashed** in the panel,
  label `\exists!` (3). Roughly 11–12.
- Discoverable: tool buttons, P (Line: dashed).
- Watch: the auto-placed X, Y, Z don't jump when P is added (placing one
  node pins the others where they were drawn); the arrow tool stays active
  between the two drags (no tool re-pick); new arrows default to the
  right head.

**D6. "delete this square"** (two figures: 07-d4-square, then 02-square-grid)

- n-gon, ideal (2): click · ⌫. Or right-click → *Delete*.
- The commutative square, ideal (2): marquee-drag around it · ⌫. Or ⌘A ·
  ⌫ (2).
- Discoverable: M, F (⌫ named in the footnote).
- Watch: deleting a node deletes its arrows (no dangling arrow error);
  deleting from a `grid` block leaves a parseable block.

### Everywhere

**G1. "undo that"** — after each task above.

- Ideal (1): ⌘Z. One drag = one undo step, one panel change = one step,
  a whole label edit = one step (not one per character), and the panel
  shows the restored value at once.
- Watch: undo after a slider drag doesn't step back one tick at a time;
  undo doesn't leave the selection pointing at a deleted thing; ⌘⇧Z
  redoes.

## Things that would annoy her whatever the task (checked on every frame)

- A hit target under 24 × 24 px at the default zoom (handles, chips,
  steppers, checkboxes, swatches).
- A handle that sits under a label or another handle.
- The wrong thing selected by a click (G instead of K, the graph instead of
  the vertex label on a second click, a blob instead of its element).
- State that silently changes something else (rotating relabels; resizing
  moves; a panel edit changes another object in a multi-selection).
- Layout jumps: the canvas re-fits or the panel changes height mid-gesture;
  the exact render swapping in at a different place from the draft.
- Panel fields that are unreadable: clipped labels, raw LaTeX where a
  preview is promised, a field too narrow for `\ker\varphi`.
- Breaking the LaTeX look: non-theme colours, sans-serif in the panel,
  emoji, rounded web-style controls.
- Things reachable only by typing shorthand.

## The bar, from the comparables

What Keynote and Figma do for the same gestures. Where Ntex Figures does
less, that is friction even when the task is possible.

| Gesture | Keynote / Figma | What that implies here |
| --- | --- | --- |
| select | click hits the visible ink with a few px of slop; a click inside a group goes to the group first, a double-click goes in | thin arrows need a wide hit band; blob-in-blob needs a rule she can predict |
| resize | 8 square handles ≈ 8 px drawn, ≈ 20–24 px hit area; ⇧ keeps the ratio, ⌥ from centre | the n-gon radius dot and blob handles need that hit area even if drawn small |
| rotate | Figma: hover just outside a corner turns the cursor into ↻; Keynote: ⌘-drag a handle. Live "45°" badge | a separate ↻ knob is fine (more findable on a trackpad); the badge is a must |
| inspector | always visible, fields in a fixed order, values commit on Return or blur, one undo per commit | no layout jump when the selection changes kind; scrubbing a field must not make 40 undo steps |
| context menu | two-finger click; the menu is the same on the canvas and the layers list | every panel action that's a verb (Delete, Duplicate, Hide labels) belongs in the menu too |
| nudge | arrows 1 px (Figma) / 1 pt, ⇧ ×10 | here: cell or 0.1 cm, ⌥ fine; matches |
| duplicate | ⌘D, the copy offset and selected, a second ⌘D repeats the offset | the copy must be selected so the next gesture acts on it |
| undo | each gesture is one step; selection is restored with it | ⌘Z after a panel edit should leave the same object selected |

## How often she'd hit each task (for ranking fixes)

From her four kinds and how she's described working ("a visual while
working", mid-lecture):

- **Every figure:** D2 (arrow head), D3 (label), D4 (label side), D1 (bend),
  N3/N4 (vertex labels), G1 (undo), D6 (delete).
- **Most figures of that kind:** N1 (axes), N2 (sit flat), N6 (size/angle),
  B1 (blob size), B2/B3 (elements), C1 (rotate), C3 (labels/legend).
- **Now and then:** N5 (duplicate + turn: the D4 sequence pictures), N7,
  B5, B6, C2, C4, C5, D5 (rebuilding a sketch).

A friction on an "every figure" task outranks a failure on a "now and then"
one.

---

# Phase 2: what the real window took (2026-10-12, after FIG-INTERACT)

Driven by `studio/tests/usability.rs` (`cargo test --test usability`):
HeadlessAppContext, offscreen Metal, a 1180 × 900 window (the real default),
temp `.fig` copies and temp `NTEX_STUDIO_CONFIG` / `STATE`. Every gesture is
a synthetic mouse or key event aimed at what is drawn:

- canvas objects where the Scene draws them;
- handles where they are painted;
- panel and menu controls by their laid-out bounds (`ui_bounds`, cleared
  before every frame so a control that isn't drawn can't be found).

No internal command is called to do a task. Per-step PNGs are in
`studio/evidence/usability/` (105 of them, `<task>-NN-<what>.png`), and the
full log (steps, issues, source diff, undo check, drag latency) is in
`evidence/usability/results.txt`. Every task ends with an undo/redo check:
⌘Z × the number of undo steps the task made must restore the source exactly,
and ⌘⇧Z must redo it. **This passed on every task.**

Some tasks have a "b/c/d" variant for a second route or a second figure.
"Steps" counts her gestures as in phase 1. A few harness-only checks (for
example, ticking a box back on to test that it remembers) are left out of
the counts below.

## Results

| Task | Result | Steps (ideal) | Issues |
| --- | --- | --- | --- |
| N1 draw D4 + diagonal axis | success | 3 (3) | Axis chips are bare `1 2 1.5 2.5`: nothing says that 1 is the diagonal through 1 and 3, and hovering one previews nothing. The chips are 20×20 px. |
| N2 sit flat (square) | success | 2 (2) | — |
| N2b sit flat (pentagon, odd n) | success | 2 (2) | Correct: the edge ends up at the bottom. |
| N3 hide vertex numbers | success | 2 (2) by panel, 2 (2) by menu | Labels come back as they were. |
| N4 label vertices a b c d (on canvas) | friction | 15 (6) | Tab in the in-place field doesn't move to the next vertex, so it takes four double-click / type / Return rounds. The in-place field is tiny mono text, offset left of the label it replaces (N4-02). |
| N4b the same, in the panel | **fail** | 10 (6) | Tab doesn't move between the panel's vertex fields: all four letters land in field 1, giving `labels d,2,3,4`. The fields are 38 px wide. |
| N5 copy pentagon, turn once | success | 3 (3) | The copy lands beside the original, selected; Turn relabels the copy only. |
| N6 hexagon bigger + on a vertex | success | 3 (3) | The radius dot is 7 px drawn and has a 19 px grab area. It floats at the right of the box, off the shape (N6-04). The knob is fine (25 px). |
| N7 → pentagon, no fill | success | 3 (3) | Her labels are trimmed (a…e), not wiped. |
| C1 rotate Cayley so e is on top | success | 2 (2) | One knob drag, with a live "°" readout. |
| C2 Cayley smaller | success | 2 (2) | The resize dot's grab area is 19 px. |
| C3 hide names, show legend | success | 3 (3) | — |
| C4 D4 → D5 | friction | 4 (3) | Return in the Group field **deselects the graph**: the panel she's typing in vanishes. A half-typed "D" is safely ignored. |
| C5 Z6 sketch from nothing | **fail** | — | There is no Cayley tool, and right-clicking empty canvas offers only *Select all*. A Cayley graph can only be inserted with shorthand or Ask. |
| B1 kernel blob bigger | success | 2 (2) | The click inside K picks K (innermost). G doesn't grow, so the larger K's label crowds G's outline. Corner handles have a 19 px grab area. |
| B2 add element to H | friction | 5 (3) | *+ Element* adds a placeholder `x` (`{e', h, x "h'"}` after renaming) and doesn't focus its field: the toast sends her to the panel. |
| B3 rename g → g^{-1} | success | 3 (3) | The name is kept and the label changes, so the `g |-> h` arrow still connects. |
| B4 send g to h | friction | 2 (2) +2 | Element → element defaults to `->`, not `↦`. She has to fix it in the panel. |
| B5 unshade K, maroon | success | 3 (3) | The swatches are **16×16 px** in every panel. |
| B6 remove G's label | success | 4 (3) | `blob G ""` round-trips with no warning. There's no menu route. |
| B6b H's label inside | success | 2 (2) | — |
| D1 bend arrow off a crossing | success | 3 (2) | The extra click is fair: the line lies on A→B, so she clicks the label. The bend and label handles are 19 px apart, and the bend handle's grab area is 19 px. The readout "bend left 30°" is good. |
| D2 → surjection | success | 2 (2) | The arrow hit band is 18 px, which is good. Head glyph chips are 26×20. φ is kept: `-φ->>`. |
| D3 f → φ | friction | 4 (3) | **Double-clicking an arrow's label doesn't edit it**; only Return does. The edit also turns her `grid … end` block into statements. |
| D3b remove f's label | friction | 4 (2) | The arrow's menu has no *Hide label* (its *Show labels* entry is greyed out). The only route is clearing the panel field. |
| D4 g's label to the other side | success | 2 (2) | Only once the arrow is selected: dragging an unselected arrow's label does nothing. The grid becomes statements. |
| D5 sketch → pullback | friction | 22 (12) | **Dragging one auto-placed node re-flows all the others** (D5-03: Y and Z jump while she drags X), so she drags until it settles. Otherwise fine: the node tool opens the label field, and the arrow tool stays on between arrows. |
| D6 delete n-gon | success | 2 (2) | — |
| D6b delete commutative square (marquee ⌫) | **fail** | 2 (2) | Toast: *"! Couldn't delete: item 8 isn't an arrow"*; nothing is deleted. |
| D6c ⌘A ⌫ on a plain diagram | **fail** | 2 (2) | Toast: *"! Couldn't delete: item 7 isn't an arrow"*. |
| D6d delete one corner node | success | 2 (2) | Its arrows go too, with no dangling errors. The grid becomes statements. |
| G1 undo | success | 1 per gesture | A whole slider drag is 1 undo step, and typing "45" in Angle is 1 step. ⌘Z restores both, and the selection survives undo. |

**Tally:** 32 attempts. **21 success, 7 friction, 4 fail** (N4b, C5, D6b,
D6c).

**Drag input → frame** (dispatch + layout + paint, CPU, headless): every
drag in every task had a p95 ≤ 4.5 ms and a max ≤ 4.8 ms. There were no
layout jumps of the canvas or the panel mid-gesture, apart from the
auto-placement re-flow in D5.

**Targets under 24 px** (the Z1 probe, at the default zoom):

- every colour swatch: 16×16;
- axis chips `1`, `2`: 20×20;
- `1.5`, `2.5`: 33×20;
- ⟲ ⟳: 24×20;
- arrow head chips: 26×20;
- handles: radius dot, blob corners, bend and Cayley resize all have a
  19 px grab area (`GRAB_PX = 9`). The ↻ knob is fine at 25 px.

## Top 10 fixes, ranked by how much they'd annoy her

1. **⌘A ⌫ and marquee ⌫ fail on any diagram with nodes and arrows.**
   `tools.rs` `delete_selection` sorts arrows by `usize::MAX − i` but nodes
   by `0`, so nodes are deleted *first*. Each node takes its arrows with it,
   and the arrow indices that follow are stale (*"item N isn't an arrow"*).
   Delete arrows and texts high → low first, then nodes, or skip arrows
   that are already gone. "Delete this" is an every-figure gesture, and a
   failure toast for it erodes trust fast.
2. **Dragging one auto-placed node re-flows the rest** (D5-03). In a quick
   sketch, every node is auto-placed: the first drag writes `at` for that
   node only, and the others jump to "nearest free cells". On the first
   drag, pin every auto-placed node where it is drawn, so only the one she
   moves moves.
3. **Double-click on an arrow's label doesn't edit it.** Once the arrow is
   selected, the label's own swap handle (`HKind::ArrowLabel`) sits on the
   label and takes the second press before the `click_count >= 2` branch
   in `tools.rs`. Check for a double-click before the handle hit. (The
   footnote says double-click edits labels; it works for nodes, elements
   and vertex labels, so this one surprises.)
4. **Vertex labels: no Tab to the next vertex**, neither in the in-place
   field nor across the panel's per-vertex fields (N4b ends `d,2,3,4`).
   "Label the vertices a b c d" is an every-n-gon task. Make Tab / ⇧Tab
   walk vertex k → k+1 in both places, widen the panel fields (38 px now),
   and make the in-place field match the label's size.
5. **Small targets, all at once:** swatches 16 px; axis chips 20 px; head
   chips and ⟲ ⟳ 20 px tall; handle grab radius 9 px. Raise `GRAB_PX` to
   12 or more (keep the drawn size), make swatches and chips at least 24 px
   tall, and draw the n-gon radius dot on a vertex direction (or on the box
   edge). It floats off the hexagon now and reads as a stray dot.
6. **No way to insert a Cayley graph without shorthand** (C5). Add
   *Insert ▸ Cayley graph / n-gon / blob / text* to the empty-canvas menu,
   which offers only *Select all* now. The Group field then does the rest.
   This is the one place the rule "anything the shorthand can say, the
   mouse can do" is broken outright.
7. **Editing anything in a `grid … end` block converts her ASCII art to
   statements.** This happens on a relabel, a swap or a node delete. The
   comment says so honestly, but the art she typed in Neovim is gone after
   one click. For local edits, rewrite the label or option inside the
   grid text, and convert only when the art can't express the change.
8. **Changing the Cayley group deselects the graph** (C4): the panel
   vanishes on Return. Keep the selection across the rename, since the
   graph is the same object.
9. **Element → element arrows default to `->`.** It should be `↦`, as her
   kernel pictures always are. Separately, *+ Element* should focus the new
   element's field with a sensible name rather than a placeholder `x`, so
   adding is one click plus typing.
10. **Arrow labels have no quick hide, and the axis chips don't say what
    they are.** Add *Hide label* to an arrow's menu, and let dragging an
    unselected arrow's label swap it as the footnote promises. Name the
    axis chips by what they pass through (`1–3`, `edge 1–2`), and preview
    the dashed axis on hover.

Lower, but noticed:

- Any canvas edit reformats every blob line (`blob G "G" {…}` →
  `blob G {…}`, `in G shaded {e, k}` → `{e, k} in G shaded`), so a
  one-blob change shows as a whole-file diff in her `.fig`.
- Growing K doesn't grow G.
- Disabled menu entries (Rotate 90° and Show labels on an arrow) add
  noise.

## Where the builders did better than expected

- **N-gons are Keynote-grade.** Sit flat (odd n correct), sides, fill,
  duplicate and turn, rotate and resize all hit the ideal step count. Turn
  and Angle are distinct ("relabels" says which is which), and changing
  the sides keeps her labels.
- **Undo is right everywhere.** One gesture is one step: a slider drag is
  one step, typing in a field is one step, and a whole label edit is one
  step. Redo works, and the selection survives.
- **Spelling is preserved:** `-φ->>` stays φ, renaming an element's label
  keeps its name so arrows don't break, and `blob G ""` round-trips.
- **Picking is predictable.** The innermost blob wins, a Cayley graph is
  taken whole on the first click, the arrow hit band is 18 px, and hover
  outlines show what a click will take.
- **Readouts and hints:** "15°" and "bend left 30°" follow the pointer, and
  every selection has its own footnote naming its keys.
- **Feel:** drag input-to-frame p95 is ≤ 4.5 ms on every handle; there's no
  draft/exact swap jump. The panel is fully in the LaTeX look (small caps,
  \fbox chips, Definition box).
