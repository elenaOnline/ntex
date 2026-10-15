# Ntex Figures reference

A `.fig` file has one statement per line (or `;` between them). `%` starts a
comment. Anything the window can do can also be typed here, and the other
way round. The full spec is in `~/.config/texnvim/studio/figures/FIGURES.md`.

## Arrows: `FROM <arrow> TO [options]`

```
A -> B          A -f-> B          A -"g \circ f"-> B
A ->> B  onto    A >-> B  into     A hook-> B  inclusion
A => B   double  A --> B  dashed   A ..> B     dotted
B <-f- A         A <-> B           A = B       g |-> h   (or ↦)
```

- **Shape:** `bend`, `bend-left`, `bend-right`, `bend=40`, `loop`
- **Label:** `swap`, `iso`, `near-start`, `near-end`, `middle`
- **Line:** `dashed`, `dotted`, `thick`, `color=…`

Colours: `blue violet teal maroon green ochre grey ink`.

## Placing things

- `A at 1,0` puts A in a cell (column, row; rows go down).
- `A at (3.2,-1)` is in centimetres, with y going up.
- With no position, things are placed for you (a triangle, a square, a chain,
  a fan).
- Globals: `spacing 2.8,2`, `scale 1.2`, `size small|normal|large`.
- Labels: `A "\mathbb{Z}/n\mathbb{Z}"`. Unicode is fine: φ ℤ ∘ ≅ x₁ g².

## A commutative triangle (first isomorphism theorem)

```
G -π->> G/K
G/K -"\bar\varphi"-> φ(G) iso
G -φ-> φ(G) swap
```

## A commutative square, as ASCII art

```
grid
  A --f--> B
  |g       |h
  v        v
  C --k--> D
end
```

## Sets and functions (blobs)

```
blob NAME ["label"] [{elements}] [in PARENT] [shaded] [color=…]
     [shape=round|tall|wide|bean] [size=w,h] [angle=θ] [at …]
     [label=above|below|left|right|inside]
```

```
blob G "G" {e, g, g^2, g^3}
blob K "\ker\varphi" in G shaded {e, g^2}
blob H "H" {e', h} angle=15
G -φ-> H
g |-> h ; e |-> e'
```

`angle=θ` turns the outline and its points θ° counter-clockwise. A blob
inside another turns with it.

## Dihedral pictures (n-gons)

```
ngon P 4 axes rotation
ngon Q 4 turn=1
ngon R 6 labels a,b,,d,e,f plain color=teal
ngon S 4 angle=45 labels=none
```

- `labels a,b,…` names the vertices, counter-clockwise from vertex 1. An
  empty entry (`a,b,,d`) hides that one label. `labels=none` hides them all
  and remembers them.
- `axes` draws every reflection axis; `axis k` or `axis k.5` draws one.
- `axis k "s"` labels that axis `s` at its end: `axis 1 "s" axis 2 "sr"`.
- `rotation[=k]` adds an arc arrow labelled r or r^k. Clockwise: `rotation cw`
  or `rotation=-k` (r^-k). On another edge: `rotation edge=3` (`rotation cw
  edge=3`). Name it: `rotation "ρ"`, `rotation=2 "ρ^2"`.
- `turn=k` and `flip=k` act on the labels (the group action); `turn=-k`
  turns the other way.
- `angle=θ` turns the whole picture θ° counter-clockwise, labels and all.
- `plain` leaves out the tinted fill. `color=…` sets its tones.
- `radius=r` (cm) and `at …` set its size and place.

## Cayley graphs

Known groups are D4, Z6, Q8, S3, S4, A4, V4, Z2xZ3 and so on. You can also
give a presentation, up to 96 elements.

- `layout=circle|prism|grid`
- `labels=none`
- `gens r^-1:maroon,s:hidden` draws r⁻¹ in maroon and hides s
- `vertex w "x"` names the vertex w (`vertex r^2s "x"`; `vertex ρσ "x"` works too)
- `legend`
- `radius=r`
- `angle=θ` turns the layout.

```
cayley D4 <r, s> legend
cayley <a, b | a^3, b^2, abab> legend angle=30
```

## Free text

```
text "kernel ↦ identity" at (0,-2)
text "kernel" at (0,-2) color=teal size=large   % tiny small normal large huge
```

## In the window

Anything above can be done with the mouse. Each gesture is one undo step.

- **Select:** click; ⇧-click adds or removes (a click, no drag); drag on empty
  canvas for a marquee; ⌘A all, esc none. A second click on an object's label
  edits it; so does Return; Tab and ⇧Tab walk every label while editing.
  Double-click a Cayley vertex name to rename it.
- **Move:** drag. ⌥-drag copies (the arrows between the copies come along).
  ⇧ locks to horizontal or vertical (a ⇧-click without moving toggles
  selection instead). ⌘ turns snapping off. Esc cancels a drag.
- **Handles** resize and rotate (⌘ turns snapping off). An arrow's middle dot
  bends it, its end dots reattach it to another object, its label slides
  across the line to swap sides.
- **Panel:** Align and Distribute (three or more), Auto-place (clears `at`),
  Add loop arrow on a node, the Figure section (Width, Scale, Spacing).
- **Page size:** the header shows the figure's size on the page. Hold ⌘ for a
  page frame and ⌘-drag a corner (where no object is) to scale the whole
  figure.
- **Keys:**
  - Arrows nudge the selection (⌥ finer).
  - Return edits the label; ⌫ deletes.
  - ⌘D duplicates (repeating the last ⌥-drag's offset).
  - ⌘C ⌘X ⌘V copy, cut and paste as shorthand; a paste never lands on the
    originals (one cell off for nodes, half a cell for shapes).
  - ⌘R reverses the selected arrows (on a rotation arrow, turns it round).
  - [ and ] rotate by 15°; − and = shrink or grow; 3–9 set an n-gon's sides.
  - L shows or hides labels.
  - ⌘Z / ⌘⇧Z undo and redo; ⌘T tidies; ⌘S saves and builds the PDF.
  - ⌘= ⌘- zoom, ⌘0 fits, ⌘1 shows actual size (1 cm is 1 cm).
  - ⌘I toggles the properties panel; ⌘/ toggles this reference.
- Right-click: Edit label, Duplicate, Copy, Cut, Paste, Reverse, Rotate 90°,
  Distribute, Hide labels, Colour, Auto-place, Add loop arrow, order, Delete.
