-- Headless verifier: types examples into a real ntex and records what came
-- out. Run by reference/build.mjs (and tools/migration/decisions.mjs):
--
--   nvim --clean --headless -l verify.lua <jobs.json> <out.json>
--
-- Same technique as tests/driver.lua: this *controller* Neovim starts the
-- real config in a child (`nvim --embed --headless`) and types into it one
-- key at a time over RPC, so every example is what the config really does.
-- No windows open, nothing compiles (TEXNVIM_NO_COMPILE=1), Skim/osascript
-- are never touched. A fresh child is started every 150 examples (VimTeX's
-- syntax breaks after ~220 .tex buffers in one session).
--
-- $TEXNVIM_APPNAME: which config (default texnvim). $TEXNVIM_SNIPPETS and
-- $LATEXSUITE_DIR are passed through.
--
-- jobs.json: { examples = [ { id, before = "text with ‸", keys = "x2<Tab>",
--                             ft = "tex"|"md", select = <n chars from ‸> } ],
--              dump = true|false }
-- out.json:  { examples = { [id] = { lines, row, col, mode, sel, marks } },
--              dump = { keymaps, commands, prefs, snippets, errors, options, config } }

io.stdout:setvbuf("no")
local JOBS, OUT = arg[1], arg[2]
local jobs = vim.json.decode(table.concat(vim.fn.readfile(JOBS), "\n"))

local child_env = {
  NVIM_APPNAME = vim.env.TEXNVIM_APPNAME or "texnvim",
  TEXNVIM_SNIPPETS = vim.env.TEXNVIM_SNIPPETS,
  LATEXSUITE_DIR = vim.env.LATEXSUITE_DIR,
  TEXNVIM_NO_COMPILE = "1",
  TEXNVIM_IMAGES = "0",
  TEXNVIM_NO_REFERENCE = "1", -- never start a reference rebuild from inside the build
  TEXNVIM_NO_VIEWER = "1", -- never a viewer (Skim, cmux pane, browser)
  HOME = vim.env.HOME,
  PATH = vim.env.PATH,
  TERM = "xterm-256color",
}

local chan, child_pid
local deadline = nil
local watchdog = vim.uv.new_timer()
watchdog:start(1000, 1000, function()
  if deadline and vim.uv.now() > deadline and child_pid then
    io.stderr:write("verify.lua: child nvim stopped answering, killing it\n")
    vim.uv.kill(child_pid, "sigkill")
    deadline = nil
  end
end)

local function start()
  if chan then
    pcall(vim.rpcrequest, chan, "nvim_command", "qa!")
    vim.fn.jobstop(chan)
  end
  chan = vim.fn.jobstart({ "nvim", "--embed", "--headless", "-n", "-i", "NONE" }, { rpc = true, env = child_env, clear_env = true })
  assert(chan > 0, "could not start child nvim")
  child_pid = vim.fn.jobpid(chan)
end

local function rpc(method, ...)
  deadline = vim.uv.now() + 20000
  local res = vim.rpcrequest(chan, method, ...)
  deadline = nil
  return res
end
local function lua(code, ...)
  return rpc("nvim_exec_lua", code, { ... })
end
local function sync()
  rpc("nvim_eval", "1")
  rpc("nvim_eval", "1")
end

local CURSOR = "‸"

local function tokens(keys)
  local out, i = {}, 1
  while i <= #keys do
    local special = keys:match("^<[%w%-]+>", i)
    if special then
      out[#out + 1] = special
      i = i + #special
    else
      local ch = keys:match("^[%z\1-\127\194-\244][\128-\191]*", i)
      if ch == "<" then
        out[#out + 1] = "<lt>"
        i = i + 1
      else
        out[#out + 1] = ch
        i = i + #ch
      end
    end
  end
  return out
end

local function split_marker(text)
  local lines = vim.split(text, "\n", { plain = true })
  for r, l in ipairs(lines) do
    local c = l:find(CURSOR, 1, true)
    if c then
      lines[r] = l:sub(1, c - 1) .. l:sub(c + #CURSOR)
      return lines, r, c - 1
    end
  end
  return lines, #lines, #lines[#lines]
end

local tmpdir = vim.fn.tempname()
vim.fn.mkdir(tmpdir, "p")
local file_n = 0

local function open(text, ft)
  file_n = file_n + 1
  local path = ("%s/ex%04d.%s"):format(tmpdir, file_n, ft or "tex")
  local lines, r, c = split_marker(text)
  rpc("nvim_input", "<Esc>")
  sync()
  lua([[
    local path, lines, r, c = ...
    pcall(function() require("luasnip").unlink_current() end)
    vim.cmd("silent! %bwipeout!")
    vim.cmd.edit(path)
    local buf = vim.api.nvim_get_current_buf()
    vim.b[buf].texnvim_template_done = true
    vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
    vim.api.nvim_win_set_cursor(0, { r, c })
  ]], path, lines, r, c)
  sync()
  return lines, r, c
end

-- state after typing, including every LuaSnip tabstop of the active snippet
local STATE = [[
  local ls = require("luasnip")
  local types = require("luasnip.util.types")
  local buf = vim.api.nvim_get_current_buf()
  local cur = vim.api.nvim_win_get_cursor(0)
  local mode = vim.fn.mode()
  local sel
  if mode == "s" or mode == "v" then
    sel = table.concat(vim.fn.getregion(vim.fn.getpos("v"), vim.fn.getpos("."), { type = "v" }), "\n")
  end
  local marks = {}
  local node = ls.session.current_nodes[buf]
  if node then
    local root = node.parent and node.parent.snippet or node
    for _ = 1, 20 do
      local p = root.parent and root.parent.snippet
      if not p or p == root then break end
      root = p
    end
    local function walk(n, depth)
      if depth > 8 then return end
      local t = n.type
      local ok, from, to = pcall(function() return n:get_buf_position() end)
      if ok and from then
        if t == types.insertNode and depth > 1 and n.pos and n.pos > 0 then
          marks[#marks + 1] = { kind = "tab", k = n.pos, from = from, to = to }
        elseif t == types.functionNode and depth > 1 then
          marks[#marks + 1] = { kind = "mirror", from = from, to = to }
        elseif (t == types.exitNode or (t == types.insertNode and n.pos == 0)) and depth >= 1 then
          marks[#marks + 1] = { kind = "exit", from = from, to = to }
        end
      end
      for _, c in ipairs(n.nodes or {}) do walk(c, depth + 1) end
      if n.snip then walk(n.snip, depth + 1) end
    end
    walk(root, 0)
  end
  return { lines = vim.api.nvim_buf_get_lines(0, 0, -1, false), row = cur[1], col = cur[2], mode = mode, sel = sel, marks = marks }
]]

local PER_CHILD = 150
local results = {}
local list = jobs.examples or {}
for n, job in ipairs(list) do
  if (n - 1) % PER_CHILD == 0 then
    start()
  end
  local ok, err = pcall(function()
    local lines, r, c = open(job.before, job.ft)
    if job.select then
      lua([[
        local r, c, len = ...
        vim.api.nvim_win_set_cursor(0, { r, c })
        vim.cmd("normal! v" .. (len > 1 and ((len - 1) .. "l") or ""))
      ]], r, c, job.select)
      sync()
      rpc("nvim_input", "<C-g>")
      sync()
    else
      local len = #lines[r]
      rpc("nvim_input", (c >= len and len > 0) and "A" or "i")
      sync()
    end
    for _, tk in ipairs(tokens(job.keys)) do
      rpc("nvim_input", tk)
      sync()
    end
    results[job.id] = lua(STATE)
  end)
  if not ok then
    results[job.id] = { error = tostring(err) }
  end
end

local dump = vim.NIL
if jobs.dump then
  start()
  open("\\documentclass{article}\n\\begin{document}\n‸\n\\end{document}", "tex")
  -- in insert mode, so completion (blink.cmp) insert-mode maps are attached too
  rpc("nvim_input", "i")
  sync()
  dump = lua([[
    local cfg = vim.fn.stdpath("config")
    local maps = {}
    for _, mode in ipairs({ "n", "i", "v", "x", "s", "o", "c", "t" }) do
      for _, scope in ipairs({ "global", "buffer" }) do
        local list = scope == "global" and vim.api.nvim_get_keymap(mode) or vim.api.nvim_buf_get_keymap(0, mode)
        for _, m in ipairs(list) do
          local src = ""
          if m.callback then
            local i = debug.getinfo(m.callback, "S")
            src = i.source:gsub("^@", "") .. ":" .. i.linedefined
          end
          local script = ""
          if m.sid and m.sid > 0 then
            local s = vim.fn.getscriptinfo({ sid = m.sid })[1]
            script = s and s.name or ""
          end
          maps[#maps + 1] = { mode = mode, scope = scope, lhs = m.lhs, rhs = m.rhs or "", desc = m.desc or "", src = src, script = script }
        end
      end
    end
    local cmds = {}
    local function add_cmds(tbl, scope)
      for name, c in pairs(tbl) do
        cmds[#cmds + 1] = { name = name, scope = scope, desc = c.definition or "", nargs = c.nargs, script = (c.script_id and c.script_id > 0) and ((vim.fn.getscriptinfo({ sid = c.script_id })[1] or {}).name or "") or "" }
      end
    end
    add_cmds(vim.api.nvim_get_commands({ builtin = false }), "global")
    add_cmds(vim.api.nvim_buf_get_commands(0, {}), "buffer")
    local prefs = {}
    for k, v in pairs(require("config.prefs")) do
      prefs[k] = v == nil and vim.NIL or v
    end
    local prefs_defaults = require("config.prefs")._defaults or {}
    local snips = require("snips")
    local list = snips.list()
    for _, m in ipairs(list) do
      m.check = nil
    end
    local errors = {}
    for p, e in pairs(snips.errors) do
      errors[#errors + 1] = { file = vim.fs.basename(p), error = tostring(e) }
    end
    return {
      keymaps = maps,
      commands = cmds,
      prefs = prefs,
      prefs_defaults = prefs_defaults,
      snippets = list,
      snippets_dir = snips.dir(),
      errors = errors,
      options = { number = vim.o.number, relativenumber = vim.o.relativenumber, conceallevel = vim.o.conceallevel, spelllang = vim.o.spelllang, clipboard = vim.o.clipboard, timeoutlen = vim.o.timeoutlen, mapleader = vim.g.mapleader, maplocalleader = vim.g.maplocalleader },
      config = cfg,
    }
  ]])
end

local fd = assert(io.open(OUT, "w"))
fd:write(vim.json.encode({ examples = results, dump = dump }))
fd:close()
if chan then
  pcall(rpc, "nvim_command", "qa!")
  vim.fn.jobstop(chan)
end
vim.fn.delete(tmpdir, "rf")
os.exit(0)
