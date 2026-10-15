#!/usr/bin/env node
// build.mjs — builds ~/.config/texnvim/reference/index.html, the all-in-one
// reference page for ntex. Node 24, no network.
//
//   node build.mjs            build (re-types only the snippets that changed)
//   node build.mjs --full     re-type every example
//   node build.mjs --quiet    print nothing unless something fails
//   node build.mjs --out DIR  write index.html, examples.json, build-report.txt to DIR
//   node build.mjs --check    also verify that every snippet is on the page (tests)
//
// What it reads (live, every build), all from a headless ntex (verify.lua):
//   the loaded LuaSnip snippets with their metadata (snippets/tex/*.lua, via
//   lua/snips/helpers.lua), the real keymaps / commands / prefs; every snippet
//   example and every symbol command is typed into that headless ntex too.
//   Plus snippets/decisions.json (the migration decision log),
//   prefs.lua, templates/article.tex.
// $TEXNVIM_APPNAME picks the config (default: this folder's name).
// The words live in content/*.mjs; the page's CSS and JS in client/.

import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

import { sha, whereOf, triggerText, makeJob, processResult, ORIGINS, CURSOR } from "./lib/snippets.mjs";
import { esc, keys, math, lhsToSpec, sourceWithMarks, renderPreview, prose } from "./lib/render.mjs";
import { startSection, snippetsSection, decisionsSection, featuresSection, keysSection, latexSection, symbolsSection, filesSection, commandsSection, troubleSection, sayHtml, code, slug, plainSay } from "./lib/sections.mjs";
import { wcag, apca } from "./lib/contrast.mjs";
import { VIM_KEYS, NTEX_WORDING, VIMTEX_KEEP, CATEGORIES } from "./content/vimkeys.mjs";
import { SYMBOLS, INTENDED } from "./content/symbols.mjs";
import { LATEX_MEANING, PREF_MEANING, COMMAND_INFO, filesInfo, troubleInfo } from "./content/info.mjs";
import { SNIPPET_NOTES } from "./content/notes.mjs";

const REF = path.dirname(fileURLToPath(import.meta.url));
const CFG = path.dirname(REF);
const HOME = os.homedir();
const t0 = Date.now();

// ---------------------------------------------------------------------------
// arguments and paths

const argv = process.argv.slice(2);
const flag = (f) => argv.includes(f);
const opt = (f) => {
  const i = argv.indexOf(f);
  return i >= 0 ? argv[i + 1] : null;
};
const QUIET = flag("--quiet");
const FULL = flag("--full");
const CHECK = flag("--check");
const OUT = path.resolve(opt("--out") || REF);
const log = (...a) => { if (!QUIET) console.log(...a); };

const APP = process.env.TEXNVIM_APPNAME || path.basename(CFG);
const tilde = (p) => (p.startsWith(HOME) ? "~" + p.slice(HOME.length) : p);
const P = {
  snippetsDir: path.join(CFG, "snippets"),
  decisions: path.join(CFG, "snippets", "decisions.json"),
  prefs: path.join(CFG, "prefs.lua"),
  prefsDefaults: path.join(CFG, "lua", "config", "prefs.lua"),
  template: path.join(CFG, "templates", "article.tex"),
  core: path.join(CFG, "lua", "plugins", "core.lua"),
  verify: path.join(REF, "verify.lua"),
  cache: path.join(OUT, "examples.json"),
  seedCache: path.join(REF, "examples.json"),
  index: path.join(OUT, "index.html"),
  report: path.join(OUT, "build-report.txt"),
};
const read = (p) => fs.readFileSync(p, "utf8");
const readIf = (p) => (fs.existsSync(p) ? read(p) : null);
const readJSON = (p) => {
  const s = readIf(p);
  if (s == null) return null;
  try { return JSON.parse(s); } catch { return null; }
};
function filesUnder(dir, ext) {
  if (!fs.existsSync(dir)) return [];
  return fs.readdirSync(dir, { recursive: true }).filter((f) => f.endsWith(ext)).sort().map((f) => path.join(dir, f));
}

/** Type jobs into a headless ntex (verify.lua); dump = also read its snippets, keymaps, … */
function runVerify(examples, dump) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "texnvim-ref-"));
  const jf = path.join(tmp, "jobs.json");
  const of = path.join(tmp, "out.json");
  fs.writeFileSync(jf, JSON.stringify({ examples: examples.map((j) => ({ id: j.id, before: j.before, keys: j.keys, select: j.select, ft: j.ft })), dump }));
  const env = { ...process.env, TEXNVIM_APPNAME: APP, TEXNVIM_NO_COMPILE: "1", TEXNVIM_NO_REFERENCE: "1", TEXNVIM_NO_VIEWER: "1" };
  delete env.TEXNVIM_SNIPPETS;
  const res = spawnSync("nvim", ["--clean", "--headless", "-l", P.verify, jf, of], { env, encoding: "utf8", timeout: 900000 });
  if (res.status !== 0 || !fs.existsSync(of)) {
    fs.rmSync(tmp, { recursive: true, force: true });
    throw new Error(`headless ntex failed (exit ${res.status}): ${(res.stderr || "").trim().slice(0, 2000)}`);
  }
  const out = JSON.parse(read(of));
  fs.rmSync(tmp, { recursive: true, force: true });
  return out;
}

// ---------------------------------------------------------------------------
// 1. what ntex has loaded (snippets with metadata, keymaps, prefs), cached

const luaFiles = filesUnder(path.join(CFG, "lua"), ".lua");
const snippetFiles = filesUnder(path.join(P.snippetsDir, "tex"), ".lua");
const dumpFiles = [...luaFiles, ...snippetFiles, path.join(CFG, "init.lua"), P.prefs, P.verify, path.join(CFG, "lazy-lock.json")];
const dumpHash = sha(dumpFiles.map((f) => path.relative(CFG, f) + "\0" + (readIf(f) ?? "")).join("\0"));
// contents only (not paths), so a copy of the same files (the tests) reuses the cache
const engineHash = sha([...filesUnder(path.join(CFG, "lua", "snips"), ".lua"), ...snippetFiles, P.verify, P.core, P.prefs, path.join(CFG, "lua", "config", "options.lua"), path.join(CFG, "lazy-lock.json"), path.join(REF, "lib", "snippets.mjs")]
  .map((f) => path.basename(f) + "\0" + (readIf(f) ?? "")).join("\0"));

let cache = readJSON(P.cache) || (P.cache !== P.seedCache ? readJSON(P.seedCache) : null) || {};
if (cache.version !== 2) cache = {};
cache.results = cache.results || {};
let typed = 0;
if (FULL || !cache.dump || cache.dump.hash !== dumpHash) {
  log("build: reading the snippets, keymaps and prefs of a headless ntex…");
  const out = runVerify([], true);
  cache.dump = { hash: dumpHash, data: out.dump };
}
const dump = cache.dump.data;
const snippets = dump.snippets || [];
if ((dump.errors || []).length) {
  for (const e of dump.errors) console.error(`build: ${e.file} did not load: ${e.error}`);
}

// ---------------------------------------------------------------------------
// 2. examples: which need typing into headless ntex

const jobs = []; // { id, hash, before, keys, select?, ft, … }
const entryJob = new Map();
const stableMeta = (m) => ({ kind: m.kind, trig: m.trig, pattern: m.pattern, body: m.body, where: m.where, how: m.how, line_rule: m.line_rule, eg: m.eg, ft: m.ft });
for (const m of snippets) {
  const job = makeJob(m);
  entryJob.set(m, job);
  if (job.unverifiable) continue;
  job.hash = sha(JSON.stringify({ m: stableMeta(m), before: job.before, keys: job.keys, select: job.select ?? null, ft: job.ft, engineHash }));
  job.id = job.hash;
  jobs.push(job);
  // display maths from text (dm): also typed in a .md file (\[ \] in .tex, $$ in .md)
  if (m.where === "text" && (m.trig === "dm") && job.ft === "tex") {
    const md = { before: job.before, keys: job.keys, ft: "md", ctx: "text" };
    md.hash = sha(JSON.stringify({ md: true, m: stableMeta(m), before: md.before, keys: md.keys, engineHash }));
    md.id = md.hash;
    job.md = md;
    jobs.push(md);
  }
}

const TAB = "<Tab>";
const FEATURE_JOBS = {
  "af-simple": { before: "$‸$", keys: "a/b" },
  "af-group": { before: "$‸$", keys: "(a+b)/c" },
  "af-excluded": { before: "$x^{‸}$", keys: "a/b" },
  "tab-brace": { before: "$\\sqrt{a‸}$", keys: TAB },
  "tab-dollar": { before: "so $a+b‸$ and", keys: TAB },
  "close-step": { before: "$(a‸)$", keys: ")" },
  "mx-tab": { before: "\\[\n\\begin{pmatrix}\na‸\n\\end{pmatrix}\n\\]", keys: "<Tab>b" },
  "mx-enter": { before: "\\[\n\\begin{pmatrix}\na & b‸\n\\end{pmatrix}\n\\]", keys: "<CR>c" },
  "mx-exit": { before: "\\[\n\\begin{pmatrix}\na & b \\\\\nc & d‸\n\\end{pmatrix}\n\\]", keys: "<S-CR>" },
  "mx-align": { before: "\\begin{align*}\nx &= a‸\n\\end{align*}", keys: "<CR>&= b" },
  "enlarge": { before: "$‸$", keys: "(sum", noMarks: true },
  "bs-dollar": { before: "x $‸$ y", keys: "<BS>" },
  "vis-S": { before: "$‸a+b$", keys: "S", select: 3 },
  "vis-paren": { before: "$‸a+b$", keys: "S(", select: 3 },
};
for (const [id, j] of Object.entries(FEATURE_JOBS)) {
  j.fid = id;
  j.ft = "tex";
  j.ctx = j.before.startsWith("\\[") ? "display" : "custom";
  j.hash = sha(JSON.stringify({ feature: id, ...j, engineHash }));
  j.id = j.hash;
  jobs.push(j);
}

// symbols: every command is typed as written in inline maths; it must come
// out unchanged (or as content/symbols.mjs INTENDED says, on purpose)
const escKeys = (t) => t.replace(/</g, "<lt>");
const symJobs = SYMBOLS.map(([cmd]) => {
  const job = { before: "$‸$", keys: escKeys(cmd), ctx: "inline", ft: "tex" };
  job.hash = sha(JSON.stringify({ sym: cmd, engineHash }));
  job.id = job.hash;
  jobs.push(job);
  return job;
});

const need = FULL ? jobs : jobs.filter((j) => !cache.results[j.hash]);
if (need.length) {
  log(`build: typing ${need.length} example${need.length === 1 ? "" : "s"} into headless ntex…`);
  const out = runVerify(need, false);
  for (const [id, st] of Object.entries(out.examples || {})) cache.results[id] = st;
  typed = need.length;
}
fs.mkdirSync(OUT, { recursive: true });
cache.version = 2;
if (typed) fs.writeFileSync(P.cache, JSON.stringify(cache)); // save the typing work even if rendering fails
// keep only what this build uses
const used = new Set(jobs.map((j) => j.hash));
for (const k of Object.keys(cache.results)) if (!used.has(k)) delete cache.results[k];

// ---------------------------------------------------------------------------
// 3. snippet rows

const report = [];
const flagged = [];
const firstLine = (s) => String(s).split("\n")[0];

function mathInner(whole) {
  if (/^\$[\s\S]*\$$/.test(whole) && whole.length >= 2) return { start: 1, text: whole.slice(1, -1) };
  if (whole.startsWith("\\[\n") && whole.endsWith("\n\\]")) return { start: 3, text: whole.slice(3, -3) };
  return { start: 0, text: whole };
}

const rows = [];
const resByEntry = new Map();
snippets.forEach((m, n) => {
  const job = entryJob.get(m);
  const w = whereOf(m);
  const trig = triggerText(m);
  const r = { id: `s-${n}`, e: m, group: m.group || m.file, file: m.file, line: m.line, origin: m.origin };
  r.whereLabel = w.label;
  r.whereKey = w.key;
  r.howKey = m.how;
  r.howHtml = r.howKey === "auto" ? `<span class="how auto">by itself</span>` : r.howKey === "tab" ? `<span class="how tab">${keys("Tab")}</span>` : `<span class="how visual">select, then key</span>`;
  r.trigText = trig;
  r.sortType = trig;

  // Type column
  if (m.kind === "visual") {
    r.typeHtml = `<span class="muted">select, then</span> ${keys([m.trig])}`;
  } else if (job.unverifiable) {
    r.typeHtml = `<code class="pattern" title="Lua pattern">${esc(m.pattern || m.trig)}</code>`;
  } else if (m.kind === "pattern" || m.kind === "post") {
    r.typeHtml = `${esc(trig)}<span class="eg">e.g. ${keys([job.example])}${m.how === "tab" ? " " + keys("Tab") : ""}</span>`;
  } else {
    r.typeHtml = `${keys([m.trig])}${m.how === "tab" ? " " + keys("Tab") : ""}`;
  }

  // You get
  const notes = [];
  let flag = null;
  if (job.unverifiable) {
    r.outHtml = `<span class="out plain">${esc(m.body || "(computed)")}</span>`;
    r.outPlain = m.body || "";
    flag = "no example: add eg = \"…\" to this pattern's line";
  } else {
    const st = cache.results[job.hash];
    const res = processResult(job, st);
    resByEntry.set(m, { job, res });
    if (!res.ok) {
      r.outHtml = `<span class="out plain">(no result: ${esc(res.error)})</span>`;
      r.outPlain = "";
      flag = "the engine returned an error: " + res.error;
    } else {
      const isMath = job.ctx === "inline" || job.ctx === "display";
      const inner = isMath ? mathInner(res.whole) : { start: 0, text: res.whole };
      const shift = res.start - inner.start;
      const marks = res.marks.map((mk) => ({ ...mk, a: mk.a + shift, b: mk.b + shift }));
      r.outHtml = `<span class="out">${sourceWithMarks(inner.text, marks, res.cursor + shift, { sel: res.sel, mode: res.mode })}</span>`;
      if (job.md) {
        const mdr = processResult(job.md, cache.results[job.md.hash]);
        if (mdr.ok) {
          const mm = mdr.marks.map((mk) => ({ ...mk, a: mk.a + mdr.start, b: mk.b + mdr.start }));
          r.outHtml = `<span class="ba-l">in a .tex file</span>${r.outHtml}<span class="ba-l alt-l">in a .md file</span><span class="out">${sourceWithMarks(mdr.whole, mm, mdr.cursor + mdr.start, { sel: mdr.sel, mode: mdr.mode })}</span>`;
        }
      }
      r.outPlain = res.text;
      if (isMath) r.previewHtml = renderPreview(inner.text, marks, job.ctx === "display" || inner.text.includes("\n"));
      if (res.unchanged) flag = `did not expand: typing ${job.keys} left it as typed`;
      else if (job.expected != null && res.text !== job.expected) flag = `expected ${JSON.stringify(job.expected)}, got ${JSON.stringify(res.text)}`;
    }
  }
  r.flag = flag;

  // Notes
  if (m.desc) notes.push(`<p>${prose(m.desc)}</p>`);
  const extra = SNIPPET_NOTES[m.trig];
  if (extra) notes.push(`<p>${prose(extra)}</p>`);
  const otag = { yours: "yours", changed: "changed", castel: "castel", new: "new" }[m.origin];
  if (otag) notes.push(`<p><span class="tag ${otag}">${esc(ORIGINS[m.origin])}</span></p>`);
  if (m.priority) notes.push(`<p class="muted">priority ${m.priority > 0 ? "+" + m.priority : m.priority}: tried ${m.priority > 0 ? "before" : "after"} the others</p>`);
  r.notes = notes;
  r.searchTitle = r.trigText;
  r.searchDetail = (r.outPlain || "").replace(/\s+/g, " ").slice(0, 120);
  rows.push(r);
});
for (const r of rows) {
  r.notes.push(`<p class="muted">${esc(r.file)}${r.line ? ":" + r.line : ""}</p>`);
  if (r.flag) {
    r.notes.unshift(`<p><span class="tag flag">check</span> <span class="muted">${esc(r.flag)}</span></p>`);
    flagged.push(r);
    report.push(`FLAG  ${r.trigText.padEnd(28)} ${r.file}:${r.line ?? "?"}: ${r.flag}`);
  }
  r.notesHtml = r.notes.join("");
}

// the decision log (snippets/decisions.json), written by tools/migration/decisions.mjs
const decisions = readJSON(P.decisions);
// The author's answers (snippets/decided.json) mark "needs your call" entries as decided
const decided = readJSON(path.join(CFG, "snippets", "decided.json")) || {};
for (const e of decisions?.entries || []) if (decided[e.id]) e.decided = decided[e.id];
if (!decisions) report.push("WARN  snippets/decisions.json is missing or not valid JSON: the Migration decisions section is empty");
const turned = (decisions?.entries || []).filter((e) => e.category === "off").map((e) => ({
  trigHtml: `<code>${esc(e.trigger)}</code>`, trigText: e.trigger, outHtml: esc(e.title), outText: e.title, how: e.how_off, id: e.id,
}));

// ---------------------------------------------------------------------------
// 4. features

function jobKeysSpec(k) {
  const out = [];
  let typed = "";
  const flush = () => { if (typed) out.push(typed); typed = ""; };
  const re = /<([^>]+)>|([\s\S])/g;
  const NAMES = { Tab: "Tab", CR: "Enter", "S-CR": "Shift-Enter", BS: "Backspace", "S-Tab": "Shift-Tab", lt: null };
  let m;
  while ((m = re.exec(k))) {
    if (m[1] && m[1] !== "lt") { flush(); out.push(NAMES[m[1]] || m[1]); }
    else typed += m[1] === "lt" ? "<" : m[2];
  }
  flush();
  return out;
}
function withCaret(text, cursorIdx) {
  return esc(text.slice(0, cursorIdx)) + `<span class="caret" aria-label="cursor"></span>` + esc(text.slice(cursorIdx));
}
const featureRes = {};
for (const j of Object.values(FEATURE_JOBS)) {
  const res = processResult({ ...j, ctx: "text" }, cache.results[j.hash]);
  featureRes[j.fid] = { job: j, res };
  if (!res.ok) report.push(`FLAG  feature ${j.fid}: ${res.error}`);
}
function featureHtml(id) {
  const f = featureRes[id];
  if (!f || !f.res.ok) return "";
  const j = f.job;
  const b = j.before;
  const k = b.indexOf(CURSOR);
  let beforeHtml;
  if (j.select) beforeHtml = esc(b.slice(0, k)) + `<span class="selected">${esc(b.slice(k + 1, k + 1 + j.select))}</span>` + esc(b.slice(k + 1 + j.select));
  else beforeHtml = withCaret(b.replace(CURSOR, ""), k);
  const after = sourceWithMarks(f.res.whole, (j.noMarks ? [] : f.res.marks).map((m) => ({ ...m, a: m.a + f.res.start, b: m.b + f.res.start })), f.res.cursor + f.res.start, { sel: f.res.sel, mode: f.res.mode });
  return `<div class="ba"><div><span class="ba-l">before</span><pre class="src">${beforeHtml}</pre></div><div class="ba-keys"><span class="ba-l">type</span>${keys(jobKeysSpec(j.keys))}</div><div><span class="ba-l">after</span><pre class="src">${after}</pre></div></div>`;
}

// ---------------------------------------------------------------------------
// 5. keymaps from the dump

const leader = dump.options.mapleader ?? " ";
const localleader = dump.options.maplocalleader ?? "\\";
const keyRows = [];
VIM_KEYS.forEach((k, i) => {
  const toks = Array.isArray(k.k) ? k.k : k.k.split(" ");
  const modes = k.m.split(" ");
  keyRows.push({ id: `k-${i}`, k: k.k, kText: toks.join(" "), modes, modeLabel: modeLabel(modes), what: k.what, cat: k.cat, src: "Vim", first: !!k.first });
});
function modeLabel(ms) {
  const N = { n: "Normal", i: "Insert", v: "Visual", s: "Select (snippet placeholder)", o: "after d/c/y", c: "Command line", t: "Terminal" };
  return ms.map((m) => N[m] || m).join(", ");
}
{
  const byKey = new Map();
  const visualChars = [];
  for (const m of dump.keymaps) {
    if (/<Plug>|<SNR>/.test(m.lhs)) continue;
    // the "v" listing mixes visual (x) and select (s) maps: use x and s only
    if (m.mode === "v") continue;
    const mine = (m.script || "").startsWith(dump.config) || (m.src || "").startsWith(dump.config);
    const isVimtex = /vimtex/i.test(m.rhs || "") || /vimtex/i.test(m.script || "");
    const isBlink = /blink/.test(m.src || "") && m.mode === "i";
    let tag;
    if (isBlink) {
      if (m.lhs === "<Up>" || m.lhs === "<Down>") continue;
      tag = "ntex";
    } else if (mine && !isVimtex) tag = /macos_keys/.test(m.src) || (m.desc || "").startsWith("macOS") ? "macOS keys" : "ntex";
    else if (isVimtex && VIMTEX_KEEP.has(m.lhs)) tag = mine ? "ntex" : "VimTeX";
    else if (mine && (m.lhs === "\\lv" || m.lhs === "\\ls")) tag = "ntex";
    else continue;
    if ((m.desc || "").startsWith("ntex visual snippet")) {
      visualChars.push(m.lhs);
      continue;
    }
    const mode = m.mode === "x" ? "v" : m.mode;
    const id = tag === "VimTeX" ? m.lhs + "\0" + m.rhs : m.lhs + "\0" + (m.desc || m.rhs);
    if (!byKey.has(id)) byKey.set(id, { lhs: m.lhs, desc: m.desc || m.rhs, tag, modes: new Set() });
    byKey.get(id).modes.add(mode);
  }
  let n = 0;
  const merged = new Map();
  for (const r of byKey.values()) {
    let w = null;
    for (const m of r.modes) if (NTEX_WORDING[`${m}|${r.lhs}`]) { w = NTEX_WORDING[`${m}|${r.lhs}`]; break; }
    w = w || NTEX_WORDING[`*|${r.lhs}`];
    const k = r.lhs + "\0" + (w ? w.what : r.desc);
    if (merged.has(k)) for (const m of r.modes) merged.get(k).modes.add(m);
    else merged.set(k, r);
  }
  for (const r of merged.values()) {
    const modes = ["n", "i", "v", "s", "o", "c"].filter((m) => r.modes.has(m));
    let w = null;
    for (const m of modes) if (NTEX_WORDING[`${m}|${r.lhs}`]) { w = NTEX_WORDING[`${m}|${r.lhs}`]; break; }
    w = w || NTEX_WORDING[`*|${r.lhs}`] || {};
    if (w.hidden) continue;
    const spec = lhsToSpec(r.lhs, leader, localleader);
    keyRows.push({ id: `km-${n++}`, k: spec, kText: spec.join(" "), modes, modeLabel: modeLabel(modes), what: w.what || r.desc.replace(/^(Latex Suite|ntex): /, ""), cat: w.cat || (r.tag === "VimTeX" ? "latex" : "editing"), src: r.tag, first: !!w.first });
  }
  if (visualChars.length) {
    const chars = [...new Set(visualChars)].sort();
    keyRows.push({ id: "km-visual", k: chars, kText: chars.join(" "), modes: ["s"], modeLabel: modeLabel(["s"]), what: "with text selected in maths: wrap it (visual snippets), e.g. S → \\sqrt{ … }. See Your snippets.", cat: "snippets", src: "ntex", first: false });
  }
  // the ntex keys the page promises must be there
  for (const must of ["<C-S>", "<C-Z>", " e", " r", " z", " ?", " n", " N", " g", " d", " x", " a", " f", " F", " o", " /", " b", "<M-l>", "<M-]>", "<M-[>", "<F2>", "\\lv", "\\ls", "\\ll", "\\le", "\\lc", "\\lt", "<M-Left>", "<M-Right>", "<M-BS>", "<Tab>", "<S-Tab>", "<CR>", "<S-CR>", "<C-Space>"]) {
    if (!dump.keymaps.some((m) => m.lhs === must)) report.push(`WARN  key ${must} is not mapped in ntex (the page does not list it)`);
  }
}

{
  const catOrder = Object.keys(CATEGORIES);
  const srcOrder = ["Vim", "ntex", "macOS keys", "VimTeX"];
  keyRows.sort((a, b) => catOrder.indexOf(a.cat) - catOrder.indexOf(b.cat) || srcOrder.indexOf(a.src) - srcOrder.indexOf(b.src));
}

// ---------------------------------------------------------------------------
// 6. symbols ↔ snippet outputs

// A snippet is listed for a symbol only if its verified output is, or starts
// with, exactly that command (whole command name; fixed arguments like \mathbb{R}
// must match too; example arguments like the x in \hat{x} don't).
// (not snippets whose example starts by typing a \command: \alpha2 is no way to type \alpha)
const snipOuts = rows
  .filter((r) => r.outPlain && !r.flag?.startsWith("did not") && !(resByEntry.get(r.e)?.job.example || "").startsWith("\\"))
  .map((r) => ({ r, out: r.outPlain.trim() }));
const PLACEHOLDER_ARGS = new Set(["Accents", "Structures", "Big operators", "Arrows"]);
function tokenOf(cmd, cat) {
  const first = cmd.split(" ")[0];
  if (!first.startsWith("\\")) {
    const m = first.match(/[\^_].*$/);
    return { tok: m ? m[0] : first, equal: true };
  }
  if (!/^\\[A-Za-z]/.test(first)) return { tok: first, equal: first !== "\\{" };
  if (PLACEHOLDER_ARGS.has(cat)) return { tok: first.match(/^\\[A-Za-z]+\[?/)[0], equal: false };
  return { tok: first, equal: false };
}
// the output must start with the command (\\sum_{…} for \\sum, but not \\ket{\\psi}… for \\psi)
function containsTok(out, tok) {
  if (!out.startsWith(tok)) return false;
  const next = out[tok.length];
  return !/[A-Za-z]$/.test(tok) || !next || !/[A-Za-z]/.test(next);
}
const trigHtml = (r) => (r.e.kind === "pattern" || r.e.kind === "post" ? keys([resByEntry.get(r.e)?.job.example || r.trigText]) : r.e.kind === "visual" ? `select, ${keys([r.trigText])}` : keys([r.e.trig]) + (r.howKey === "tab" ? " " + keys("Tab") : ""));
const symbolRows = SYMBOLS.map(([cmd, name, kw, cat, render, needs], i) => {
  const glyph = math(render || cmd) || `<code>${esc(cmd)}</code>`;
  const { tok, equal } = tokenOf(cmd, cat);
  const hits = [];
  for (const { r, out } of snipOuts) {
    if (equal ? out === tok : containsTok(out, tok)) hits.push({ r, len: out.length, exact: out === cmd || out === tok });
  }
  hits.sort((a, b) => b.exact - a.exact || a.len - b.len);
  const seen = new Set();
  const triggers = [];
  for (const h of hits) {
    if (seen.has(h.r.trigText) || triggers.length >= 3) continue;
    seen.add(h.r.trigText);
    triggers.push({ id: h.r.id, trig: h.r.trigText, typeHtml: trigHtml(h.r) });
  }
  // typed as written
  const sj = symJobs[i];
  const res = processResult(sj, cache.results[sj.hash]);
  const typedOut = res.ok ? res.text : null;
  const intended = INTENDED[cmd] || null;
  const onPurpose = res.ok && intended && typedOut === intended.out;
  const mangled = res.ok && typedOut !== cmd && !onPurpose;
  if (mangled) report.push(`SYMBOL ${cmd.padEnd(22)} typed as written gives ${JSON.stringify(typedOut)}`);
  if (!res.ok) report.push(`WARN  symbol ${cmd}: ${res.error}`);
  return { id: `sym-${i}`, cmd, name, kw: kw || "", cat, needs: needs || "", glyphHtml: `<span class="glyph">${glyph}</span>`, triggers, typed: typedOut, mangled, onPurpose: onPurpose ? intended : null };
});

// ---------------------------------------------------------------------------
// 7. prefs (top level and `latex = { … }`), files, commands

function luaVal(v) {
  if (v === undefined || v === null) return "nil";
  if (typeof v === "string") return JSON.stringify(v).replace(/\\t/g, "\\t");
  if (Array.isArray(v)) {
    const inner = v.map((x) => (Array.isArray(x) ? luaVal(x) : typeof x === "string" ? JSON.stringify(x) : String(x))).join(", ");
    return `{ ${inner} }`;
  }
  return String(v);
}
// comments in prefs.lua: the block above a key, or the comment after it
const prefComments = {};
{
  let block = [];
  for (const line of read(P.prefs).split("\n")) {
    const c = line.match(/^\s*--\s?(.*)$/);
    if (c) { block.push(c[1]); continue; }
    const kv = line.match(/^\s*([a-z_]+)\s*=\s*(.*?),?\s*(?:--\s*(.*))?$/);
    if (kv) prefComments[kv[1]] = (kv[3] || block.join(" ")).replace(/\s+/g, " ").trim();
    block = []; // a comment block belongs to the key right below it only
  }
}
const prefsNow = dump.prefs || {};
const prefsDef = dump.prefs_defaults || {};
const prefRows = [...new Set([...Object.keys(prefsDef), ...Object.keys(prefsNow)])].filter((k) => k !== "latex").map((k) => ({ key: k, value: luaVal(prefsNow[k]), def: luaVal(prefsDef[k]), meaning: prefComments[k] || PREF_MEANING[k] || "" }));
const latexNow = prefsNow.latex || {};
const latexDef = prefsDef.latex || {};
const latexRows = [...new Set([...Object.keys(latexDef), ...Object.keys(latexNow)])].map((k) => ({
  key: k, value: luaVal(latexNow[k]), def: luaVal(latexDef[k]), changed: luaVal(latexNow[k]) !== luaVal(latexDef[k]), meaning: LATEX_MEANING[k] || prefComments[k] || "",
}));

const H = { keys, code };
const files = filesInfo({ ...H, cfg: tilde(CFG), snippetFiles: snippetFiles.map((f) => path.basename(f)) }).map((f) => ({ ...f, exists: f.check ? fs.existsSync(f.check.replace(/^~/, HOME)) : undefined }));

const commandRows = [];
{
  const seen = new Set();
  const have = new Map(dump.commands.map((c) => [c.name, c]));
  const add = (name, src, fallback) => {
    if (seen.has(name)) return;
    const info = COMMAND_INFO[name];
    const c = have.get(name);
    if (!info && !c && !fallback) return;
    seen.add(name);
    const what = info ? info[0] : (c && c.desc) || fallback || "";
    commandRows.push({ id: "cmd-" + name, name, args: info?.[2] || "", what, keysHtml: info?.[1] ? keys(info[1].split(" ").length > 2 ? [info[1]] : info[1]) : "", src });
  };
  const mine = dump.commands.filter((c) => /^(Reference|Scratch|Snippet|Conceal|LatexSuite)/.test(c.name)).map((c) => c.name);
  for (const n of ["Reference", "Scratch", "SnippetReport", "SnippetEdit", "SnippetReload", "Conceal", ...mine.sort()]) if (have.has(n)) add(n, "ntex");
  if (have.has("Lazy")) add("Lazy", "lazy.nvim");
  add("checkhealth", "Neovim", "self-check");
  if (have.has("LuaSnipUnlinkCurrent")) add("LuaSnipUnlinkCurrent", "LuaSnip");
  // buffer-local once Copilot's server is attached, so not always in the dump
  for (const n of ["LspCopilotSignIn", "LspCopilotSignOut"]) add(n, "Copilot", COMMAND_INFO[n][0]);
  for (const n of Object.keys(COMMAND_INFO)) if (n.startsWith("Vimtex") && have.has(n)) add(n, "VimTeX");
  for (const c of dump.commands) if (c.name.startsWith("Vimtex")) add(c.name, "VimTeX", "VimTeX command");
  // the dump runs with TEXNVIM_NO_VIEWER=1 (VimTeX's viewer off), which hides these
  for (const n of ["VimtexView", "VimtexInverseSearch"]) add(n, "VimTeX");
  if (!have.has("Reference")) report.push("WARN  :Reference is not defined in ntex");
  if (!have.has("SnippetReport")) report.push("WARN  :SnippetReport is not defined in ntex");
}

const trouble = troubleInfo(H);

// ---------------------------------------------------------------------------
// 8. page

const exampleByTrigger = (t) => {
  const r = rows.find((x) => x.e.trig === t && x.e.kind !== "visual" && !x.flag && resByEntry.has(x.e));
  if (!r) return null;
  const j = entryJob.get(r.e);
  const miniTypeHtml = keys(jobKeysSpec(j.keys));
  return { id: r.id, miniTypeHtml, outHtml: r.outHtml };
};
// before/after previews for the decision log: the source, and KaTeX where it renders
function decisionPreview(text) {
  if (text == null) return { src: "", tex: null };
  const plain = text.replace(/‸/g, "");
  let tex = null;
  const inl = plain.match(/^([^$]*)\$([^$]+)\$([^$]*)$/s);
  if (inl && !inl[2].includes("\n")) tex = math(inl[2]);
  else {
    const disp = plain.match(/(?:\\\[|\$\$)\n([\s\S]*?)\n(?:\\\]|\$\$)/);
    if (disp && disp[1].trim()) tex = math(disp[1], { display: true });
  }
  return { src: text, tex };
}
const data = {
  prefs: dump.prefs,
  latex: latexNow,
  snippetRows: rows,
  snippetCount: rows.length,
  turnedOff: turned,
  exampleByTrigger,
  firstCount: keyRows.filter((k) => k.first).length,
  paths: { snippetsShort: tilde(P.snippetsDir) },
  featureHtml,
  keyRows,
  categories: CATEGORIES,
  template: read(P.template),
  symbolRows,
  files,
  prefRows,
  latexRows,
  options: dump.options,
  concealStart: !!dump.prefs.conceal,
  commandRows,
  trouble,
  decisions,
  decisionPreview,
  keysHtml: (k) => keys(jobKeysSpec(k)),
};
const sections = [startSection(data), snippetsSection(data), decisionsSection(data), featuresSection(data), keysSection(data), latexSection(data), symbolsSection(data), filesSection(data), commandsSection(data), troubleSection(data)];

const search = [];
const unent = (x) => String(x ?? "").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&amp;/g, "&");
for (const s of sections) {
  search.push({ s: s.id, a: s.id, t: s.title, d: "", k: "section" });
  for (const e of s.search) search.push({ s: s.id, ...e, t: unent(e.t), d: unent(e.d), k: unent(e.k) });
}
const css = read(path.join(REF, "client", "page.css"));
const js = read(path.join(REF, "client", "page.js"));
const verifiedN = rows.filter((r) => r.outHtml && !resByEntry.get(r.e)?.res?.error && resByEntry.has(r.e)).length;
const built = new Date();
const stamp = built.toLocaleString("en-GB", { dateStyle: "medium", timeStyle: "short" });
const vendorRel = path.relative(OUT, path.join(REF, "vendor", "katex", "katex.min.css")).split(path.sep).join("/");

const html = `<!doctype html>
<html lang="en-GB">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="light">
<title>ntex reference</title>
<link rel="stylesheet" href="${esc(vendorRel)}">
<style>
${css}
</style>
</head>
<body>
<a class="skip" href="#main">Skip to content</a>
<header class="top">
<a class="brand" href="#start">ntex <span>reference</span></a>
<div class="search" role="search">
<label for="q" class="vh">Search everything</label>
<input id="q" type="search" placeholder="Search snippets, keys, symbols, settings…" autocomplete="off" spellcheck="false" aria-controls="results" aria-expanded="false">
<span class="slash" aria-hidden="true"><kbd class="key">/</kbd></span>
<div id="results" class="results" hidden role="listbox" aria-label="Search results"></div>
</div>
</header>
<div class="shell">
<nav class="sections" aria-label="Sections"><ol>
${sections.map((s) => `<li><a href="#${s.id}">${esc(s.title)}</a></li>`).join("\n")}
</ol></nav>
<main id="main">
${sections.map((s) => `<section class="page" id="${s.id}" aria-labelledby="${s.id}-title">\n<h2 id="${s.id}-title">${esc(s.title)}</h2>\n${s.html}\n</section>`).join("\n")}
<footer>
<p>Built ${esc(stamp)} from the snippets ntex loaded from ${esc(tilde(P.snippetsDir))} (${rows.length} snippets, ${verifiedN} typed into headless ntex${flagged.length ? `, ${flagged.length} flagged` : ""}), prefs.lua and the live keymap. Rebuild: ${keys([":Reference", "Enter"])} in ntex, or <code>node ${esc(tilde(path.join(REF, "build.mjs")))}</code>. Maths by KaTeX ${esc("0.18.10")} (MIT), bundled for offline use.</p>
</footer>
</main>
</div>
<script>window.REFERENCE_DATA = ${JSON.stringify({ search, sections: Object.fromEntries(sections.map((s) => [s.id, s.title])) }).replace(/</g, "\\u003c")};</script>
<script>
${js}
</script>
</body>
</html>
`;

// ---------------------------------------------------------------------------
// 9. checks, contrast, write

if (CHECK) {
  const missing = [];
  for (const r of rows) if (!html.includes(`id="${r.id}"`)) missing.push(r.trigText);
  for (const m of snippets) {
    if (m.kind !== "pattern" && m.kind !== "post" && !html.includes(esc(m.trig))) missing.push(m.trig);
  }
  if (rows.length !== snippets.length) missing.push(`(${snippets.length} snippets loaded, ${rows.length} rows)`);
  if (!snippets.length) missing.push("(no snippets were loaded)");
  if (decisions) for (const e of decisions.entries) if (!html.includes(`id="d-${e.id}"`)) missing.push(`decision ${e.id}`);
  if (missing.length) {
    console.error("build --check: not on the page: " + missing.join(", "));
    process.exitCode = 1;
  }
}

const PAL = { bg: "#fbfbf8", surface: "#ffffff", sunk: "#f4f3ee", ink: "#1c1c21", ink2: "#50505b", blue: "#0031a9", violet: "#531ab6", green: "#006800", blueTint: "#edf1fb", violetTint: "#f3effb", greenTint: "#e8f4e8", line2: "#cbc8bd" };
const pairs = [
  ["body text / page", PAL.ink, PAL.bg],
  ["body text / table", PAL.ink, PAL.surface],
  ["secondary text / page", PAL.ink2, PAL.bg],
  ["secondary text / code bg", PAL.ink2, PAL.sunk],
  ["maths blue / table", PAL.blue, PAL.surface],
  ["maths blue / code bg", PAL.blue, PAL.sunk],
  ["link blue / page", PAL.blue, PAL.bg],
  ["keycap violet / white", PAL.violet, PAL.surface],
  ["tag 'yours' violet / violet tint", PAL.violet, PAL.violetTint],
  ["tag 'first' blue / blue tint", PAL.blue, PAL.blueTint],
  ["'by itself' green / green tint", PAL.green, PAL.greenTint],
  ["current nav blue / blue tint", PAL.blue, PAL.blueTint],
  ["pressed chip white / blue", "#ffffff", PAL.blue],
  ["chip count / blue", "#dfe6fb", PAL.blue],
  ["tabstop box border / white (non-text, 3:1)", PAL.blue, PAL.surface],
  ["tag 'needs your call' white / violet", "#ffffff", PAL.violet],
  ["needs-your-call row: body text / violet tint", PAL.ink, PAL.violetTint],
  ["needs-your-call row: secondary / violet tint", PAL.ink2, PAL.violetTint],
  ["tag 'new from Castel' green / white", PAL.green, PAL.surface],
  ["tag 'behaviour change' blue / white", PAL.blue, PAL.surface],
];
const contrast = pairs.map(([n, f, b]) => ({ n, f, b, w: wcag(f, b), a: apca(f, b) }));

fs.mkdirSync(OUT, { recursive: true });
const tmpIndex = P.index + ".tmp-" + process.pid;
fs.writeFileSync(tmpIndex, html);
fs.renameSync(tmpIndex, P.index);
fs.writeFileSync(P.cache, JSON.stringify(cache));
const reportText = [
  `ntex reference build — ${built.toISOString()}`,
  `snippets: ${rows.length} (${Object.keys(ORIGINS).map((o) => `${rows.filter((r) => r.origin === o).length} ${o}`).join(", ")}), still off: ${turned.length}, load errors: ${(dump.errors || []).length}`,
  `decisions: ${decisions ? Object.entries(decisions.counts).map(([k, v]) => `${k} ${v}`).join(", ") : "(missing)"}`,
  `examples typed this build: ${need.length}; flagged: ${flagged.length}; symbols changed by typing: ${symbolRows.filter((x) => x.mangled).length} (on purpose: ${symbolRows.filter((x) => x.onPurpose).length})`,
  `keys: ${keyRows.length} (${keyRows.filter((k) => k.src !== "Vim").length} from the live keymap), symbols: ${symbolRows.length} (${symbolRows.filter((s) => s.triggers.length).length} with a snippet), commands: ${commandRows.length}`,
  `page: ${P.index} (${(html.length / 1024).toFixed(0)} KiB), ${((Date.now() - t0) / 1000).toFixed(1)} s`,
  "",
  "Examples to check (what was typed did not give what the snippet's text suggests):",
  ...(report.length ? report : ["  (none)"]),
  "",
  "Contrast (WCAG 2 ratio; APCA Lc, text on background):",
  ...contrast.map((c) => `  ${c.n.padEnd(44)} ${c.f} on ${c.b}   ${c.w.toFixed(2)}:1   Lc ${c.a.toFixed(1)}`),
  "",
  "Every example (typed → what the engine produced; | = cursor):",
  ...rows.map((r) => {
    const j = entryJob.get(r.e);
    const res = resByEntry.get(r.e)?.res;
    if (!res || !res.ok) return `  ${r.trigText.padEnd(28)} (no example)`;
    const t = res.text;
    const shown = t.slice(0, res.cursor) + "|" + t.slice(res.cursor);
    return `  ${(r.trigText + (j.example && j.example !== r.trigText ? " [" + j.example + "]" : "")).padEnd(28)} ${j.ctx.padEnd(8)} ${JSON.stringify(shown)}${res.mode === "s" ? "  (selected: " + JSON.stringify(res.sel) + ")" : ""}   ${r.file}:${r.line}`;
  }),
  "",
].join("\n");
fs.writeFileSync(P.report, reportText);
log(reportText.split("\n").slice(0, 5).join("\n"));
if (flagged.length) log(`build: ${flagged.length} example(s) flagged, see ${tilde(P.report)}`);
