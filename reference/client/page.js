// ntex reference: sections, global search, sortable / filterable tables.
// DATA (search index) is inlined by build.mjs before this script.
(function () {
  "use strict";
  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
  const KEY = "ntex-reference:v1";

  // ---------------------------------------------------------------- state
  let state = { section: null, tables: {} };
  try {
    const raw = localStorage.getItem(KEY);
    if (raw) state = Object.assign(state, JSON.parse(raw));
  } catch (e) { /* private window, blocked storage: fine */ }
  function save() {
    try { localStorage.setItem(KEY, JSON.stringify(state)); } catch (e) { /* ignore */ }
  }

  // ---------------------------------------------------------------- sections
  const pages = $$("main > section.page");
  const ids = pages.map((p) => p.id);
  function show(id, { focus = false, keepScroll = false } = {}) {
    if (!ids.includes(id)) id = ids[0];
    for (const p of pages) p.hidden = p.id !== id;
    for (const a of $$("nav.sections a")) {
      if (a.getAttribute("href") === "#" + id) {
        a.setAttribute("aria-current", "page");
        if (a.scrollIntoView && window.matchMedia("(max-width: 1279px)").matches) a.scrollIntoView({ block: "nearest", inline: "center" });
      } else a.removeAttribute("aria-current");
    }
    state.section = id;
    save();
    if (!keepScroll) window.scrollTo(0, 0);
    if (focus) {
      const h = $("#" + id + " > h2");
      if (h) { h.setAttribute("tabindex", "-1"); h.focus({ preventScroll: true }); }
    }
    document.title = ($("#" + id + " > h2")?.textContent || "") + " · ntex reference";
  }
  function sectionOf(el) {
    const p = el.closest("section.page");
    return p ? p.id : null;
  }

  /** go to an element anywhere on the page: show its section, reveal, flash */
  function goTo(anchorId) {
    const el = document.getElementById(anchorId);
    if (!el) return;
    const sec = sectionOf(el);
    if (sec) show(sec, { keepScroll: true });
    const d = el.closest("details");
    if (d) d.open = true;
    if (el.tagName === "DETAILS") el.open = true;
    const block = el.closest(".table-block");
    if (block && el.tagName === "TR" && el.hidden) {
      clearFilters(block);
    }
    requestAnimationFrame(() => {
      el.scrollIntoView({ block: el.tagName === "TR" ? "center" : "start" });
      if (el.tagName === "TR") {
        el.classList.remove("flash");
        void el.offsetWidth;
        el.classList.add("flash");
      }
    });
  }

  document.addEventListener("click", (ev) => {
    const a = ev.target.closest("a[href^='#']");
    if (!a) return;
    const go = a.getAttribute("data-go");
    const gf = a.getAttribute("data-go-filter");
    const id = a.getAttribute("href").slice(1);
    if (go) {
      ev.preventDefault();
      history.replaceState(null, "", "#" + (sectionOf(document.getElementById(go) || a) || id));
      goTo(go);
      closeResults();
    } else if (gf) {
      ev.preventDefault();
      const [sec, dim, value] = gf.split(":");
      show(sec, { focus: true });
      history.replaceState(null, "", "#" + sec);
      const block = $("#" + sec + " .table-block");
      if (block) setFilter(block, dim, [value]);
    } else if (ids.includes(id)) {
      ev.preventDefault();
      history.replaceState(null, "", "#" + id);
      show(id, { focus: a.closest("nav.sections") !== null });
      closeResults();
    }
  });
  window.addEventListener("hashchange", () => {
    const id = location.hash.slice(1);
    if (ids.includes(id)) show(id);
    else if (document.getElementById(id)) goTo(id);
  });

  // ---------------------------------------------------------------- tables
  const blocks = $$(".table-block");
  function rowsOf(block) { return $$("tbody > tr", block); }
  function tableId(block) { return block.getAttribute("data-table"); }
  function tstate(block) {
    const id = tableId(block);
    state.tables[id] = state.tables[id] || { filters: {}, sort: null, q: "" };
    return state.tables[id];
  }
  function apply(block) {
    const st = tstate(block);
    const q = (st.q || "").trim().toLowerCase();
    const words = q ? q.split(/\s+/) : [];
    let shown = 0;
    for (const tr of rowsOf(block)) {
      let ok = true;
      for (const [dim, vals] of Object.entries(st.filters)) {
        if (!vals || vals.length === 0) continue;
        const have = (tr.getAttribute("data-f-" + dim) || "").split(" ");
        if (!vals.some((v) => have.includes(v))) { ok = false; break; }
      }
      if (ok && words.length) {
        const text = (tr._text = tr._text || tr.textContent.toLowerCase());
        ok = words.every((w) => text.includes(w));
      }
      tr.hidden = !ok;
      if (ok) shown++;
    }
    const c = $("[data-count]", block);
    if (c) c.textContent = shown;
    const empty = $(".empty", block);
    if (empty) empty.hidden = shown !== 0;
    const any = Object.values(st.filters).some((v) => v && v.length) || !!q;
    for (const b of $$("[data-clear]", block)) if (b.classList.contains("chip")) b.hidden = !any;
    for (const chip of $$(".chip[data-dim]", block)) {
      const vals = st.filters[chip.dataset.dim] || [];
      chip.setAttribute("aria-pressed", vals.includes(chip.dataset.value) ? "true" : "false");
    }
    const input = $("[data-local]", block);
    if (input && input.value !== (st.q || "")) input.value = st.q || "";
  }
  function setFilter(block, dim, vals) {
    const st = tstate(block);
    st.filters[dim] = vals;
    save();
    apply(block);
  }
  function clearFilters(block) {
    const st = tstate(block);
    st.filters = {};
    st.q = "";
    save();
    apply(block);
  }
  function sortBy(block, key, dir) {
    const st = tstate(block);
    const tbody = $("tbody", block);
    const rows = rowsOf(block);
    if (!key) {
      rows.sort((a, b) => +a.dataset.i - +b.dataset.i);
    } else {
      const coll = new Intl.Collator("en", { numeric: true, sensitivity: "base" });
      rows.sort((a, b) => {
        const x = a.getAttribute("data-s-" + key) || "";
        const y = b.getAttribute("data-s-" + key) || "";
        const r = coll.compare(x, y) || (+a.dataset.i - +b.dataset.i);
        return dir === "desc" ? -r : r;
      });
    }
    for (const r of rows) tbody.appendChild(r);
    for (const th of $$("th[aria-sort]", block)) {
      const k = $("button", th).dataset.sort;
      th.setAttribute("aria-sort", k === key ? (dir === "desc" ? "descending" : "ascending") : "none");
    }
    const sel = $("[data-sort-select]", block);
    if (sel) sel.value = key ? key + ":" + dir : "";
    st.sort = key ? [key, dir] : null;
    save();
  }

  for (const block of blocks) {
    const st = tstate(block);
    // card layout on narrow screens reads the column name from data-label
    block.addEventListener("click", (ev) => {
      const chip = ev.target.closest(".chip[data-dim]");
      if (chip) {
        const dim = chip.dataset.dim, v = chip.dataset.value;
        const cur = new Set(tstate(block).filters[dim] || []);
        cur.has(v) ? cur.delete(v) : cur.add(v);
        setFilter(block, dim, Array.from(cur));
        return;
      }
      if (ev.target.closest("[data-clear]")) { clearFilters(block); return; }
      const sb = ev.target.closest("th button[data-sort]");
      if (sb) {
        const key = sb.dataset.sort;
        const cur = tstate(block).sort;
        const dir = cur && cur[0] === key && cur[1] === "asc" ? "desc" : "asc";
        sortBy(block, key, dir);
      }
    });
    const sel = $("[data-sort-select]", block);
    if (sel) sel.addEventListener("change", () => {
      const [k, d] = sel.value ? sel.value.split(":") : [null, null];
      sortBy(block, k, d);
    });
    const input = $("[data-local]", block);
    if (input) {
      let t;
      input.addEventListener("input", () => {
        clearTimeout(t);
        t = setTimeout(() => { tstate(block).q = input.value; save(); apply(block); }, 80);
      });
      input.addEventListener("keydown", (ev) => {
        if (ev.key === "Escape") { input.value = ""; tstate(block).q = ""; save(); apply(block); }
      });
    }
    if (st.sort) sortBy(block, st.sort[0], st.sort[1]);
    apply(block);
  }
  // data-label on every table cell (card layout)
  for (const t of $$("table.data")) {
    const heads = $$("thead th", t).map((th) => th.textContent.trim());
    for (const tr of $$("tbody tr", t)) $$("td", tr).forEach((td, i) => { if (!td.hasAttribute("data-label")) td.setAttribute("data-label", heads[i] || ""); });
  }

  // ---------------------------------------------------------------- search
  const q = $("#q");
  const box = $("#results");
  const index = (window.REFERENCE_DATA && window.REFERENCE_DATA.search) || [];
  const titles = (window.REFERENCE_DATA && window.REFERENCE_DATA.sections) || {};
  for (const e of index) e._h = (e.t + " \u0001 " + (e.d || "") + " \u0001 " + (e.k || "")).toLowerCase();
  const escH = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  function hl(s, words) {
    let out = escH(s);
    for (const w of words) {
      if (w.length < 1) continue;
      const ew = escH(w).replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
      out = out.replace(new RegExp("(" + ew + ")(?![^<]*>)", "gi"), "<mark>$1</mark>");
    }
    return out;
  }
  function score(e, words, raw) {
    const t = e.t.toLowerCase();
    let s = 0;
    for (const w of words) {
      const i = e._h.indexOf(w);
      if (i < 0) return -1;
      if (t === w) s += 100;
      else if (t.startsWith(w)) s += 40;
      else if (t.includes(w)) s += 20;
      else if ((e.d || "").toLowerCase().includes(w)) s += 8;
      else s += 2;
    }
    if (t === raw) s += 200;
    return s;
  }
  let sel = -1;
  function closeResults() {
    box.hidden = true;
    q.setAttribute("aria-expanded", "false");
    sel = -1;
  }
  function run() {
    const raw = q.value.trim().toLowerCase();
    if (!raw) { closeResults(); box.innerHTML = ""; return; }
    const words = raw.split(/\s+/);
    const hits = [];
    for (const e of index) {
      const s = score(e, words, raw);
      if (s >= 0) hits.push([s, e]);
    }
    hits.sort((a, b) => b[0] - a[0]);
    const groups = new Map();
    for (const [, e] of hits) {
      if (!groups.has(e.s)) groups.set(e.s, []);
      groups.get(e.s).push(e);
    }
    // sections in page order, the best-scoring group first
    const order = Array.from(groups.keys());
    let html = "";
    if (!hits.length) html = `<p class="none">Nothing found for “${escH(q.value)}”.</p>`;
    for (const sec of order) {
      const list = groups.get(sec);
      html += `<h3>${escH(titles[sec] || sec)} <span class="n">${list.length}</span></h3><ul>`;
      for (const e of list.slice(0, 8)) {
        html += `<li><a href="#${escH(sec)}" data-go="${escH(e.a)}" role="option"><span class="r-t">${hl(e.t, words)}</span>${e.d ? `<span class="r-d">${hl(e.d, words)}</span>` : ""}</a></li>`;
      }
      html += `</ul>`;
      if (list.length > 8) html += `<p class="more">and ${list.length - 8} more: refine the search, or use the section's own filter</p>`;
    }
    box.innerHTML = html;
    box.hidden = false;
    q.setAttribute("aria-expanded", "true");
    sel = -1;
  }
  let timer;
  q.addEventListener("input", () => { clearTimeout(timer); timer = setTimeout(run, 60); });
  q.addEventListener("focus", () => { if (q.value.trim()) run(); });
  function items() { return $$("a[data-go]", box); }
  function move(d) {
    const it = items();
    if (!it.length) return;
    sel = Math.max(-1, Math.min(it.length - 1, sel + d));
    it.forEach((a, i) => a.setAttribute("aria-selected", i === sel ? "true" : "false"));
    if (sel >= 0) it[sel].focus({ preventScroll: false });
    else q.focus();
  }
  q.addEventListener("keydown", (ev) => {
    if (ev.key === "ArrowDown") { ev.preventDefault(); move(1); }
    else if (ev.key === "Enter") { const it = items(); if (it.length) { ev.preventDefault(); it[0].click(); q.blur(); } }
    else if (ev.key === "Escape") { q.value = ""; run(); q.blur(); }
  });
  box.addEventListener("keydown", (ev) => {
    if (ev.key === "ArrowDown") { ev.preventDefault(); move(1); }
    else if (ev.key === "ArrowUp") { ev.preventDefault(); move(-1); }
    else if (ev.key === "Escape") { q.value = ""; run(); q.focus(); }
  });
  document.addEventListener("keydown", (ev) => {
    const t = ev.target;
    const typing = t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable);
    if (ev.key === "/" && !typing && !ev.metaKey && !ev.ctrlKey && !ev.altKey) {
      ev.preventDefault();
      q.focus();
      q.select();
    } else if (ev.key === "Escape" && !typing && !box.hidden) {
      closeResults();
    }
  });
  document.addEventListener("click", (ev) => {
    if (!ev.target.closest(".search")) closeResults();
  });

  // ---------------------------------------------------------------- start
  const initial = location.hash.slice(1);
  if (ids.includes(initial)) show(initial);
  else if (initial && document.getElementById(initial)) { show(ids[0]); goTo(initial); }
  else show(state.section || ids[0]);
  document.documentElement.classList.add("js");
})();
