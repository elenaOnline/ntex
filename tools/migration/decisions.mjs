#!/usr/bin/env node
// Builds the migration decision log:
//   snippets/decisions.json   (the reference page renders it; tests/run.sh decisions re-checks it)
//   snippets/DECISIONS.md     (for reading)
//
//   node decisions.mjs                     re-type the "after" examples into this ntex, rebuild both files
//   node decisions.mjs --before FILE       freeze FILE (results from the old setup) as before.json first
//   node decisions.mjs --after FILE        use FILE instead of typing the "after" examples again
//
// "Before" was typed into the old setup (the Latex Suite engine, tag
// pre-luasnip) on 2026-09-30 and is frozen in before.json; the old setup no
// longer exists after the migration, so it can't be typed again.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { probes, SNAPSHOT } from "./probes.mjs";
import { CATEGORIES, CALLS, CHANGES, CULPRIT_NOTES, CULPRIT_PROBES, CULPRIT_TITLES, CASTEL, DROPPED_OWN, OFF_DESC } from "./spec.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const CFG = path.resolve(HERE, "../..");
const APP = process.env.TEXNVIM_APPNAME || path.basename(CFG);
const argv = process.argv.slice(2);
const opt = (f) => { const i = argv.indexOf(f); return i >= 0 ? argv[i + 1] : null; };
const BEFORE_FILE = path.join(HERE, "before.json");

// ---------------------------------------------------------------------------
// results

const ALL = probes();
const byId = new Map(ALL.map((p) => [p.id, p]));

function slim(st) {
  if (!st) return null;
  if (st.error) return { error: st.error };
  return { lines: st.lines, row: st.row, col: st.col, mode: st.mode, sel: st.sel ?? null };
}

if (opt("--before")) {
  const raw = JSON.parse(fs.readFileSync(opt("--before"), "utf8"));
  const out = { from: "the old ntex (Latex Suite engine, git tag pre-luasnip), typed headless", taken: raw.taken, results: {} };
  for (const [id, st] of Object.entries(raw.results)) if (byId.has(id)) out.results[id] = slim(st);
  fs.writeFileSync(BEFORE_FILE, JSON.stringify(out));
  console.log("froze", Object.keys(out.results).length, "before-results →", BEFORE_FILE);
}
const BEFORE = JSON.parse(fs.readFileSync(BEFORE_FILE, "utf8"));

function typeAfter() {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "ntex-decisions-"));
  fs.writeFileSync(path.join(tmp, "jobs.json"), JSON.stringify({ examples: ALL.map((p) => ({ id: p.id, before: p.before, keys: p.keys, select: p.select, ft: p.ft })), dump: true }));
  const env = { ...process.env, TEXNVIM_APPNAME: APP, TEXNVIM_NO_COMPILE: "1", TEXNVIM_NO_REFERENCE: "1" };
  delete env.TEXNVIM_SNIPPETS;
  delete env.LATEXSUITE_DIR;
  const res = spawnSync("nvim", ["--clean", "--headless", "-l", path.join(CFG, "reference", "verify.lua"), path.join(tmp, "jobs.json"), path.join(tmp, "out.json")], { env, encoding: "utf8", timeout: 1800000 });
  if (res.status !== 0) throw new Error("typing into ntex failed: " + res.stderr);
  const data = JSON.parse(fs.readFileSync(path.join(tmp, "out.json"), "utf8"));
  fs.rmSync(tmp, { recursive: true, force: true });
  return { taken: new Date().toISOString(), results: data.examples, snippets: data.dump.snippets };
}
let AFTER;
if (opt("--after")) {
  AFTER = JSON.parse(fs.readFileSync(opt("--after"), "utf8"));
  if (!AFTER.snippets) AFTER.snippets = typeAfter().snippets;
} else {
  console.log(`typing ${ALL.length} examples into ${APP} (headless)…`);
  AFTER = typeAfter();
}

// ---------------------------------------------------------------------------
// rendering a result as text: ‸ = the cursor (insert mode)

function render(st) {
  if (!st || st.error) return null;
  const lines = [...st.lines];
  if (st.mode === "i") {
    const b = Buffer.from(lines[st.row - 1] ?? "");
    lines[st.row - 1] = b.subarray(0, st.col).toString() + "‸" + b.subarray(st.col).toString();
  }
  return lines.join("\n");
}
const shot = (st) => (st && !st.error ? { text: render(st), mode: st.mode, sel: st.sel ?? null } : { text: null, error: st ? st.error : "not typed" });

function example(p) {
  const old = shot(BEFORE.results[p.id]);
  const now = shot(AFTER.results[p.id]);
  return {
    ft: p.ft, before: p.before, keys: p.keys, select: p.select ?? null,
    old: { ...old, verified: BEFORE.taken },
    after: { ...now, verified: now.text != null },
    same: old.text === now.text && old.mode === now.mode && (old.sel ?? null) === (now.sel ?? null),
  };
}

function find(ref) {
  const out = [];
  for (const p of ALL) {
    if (ref.snippet != null && p.group === "snippet" && p.index === ref.snippet) out.push(p);
    else if (ref.symbol && p.group === "symbol" && p.cmd === ref.symbol) out.push(p);
    else if (ref.after_letter != null && p.group === "after-letter" && p.index === ref.after_letter) out.push(p);
    else if (ref.case && p.group === "case" && p.name === ref.case) out.push(p);
    else if (ref.castel && p.group === "castel" && p.name === ref.castel) out.push(p);
    else if (ref.castel_line && p.group === "castel-row" && p.line === ref.castel_line) out.push(p);
    else if (ref.off != null && p.group === "off" && p.off === ref.off) out.push(p);
  }
  if (!out.length) throw new Error("no probe for " + JSON.stringify(ref));
  return out;
}
const examples = (refs) => refs.flatMap(find).map(example);

// ---------------------------------------------------------------------------
// old snippet → the new snippet that carries it (file:line)

const NEW = AFTER.snippets;
const WHERE = (opts) => (opts.includes("t") ? "text" : opts.includes("M") ? "display" : opts.includes("n") ? "inline" : "math");
const REGEX_TO_NEW = {
  33: { trig: "dm", line_rule: "after text" }, 34: { trig: "beg", where: "display" }, 35: { trig: "beg", where: "inline" },
  47: { trig: "log" }, 53: { trig: "det" },
  55: { post: "hat" }, 56: { post: "bar" }, 57: { post: "dot" }, 58: { post: "ddot" }, 59: { post: "tilde" }, 60: { post: "und" }, 61: { post: "vec" },
  62: { post: ",." }, 63: { post: ".," }, 64: { trig: "\\greek,." }, 65: { trig: "\\greek.," },
  75: { trig: "‹letter›‹digit›" }, 76: { trig: "x_{‹digits›}‹digit›" }, 77: { trig: "\\‹accent›{x}‹digit›" }, 78: { trig: "\\‹accent›{x}_{‹digits›}‹digit›" },
  79: { trig: "\\‹accent›{\\‹accent›{x}}‹digit›" }, 80: { trig: "\\‹accent›{\\‹accent›{x}}_{‹digits›}‹digit›" },
  120: { trig: "pa‹y›‹x›" }, 122: { trig: "int" }, 130: { trig: "sin" }, 131: { trig: "\\sin‹letter›" }, 132: { trig: "\\sinh‹letter›" }, 133: { trig: "arcsec" },
  154: { trig: "pmat", where: "display" }, 155: { trig: "cases", where: "display" }, 156: { trig: "pmat", where: "inline" }, 157: { trig: "cases", where: "inline" },
  176: { trig: "iden‹n›" }, 177: { trig: "dm", line_rule: "list item" },
};
function newFor(row) {
  let want;
  if (REGEX_TO_NEW[row.index]) want = REGEX_TO_NEW[row.index];
  else if (row.kind === "visual") want = { trig: row.trigger, kind: "visual" };
  else want = { trig: row.trigger, where: WHERE(row.options) };
  if (row.index === 32) want.line_rule = "start";
  const hits = NEW.filter((m) => {
    if (want.post) return m.kind === "post" && m.trig === want.post;
    if (m.trig !== want.trig) return false;
    if (want.kind && m.kind !== want.kind) return false;
    if (!want.kind && m.kind === "visual") return false;
    if (want.where && m.where !== want.where && !(want.where === "math" && m.kind !== "text")) return false;
    if (want.line_rule && m.line_rule !== want.line_rule) return false;
    return true;
  });
  return hits[0] || null;
}
const loc = (m) => (m ? `${m.file}:${m.line}` : null);

// ---------------------------------------------------------------------------
// entries

const entries = [];
const covered = new Set(); // old snippet indexes with an entry outside "kept"
const rowOf = (i) => SNAPSHOT.rows[i];
const disp = (row) => (row.kind === "regex" ? row.trigger : row.trigger);
const repText = (r) => (typeof r === "string" ? r : "(computed)");

// 1. needs your call
for (const c of CALLS) {
  entries.push({ id: c.id, category: "call", trigger: c.trigger, title: c.title, now: c.now, before_text: c.before ?? null, alternative: c.alternative, recommendation: c.recommendation, how: c.how ?? null, side: c.side ?? null, examples: examples(c.probes) });
}

// 2. behaviour changes: general ones, then one per snippet that used to fire inside commands
for (const c of CHANGES) {
  entries.push({ id: c.id, category: "change", trigger: c.trigger, title: c.title, why: c.why, examples: examples(c.probes) });
}
const culprits = new Map();
for (const [cmd, v] of Object.entries(SNAPSHOT.symbol_culprits)) {
  if (!culprits.has(v.index)) culprits.set(v.index, []);
  culprits.get(v.index).push({ cmd, at: v.at });
}
for (const [index, list] of [...culprits.entries()].sort((a, b) => a[0] - b[0])) {
  const row = rowOf(index);
  covered.add(index);
  const name = row.kind === "regex" ? humanTrigger(row) : row.trigger;
  const cmds = list.map((x) => x.cmd);
  const nm = newFor(row);
  entries.push({
    id: "change-inside-" + index, category: "change", trigger: name, origin: row.origin, group: row.group, old_line: row.line, file: loc(nm),
    title: CULPRIT_TITLES[row.trigger] || `${name} no longer fires inside ${cmds.slice(0, 3).map((c) => c.split(/[ {_]/)[0]).join(", ")}${cmds.length > 3 ? ", …" : ""}`,
    why: `Typing ${cmds.map((c) => "`" + c + "`").join(", ")} as written used to set it off part-way (at “${list[0].at}”). Its trigger can't fire right after a backslash or inside a word any more, so they type as written. ${name} itself works as before.${CULPRIT_NOTES[row.trigger] ? " " + CULPRIT_NOTES[row.trigger] : ""}`,
    examples: examples([...list.map((x) => ({ symbol: x.cmd })), ...(CULPRIT_PROBES[row.trigger] || []), { snippet: index }]),
  });
}
function humanTrigger(row) {
  const t = row.trigger;
  if (/\(\[a-zA-Z\]\)(\w+)/.test(t)) return "‹letter›" + t.replace(/^\(\[a-zA-Z\]\)/, "") + " (postfix)";
  if (t.includes("arcsin|sin")) return "sin, cos, tan, arcsin, …";
  if (t.includes("(det)")) return "det";
  return t;
}

// 3. kept as is
const CASTEL_SAME = new Map(CASTEL.filter((c) => c[3] === "same").map((c) => [c[1], c[0]]));
for (const row of SNAPSHOT.rows) {
  if (covered.has(row.index)) continue;
  if (DROPPED_OWN.some((d) => d.index === row.index)) continue;
  const ex = examples([{ snippet: row.index }]);
  const nm = newFor(row);
  const diff = ex.filter((e) => !e.same);
  if (diff.length) {
    console.error(`UNEXPLAINED difference for ${row.trigger} (#${row.index}):`);
    for (const e of diff) console.error(`   old ${JSON.stringify(e.old.text)}  new ${JSON.stringify(e.after.text)}`);
    process.exitCode = 1;
  }
  const name = row.kind === "regex" ? (row.job?.say?.replace(/`/g, "") || row.trigger) : row.trigger;
  entries.push({
    id: "kept-" + row.index, category: "kept", trigger: name, raw_trigger: row.trigger, output: repText(row.replacement), origin: row.origin, group: row.group, old_line: row.line, file: loc(nm),
    where: row.kind === "visual" ? "select, then key" : WHERE(row.options) + (row.options.includes("A") ? "" : ", Tab"),
    castel: CASTEL_SAME.has(row.trigger) ? `Castel has the same (tex.snippets line ${CASTEL_SAME.get(row.trigger)})` : null,
    note: row.index === 32 || row.index === 34 ? "The tab inside the block stays a tab character, as in Latex Suite (LuaSnip would turn it into spaces; say if you'd prefer that)." : null,
    examples: ex,
  });
  if (!nm) { console.error(`no new snippet found for ${row.trigger} (#${row.index})`); process.exitCode = 1; }
}

// 4–6. Castel
for (const [line, trig, does, status, note, probe] of CASTEL) {
  const cat = status === "new" ? "castel-new" : status === "differs" ? "castel-differs" : status === "same" ? null : "dropped";
  if (!cat) continue;
  const nm = status === "new" ? NEW.find((m) => m.origin === "castel" && m.trig === trig) : null;
  const how = { personal: "Castel's personal", collides: "Castel, collides", unreachable: "Castel, can't be reached", off: "Castel, kept off like yours" }[status];
  entries.push({
    id: `castel-${line}-${trig.replace(/[^A-Za-z0-9]+/g, "_")}`, category: cat, trigger: trig, castel_line: line, castel_does: does, status, why: note, file: loc(nm), dropped_kind: how ?? null,
    title: status === "new" ? `${trig} → ${does}` : `${trig}: Castel's gives ${does}`,
    examples: examples([probe || { castel_line: line }]),
  });
}
for (const d of DROPPED_OWN) {
  entries.push({ id: "dropped-own-" + d.index, category: "dropped", trigger: d.trigger, title: d.title, why: d.why, dropped_kind: "yours, never fired", examples: examples([{ snippet: d.index }]) });
}

// 7. still turned off
SNAPSHOT.turned.forEach((t, i) => {
  const desc = OFF_DESC[t.trigger] || describe(t);
  entries.push({
    id: "off-" + i, category: "off", trigger: t.trigger, title: desc, how_off: t.how + (t.line ? ` (snippets.js line ${t.line})` : ""), output: repText(t.replacement), options: t.options,
    examples: examples([{ off: i }]),
  });
});
function describe(t) {
  if (typeof t.replacement === "string" && !t.trigger.startsWith("/")) return `${t.trigger} → ${t.replacement.replace(/\$\{\d+:([^}]*)\}/g, "$1").replace(/\$\d/g, "")}${t.options.includes("t") ? " (in text)" : ""}`;
  return t.description || t.trigger;
}

// ---------------------------------------------------------------------------
// write

const counts = Object.fromEntries(CATEGORIES.map((c) => [c.id, entries.filter((e) => e.category === c.id).length]));
const verifiedAfter = entries.flatMap((e) => e.examples).filter((x) => x.after.verified).length;
const local = (iso) => new Date(iso).toLocaleString("sv-SE").slice(0, 16); // 2026-09-30 17:26, local time
const doc = {
  title: "ntex snippet migration: decisions",
  date: "2026-09-30",
  before_from: `${BEFORE.from}, ${local(BEFORE.taken)}`,
  after_from: `${APP === "texnvim-next" ? "the new ntex" : APP} (LuaSnip, snippets/tex/*.lua), ${local(AFTER.taken)}`,
  counts,
  examples: { total: entries.flatMap((e) => e.examples).length, after_verified: verifiedAfter },
  categories: CATEGORIES,
  entries,
};
const OUT_DIR = path.join(CFG, "snippets");
let DECIDED = {};
try { DECIDED = JSON.parse(fs.readFileSync(path.join(OUT_DIR, "decided.json"), "utf8")); } catch {}
fs.writeFileSync(path.join(OUT_DIR, "decisions.json"), JSON.stringify(doc, null, 1) + "\n");
fs.writeFileSync(path.join(OUT_DIR, "DECISIONS.md"), markdown(doc));
console.log("counts:", JSON.stringify(counts), "examples:", doc.examples.total);

function markdown(d) {
  const L = [];
  const code = (s) => (s == null ? "—" : "`" + String(s).replace(/`/g, "ˋ").replace(/\n/g, "⏎") + "`");
  const ex = (e) => {
    const sel = (x) => (x.sel ? ` (selected: ${code(x.sel)})` : "");
    return `${e.ft === "md" ? ".md" : ".tex"}: in ${code(e.before)} type ${code(e.keys.replace(/<lt>/g, "<"))}${e.select ? ` with ${e.select} characters selected` : ""} → before ${code(e.old.text)}${sel(e.old)}, now ${code(e.after.text)}${sel(e.after)}`;
  };
  L.push(`# Snippet migration: decisions (2026-09-30)`);
  L.push("");
  L.push(`ntex moved from Obsidian's Latex Suite format (~/.config/latex-suite/snippets.js, read through a converter) to native LuaSnip snippets in ~/.config/texnvim/snippets/tex/*.lua, in the style of Gilles Castel's set. This file lists every decision. The same list, with rendered maths and filters, is the **Migration decisions** section of the reference page (Space ? in ntex).`);
  L.push("");
  L.push(`Every example was typed into the old setup (before) and the new one (now) by a headless ntex: before ${d.before_from}; now ${d.after_from}. ‸ marks the cursor. \`tests/run.sh decisions\` types every “now” again.`);
  L.push("");
  L.push(`Roll back the whole migration: \`git -C ~/.config/texnvim checkout pre-luasnip\` (and \`git -C ~/.config/texnvim checkout main\` to come back).`);
  L.push("");
  L.push("| Section | Entries |");
  L.push("| --- | --- |");
  for (const c of d.categories) L.push(`| [${c.title}](#${c.title.toLowerCase().replace(/[^a-z0-9 -]/g, "").replace(/ /g, "-")}) | ${d.counts[c.id]} |`);
  for (const c of d.categories) {
    const list = d.entries.filter((e) => e.category === c.id);
    L.push("");
    L.push(`## ${c.title}`);
    L.push("");
    L.push(c.blurb);
    L.push("");
    if (c.id === "kept") {
      L.push("| Trigger | Output | Where | Origin | Now in | Note |");
      L.push("| --- | --- | --- | --- | --- | --- |");
      for (const e of list) L.push(`| ${code(e.trigger)} | ${code(e.output)} | ${e.where} | ${e.origin} | ${e.file ?? "—"} | ${[e.castel, e.note].filter(Boolean).join(" ")} |`);
      L.push("");
      L.push(`Each was typed in a .tex and a .md file in both setups: identical.`);
      continue;
    }
    if (c.id === "off") {
      L.push("| Trigger | What it would do | Status |");
      L.push("| --- | --- | --- |");
      for (const e of list) L.push(`| ${code(e.trigger)} | ${e.title.replace(/\|/g, "\\|")} | ${e.how_off} |`);
      L.push("");
      L.push(`Each was typed into both setups: off in both (or one of your snippets answers, the same way).`);
      continue;
    }
    for (const e of list) {
      L.push(`### ${e.title}`);
      L.push("");
      L.push(`Trigger: ${code(e.trigger)}${e.file ? ` · now in ${e.file}` : ""}${e.origin ? ` · ${e.origin}` : ""}${e.castel_line ? ` · Castel's tex.snippets line ${e.castel_line}` : ""}${e.dropped_kind ? ` · ${e.dropped_kind}` : ""}`);
      L.push("");
      if (e.category === "call") {
        const dec = DECIDED[e.id];
        if (dec) L.push(`**Decided ${dec.date}:** ${dec.answer}\n`);
        L.push(`**Now:** ${e.now}`);
        if (e.before_text) L.push(`\n**Before:** ${e.before_text}`);
        L.push(`\n**Alternative:** ${e.alternative}`);
        L.push(`\n**Recommendation:** ${e.recommendation}`);
        if (e.side) L.push(`\n${e.side}`);
        if (e.how) L.push(`\n${e.how}`);
      } else if (e.why) {
        L.push(e.why);
      }
      if (e.examples.length) {
        L.push("");
        for (const x of e.examples) L.push(`- ${ex(x)}`);
      }
      L.push("");
    }
  }
  return L.join("\n") + "\n";
}
