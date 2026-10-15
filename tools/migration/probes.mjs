// The examples the decision log is checked with: every old snippet's own
// example (in .tex and .md), every symbol-finder command typed as written,
// and the cases that show a behaviour change. Each probe is typed into the
// old setup (before) and the new one (after) by reference/verify.lua.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { SYMBOLS } from "../../reference/content/symbols.mjs";
import { CASTEL_PROBES, OFF_PROBES } from "./spec.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
export const SNAPSHOT = JSON.parse(fs.readFileSync(path.join(HERE, "latex-suite-snapshot.json"), "utf8"));

const toMd = (before) => before.replace(/^\\\[\n/, "$$$$\n").replace(/\n\\\]$/, "\n$$$$");
const esc = (s) => s.replace(/</g, "<lt>");

export const ENGLISH = "Let us admit the landmark result: the subset of all sets is a set, and the inner sum is finite.";

export function probes() {
  const out = [];
  const add = (p) => { p.id = p.id || [p.group, p.ft, p.before, p.keys, p.select ?? ""].join("|"); out.push(p); };
  // 1. each old snippet's example, in .tex and in .md
  for (const r of SNAPSHOT.rows) {
    if (!r.job) continue;
    for (const ft of ["tex", "md"]) {
      add({ group: "snippet", index: r.index, ft, before: ft === "md" ? toMd(r.job.before) : r.job.before, keys: r.job.keys, select: r.job.select || undefined });
    }
  }
  // 2. symbol-finder commands typed as written in $…$ (.tex)
  for (const [cmd] of SYMBOLS) add({ group: "symbol", cmd, ft: "tex", before: "$‸$", keys: esc(cmd) });
  // 3. letter triggers right after a letter
  for (const r of SNAPSHOT.rows) {
    if (r.kind !== "string" || !/^[A-Za-z]/.test(r.trigger) || !r.options.includes("A")) continue;
    if (r.options.includes("t")) add({ group: "after-letter", index: r.index, ft: "tex", before: "Hello‸", keys: esc(r.trigger) });
    else add({ group: "after-letter", index: r.index, ft: "tex", before: r.options.includes("M") ? "\\[\nA‸\n\\]" : "$A‸$", keys: esc(r.trigger) });
  }
  // 4. specific cases
  const sp = (name, ft, before, keys, extra = {}) => add({ group: "case", name, ft, before, keys, ...extra });
  sp("leq2", "tex", "$‸$", "\\leq2");
  sp("sqrt2", "tex", "$‸$", "\\sqrt2");
  sp("log2", "tex", "$‸$", "log2");
  sp("sin2", "tex", "$‸$", "sin2");
  sp("log_2", "tex", "$‸$", "log_2");
  sp("ab2", "tex", "$‸$", "ab2");
  sp("alpha2", "tex", "$‸$", "\\alpha2");
  sp("ce-H2O", "tex", "$\\ce{ ‸ }$", "H2O");
  sp("ce-arrow", "tex", "$\\ce{ ‸ }$", "A -> B");
  sp("pu-units", "tex", "$\\pu{ ‸ }$", "3 m2");
  sp("label-sub", "tex", "\\begin{equation}\nx \\label{eq:‸}\n\\end{equation}", "sub");
  sp("ref-in-math", "tex", "$x = \\eqref{‸}$", "uu");
  sp("brace-after-backslash", "tex", "$‸$", "\\{x\\}");
  sp("x_i-literal", "tex", "$‸$", "x_{i}");
  sp("comment-math", "tex", "% $‸$", "uu x2");
  sp("comment-text", "tex", "% ‸", "mk");
  sp("mk-after-letter", "md", "Hello‸", "mk");
  sp("mk-inline-code", "md", "`‸`", "mk");
  sp("dm-list-admit", "md", "- we ‸", "admit");
  sp("dm-list-landmark", "md", "1. a ‸", "landmark");
  sp("dm-list-indented", "md", "  - item ‸", "dm");
  sp("dm-indent-tex", "tex", "Intro\n‸", "dm");
  sp("english-text-tex", "tex", "‸", esc(ENGLISH));
  sp("english-text-md", "md", "‸", esc(ENGLISH));
  sp("english-textcmd", "tex", "$x \\text{‸}$", esc(ENGLISH));
  sp("vis-in-text", "tex", "plain ‸text here", "S", { select: 4 });
  sp("o+-display", "tex", "\\[\n‸\n\\]", "o+");
  sp("int-line-start", "tex", "\\[\n‸\n\\]", "int");
  sp("mk-tex", "tex", "Let ‸", "mk");
  sp("dm-md", "md", "Intro\n‸", "dm");
  sp("autofraction", "tex", "$‸$", "(a+b)/c");
  sp("tabout", "tex", "$\\sqrt{a‸}$", "<Tab><Tab>");
  sp("matrix-enter", "tex", "\\[\n\\begin{pmatrix}\na & b‸\n\\end{pmatrix}\n\\]", "<CR>c");
  sp("enlarge", "tex", "$‸$", "(sum");
  sp("bs-dollar", "tex", "x $‸$ y", "<BS>");
  sp("@a", "tex", "$‸$", "@a");
  sp("alpha", "tex", "$‸$", "alpha");
  sp("tox", "tex", "$‸$", "\\tox");
  sp("Auu-fire-after-letter", "tex", "$A ‸$", "uu");
  sp("Acdot", "tex", "$A‸$", "cdot");
  // 5. what Castel's additions do (before: nothing, the keys stay as typed)
  const ca = (name, ft, before, keys, extra = {}) => add({ group: "castel", name, ft, before, keys, ...extra });
  for (const t of ["mcal", "lll", "star", "perp", "pi", "zeta", "~~", "<!", "<>", "DD"]) ca(t, "tex", "$‸$", esc(t));
  ca("compl", "tex", "$A‸$", "compl");
  ca("lr", "tex", "$‸$", "lr<Tab>");
  ca("lrb", "tex", "$‸$", "lrb<Tab>");
  ca("/", "tex", "$‸a+b$", "/", { select: 3 });
  for (const t of ["table", "fig", "desc", "pac", "plot", "bigfun"]) ca(t, "tex", "‸", t + "<Tab>");
  ca("table-md", "md", "‸", "table<Tab>");
  ca("pi-in-word", "tex", "$‸$", "\\varpi \\epsilon spin");
  // 6. every Castel trigger, typed into your setup
  for (const [line, list] of Object.entries(CASTEL_PROBES)) {
    list.forEach(([ft, before, keys, select], k) => add({ group: "castel-row", line: +line, k, ft, before, keys, select }));
  }
  // 7. every still-off default, typed: it stays off (or yours answers)
  for (const [i, list] of Object.entries(OFF_PROBES)) {
    list.forEach(([ft, before, keys], k) => add({ group: "off", off: +i, k, ft, before, keys: esc(keys) }));
  }
  return out;
}
