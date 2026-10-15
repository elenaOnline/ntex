// Extra notes for the Notes column, keyed by trigger (as shown on the page).
// Backticks become code.

export const SNIPPET_NOTES = {
  dm: "In a .tex file you get `\\[ … \\]`, in a .md file `$$ … $$`. Three versions: at the start of a line, after other text (the block starts on the next line), and at the end of a list item (indented to the item).",
  mk: "Inline maths.",
  qed: "Types ∎ (`\\blacksquare`). Inside a `proof` environment amsthm already ends the proof with □, so this adds a second mark.",
  C: "Needs `\\usepackage{cancel}` in the preamble.",
  K: "Needs `\\usepackage{cancel}` in the preamble.",
  bra: "Needs the `physics` or `braket` package.",
  ket: "Needs the `physics` or `braket` package.",
  brk: "Needs the `physics` or `braket` package.",
  outer: "Needs the `physics` or `braket` package. The second `\\psi` copies the first as you type.",
  cee: "Needs `\\usepackage[version=4]{mhchem}`. No maths snippets fire inside `\\ce{…}`.",
  pu: "Needs `\\usepackage[version=4]{mhchem}`. No maths snippets fire inside `\\pu{…}`.",
  mod: "Types a plain `/`: a slash with no auto-fraction.",
  "\\sum": "`sum` becomes `\\sum` by itself; Tab then adds the limits.",
  "\\prod": "`prod` becomes `\\prod` by itself; Tab then adds the limits.",
  "\\int": "`int` becomes `\\int` by itself; Tab then adds the integrand and `dx`.",
  tayl: "The copies of `f`, `x`, `h` follow the first ones as you type.",
  plot: "Needs `\\usepackage{pgfplots}`.",
  _: "Typing `{` right after it is skipped, so `x_{i}` typed as written gives `x_{i}`.",
};
