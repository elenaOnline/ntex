# Theme workshop

`specimen.tex` uses every element of the notes theme. The theme itself lives at
`../tex/ntex-notes.sty` (that's the one new documents use). Snapshots of earlier
looks: `ntex-notes.v1.sty` (left rule, Modus colours), `.v3` (title bars), `.v4`
(left rule, white-mixed tints).

Compile outside ntex:

    tectonic -Z search-path=$HOME/.config/texnvim/templates/tex specimen.tex

Inside ntex (`ntex specimen.tex`) the search path is set for you.
