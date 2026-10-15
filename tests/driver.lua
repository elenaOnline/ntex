-- Headless test driver. Run through tests/run.sh.
--
-- This file runs in a *controller* Neovim (`nvim --clean --headless -l`),
-- which starts the real config in a child (`nvim --embed --headless`,
-- NVIM_APPNAME=texnvim) and types into it one key at a time over RPC, the way
-- a person types: every key is fully processed (autosnippets included) before
-- the next one is sent. No windows are ever opened.
--
-- Buffer notation in cases: "‸" marks the cursor (insert mode).

io.stdout:setvbuf("no")
local SUITE = arg[1] or "mine"
local OUT = arg[2]

local child_env = {
  NVIM_APPNAME = vim.env.TEXNVIM_APPNAME or "texnvim",
  TEXNVIM_SNIPPETS = assert(vim.env.TEXNVIM_SNIPPETS),
  TEXNVIM_NO_REFERENCE = "1",
  -- never a viewer: no Skim reload after a compile, no \lv / \ls, no browser
  TEXNVIM_NO_VIEWER = "1",
  -- nothing compiles unless asked: a compile can make VimTeX talk to Skim
  -- (the ide suite compiles: compile errors are what it tests; no viewer, see below)
  TEXNVIM_NO_COMPILE = ((SUITE == "system" and vim.env.TEXNVIM_TEST_COMPILE == "1") or SUITE == "ide") and "0" or "1",
  -- images off while typing tests; the health suite turns them on (with the
  -- same override the ntex launcher sets inside cmux) to check snacks.image
  TEXNVIM_IMAGES = SUITE == "system" and "1" or "0",
  SNACKS_GHOSTTY = SUITE == "system" and "true" or nil,
  HOME = vim.env.HOME,
  PATH = vim.env.PATH,
  TERM = "xterm-256color",
}
local chan = vim.fn.jobstart({ "nvim", "--embed", "--headless", "-n", "-i", "NONE" }, { rpc = true, env = child_env, clear_env = true })
assert(chan > 0, "could not start child nvim")

-- watchdog: if the child stops answering (a bug, or a prompt), kill it rather
-- than hang forever
local child_pid = vim.fn.jobpid(chan)
local deadline = nil
local watchdog = vim.uv.new_timer()
watchdog:start(1000, 1000, function()
  if deadline and vim.uv.now() > deadline then
    io.stderr:write("\n!! child nvim did not answer for 30 s — killing it\n")
    vim.uv.kill(child_pid, "sigkill")
    deadline = nil
  end
end)

local function rpc(method, ...)
  deadline = vim.uv.now() + 30000
  local res = vim.rpcrequest(chan, method, ...)
  deadline = nil
  return res
end
local function lua(code, ...)
  return rpc("nvim_exec_lua", code, { ... })
end
local function sync()
  -- non-fast requests run only once typeahead is consumed; the second one also
  -- lets vim.schedule() callbacks queued by the first key run.
  rpc("nvim_eval", "1")
  rpc("nvim_eval", "1")
end

local CURSOR = "‸"

--- split "ab<Tab>c" into {"a","b","<Tab>","c"}
local function tokens(keys)
  local out = {}
  local i = 1
  while i <= #keys do
    local special = keys:match("^<[%w%-]+>", i)
    if special then
      out[#out + 1] = special
      i = i + #special
    else
      local ch = keys:match("^[%z\1-\127\194-\244][\128-\191]*", i)
      if ch == "<" then
        ch = "<lt>"
        out[#out + 1] = ch
        i = i + 1
      else
        out[#out + 1] = ch
        i = i + #ch
      end
    end
  end
  return out
end

local timings = { e2e = {} }

local function type_keys(keys)
  for _, tk in ipairs(tokens(keys)) do
    local t0 = vim.uv.hrtime()
    rpc("nvim_input", tk)
    sync()
    timings.e2e[#timings.e2e + 1] = (vim.uv.hrtime() - t0) / 1e6
  end
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
  return lines, nil, nil
end

local tmpdir = vim.fn.tempname()
vim.fn.mkdir(tmpdir, "p")
local file_n = 0

--- open a fresh buffer of the given filetype containing `text`, insert mode at ‸
--- reuse = true: keep the current buffer if it has that filetype, just reset
--- its text (VimTeX's syntax breaks after ~220 .tex buffers in one session).
local function open(ft, text, reuse)
  file_n = file_n + 1
  local path = ("%s/case%03d.%s"):format(tmpdir, file_n, ft == "tex" and "tex" or "md")
  local lines, r, c = split_marker(text)
  rpc("nvim_input", "<Esc>")
  sync()
  lua([[
    local path, lines, r, c, reuse = ...
    pcall(function() require("luasnip").unlink_current() end)
    local want = path:match("%.(%w+)$") == "tex" and "tex" or "markdown"
    if not (reuse and vim.bo.filetype == want) then
      vim.cmd("silent! %bwipeout!")
      vim.cmd.edit(path)
    end
    local buf = vim.api.nvim_get_current_buf()
    vim.b[buf].texnvim_template_done = true
    vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
    vim.api.nvim_win_set_cursor(0, { r, c })
  ]], path, lines, r, c, reuse == true)
  sync()
  local len = #lines[r]
  if c >= len and len > 0 then
    rpc("nvim_input", "A")
  else
    rpc("nvim_input", "i")
  end
  sync()
end

local function state()
  return lua([[
    local cur = vim.api.nvim_win_get_cursor(0)
    local mode = vim.fn.mode()
    local sel = nil
    if mode == "s" or mode == "v" then
      sel = table.concat(vim.fn.getregion(vim.fn.getpos("v"), vim.fn.getpos("."), { type = "v" }), "\n")
    end
    return { lines = vim.api.nvim_buf_get_lines(0, 0, -1, false), row = cur[1], col = cur[2], mode = mode, sel = sel }
  ]])
end

local function render(st)
  local lines = vim.deepcopy(st.lines)
  if st.mode == "i" then
    local l = lines[st.row] or ""
    lines[st.row] = l:sub(1, st.col) .. CURSOR .. l:sub(st.col + 1)
  end
  return table.concat(lines, "\n")
end

local results = { pass = 0, fail = 0, failures = {} }

local function check(name, expect, opts)
  opts = opts or {}
  local st = state()
  local got
  if expect:find(CURSOR, 1, true) then
    got = render(st)
  else
    got = table.concat(st.lines, "\n")
  end
  local ok = got == expect
  if ok and opts.mode and st.mode ~= opts.mode then
    ok = false
    got = got .. "   (mode " .. st.mode .. ", wanted " .. opts.mode .. ")"
  end
  if ok and opts.sel and st.sel ~= opts.sel then
    ok = false
    got = got .. "   (selection " .. vim.inspect(st.sel) .. ", wanted " .. vim.inspect(opts.sel) .. ")"
  end
  if ok then
    results.pass = results.pass + 1
    io.write(("  ok    %s\n"):format(name))
  else
    results.fail = results.fail + 1
    results.failures[#results.failures + 1] = name
    io.write(("  FAIL  %s\n        expected: %s\n        got:      %s\n"):format(name, vim.inspect(expect), vim.inspect(got)))
  end
  return ok
end

local function ok(name, cond, detail)
  if cond then
    results.pass = results.pass + 1
    io.write(("  ok    %s\n"):format(name))
  else
    results.fail = results.fail + 1
    results.failures[#results.failures + 1] = name
    io.write(("  FAIL  %s %s\n"):format(name, detail or ""))
  end
  return cond
end

--- one scenario: open, type, check (steps may alternate keys/expectations)
local function case(name, ft, text, steps)
  open(ft, text)
  for k = 1, #steps, 2 do
    type_keys(steps[k])
    local exp = steps[k + 1]
    local opts
    if type(exp) == "table" then
      opts = exp
      exp = exp[1]
    end
    local label = name
    if #steps > 2 then
      label = name .. " [" .. ((k + 1) / 2) .. "]"
    end
    check(("%-4s %s"):format(ft, label), exp, opts)
  end
end

--- select `len` characters starting at the cursor marker (select mode)
local function select_from_marker(ft, text, len)
  open(ft, text)
  rpc("nvim_input", "<Esc>")
  sync()
  local lines, r, c = split_marker(text)
  lua([[
    local r, c, len = ...
    vim.api.nvim_win_set_cursor(0, { r, c })
    vim.cmd("normal! v" .. (len > 1 and ((len - 1) .. "l") or ""))
  ]], r, c, len)
  sync()
  rpc("nvim_input", "<C-g>")
  sync()
end

io.write(("\n== suite: %s  (%s, snippets: %s)\n"):format(SUITE, child_env.NVIM_APPNAME, child_env.TEXNVIM_SNIPPETS))
local info = lua([[
  local list = require("snips").list()
  local c = { auto = 0, tab = 0, visual = 0 }
  for _, m in ipairs(list) do c[m.how] = c[m.how] + 1 end
  return { n = #list, c = c, errors = vim.tbl_count(require("snips").errors) }
]])
io.write(("   loaded %d snippets (%d by themselves, %d on Tab, %d visual), load errors: %d\n"):format(
  info.n, info.c.auto, info.c.tab, info.c.visual, info.errors))

local cases = dofile(vim.fs.joinpath(vim.fs.dirname(debug.getinfo(1).source:sub(2)), "cases.lua"))
cases.run(SUITE, {
  case = case,
  check = check,
  ok = ok,
  type_keys = type_keys,
  select_from_marker = select_from_marker,
  open = open,
  lua = lua,
  rpc = rpc,
  sync = sync,
  state = state,
  render = render,
  timings = timings,
  results = results,
  tmpdir = tmpdir,
})

io.write(("\n== %s: %d passed, %d failed\n"):format(SUITE, results.pass, results.fail))
if OUT then
  local fd = io.open(OUT, "w")
  fd:write(vim.json.encode({ pass = results.pass, fail = results.fail, failures = results.failures, suite = SUITE }))
  fd:close()
end
pcall(rpc, "nvim_command", "qa!")
vim.fn.jobstop(chan)
vim.fn.delete(tmpdir, "rf")
os.exit(results.fail == 0 and 0 or 1)
