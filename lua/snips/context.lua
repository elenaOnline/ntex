-- Per-keystroke editing context, shared by every snippet condition and by the
-- Tab/Enter/`/` features. Computed once per (buffer, changedtick, cursor) and
-- cached, so the conditions of several matching snippets cost one computation.
--
-- Mode flags (names kept from Latex Suite, which this grew out of):
--   text, inlineMath, blockMath, codeMath, codeBlock (false|lang), code,
--   textEnv (inside \text{} etc. in math), snippetlessEnv (\begin{}, \label{},
--   \ce{}, … in math: no maths snippets), comment (a % comment in .tex)
--
-- Math detection:
--   tex      → VimTeX's syntax stack, using the same rule as
--              vimtex#syntax#in_mathzone(): the innermost of texMathZone*/
--              texMathText*/texMathTag/texRefArg decides. The zone's group name
--              tells inline ($…$, \(…\), \ensuremath) from display.
--   markdown → lua/snips/mdmath.lua ($ / $$ scanner).

local api = vim.api
local mdmath = require("snips.mdmath")

local M = {}

M.filetypes = { tex = "tex", plaintex = "tex", markdown = "markdown" }

-- upstream src/editor_context/default_text_areas.ts (order matters: the first
-- entry with a given name decides which argument counts)
local TEXT_AREA = {
  { "text", { 0 } }, { "textrm", { 0 } }, { "textup", { 0 } }, { "textit", { 0 } }, { "textbf", { 0 } },
  { "textsf", { 0 } }, { "texttt", { 0 } }, { "textnormal", { 0 } }, { "clap", { 0 } }, { "textllap", { 0 } },
  { "textrlap", { 0 } }, { "textclap", { 0 } }, { "hbox", { 0 } }, { "mbox", { 0 } }, { "fbox", { 0 } },
  { "framebox", { 0 } }, { "colorbox", { 1 } }, { "fcolorbox", { 2 } },
}
local SNIPPETLESS = {
  { "tag", { 0 } }, { "begin", { 0 } }, { "end", { 0 } }, { "mmlToken", { 0, 1 } }, { "unicode", { 0 } },
  { "textcolor", { 0 } }, { "color", { 0 } }, { "colorbox", { 0 } }, { "fcolorbox", { 0, 1 } },
  { "operatorname", { 0 } }, { "style", { 0 } },
  -- ntex: labels, references and chemistry (mhchem) arguments have their own
  -- syntax; maths snippets would break it (\ce{A -> B}, \label{eq:sub}).
  { "label", { 0 } }, { "ref", { 0 } }, { "eqref", { 0 } }, { "cref", { 0 } }, { "Cref", { 0 } },
  { "autoref", { 0 } }, { "cite", { 0 } }, { "ce", { 0 } }, { "pu", { 0 } },
  { "url", { 0 } }, { "href", { 0 } },
}
local ALL_TEXT_AREAS = {}
local SNIPPETLESS_NAMES = {}
for _, e in ipairs(TEXT_AREA) do
  if not ALL_TEXT_AREAS[e[1]] then ALL_TEXT_AREAS[e[1]] = e[2] end
end
for _, e in ipairs(SNIPPETLESS) do
  if not ALL_TEXT_AREAS[e[1]] then ALL_TEXT_AREAS[e[1]] = e[2] end
  SNIPPETLESS_NAMES[e[1]] = true
end

-- Environments that *are* a math zone in LaTeX (a boundary for scope scans).
M.MATH_ENVS = {}
for _, n in ipairs({ "equation", "align", "alignat", "flalign", "gather", "multline", "eqnarray", "math", "displaymath", "dmath" }) do
  M.MATH_ENVS[n] = true
  M.MATH_ENVS[n .. "*"] = true
end

local INLINE_ZONES = { texMathZoneTI = true, texMathZoneLI = true, texMathZoneEnsured = true, texMathZoneX = true }

---------------------------------------------------------------------------
-- Scope scanner: which macro arguments / environments enclose the cursor?

local B_BS, B_LB, B_RB, B_LS, B_RS, B_DOLLAR = 92, 123, 125, 91, 93, 36

local function escaped(s, i)
  local n = 0
  i = i - 1
  while i >= 1 and s:byte(i) == B_BS do
    n = n + 1
    i = i - 1
  end
  return n % 2 == 1
end

-- find the unescaped opening bracket matching the closer at i (backwards)
local function match_back(s, i, open, close)
  local depth = 0
  while i >= 1 do
    local c = s:byte(i)
    if c == close and not escaped(s, i) then
      depth = depth + 1
    elseif c == open and not escaped(s, i) then
      depth = depth - 1
      if depth == 0 then
        return i
      end
    end
    i = i - 1
  end
end

local function is_letter(c)
  return c and ((c >= 65 and c <= 90) or (c >= 97 and c <= 122))
end

-- For an unmatched `{` at i: which command owns it, and which argument is it?
local function owner(s, i)
  local j = i - 1
  local arg = 0
  while j >= 1 do
    local c = s:byte(j)
    if c == 32 or c == 9 then
      j = j - 1
    elseif c == B_RB and not escaped(s, j) then
      local k = match_back(s, j, B_LB, B_RB)
      if not k then
        return nil
      end
      arg = arg + 1
      j = k - 1
    elseif c == B_RS and not escaped(s, j) then
      local k = match_back(s, j, B_LS, B_RS)
      if not k then
        return nil
      end
      j = k - 1
    elseif is_letter(c) then
      local k = j
      while k > 1 and is_letter(s:byte(k - 1)) do
        k = k - 1
      end
      if k > 1 and s:byte(k - 1) == B_BS and not escaped(s, k - 1) then
        local name = s:sub(k, j)
        if s:byte(j + 1) == 42 then -- starred: \operatorname*{
          name = name .. "*"
        end
        return name, arg, k - 1
      end
      return nil
    else
      return nil
    end
  end
end

--- Innermost-first list of frames enclosing the end of `s`:
---   {kind="command", name=, arg=} | {kind="group"} | {kind="environment", name=} | {kind="math"}
function M.scope_stack(s, stop_at_dollar)
  local frames = {}
  local ends = {}
  local depth = 0
  local i = #s
  while i >= 1 do
    local c = s:byte(i)
    if c == B_RB then
      if not escaped(s, i) then
        depth = depth + 1
      end
      i = i - 1
    elseif c == B_LB then
      if escaped(s, i) then
        i = i - 1
      elseif depth > 0 then
        depth = depth - 1
        i = i - 1
      else
        local name, arg, start = owner(s, i)
        if name then
          frames[#frames + 1] = { kind = "command", name = name, arg = arg }
          i = start - 1
        else
          frames[#frames + 1] = { kind = "group" }
          i = i - 1
        end
      end
    elseif c == B_BS and not escaped(s, i) then
      local nxt = s:byte(i + 1)
      if nxt == 40 or nxt == B_LS then -- \( or \[
        frames[#frames + 1] = { kind = "math" }
        return frames
      end
      local kw, env = s:match("^\\(%a+)%s*{([^{}]*)}", i)
      if kw == "end" and depth == 0 then
        ends[#ends + 1] = env
      elseif kw == "begin" and depth == 0 then
        if ends[#ends] == env then
          ends[#ends] = nil
        else
          frames[#frames + 1] = { kind = "environment", name = env }
          if M.MATH_ENVS[env] then
            frames[#frames + 1] = { kind = "math" }
            return frames
          end
        end
      end
      i = i - 1
    elseif c == B_DOLLAR and stop_at_dollar and not escaped(s, i) and depth == 0 then
      frames[#frames + 1] = { kind = "math" }
      return frames
    else
      i = i - 1
    end
  end
  frames[#frames + 1] = { kind = "math" }
  return frames
end

--- upstream isMacroArgumentCount
function M.frame_matches(frame, areas)
  for _, a in ipairs(areas) do
    if a.name == frame.name then
      if a.arguments == nil or a.arguments == vim.NIL then
        return true
      end
      for _, n in ipairs(a.arguments) do
        if n == frame.arg then
          return true
        end
      end
      return false
    end
  end
  return false
end

-- upstream Context.inTextEnvironment(): "text" | "none" | nil
local function text_environment(stack)
  for _, f in ipairs(stack) do
    if f.kind == "math" then
      return nil
    elseif f.kind == "command" then
      local args = ALL_TEXT_AREAS[f.name]
      if args and vim.tbl_contains(args, f.arg) then
        return SNIPPETLESS_NAMES[f.name] and "none" or "text"
      end
    end
  end
  return nil
end

---------------------------------------------------------------------------
-- Math detection

--- tex: VimTeX syntax. Returns kind ("inline"/"display"/nil) and textEnv flag.
local function tex_stack(lnum, col)
  local ok, names = pcall(vim.fn["vimtex#syntax#stack"], lnum, math.max(col, 1))
  if not ok or type(names) ~= "table" then
    return {}
  end
  return names
end

local function tex_math(lnum, col)
  local names = tex_stack(lnum, col)
  for _, n in ipairs(names) do
    if n:find("^texComment") then
      return nil, false, false, true
    end
  end
  -- The character before the cursor is a math delimiter ($, \), \], …). It is
  -- part of the zone both when it opens it ($|x$) and when it closes it
  -- ($x$|); only in the first case is the cursor inside math.
  local last = names[#names]
  if last and last:find("^texMathDelim") then
    local zone = names[#names - 1]
    local line = vim.api.nvim_buf_get_lines(0, lnum - 1, lnum, false)[1] or ""
    if col >= #line then
      return nil, false, false
    end
    local after = tex_stack(lnum, col + 1)
    if not zone or not vim.tbl_contains(after, zone) then
      return nil, false, false
    end
  end
  local innermost, zone
  for k = #names, 1, -1 do
    local n = names[k]
    if not innermost and (n:find("^texMathZone") or n:find("^texMathText") or n == "texMathTag" or n:find("^texRefArg")) then
      innermost = n
    end
    if n:find("^texMathZone") then
      zone = n
      if innermost then
        break
      end
    end
  end
  if not zone then
    return nil, false, false
  end
  local kind = INLINE_ZONES[zone] and "inline" or "display"
  if innermost and innermost:find("^texMathZone") then
    return kind, false, false
  elseif innermost and innermost:find("^texMathText") then
    return kind, true, false
  end
  return kind, false, true -- \tag{}, \ref{} inside math: no snippets
end

-- VimTeX doesn't parse maths inside optional arguments ([…]), so in
-- \begin{proof}[NTS: $x$] or \item[$a$] it reports text. Fallback, current
-- line only: is the cursor inside an inline $…$ or \(…\) that was opened inside
-- an unclosed [ ? Returns the 0-based column where the maths content starts.
local function bracketed_inline_math(s)
  local i, n = 1, #s
  local sq = 0 -- unclosed [ outside maths
  local open, open_sq = nil, 0
  while i <= n do
    local c = s:sub(i, i)
    if c == "\\" then
      local nx = s:sub(i + 1, i + 1)
      if nx == "(" and not open then
        open, open_sq = i + 1, sq -- content starts after "\("
      elseif nx == ")" and open then
        open = nil
      end
      i = i + 2 -- \$ \[ \] \{ … are escapes/other delimiters, never counted
    elseif c == "%" then
      return nil -- the rest of the line is a comment
    elseif c == "$" then
      if s:sub(i + 1, i + 1) == "$" then
        i = i + 2 -- $$ display maths: VimTeX handles those
      else
        if open then
          open = nil
        else
          open, open_sq = i, sq -- content starts after "$"
        end
        i = i + 1
      end
    else
      if not open then
        if c == "[" then
          sq = sq + 1
        elseif c == "]" and sq > 0 then
          sq = sq - 1
        end
      end
      i = i + 1
    end
  end
  if open and open_sq > 0 then
    return open
  end
end
M._bracketed_inline_math = bracketed_inline_math

---------------------------------------------------------------------------

local cache = { buf = -1, tick = -1, row = -1, col = -1, ctx = nil }

local Ctx = {}
Ctx.__index = Ctx

--- Scope frames (innermost first) — only meaningful in math.
function Ctx:stack()
  if self._stack then
    return self._stack
  end
  local text
  if self.math_start then
    local s = self.math_start
    local lines = api.nvim_buf_get_lines(self.buf, s.row, self.row + 1, false)
    lines[#lines] = self.line_to_cursor
    lines[1] = lines[1]:sub(s.col + 1)
    if #lines == 1 and s.row == self.row then
      lines[1] = self.line_to_cursor:sub(s.col + 1)
    end
    text = table.concat(lines, "\n")
    self._stack = M.scope_stack(text, false)
  else
    local from = math.max(0, self.row - 40)
    local lines = api.nvim_buf_get_lines(self.buf, from, self.row + 1, false)
    lines[#lines] = self.line_to_cursor
    text = table.concat(lines, "\n")
    self._stack = M.scope_stack(text, true)
  end
  return self._stack
end

function M.get_at(buf, row, col)
  local ft = M.filetypes[vim.bo[buf].filetype]
  if not ft then
    return nil
  end
  local line = api.nvim_buf_get_lines(buf, row, row + 1, false)[1] or ""
  local ctx = setmetatable({
    buf = buf,
    ft = ft,
    row = row,
    col = col,
    line = line,
    line_to_cursor = line:sub(1, col),
    blocked = false,
  }, Ctx)
  local mode = {
    text = false, inlineMath = false, blockMath = false, codeMath = false,
    codeBlock = false, code = false, textEnv = false, snippetlessEnv = false, comment = false,
  }
  ctx.mode = mode
  local kind
  if ft == "tex" then
    local text_env, snippetless, comment
    kind, text_env, snippetless, comment = tex_math(row + 1, col)
    if text_env then mode.textEnv = true end
    if snippetless then mode.snippetlessEnv = true end
    if comment then mode.comment = true end
    if not kind and not comment then
      local start = bracketed_inline_math(ctx.line_to_cursor)
      if start then
        kind = "inline"
        ctx.math_start = { row = row, col = start }
      end
    end
  else
    local st = mdmath.at(buf, row, col, line)
    kind = st.kind
    ctx.math_start = st.start
    mode.code = st.code
    mode.codeBlock = st.codeblock
    if st.codemath then
      mode.codeMath = true
      kind = nil
    end
  end
  ctx.math_kind = kind or (mode.codeMath and "display") or nil
  if kind == "inline" then
    mode.inlineMath = true
  elseif kind == "display" then
    mode.blockMath = true
  end
  local in_math = mode.inlineMath or mode.blockMath or mode.codeMath
  if in_math and not mode.snippetlessEnv then
    local te = text_environment(ctx:stack())
    if te == "text" then
      mode.textEnv = true
    elseif te == "none" then
      mode.snippetlessEnv = true
    end
  end
  mode.text = (mode.codeBlock == false) and not in_math and not mode.code and not mode.comment
  return ctx
end

--- Context at the cursor of the current window, cached per keystroke.
function M.get()
  local buf = api.nvim_get_current_buf()
  local tick = api.nvim_buf_get_changedtick(buf)
  local cur = api.nvim_win_get_cursor(0)
  if cache.buf == buf and cache.tick == tick and cache.row == cur[1] and cache.col == cur[2] then
    return cache.ctx
  end
  local ctx = nil
  if M.filetypes[vim.bo[buf].filetype] then
    ctx = M.get_at(buf, cur[1] - 1, cur[2])
  end
  cache.buf, cache.tick, cache.row, cache.col, cache.ctx = buf, tick, cur[1], cur[2], ctx
  return ctx
end

function M.invalidate()
  cache.buf, cache.ctx = -1, nil
end

--- in maths at all (also inside \text{} there)
function M.in_math(mode)
  return (mode.inlineMath or mode.blockMath or mode.codeMath) and not mode.comment
end
--- in maths, and not inside \text{} / \label{} / \begin{} / \ce{} …: where maths snippets fire
function M.strictly_in_math(mode)
  return M.in_math(mode) and not mode.textEnv and not mode.snippetlessEnv
end

return M
