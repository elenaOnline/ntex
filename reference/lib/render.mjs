// HTML helpers for the reference build: escaping, keycaps, KaTeX, tables.
import katex from "../vendor/katex/katex.mjs";
import "../vendor/katex/contrib/mhchem.mjs"; // \ce, \pu

export const esc = (s) =>
  String(s ?? "").replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

// ---------------------------------------------------------------------------
// KaTeX (pre-rendered at build time; the page only needs the CSS + fonts)

const MACROS = {
  // not in KaTeX; close enough for a preview
  "\\cancelto": "\\overset{#1}{\\cancel{#2}}",
  "\\qedhere": "",
};

export function math(src, { display = false, trust = false } = {}) {
  try {
    return katex.renderToString(src, { displayMode: display, throwOnError: true, strict: "ignore", trust, output: "html", macros: { ...MACROS } });
  } catch {
    return null;
  }
}

/** `$x$`-style source inside prose: `…` → code, $…$ → KaTeX */
export function prose(s) {
  return esc(s).replace(/`([^`]+)`/g, (_, x) => `<code>${x}</code>`);
}

// ---------------------------------------------------------------------------
// keycaps

const NAMED = {
  Esc: "Esc", Enter: "Enter", Tab: "Tab", Space: "Space", Backspace: "⌫", Up: "↑", Down: "↓", Left: "←", Right: "→",
  Ctrl: "Ctrl", Shift: "Shift", Option: "⌥", Cmd: "⌘",
};
const MOD = /^(Ctrl|Shift|Option|Cmd)-(.+)$/;

function cap(t) {
  if (/^F\d+$/.test(t) || NAMED[t]) return `<kbd class="key named" title="${esc(t)}">${esc(NAMED[t] || t)}</kbd>`;
  return `<kbd class="key">${esc(t)}</kbd>`;
}

/** "Ctrl-w v" | [":w", "Enter"] → keycaps */
export function keys(spec) {
  const toks = Array.isArray(spec) ? spec : String(spec).split(" ").filter(Boolean);
  return `<span class="keys">${toks
    .map((t) => {
      const parts = [];
      let rest = t;
      let m;
      while ((m = MOD.exec(rest))) {
        parts.push(m[1]);
        rest = m[2];
      }
      if (parts.length === 0) return cap(t);
      parts.push(rest);
      return `<span class="chord">${parts.map(cap).join('<span class="plus" aria-hidden="true">+</span>')}</span>`;
    })
    .join(" ")}</span>`;
}

/** Neovim lhs (as nvim_get_keymap reports it) → our key spec */
export function lhsToSpec(lhs, leader = " ", localleader = "\\") {
  const out = [];
  let s = lhs;
  if (leader && s.startsWith(leader) && leader === " ") {
    out.push("Space");
    s = s.slice(1);
  }
  const NAMES = { CR: "Enter", BS: "Backspace", Tab: "Tab", Esc: "Esc", Space: "Space", Down: "Down", Up: "Up", Left: "Left", Right: "Right" };
  const special = /^<([^>]+)>/;
  let typed = "";
  const flush = () => {
    if (typed) out.push(typed);
    typed = "";
  };
  while (s.length) {
    const m = special.exec(s);
    if (m && m[1] !== "lt") {
      flush();
      const parts = m[1].split("-");
      const key = parts.pop();
      const mods = parts.map((p) => ({ C: "Ctrl", S: "Shift", M: "Option", A: "Option", D: "Cmd" })[p] || p);
      let k = NAMES[key] || key;
      if (mods.length && /^[A-Z]$/.test(k) && !mods.includes("Shift")) k = k.toLowerCase();
      out.push([...mods, k].join("-"));
      s = s.slice(m[0].length);
    } else if (m) {
      typed += "<";
      s = s.slice(m[0].length);
    } else {
      typed += s[0];
      s = s.slice(1);
    }
  }
  flush();
  return out;
}

// ---------------------------------------------------------------------------
// snippet output with tabstops

/**
 * Source view of an example: tabstops as small numbered boxes, the one the
 * cursor is in highlighted, the end of the snippet as a thin caret.
 */
export function sourceWithMarks(text, marks, cursor, { sel = null, mode = "i" } = {}) {
  const tabs = marks.filter((m) => m.kind === "tab" || m.kind === "mirror");
  const hasTabs = marks.some((m) => m.kind === "tab");
  const pts = [];
  for (const m of tabs) pts.push(m);
  const exit = marks.find((m) => m.kind === "exit");
  // active tabstop: the one containing the cursor
  let active = null;
  for (const m of tabs) if (m.kind === "tab" && cursor >= m.a && cursor <= m.b && (active === null || m.k < active.k)) active = m;
  const events = [];
  for (const m of pts) events.push(m);
  events.sort((x, y) => x.a - y.a || x.b - y.b);
  let out = "";
  let pos = 0;
  const caret = (label) => `<span class="caret" title="${label}" aria-label="${label}"></span>`;
  let caretDone = false;
  const place = (upto) => {
    let chunk = text.slice(pos, upto);
    if (!caretDone && !active && cursor >= pos && cursor <= upto && !(mode === "s" && sel)) {
      out += esc(chunk.slice(0, cursor - pos)) + caret("cursor");
      chunk = chunk.slice(cursor - pos);
      caretDone = true;
    }
    out += esc(chunk);
    pos = upto;
  };
  for (const m of events) {
    if (m.a < pos) continue;
    place(m.a);
    const inner = esc(text.slice(m.a, m.b));
    const cls = m.kind === "mirror" ? "ts mirror" : m === active ? "ts now" : "ts";
    const label = m.kind === "mirror" ? "copy of a tabstop" : `tabstop ${m.k}${m === active ? " (cursor)" : ""}`;
    out += `<span class="${cls}" title="${label}">${inner || "&#8203;"}${m.kind === "tab" && hasTabs ? `<sup>${m.k}</sup>` : ""}</span>`;
    pos = m.b;
  }
  place(text.length);
  if (exit && hasTabs && exit.a >= text.length - 0) {
    out += `<span class="end" title="end of the snippet: the last Tab lands here">⇥</span>`;
  }
  if (mode === "s" && sel && !active) out = `<span class="selected" title="stays selected: type another visual snippet key to wrap again">${out}</span>`;
  return out;
}

/** LaTeX for the preview: tabstops become \htmlClass boxes */
export function previewTex(text, marks) {
  const tabs = marks.filter((m) => m.kind === "tab" || m.kind === "mirror").sort((x, y) => y.a - x.a);
  let s = text;
  for (const m of tabs) {
    const inner = s.slice(m.a, m.b);
    const box = `\\htmlClass{tsbox}{${inner ? inner : "\\phantom{0}"}}`;
    s = s.slice(0, m.a) + box + s.slice(m.b);
  }
  return s;
}

export function renderPreview(text, marks, display) {
  const src = text.replace(/^\s+|\s+$/g, "");
  if (!src) return null;
  const withBoxes = previewTex(text, marks).trim();
  return math(withBoxes, { display, trust: true }) || math(src, { display }) || null;
}
