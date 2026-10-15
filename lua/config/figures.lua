-- Ntex Figures in ntex (studio/figures/FIGURES.md, "In ntex"): diagrams as
-- small text files, figures/NAME.fig next to the .tex, built by `ntex-fig`
-- into figures/NAME.pdf, which the .tex includes.
--
--   Space g   (.tex / .md) on a line with \includegraphics{…figures/NAME…}
--             (NAME may be sub/deep: figures/sub/deep.fig): open that
--             figure's window. Anywhere else: ask for a name
--             (default fig-N, the next free number), create figures/NAME.fig
--             (`ntex-fig new`, or M.TEMPLATE without ntex-fig) and put this
--             on its own lines below the cursor (or in place of a blank line):
--               \begin{center}
--                 \includegraphics{figures/NAME}
--               \end{center}
--   Space g   (.fig) open this figure's window
--   :Figure [name]   the same as Space g; with a name, that figure
--
-- .fig files get filetype `ntexfig` (syntax/ntexfig.vim, ftplugin/ntexfig.lua).
-- Saving one runs `ntex-fig build` in the background, and `ntex-fig check`'s
-- messages become diagnostics on their lines (namespace "ntexfig").
--
-- The live PDF follows: a figure PDF changing (the window, the CLI, a .fig
-- save) recompiles every open .tex that includes it, through the same
-- compile-on-save path the live PDF uses (lua/plugins/core.lua). One
-- debounced fs_event watch per figures/ folder, started when a .tex that
-- mentions figures/ is read or saved.
--
-- The window is Ntex Studio's (`ntex-studio --figure PATH`), detached, like
-- Space n. TEXNVIM_NO_VIEWER=1 (the tests) never launches it; the would-be
-- command lines go to M.launched. TEXNVIM_NO_COMPILE=1 records recompiles in
-- M.recompiles without compiling.
local M = {}

local studio_dir = vim.fs.joinpath(vim.fn.stdpath("config"), "studio")
M.studio_binary = vim.fs.joinpath(studio_dir, "target", "release", "ntex-studio")
M.fig_release = vim.fs.joinpath(studio_dir, "target", "release", "ntex-fig")
M.ns = vim.api.nvim_create_namespace("ntexfig")
M.DEBOUNCE_MS = 250
-- ntex-fig's notes ("`A` is new here, so it became a node") come on almost
-- every line; only errors and warnings become diagnostics unless this is true
M.show_notes = false

M.launched = {} -- argv of every window launch (and would-be launch), newest last; tests read it
M.recompiles = {} -- { tex = path, pdf = path } per recompile asked for, newest last; tests read it
M.recompile_hook = nil -- tests may set function(buf, pdf); replaces the compile
M.stats = { checks = 0, builds = 0 }
M.running = { checks = 0, builds = 0 } -- ntex-fig processes not finished yet
M.watches = {} -- figures dir -> { handle, bufs = { [buf] = true }, timers = { [file] = timer }, mtimes = {} }

local function say(msg, level)
  vim.schedule(function()
    vim.notify(msg, level or vim.log.levels.INFO)
  end)
end

local function exists(p)
  return vim.uv.fs_stat(p) ~= nil
end

-- ---------------------------------------------------------------------------
-- what a new .fig starts with when ntex-fig isn't installed (otherwise
-- `ntex-fig new` writes the same, studio/figures/template.fig). NAME is the
-- figure's name; the cheat sheet is studio/figures/reference.md (⌘/ in the
-- window).

M.TEMPLATE = [[
% Ntex Figures — this file draws figures/NAME.pdf. Save to rebuild; Space g opens the window.
% ⌘/ in the window shows the reference (or ~/.config/texnvim/studio/figures/FIGURES.md).
]]

--- M.TEMPLATE for the figure at `fig`: NAME is the part of the path after
--- the last `figures/` folder, without `.fig` (as `ntex-fig new` writes it).
function M.template_for(fig)
  local rest = fig:match(".*/figures/(.+)%.fig$") or vim.fn.fnamemodify(fig, ":t:r")
  return (M.TEMPLATE:gsub("NAME", function() return rest end))
end

-- ---------------------------------------------------------------------------
-- finding the figure on a line

--- every \includegraphics[opts]{arg} on a line: { {s = , e = , arg = } } (1-based)
function M.graphics_on_line(line)
  local out = {}
  local init = 1
  while true do
    local s, e = line:find("\\includegraphics", init, true)
    if not s then
      break
    end
    local p = e + 1
    if line:sub(p, p) == "*" then
      p = p + 1
    end
    p = line:find("%S", p) or (#line + 1)
    if line:sub(p, p) == "[" then
      local a, b = line:find("%b[]", p)
      if a == p then
        p = line:find("%S", b + 1) or (#line + 1)
      end
    end
    if line:sub(p, p) == "{" then
      local a, b = line:find("%b{}", p)
      if a == p then
        out[#out + 1] = { s = s, e = b, arg = line:sub(a + 1, b - 1) }
        init = b + 1
      else
        init = p + 1
      end
    else
      init = e + 1
    end
  end
  -- Markdown: ![alt](figures/x.pdf)
  for s, arg, e in line:gmatch("()!%b[]%(([^)%s]+)%)()") do
    out[#out + 1] = { s = s, e = e - 1, arg = arg }
  end
  table.sort(out, function(a, b)
    return a.s < b.s
  end)
  return out
end

--- "…figures/NAME[.pdf]" → the .fig path, against `dir`; nil if not a figure.
--- NAME may sit in subfolders of figures/ ("figures/sub/deep" → figures/sub/deep.fig).
function M.fig_from_arg(arg, dir)
  arg = vim.trim(arg)
  local pre, name = arg:match("^(.-figures)/(.+)$")
  if not pre then
    return nil
  end
  local parts = vim.split(name, "/", { plain = true })
  local last = parts[#parts]
  local base, ext = last:match("^(.+)%.(%w+)$")
  if base then
    if ext ~= "pdf" and ext ~= "fig" then
      return nil -- figures/photo.png is not ours
    end
    parts[#parts] = base
  end
  for _, p in ipairs(parts) do
    -- figures//x, figures/sub/, figures/../x: not a figure under figures/
    if p == "" or p == "." or p == ".." then
      return nil
    end
  end
  name = table.concat(parts, "/")
  local folder
  if pre:sub(1, 1) == "/" or pre:sub(1, 1) == "~" then
    folder = vim.fs.normalize(pre)
  else
    folder = vim.fs.normalize(vim.fs.joinpath(dir, pre))
  end
  -- "./figures" → "dir/./figures": normalize leaves the "." in some versions
  folder = folder:gsub("/%./", "/"):gsub("/%.$", "")
  return vim.fs.joinpath(folder, name .. ".fig")
end

--- the figure the cursor's line includes (the one under the cursor if several)
function M.figure_at_cursor(buf, row, col, dir)
  local line = vim.api.nvim_buf_get_lines(buf, row - 1, row, false)[1] or ""
  local found
  for _, g in ipairs(M.graphics_on_line(line)) do
    local fig = M.fig_from_arg(g.arg, dir)
    if fig then
      if col + 1 >= g.s and col + 1 <= g.e then
        return fig
      end
      found = found or fig
    end
  end
  return found
end

-- ---------------------------------------------------------------------------
-- names

--- "My Figure.fig" → "my-figure"; only a-z 0-9 - _
function M.sanitize(name)
  name = vim.trim(name or ""):gsub("%.fig$", ""):gsub("%.pdf$", ""):lower()
  name = name:gsub("%s+", "-"):gsub("[^a-z0-9%-_]", "")
  return name
end

--- fig-N, N one more than the highest fig-N in `folder` (.fig or .pdf)
function M.next_name(folder)
  local top = 0
  local fs = vim.uv.fs_scandir(folder)
  if fs then
    while true do
      local name = vim.uv.fs_scandir_next(fs)
      if not name then
        break
      end
      local n = name:match("^fig%-(%d+)%.fig$") or name:match("^fig%-(%d+)%.pdf$")
      if n then
        top = math.max(top, tonumber(n))
      end
    end
  end
  return "fig-" .. (top + 1)
end

M.NEW_TIMEOUT_MS = 5000

--- create figures/NAME.fig, unless it is there: `ntex-fig new FIG` (the
--- starting file the CLI ships) when ntex-fig is installed, else M.template_for(fig).
--- Synchronous: the file exists when this returns true.
function M.ensure_file(fig)
  if exists(fig) then
    return true
  end
  vim.fn.mkdir(vim.fs.dirname(fig), "p")
  local bin = M.fig_bin()
  if bin then
    -- (wait's timeout kills it: no stray process)
    local ok, r = pcall(function()
      return vim.system({ bin, "new", fig }, { text = true, stdin = false, cwd = vim.fs.dirname(fig) }):wait(M.NEW_TIMEOUT_MS)
    end)
    M.last_new = { fig = fig, code = ok and r and r.code or nil }
    if exists(fig) then
      return true
    end
    -- didn't start, failed, or a stub without `new`: the built-in template
  end
  local fd = io.open(fig, "w")
  if not fd then
    say("Ntex Figures: can't write " .. fig, vim.log.levels.ERROR)
    return false
  end
  fd:write(M.template_for(fig))
  fd:close()
  return true
end

-- ---------------------------------------------------------------------------
-- ntex-fig (the CLI): check, build

--- the ntex-fig to run: on PATH (~/.local/bin/ntex-fig), else the release build
function M.fig_bin()
  local p = vim.fn.exepath("ntex-fig")
  if p ~= "" then
    return p
  end
  if vim.fn.executable(M.fig_release) == 1 then
    return M.fig_release
  end
  return nil
end

local warned_unbuilt = false
local function unbuilt(text)
  if text and not text:find("isn't built", 1, true) then
    return false
  end
  if not warned_unbuilt then
    warned_unbuilt = true
    say("Ntex Figures isn't built: ~/.config/texnvim/studio/build.sh", vim.log.levels.WARN)
  end
  return true
end

local SEV = vim.diagnostic.severity

local function severity_of(text)
  local l = text:lower()
  if l:find("^%s*error") or l:find("^%s*!") then
    return SEV.ERROR
  elseif l:find("^%s*warn") then
    return SEV.WARN
  elseif l:find("^%s*note") or l:find("^%s*info") then
    return SEV.HINT
  elseif l:find("^%s*hint") then
    return SEV.HINT
  end
  return nil
end

--- `ntex-fig check` output → diagnostics (lnum 0-based). Takes text lines
--- ("kernel.fig:4:7: warning: line 4: …", "error: line 4: …", "line 4: …")
--- or JSON (a list of {severity, span = {line, col}, message}, or one per line).
function M.parse_check(text, nlines)
  local out = {}
  nlines = math.max(nlines or 1, 1)
  local function add(lnum, col, sev, msg)
    if (sev == SEV.INFO or sev == SEV.HINT) and not M.show_notes then
      return
    end
    lnum = math.min(math.max(lnum, 1), nlines)
    out[#out + 1] = {
      lnum = lnum - 1,
      col = math.max((col or 1) - 1, 0),
      severity = sev or SEV.ERROR,
      message = vim.trim(msg),
      source = "ntex-fig",
    }
  end
  local function from_json(d)
    if type(d) ~= "table" or type(d.message) ~= "string" then
      return
    end
    local ln = tonumber(d.message:match("^[Ll]ine (%d+)")) or (type(d.span) == "table" and tonumber(d.span.line)) or tonumber(d.line) or 1
    local col = type(d.span) == "table" and tonumber(d.span.col) or tonumber(d.col)
    local sev = ({ Error = SEV.ERROR, Warning = SEV.WARN, Note = SEV.HINT, error = SEV.ERROR, warning = SEV.WARN, note = SEV.HINT })[d.severity]
    add(ln, col and col + 1 or 1, sev or SEV.ERROR, (d.message:gsub("^[Ll]ine %d+:?%s*", "")))
  end
  local trimmed = vim.trim(text or "")
  if trimmed:sub(1, 1) == "[" or trimmed:sub(1, 1) == "{" then
    local ok, data = pcall(vim.json.decode, trimmed)
    if ok and type(data) == "table" then
      if data.message then
        from_json(data)
      else
        for _, d in ipairs(data.diagnostics or data) do
          from_json(d)
        end
      end
      return out
    end
    local any = false
    for line in trimmed:gmatch("[^\n]+") do
      local ok2, d = pcall(vim.json.decode, line)
      if ok2 and type(d) == "table" then
        any = true
        from_json(d)
      end
    end
    if any then
      return out
    end
  end
  for raw in (text or ""):gmatch("[^\n]+") do
    local line = raw:gsub("\r$", "")
    local lnum, col, rest
    -- file:line:col: … / file:line: …   (file has no spaces)
    local f, l, c, r = line:match("^(.-%.fig):(%d+):(%d+):%s*(.*)$")
    if not f then
      f, l, c, r = line:match("^(%S-):(%d+):(%d+):%s*(.*)$")
    end
    if f then
      lnum, col, rest = tonumber(l), tonumber(c), r
    else
      f, l, r = line:match("^(%S-):(%d+):%s*(.*)$")
      if f then
        lnum, rest = tonumber(l), r
      end
    end
    local sev
    if lnum then
      sev = severity_of(rest)
      rest = rest:gsub("^%s*%a+:%s*", function(w)
        return severity_of(w) and "" or w
      end)
      rest = rest:gsub("^[Ll]ine %d+[:,]?%s*", "")
    else
      sev = severity_of(line)
      local body = line
      if sev then
        body = line:gsub("^%s*!?%s*%a*:?%s*", "", 1)
      end
      local l2, c2, r2 = body:match("^[Ll]ine (%d+):(%d+):?%s*(.*)$")
      if not l2 then
        l2, r2 = body:match("^[Ll]ine (%d+)[:,]?%s*(.*)$")
      end
      if not l2 then
        -- "… at line 4", "(line 4)"
        l2, r2 = line:match("[Ll]ine (%d+)[:,]?%s*(.*)$")
      end
      if l2 then
        lnum, col, rest = tonumber(l2), tonumber(c2), r2
      end
    end
    if lnum and rest and rest ~= "" then
      add(lnum, col, sev, rest)
    end
  end
  return out
end

local check_seq = {} -- buf -> the newest check's number

--- run `ntex-fig check` on the buffer's file; set its diagnostics (async)
function M.check(buf, done, quiet)
  local path = vim.api.nvim_buf_get_name(buf)
  local bin = M.fig_bin()
  if not bin then
    if not quiet then
      unbuilt()
    end
    return false
  end
  M.stats.checks = M.stats.checks + 1
  local seq = M.stats.checks
  check_seq[buf] = seq
  M.running.checks = M.running.checks + 1
  local ok = pcall(vim.system, { bin, "check", path }, { text = true, stdin = false }, function(r)
    vim.schedule(function()
      M.running.checks = M.running.checks - 1
      -- only the newest check of a buffer counts (an older one may finish later)
      if not vim.api.nvim_buf_is_valid(buf) or check_seq[buf] ~= seq then
        return
      end
      local text = (r.stdout or "") .. "\n" .. (r.stderr or "")
      if r.code ~= 0 and (r.stderr or ""):find("isn't built", 1, true) then
        if not quiet then
          unbuilt(r.stderr)
        end
        return
      end
      local list = M.parse_check(text, vim.api.nvim_buf_line_count(buf))
      vim.diagnostic.set(M.ns, buf, list)
      M.last_check = { buf = buf, code = r.code, n = #list }
      if done then
        done(list)
      end
    end)
  end)
  if not ok then
    M.running.checks = M.running.checks - 1
  end
  return ok
end

M.building = {} -- fig -> true while a build runs; "again" if a save came meanwhile

--- `ntex-fig build FIG` in the background; one at a time per file
function M.build(fig, opts)
  opts = opts or {}
  if M.building[fig] then
    M.building[fig] = "again"
    return true
  end
  local bin = M.fig_bin()
  if not bin then
    if not opts.quiet then
      unbuilt()
    end
    return false
  end
  M.building[fig] = true
  M.stats.builds = M.stats.builds + 1
  M.running.builds = M.running.builds + 1
  local ok, err = pcall(vim.system, { bin, "build", fig }, { text = true, stdin = false, cwd = vim.fs.dirname(fig) }, function(r)
    vim.schedule(function()
      M.running.builds = M.running.builds - 1
      local again = M.building[fig] == "again"
      M.building[fig] = nil
      if r.code ~= 0 and not unbuilt(r.stderr) and not opts.quiet then
        -- the reasons are diagnostics already (check); this is the one-line summary
        -- the last line says why (the diagnostics come before it)
        local first = ""
        for l in (((r.stderr or "") ~= "" and r.stderr) or r.stdout or ""):gmatch("[^\n]+") do
          first = vim.trim(l)
        end
        first = first:gsub("^.-%.fig:%s*", "")
        say(("Ntex Figures: %s didn't build%s"):format(vim.fs.basename(fig), first ~= "" and (": " .. first) or ""), vim.log.levels.WARN)
      end
      M.last_build = { fig = fig, code = r.code }
      if again then
        M.build(fig, opts)
      end
    end)
  end)
  if not ok then
    M.building[fig] = nil
    M.running.builds = M.running.builds - 1
    say("Ntex Figures: ntex-fig didn't start: " .. tostring(err), vim.log.levels.ERROR)
  end
  return ok
end

-- ---------------------------------------------------------------------------
-- the window (Ntex Studio --figure)

function M.command(fig)
  return { M.studio_binary, "--figure", fig }
end

--- launch (or bring forward) the figure's window. Returns true if launched.
function M.launch(fig)
  local argv = M.command(fig)
  M.launched[#M.launched + 1] = argv
  if vim.env.TEXNVIM_NO_VIEWER == "1" then
    say("Ntex Figures: not opening " .. vim.fs.basename(fig) .. " (TEXNVIM_NO_VIEWER=1)")
    return false
  end
  if vim.fn.executable(M.studio_binary) ~= 1 then
    say("Ntex Studio isn't built: ~/.config/texnvim/studio/build.sh", vim.log.levels.WARN)
    return false
  end
  -- detached, no pipes: it outlives this ntex, and never writes into a closed one
  local ok, err = pcall(vim.system, argv, { detach = true, stdin = false, stdout = false, stderr = false })
  if not ok then
    say("Ntex Studio didn't start: " .. tostring(err), vim.log.levels.ERROR)
    return false
  end
  return true
end

--- make sure the .fig exists, build its PDF if missing, open its window
function M.open_figure(fig)
  if not M.ensure_file(fig) then
    return false
  end
  local pdf = fig:gsub("%.fig$", ".pdf")
  if not exists(pdf) then
    M.build(fig, { quiet = true })
  end
  return M.launch(fig)
end

-- ---------------------------------------------------------------------------
-- Space g

local DOC_FT = { tex = true, plaintex = true, markdown = true }

--- put the center/includegraphics block on its own lines at the cursor
function M.insert_block(buf, row, name)
  local line = vim.api.nvim_buf_get_lines(buf, row - 1, row, false)[1] or ""
  local indent = line:match("^%s*")
  local block = {
    indent .. "\\begin{center}",
    indent .. "  \\includegraphics{figures/" .. name .. "}",
    indent .. "\\end{center}",
  }
  local at
  if vim.trim(line) == "" then
    vim.api.nvim_buf_set_lines(buf, row - 1, row, false, block)
    at = row
  else
    vim.api.nvim_buf_set_lines(buf, row, row, false, block)
    at = row + 1
  end
  for _, win in ipairs(vim.fn.win_findbuf(buf)) do
    vim.api.nvim_win_set_cursor(win, { at + 1, #indent + 2 })
  end
  return at
end

--- create figures/NAME.fig next to the buffer, insert the block, open it
function M.create(buf, name, row)
  local dir = vim.fs.dirname(vim.api.nvim_buf_get_name(buf))
  local clean = M.sanitize(name)
  if clean == "" then
    say("Ntex Figures: a figure name needs letters or digits (a-z 0-9 - _)", vim.log.levels.WARN)
    return nil
  end
  local fig = vim.fs.joinpath(dir, "figures", clean .. ".fig")
  if not M.ensure_file(fig) then
    return nil
  end
  M.insert_block(buf, row, clean)
  M.watch_buffer(buf)
  M.open_figure(fig)
  return fig
end

--- Space g: open the figure on this line, or make a new one (name given or asked)
function M.space_g(name)
  local buf = vim.api.nvim_get_current_buf()
  local ft = vim.bo[buf].filetype
  local path = vim.api.nvim_buf_get_name(buf)
  if ft == "ntexfig" then
    if path == "" then
      say("Ntex Figures: save this figure first (:w figures/NAME.fig)", vim.log.levels.WARN)
      return
    end
    if vim.bo[buf].modified then
      vim.cmd("silent update")
    end
    return M.open_figure(path)
  end
  if not DOC_FT[ft] then
    say("Ntex Figures: Space g works in .tex, .md and .fig files", vim.log.levels.WARN)
    return
  end
  if path == "" then
    say("Ntex Figures: save this file first; its figures go in a figures/ folder next to it", vim.log.levels.WARN)
    return
  end
  local dir = vim.fs.dirname(path)
  local cur = vim.api.nvim_win_get_cursor(0)
  if name and name ~= "" then
    return M.create(buf, name, cur[1])
  end
  local fig = M.figure_at_cursor(buf, cur[1], cur[2], dir)
  if fig then
    M.watch_buffer(buf)
    return M.open_figure(fig)
  end
  local default = M.next_name(vim.fs.joinpath(dir, "figures"))
  vim.ui.input({ prompt = "New figure (figures/NAME.fig): ", default = default }, function(input)
    if input == nil then
      return
    end
    vim.schedule(function()
      if vim.api.nvim_buf_is_valid(buf) then
        vim.api.nvim_buf_call(buf, function()
          M.create(buf, input, math.min(cur[1], vim.api.nvim_buf_line_count(buf)))
        end)
      end
    end)
  end)
end

-- ---------------------------------------------------------------------------
-- the live PDF follows figures: watch figures/ next to open .tex files

--- the figures folders a buffer refers to (its own figures/ when it mentions one)
function M.figure_dirs(buf)
  local path = vim.api.nvim_buf_get_name(buf)
  if path == "" then
    return {}
  end
  local dir = vim.fs.dirname(path)
  local dirs, seen = {}, {}
  for _, line in ipairs(vim.api.nvim_buf_get_lines(buf, 0, -1, false)) do
    if line:find("figures/", 1, true) then
      for _, g in ipairs(M.graphics_on_line(line)) do
        local fig = M.fig_from_arg(g.arg, dir)
        if fig then
          local d = vim.fs.dirname(fig)
          if not seen[d] then
            seen[d] = true
            dirs[#dirs + 1] = d
          end
        end
      end
      local own = vim.fs.joinpath(dir, "figures")
      if not seen[own] then
        seen[own] = true
        dirs[#dirs + 1] = own
      end
    end
  end
  return dirs
end

--- does the buffer include figures/NAME (from `folder`)?
local function mentions(buf, folder, name)
  local dir = vim.fs.dirname(vim.api.nvim_buf_get_name(buf))
  for _, line in ipairs(vim.api.nvim_buf_get_lines(buf, 0, -1, false)) do
    if line:find(name, 1, true) then
      for _, g in ipairs(M.graphics_on_line(line)) do
        local fig = M.fig_from_arg(g.arg, dir)
        if fig and fig == vim.fs.joinpath(folder, name .. ".fig") then
          return true
        end
      end
    end
  end
  return false
end

--- recompile a .tex because a figure changed: the compile-on-save path
function M.recompile(buf, pdf)
  M.recompiles[#M.recompiles + 1] = { tex = vim.api.nvim_buf_get_name(buf), pdf = pdf }
  if M.recompile_hook then
    return M.recompile_hook(buf, pdf)
  end
  if vim.env.TEXNVIM_NO_COMPILE == "1" then
    return
  end
  -- lua/plugins/core.lua's BufWritePost handler: compiles with VimTeX, or
  -- queues one if a compile is running
  pcall(vim.api.nvim_buf_call, buf, function()
    vim.api.nvim_exec_autocmds("BufWritePost", { group = "texnvim_compile_on_save", buffer = buf, modeline = false })
  end)
end

local function changed(w, folder, file)
  if not file then
    -- some events come without a name: every included figure here
    for buf in pairs(w.bufs) do
      if vim.api.nvim_buf_is_valid(buf) then
        M.recompile(buf, folder)
      end
    end
    return
  end
  local name = file:match("^(.+)%.pdf$")
  if not name then
    return
  end
  local pdf = vim.fs.joinpath(folder, file)
  local st = vim.uv.fs_stat(pdf)
  if not st then
    return -- deleted: nothing new to show
  end
  local stamp = st.mtime.sec * 1e9 + st.mtime.nsec + st.size
  if w.mtimes[file] == stamp then
    return
  end
  w.mtimes[file] = stamp
  for buf in pairs(w.bufs) do
    if vim.api.nvim_buf_is_valid(buf) and vim.api.nvim_buf_is_loaded(buf) and mentions(buf, folder, name) then
      M.recompile(buf, pdf)
    end
  end
end

local function start_watch(folder)
  local w = M.watches[folder]
  if w then
    return w
  end
  if vim.fn.isdirectory(folder) ~= 1 then
    return nil
  end
  local h = vim.uv.new_fs_event()
  if not h then
    return nil
  end
  w = { handle = h, bufs = {}, timers = {}, mtimes = {} }
  -- PDFs already there are not news
  local fs = vim.uv.fs_scandir(folder)
  while fs do
    local n = vim.uv.fs_scandir_next(fs)
    if not n then
      break
    end
    if n:match("%.pdf$") then
      local st = vim.uv.fs_stat(vim.fs.joinpath(folder, n))
      if st then
        w.mtimes[n] = st.mtime.sec * 1e9 + st.mtime.nsec + st.size
      end
    end
  end
  local ok = pcall(h.start, h, folder, {}, function(err, file)
    if err then
      return
    end
    if file and not file:match("%.pdf$") then
      return
    end
    local key = file or ""
    local t = w.timers[key]
    if not t then
      t = vim.uv.new_timer()
      w.timers[key] = t
    end
    t:stop()
    t:start(M.DEBOUNCE_MS, 0, vim.schedule_wrap(function()
      if M.watches[folder] == w then
        changed(w, folder, file and vim.fs.basename(file) or nil)
      end
    end))
  end)
  if not ok then
    h:close()
    return nil
  end
  M.watches[folder] = w
  return w
end

local function stop_watch(folder)
  local w = M.watches[folder]
  if not w then
    return
  end
  M.watches[folder] = nil
  for _, t in pairs(w.timers) do
    t:stop()
    t:close()
  end
  pcall(function()
    w.handle:stop()
    w.handle:close()
  end)
end

--- start (lazily) the watches a .tex buffer needs; forget the ones it doesn't
function M.watch_buffer(buf)
  if not vim.api.nvim_buf_is_valid(buf) then
    return
  end
  local name = vim.api.nvim_buf_get_name(buf)
  if not name:match("%.tex$") then
    return
  end
  local want = {}
  for _, d in ipairs(M.figure_dirs(buf)) do
    want[d] = true
    local w = start_watch(d)
    if w then
      w.bufs[buf] = true
    end
  end
  for folder, w in pairs(M.watches) do
    if w.bufs[buf] and not want[folder] then
      w.bufs[buf] = nil
      if next(w.bufs) == nil then
        stop_watch(folder)
      end
    end
  end
end

function M.unwatch_buffer(buf)
  for folder, w in pairs(M.watches) do
    if w.bufs[buf] then
      w.bufs[buf] = nil
      if next(w.bufs) == nil then
        stop_watch(folder)
      end
    end
  end
end

-- ---------------------------------------------------------------------------

function M.setup()
  vim.filetype.add({ extension = { fig = "ntexfig" } })

  local grp = vim.api.nvim_create_augroup("texnvim_figures", { clear = true })
  -- .fig: check on open; check + build on save
  vim.api.nvim_create_autocmd("BufReadPost", {
    group = grp,
    pattern = "*.fig",
    callback = function(ev)
      M.check(ev.buf, nil, true)
    end,
  })
  vim.api.nvim_create_autocmd("BufWritePost", {
    group = grp,
    pattern = "*.fig",
    callback = function(ev)
      if vim.fn.filereadable(ev.match) == 0 then
        return
      end
      M.check(ev.buf)
      M.build(vim.api.nvim_buf_get_name(ev.buf))
    end,
  })
  -- .tex: watch the figures/ folders it includes from
  vim.api.nvim_create_autocmd({ "BufReadPost", "BufWritePost" }, {
    group = grp,
    pattern = "*.tex",
    callback = function(ev)
      M.watch_buffer(ev.buf)
    end,
  })
  vim.api.nvim_create_autocmd({ "BufWipeout", "BufDelete" }, {
    group = grp,
    pattern = "*.tex",
    callback = function(ev)
      M.unwatch_buffer(ev.buf)
    end,
  })
  vim.api.nvim_create_autocmd("VimLeavePre", {
    group = grp,
    callback = function()
      for folder in pairs(M.watches) do
        stop_watch(folder)
      end
    end,
  })

  vim.api.nvim_create_user_command("Figure", function(o)
    M.space_g(o.args ~= "" and o.args or nil)
  end, {
    nargs = "?",
    desc = "Ntex Figures: open the figure on this line, or make figures/NAME.fig and include it",
  })
  vim.keymap.set("n", "<leader>g", function()
    M.space_g()
  end, { desc = "Ntex Figures: open the figure on this line, or make a new one" })
end

return M
