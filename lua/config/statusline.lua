-- Two rows of status, chosen 2026-10-15 ("ntex Looks", two-tier):
--
--   winbar (top of a .tex window)   where you are:  §1.2 Title › proof › align*      ∫ math
--   statusline (bottom)             how it's going: ✓ compiled 0.9s   file.tex [+]   ⇥ snippet  E1 W1 H2  27:31
--
-- The math / text marker uses the same test the snippets use before they fire
-- (lua/snips/context.lua, strictly_in_math), so when a snippet does or doesn't
-- expand, the marker says which side of that test the cursor was on:
--   ∫ math   math snippets fire here
--   ¶ \text  inside \text{} (or \label{}, \ce{} …) within math: they don't
--   ¶ text   prose
--   % comment
--
-- The compile segment matters because a failed tectonic run leaves the PDF
-- pane showing the last good build: ✓ compiled 0.9s / ◌ compiling… /
-- ✗ 2 errors · line 14 (the first error, from lua/ide/compile_errors.lua).
--
-- E W I H are diagnostic counts (errors, warnings, info, hints; Harper's
-- grammar and spelling notes are most of the hints), shown only when non-zero.
-- Colors: the Ntex* groups in lua/config/theme.lua.
local M = {}

local api = vim.api

---------------------------------------------------------------------------
-- compile state (one document per ntex, so one state)

M.compile = { state = nil, started = nil, secs = nil }

local function redraw()
  vim.schedule(function()
    pcall(vim.cmd, "redrawstatus!")
  end)
end

---------------------------------------------------------------------------
-- location: section number + title, open environments (cached per tick+line)

local LEVELS = { part = 0, chapter = 1, section = 2, subsection = 3, subsubsection = 4 }
local cache = { buf = -1, tick = -1, row = -1, val = nil }

local function title_of(arg)
  local t = arg:sub(2, -2)
  t = t:gsub("%$", ""):gsub("\\%a+%s*", ""):gsub("[{}]", ""):gsub("%s+", " ")
  t = vim.trim(t)
  if vim.fn.strchars(t) > 40 then
    t = vim.fn.strcharpart(t, 0, 39) .. "…"
  end
  return t
end

local function location(buf, row)
  local tick = api.nvim_buf_get_changedtick(buf)
  if cache.buf == buf and cache.tick == tick and cache.row == row then
    return cache.val
  end
  local lines = api.nvim_buf_get_lines(buf, 0, row, false)
  local count = { 0, 0, 0, 0, 0 } -- part … subsubsection
  local top -- highest level used (chapter or section)
  local cur -- { level, number?, title }
  local envs = {}
  for _, line in ipairs(lines) do
    line = line:gsub("\\%%", "  "):gsub("%%.*$", "") -- drop comments
    for kw, star, arg in line:gmatch("\\(%a+)(%*?)%s*(%b{})") do
      local lvl = LEVELS[kw]
      if lvl then
        if star == "" then
          count[lvl + 1] = count[lvl + 1] + 1
          for k = lvl + 2, 5 do
            count[k] = 0
          end
        end
        if lvl > 0 then
          top = top and math.min(top, lvl) or lvl
        end
        cur = { level = lvl, starred = star ~= "", title = title_of(arg) }
        envs = {} -- environments are not left open across a section heading; anything unclosed above it is noise
      elseif kw == "begin" then
        local name = arg:sub(2, -2)
        if name ~= "document" then
          envs[#envs + 1] = name
        end
      elseif kw == "end" then
        local name = arg:sub(2, -2)
        for k = #envs, 1, -1 do
          if envs[k] == name then
            for j = #envs, k, -1 do
              envs[j] = nil
            end
            break
          end
        end
      end
    end
  end
  local parts = {}
  if cur then
    local num = ""
    if not cur.starred and cur.level > 0 then
      local nums = {}
      for k = (top or cur.level) + 1, cur.level + 1 do
        nums[#nums + 1] = tostring(count[k])
      end
      num = table.concat(nums, ".")
    end
    parts[1] = "§" .. num .. " " .. cur.title
  end
  for _, e in ipairs(envs) do
    parts[#parts + 1] = e
  end
  cache.buf, cache.tick, cache.row, cache.val = buf, tick, row, parts
  return parts
end

local function marker()
  local ok, context = pcall(require, "snips.context")
  if not ok then
    return nil
  end
  local ok2, ctx = pcall(context.get)
  if not ok2 or not ctx then
    return nil
  end
  local mode = ctx.mode
  if mode.comment then
    return "%#NtexWinbar#%% comment "
  elseif context.strictly_in_math(mode) then
    return "%#NtexWinbarMath#∫ math "
  elseif context.in_math(mode) then
    return "%#NtexWinbar#¶ \\text "
  end
  return "%#NtexWinbar#¶ text "
end

local function esc(s)
  return (s:gsub("%%", "%%%%"))
end

function M.winbar()
  local win = vim.g.statusline_winid or api.nvim_get_current_win()
  local buf = api.nvim_win_get_buf(win)
  local row = api.nvim_win_get_cursor(win)[1]
  local parts = location(buf, row)
  local out = { "%#NtexWinbar# %<" }
  for i, p in ipairs(parts) do
    if i > 1 then
      out[#out + 1] = "%#NtexWinbarSep# › %#NtexWinbar#"
    end
    out[#out + 1] = esc(p)
  end
  out[#out + 1] = "%="
  if win == api.nvim_get_current_win() then
    out[#out + 1] = marker() or ""
  end
  return table.concat(out)
end

---------------------------------------------------------------------------
-- bottom bar

local S = vim.diagnostic.severity

local function compile_seg(buf)
  local c = M.compile
  if c.state == "running" then
    return "%#NtexBusy# ◌ compiling… "
  elseif c.state ~= "ok" and c.state ~= "failed" then
    return ""
  end
  -- VimTeX decides success from its quickfix parse, which can come back empty
  -- (it swallows its own errors) for a run tectonic failed; the errors
  -- compile_errors.lua read from the log are the better witness.
  local ns = package.loaded["ide.compile_errors"] and package.loaded["ide.compile_errors"].ns
  local errs = ns and vim.diagnostic.get(buf, { namespace = ns, severity = S.ERROR }) or {}
  if #errs > 0 then
    table.sort(errs, function(a, b)
      return a.lnum < b.lnum
    end)
    return string.format("%%#NtexErr# ✗ %d error%s · line %d ", #errs, #errs == 1 and "" or "s", errs[1].lnum + 1)
  elseif c.state == "failed" then
    return "%#NtexErr# ✗ compile failed "
  end
  return "%#NtexOk# ✓ compiled" .. (c.secs and string.format(" %.1fs", c.secs) or "") .. " "
end

local function diag_seg(buf)
  local n = vim.diagnostic.count(buf)
  local out = {}
  local function add(sev, letter, group)
    if (n[sev] or 0) > 0 then
      out[#out + 1] = string.format("%%#%s#%s%d", group, letter, n[sev])
    end
  end
  add(S.ERROR, "E", "NtexErr")
  add(S.WARN, "W", "NtexWarn")
  add(S.INFO, "I", "NtexInfo")
  add(S.HINT, "H", "NtexHint")
  return #out > 0 and (table.concat(out, " ") .. "  ") or ""
end

local function snippet_seg()
  local m = api.nvim_get_mode().mode:sub(1, 1)
  if m ~= "i" and m ~= "s" then
    return ""
  end
  local ok, ls = pcall(require, "luasnip")
  if ok and ls.in_snippet and ls.in_snippet() then
    return "%#NtexSnip#⇥ snippet  "
  end
  return ""
end

function M.render()
  local win = vim.g.statusline_winid or api.nvim_get_current_win()
  local buf = api.nvim_win_get_buf(win)
  local active = win == api.nvim_get_current_win()
  if not active then
    return " %f%( %m%)%=%l:%v "
  end
  local tex = vim.bo[buf].filetype == "tex"
  return table.concat({
    tex and compile_seg(buf) or "",
    "%#StatusLine#  %<%f%( %m%)",
    "%=",
    snippet_seg(),
    diag_seg(buf),
    "%#StatusLine#%l:%v ",
  })
end

---------------------------------------------------------------------------

local function want_winbar(win, buf)
  return api.nvim_win_get_config(win).relative == "" and vim.bo[buf].filetype == "tex" and vim.bo[buf].buftype == ""
end

local WINBAR = "%!v:lua.require'config.statusline'.winbar()"

function M.setup()
  vim.o.statusline = "%!v:lua.require'config.statusline'.render()"
  vim.o.ruler = false -- position is in the bar

  local grp = api.nvim_create_augroup("texnvim_statusline", { clear = true })
  api.nvim_create_autocmd({ "BufWinEnter", "FileType", "WinEnter" }, {
    group = grp,
    callback = function()
      local win = api.nvim_get_current_win()
      local buf = api.nvim_win_get_buf(win)
      if want_winbar(win, buf) then
        if vim.wo[win].winbar ~= WINBAR then
          vim.wo[win].winbar = WINBAR
        end
      elseif vim.wo[win].winbar == WINBAR then
        vim.wo[win].winbar = ""
      end
    end,
  })
  api.nvim_create_autocmd("User", {
    group = grp,
    pattern = "VimtexEventCompileStarted",
    callback = function()
      M.compile.state, M.compile.started, M.compile.secs = "running", vim.uv.hrtime(), nil
      redraw()
    end,
  })
  api.nvim_create_autocmd("User", {
    group = grp,
    pattern = { "VimtexEventCompileSuccess", "VimtexEventCompileFailed", "VimtexEventCompileStopped" },
    callback = function(ev)
      local c = M.compile
      if ev.match == "VimtexEventCompileStopped" then
        c.state = nil
      else
        c.state = ev.match == "VimtexEventCompileSuccess" and "ok" or "failed"
        c.secs = c.started and (vim.uv.hrtime() - c.started) / 1e9 or nil
      end
      c.started = nil
      -- compile_errors.lua sets its diagnostics in the same event; draw after it
      redraw()
    end,
  })
  api.nvim_create_autocmd({ "DiagnosticChanged", "ModeChanged" }, {
    group = grp,
    callback = redraw,
  })
end

return M
