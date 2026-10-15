// Snippet side of the reference build: the loaded LuaSnip snippets (as a
// headless ntex lists them, with their metadata from lua/snips/helpers.lua),
// an example job per snippet, and the engine's answer turned into rows.
import crypto from "node:crypto";
import { VISUAL_SAMPLE } from "../content/examples.mjs";

export const CURSOR = "‸";

export function sha(s, n = 16) {
  return crypto.createHash("sha1").update(s).digest("hex").slice(0, n);
}

// ---------------------------------------------------------------------------
// metadata in words

export const ORIGINS = {
  yours: "yours",
  changed: "changed default",
  default: "Latex Suite default",
  castel: "from Castel",
  new: "new",
};

export function whereOf(m) {
  if (m.kind === "visual") return { key: "math", label: "selection in maths" };
  if (m.where === "text") {
    const line = { start: " at the start of a line", "after text": " after other text on the line", "list item": " at the end of a list item" }[m.line_rule] || "";
    return { key: "text", label: "text" + line + (m.ft === "tex" ? " (.tex)" : "") };
  }
  if (m.where === "inline") return { key: "inline", label: "inline maths" };
  if (m.where === "display") return { key: "display", label: "display maths" };
  return { key: "math", label: "any maths" };
}

/** Readable trigger: the pattern's name (helpers `name`) or the trigger itself. */
export function triggerText(m) {
  if (m.kind === "post") return "‹letter›" + m.trig;
  return m.trig;
}

// ---------------------------------------------------------------------------
// examples

const esc = (s) => s.replace(/</g, "<lt>");

/** Body text as it reads once expanded: tabstops show their default text,
 *  and a repeated <n> (a mirror) shows the same text as the first <n:…>. */
export function plainBody(body, captures = []) {
  // escapes first, left to right (as helpers.parse): %% → %, %< → <, %> → >
  // (held as private-use characters so they never read as tabstops or captures)
  const ESC = { "%": "\uE000", "<": "\uE001", ">": "\uE002" };
  body = body.replace(/%([%<>])/g, (_, c) => ESC[c]);
  const defaults = {};
  for (const m of body.matchAll(/<(\d+):([^<>\n]*)>/g)) if (!(m[1] in defaults)) defaults[m[1]] = m[2];
  return body
    .replace(/<(\d*):([^<>\n]*)>/g, "$2")
    .replace(/<(\d*)>/g, (_, n) => (n !== "" && n in defaults ? defaults[n] : ""))
    .replace(/%(\d)/g, (_, k) => captures[+k - 1] ?? "")
    .replace(/[\uE000-\uE002]/g, (c) => ({ "\uE000": "%", "\uE001": "<", "\uE002": ">" })[c]);
}

function contextFor(m) {
  if (m.kind === "visual") return { kind: "inline", before: "$" + CURSOR + VISUAL_SAMPLE + "$" };
  if (m.where === "text") {
    if (m.line_rule === "after text") return { kind: "text", before: "so we get " + CURSOR };
    if (m.line_rule === "list item") return { kind: "text", before: "- item " + CURSOR, ft: "md" };
    return { kind: "text", before: CURSOR };
  }
  if (m.where === "display") return { kind: "display", before: "\\[\n" + CURSOR + "\n\\]" };
  return { kind: "inline", before: "$" + CURSOR + "$" };
}

/**
 * One job per snippet: what to type where, plus what we expect to see.
 * Returns { before, keys, select?, ft, ctx, say, expected, example } or { unverifiable }.
 */
export function makeJob(m) {
  const ctx = contextFor(m);
  const tab = m.how === "tab" ? "<Tab>" : "";
  const ft = m.ft === "markdown" ? "md" : ctx.ft || "tex";
  if (m.kind === "visual") {
    const expected = m.body ? plainBody(m.body).replace("%V", VISUAL_SAMPLE) : null;
    return { before: ctx.before, keys: esc(m.trig), select: [...VISUAL_SAMPLE].length, ft, ctx: ctx.kind, visual: true, say: null, expected: expected && expected.replace(/%V/g, VISUAL_SAMPLE) };
  }
  let typed = m.eg;
  if (!typed) {
    if (m.kind === "post") typed = "x" + m.trig;
    else if (m.kind === "pattern") return { unverifiable: true };
    else typed = m.trig;
  }
  let expected = null;
  if (m.body && !m.computed) {
    if (m.kind === "post") expected = plainBody(m.body, [typed[0]]);
    else if (m.kind !== "pattern") expected = plainBody(m.body);
    if (expected != null && ctx.kind === "inline") expected = expected.replace(/\s+$/, "");
  }
  const say = m.eg || m.kind === "post" ? `\`${m.eg || typed}\`` : null;
  return { before: ctx.before, keys: esc(typed) + tab, ft, ctx: ctx.kind, say, expected, example: typed };
}

// ---------------------------------------------------------------------------
// results

/** byte column → JS string index on a line */
function byteToIndex(line, col) {
  const b = Buffer.from(line, "utf8");
  return b.subarray(0, col).toString("utf8").length;
}

/**
 * Turn the engine's state into { text, marks[], cursor, sel, whole } where
 * text is the part of the buffer the snippet produced (context stripped) and
 * marks are tabstops as string offsets into text.
 */
export function processResult(job, st) {
  if (!st || st.error) return { ok: false, error: st ? st.error : "no result" };
  const lines = st.lines;
  const full = lines.join("\n");
  const lineStart = [];
  let acc = 0;
  for (const l of lines) {
    lineStart.push(acc);
    acc += l.length + 1;
  }
  const off = (row0, col) => lineStart[row0] + byteToIndex(lines[row0] ?? "", col);
  const k = job.before.indexOf(CURSOR);
  let prefix = job.before.slice(0, k);
  let suffix = job.before.slice(k + CURSOR.length);
  if (job.select) suffix = suffix.slice(VISUAL_SAMPLE.length);
  let start = 0, end = full.length;
  if (full.startsWith(prefix) && full.endsWith(suffix) && full.length >= prefix.length + suffix.length) {
    start = prefix.length;
    end = full.length - suffix.length;
  }
  const text = full.slice(start, end);
  const marks = [];
  for (const m of st.marks || []) {
    const a = off(m.from[0], m.from[1]) - start;
    const b = off(m.to[0], m.to[1]) - start;
    if (a < 0 || b > text.length) continue;
    marks.push({ kind: m.kind, k: m.k ?? null, a, b });
  }
  const cursor = off(st.row - 1, st.col) - start;
  // the keys as literal text: if that is all we got, nothing expanded
  const literal = job.keys.replace(/<lt>/g, "<").replace(/<Tab>/g, "");
  const unchanged = !job.select && text.replace(/\s+$/, "") === literal;
  return { ok: true, text, marks, cursor, mode: st.mode, sel: st.sel ?? null, whole: full, start, unchanged };
}
