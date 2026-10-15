"""ADVERSARY: build verification/expected.json (and the case list at the end of
acceptance.md) from FIGURES.md + model.rs alone, before reading figures/src.

    python3 -IB gen_expected.py

Every case: .fig source -> the model facts the parser must produce, the
diagnostics, and (for n-gons and Cayley graphs) the geometry/group facts the
layout must produce. `tier`: "spec" = stated by FIGURES.md; "inferred" = the
only sensible reading of FIGURES.md; "gap" = FIGURES.md is silent and this
records what a notes-writer would expect (reported separately, never counted
as an implementation bug on its own).
"""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import groups as G
SW = ("label typed right of an up arrow / on a leftward arrow keeps that place only with side=Swap (tikz-cd auto = left of travel). "
      "Corrected after review: my first expectation (Auto) was wrong; the implementation's original Swap was right.")

HERE = os.path.dirname(os.path.abspath(__file__))
CASES = []
GROUPS = {}

ARROW_DEFAULTS = {"label": None, "head": "To", "line": "Solid", "side": "Auto", "bend": 0.0,
                  "is_loop": False, "iso": False, "thick": False, "color": None,
                  "label_pos": "Middle"}


def arrow(frm, to, label=None, **kw):
    a = dict(ARROW_DEFAULTS)
    a.update(kw)
    a["from"], a["to"], a["label"] = frm, to, label
    return a


def case(id, title, src, tier="spec", why="", **expect):
    diag = expect.pop("diag", {"clean": True})
    build = expect.pop("build", {"compile": True})
    CASES.append({"id": id, "title": title, "src": src, "tier": tier, "why": why,
                  "expect": expect, "diag": diag, "build": build})


# ======================================================================== A
# Arrows: every row of FIGURES.md's arrow table, then the options.
A = "FIGURES.md arrow table"
case("A01", "plain arrow", "A -> B\n", why=A, arrows=[arrow("A", "B")], node_count=2,
     nodes=[{"name": "A", "label": "A"}, {"name": "B", "label": "B"}])
case("A02", "labelled", "A -f-> B\n", why=A, arrows=[arrow("A", "B", "f")])
case("A03", "quoted label with spaces", 'A -"g \\circ f"-> B\n', why=A,
     arrows=[arrow("A", "B", "g \\circ f")])
case("A04", "surjection", "A ->> B\n", why=A, arrows=[arrow("A", "B", head="TwoHeads")])
case("A05", "labelled surjection, Unicode label", "A -π->> B\n", why=A + " + Unicode table (π → \\pi)",
     arrows=[arrow("A", "B", "\\pi", head="TwoHeads")])
case("A06", "injection (tail)", "A >-> B\n", why=A, arrows=[arrow("A", "B", head="Tail")])
case("A07", "labelled injection", "A >-ι-> B\n", why=A, arrows=[arrow("A", "B", "\\iota", head="Tail")])
case("A08", "inclusion (hook)", "A hook-> B\n", why=A, arrows=[arrow("A", "B", head="Hook")])
case("A09", "labelled hook", "A -ι-hook-> B\n", why=A, arrows=[arrow("A", "B", "\\iota", head="Hook")])
case("A10", "double", "A => B\n", why=A, arrows=[arrow("A", "B", head="Double")])
case("A11", "labelled double", "A =f=> B\n", why=A, arrows=[arrow("A", "B", "f", head="Double")])
case("A12", "dashed", "A --> B\n", why=A, arrows=[arrow("A", "B", line="Dashed")])
case("A13", "labelled dashed", "A --f--> B\n", why=A, arrows=[arrow("A", "B", "f", line="Dashed")])
case("A14", "dotted", "A ..> B\n", why=A, arrows=[arrow("A", "B", line="Dotted")])
case("A15", "labelled dotted", "A .f.> B\n", why=A, arrows=[arrow("A", "B", "f", line="Dotted")])
case("A16", "reversed", "B <-f- A\n", why=A + ": `B <-f- A` is the same as `A -f-> B`",
     arrows=[arrow("A", "B", "f")])
case("A17", "both ways", "A <-> B\n", why=A, arrows=[arrow("A", "B", head="Both")])
case("A18", "labelled both ways", "A <-f-> B\n", why=A, arrows=[arrow("A", "B", "f", head="Both")])
case("A19", "equals", "A = B\n", why=A, arrows=[arrow("A", "B", head="Equal")])
case("A20", "maps-to", "g |-> h\n", why=A, arrows=[arrow("g", "h", head="MapsTo")])
case("A21", "maps-to, Unicode", "g ↦ h\n", why=A, arrows=[arrow("g", "h", head="MapsTo")])
case("A22", "quoted label with a dash", 'A -"f-g"-> B\n', why=A + ": quotes when the label has dashes",
     arrows=[arrow("A", "B", "f-g")])
O = "FIGURES.md 'Options after TO'; model.rs: bend in degrees, + = left"
case("A23", "bend", "A -f-> B bend\n", why=O + "; bend = 30°", arrows=[arrow("A", "B", "f", bend=30.0)])
case("A24", "bend-right swap", "A -f-> B bend-right swap\n", why=O,
     arrows=[arrow("A", "B", "f", bend=-30.0, side="Swap")])
case("A25", "bend=40 near-end thick", "A -> B bend=40 near-end thick\n", why=O,
     arrows=[arrow("A", "B", bend=40.0, label_pos="NearEnd", thick=True)])
case("A26", "loop", "A -f-> A loop\n", why=O + " (`loop`: a self-arrow)",
     arrows=[arrow("A", "A", "f", is_loop=True)], node_count=1)
case("A27", "iso with and without a label", "A -f-> B iso\nB -> C iso\n", why=O + "; model.rs keeps iso as a flag",
     arrows=[arrow("A", "B", "f", iso=True),
             arrow("B", "C", {"any_of": [None, "\\cong"]}, iso=True)])
case("A28", "dashed, colour, near-start options", "A -> B dashed color=teal near-start\n", why=O,
     arrows=[arrow("A", "B", line="Dashed", color="teal", label_pos="NearStart")])
case("A29", "dotted option on a solid arrow, bend-left", "A -g-> B dotted bend-left\n", why=O,
     arrows=[arrow("A", "B", "g", line="Dotted", bend=30.0)])
case("A30", "unknown option is a warning", "A -> B wiggly\nB -> C\n",
     why="FIGURES.md: 'an unknown option is a warning, not an error'",
     arrows=[arrow("A", "B"), arrow("B", "C")], diag={"warn_lines": [1], "no_errors": True})
case("A31", "non-palette colour is a warning", "A -> B color=pink\n",
     why="palette names are blue violet teal maroon green ochre grey ink; anything else is an unknown option value",
     tier="inferred", arrows=[arrow("A", "B", color={"any_of": [None, "pink"]})],
     diag={"any_lines": [1], "no_errors": True})
case("A32", "`;` separates statements", 'A -f-> B; B -g-> C; A -"g \\circ f"-> C\n',
     why="FIGURES.md: '`;` also separates statements'",
     arrows=[arrow("A", "B", "f"), arrow("B", "C", "g"), arrow("A", "C", "g \\circ f")], node_count=3)
case("A33", "every palette colour", "\n".join(f"A{i} -> B{i} color={c}" for i, c in
     enumerate("blue violet teal maroon green ochre grey ink".split())) + "\n",
     why="FIGURES.md palette list", arrows=[arrow(f"A{i}", f"B{i}", color=c) for i, c in
     enumerate("blue violet teal maroon green ochre grey ink".split())])
case("A34", "LaTeX-command label and token nodes", "π₁(X) -f_*-> π₁(Y)\n",
     why="labels are tokens; sub-digits ₁ → _{1}",
     arrows=[arrow("π₁(X)", "π₁(Y)", "f_*")],
     nodes=[{"name": "π₁(X)", "label": "\\pi_{1}(X)"}, {"name": "π₁(Y)", "label": "\\pi_{1}(Y)"}])
case("A35", "arrow label is a control sequence", "A -\\varphi-> B\nB -\\ker\\psi-> C\n", why=A,
     arrows=[arrow("A", "B", "\\varphi"), arrow("B", "C", "\\ker\\psi")])

# ======================================================================== N
# Nodes, labels, Unicode
U = "FIGURES.md 'Names and labels' + Unicode table"
case("N01", "quoted node label", 'A "\\mathbb{Z}/n\\mathbb{Z}" at 0,0\nA -> B\n', why=U,
     nodes=[{"name": "A", "label": "\\mathbb{Z}/n\\mathbb{Z}", "pos": {"Cell": [0, 0]}},
            {"name": "B", "label": "B"}], arrows=[arrow("A", "B")], node_count=2)
case("N02", "cell, fractional cell, absolute", "A at 1,0\nB at 0.5,1\nC at (3.2,-1)\nA -> B\nB -> C\n",
     why="FIGURES.md 'Placement'", nodes=[{"name": "A", "pos": {"Cell": [1, 0]}},
     {"name": "B", "pos": {"Cell": [0.5, 1]}}, {"name": "C", "pos": {"Abs": [3.2, -1]}}], node_count=3)
case("N03", "first mention wins", 'A "X" -> B\nA "Y" -> C\n', tier="inferred",
     why="FIGURES.md: 'a quoted label follows its first mention'; 'First mention wins'",
     nodes=[{"name": "A", "label": "X"}], arrows=[arrow("A", "B"), arrow("A", "C")], node_count=3,
     diag={"no_errors": True})
case("N04", "token shapes", "G/K -> \\ker\\varphi\nZ_n -> H'\n\\mathbb{Z} -> G^{ab}\n",
     why="FIGURES.md token examples (G/K, Z_n, H', \\ker\\varphi, \\mathbb{Z}, braces when balanced)",
     nodes=[{"name": n, "label": n} for n in ["G/K", "\\ker\\varphi", "Z_n", "H'", "\\mathbb{Z}", "G^{ab}"]],
     arrows=[arrow("G/K", "\\ker\\varphi"), arrow("Z_n", "H'"), arrow("\\mathbb{Z}", "G^{ab}")], node_count=6)
case("N05", "Unicode node tokens", "φ(G) -> ℤ/nℤ\n", why=U,
     nodes=[{"name": "φ(G)", "label": "\\varphi(G)"}, {"name": "ℤ/nℤ", "label": "\\mathbb{Z}/n\\mathbb{Z}"}],
     arrows=[arrow("φ(G)", "ℤ/nℤ")])
case("N06", "sub/superscript digits", "x₁ -> g²\n", why=U + " (x₁ → x_{1}, g² → g^{2})",
     nodes=[{"name": "x₁", "label": "x_{1}"}, {"name": "g²", "label": "g^{2}"}])
case("N07", "multi-digit sub/superscripts", "x₁₂ -> g³²\n", tier="inferred",
     why="'sub/superscript digits': a run is one script (x_{1}_{2} is a TeX 'Double subscript' error)",
     nodes=[{"name": "x₁₂", "label": "x_{12}"}, {"name": "g³²", "label": "g^{32}"}])
case("N08", "binary operators in tokens", "A⊗B -> A⊕B\nG×H -> K\n", why=U,
     nodes=[{"name": "A⊗B", "label": "A\\otimes B"}, {"name": "A⊕B", "label": "A\\oplus B"},
            {"name": "G×H", "label": "G\\times H"}])
case("N09", "symbol table in quoted labels",
     'X "∅ ⊆ U ∩ V ⊂ U ∪ V" at 0,0\nY "a ∈ S, b ∉ S" at 1,0\nZ "n ≤ m ≥ k ≠ ∞" at 2,0\nW "f∘g ≅ h" at 3,0\n',
     why=U + "; aliases accepted: \\le/\\leq, \\ne/\\neq, \\varnothing/\\emptyset",
     nodes=[{"name": "X", "label": "\\emptyset \\subseteq U \\cap V \\subset U \\cup V"},
            {"name": "Y", "label": "a \\in S, b \\notin S"},
            {"name": "Z", "label": "n \\leq m \\geq k \\neq \\infty"},
            {"name": "W", "label": "f\\circ g \\cong h"}], node_count=4)
case("N10", "ε, Φ, Ψ", "A -ε-> B\nΦ -> Ψ\n", why=U + " (ε → \\varepsilon)",
     arrows=[arrow("A", "B", "\\varepsilon"), arrow("Φ", "Ψ")],
     nodes=[{"name": "Φ", "label": "\\Phi"}, {"name": "Ψ", "label": "\\Psi"}])
case("N11", "double-struck chain", "ℕ hook-> ℤ ; ℤ hook-> ℚ ; ℚ hook-> ℝ ; ℝ hook-> ℂ\n", why=U,
     nodes=[{"name": c, "label": f"\\mathbb{{{l}}}"} for c, l in zip("ℕℤℚℝℂ", "NZQRC")], node_count=5,
     arrows=[arrow(a, b, head="Hook") for a, b in zip("ℕℤℚℝ", "ℤℚℝℂ")])
case("N12", "Greek letter followed by a letter", "φx -> ψg\n", tier="inferred",
     why="φ → \\varphi must not swallow the next letter: \\varphix is an undefined control sequence",
     nodes=[{"name": "φx", "label": "\\varphi x"}, {"name": "ψg", "label": "\\psi g"}])
case("N13", "maps-to symbol in a label", 'A -"x ↦ x²"-> B\n', why=U + " (↦ → \\mapsto inside a label)",
     arrows=[arrow("A", "B", "x \\mapsto x^{2}")])
case("N14", "node statement alone", 'A "G" at 2,1\n', why="FIGURES.md placement; a lone node is a figure",
     nodes=[{"name": "A", "label": "G", "pos": {"Cell": [2, 1]}}], node_count=1)

# ======================================================================== S
# Structure: comments, ;, globals, text
case("S01", "comments and blank lines", "% header\nA -> B % trailing\n\n% another\nB -> C\n",
     why="FIGURES.md: '%' starts a comment to the end of the line; blank lines fine",
     arrows=[arrow("A", "B"), arrow("B", "C")], node_count=3, min_comments=2)
case("S02", "escaped percent is not a comment", 'A -"50\\%"-> B\n', tier="inferred",
     why="'% starts a comment … as in LaTeX': in LaTeX \\% is a percent sign",
     arrows=[arrow("A", "B", "50\\%")])
case("S03", "`;` inside a quoted label", 'A -"a;b"-> B\n', tier="inferred",
     why="quotes delimit a label; `;` only separates statements outside quotes",
     arrows=[arrow("A", "B", "a;b")])
case("S04", "globals", "scale 1.2\nspacing 2.8,2\nsize small\nA -> B\n", why="FIGURES.md 'Free text and globals'",
     globals={"scale": 1.2, "spacing": [2.8, 2.0], "size": "small"}, arrows=[arrow("A", "B")])
case("S05", "free text", 'text "kernel ↦ identity" at (1,2)\nA -> B\n', why="FIGURES.md 'Free text'",
     texts=[{"contains": ["kernel", "identity"], "pos": {"Abs": [1, 2]}}], arrows=[arrow("A", "B")])
case("S06", "CRLF line endings", "A -> B\r\nB -f-> C\r\n", tier="inferred",
     why="line-oriented; a \\r must not leak into names or labels",
     arrows=[arrow("A", "B"), arrow("B", "C", "f")], node_count=3)
case("S07", "tabs and indentation", "\tA -> B\n    B  ->   C\n", tier="inferred",
     why="whitespace separates; indentation is not meaningful outside grid blocks",
     arrows=[arrow("A", "B"), arrow("B", "C")])
case("S08", "defaults", "A -> B\n", why="model.rs Globals::default",
     globals={"scale": 1.0, "spacing": [1.9, 1.35], "size": "normal"})

# ======================================================================== G
# Grid blocks
GR = "FIGURES.md 'Grid blocks'"
case("G01", "the spec's square", "grid\n  A --f--> B\n  |g       |h\n  v        v\n  C --k--> D\nend\n", why=GR,
     grid_cells={"A": [0, 0], "B": [1, 0], "C": [0, 1], "D": [1, 1]}, node_count=4,
     arrows=[arrow("A", "B", "f"), arrow("A", "C", "g"), arrow("B", "D", "h"), arrow("C", "D", "k")],
     grid_blocks=1)
case("G02", "pullback with a diagonal and bent arrows after end",
     "grid\n  X\n     \\u\n      P --q--> B\n      |p       |g\n      v        v\n      A --f--> C\nend\n"
     "X -x-> B bend-left\nX -y-> A bend-right\n", why=GR + " (diagonal `\\label` down-right; statements after end reference grid nodes)",
     grid_cells={"X": [0, 0], "P": [1, 1], "B": [2, 1], "A": [1, 2], "C": [2, 2]}, node_count=5,
     arrows=[arrow("X", "P", "u"), arrow("P", "B", "q"), arrow("P", "A", "p"), arrow("B", "C", "g"),
             arrow("A", "C", "f"), arrow("X", "B", "x", bend=30.0), arrow("X", "A", "y", bend=-30.0)])
case("G03", "triangle, down-right diagonal",
     "grid\n  A --f--> B\n     \\h    |g\n           v\n           C\nend\n", why=GR,
     grid_cells={"A": [0, 0], "B": [1, 0], "C": [1, 1]}, node_count=3,
     arrows=[arrow("A", "B", "f"), arrow("A", "C", "h"), arrow("B", "C", "g")])
case("G04", "triangle, down-left diagonal",
     "grid\n  A --f--> B\n  |g     /h\n  v\n  C\nend\n", why=GR + " (`/label` down-left: from the upper-right node)",
     grid_cells={"A": [0, 0], "B": [1, 0], "C": [0, 1]}, node_count=3,
     arrows=[arrow("A", "B", "f"), arrow("A", "C", "g"), arrow("B", "C", "h")])
case("G05", "misaligned columns + Unicode labels",
     "grid\n  G --φ--> H\n  |π        |ψ\n  v         v\n   K --χ--> L\nend\n",
     why=GR + ": columns by clustering token starts (G@2,K@3 → col 0; H@11,L@12 → col 1); verticals within 2 columns",
     grid_cells={"G": [0, 0], "H": [1, 0], "K": [0, 1], "L": [1, 1]}, node_count=4,
     arrows=[arrow("G", "H", "\\varphi"), arrow("G", "K", "\\pi"), arrow("H", "L", "\\psi"), arrow("K", "L", "\\chi")])
case("G06", "up arrows", "grid\n  A --f--> B\n  ^        ^\n  |s       |t\n  C --g--> D\nend\n",
     why=GR + ": a column starting with ^ points up (from the lower node)",
     grid_cells={"A": [0, 0], "B": [1, 0], "C": [0, 1], "D": [1, 1]},
     arrows=[arrow("A", "B", "f"), arrow("C", "A", "s", side="Swap"), arrow("D", "B", "t", side="Swap"), arrow("C", "D", "g")], side_note=SW)
case("G07", "dashed vertical with `:`", "grid\n  A --f--> B\n  :u       |g\n  v        v\n  C --h--> D\nend\n",
     why=GR + ": ':' instead of '|' is dashed",
     arrows=[arrow("A", "B", "f"), arrow("A", "C", "u", line="Dashed"), arrow("B", "D", "g"), arrow("C", "D", "h")])
case("G08", "equals verticals, surjection, injection, reversed, dotted",
     "grid\n  A -->>   B >-->   C\n  ||       ‖\n  E <--f-- F ..>    J\nend\n",
     why=GR + ": `-->>` surjection, `>-->` injection, `<--f--` reversed, `..>` dotted, `‖`/`||` equals",
     grid_cells={"A": [0, 0], "B": [1, 0], "C": [2, 0], "E": [0, 1], "F": [1, 1], "J": [2, 1]},
     arrows=[arrow("A", "B", head="TwoHeads"), arrow("B", "C", head="Tail"), arrow("A", "E", head="Equal"),
             arrow("B", "F", head="Equal"), arrow("F", "E", "f", side="Swap"), arrow("F", "J", line="Dotted")], side_note=SW)
case("G09", "spaced dashes are dashed", "grid\n  A - - > B\n  X ..>   Y\nend\n", why=GR + ": `- - >` is dashed",
     grid_cells={"A": [0, 0], "B": [1, 0], "X": [0, 1], "Y": [1, 1]},
     arrows=[arrow("A", "B", line="Dashed"), arrow("X", "Y", line="Dotted")])
case("G10", "labelled surjection and injection in a grid", "grid\n  K >--ι--> G --π-->> Q\nend\n", tier="inferred",
     why=GR + ": 'the label is between the dashes' applied to `-->>` and `>-->`",
     grid_cells={"K": [0, 0], "G": [1, 0], "Q": [2, 0]},
     arrows=[arrow("K", "G", "\\iota", head="Tail"), arrow("G", "Q", "\\pi", head="TwoHeads")])
case("G11", "grid + a diagonal statement after end",
     "grid\n  A --f--> B\n  |g       |h\n  v        v\n  C --k--> D\nend\nA -\"h \\circ f\"-> D\n", why=GR,
     arrows=[arrow("A", "B", "f"), arrow("A", "C", "g"), arrow("B", "D", "h"), arrow("C", "D", "k"),
             arrow("A", "D", "h \\circ f")])
case("G12", "vertical label that starts with a backslash",
     "grid\n  G --\\varphi--> H\n  |\\pi          |\\psi\n  v             v\n  K --\\chi-->   L\nend\n", tier="inferred",
     why=GR + ": 'a label written straight after' the bar; `|\\pi` is not a diagonal (that is `\\label` alone)",
     grid_cells={"G": [0, 0], "H": [1, 0], "K": [0, 1], "L": [1, 1]},
     arrows=[arrow("G", "H", "\\varphi"), arrow("G", "K", "\\pi"), arrow("H", "L", "\\psi"), arrow("K", "L", "\\chi")])
case("G13", "short exact sequence with two zeros", "grid\n  0 --> K >--ι--> G --π-->> Q --> 0\nend\n", tier="gap",
     why="FIGURES.md names nodes by token and 'later mentions just refer to it' — but every SES writes 0 twice. "
         "A notes-writer expects five cells in a row, not an arrow looping back to the first 0.",
     node_cells_distinct=5, arrows_count=4, diag={"no_errors": True})
case("G14", "first isomorphism theorem",
     "grid\n  G   --φ-->  H\n  |π          ^\n  v           |ι\n  G/K --ψ--> \\varphi(G)\nend\n", tier="inferred",
     why=GR + "; columns cluster G,G/K @2 and H@14,\\varphi(G)@13; the up arrow goes from \\varphi(G) to H",
     grid_cells={"G": [0, 0], "H": [1, 0], "G/K": [0, 1], "\\varphi(G)": [1, 1]},
     arrows=[arrow("G", "H", "\\varphi"), arrow("G", "G/K", "\\pi"), arrow("\\varphi(G)", "H", "\\iota", side="Swap"),
             arrow("G/K", "\\varphi(G)", "\\psi")], diag={"no_errors": True})
case("G15", "three-row grid with mixed verticals",
     "grid\n  A --> B\n  |     :\n  v     v\n  C --> D\n  ‖     ^\n  ‖     |\n  E --> F\nend\n", why=GR,
     grid_cells={"A": [0, 0], "B": [1, 0], "C": [0, 1], "D": [1, 1], "E": [0, 2], "F": [1, 2]},
     arrows=[arrow("A", "B"), arrow("A", "C"), arrow("B", "D", line="Dashed"), arrow("C", "D"),
             arrow("C", "E", head="Equal"), arrow("F", "D", side={"any_of": ["Auto", "Swap"]}), arrow("E", "F")])

# ======================================================================== B
# Blobs
BL = "FIGURES.md 'Blobs'"
case("B01", "the spec's kernel picture",
     'blob G "G" {e, g, g^2, g^3}\nblob K "\\ker\\varphi" in G shaded {e, g^2}\nblob H "H" {e\', h}\nG -φ-> H\ng |-> h ; e |-> e\'\n',
     why=BL + ". The example lists e, g^2 in both G and K although 'an element listed in a child blob is also "
         "in its parent; don't list it twice' and names 'must be unique': expected: K owns e, g^2; G owns g, g^3; no error",
     blobs=[{"name": "G", "label": "G", "parent": None, "shaded": False, "elements_include": ["g", "g^3"]},
            {"name": "K", "label": "\\ker\\varphi", "parent": "G", "shaded": True, "elements": ["e", "g^2"]},
            {"name": "H", "label": "H", "parent": None, "elements": ["e'", "h"]}],
     arrows=[arrow("G", "H", "\\varphi"), arrow("g", "h", head="MapsTo"), arrow("e", "e'", head="MapsTo")],
     node_count=0, unique_elements=True, diag={"no_errors": True})
case("B02", "blob options", 'blob X "X" {a, b} shape=bean size=3,2 label=below color=teal\nblob Y {c} shape=tall at (4,0) label=inside\n',
     why=BL + " options; a blob without a quoted label is labelled by its name (inferred)",
     blobs=[{"name": "X", "label": "X", "elements": ["a", "b"], "shape": "Bean", "size": [3, 2],
             "label_place": "Below", "color": "teal"},
            {"name": "Y", "label": {"any_of": ["Y", ""]}, "elements": ["c"], "shape": "Tall",
             "pos": {"Abs": [4, 0]}, "label_place": "Inside"}])
case("B03", "nested three deep", 'blob G "G" {g}\nblob H "H" in G {h}\nblob K "K" in H shaded {k}\n', why=BL,
     blobs=[{"name": "G", "parent": None, "elements": ["g"]}, {"name": "H", "parent": "G", "elements": ["h"]},
            {"name": "K", "parent": "H", "elements": ["k"], "shaded": True}])
case("B04", "function between sets", 'blob A "A" {a_1, a_2}\nblob B "B" {b}\nA -f-> B\na_1 |-> b\na_2 ↦ b\n', why=BL,
     blobs=[{"name": "A", "elements": ["a_1", "a_2"]}, {"name": "B", "elements": ["b"]}],
     arrows=[arrow("A", "B", "f"), arrow("a_1", "b", head="MapsTo"), arrow("a_2", "b", head="MapsTo")], node_count=0)
case("B05", "projection onto a quotient, Unicode elements",
     'blob G "G" {ε, g}\nblob Q "G/K" {K, gK}\nG -π->> Q\nε |-> K ; g |-> gK\n', why=BL + " + Unicode",
     blobs=[{"name": "G", "elements": ["ε", "g"], "element_labels": ["\\varepsilon", "g"]},
            {"name": "Q", "label": "G/K", "elements": ["K", "gK"]}],
     arrows=[arrow("G", "Q", "\\pi", head="TwoHeads"), arrow("ε", "K", head="MapsTo"), arrow("g", "gK", head="MapsTo")])
case("B06", "duplicate element name in unrelated blobs", "blob A {x}\nblob B {x}\n",
     why=BL + ": 'Element names … must be unique in the figure'", diag={"any_lines": [2]})
case("B07", "unknown parent", 'blob K "K" in Nowhere {k}\nA -> B\n', tier="inferred",
     why="`in PARENT` must name a blob; anything else is reported, and the rest renders",
     diag={"any_lines": [1]}, arrows=[arrow("A", "B")])
case("B08", "braced element tokens", 'blob Z "\\mathbb{Z}" {g^{2}, x_{1}, -1}\n', why="FIGURES.md tokens: braces inside a token when balanced",
     blobs=[{"name": "Z", "label": "\\mathbb{Z}", "elements": ["g^{2}", "x_{1}", "-1"]}])
case("B09", "unknown blob option", 'blob X "X" sparkly {a}\n', why="unknown option → warning",
     blobs=[{"name": "X", "elements": ["a"]}], diag={"warn_lines": [1], "no_errors": True})
case("B10", "blobs in a row (layout)", 'blob A "A" {a}\nblob B "B" {b}\nblob C "C" {c}\n', why=BL + ": top-level blobs go in a row, left to right, in order",
     blobs=[{"name": n} for n in "ABC"], blob_row=["A", "B", "C"])


# ======================================================================== P
# N-gons. Convention (FIGURES.md + model.rs): labels counter-clockwise from
# vertex 1, which is the top vertex for odd n and the top-left vertex for even
# n (an even polygon sits on a flat bottom edge — FIG-POLISH spec change), so
# slot i sits at base + 360°·i/n, base = 90° (odd) or 90° + 180°/n (even). turn=k applies r^k (r =
# rotation by 360/n counter-clockwise, the standard r), so the label that
# started at slot i is at slot i+k. Axis code m (2k = vertex k, 2k+1 = edge
# k's midpoint) is the line at angle 90° + 180°·(m-2)/n; reflecting across it
# sends slot i to slot (m-2-i) mod n. flip applies after turn.

def slots(n, turn=0, flip=None):
    out = {}
    for j in range(1, n + 1):
        s = (j - 1 + turn) % n
        if flip is not None:
            s = (flip - 2 - s) % n
        out[str(j)] = s
    return out


def base(n):
    return 90.0 + 180.0 / n if n % 2 == 0 else 90.0


def axis_dir(n, m):
    return (base(n) + 180.0 * (m - 2) / n) % 180.0


def all_axes(n):
    return sorted((base(n) + 180.0 * k / n) % 180.0 for k in range(n))


def ngon(name, n, labels=None, **kw):
    d = {"name": name, "n": n, "labels": labels or [str(i) for i in range(1, n + 1)],
         "all_axes": False, "axes": [], "rotation": 0, "turn": 0, "flip": None}
    d.update(kw)
    return d


NG = "FIGURES.md 'N-gons' + model.rs NGon (axis k → 2k, k.5 → 2k+1)"
case("P01", "square, defaults", "ngon P 4\n", why=NG, ngons=[ngon("P", 4, slots=slots(4))])
case("P02", "pentagon, labels, all axes, rotation", "ngon P 5 labels a,b,c,d,e axes rotation\n", why=NG,
     ngons=[ngon("P", 5, list("abcde"), all_axes=True, rotation=1, slots=slots(5), axis_dirs=all_axes(5))])
case("P03", "hexagon, two vertex axes, rotation=2", "ngon Q 6 axis 1 axis 2 rotation=2\n", why=NG,
     ngons=[ngon("Q", 6, axes=[2, 4], rotation=2, axis_dirs=sorted([axis_dir(6, 2), axis_dir(6, 4)]))])
case("P04", "hexagon, edge axes", "ngon Q 6 axis 1.5 axis 3.5\n", why=NG + " (1.5 → 3, 3.5 → 7)",
     ngons=[ngon("Q", 6, axes=[3, 7], axis_dirs=sorted([axis_dir(6, 3), axis_dir(6, 7)]))])
case("P05", "triangle turned once", "ngon T 3 turn=1\n", why=NG,
     ngons=[ngon("T", 3, turn=1, slots=slots(3, 1))])
case("P06", "square flipped across axis 1", "ngon T 4 flip=1\n", why=NG,
     ngons=[ngon("T", 4, flip=2, slots=slots(4, 0, 2))])
case("P07", "square turned then flipped", "ngon T 4 turn=1 flip=1\n", why=NG + ": flip applies after turn",
     ngons=[ngon("T", 4, turn=1, flip=2, slots=slots(4, 1, 2))])
case("P08", "hexagon flipped across an edge axis", "ngon S 6 flip=1.5\n", why=NG,
     ngons=[ngon("S", 6, flip=3, slots=slots(6, 0, 3))])
case("P09", "odd n with an edge axis", "ngon T 3 axis 1.5\nA -> B\n", tier="inferred",
     why="k.5 is only meaningful 'if n is even'; for odd n the axis through vertex k already bisects the opposite edge. "
         "Expected: a diagnostic on line 1 (accepting it as the equivalent vertex axis 3, code 6, is fine too)",
     ngons=[ngon("T", 3, axes={"any_of": [[], [6]]})], diag={"any_lines": [1], "no_errors": True},
     arrows=[arrow("A", "B")])
case("P10", "n too small", "ngon P 2\nA -> B\n", tier="inferred", why="a polygon needs n ≥ 3",
     diag={"any_lines": [1]}, arrows=[arrow("A", "B")])
case("P11", "axis out of range", "ngon P 4 axis 5\n", tier="inferred", why="vertices are 1..n",
     diag={"any_lines": [1]})
case("P12", "radius and absolute position", "ngon P 4 radius=1.5 at (2,0)\n", why=NG,
     ngons=[ngon("P", 4, radius=1.5, pos={"Abs": [2, 0]})])
case("P13", "a sequence: e, r, s", "ngon A 3 at (0,0)\nngon B 3 turn=1 at (3,0)\nngon C 3 flip=1 at (6,0)\n",
     why=NG + ": 'Several ngons in a row show a sequence'",
     ngons=[ngon("A", 3, slots=slots(3), pos={"Abs": [0, 0]}), ngon("B", 3, turn=1, slots=slots(3, 1), pos={"Abs": [3, 0]}),
            ngon("C", 3, flip=2, slots=slots(3, 0, 2), pos={"Abs": [6, 0]})])
case("P14", "turn larger than n", "ngon P 5 turn=7\n", tier="inferred", why="r^7 = r^2 in D5",
     ngons=[ngon("P", 5, turn={"any_of": [7, 2]}, slots=slots(5, 7))])
case("P15", "hexagon, all axes", "ngon H 6 axes\n", why=NG + ": n even → 3 vertex-vertex + 3 edge-edge axes",
     ngons=[ngon("H", 6, all_axes=True, axis_dirs=all_axes(6))])
case("P16", "heptagon all axes, turned and flipped", "ngon P 7 axes turn=3 flip=2\n", why=NG,
     ngons=[ngon("P", 7, all_axes=True, turn=3, flip=4, slots=slots(7, 3, 4), axis_dirs=all_axes(7))])
case("P17", "missing n", "ngon P\nA -> B\n", why="FIGURES.md error handling", diag={"error_lines": [1]},
     arrows=[arrow("A", "B")])

# ======================================================================== C
# Cayley graphs (group facts computed in groups.py: own models + own Todd-Coxeter + sympy)

def cay(id, title, src, spec, gens, key, tier="spec", why="", layout="Auto", labels=True, legend=False, **kw):
    if key not in GROUPS:
        if spec[0] == "named":
            GROUPS[key] = G.named(spec[1], gens)
        else:
            GROUPS[key] = G.presentation(spec[1], spec[2])
    gspec = {"Named": spec[1]} if spec[0] == "named" else {"Presentation": [spec[1], spec[2]]}
    c = {"group": gspec, "gens": gens, "layout": layout, "labels": labels, "legend": legend, "group_key": key}
    c.update(kw.pop("cayley_extra", {}))
    case(id, title, src, tier=tier, why=why, cayleys=[c], **kw)


CY = "FIGURES.md 'Cayley graphs'"
cay("C01", "D4 <r, s>", "cayley D4 <r, s>\n", ("named", "D4"), ["r", "s"], "D4<r,s>", why=CY + "; |D4| = 8; r: 8 arcs, s involution: 4 edges")
cay("C02", "D4 from a presentation", "cayley <a, b | a^4, b^2, (ab)^2>\n", ("pres", ["a", "b"], ["a^4", "b^2", "(ab)^2"]),
    ["a", "b"], "<a,b|a^4,b^2,(ab)^2>", why=CY)
cay("C03", "Z6 additive", "cayley Z6 <1>\n", ("named", "Z6"), ["1"], "Z6<1>", why=CY)
cay("C04", "C5", "cayley C5 <r>\n", ("named", "C5"), ["r"], "C5<r>", why=CY)
cay("C05", "V4", "cayley V4 <a, b>\n", ("named", "V4"), ["a", "b"], "V4<a,b>", why=CY + "; both involutions: 2 + 2 edges")
cay("C06", "Z2xZ2", "cayley Z2xZ2 <a, b>\n", ("named", "Z2xZ2"), ["a", "b"], "Z2xZ2<a,b>", why=CY)
cay("C07", "Q8", "cayley Q8 <i, j>\n", ("named", "Q8"), ["i", "j"], "Q8<i,j>", why=CY + "; i, j of order 4: 8 + 8 arcs")
cay("C08", "S3", "cayley S3 <a, b>\n", ("named", "S3"), ["a", "b"], "S3<a,b>", why=CY + "; order 6, non-abelian")
cay("C09", "S4", "cayley S4 <a, b>\n", ("named", "S4"), ["a", "b"], "S4<a,b>", why=CY + "; order 24")
cay("C10", "A4", "cayley A4 <a, b>\n", ("named", "A4"), ["a", "b"], "A4<a,b>", why=CY + "; order 12, no element of order 6")
cay("C11", "Z3 x Z4 (spaced)", "cayley Z3 x Z4 <a, b>\n", ("named", "Z3xZ4"), ["a", "b"], "Z3xZ4<a,b>",
    why=CY + ": `Zm x Zn`; grid layout for products", cayley_extra={"group": {"Named": {"any_of": ["Z3xZ4", "Z3 x Z4"]}}})
cay("C12", "A5 from a presentation", "cayley <a, b | a^2, b^3, (ab)^5>\n", ("pres", ["a", "b"], ["a^2", "b^3", "(ab)^5"]),
    ["a", "b"], "<a,b|a^2,b^3,(ab)^5>", why=CY + ": 60 ≤ 96; a: 30 edges, b: 60 arcs")
cay("C13", "S4 from a presentation", "cayley <a, b | a^2, b^3, (ab)^4>\n", ("pres", ["a", "b"], ["a^2", "b^3", "(ab)^4"]),
    ["a", "b"], "<a,b|a^2,b^3,(ab)^4>", why=CY)
cay("C14", "cyclic from a presentation", "cayley <a | a^7>\n", ("pres", ["a"], ["a^7"]), ["a"], "<a|a^7>", why=CY)
cay("C16", "options", "cayley D3 <r, s> layout=circle legend labels=none\n", ("named", "D3"), ["r", "s"], "D3<r,s>",
    why=CY, layout="Circle", labels=False, legend=True)
cay("C17", "Z2: a single involution", "cayley Z2 <1>\n", ("named", "Z2"), ["1"], "Z2<1>", why=CY + ": involution → 1 edge")
cay("C18", "D6 prism", "cayley D6 <r, s>\n", ("named", "D6"), ["r", "s"], "D6<r,s>", why=CY + ": 12 elements, prism (two hexagons)")
cay("C19", "Q8 from a presentation", "cayley <a, b | a^4, a^2b^2, b^3aba>\n", ("pres", ["a", "b"], ["a^4", "a^2b^2", "b^3aba"]),
    ["a", "b"], "<a,b|a^4,a^2b^2,b^3aba>", why=CY + "; relators are words, juxtaposed")
cay("C20", "Z6 <2, 3>", "cayley Z6 <2, 3>\n", ("named", "Z6"), ["2", "3"], "Z6<2,3>", tier="inferred",
    why="additive generators are elements: 2 has order 3 (6 arcs), 3 has order 2 (3 edges)")
cay("C21", "generator order decides colours", "cayley D4 <s, r>\n", ("named", "D4"), ["s", "r"], "D4<s,r>",
    why=CY + ": s gets blue (4 edges, no head), r maroon (8 arcs)")
cay("C22", "D4 with layout=grid", "cayley D4 <r, s> layout=grid labels=none\n", ("named", "D4"), ["r", "s"], "D4<r,s>",
    why=CY, layout="Grid", labels=False)
GROUPS["<a,b|a^2,b^2>"] = G.presentation(["a", "b"], ["a^2", "b^2"], check_sympy=False)
case("C15", "infinite dihedral is refused, the rest renders", "cayley <a, b | a^2, b^2>\nA -> B\n",
     why=CY + ": coset enumeration ≤ 96 elements; D∞ is infinite → a message for line 1, and A -> B still builds",
     cayleys=[{"group": {"Presentation": [["a", "b"], ["a^2", "b^2"]]}, "gens": ["a", "b"]}],
     arrows=[arrow("A", "B")], diag={}, build={"compile": True, "report_line": 1})
case("C23", "unknown group name", "cayley Foo7 <a>\nA -> B\n", tier="inferred",
     why="not in the known list; reported (parse or build) on line 1", arrows=[arrow("A", "B")],
     diag={}, build={"compile": True, "report_line": 1})

# ======================================================================== E
# Error recovery
ER = "FIGURES.md 'Error handling': 'line N: …', the rest still renders"
case("E01", "arrow with no target", "A ->\nB -> C\n", why=ER, diag={"error_lines": [1]}, arrows=[arrow("B", "C")])
case("E02", "nonsense line", "frobnicate the widget\nB -> C\n", why=ER, diag={"any_lines": [1]}, arrows=[arrow("B", "C")])
case("E03", "unbalanced quote", 'A "unclosed -> B\nC -> D\n', why=ER, diag={"any_lines": [1]}, arrows=[arrow("C", "D")])
case("E04", "grid with no end", "A -> B\ngrid\n  C --> D\n", why=ER, diag={"any_lines": [2]}, arrows_include=[arrow("A", "B")])
case("E05", "several bad lines", "A ->\nB -> C\n->> D\nC -> D\n", why=ER, diag={"error_lines": [1, 3]},
     arrows=[arrow("B", "C"), arrow("C", "D")])
case("E06", "empty file", "", why="an empty figure parses to nothing, quietly", diag={"clean": True},
     node_count=0, build={"compile": "either"})
case("E07", "malformed position", "A at 1\nA -> B\n", why=ER, diag={"any_lines": [1]}, arrows=[arrow("A", "B")])
case("E08", "malformed global", "spacing banana\nA -> B\n", why=ER, diag={"any_lines": [1]}, arrows=[arrow("A", "B")])
case("E09", "only comments", "% nothing yet\n", why="comments only: no diagnostics", diag={"clean": True},
     min_comments=1, build={"compile": "either"})

# ======================================================================== L
# Layout-only facts (auto placement), checked in adversary_build
LA = "FIGURES.md 'Nodes without a position are auto-placed'"
case("L01", "chain goes left to right", "A -> B\nB -> C\nC -> D\n", why=LA, auto_chain=["A", "B", "C", "D"])
case("L02", "3-cycle becomes a triangle", "A -> B\nB -> C\nA -> C\n", why=LA, auto_shape={"kind": "triangle", "nodes": ["A", "B", "C"]})
case("L03", "4-cycle becomes a square", "A -> B\nB -> D\nA -> C\nC -> D\n", why=LA, auto_shape={"kind": "square", "nodes": ["A", "B", "C", "D"]})
case("L04", "branching becomes a fan", "A -> B\nA -> C\nA -> D\n", why=LA, auto_shape={"kind": "fan", "nodes": ["A", "B", "C", "D"]})
case("L05", "partial placement: rest to nearest free cells", "A at 0,0\nB at 2,0\nA -> C\nC -> B\n", why=LA,
     auto_shape={"kind": "integer_cells", "nodes": ["A", "B", "C"]})
case("L06", "spacing global moves cells", "spacing 3,2\nA at 0,0\nB at 1,1\nA -> B\n", why="FIGURES.md 'spacing 2.8,2 changes that'",
     cell_vector={"from": "A", "to": "B", "dx": 3.0, "dy": -2.0})
case("L07", "default cell size", "A at 0,0\nB at 1,1\nA -> B\n", why="cells are 1.9 cm × 1.35 cm (FIG-POLISH: tikz-cd's default size); rows go down, y up",
     cell_vector={"from": "A", "to": "B", "dx": 1.9, "dy": -1.35})

# ======================================================================== R
# Her kind of figure, end to end (build + look)
case("R01", "kernel and image, with a projection",
     'blob G "G" {e, g, g^2, g^3}\nblob K "\\ker\\varphi" in G shaded {e, g^2}\nblob H "H" {e\', h}\nG -φ-> H\ng |-> h ; e |-> e\'\n'
     'g^3 |-> h ; g^2 |-> e\'\ntext "kernel ↦ identity" at (0,-2.2)\n',
     why="FIGURES.md blob example + text", tier="inferred", diag={"no_errors": True}, look=True)
case("R02", "universal property of the quotient",
     "grid\n  G --φ--> H\n  |π     /\\bar\\varphi\n  v\n  G/K\nend\n", tier="inferred",
     why=GR + ": `/label` down-left from the upper-right node H to G/K; the label starts with a backslash but follows `/`, so it is not a `\\` diagonal",
     grid_cells={"G": [0, 0], "H": [1, 0], "G/K": [0, 1]},
     arrows=[arrow("G", "H", "\\varphi"), arrow("G", "G/K", "\\pi"), arrow("H", "G/K", "\\bar\\varphi")],
     diag={"no_errors": True}, look=True)
case("R03", "D4 Cayley graph with legend", "cayley D4 <r, s> legend\n", why=CY, diag={"clean": True}, look=True)
case("R04", "dihedral sequence", "ngon A 4 axes at (0,0)\nngon B 4 turn=1 rotation at (3.5,0)\nngon C 4 flip=1.5 at (7,0)\n",
     why=NG, look=True)
case("R05", "SES of groups, Unicode",
     "1 at 0,0\nN at 1,0\nG at 2,0\nG/N at 3,0\n1' \"1\" at 4,0\n1 -> N ; N >-ι-> G ; G -π->> G/N ; G/N -> 1'\n",
     why="placement + labels declared first (first mention wins), then the arrows; 1' is labelled 1",
     nodes=[{"name": "1'", "label": "1", "pos": {"Cell": [4, 0]}}],
     arrows=[arrow("1", "N"), arrow("N", "G", "\\iota", head="Tail"), arrow("G", "G/N", "\\pi", head="TwoHeads"),
             arrow("G/N", "1'")], look=True)
case("R06", "square with iso and bends", "grid\n  A --f--> B\n  |g       |h\n  v        v\n  C --k--> D\nend\nA -> D iso\nA -m-> B bend-left\n",
     why=GR, look=True)

# mark which cases the 'look' pass rasterises
for c in CASES:
    c["look"] = c["expect"].pop("look", False)

# ------------------------------------------------------------------ output
ids = [c["id"] for c in CASES]
assert len(ids) == len(set(ids)), "duplicate ids"
out = {"version": 1, "generated_by": "verification/gen_expected.py (ADVERSARY)",
       "palette": "blue violet teal maroon green ochre grey ink".split(),
       "generator_colours": ["blue", "maroon", "teal", "green"],
       "cases": CASES, "groups": GROUPS}
with open(os.path.join(HERE, "expected.json"), "w") as f:
    json.dump(out, f, ensure_ascii=False, indent=1)

# the case table at the end of acceptance.md
acc = os.path.join(HERE, "acceptance.md")
head = open(acc).read().split("<!-- CASES (generated) -->")[0] if os.path.exists(acc) else ""
lines = ["<!-- CASES (generated) -->", "", f"## Cases ({len(CASES)})", ""]
for c in CASES:
    src = c["src"].rstrip("\n").replace("\r", "␍")
    lines.append(f"### {c['id']} — {c['title']}  `[{c['tier']}]`")
    lines.append("")
    lines.append("```fig")
    lines.append(src if src else "(empty)")
    lines.append("```")
    facts = []
    for k, v in c["expect"].items():
        facts.append(f"- **{k}**: `{json.dumps(v, ensure_ascii=False)}`")
    facts.append(f"- **diagnostics**: `{json.dumps(c['diag'], ensure_ascii=False)}`")
    if c["build"] != {"compile": True}:
        facts.append(f"- **build**: `{json.dumps(c['build'], ensure_ascii=False)}`")
    lines += facts
    if c["why"]:
        lines.append(f"- *why*: {c['why']}")
    lines.append("")
lines.append("## Groups (own models + own Todd–Coxeter; sympy agrees on every presentation)")
lines.append("")
lines.append("| key | order | per generator: order → edges | total edges | element orders | abelian |")
lines.append("| --- | --- | --- | --- | --- | --- |")
for k, g in GROUPS.items():
    if g.get("order") is None:
        lines.append(f"| `{k}` | infinite / > 20000 cosets | | | | |")
        continue
    per = ", ".join(f"{x['name']}: {x['order']} → {x['edges']}" for x in g["gens"])
    lines.append(f"| `{k}` | {g['order']} | {per} | {g['total_edges']} | {g['element_orders']} | {g['abelian']} |")
with open(acc, "w") as f:
    f.write(head + "\n".join(lines) + "\n")
print(f"{len(CASES)} cases, {len(GROUPS)} groups")
