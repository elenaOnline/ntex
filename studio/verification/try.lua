-- Adversary's measuring driver (headless). Modeled on ~/.config/texnvim/tests/driver.lua.
--   TEXNVIM_SNIPPETS=<temp copy> nvim --clean --headless -l try.lua cases.json out.json
-- cases.json: [{ name, entry (Lua lines placed inside a group list), file?, ft ("tex"|"md"),
--   text (buffer, ‸ = cursor), steps = [keys…], select? = n chars at ‸ (select mode) }]
-- Also: { name, op = "list" } dumps require("snips").list().
io.stdout:setvbuf("no")
local CASES, OUT = arg[1], arg[2]
local SNIPS = assert(vim.env.TEXNVIM_SNIPPETS)
local child_env = {
  NVIM_APPNAME = "texnvim",
  TEXNVIM_SNIPPETS = SNIPS,
  TEXNVIM_NO_REFERENCE = "1",
  TEXNVIM_NO_VIEWER = "1",
  TEXNVIM_NO_COMPILE = "1",
  TEXNVIM_NO_LSP = "1",
  TEXNVIM_NO_WATCH = "1", -- only explicit reloads (a delayed fs_event reload could swap snippets mid-typing)
  TEXNVIM_IMAGES = "0",
  HOME = vim.env.HOME,
  PATH = vim.env.PATH,
  TERM = "xterm-256color",
}
local chan = vim.fn.jobstart({ "nvim", "--embed", "--headless", "-n", "-i", "NONE" }, { rpc = true, env = child_env, clear_env = true })
assert(chan > 0)
local child_pid = vim.fn.jobpid(chan)
local deadline
local wd = vim.uv.new_timer()
wd:start(1000, 1000, function()
  if deadline and vim.uv.now() > deadline then
    vim.uv.kill(child_pid, "sigkill")
    deadline = nil
  end
end)
local function rpc(m, ...)
  deadline = vim.uv.now() + 30000
  local r = vim.rpcrequest(chan, m, ...)
  deadline = nil
  return r
end
local function lua(code, ...) return rpc("nvim_exec_lua", code, { ... }) end
local function sync() rpc("nvim_eval", "1"); rpc("nvim_eval", "1") end
local CURSOR = "‸"
local function tokens(keys)
  local out, i = {}, 1
  while i <= #keys do
    local special = keys:match("^<[%w%-]+>", i)
    if special and special ~= "<>" then
      out[#out + 1] = special; i = i + #special
    else
      local ch = keys:match("^[%z\1-\127\194-\244][\128-\191]*", i)
      if ch == "<" then ch = "<lt>" end
      out[#out + 1] = ch; i = i + (ch == "<lt>" and 1 or #ch)
    end
  end
  return out
end
local function type_keys(keys)
  for _, tk in ipairs(tokens(keys)) do rpc("nvim_input", tk); sync() end
end
local function split_marker(text)
  local lines = vim.split(text, "\n", { plain = true })
  for r, l in ipairs(lines) do
    local c = l:find(CURSOR, 1, true)
    if c then lines[r] = l:sub(1, c - 1) .. l:sub(c + #CURSOR); return lines, r, c - 1 end
  end
  return lines, 1, 0
end
local tmp = vim.fn.tempname(); vim.fn.mkdir(tmp, "p")
local n = 0
lua([[
  _G.__adv = {}
  vim.api.nvim_create_autocmd("User", { pattern = "LuasnipPreExpand", callback = function()
    local sn = require("luasnip").session.event_node
    local m = sn and sn.ntex or {}
    local src = sn and sn._source or {}
    table.insert(_G.__adv, { trig = m.trig or (sn and sn.trigger), kind = m.kind, file = src.file and vim.fs.basename(src.file) or "?" })
  end })
]])
local function open(ft, text)
  n = n + 1
  local path = ("%s/c%03d.%s"):format(tmp, n, ft == "tex" and "tex" or "md")
  local lines, r, c = split_marker(text)
  rpc("nvim_input", "<Esc>"); sync()
  lua([[
    local path, lines, r, c = ...
    pcall(function() require("luasnip").unlink_current() end)
    vim.cmd("silent! %bwipeout!")
    vim.cmd.edit(path)
    local buf = vim.api.nvim_get_current_buf()
    vim.b[buf].texnvim_template_done = true
    vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
    vim.api.nvim_win_set_cursor(0, { r, c })
    _G.__adv = {}
  ]], path, lines, r, c)
  sync()
  local len = #lines[r]
  rpc("nvim_input", (c >= len and len > 0) and "A" or "i"); sync()
end
local function state()
  return lua([[
    local cur = vim.api.nvim_win_get_cursor(0)
    local mode = vim.fn.mode()
    local sel
    if mode == "s" or mode == "v" then
      sel = table.concat(vim.fn.getregion(vim.fn.getpos("v"), vim.fn.getpos("."), { type = "v" }), "\n")
    end
    local ex = _G.__adv; _G.__adv = {}
    return { lines = vim.api.nvim_buf_get_lines(0, 0, -1, false), row = cur[1], col = cur[2], mode = mode, sel = sel, expanded = ex }
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
local HEADER = [[
local h = require("snips.helpers")
local math, math_tab, text, suffix, post, pat, vis = h.math, h.math_tab, h.text, h.suffix, h.post, h.pat, h.vis
return h.group("Adversary", {
]]
local function write_file(path, s)
  local fd = assert(io.open(path, "w")); fd:write(s); fd:close()
end
local cases = vim.json.decode(table.concat(vim.fn.readfile(CASES), "\n"))
local results = {}
for _, cs in ipairs(cases) do
  local res = { name = cs.name }
  if cs.op == "list" then
    res.list = lua([[ local out = {} for _, m in ipairs(require("snips").list()) do
      out[#out+1] = { trigger = m.trig, pattern = m.pattern, kind = m.kind, where = m.where, how = m.how, file = m.file, line = m.line,
        line_rule = m.line_rule, ft = m.ft, priority = m.priority, eff = m.effective_priority, body = m.body } end return out ]])
  elseif cs.op == "errors" then
    res.errors = lua([[ local out = {} for k, v in pairs(require("snips").errors) do out[vim.fs.basename(k)] = tostring(v) end return out ]])
    if vim.tbl_isempty(res.errors) then res.errors = vim.empty_dict() end
  elseif cs.op == "checkpath" then
    res.check = lua([[ local ok, err = require("snips").check(...) return { ok = ok, err = err and tostring(err) or vim.NIL } ]], cs.path)
  elseif cs.op == "check" then
    local path = SNIPS .. "/tex/" .. (cs.file or "zz_adv.lua")
    write_file(path, cs.text)
    res.check = lua([[ local ok, err = require("snips").check(...) return { ok = ok, err = err and tostring(err) or vim.NIL } ]], path)
    os.remove(path)
  else
    local path = SNIPS .. "/tex/" .. (cs.file or "zz_adv.lua")
    if cs.entry then
      write_file(path, cs.raw and cs.entry or (HEADER .. cs.entry .. "\n})\n"))
      local r = lua([[ local p = ... local ok = require("snips").reload_file(p)
        return { ok = ok, err = require("snips").errors[vim.fs.normalize(p)] and tostring(require("snips").errors[vim.fs.normalize(p)]) or vim.NIL } ]], path)
      res.load = r
    end
    if cs.select then
      open(cs.ft, cs.text)
      rpc("nvim_input", "<Esc>"); sync()
      local _, r, c = split_marker(cs.text)
      lua([[ local r, c, len = ... vim.api.nvim_win_set_cursor(0, { r, c })
        vim.cmd("normal! v" .. (len > 1 and ((len - 1) .. "l") or "")) ]], r, c, cs.select)
      sync(); rpc("nvim_input", "<C-g>"); sync()
    else
      open(cs.ft, cs.text)
    end
    res.steps = {}
    for _, k in ipairs(cs.steps or {}) do
      type_keys(k)
      local st = state()
      res.steps[#res.steps + 1] = { keys = k, buffer = render(st), mode = st.mode, sel = st.sel, expanded = st.expanded }
    end
    if cs.entry then
      os.remove(path)
      lua([[ local p = ... local ls = require("luasnip")
        -- drop the file's snippets: reload an empty group in its place, then remove the file
        local fd = io.open(p, "w") fd:write('return require("snips.helpers").group("x", {})') fd:close()
        require("snips").reload_file(p) os.remove(p) ]], path)
    end
  end
  results[#results + 1] = res
  io.write(".")
end
write_file(OUT, vim.json.encode(results))
pcall(rpc, "nvim_command", "qa!")
vim.fn.jobstop(chan)
vim.fn.delete(tmp, "rf")
io.write("\ndone\n")
os.exit(0)
