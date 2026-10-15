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
- **Label:** `swap`, `iso`, `near-start`, `near-end`
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
- `rotation[=k]` adds an arc arrow labelled r or r^k.
- `turn=k` and `flip=k` act on the labels (the group action).
- `angle=θ` turns the whole picture θ° counter-clockwise, labels and all.
- `plain` leaves out the tinted fill. `color=…` sets its tones.
- `radius=r` (cm) and `at …` set its size and place.

## Cayley graphs

Known groups are D4, Z6, Q8, S3, S4, A4, V4, Z2xZ3 and so on. You can also
give a presentation, up to 96 elements.

- `layout=circle|prism|grid`
- `labels=none`
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
```

## In the window

- Click selects. ⇧-click adds to the selection. Drag on empty canvas for a
  marquee.
- Handles resize and rotate (⇧ turns snapping off). Each drag is one undo
  step.
- **Keys:**
  - Arrows nudge the selection.
  - Return edits the label; ⌫ deletes.
  - ⌘D duplicates.
  - [ and ] rotate by 15°; − and = shrink or grow.
  - L shows or hides labels.
  - ⌘I toggles the properties panel; ⌘/ toggles this reference.
