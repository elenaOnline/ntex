-- The small vocabulary the snippet files in ~/.config/texnvim/snippets/tex/
-- are written in. Every helper returns an ordinary LuaSnip snippet
-- (ls.snippet with fmta-built nodes, a condition, a priority), so the files
-- stay one readable line per snippet while LuaSnip does the work.
--
--   math(trig, body, origin, opts)       maths, expands by itself (autosnippet)
--   math_tab(trig, body, origin, opts)   maths, expands on Tab
--   suffix(trig, body, origin, opts)     maths, meant to follow a letter: xsr → x^{2}
--   post(trig, body, origin, opts)       maths, grabs the letter before: xhat → \hat{x}  (%1 = the letter)
--   pat(pattern, body, origin, opts)     maths, a Lua pattern trigger: iden3, x2 → x_{2}  (%1, %2 = captures)
--   env(trig, name, origin, opts)        maths, \begin{name}…\end{name}: on its own lines in display maths, one line inline
--   text(trig, body, origin, opts)       text (outside maths), expands by itself
--   vis(key, body, origin, opts)         select text in maths, type key: %V = the selection
--   group(title, { … })                  wraps a file's list: sets the group shown on the reference page
--
-- body: the output, best as a [[long string]] so backslashes stay single.
--   <>          a tabstop (Tab goes to the next one; after the last, Tab leaves the snippet)
--   <:text>     a tabstop that starts out holding `text` (selected, so typing replaces it)
--   <1> <1:x>   numbered tabstops; the same number twice = a mirror (both change together)
--   <0>         where the last Tab leaves you (default: the end)
--   %1 %2       text captured by a post/pat trigger;  %V  the selection (vis);  %% a literal %
--   %< %>       a literal < or >, for text that would read as a tabstop: %<%> gives <>
--   a function  body = function(captures, line) return "…" end computes the output (same syntax)
-- Literal < and > are fine as long as they don't look like a tabstop.
--
-- origin: "yours" (your own), "changed" (a Latex Suite default you edited),
--   "default" (Latex Suite default, as shipped), "castel" (added from Gilles
--   Castel's set), "new" (added in the 2026-09-30 migration).
-- opts (all optional): where = "inline"|"display" (maths only),
--   line = "start"|"after text"|"list item" (text only), priority = n (like
--   Latex Suite's: higher wins), check = function(line, match, captures) → bool,
--   eg = "keys to type" (example for the reference page), desc = "…",
--   ft = "tex"|"markdown" (only in that kind of file).
local ls = require("luasnip")
local s, t, i, f, d, sn = ls.snippet, ls.text_node, ls.insert_node, ls.function_node, ls.dynamic_node, ls.snippet_node
local fmta = require("luasnip.extras.fmt").fmta
local events = require("luasnip.util.events")
local cond = require("snips.cond")
local context = require("snips.context")
local settings = require("snips.settings")

local M = {}

---------------------------------------------------------------------------
-- body → tokens

--- Split a body into tokens: {text=}, {tab=n, default=}, {cap=n}, {sel=true}.
function M.parse(body)
  local toks, buf = {}, {}
  local function flush()
    if #buf > 0 then
      toks[#toks + 1] = { text = table.concat(buf) }
      buf = {}
    end
  end
  local explicit = {}
  local pos, n = 1, #body
  while pos <= n do
    local c = body:sub(pos, pos)
    if c == "<" then
      local num, colon, default = body:match("^<(%d*)(:?)([^<>\n]*)>", pos)
      if num and (colon == ":" or default == "") then
        flush()
        local tok = { tab = num ~= "" and tonumber(num) or false, default = default }
        if tok.tab then
          explicit[tok.tab] = true
        end
        toks[#toks + 1] = tok
        pos = pos + #num + #colon + #default + 2
      else
        buf[#buf + 1] = c
        pos = pos + 1
      end
    elseif c == "%" then
      local nxt = body:sub(pos + 1, pos + 1)
      if nxt:match("%d") then
        flush()
        toks[#toks + 1] = { cap = tonumber(nxt) }
        pos = pos + 2
      elseif nxt == "V" then
        flush()
        toks[#toks + 1] = { sel = true }
        pos = pos + 2
      elseif nxt == "%" or nxt == "<" or nxt == ">" then
        -- %% a literal %; %< %> a literal < > that would read as a tabstop
        buf[#buf + 1] = nxt
        pos = pos + 2
      else
        buf[#buf + 1] = c
        pos = pos + 1
      end
    else
      buf[#buf + 1] = c
      pos = pos + 1
    end
  end
  flush()
  -- number the <> tabstops: 1, 2, … skipping numbers used explicitly
  local nextn = 1
  for _, tk in ipairs(toks) do
    if tk.tab == false then
      while explicit[nextn] do
        nextn = nextn + 1
      end
      tk.tab = nextn
      nextn = nextn + 1
    end
  end
  return toks
end

local function has_tabs(toks)
  for _, tk in ipairs(toks) do
    if tk.tab and tk.tab > 0 then
      return true
    end
  end
  return false
end

--- Plain text of a token list (tabstops show their default text).
function M.plain(toks, captures, selection)
  local out = {}
  for _, tk in ipairs(toks) do
    if tk.text then
      out[#out + 1] = tk.text
    elseif tk.tab then
      out[#out + 1] = tk.default or ""
    elseif tk.cap then
      out[#out + 1] = (captures or {})[tk.cap] or ("%" .. tk.cap)
    elseif tk.sel then
      out[#out + 1] = selection or "%V"
    end
  end
  return table.concat(out)
end

-- In inline maths a trailing space is dropped (prefs.lua
-- latex.remove_snippet_whitespace): Latex Suite did that, so `o+` gives
-- "\oplus" in $…$ and "\oplus " in display maths.
local function trailing_ws(ws)
  return f(function(_, parent)
    local env = parent.snippet.env
    if env.NTEX_INLINE == "1" and settings.get().remove_whitespace then
      return ""
    end
    return ws
  end)
end

local function capture_node(k)
  return f(function(_, parent)
    return parent.snippet.captures[k] or ""
  end)
end

local FMT_OPTS = { trim_empty = false, dedent = false, repeat_duplicates = true }

--- tokens → LuaSnip nodes, through fmta (`<>` placeholders).
--- selection: lines of the selected text (vis), or nil.
function M.nodes(toks, selection)
  -- split off trailing spaces/tabs of the last text token (a trailing
  -- newline stays in the text: a function node can't return "\n", and
  -- LuaSnip failing mid-expansion breaks every later snippet in that session)
  local ws
  local last = toks[#toks]
  if last and last.text and last.text:match("[ \t]$") then
    local has_exit = false
    for _, tk in ipairs(toks) do
      if tk.tab == 0 then
        has_exit = true
      end
    end
    if not has_exit then
      toks = vim.list_slice(toks, 1, #toks - 1)
      local body, space = last.text:match("^(.-)([ \t]+)$")
      if body ~= "" then
        toks[#toks + 1] = { text = body }
      end
      ws = space
    end
  end
  local tpl, args = {}, {}
  local ncap = 0
  for _, tk in ipairs(toks) do
    if tk.text then
      tpl[#tpl + 1] = (tk.text:gsub("<", "<<"):gsub(">", ">>"))
    elseif tk.tab then
      tpl[#tpl + 1] = "<" .. tk.tab .. ">"
      if not args[tk.tab] then
        if tk.tab == 0 then
          args[0] = i(0)
        else
          args[tk.tab] = i(tk.tab, vim.split(tk.default or "", "\n", { plain = true }))
        end
      end
    elseif tk.cap then
      ncap = ncap + 1
      local key = "cap" .. ncap
      tpl[#tpl + 1] = "<" .. key .. ">"
      args[key] = capture_node(tk.cap)
    elseif tk.sel then
      ncap = ncap + 1
      local key = "sel" .. ncap
      tpl[#tpl + 1] = "<" .. key .. ">"
      args[key] = t(selection or { "" })
    end
  end
  if ws then
    tpl[#tpl + 1] = "<ws>"
    args.ws = trailing_ws(ws)
  end
  return fmta(table.concat(tpl), args, FMT_OPTS)
end

---------------------------------------------------------------------------
-- snippets

local function on_pre_expand(snip)
  -- a new undo step per expansion: u / Ctrl-z brings back what you typed
  vim.o.undolevels = vim.o.undolevels
  local meta = snip.ntex
  -- A tab in a body (dm, beg) stays a tab character, as it did in Latex
  -- Suite: LuaSnip turns tabs into spaces when 'expandtab' is on, so it is
  -- off while the snippet is inserted (expansion is synchronous).
  if meta and meta.keep_tabs and vim.bo.expandtab then
    local buf = vim.api.nvim_get_current_buf()
    vim.bo[buf].expandtab = false
    vim.schedule(function()
      if vim.api.nvim_buf_is_valid(buf) then
        vim.bo[buf].expandtab = true
      end
    end)
  end
  if meta and meta.enlarge and settings.get().enlarge then
    vim.schedule(function()
      require("snips.features").auto_enlarge()
    end)
  end
end

local function enlarges(text)
  for _, trig in ipairs(settings.get().enlarge_triggers) do
    if text:find(trig, 1, true) then
      return true
    end
  end
  return false
end

--- The shared constructor. spec = { kind, trig, pattern?, body, origin, opts,
--- auto, where, boundary, lead, len }
local function make(spec)
  local opts = spec.opts or {}
  local body = spec.body
  local toks = type(body) == "string" and M.parse(body) or nil
  local where = opts.where or spec.where
  local boundary = opts.boundary or spec.boundary
  local lead = spec.lead or 0
  local check, line_rule, only_ft = opts.check, opts.line, opts.ft

  local function condition(line, match, captures)
    local st = settings.get()
    if not st.snippets_enabled then
      return false
    end
    if spec.kind == "visual" then
      return false -- only lua/snips/visual.lua expands these
    end
    if not cond.boundary(boundary, line, match, lead) then
      return false
    end
    if line_rule and not cond.line(line_rule, line, match) then
      return false
    end
    if check and not check(line, match, captures) then
      return false
    end
    local ctx = context.get()
    if only_ft and ctx and ctx.ft ~= only_ft then
      return false
    end
    if not ctx then
      return false
    end
    return cond.where(where, ctx, line:sub(1, #line - #match))
  end

  local function resolve()
    local ctx = context.get()
    return { env_override = { NTEX_INLINE = (ctx and ctx.mode.inlineMath) and "1" or "" } }
  end

  local nodes
  if spec.kind == "visual" then
    -- built at expansion time from the selection (LS_SELECT_RAW)
    nodes = {
      d(1, function(_, parent)
        local sel = parent.snippet.env.LS_SELECT_RAW
        if type(sel) ~= "table" then
          sel = { tostring(sel or "") }
        end
        if has_tabs(toks) then
          return sn(nil, M.nodes(toks, sel))
        end
        -- no tabstops: the result stays selected, so another key wraps it again
        local text = M.plain(toks, nil, table.concat(sel, "\n"))
        return sn(nil, { i(1, vim.split(text, "\n", { plain = true })) })
      end),
    }
  elseif toks then
    nodes = M.nodes(toks)
  else
    -- a function body: computed from the captures and the line when it expands
    nodes = {
      d(1, function(_, parent)
        local snip = parent.snippet
        local out = body(snip.captures or {}, snip.env.NTEX_LINE or "")
        local tk = M.parse(out or "")
        if not has_tabs(tk) then
          tk[#tk + 1] = { tab = 1, default = "" }
        end
        return sn(nil, M.nodes(tk))
      end),
    }
  end

  local trig = spec.pattern or spec.trig
  local plain_out = toks and M.plain(toks) or ""
  local prio = 1000 + 100 * (opts.priority or 0) + (spec.len or #trig)
  local snip = s({
    trig = trig,
    name = spec.trig,
    desc = opts.desc or (spec.trig .. " → " .. (toks and plain_out or "(computed)")),
    trigEngine = spec.pattern and "pattern" or "plain",
    wordTrig = false,
    hidden = true,
    priority = prio,
    snippetType = spec.auto and "autosnippet" or "snippet",
    condition = condition,
    resolveExpandParams = function(_, line)
      local r = resolve()
      r.env_override.NTEX_LINE = line
      return r
    end,
  }, nodes, {
    callbacks = { [-1] = { [events.pre_expand] = on_pre_expand } },
  })
  snip.ntex = {
    kind = spec.kind,
    trig = spec.trig,
    pattern = spec.pattern,
    body = type(body) == "string" and body or nil,
    computed = type(body) == "function",
    origin = spec.origin or opts.origin or "new",
    where = spec.kind == "text" and "text" or (where or "math"),
    how = spec.kind == "visual" and "visual" or (spec.auto and "auto" or "tab"),
    line_rule = line_rule,
    ft = only_ft,
    boundary = boundary,
    priority = opts.priority or 0,
    eg = opts.eg,
    note = opts.note,
    desc = opts.desc,
    enlarge = toks ~= nil and enlarges(plain_out),
    keep_tabs = type(body) == "function" or (type(body) == "string" and body:find("\t", 1, true) ~= nil),
  }
  return snip
end

-- (trig, body, origin, opts) or (trig, body, opts)
local function args(origin, opts)
  if type(origin) == "table" then
    return origin.origin, origin
  end
  return origin, opts or {}
end

function M.math(trig, body, origin, opts)
  origin, opts = args(origin, opts)
  return make({ kind = "math", trig = trig, body = body, origin = origin, opts = opts, auto = true, where = "math", boundary = "word" })
end

function M.math_tab(trig, body, origin, opts)
  origin, opts = args(origin, opts)
  return make({ kind = "math", trig = trig, body = body, origin = origin, opts = opts, auto = false, where = "math", boundary = "word" })
end

function M.suffix(trig, body, origin, opts)
  origin, opts = args(origin, opts)
  return make({ kind = "suffix", trig = trig, body = body, origin = origin, opts = opts, auto = true, where = "math", boundary = "suffix" })
end

function M.post(trig, body, origin, opts)
  origin, opts = args(origin, opts)
  local pattern = "(%a)" .. vim.pesc(trig)
  return make({ kind = "post", trig = trig, pattern = pattern, body = body, origin = origin, opts = opts, auto = true, where = "math", boundary = "suffix", len = #trig + 1 })
end

function M.pat(pattern, body, origin, opts)
  origin, opts = args(origin, opts)
  return make({ kind = "pattern", trig = opts.name or pattern, pattern = pattern, body = body, origin = origin, opts = opts, auto = not opts.tab, where = "math", boundary = "word", len = opts.len or 0 })
end

function M.text(trig, body, origin, opts)
  origin, opts = args(origin, opts)
  return make({ kind = "text", trig = trig, body = body, origin = origin, opts = opts, auto = not opts.tab, where = "text", boundary = "word" })
end

function M.vis(key, body, origin, opts)
  origin, opts = args(origin, opts)
  return make({ kind = "visual", trig = key, body = body, origin = origin, opts = opts, auto = false, where = "math", boundary = "none" })
end

--- \begin{name}…\end{name}: on separate lines in display maths, on one line
--- inline (Latex Suite's M / n pair). Returns two snippets.
function M.env(trig, name, origin, opts)
  origin, opts = args(origin, opts)
  local o1 = vim.tbl_extend("force", opts, { where = "display", desc = opts.desc or (trig .. " → \\begin{" .. name .. "} … (display maths)") })
  local o2 = vim.tbl_extend("force", opts, { where = "inline", desc = opts.desc or (trig .. " → \\begin{" .. name .. "} … (inline maths)") })
  return {
    make({ kind = "math", trig = trig, body = "\\begin{" .. name .. "}\n<>\n\\end{" .. name .. "}", origin = origin, opts = o1, auto = true, where = "display", boundary = "word" }),
    make({ kind = "math", trig = trig, body = "\\begin{" .. name .. "}<>\\end{" .. name .. "}", origin = origin, opts = o2, auto = true, where = "inline", boundary = "word" }),
  }
end

--- Display-maths delimiters for the current file: \[ \] in .tex, $$ $$ in .md.
function M.display_delims()
  if vim.bo.filetype == "markdown" then
    return "$$", "$$"
  end
  return "\\[", "\\]"
end

--- A file's snippets: stamps the group title on each, flattens env pairs.
function M.group(title, list)
  local out = {}
  local function add(x)
    if x.ntex then
      x.ntex.group = title
      out[#out + 1] = x
    else
      for _, y in ipairs(x) do
        add(y)
      end
    end
  end
  add(list)
  return out
end

return M
