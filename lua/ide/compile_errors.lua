-- Compile errors where they happen: after every VimTeX compile (live PDF's
-- autosave, Ctrl-s, \ll) the tectonic log is read and its errors and
-- warnings become diagnostics (namespace "tectonic") on the right file and
-- line, \input files included. A compile without errors clears them.
--
-- Why not VimTeX's error list? Its log parser expects pdflatex's
-- -file-line-error format; tectonic writes the classic "! message" + "l.12"
-- format, so "Undefined control sequence" or "Missing $ inserted" never
-- reached \le (and VimTeX called those compiles successful). This reads the
-- log itself:
--   ! <error>  …  l.<line> <text up to the error>   → error at that line,
--                                                       underlining the
--                                                       last token TeX read
--   "… on input line N" in the message               → line N (e.g. the
--                                                       \begin{align} that
--                                                       was never ended)
--   Runaway argument? <text>                         → the line holding that text
--   LaTeX / Package / Class … Warning … line N       → warning
--   Overfull / Underfull \hbox … at lines N--M       → hidden, or a faint
--                                                       hint (prefs.lua
--                                                       box_warnings = true)
-- The file comes from TeX's "(file" … ")" stack in the log, checked against
-- the text TeX quotes.
--
-- While you type (insert mode) an error on the line you are typing in waits
-- until you press Esc: live PDF compiles mid-equation, and a red note under
-- the half-typed formula would only be noise. Errors elsewhere show at once.
local prefs = require("config.prefs")

local M = {}
M.ns = vim.api.nvim_create_namespace("tectonic")
M.last = {} -- bufnr -> full list (the insert-mode filter hides some)
M.owned = {} -- main .tex -> { bufnr = true } holding our diagnostics
M.stats = { runs = 0, last_ms = 0 }

local S = vim.diagnostic.severity

local function read(path)
  local fd = io.open(path, "r")
  if not fd then
    return nil
  end
  local s = fd:read("*a")
  fd:close()
  return s
end

local function exists(p)
  local st = vim.uv.fs_stat(p)
  return st ~= nil and st.type == "file"
end

--- a file name as TeX prints it ("chap", "./sec/a.tex", "/abs/x.tex") → path
local function resolve(name, root)
  if name == "" then
    return nil
  end
  local cands = {}
  if name:sub(1, 1) == "/" then
    cands = { name, name .. ".tex" }
  else
    local p = vim.fs.normalize(vim.fs.joinpath(root, name))
    cands = { p, p .. ".tex" }
  end
  for _, c in ipairs(cands) do
    if exists(c) and (c:match("%.tex$") or c:match("%.ltx$") or c:match("%.sty$") or c:match("%.cls$") or not c:match("%.%w+$")) then
      return c
    end
  end
  return nil
end

-- TeX wraps its log at 79 characters; put those lines back together.
local function unwrap(text)
  local out, cur = {}, nil
  for line in (text .. "\n"):gmatch("([^\n]*)\n") do
    line = line:gsub("\r$", "")
    if cur then
      cur = cur .. line
    else
      cur = line
    end
    if vim.fn.strchars(line) ~= 79 then
      out[#out + 1] = cur
      cur = nil
    end
  end
  if cur then
    out[#out + 1] = cur
  end
  return out
end

--- the buffer of `path` (exact name, unlike bufnr()'s pattern match), or -1
local function buf_of(path)
  return vim.fn.bufexists(path) == 1 and vim.fn.bufadd(path) or -1
end

local function first_line(path, lnum)
  local buf = buf_of(path)
  if buf > 0 and vim.api.nvim_buf_is_loaded(buf) then
    return vim.api.nvim_buf_get_lines(buf, lnum - 1, lnum, false)[1]
  end
  local ok, lines = pcall(vim.fn.readfile, path, "", lnum)
  return ok and lines[lnum] or nil
end

local function file_lines(path)
  local buf = buf_of(path)
  if buf > 0 and vim.api.nvim_buf_is_loaded(buf) then
    return vim.api.nvim_buf_get_lines(buf, 0, -1, false)
  end
  local ok, lines = pcall(vim.fn.readfile, path)
  return ok and lines or {}
end

--- where in source line `src` the TeX context `ctx` ("Let $x = \foo") ends;
--- returns the 0-based start / end columns of the last token TeX read
local function locate(src, ctx)
  if not src or not ctx then
    return nil
  end
  ctx = ctx:gsub("^%.%.%.", ""):gsub("%s+$", "")
  if ctx == "" then
    return nil
  end
  local s, e = src:find(ctx, 1, true)
  if not s then
    -- TeX shows at most ~half a line; try the tail of what it showed
    local tail = ctx:sub(-20)
    s, e = src:find(tail, 1, true)
    if not s then
      return nil
    end
  end
  local before = src:sub(1, e)
  local cs = before:find("\\%a+%*?$") or before:find("\\.$")
  local start = cs or e
  return start - 1, e
end

--- parse a tectonic / LaTeX log. Returns { {file, lnum, col, end_col, severity, message, code} }
function M.parse(text, main)
  local root = vim.fs.dirname(main)
  local lines = unwrap(text)
  local items = {}
  local stack = { main }
  local runaway
  local i = 1
  local n = #lines

  local function current()
    for k = #stack, 1, -1 do
      if stack[k] ~= false then
        return stack[k]
      end
    end
    return main
  end

  local function scan_parens(line)
    -- "(file" pushes, ")" pops; anything else in parentheses pushes a dummy
    local pos = 1
    while true do
      local a = line:find("[()]", pos)
      if not a then
        break
      end
      if line:sub(a, a) == "(" then
        local name = line:match("^%(([^%s()\"{}]+)", a)
        local path = name and resolve(name, root) or nil
        stack[#stack + 1] = path or false
        pos = a + 1 + (name and #name or 0)
      else
        if #stack > 1 then
          stack[#stack] = nil
        end
        pos = a + 1
      end
    end
  end

  local function add(it)
    it.file = it.file or current()
    items[#items + 1] = it
  end

  while i <= n do
    local line = lines[i]
    if line == "Runaway argument?" then
      runaway = lines[i + 1]
      i = i + 2
    elseif line:sub(1, 2) == "! " or line:match("^!%S") then
      local msg = line:gsub("^!%s*", "")
      -- find "l.<n> <context>" within the next lines
      local lnum, ctx
      local j = i + 1
      while j <= n and j <= i + 25 do
        local l = lines[j]
        local ln, rest = l:match("^l%.(%d+) ?(.*)$")
        if ln then
          lnum, ctx = tonumber(ln), rest
          break
        end
        if l:sub(1, 2) == "! " then
          break
        end
        j = j + 1
      end
      local file = current()
      local it = { severity = S.ERROR, message = msg:gsub("%.$", ""), code = "error" }
      if msg:match("^Missing %$ inserted") then
        it.message = "Missing $ inserted (maths outside $…$, or a $ not closed)"
      end
      local undef = msg:match("^Undefined control sequence")
      if undef and ctx then
        local cmd = ctx:match("(\\%a+%*?)%s*$") or ctx:match("(\\.)%s*$")
        if cmd then
          it.message = "Undefined control sequence " .. cmd
        end
      end
      local on_line = msg:match("on input line (%d+)")
      if on_line then
        -- "\begin{align} on input line 5 ended by \end{document}": the fix is at 5
        it.lnum = tonumber(on_line)
        local env = msg:match("\\begin{([^}]+)}")
        if env then
          it.message = ("\\begin{%s} is never ended (TeX reached %s%s)"):format(env,
            msg:match("ended by (\\end{[^}]+})") or "the end", lnum and (" on line " .. lnum) or "")
        end
      elseif lnum then
        it.lnum = lnum
        -- the context belongs to this file? (the paren stack can be fooled)
        local src = first_line(file, lnum)
        local c0, c1 = locate(src, ctx)
        if not c0 and file ~= main then
          local msrc = first_line(main, lnum)
          c0, c1 = locate(msrc, ctx)
          if c0 then
            file = main
          end
        end
        it.col, it.end_col = c0, c1
      elseif runaway then
        -- "File ended while scanning use of \textbf": find the runaway text
        local probe = runaway:gsub("\\par.*$", ""):gsub("^%s*{?", ""):sub(1, 30)
        probe = vim.trim(probe)
        local cmd = msg:match("use of (\\%a+)")
        for _, f in ipairs({ file, main }) do
          local fl = file_lines(f)
          for k, l in ipairs(fl) do
            if probe ~= "" and l:find(probe, 1, true) then
              it.lnum, file = k, f
              local s = cmd and l:find(cmd, 1, true)
              if s then
                it.col, it.end_col = s - 1, s - 1 + #cmd
              end
              break
            end
          end
          if it.lnum then
            break
          end
        end
        if cmd then
          it.message = msg:gsub("%s*%.?%s*$", "") .. ": a { is never closed"
        end
      end
      runaway = nil
      if it.lnum then
        it.file = file
        add(it)
      else
        -- no line at all: show it on line 1 so it is not lost
        it.file, it.lnum = main, 1
        it.unlocated = true
        add(it)
      end
      i = (j > i and j <= n) and j + 1 or i + 1
    else
      local kind, rest = line:match("^(LaTeX Warning): (.*)$")
      if not kind then
        kind, rest = line:match("^(Package %S+ Warning): (.*)$")
      end
      if not kind then
        kind, rest = line:match("^(Class %S+ Warning): (.*)$")
      end
      if not kind then
        kind, rest = line:match("^(LaTeX Font Warning): (.*)$")
      end
      if kind then
        -- continuation lines: "(hyperref)                 more text"
        local j = i + 1
        while j <= n and lines[j]:match("^%([%w@]+%)%s") do
          rest = rest .. " " .. vim.trim(lines[j]:gsub("^%([%w@]+%)%s+", ""))
          j = j + 1
        end
        local ln = rest:match("on input line (%d+)%.?%s*$") or rest:match("line (%d+)")
        if ln then
          local msg = rest:gsub("%s*on input line %d+%.?%s*$", ""):gsub("%.$", "")
          local font = kind == "LaTeX Font Warning"
          local it = {
            lnum = tonumber(ln),
            severity = font and S.HINT or S.WARN,
            message = msg,
            code = font and "box" or "warning",
          }
          -- "Reference `sec:x' on page 1 undefined": underline {sec:x}
          local key = msg:match("^Reference `([^']+)'") or msg:match("^Citation `([^']+)'")
          if key then
            local src = first_line(current(), it.lnum)
            local s = src and (src:find("{" .. key .. "}", 1, true) or src:find(key, 1, true))
            if s then
              local cs = src:sub(1, s):find("\\%a+%*?{?$")
              it.col = (cs or s) - 1
              it.end_col = src:find("}", s, true) or (s + #key)
            end
          end
          add(it)
        end
        i = j
      else
        local box, l1 = line:match("^(%a+full \\[hv]box) .- at lines? (%d+)")
        if not box then
          box, l1 = line:match("^(%a+full \\[hv]box) .- detected at line (%d+)")
        end
        if box then
          add({ lnum = tonumber(l1), severity = S.HINT, message = line:gsub("%s*in paragraph", ""), code = "box" })
          i = i + 1
        else
          scan_parens(line)
          i = i + 1
        end
      end
    end
  end
  return items
end

--- the diagnostics a log gives, grouped by buffer
function M.collect(log, main)
  local text = read(log)
  if not text then
    return nil
  end
  local items = M.parse(text, main)
  local by_buf = {}
  local seen = {}
  for _, it in ipairs(items) do
    if it.code ~= "box" or prefs.box_warnings then
      local key = it.file .. ":" .. it.lnum .. ":" .. it.message
      if not seen[key] then
        seen[key] = true
        local buf = buf_of(it.file)
        if buf < 0 then
          buf = vim.fn.bufadd(it.file)
          vim.bo[buf].buflisted = false
        end
        by_buf[buf] = by_buf[buf] or {}
        local d = {
          lnum = math.max(it.lnum - 1, 0),
          col = it.col or 0,
          end_col = it.end_col,
          severity = it.severity,
          message = it.message,
          source = "tectonic",
          code = it.code,
        }
        if not d.end_col then
          -- no column from TeX: underline the line's text, not its indent
          local src = first_line(it.file, it.lnum) or ""
          local s = src:find("%S") or 1
          d.col, d.end_col = s - 1, #src
        end
        table.insert(by_buf[buf], d)
      end
    end
  end
  return by_buf
end

local function in_insert()
  return vim.api.nvim_get_mode().mode:sub(1, 1) == "i"
end

--- set a buffer's diagnostics, holding back the cursor line while typing
function M.apply(buf)
  local all = M.last[buf]
  if not all or not vim.api.nvim_buf_is_valid(buf) then
    return
  end
  local shown = all
  if in_insert() and buf == vim.api.nvim_get_current_buf() then
    local row = vim.api.nvim_win_get_cursor(0)[1] - 1
    shown = vim.tbl_filter(function(d)
      return d.lnum ~= row
    end, all)
    M.held = #shown ~= #all and buf or nil
  end
  vim.diagnostic.set(M.ns, buf, shown)
end

--- after a compile of `main` (log at `log`): replace our diagnostics
function M.update(main, log)
  local t0 = vim.uv.hrtime()
  local by_buf = M.collect(log, main)
  if not by_buf then
    return
  end
  local owned = M.owned[main] or {}
  for buf in pairs(owned) do
    if not by_buf[buf] and vim.api.nvim_buf_is_valid(buf) then
      M.last[buf] = nil
      vim.diagnostic.reset(M.ns, buf)
    end
  end
  owned = {}
  for buf, list in pairs(by_buf) do
    owned[buf] = true
    M.last[buf] = list
    M.apply(buf)
  end
  M.owned[main] = owned
  M.stats.runs = M.stats.runs + 1
  M.stats.last_ms = (vim.uv.hrtime() - t0) / 1e6
end

--- the main file and log of the current buffer's VimTeX project
local function project()
  local ok, info = pcall(vim.fn.eval, "[b:vimtex.tex, b:vimtex.compiler.get_file('log')]")
  if ok and type(info) == "table" and info[1] ~= "" then
    return vim.fs.normalize(info[1]), info[2]
  end
end

function M.setup()
  if prefs.compile_errors == false then
    return
  end
  -- show at once, also in insert mode (M.apply holds back the line you type in)
  vim.diagnostic.config({ update_in_insert = true }, M.ns)
  local grp = vim.api.nvim_create_augroup("texnvim_compile_errors", { clear = true })
  vim.api.nvim_create_autocmd("User", {
    group = grp,
    pattern = { "VimtexEventCompileSuccess", "VimtexEventCompileFailed" },
    callback = function()
      local main, log = project()
      if main and log and log ~= "" then
        M.update(main, log)
      end
    end,
  })
  vim.api.nvim_create_autocmd("InsertLeave", {
    group = grp,
    callback = function(ev)
      if M.held == ev.buf then
        M.held = nil
        M.apply(ev.buf)
      end
    end,
  })
end

return M
