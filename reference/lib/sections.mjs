// The page's sections. Each returns { id, title, html, search: [entries] }.
// Search entries: { a: anchor id, t: title, d: detail, k: extra keywords }.
import fs from "node:fs";
import { esc, keys, math, prose } from "./render.mjs";
import { latexStyle } from "../content/latex.mjs";

const code = (s) => `<code>${esc(s)}</code>`;
const slug = (s) => String(s).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
/** "a letter, then `hat`" → text with the backticked parts as keycaps */
export const sayHtml = (s) => esc(s).replace(/`([^`]+)`/g, (_, x) => `<kbd class="key">${x}</kbd>`);
const plainSay = (s) => String(s || "").replace(/`/g, "");

// ---------------------------------------------------------------------------
// generic table

/**
 * cols: [{ key, label, sort?: true, cls? }]
 * rows: [{ id, cells: {key: html}, sort: {key: value}, filters: {dim: "v1 v2"}, cls? }]
 * filters: [{ dim, label, options: [[value, label, count?]] }]
 */
export function dataTable({ id, caption, cols, rows, filters = [], toggles = [], localSearch = true, note = "" }) {
  const chips = filters
    .map(
      (f) => `<div class="chips" role="group" aria-label="${esc(f.label)}"><span class="chips-label">${esc(f.label)}</span>${f.options
        .map(([v, l, n]) => `<button type="button" class="chip" data-dim="${esc(f.dim)}" data-value="${esc(v)}" aria-pressed="false">${l}${n != null ? ` <span class="n">${n}</span>` : ""}</button>`)
        .join("")}</div>`,
    )
    .join("");
  const tog = toggles
    .map((t) => `<button type="button" class="chip toggle" data-dim="${esc(t.dim)}" data-value="${esc(t.value)}" aria-pressed="false">${t.label}</button>`)
    .join("");
  const sortable = cols.filter((c) => c.sort);
  const sortSelect = sortable.length
    ? `<label class="sort-narrow">Sort <select data-sort-select>${[`<option value="">as listed</option>`, ...sortable.flatMap((c) => [`<option value="${c.key}:asc">${esc(c.label)} ↑</option>`, `<option value="${c.key}:desc">${esc(c.label)} ↓</option>`])].join("")}</select></label>`
    : "";
  const search = localSearch
    ? `<label class="local-search"><span class="vh">Filter ${esc(caption)}</span><input type="search" data-local placeholder="Filter this table" autocomplete="off" spellcheck="false"></label>`
    : "";
  const head = cols
    .map((c) =>
      c.sort
        ? `<th scope="col" class="${c.cls || ""}" aria-sort="none"><button type="button" data-sort="${c.key}">${esc(c.label)}<span class="arrow" aria-hidden="true"></span></button></th>`
        : `<th scope="col" class="${c.cls || ""}">${esc(c.label)}</th>`,
    )
    .join("");
  const body = rows
    .map((r, i) => {
      const data = [`data-i="${i}"`];
      for (const [k, v] of Object.entries(r.sort || {})) data.push(`data-s-${k}="${esc(v)}"`);
      for (const [k, v] of Object.entries(r.filters || {})) data.push(`data-f-${k}="${esc(v)}"`);
      const tds = cols.map((c) => `<td class="${c.cls || ""}" data-label="${esc(c.label)}"><div class="cell">${r.cells[c.key] ?? ""}</div></td>`).join("");
      return `<tr id="${esc(r.id)}" class="${r.cls || ""}" ${data.join(" ")}>${tds}</tr>`;
    })
    .join("\n");
  return `<div class="table-block" data-table="${id}">
<div class="table-tools">${search}${sortSelect}<p class="count" aria-live="polite"><span data-count>${rows.length}</span> of ${rows.length}</p></div>
${chips || tog ? `<div class="filters">${chips}${tog ? `<div class="chips" role="group" aria-label="Show only"><span class="chips-label">Show only</span>${tog}</div>` : ""}<button type="button" class="chip clear" data-clear hidden>Clear filters</button></div>` : ""}
${note}
<div class="table-wrap" tabindex="0" role="region" aria-label="${esc(caption)}"><table class="data" id="${id}"><caption class="vh">${esc(caption)}</caption><thead><tr>${head}</tr></thead><tbody>
${body}
</tbody></table></div>
<p class="empty" hidden>Nothing matches. <button type="button" class="linklike" data-clear>Clear filters</button></p>
</div>`;
}

function counts(rows, dim) {
  const m = new Map();
  for (const r of rows) for (const v of String(r.filters[dim] || "").split(" ").filter(Boolean)) m.set(v, (m.get(v) || 0) + 1);
  return m;
}

// ---------------------------------------------------------------------------
// 1. Start here

export function startSection(d) {
  const ex = (trig) => d.exampleByTrigger(trig);
  const pick = ["mk", "‹letter›‹digit›", "//", "sr", "hat", "sq", "RR", "uu", "inn", "->", "\\sum", "dint", "lim", "pmat", "beg", "text"];
  const cards = pick
    .map((t) => ex(t))
    .filter(Boolean)
    .map((r) => `<li><a href="#snippets" data-go="${r.id}" class="mini">${r.miniTypeHtml}<span class="arrow-to" aria-hidden="true">→</span><span class="mini-out">${r.outHtml}</span></a></li>`)
    .join("");
  const delay = d.prefs.live_pdf_delay_ms ?? 800;
  const viewer = d.prefs.pdf_viewer ?? "tdf";
  const html = `
<p class="lede">The whole loop, in order. Everything here is exactly what your setup does today; the other sections go deeper.</p>
<ol class="loop">
<li><h3>Open a file</h3>
<p>In a cmux terminal: ${keys(["ntex lecture-03.tex", "Enter"])}. Any path works; a missing folder is created when you save. A new or empty <code>.tex</code> file starts from your template (article, amsmath, theorem environments).</p>
<p>Throwaway notes: ${keys(["ntex", "Enter"])} then ${keys([":Scratch", "Enter"])} (or ${keys("Space s")}), which opens <code>~/scratch/scratch-<var>date-time</var>.tex</code>.</p></li>
<li><h3>Type</h3>
<p>Neovim opens in <em>normal mode</em>. Press ${keys("i")} to type (the bottom line shows <code>-- INSERT --</code>), ${keys("Esc")} to stop. ${keys("Ctrl-s")} saves (normal, insert and visual mode; not on the ${keys(":")} line), ${keys("Ctrl-z")} undoes even while typing.</p></li>
<li><h3>Maths: the snippets you'll use most</h3>
<p>${keys("mk")} opens inline maths ${code("$…$")}, ${keys("dm")} a display block: ${code("\\[ … \\]")} in a .tex file, ${code("$$ … $$")} in a .md file. Inside maths, triggers expand as you type; the ones marked Tab need ${keys("Tab")}. ${keys("Tab")} then jumps to the next empty slot and out of brackets; ${keys("Shift-Tab")} goes back.</p>
<ul class="minis">${cards}</ul>
<p><a href="#snippets">All ${d.snippetCount} snippets</a>, with a verified example for each.</p></li>
<li><h3>Watch the PDF</h3>
<p>Pause typing for ${(delay / 1000).toLocaleString("en-GB")} s and the file saves and recompiles by itself${d.prefs.live_pdf ? "" : " (live PDF is <strong>off</strong> in prefs.lua right now)"}. ${keys("Esc")} then ${keys("\\lv")} shows the PDF in a cmux pane to the right (${code(viewer)}); ${keys("\\ls")} opens Skim at the cursor. A failed compile keeps the last good PDF; ${keys("\\le")} shows the errors.</p></li>
<li><h3>Save and close</h3>
<p>${keys("Ctrl-s")} saves. ${keys(["Esc", ":q", "Enter"])} closes (it asks if something is unsaved); ${keys([":q!", "Enter"])} closes without saving.</p></li>
<li><h3>When stuck</h3>
<ul>
<li>Weird mode? ${keys("Esc")} ${keys("Esc")}. Wrong change? ${keys("Ctrl-z")} or ${keys("u")}.</li>
<li>A snippet didn't fire: are you in maths, at the start of a word? ${keys("Space r")} (:SnippetReport) lists what loaded. See <a href="#troubleshooting">Troubleshooting</a>.</li>
<li>Compile problems: ${keys("\\le")}.</li>
<li>This page: ${keys("Space ?")} or ${keys([":Reference", "Enter"])}. Press ${keys("/")} here to search it.</li>
</ul></li>
</ol>
<p class="aside">Then: the ${d.firstCount} keys marked <span class="tag first">first</span> in <a href="#keys" data-go-filter="keys:first:1">Vim keys</a>.</p>`;
  return { id: "start", title: "Start here", html, search: [
    { a: "start", t: "Daily loop: open, type, maths, PDF, save, stuck", d: "ntex file.tex, :Scratch, i, Esc, Ctrl-s, \\lv, :q", k: "start begin how to" },
  ] };
}

// ---------------------------------------------------------------------------
// 2. Snippets

export function snippetsSection(d) {
  const rows = d.snippetRows;
  const groups = [...new Set(rows.map((r) => r.group))];
  const fr = rows.map((r) => ({ filters: { group: slug(r.group), where: r.whereKey, how: r.howKey, origin: r.origin } }));
  const gc = counts(fr, "group");
  const wc = counts(fr, "where");
  const hc = counts(fr, "how");
  const oc = counts(fr, "origin");
  const WHERE = [["math", "any math"], ["inline", "inline math"], ["display", "display math"], ["text", "text"]];
  const table = dataTable({
    id: "tbl-snippets",
    caption: "Your snippets",
    cols: [
      { key: "type", label: "Type", sort: true, cls: "c-type" },
      { key: "get", label: "You get", sort: true, cls: "c-get" },
      { key: "where", label: "Where", sort: true, cls: "c-where" },
      { key: "how", label: "How", sort: true, cls: "c-how" },
      { key: "group", label: "File", sort: true, cls: "c-group" },
      { key: "notes", label: "Notes", cls: "c-notes" },
    ],
    rows: rows.map((r) => ({
      id: r.id,
      cls: r.flag ? "flagged" : "",
      cells: {
        type: r.typeHtml,
        get: `<div class="out">${r.outHtml}</div>${r.previewHtml ? `<div class="preview">${r.previewHtml}</div>` : ""}`,
        where: esc(r.whereLabel),
        how: r.howHtml,
        group: esc(r.group),
        notes: r.notesHtml,
      },
      sort: { type: r.sortType, get: r.sortGet, where: r.whereLabel, how: r.howKey, group: String(groups.indexOf(r.group)).padStart(3, "0") + r.group },
      filters: { group: slug(r.group), where: r.whereKey, how: r.howKey, origin: r.origin },
    })),
    filters: [
      { dim: "where", label: "Where", options: WHERE.filter(([v]) => wc.get(v)).map(([v, l]) => [v, l, wc.get(v)]) },
      { dim: "how", label: "How", options: [["auto", "by itself", hc.get("auto")], ["tab", "Tab", hc.get("tab")], ["visual", "select, then key", hc.get("visual")]].filter((o) => o[2]) },
      { dim: "origin", label: "Origin", options: [["yours", "yours", oc.get("yours")], ["changed", "changed default", oc.get("changed")], ["default", "Latex Suite default", oc.get("default")], ["castel", "from Castel", oc.get("castel")], ["new", "new", oc.get("new")]].filter((o) => o[2]) },
      { dim: "group", label: "Group", options: groups.map((g) => [slug(g), esc(g), gc.get(slug(g))]) },
    ],
  });
  const off = d.turnedOff.map((t) => `<tr><td>${t.trigHtml}</td><td>${t.outHtml}</td><td>${esc(t.how)}</td></tr>`).join("");
  const flagged = rows.filter((r) => r.flag);
  const html = `
<p class="lede">Every snippet ntex loaded from <code>${esc(d.paths.snippetsShort)}/tex/*.lua</code>, file by file in the order of the lines, each with an example that was typed into ntex (headless) to see what really comes out. Boxes are tabstops: <span class="ts now">&#8203;<sup>1</sup></span> is where the cursor lands, ${keys("Tab")} moves to the next, <span class="end">⇥</span> is where the last ${keys("Tab")} leaves you. ${keys("Space e")} opens a file; a line there reads ${code('math("uu", [[\\cup]], "yours")')}: trigger, output, origin.</p>
<div class="explain">
<details><summary>How to read a row</summary>
<ul>
<li><strong>Type</strong>: the keys. Snippets marked <span class="how auto">by itself</span> expand the moment the last key is typed; <span class="how tab">Tab</span> ones need ${keys("Tab")} after the trigger.</li>
<li><strong>Where</strong>: “any math” works in ${code("$…$")}, ${code("\\[…\\]")} and equation environments; “text” only outside maths. Nothing fires inside ${code("\\text{…}")} except snippets made for it.</li>
<li><strong>Word starts</strong>: a trigger that starts with a letter fires only at the start of a word, never inside a ${code("\\command")}: ${code("A uu")} gives ${code("A \\cup")}, ${code("\\subset")} types as written. Suffix triggers (${code("sr")}, ${code("_")}, ${code("dag")}, …) and ‹letter› triggers (${code("xhat")} → ${code("\\hat{x}")}, ${code("x2")} → ${code("x_{2}")}) follow a letter on purpose.</li>
<li><strong>Pattern triggers</strong> (like ‹letter›‹digit›) match a Lua pattern; the example shows one way to type it.</li>
<li><strong>Visual snippets</strong>: select text (mouse, or Shift+arrows; from ${keys("v")} press ${keys("Ctrl-g")}), then type the key: ${keys("S")} wraps it in ${code("\\sqrt{ }")}. The result stays selected, so ${keys("(")} then ${keys("S")} gives ${code("\\sqrt{ (…) }")}. Outside maths the key just replaces the selection.</li>
<li><strong>Computed</strong> snippets: ${code("x2")} → ${code("x_{2}")} (another digit extends it: ${code("x24")} → ${code("x_{24}")}), ${code("iden3")} → a 3×3 identity matrix, ${code("dm")} in a list item.</li>
<li><strong>Order</strong>: higher priority first, then longer triggers first. ${code("inn")} gives ${code("\\in")}: ${code("nn")} can't fire after the ${code("i")}.</li>
</ul></details>
</div>
${flagged.length ? `<p class="warn" role="note">${flagged.length} example${flagged.length > 1 ? "s" : ""} did not come out as the snippet's text suggests: marked <span class="tag flag">check</span> below (also in <code>reference/build-report.txt</code>).</p>` : ""}
${table}
<details class="turned-off" id="turned-off"><summary><h3>Still turned off <span class="n">${d.turnedOff.length}</span></h3><span class="sub">Latex Suite defaults you had commented out or removed, and your own commented-out ones; each was typed to check it stays off</span></summary>
<div class="table-wrap"><table class="data simple"><thead><tr><th scope="col">Trigger</th><th scope="col">Would do</th><th scope="col">Was</th></tr></thead><tbody>${off}</tbody></table></div>
<p>To have one back, add it as a line to a snippet file (${keys("Space e")}); the <a href="#decisions" data-go-filter="decisions:cat:off">Migration decisions</a> section shows each with its example.</p>
</details>`;
  const search = rows.map((r) => ({ a: r.id, t: r.searchTitle, d: r.searchDetail, k: `${r.group} ${r.whereLabel} ${r.origin} snippet` }));
  for (const t of d.turnedOff) search.push({ a: "turned-off", t: `${t.trigText} (still off)`, d: t.outText, k: "disabled commented default turned off" });
  return { id: "snippets", title: "Your snippets", html, search };
}

// ---------------------------------------------------------------------------
// 2b. Migration decisions (snippets/decisions.json)

export function decisionsSection(d) {
  const log = d.decisions;
  if (!log) {
    return { id: "decisions", title: "Migration decisions", html: `<p class="warn">snippets/decisions.json is missing: run <code>node ~/.config/texnvim/tools/migration/decisions.mjs</code>.</p>`, search: [] };
  }
  const cats = log.categories;
  const catTitle = Object.fromEntries(cats.map((c) => [c.id, c.title]));
  const caret = (t) => esc(t).replace(/‸/g, '<span class="caret" aria-label="cursor"></span>');
  const shotHtml = (x, label) => {
    if (x.text == null) return `<div class="dx-shot"><span class="ba-l">${label}</span><span class="muted">(no result: ${esc(x.error || "")})</span></div>`;
    const pv = d.decisionPreview(x.text);
    const sel = x.sel ? ` <span class="dx-sel">selected: <code>${esc(x.sel)}</code></span>` : "";
    return `<div class="dx-shot"><span class="ba-l">${label}</span><code class="src">${caret(x.text)}</code>${sel}${pv.tex ? `<div class="dx-tex">${pv.tex}</div>` : ""}</div>`;
  };
  const exHtml = (x) => {
    const where = x.select ? `select <code>${esc([...x.before.replace("‸", "")].slice(x.before.indexOf("‸"), x.before.indexOf("‸") + x.select).join(""))}</code> in <code>${caret(x.before.replace(/‸/, "‸"))}</code>` : `in <code>${caret(x.before)}</code>`;
    const head = `<div class="dx-keys"><span class="dx-ft">.${x.ft === "md" ? "md" : "tex"}</span> ${where} type ${d.keysHtml(x.keys)}</div>`;
    const same = x.old.text === x.after.text && (x.old.sel ?? null) === (x.after.sel ?? null);
    const body = same ? `<div class="dx-ba one">${shotHtml(x.after, "before = now")}</div>` : `<div class="dx-ba">${shotHtml(x.old, "before")}${shotHtml(x.after, "now")}</div>`;
    return `<div class="dx${same ? " same" : ""}">${head}${body}</div>`;
  };
  const p = (label, text) => (text ? `<p><strong>${label}</strong> ${prose(text)}</p>` : "");
  const rowFor = (e) => {
    const tags = e.category === "call"
      ? [e.decided ? `<span class="tag cat-kept" title="you decided ${esc(e.decided.date)}">decided</span>` : `<span class="tag call" title="needs your call">needs your call</span>`]
      : [`<span class="tag cat-${e.category}">${esc(catTitle[e.category])}</span>`];
    if (e.origin) tags.push(`<span class="tag ${e.origin === "yours" ? "yours" : "plain"}">${esc(e.origin)}</span>`);
    let text = "";
    if (e.category === "call") {
      text = (e.decided ? `<p class="rec"><strong>Your call (${esc(e.decided.date)}):</strong> ${prose(e.decided.answer)}</p>` : "") + p("Now:", e.now) + p("Before:", e.before_text) + p("Alternative:", e.alternative) + `<p class="rec"><strong>Recommendation:</strong> ${prose(e.recommendation)}</p>` + (e.side ? `<p>${prose(e.side)}</p>` : "") + (e.how ? `<p class="muted">${prose(e.how)}</p>` : "");
    } else if (e.category === "kept") {
      text = `<p>${code(e.output)} <span class="muted">· ${esc(e.where)}</span></p>` + (e.castel ? `<p class="muted">${esc(e.castel)}</p>` : "") + (e.note ? `<p class="muted">${prose(e.note)}</p>` : "");
    } else if (e.category === "off") {
      text = `<p class="muted">${esc(e.how_off)}</p>`;
    } else {
      text = (e.castel_does && e.category !== "castel-new" ? `<p>Castel's: ${esc(e.castel_does)}</p>` : "") + (e.why ? `<p>${prose(e.why)}</p>` : "");
    }
    const where = [e.file ? `now in ${esc(e.file)}` : "", e.castel_line ? `Castel's tex.snippets line ${e.castel_line}` : "", e.dropped_kind ? esc(e.dropped_kind) : "", e.old_line ? `snippets.js line ${e.old_line}` : ""].filter(Boolean).join(" · ");
    const exs = e.examples || [];
    const shown = exs.slice(0, e.category === "change" ? 4 : 3);
    const more = exs.slice(shown.length);
    const exCell = shown.map(exHtml).join("") + (more.length ? `<details class="dx-more"><summary>${more.length} more example${more.length > 1 ? "s" : ""}</summary>${more.map(exHtml).join("")}</details>` : "") || `<span class="muted">(not a typing change)</span>`;
    const differs = exs.some((x) => x.old.text !== x.after.text);
    return {
      id: "d-" + e.id,
      cls: e.category === "call" && !e.decided ? "call" : "",
      cells: {
        what: `<div class="d-tags">${tags.join(" ")}</div><h4 class="d-title">${esc(e.title)}</h4><p class="d-trig">${code(e.trigger)}${where ? ` <span class="muted">${where}</span>` : ""}</p>${text}`,
        ex: exCell,
      },
      sort: { what: String(cats.findIndex((c) => c.id === e.category)).padStart(2, "0") + e.title },
      filters: { cat: e.category, differs: differs ? "1" : "0" },
    };
  };
  const rows = log.entries.map(rowFor);
  const n = log.counts;
  const table = dataTable({
    id: "tbl-decisions",
    caption: "Migration decisions",
    cols: [
      { key: "what", label: "Decision", sort: true, cls: "c-what" },
      { key: "ex", label: "Typed before (Latex Suite) and now (LuaSnip)", cls: "c-ex" },
    ],
    rows,
    filters: [{ dim: "cat", label: "Show", options: cats.map((c) => [c.id, `${c.id === "call" ? "⚑ " : ""}${esc(c.title)}`, n[c.id]]) }],
    toggles: [{ dim: "differs", value: "1", label: "only where before ≠ now" }],
  });
  const calls = log.entries.filter((e) => e.category === "call" && !e.decided);
  const done = log.entries.filter((e) => e.category === "call" && e.decided);
  const html = `
<p class="lede">On 2026-09-30 ntex moved from Obsidian's Latex Suite format to native LuaSnip snippets (<code>snippets/tex/*.lua</code>). Every decision of that move is here. Each example was typed into the <strong>old</strong> setup (before) and the <strong>new</strong> one (now) by a headless ntex; <span class="caret" aria-label="cursor"></span> is the cursor.</p>
<div class="d-summary">
<p><strong>${calls.length} still need${calls.length === 1 ? "s" : ""} your call</strong>${calls.length ? ` (marked <span class="tag call">needs your call</span>; what is in place now is the recommendation): ${calls.map((e) => `<a href="#decisions" data-go="d-${e.id}">${esc(e.title)}</a>`).join(" · ")}` : ""}.${done.length ? ` Decided: ${done.map((e) => `<a href="#decisions" data-go="d-${e.id}">${esc(e.title)}</a>`).join(" · ")} (answers in <code>snippets/decided.json</code>).` : ""}</p>
<p>${n.change} behaviour changes to your snippets (mostly: no longer fires inside a <code>\\command</code>), ${n.kept} kept exactly as they were, ${n["castel-new"]} new from Castel (veto any by deleting its line), ${n["castel-differs"]} where Castel differs and yours stays, ${n.dropped} dropped, ${n.off} still turned off.</p>
<p class="muted">The same list as text: <code>~/.config/texnvim/snippets/DECISIONS.md</code>. Roll the whole move back: <code>git -C ~/.config/texnvim checkout pre-luasnip</code>. <code>tests/run.sh decisions</code> types every “now” again.</p>
</div>
${table}`;
  const search = log.entries.map((e) => ({ a: "d-" + e.id, t: `${e.title}`, d: `${catTitle[e.category]}: ${e.trigger}`, k: `decision migration ${e.category} ${e.trigger}` }));
  return { id: "decisions", title: "Migration decisions", html, search };
}

// ---------------------------------------------------------------------------
// 3. Maths typing features (settings: prefs.lua latex = { … })

export function featuresSection(d) {
  const s = d.latex || {};
  const lua = (x) => (Array.isArray(x) ? `{ ${x.map(lua).join(", ")} }` : typeof x === "string" ? JSON.stringify(x).replace(/\\t/g, "\\t") : String(x));
  const v = (k) => `<span class="setting"><code>${esc(k)}</code> = <code>${esc(k === "conceal" ? String(!!d.prefs.conceal) : lua(s[k]))}</code></span>`;
  const f = (id) => d.featureHtml(id);
  const envs = s.matrix_envs || [];
  const texEnvs = s.matrix_envs_tex || [];
  const cards = [
    {
      id: "feat-autofraction", title: "Auto-fraction: <kbd class=\"key\">/</kbd>",
      text: `In maths, typing ${code("/")} turns what is before it into a fraction's numerator. It takes back to the last space or ${code(s.autofraction_breaking_chars ? "+ - =" : "")} (breaking characters), and a bracketed group as a whole, dropping its outer brackets. Inside ${code("^{…}")} and ${code("\\pu{…}")} it stays a plain slash. ${code("//")} gives an empty fraction.`,
      settings: ["autofraction", "autofraction_symbol", "autofraction_breaking_chars", "autofraction_excluded"],
      ex: ["af-simple", "af-group", "af-excluded"],
    },
    {
      id: "feat-tabout", title: "Tabout: <kbd class=\"key named\">Tab</kbd>",
      text: `When no snippet is waiting, ${keys("Tab")} steps past the next closing ${code(")")} ${code("]")} ${code("}")} ${code("\\rangle")} … and, at the end of the maths, out of ${code("$…$")}. Typing a closing bracket that is already there steps over it instead of doubling it.`,
      settings: ["tabout", "tabout_closing", "tabout_exit_only_at_eol"],
      ex: ["tab-brace", "tab-dollar", "close-step"],
    },
    {
      id: "feat-matrix", title: "Matrix keys: <kbd class=\"key named\">Tab</kbd> <kbd class=\"key named\">Enter</kbd> <kbd class=\"key named\">Shift</kbd>+<kbd class=\"key named\">Enter</kbd>",
      text: `Inside ${envs.map(code).join(", ")}${texEnvs.length ? `, and in .tex files also ${texEnvs.map(code).join(", ")}` : ""}: ${keys("Tab")} types ${code(" & ")} (next column), ${keys("Enter")} types ${code(" \\\\")} and starts the next row, ${keys("Shift-Enter")} leaves the environment.`,
      settings: ["matrix_shortcuts", "matrix_envs", "matrix_envs_tex"],
      ex: ["mx-tab", "mx-enter", "mx-exit", "mx-align"],
    },
    {
      id: "feat-enlarge", title: "Auto-enlarging brackets",
      text: `After a snippet types one of the trigger commands, brackets around it become ${code("\\left( … \\right)")} so they grow with the contents.`,
      settings: ["enlarge_brackets", "enlarge_triggers"],
      ex: ["enlarge"],
    },
    {
      id: "feat-dollar", title: "Deleting an empty <code>$…$</code> pair",
      text: `${keys("Backspace")} between two dollars with nothing inside deletes both.`,
      settings: ["auto_delete_dollar"],
      ex: ["bs-dollar"],
    },
    {
      id: "feat-visual", title: "Visual snippets",
      text: `Select text in maths (mouse drag, or Shift+arrows; from ${keys("v")} press ${keys("Ctrl-g")} first), then type ${keys("S")} (square root), ${keys("C")} (cancel), ${keys("U")} (underbrace), ${keys("O")} (overbrace), ${keys("B")} (underset), ${keys("K")} (cancel to), ${keys("(")} ${keys("[")} ${keys("{")} (brackets). The result stays selected so you can wrap again. Outside maths the key replaces the selection as usual.`,
      settings: [],
      ex: ["vis-S", "vis-paren"],
    },
    {
      id: "feat-conceal", title: "Conceal and <kbd class=\"key named\">F2</kbd>",
      text: `Pretty symbols in the text: ${code("\\alpha")} shows as α, ${code("\\mathbb{R}")} as ℝ, ${code("x_{i}")} as xᵢ${d.prefs.conceal_scripts === false ? " (sub/superscripts are kept as typed: conceal_scripts = false)" : ""}. The line the cursor is on always shows the real source. ${keys("F2")} (or ${keys("Space c")}) switches it on and off. It starts ${d.concealStart ? "<strong>on</strong>" : "<strong>off</strong>"} (prefs.lua ${code("conceal = " + String(!!d.prefs.conceal))}). Concealed symbols use the maths colour (lua/config/theme.lua).`,
      settings: ["conceal"],
      static: [["\\alpha + \\beta \\in \\mathbb{R}", "α + β ∈ ℝ"]],
    },
    {
      id: "feat-completion", title: "Completion: <kbd class=\"key\">Ctrl</kbd>+<kbd class=\"key named\">Space</kbd>",
      text: `The menu opens by itself after a backslash, inside ${code("\\ref{")}, ${code("\\eqref{")}, ${code("\\cite{")}, ${code("\\begin{")} …, and for file paths; ${keys("Ctrl-Space")} opens it anywhere. Pick with ${keys("Down")} ${keys("Up")} or ${keys("Ctrl-n")} ${keys("Ctrl-p")}, accept with ${keys("Enter")} (only after picking, so ${keys("Enter")} never eats a matrix row) or ${keys("Ctrl-y")}; ${keys("Ctrl-e")} closes it. ${keys("Tab")} never accepts a completion: it belongs to snippets. Labels, citations and your ${code("\\newcommand")}s come from texlab.`,
      settings: [],
    },
  ];
  const html = `
<p class="lede">What ntex does besides snippets (these came from Obsidian's Latex Suite), with your current settings from prefs.lua ${code("latex = { … }")}. Examples are typed into ntex headless; <span class="caret" aria-label="cursor"></span> is the cursor.</p>
<div class="cards">
${cards
  .map(
    (c) => `<article class="card" id="${c.id}"><h3>${c.title}</h3><p>${c.text}</p>${c.settings.length ? `<p class="settings">${c.settings.map(v).join(" ")}</p>` : ""}${(c.ex || []).map(f).join("")}${(c.static || [])
      .map(([a, b]) => `<div class="ba"><div><span class="ba-l">source</span><code class="src">${esc(a)}</code></div><div><span class="ba-l">shown</span><code class="src">${esc(b)}</code></div></div>`)
      .join("")}</article>`,
  )
  .join("\n")}
</div>`;
  const search = cards.map((c) => ({ a: c.id, t: c.title.replace(/<[^>]+>/g, "").replace(/\s+/g, " "), d: c.text.replace(/<[^>]+>/g, "").slice(0, 140), k: "feature latex suite prefs " + c.settings.join(" ") }));
  return { id: "features", title: "Maths typing features", html, search };
}

// ---------------------------------------------------------------------------
// 4. Vim keys

export function keysSection(d) {
  const rows = d.keyRows;
  const cats = Object.entries(d.categories).filter(([k]) => rows.some((r) => r.cat === k));
  const cc = counts(rows.map((r) => ({ filters: { cat: r.cat } })), "cat");
  const MODES = [["n", "Normal"], ["i", "Insert"], ["v", "Visual"], ["s", "Select (snippet placeholder)"], ["o", "after d/c/y"], ["c", "Command line"]];
  const SRC = ["Vim", "ntex", "VimTeX", "macOS keys"];
  const sc = counts(rows.map((r) => ({ filters: { src: slug(r.src) } })), "src");
  const table = dataTable({
    id: "tbl-keys",
    caption: "Vim keys",
    cols: [
      { key: "keys", label: "Keys", sort: true, cls: "c-keys" },
      { key: "mode", label: "Mode", sort: true, cls: "c-mode" },
      { key: "what", label: "What it does", sort: true, cls: "c-what" },
      { key: "cat", label: "Category", sort: true, cls: "c-cat" },
      { key: "src", label: "Source", sort: true, cls: "c-src" },
    ],
    rows: rows.map((r) => ({
      id: r.id,
      cells: {
        keys: `${keys(r.k)}${r.first ? ` <span class="tag first" title="learn these first">first</span>` : ""}`,
        mode: esc(r.modeLabel),
        what: prose(r.what),
        cat: esc(d.categories[r.cat] || r.cat),
        src: `<span class="src-tag src-${slug(r.src)}">${esc(r.src)}</span>`,
      },
      sort: { keys: r.kText, mode: r.modeLabel, what: r.what, cat: String(Object.keys(d.categories).indexOf(r.cat)).padStart(2, "0"), src: r.src },
      filters: { cat: r.cat, mode: r.modes.join(" "), src: slug(r.src), first: r.first ? "1" : "0" },
    })),
    filters: [
      { dim: "cat", label: "Category", options: cats.map(([k, l]) => [k, esc(l), cc.get(k)]) },
      { dim: "mode", label: "Mode", options: MODES },
      { dim: "src", label: "Source", options: SRC.filter((s) => sc.get(slug(s))).map((s) => [slug(s), s, sc.get(slug(s))]) },
    ],
    toggles: [{ dim: "first", value: "1", label: `learn these first (${rows.filter((r) => r.first).length})` }],
  });
  const html = `
<p class="lede">Vim's own keys plus everything your config adds. The ntex, VimTeX and macOS rows come from the live keymap of ntex, so they are exactly what is mapped. Relative line numbers are on: the number beside a line is how far away it is, so ${keys("5j")} or ${keys("d3j")} reaches it. Leader is ${keys("Space")}; VimTeX's keys start with ${keys("\\")} (local leader).</p>
${table}`;
  const search = rows.map((r) => ({ a: r.id, t: r.kText, d: r.what, k: `${d.categories[r.cat]} ${r.modeLabel} ${r.src} key` }));
  return { id: "keys", title: "Vim keys", html, search };
}

// ---------------------------------------------------------------------------
// 5. LaTeX standards

export function latexSection(d) {
  const tag = (fn) => (strings, ...vals) => fn(String.raw({ raw: strings.raw }, ...vals).replace(/\\`/g, "`"));
  const helpers = {
    src: tag((s) => `<pre class="src"><code>${esc(s.replace(/^\n/, ""))}</code></pre>`),
    tex: tag((s) => math(s) || `<code>${esc(s)}</code>`),
    c: tag((s) => `<code>${esc(s)}</code>`),
    show: (source, render, note) => {
      const r = math(render || source, { display: true });
      return `<div class="show"><div class="show-src"><span class="ba-l">you write</span><pre class="src"><code>${esc(source)}</code></pre></div>${r ? `<div class="show-out"><span class="ba-l">you get</span><div class="rendered">${r}</div></div>` : ""}${note ? `<p class="show-note">${note}</p>` : ""}</div>`;
    },
    template: `<pre class="src"><code>${esc(d.template)}</code></pre>`,
  };
  const parts = latexStyle(helpers);
  const toc = `<nav class="toc" aria-label="On this page"><ul>${parts.map((p) => `<li><a href="#latex" data-go="ls-${p.id}">${esc(p.title)}</a></li>`).join("")}</ul></nav>`;
  const html = `<p class="lede">A practical style guide for maths documents, following amsmath / AMS practice. Rendered examples are KaTeX, which is close to, not identical with, the PDF.</p>
${toc}
${parts.map((p) => `<section class="sub" id="ls-${p.id}" aria-labelledby="ls-${p.id}-h"><h3 id="ls-${p.id}-h">${esc(p.title)}</h3>${p.body}</section>`).join("\n")}`;
  const plainBody = (b) => b.replace(/<div class="show-out">[\s\S]*?<\/div><\/div>/g, " ").replace(/<span class="katex[\s\S]*?<\/span><\/span><\/span>/g, "").replace(/<span class="ba-l">[^<]*<\/span>/g, " ").replace(/<[^>]+>/g, " ").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&amp;/g, "&").replace(/\s+/g, " ").trim();
  const search = parts.map((p) => ({ a: `ls-${p.id}`, t: p.title, d: plainBody(p.body).slice(0, 160), k: "latex style standard " + p.body.replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").slice(0, 4000) }));
  return { id: "latex", title: "LaTeX standards", html, search };
}

// ---------------------------------------------------------------------------
// 6. Symbols

export function symbolsSection(d) {
  const rows = d.symbolRows;
  const cats = [...new Set(rows.map((r) => r.cat))];
  const cc = counts(rows.map((r) => ({ filters: { cat: slug(r.cat) } })), "cat");
  const withSnip = rows.filter((r) => r.triggers.length).length;
  const nMangled = rows.filter((r) => r.mangled).length;
  const links = (r) => r.triggers.map((t) => `<a href="#snippets" data-go="${t.id}" class="snip-link">${t.typeHtml}</a>`).join(" ");
  const nPurpose = rows.filter((r) => r.onPurpose).length;
  const how = (r) => {
    if (r.onPurpose) {
      return `${r.triggers.length ? links(r) + " " : ""}<span class="gc">type it as written: you get <code>${esc(r.typed)}</code> on purpose (${esc(r.onPurpose.why)})</span>`;
    }
    if (!r.mangled) {
      return r.triggers.length ? `${links(r)} <span class="muted">or type it as written</span>` : `<span class="none">type it as written</span>`;
    }
    const parts = [`<span class="tag flag">gets changed</span>`];
    parts.push(`<span class="gc">typing <code>${esc(r.cmd)}</code> gives <code class="bad">${esc(r.typed)}</code>. This should not happen: the mangling test (tests/run.sh mangling) fails too.</span>`);
    if (r.triggers.length) parts.push(`<span class="gc">Instead: use ${links(r)}.</span>`);
    return parts.join(" ");
  };
  const table = dataTable({
    id: "tbl-symbols",
    caption: "Symbols",
    cols: [
      { key: "glyph", label: "Symbol", cls: "c-glyph" },
      { key: "cmd", label: "LaTeX", sort: true, cls: "c-cmd" },
      { key: "name", label: "Name", sort: true, cls: "c-name" },
      { key: "snip", label: "How to type it", sort: true, cls: "c-snip" },
      { key: "cat", label: "Category", sort: true, cls: "c-cat" },
    ],
    rows: rows.map((r) => ({
      id: r.id,
      cls: r.mangled ? "mangled" : "",
      cells: {
        glyph: r.glyphHtml,
        cmd: `<code>${esc(r.cmd)}</code>`,
        name: `${esc(r.name)}${r.needs ? ` <span class="needs">needs ${esc(r.needs)}</span>` : ""}`,
        snip: how(r),
        cat: esc(r.cat),
      },
      sort: { cmd: r.cmd, name: r.name, snip: r.mangled ? "2" : r.triggers.length ? "0" + plainSay(r.triggers[0].trig) : "1", cat: String(cats.indexOf(r.cat)).padStart(2, "0") },
      filters: { cat: slug(r.cat), snip: r.triggers.length ? "1" : "0", mangled: r.mangled ? "1" : "0", purpose: r.onPurpose ? "1" : "0" },
    })),
    filters: [{ dim: "cat", label: "Category", options: cats.map((c) => [slug(c), esc(c), cc.get(slug(c))]) }],
    toggles: [
      { dim: "snip", value: "1", label: `has a snippet (${withSnip})` },
      { dim: "mangled", value: "1", label: `gets changed while typing (${nMangled})` },
      { dim: "purpose", value: "1", label: `changes on purpose (${nPurpose})` },
    ],
  });
  const html = `<p class="lede">How do I type ⊆? Search by name or keyword (“subset”, “union”, “norm”). Every command below was typed as written into ntex (inside <code>$…$</code>): ${nMangled === 0 ? "<strong>none</strong> of them gets changed on the way" : `<strong>${nMangled}</strong> of them get changed on the way, and say so`}${nPurpose ? ` (${nPurpose} change${nPurpose > 1 ? "s" : ""} on purpose, marked)` : ""}; before the move to LuaSnip, 68 did. A snippet is listed only if its verified output is exactly that command (or starts with it, like <code>\\sum_{…}</code>).</p>${table}`;
  const search = rows.map((r) => ({ a: r.id, t: `${r.cmd}  ${r.name}`, d: (r.mangled ? "gets changed while typing. " : r.onPurpose ? "changes on purpose. " : "") + (r.triggers.length ? "snippet: " + r.triggers.map((t) => plainSay(t.trig)).join(", ") : r.kw), k: `${r.kw} ${r.cat} symbol` }));
  return { id: "symbols", title: "Symbol finder", html, search };
}

// ---------------------------------------------------------------------------
// 7. Settings and files

export function filesSection(d) {
  const files = d.files
    .map(
      (f) => `<tr id="${f.id}"><td><code class="path">${esc(f.path)}</code>${f.exists === false ? ` <span class="tag missing">not created yet</span>` : ""}</td><td>${f.what}</td><td>${f.edit}</td><td><span class="applies ${slug(f.applies)}">${esc(f.applies)}</span></td></tr>`,
    )
    .join("");
  const prefs = d.prefRows
    .map((p) => `<tr id="pref-${p.key}"><td><code>${esc(p.key)}</code></td><td><code>${esc(p.value)}</code></td><td><code>${esc(p.def)}</code></td><td>${esc(p.meaning)}</td></tr>`)
    .join("");
  const latex = dataTable({
    id: "tbl-latex",
    caption: "prefs.lua latex = { … }",
    cols: [
      { key: "key", label: "Key", sort: true, cls: "c-key" },
      { key: "value", label: "Now", cls: "c-value" },
      { key: "def", label: "Default", cls: "c-value" },
      { key: "meaning", label: "Meaning", cls: "c-meaning" },
    ],
    rows: d.latexRows.map((r) => ({
      id: "latex-" + slug(r.key),
      cells: {
        key: `<code>${esc(r.key)}</code>`,
        value: `<code class="val">${esc(r.value)}</code>${r.changed ? ` <span class="tag changed">yours</span>` : ""}`,
        def: `<code class="val">${esc(r.def)}</code>`,
        meaning: esc(r.meaning),
      },
      sort: { key: r.key },
      filters: { changed: r.changed ? "1" : "0" },
    })),
    toggles: [{ dim: "changed", value: "1", label: "different from the default" }],
  });
  const html = `
<p class="lede">Where everything lives, what it controls, and whether a change applies at once or needs a restart of ntex.</p>
<section class="sub" id="files-map" aria-labelledby="files-map-h"><h3 id="files-map-h">Files</h3>
<div class="table-wrap"><table class="data simple files"><thead><tr><th scope="col">File</th><th scope="col">Controls</th><th scope="col">How to edit</th><th scope="col">Applies</th></tr></thead><tbody>${files}</tbody></table></div></section>
<section class="sub" id="prefs" aria-labelledby="prefs-h"><h3 id="prefs-h"><code>prefs.lua</code>: your ntex preferences</h3>
<p>${code("~/.config/texnvim/prefs.lua")}: edit, save, restart ntex. Values below are what ntex loaded.</p>
<div class="table-wrap"><table class="data simple prefs"><thead><tr><th scope="col">Key</th><th scope="col">Now</th><th scope="col">Default</th><th scope="col">Meaning</th></tr></thead><tbody>${prefs}</tbody></table></div></section>
<section class="sub" id="latex-prefs" aria-labelledby="latex-prefs-h"><h3 id="latex-prefs-h"><code>latex = { … }</code> in prefs.lua: maths typing</h3>
<p>These were the Latex Suite settings (until 2026-09-30 read from <code>~/.config/latex-suite/obsidian-settings.json</code>); the values are the ones your Obsidian import had. Change one in prefs.lua, e.g. <code>latex = { autofraction = false }</code> (only the keys you change), then restart ntex.</p>
${latex}</section>
<section class="sub" id="more-files" aria-labelledby="more-files-h"><h3 id="more-files-h">Vim options, plugins, tests, uninstall</h3>
<ul class="spaced">
<li><strong>Vim options</strong> (line numbers, wrapping, spelling languages, clipboard, …): <code>lua/config/options.lua</code>. Relative numbers are the two lines <code>o.number = true</code> and <code>o.relativenumber = true</code> there (now: number ${d.options.number ? "on" : "off"}, relative ${d.options.relativenumber ? "on" : "off"}). Restart ntex after editing; to try one first, ${keys([":set norelativenumber", "Enter"])}.</li>
<li><strong>Plugins</strong> are pinned in <code>lazy-lock.json</code> (committed in the config's git repo). ${keys([":Lazy", "Enter"])} shows them; ${keys([":Lazy restore", "Enter"])} puts every plugin back to the locked version.</li>
<li><strong>Tests</strong>: <code>~/.config/texnvim/tests/run.sh</code> (everything, a few minutes, no windows), or one suite: <code>run.sh mine</code>, <code>defaults</code>, <code>mangling</code>, <code>english</code>, <code>decisions</code>, <code>perf</code>, <code>system</code>, <code>reference</code>.</li>
<li><strong>Roll back the move to LuaSnip</strong>: <code>git -C ~/.config/texnvim checkout pre-luasnip</code>, restart ntex; <code>git -C ~/.config/texnvim checkout main</code> comes back.</li>
<li><strong>Font</strong>: JuliaMono, set for cmux in <code>~/.config/ghostty/config</code> (<code>font-family = JuliaMono</code>). Reload with <code>cmux reload-config</code> from a cmux terminal, or restart cmux.</li>
<li><strong>Uninstall</strong>: <pre class="src"><code>rm -rf ~/.config/texnvim ~/.local/share/texnvim ~/.local/state/texnvim ~/.cache/texnvim
rm ~/.local/bin/ntex</code></pre> <code>~/.config/latex-suite/</code> is not ntex's: Superset and Obsidian use it.</li>
</ul></section>`;
  const search = [
    ...d.files.map((f) => ({ a: f.id, t: f.path, d: f.what.replace(/<[^>]+>/g, ""), k: "file config " + f.applies })),
    ...d.prefRows.map((p) => ({ a: "pref-" + p.key, t: `prefs.lua: ${p.key} = ${p.value}`, d: p.meaning, k: "preference prefs" })),
    ...d.latexRows.map((r) => ({ a: "latex-" + slug(r.key), t: `latex.${r.key} = ${r.value}`, d: r.meaning, k: "setting latex suite prefs" })),
    { a: "more-files", t: "options.lua, lazy-lock.json, tests, rollback, font, uninstall", d: "relative line numbers, :Lazy restore, run.sh, pre-luasnip, JuliaMono", k: "relativenumber plugins restore uninstall ghostty rollback" },
  ];
  return { id: "files", title: "Settings & files", html, search };
}

// ---------------------------------------------------------------------------
// 8. Commands

export function commandsSection(d) {
  const rows = d.commandRows
    .map((c) => `<tr id="${c.id}"><td><code>:${esc(c.name)}${c.args ? " " + esc(c.args) : ""}</code></td><td>${c.keysHtml || ""}</td><td>${prose(c.what)}</td><td><span class="src-tag src-${slug(c.src)}">${esc(c.src)}</span></td></tr>`)
    .join("");
  const html = `<p class="lede">Type <kbd class="key">:</kbd>, the command, ${keys("Enter")}. ${keys("Tab")} completes names. Commands from your config and VimTeX come from the live command list.</p>
<div class="table-wrap"><table class="data simple commands"><thead><tr><th scope="col">Command</th><th scope="col">Key</th><th scope="col">What it does</th><th scope="col">From</th></tr></thead><tbody>${rows}</tbody></table></div>`;
  return { id: "commands", title: "Commands", html, search: d.commandRows.map((c) => ({ a: c.id, t: ":" + c.name, d: c.what, k: "command " + c.src })) };
}

// ---------------------------------------------------------------------------
// 9. Troubleshooting

export function troubleSection(d) {
  const items = d.trouble;
  const html = `<p class="lede">Symptom first, then the fix.</p>
<div class="trouble">${items
    .map((t) => `<details class="tr" id="${t.id}"><summary><h3>${t.title}</h3></summary><div class="tr-body">${t.body}</div></details>`)
    .join("\n")}</div>`;
  return { id: "troubleshooting", title: "Troubleshooting", html, search: items.map((t) => ({ a: t.id, t: t.title.replace(/<[^>]+>/g, ""), d: t.body.replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").slice(0, 160), k: "problem fix help " + (t.k || "") })) };
}

export { code, slug, plainSay };
