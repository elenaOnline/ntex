-- Maths typing features beyond snippets (they came from Obsidian's Latex
-- Suite, src/features/*.ts): auto-fraction, tabout, matrix shortcuts,
-- auto-enlarge brackets, auto-delete $, and the Tab / Shift-Tab / Enter /
-- Shift-Enter keys. Settings: prefs.lua `latex = { … }` (lua/snips/settings.lua).
--
-- Key order (as in Latex Suite):
--   Tab:   expand a Tab snippet → next tabstop → (in a matrix: leave the
--          enclosing bracket on this line → insert " & ") → tabout → Tab
--   Enter: accept a *selected* completion → in a matrix: " \\" + newline → Enter
--   /:     an autosnippet that ends in "/" (e.g. //) → auto-fraction → "/"

local api = vim.api
local ls = require("luasnip")
local context = require("snips.context")
local mdmath = require("snips.mdmath")
local settings = require("snips.settings")

local M = {}

local function S()
  return settings.get()
end

--- Expand LuaSnip nodes in place of [from, to) (0-based {row, col}).
local function expand_nodes(nodes, from, to)
  vim.o.undolevels = vim.o.undolevels -- own undo step
  ls.snip_expand(ls.snippet({ trig = "", hidden = true }, nodes), {
    clear_region = { from = from, to = to },
    pos = from,
    indent = false, -- re-inserted text keeps its own indentation
  })
end
M.expand_nodes = expand_nodes

local function feed(keys)
  api.nvim_feedkeys(vim.keycode(keys), "n", false)
end

local function set_cursor(pos)
  api.nvim_win_set_cursor(0, { pos[1] + 1, pos[2] })
end

local function get_text(buf, from, to)
  return table.concat(api.nvim_buf_get_text(buf, from[1], from[2], to[1], to[2], {}), "\n")
end

--- position `off` bytes (0-based) into text that starts at `from`
local function advance(from, text, off)
  local row, col = from[1], from[2]
  local prefix = text:sub(1, off)
  local nl = 0
  local last = 0
  for p in prefix:gmatch("()\n") do
    nl = nl + 1
    last = p
  end
  if nl == 0 then
    return { row, col + off }
  end
  return { row + nl, off - last }
end

---------------------------------------------------------------------------
-- equation bounds: {outer_start, inner_start, inner_end, outer_end} as {row, col}

function M.bounds(ctx)
  if not ctx or not ctx.math_kind then
    return nil
  end
  if ctx.ft == "markdown" then
    if ctx.mode.codeMath or not ctx.math_start then
      return nil
    end
    local s = ctx.math_start
    local dl = ctx.math_kind == "display" and 2 or 1
    local ie, oe = mdmath.find_end(ctx.buf, ctx.row, ctx.col, ctx.math_kind)
    if not ie then
      local last = api.nvim_buf_line_count(ctx.buf) - 1
      if ctx.math_kind == "inline" then
        ie = { row = ctx.row, col = #ctx.line }
      else
        ie = { row = last, col = #(api.nvim_buf_get_lines(ctx.buf, last, last + 1, false)[1] or "") }
      end
      oe = ie
    end
    return {
      outer_start = { s.row, s.col - dl },
      inner_start = { s.row, s.col },
      inner_end = { ie.row, ie.col },
      outer_end = { oe.row, oe.col },
    }
  end
  local ok, sur = pcall(vim.fn["vimtex#env#get_surrounding"], "math")
  if not ok or type(sur) ~= "table" or type(sur[1]) ~= "table" or not sur[1].lnum then
    return nil
  end
  local o, c = sur[1], sur[2]
  return {
    outer_start = { o.lnum - 1, o.cnum - 1 },
    inner_start = { o.lnum - 1, o.cnum - 1 + #o.match },
    inner_end = { c.lnum - 1, c.cnum - 1 },
    outer_end = { c.lnum - 1, c.cnum - 1 + #c.match },
  }
end

---------------------------------------------------------------------------
-- tokenizer (upstream src/utils/tokenizer.ts)

local function tokenize(s)
  local toks = {}
  local i, n = 1, #s
  while i <= n do
    local c = s:sub(i, i)
    if c:match("%s") then
      i = i + 1
    elseif c == "%" then
      local j = s:find("\n", i, true) or (n + 1)
      toks[#toks + 1] = { text = s:sub(i, j - 1), s = i - 1, e = j - 1 }
      i = j
    elseif c == "\\" then
      local name = s:match("^\\%a+", i)
      local tok = name or s:sub(i, i + 1)
      toks[#toks + 1] = { text = tok, s = i - 1, e = i - 1 + #tok }
      i = i + #tok
    else
      -- keep UTF-8 sequences together
      local ch = s:match("^[%z\1-\127\194-\244][\128-\191]*", i) or c
      toks[#toks + 1] = { text = ch, s = i - 1, e = i - 1 + #ch }
      i = i + #ch
    end
  end
  return toks
end

local LEFT = { ["\\left"] = true, ["\\bigl"] = true, ["\\Bigl"] = true, ["\\biggl"] = true, ["\\Biggl"] = true }
local RIGHT = { ["\\right"] = true, ["\\bigr"] = true, ["\\Bigr"] = true, ["\\biggr"] = true, ["\\Biggr"] = true }
local DELIMS = {}
for _, d in ipairs({ "(", ")", "[", "]", "\\lbrack", "\\rbrack", "\\{", "\\}", "\\lbrace", "\\rbrace", "<", ">",
  "\\langle", "\\rangle", "\\lt", "\\gt", "|", "\\vert", "\\lvert", "\\rvert", "\\|", "\\Vert", "\\lVert", "\\rVert",
  "\\lfloor", "\\rfloor", "\\lceil", "\\rceil", "\\ulcorner", "\\urcorner", "/", "\\\\", "\\backslash", "\\uparrow",
  "\\downarrow", "\\Uparrow", "\\Downarrow", "." }) do
  DELIMS[d] = true
end
local DELIM_PAIRS = {
  ["("] = ")", ["["] = "]", ["{"] = "}", ["\\lbrack"] = "\\rbrack", ["\\lbrace"] = "\\rbrace", ["\\langle"] = "\\rangle",
  ["\\lvert"] = "\\rvert", ["\\lVert"] = "\\rVert", ["\\lfloor"] = "\\rfloor", ["\\lceil"] = "\\rceil",
  ["\\ulcorner"] = "\\urcorner", ["<"] = ">",
}

local function is_closing(toks, k)
  local cur = toks[k]
  local prev = toks[k - 1]
  if prev then
    if RIGHT[prev.text] and DELIMS[cur.text] then
      return true
    end
    if LEFT[prev.text] and DELIMS[cur.text] then
      return false
    end
  end
  return S().tabout_closing[cur.text] == true
end

---------------------------------------------------------------------------
-- tabout (upstream src/features/tabout.ts)

function M.tabout(ctx)
  if not S().tabout or not context.in_math(ctx.mode) then
    return false
  end
  local b = M.bounds(ctx)
  if not b then
    return false
  end
  local cur = { ctx.row, ctx.col }
  local text = get_text(ctx.buf, b.inner_start, b.inner_end)
  local rel = #get_text(ctx.buf, b.inner_start, cur)
  local toks = tokenize(text)
  local start
  for k, tk in ipairs(toks) do
    if tk.e > rel then
      start = k
      break
    end
  end
  if start then
    for k = start, #toks do
      if is_closing(toks, k) then
        set_cursor(advance(b.inner_start, text, toks[k].e))
        return true
      end
      local tk = toks[k]
      if RIGHT[tk.text] and (not toks[k + 1] or not DELIMS[toks[k + 1].text]) then
        set_cursor(advance(b.inner_start, text, tk.e))
        return true
      end
    end
  end
  local remaining = text:sub(rel + 1)
  local at_end = remaining:match("^%s*$") ~= nil
  if not at_end and S().tabout_eol_only then
    return false
  end
  if b.outer_start[1] == b.outer_end[1] then
    set_cursor(b.outer_end)
    return true
  end
  -- multi-line equation: continue on the line after the closing delimiter
  local buf = ctx.buf
  local end_row = b.outer_end[1]
  local indent = (api.nvim_buf_get_lines(buf, b.outer_start[1], b.outer_start[1] + 1, false)[1] or ""):match("^%s*")
  local cur_line = api.nvim_buf_get_lines(buf, ctx.row, ctx.row + 1, false)[1]
  if cur_line:match("%s+$") then
    api.nvim_buf_set_lines(buf, ctx.row, ctx.row + 1, false, { (cur_line:gsub("%s+$", "")) })
  end
  if end_row == api.nvim_buf_line_count(buf) - 1 then
    api.nvim_buf_set_lines(buf, end_row + 1, end_row + 1, false, { indent })
  else
    local nxt = api.nvim_buf_get_lines(buf, end_row + 1, end_row + 2, false)[1]
    if nxt:match("^%s*$") then
      api.nvim_buf_set_lines(buf, end_row + 1, end_row + 2, false, { indent })
    else
      set_cursor({ end_row + 1, 0 })
      return true
    end
  end
  set_cursor({ end_row + 1, #indent })
  return true
end

---------------------------------------------------------------------------
-- matrix shortcuts (upstream src/features/matrix_shortcuts.ts)

--- innermost environment/command frame, if it is a matrix-like one
local function matrix_frame(ctx)
  if not S().matrix or not context.strictly_in_math(ctx.mode) then
    return nil
  end
  local envs = ctx.ft == "tex" and S().matrix_envs_tex or S().matrix_envs
  for _, f in ipairs(ctx:stack()) do
    if f.kind == "environment" then
      return envs[f.name] and f or nil
    elseif f.kind == "command" then
      return S().matrix_macros[f.name] and f or nil
    elseif f.kind == "math" then
      return nil
    end
  end
end

-- where does `\end{name}` (matching the enclosing \begin) end? {row, col} or nil
local function env_end(ctx, name)
  local lines = api.nvim_buf_get_lines(ctx.buf, ctx.row, ctx.row + 300, false)
  local depth = 0
  local pat_b = "\\begin{" .. name .. "}"
  local pat_e = "\\end{" .. name .. "}"
  for k, line in ipairs(lines) do
    local from = (k == 1) and (ctx.col + 1) or 1
    while true do
      local bi = line:find(pat_b, from, true)
      local ei = line:find(pat_e, from, true)
      if not ei then
        if bi then
          depth = depth + 1
          from = bi + #pat_b
        else
          break
        end
      elseif bi and bi < ei then
        depth = depth + 1
        from = bi + #pat_b
      else
        if depth == 0 then
          return { ctx.row + k - 1, ei - 1 + #pat_e }
        end
        depth = depth - 1
        from = ei + #pat_e
      end
    end
  end
end

local function frame_multiline(ctx, f)
  if f.begin_row and f.begin_row < ctx.row then
    return true
  end
  local e = env_end(ctx, f.name)
  return e ~= nil and e[1] > ctx.row
end

-- What a new matrix row starts with: the first run of spaces and & at the
-- line start or right after a \\ on this line, minus leading spaces (Latex
-- Suite's /(\\begin{[^]]*}|\\\\|^)((?:\s|&)+)/; its \begin{…} branch only
-- matches one-letter names, so it is left out).
local function matrix_line_lead(line)
  local run = line:match("^[%s&]+")
  local i = 1
  while not run do
    local a = line:find("\\\\", i, true)
    if not a then
      break
    end
    run = line:match("^[%s&]+", a + 2)
    i = a + 1
  end
  return run and (run:gsub("^%s+", "")) or ""
end

function M.matrix_newline(ctx)
  local f = matrix_frame(ctx)
  if not f then
    return false
  end
  local added = matrix_line_lead(ctx.line)
  if frame_multiline(ctx, f) then
    local indent = ctx.line:match("^%s*")
    api.nvim_buf_set_text(ctx.buf, ctx.row, ctx.col, ctx.row, ctx.col, { " \\\\", indent .. added })
    set_cursor({ ctx.row + 1, #indent + #added })
  else
    local ins = " \\\\  " .. added
    api.nvim_buf_set_text(ctx.buf, ctx.row, ctx.col, ctx.row, ctx.col, { ins })
    set_cursor({ ctx.row, ctx.col + #ins })
  end
  return true
end

function M.matrix_cell(ctx)
  if not matrix_frame(ctx) then
    return false
  end
  api.nvim_buf_set_text(ctx.buf, ctx.row, ctx.col, ctx.row, ctx.col, { " & " })
  set_cursor({ ctx.row, ctx.col + 3 })
  return true
end

function M.matrix_exit(ctx)
  local f = matrix_frame(ctx)
  if not f then
    return false
  end
  if frame_multiline(ctx, f) then
    local nxt = api.nvim_buf_get_lines(ctx.buf, ctx.row + 1, ctx.row + 2, false)[1]
    if not nxt then
      return false
    end
    local to = #nxt
    local s, e, name = nxt:find("\\end{([^}]*)}")
    local envs = ctx.ft == "tex" and S().matrix_envs_tex or S().matrix_envs
    if s and envs[name] then
      to = e
    end
    set_cursor({ ctx.row + 1, to })
  else
    local e = env_end(ctx, f.name)
    if not e then
      return false
    end
    set_cursor(e)
  end
  return true
end

--- Tab inside a matrix: first leave an enclosing bracket on this line
function M.matrix_priority_tabout(ctx)
  if not matrix_frame(ctx) or not S().tabout then
    return false
  end
  local rest = ctx.line:sub(ctx.col + 1)
  local closing, opening = {}, {}
  for o, c in pairs(DELIM_PAIRS) do
    if S().tabout_closing[c] then
      closing[c] = true
      opening[o] = true
    end
  end
  local stack = 0
  for _, tk in ipairs(tokenize(rest)) do
    if closing[tk.text] then
      if stack == 0 then
        set_cursor({ ctx.row, ctx.col + tk.e })
        return true
      end
      stack = stack - 1
    elseif opening[tk.text] then
      stack = stack + 1
    end
  end
  return false
end

---------------------------------------------------------------------------
-- auto-enlarge brackets (upstream src/features/auto_enlarge_brackets.ts)

local ENLARGE_OPEN = { ["("] = ")", ["["] = "]", ["\\{"] = "\\}", ["\\langle"] = "\\rangle", ["\\lvert"] = "\\rvert", ["\\lVert"] = "\\rVert", ["\\lfloor"] = "\\rfloor", ["\\lceil"] = "\\rceil" }
local SIZE = {}
for _, n in ipairs({ "\\big", "\\Big", "\\bigg", "\\Bigg", "\\bigl", "\\Bigl", "\\biggl", "\\Biggl", "\\bigr", "\\Bigr", "\\biggr", "\\Biggr", "\\left", "\\right" }) do
  SIZE[n] = true
end

function M.auto_enlarge(ctx)
  if not S().enlarge then
    return false
  end
  ctx = ctx or context.get()
  if not ctx then
    return false
  end
  local b = M.bounds(ctx)
  if not b then
    return false
  end
  local text = get_text(ctx.buf, b.inner_start, b.inner_end)
  local toks = tokenize(text)
  local stack, edits = {}, {}
  for k, tk in ipairs(toks) do
    local prev = toks[k - 1]
    local sized = prev and SIZE[prev.text]
    if ENLARGE_OPEN[tk.text] then
      stack[#stack + 1] = { tk = tk, sized = sized, k = k }
    else
      for depth = #stack, 1, -1 do
        local o = stack[depth]
        if ENLARGE_OPEN[o.tk.text] == tk.text then
          for _ = depth, #stack do
            table.remove(stack)
          end
          if not o.sized and not sized then
            local content = text:sub(o.tk.e + 1, tk.s)
            for _, trig in ipairs(S().enlarge_triggers) do
              if content:find(trig, 1, true) then
                edits[#edits + 1] = { o.tk, tk }
                break
              end
            end
          end
          break
        end
      end
    end
  end
  if #edits == 0 then
    return false
  end
  local sp = S().enlarge_space and " " or ""
  -- apply from the end so earlier offsets stay valid
  local all = {}
  for _, e in ipairs(edits) do
    all[#all + 1] = { tk = e[1], rep = "\\left" .. e[1].text .. sp }
    all[#all + 1] = { tk = e[2], rep = sp .. "\\right" .. e[2].text }
  end
  table.sort(all, function(a, c)
    return a.tk.s > c.tk.s
  end)
  for _, a in ipairs(all) do
    local from = advance(b.inner_start, text, a.tk.s)
    local to = advance(b.inner_start, text, a.tk.e)
    api.nvim_buf_set_text(ctx.buf, from[1], from[2], to[1], to[2], { a.rep })
  end
  return true
end

---------------------------------------------------------------------------
-- auto-fraction (upstream src/features/autofraction.ts)

local AF_GREEK = {}
for _, g in ipairs(vim.split("alpha|beta|gamma|Gamma|delta|Delta|epsilon|varepsilon|zeta|eta|theta|Theta|iota|kappa|lambda|Lambda|mu|nu|omicron|xi|Xi|pi|Pi|rho|sigma|Sigma|tau|upsilon|Upsilon|varphi|phi|Phi|chi|psi|Psi|omega|Omega", "|", { plain = true })) do
  AF_GREEK[g] = true
end

local function find_matching_back(s, i, open, close)
  local depth = 0
  for j = i, 1, -1 do
    local c = s:sub(j, j)
    if c == close then
      depth = depth + 1
    elseif c == open then
      depth = depth - 1
      if depth == 0 then
        return j
      end
    end
  end
end

local function within_excluded(ctx, b)
  local cur = { ctx.row, ctx.col }
  local text = get_text(ctx.buf, b.inner_start, b.inner_end)
  local pos = #get_text(ctx.buf, b.inner_start, cur) -- 0-based offset of cursor
  local closer = { ["{"] = "}", ["["] = "]", ["("] = ")" }
  for _, env in ipairs(S().autofraction_excluded) do
    local ob = env.open:sub(-1)
    local offset, search = 0, env.open
    if closer[ob] and env.close == closer[ob] then
      offset = #env.open - 1
      search = ob
    end
    -- lastIndexOf(open, pos - 1)
    local left
    local from = 1
    while true do
      local p = text:find(env.open, from, true)
      if not p or p - 1 > pos - 1 then
        break
      end
      left = p - 1
      from = p + 1
    end
    while left do
      -- findMatchingBracket(text, left + offset, search, close)
      local depth, right = 0, nil
      local k = left + offset + 1
      while k <= #text do
        if text:sub(k, k + #search - 1) == search then
          depth = depth + 1
        elseif text:sub(k, k + #env.close - 1) == env.close then
          depth = depth - 1
          if depth == 0 then
            right = k - 1
            break
          end
        end
        k = k + 1
      end
      if not right then
        break
      end
      if right >= pos and pos >= left + #env.open then
        return true
      end
      if left <= 0 then
        break
      end
      local nl
      local f2 = 1
      while true do
        local p = text:find(env.open, f2, true)
        if not p or p - 1 > left - 1 then
          break
        end
        nl = p - 1
        f2 = p + 1
      end
      left = nl
    end
  end
  return false
end

function M.autofraction(ctx)
  if not S().autofraction or not context.strictly_in_math(ctx.mode) then
    return false
  end
  local b = M.bounds(ctx)
  if not b then
    return false
  end
  if within_excluded(ctx, b) then
    return false
  end
  local cur = { ctx.row, ctx.col }
  local line = get_text(ctx.buf, b.inner_start, cur)
  -- allow spaces after greek letters: "\alpha x" stays one token
  local masked = line:gsub("(%a+) ([^ ])", function(word, nxt)
    if AF_GREEK[word] then
      return word .. "#" .. nxt
    end
  end)
  local breaking = " $([{\n" .. S().autofraction_breaking
  local start = 0
  local i = #masked
  while i >= 1 do
    local ch = masked:sub(i, i)
    if ch == ")" or ch == "]" or ch == "}" then
      local open = ({ [")"] = "(", ["]"] = "[", ["}"] = "{" })[ch]
      local j = find_matching_back(masked, i, open, ch)
      if not j then
        return false
      end
      i = j
      ch = masked:sub(i, i)
      -- (upstream re-checks the *closing* char, which is never breaking)
      i = i - 1
    else
      if breaking:find(ch, 1, true) then
        start = i
        break
      end
      i = i - 1
    end
  end
  if start == #line then
    return false
  end
  local numerator = line:sub(start + 1)
  if numerator:sub(1, 1) == "(" and numerator:sub(-1) == ")" then
    local depth, close = 0, nil
    for k = 1, #numerator do
      local c = numerator:sub(k, k)
      if c == "(" then
        depth = depth + 1
      elseif c == ")" then
        depth = depth - 1
        if depth == 0 then
          close = k
          break
        end
      end
    end
    if close == #numerator then
      numerator = numerator:sub(2, -2)
    end
  end
  local t, i = ls.text_node, ls.insert_node
  local nodes
  if numerator == "" then
    nodes = { t(S().autofraction_symbol .. "{"), i(1), t("}{"), i(2), t("}") }
  else
    -- the numerator may span lines (display maths)
    local lines = vim.split(S().autofraction_symbol .. "{" .. numerator .. "}{", "\n", { plain = true })
    nodes = { t(lines), i(1), t("}") }
  end
  local from = advance(b.inner_start, line, start)
  expand_nodes(nodes, from, cur)
  vim.schedule(function()
    M.auto_enlarge()
  end)
  return true
end

---------------------------------------------------------------------------
-- auto-delete $ (upstream src/editor_extensions/autoDelete$.ts): Backspace in
-- an empty `$|$` (or between `$$|$$`) deletes both delimiters.

function M.auto_delete_dollar(ctx)
  if not S().auto_delete_dollar then
    return false
  end
  local l, c = ctx.line, ctx.col
  if l:sub(c - 1, c + 2) == "$$$$" and l:sub(c - 2, c - 2) ~= "$" and l:sub(c + 3, c + 3) ~= "$" then
    api.nvim_buf_set_text(ctx.buf, ctx.row, c - 2, ctx.row, c + 2, { "" })
    return true
  end
  if l:sub(c, c + 1) == "$$" and l:sub(c - 1, c - 1) ~= "$" and l:sub(c + 2, c + 2) ~= "$" then
    api.nvim_buf_set_text(ctx.buf, ctx.row, c - 1, ctx.row, c + 1, { "" })
    return true
  end
  return false
end

---------------------------------------------------------------------------
-- key handlers

local function blink()
  local ok, b = pcall(require, "blink.cmp")
  return ok and b or nil
end

function M.tab()
  local selecting = vim.fn.mode():sub(1, 1) == "s"
  if not selecting and ls.expandable() then
    ls.expand()
    return
  end
  if ls.locally_jumpable(1) then
    ls.jump(1)
    return
  end
  if selecting then
    feed("<Esc>a")
    return
  end
  local ctx = context.get()
  if ctx and (M.matrix_priority_tabout(ctx) or M.matrix_cell(ctx) or M.tabout(ctx)) then
    return
  end
  feed("<Tab>")
end

function M.shift_tab()
  if ls.locally_jumpable(-1) then
    ls.jump(-1)
    return
  end
  feed("<S-Tab>")
end

function M.enter()
  local b = blink()
  if b and b.is_visible() and b.get_selected_item() then
    b.accept()
    return
  end
  local ctx = context.get()
  if ctx and M.matrix_newline(ctx) then
    return
  end
  -- <C-g>u: start a new undo step for each new line
  feed("<C-g>u<CR>")
end

function M.shift_enter()
  local ctx = context.get()
  if ctx and M.matrix_exit(ctx) then
    return
  end
  feed("<CR>")
end

--- Would typing `/` here complete an autosnippet trigger (like //)?
local function completes_trigger(ctx)
  local ltc = ctx.line_to_cursor .. "/"
  -- every snippet lives under "tex" (.md reaches them through filetype_extend)
  for _, snip in ipairs(ls.get_snippets("tex", { type = "autosnippets" })) do
    local m = snip.ntex
    if m and not m.pattern and m.trig:sub(-1) == "/" and ltc:sub(-#m.trig) == m.trig then
      return true
    end
  end
  return false
end

function M.slash()
  local ctx = context.get()
  if ctx and S().snippets_enabled and context.in_math(ctx.mode) then
    -- an autosnippet ending in "/" (like //) wins over auto-fraction
    if completes_trigger(ctx) then
      feed("/")
      return
    end
  end
  if ctx and M.autofraction(ctx) then
    return
  end
  feed("/")
end

function M.closing(ch)
  local ctx = context.get()
  if ctx and S().tabout and context.in_math(ctx.mode) and ctx.line:sub(ctx.col + 1, ctx.col + 1) == ch then
    set_cursor({ ctx.row, ctx.col + 1 })
    return
  end
  feed(ch)
end

--- `{` right after `_{` or `^{` that is already closed (`x_{|}`, from the
--- `_` snippet): skip it, so typing x_{i} as written gives x_{i}, not x_{{i}}.
function M.opening(ch)
  local ctx = context.get()
  if ctx and ch == "{" and context.strictly_in_math(ctx.mode) then
    local before = ctx.line_to_cursor
    if (before:sub(-2) == "_{" or before:sub(-2) == "^{") and ctx.line:sub(ctx.col + 1, ctx.col + 1) == "}" then
      return
    end
  end
  feed(ch)
end

function M.backspace()
  local ctx = context.get()
  if ctx and M.auto_delete_dollar(ctx) then
    return
  end
  feed("<BS>")
end

--- Buffer-local insert-mode keys for tex/markdown buffers.
function M.attach(buf)
  local o = { buffer = buf, silent = true }
  vim.keymap.set({ "i", "s" }, "<Tab>", M.tab, vim.tbl_extend("force", o, { desc = "ntex: expand / next tabstop / tabout" }))
  vim.keymap.set({ "i", "s" }, "<S-Tab>", M.shift_tab, vim.tbl_extend("force", o, { desc = "ntex: previous tabstop" }))
  vim.keymap.set("i", "<CR>", M.enter, vim.tbl_extend("force", o, { desc = "ntex: matrix newline / accept completion" }))
  vim.keymap.set("i", "<S-CR>", M.shift_enter, vim.tbl_extend("force", o, { desc = "ntex: leave matrix row" }))
  vim.keymap.set("i", "/", M.slash, vim.tbl_extend("force", o, { desc = "ntex: auto-fraction" }))
  vim.keymap.set("i", "<BS>", M.backspace, vim.tbl_extend("force", o, { desc = "ntex: auto-delete $" }))
  vim.keymap.set("i", "{", function()
    M.opening("{")
  end, vim.tbl_extend("force", o, { desc = "ntex: skip { right after an auto-closed _{" }))
  for _, ch in ipairs({ ")", "]", "}" }) do
    vim.keymap.set("i", ch, function()
      M.closing(ch)
    end, vim.tbl_extend("force", o, { desc = "ntex: step over closing bracket" }))
  end
end

return M
