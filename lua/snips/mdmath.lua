-- Markdown math-zone scanner: `$…$` (inline) and `$$…$$` (display).
--
-- Treesitter's markdown_inline `latex_block` only exists once a zone is
-- closed and never spans a blank line, so it is wrong in exactly the moments
-- that matter (just after `dm` inserts `$$\n\n$$`). This is a small,
-- deterministic scanner instead, with the state at each line start cached per
-- buffer and invalidated from the first changed line (nvim_buf_attach).
--
-- Rules (close to Obsidian): `\$` is escaped; `$$` toggles display math and may
-- span lines (blank lines included); a single `$` opens inline math unless it is
-- followed by whitespace, and inline math ends at the next `$` or at end of
-- line; nothing counts inside inline code spans or fenced code blocks, except a
-- ```math fence, which is display math (upstream's forceMathLanguages).

local M = {}

local caches = {} ---@type table<integer, {valid: integer, states: table}>

-- fenced code languages treated as display math (upstream forceMathLanguages)
M.force_math = { "math" }
local function force(lang)
  return vim.tbl_contains(M.force_math, lang)
end

local function cache_for(buf)
  local c = caches[buf]
  if not c then
    c = { valid = 0, states = {} }
    caches[buf] = c
    vim.api.nvim_buf_attach(buf, false, {
      on_lines = function(_, _, _, first)
        local cc = caches[buf]
        if not cc then
          return true
        end
        -- states[i] is the state at the start of line i (1-based); a change on
        -- line first+1 (1-based) invalidates states from first+2 onward.
        if cc.valid > first + 1 then
          cc.valid = first + 1
        end
      end,
      on_detach = function()
        caches[buf] = nil
      end,
      on_reload = function()
        local cc = caches[buf]
        if cc then
          cc.valid = 0
        end
      end,
    })
  end
  return c
end

local function fence_open(line)
  local lead, marker, info = line:match("^([%s>]*)(```+)%s*([^`]*)$")
  if not marker then
    lead, marker, info = line:match("^([%s>]*)(~~~+)%s*(.*)$")
  end
  if marker then
    local lang = (info or ""):match("^([%w_+%-%.]*)") or ""
    return marker, lang
  end
end

local function fence_close(line, marker)
  local lead, m = line:match("^([%s>]*)([`~]+)%s*$")
  return m and m:sub(1, 1) == marker:sub(1, 1) and #m >= #marker
end

-- Scan `line` from its start given line-start `state`, up to byte `stop`
-- (exclusive, 0-based: characters [1, stop] are consumed) or to the end.
-- Returns the state at `stop` (math/code info) and the state for the next line.
local function scan(line, state, stop, row)
  local display = state.display -- nil or {row=, col=} (0-based inner start)
  local fence = state.fence -- nil or {marker=, lang=}
  local here = { display = nil, inline = nil, code = false, fence = nil }

  if fence then
    if fence_close(line, fence.marker) then
      here.fence = fence
      return here, { display = display }
    end
    here.fence = fence
    return here, { display = display, fence = fence }
  end
  if not display then
    local marker, lang = fence_open(line)
    if marker then
      here.fence = nil
      here.code = true
      return here, { display = nil, fence = { marker = marker, lang = lang, row = row } }
    end
  end

  local n = #line
  stop = stop or n
  local i = 1
  local inline = nil
  while i <= n do
    if i > stop then
      break
    end
    local j = line:find("[\\`%$]", i)
    if not j or j > stop then
      break
    end
    local c = line:sub(j, j)
    if c == "\\" then
      i = j + 2
    elseif c == "`" then
      if display or inline then
        i = j + 1
      else
        local run = line:match("^`+", j)
        local close = line:find(run, j + #run, true)
        -- make sure the closing run is exactly as long
        while close and line:sub(close + #run, close + #run) == "`" do
          close = line:find(run, close + #run + 1, true)
        end
        if close then
          if stop < close + #run - 1 then
            -- cursor inside the code span
            here.code = true
            return here, nil
          end
          i = close + #run
        else
          i = j + #run
        end
      end
    else -- "$"
      -- a "$$" split by the cursor ($|$) is an empty inline zone
      local double = line:sub(j + 1, j + 1) == "$" and j + 1 <= stop
      if display then
        if double then
          display = nil
          i = j + 2
        else
          i = j + 1
        end
      elseif inline then
        inline = nil
        i = j + 1
      elseif double then
        display = { row = row, col = j + 1 } -- inner start, 0-based col
        i = j + 2
      else
        local nxt = line:sub(j + 1, j + 1)
        if nxt == "" or not nxt:match("%s") then
          inline = { row = row, col = j } -- inner start, 0-based col
        end
        i = j + 1
      end
    end
  end
  here.display = display
  here.inline = inline
  return here, { display = display }
end

--- Line-start state for 1-based line `lnum`.
local function state_at(buf, lnum)
  local c = cache_for(buf)
  if c.valid < 1 then
    c.states[1] = {}
    c.valid = 1
  end
  if c.valid >= lnum then
    return c.states[lnum]
  end
  local lines = vim.api.nvim_buf_get_lines(buf, c.valid - 1, lnum - 1, false)
  local st = c.states[c.valid]
  for k, line in ipairs(lines) do
    local l = c.valid + k - 1
    local _, nxt = scan(line, st, nil, l - 1)
    st = nxt or {}
    c.states[l + 1] = st
  end
  c.valid = lnum
  return st
end

--- Math state at (row 0-based, col 0-based byte = number of chars before cursor).
--- @return {kind: "inline"|"display"|nil, start: {row:integer,col:integer}?, code: boolean, codeblock: string|false, codemath: boolean}
function M.at(buf, row, col, line)
  local st = state_at(buf, row + 1)
  line = line or (vim.api.nvim_buf_get_lines(buf, row, row + 1, false)[1] or "")
  local here = scan(line, st, col, row)
  local res = { kind = nil, start = nil, code = here.code, codeblock = false, codemath = false }
  if here.fence then
    if force(here.fence.lang) then
      res.codemath = true
      res.kind = "display"
      res.start = { row = here.fence.row + 1, col = 0 }
    else
      res.codeblock = here.fence.lang ~= "" and here.fence.lang or true
    end
    return res
  end
  if here.code then
    return res
  end
  if here.inline then
    res.kind = "inline"
    res.start = here.inline
  elseif here.display then
    res.kind = "display"
    res.start = here.display
  end
  return res
end

--- Where the math zone containing the cursor ends: returns inner_end and
--- outer_end as {row, col} (0-based, end-exclusive), or nil.
function M.find_end(buf, row, col, kind)
  local lines = vim.api.nvim_buf_get_lines(buf, row, row + 200, false)
  for k, line in ipairs(lines) do
    local from = (k == 1) and (col + 1) or 1
    local i = from
    while true do
      local j = line:find("[\\%$]", i)
      if not j then
        break
      end
      if line:sub(j, j) == "\\" then
        i = j + 2
      else
        local double = line:sub(j + 1, j + 1) == "$"
        if kind == "display" and double then
          return { row = row + k - 1, col = j - 1 }, { row = row + k - 1, col = j + 1 }
        elseif kind == "inline" then
          return { row = row + k - 1, col = j - 1 }, { row = row + k - 1, col = j }
        end
        i = j + 1
      end
    end
    if kind == "inline" then
      return nil
    end
  end
end

function M.reset(buf)
  local c = caches[buf]
  if c then
    c.valid = 0
  end
end

--- Byte ranges {s, e} (0-based, end-exclusive) of math content on line `row`
--- (used by the markdown conceal). Delimiters are not included.
function M.spans(buf, row, line)
  local st = state_at(buf, row + 1)
  local out = {}
  if st.fence then
    if force(st.fence.lang) and not fence_close(line, st.fence.marker) then
      out[1] = { 0, #line }
    end
    return out
  end
  local display = st.display and 0 or nil
  local inline = nil
  local i, n = 1, #line
  if not display and fence_open(line) then
    return out
  end
  while i <= n do
    local j = line:find("[\\`%$]", i)
    if not j then
      break
    end
    local c = line:sub(j, j)
    if c == "\\" then
      i = j + 2
    elseif c == "`" then
      if display or inline then
        i = j + 1
      else
        local run = line:match("^`+", j)
        local close = line:find(run, j + #run, true)
        i = close and (close + #run) or (j + #run)
      end
    else
      local double = line:sub(j + 1, j + 1) == "$"
      if display then
        if double then
          out[#out + 1] = { display, j - 1 }
          display = nil
          i = j + 2
        else
          i = j + 1
        end
      elseif inline then
        out[#out + 1] = { inline, j - 1 }
        inline = nil
        i = j + 1
      elseif double then
        display = j + 1
        i = j + 2
      else
        local nxt = line:sub(j + 1, j + 1)
        if nxt == "" or not nxt:match("%s") then
          inline = j
        end
        i = j + 1
      end
    end
  end
  if display then
    out[#out + 1] = { display, n }
  end
  return out
end

return M
