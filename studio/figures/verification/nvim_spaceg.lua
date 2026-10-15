-- ADVERSARY: Space g path detection, in the real config, headless.
--   nvim --clean --headless -l nvim_spaceg.lua TMPROOT OUT.json
-- Controller (this file) starts the real config in a child
-- (`nvim --embed --headless -n -i NONE`, NVIM_APPNAME=texnvim) exactly like
-- tests/driver.lua, with TEXNVIM_NO_VIEWER=1 (nothing ever opens),
-- TEXNVIM_NO_COMPILE=1, TEXNVIM_NO_LSP=1, snippets from a temp copy. Every file
-- it touches is under TMPROOT. It presses <Space>g on prepared lines and reads
-- figures.launched (the would-be window launches).
io.stdout:setvbuf("no")
local T, OUT = arg[1], arg[2]
local env = {
  NVIM_APPNAME = "texnvim",
  TEXNVIM_SNIPPETS = T .. "/snippets",
  TEXNVIM_NO_REFERENCE = "1",
  TEXNVIM_NO_VIEWER = "1",
  TEXNVIM_NO_COMPILE = "1",
  TEXNVIM_NO_LSP = "1",
  TEXNVIM_IMAGES = "0",
  HOME = vim.env.HOME,
  PATH = vim.env.PATH,
  TERM = "xterm-256color",
}
local chan = vim.fn.jobstart({ "nvim", "--embed", "--headless", "-n", "-i", "NONE" }, { rpc = true, env = env, clear_env = true })
assert(chan > 0)
local pid = vim.fn.jobpid(chan)
local function rpc(m, ...) return vim.rpcrequest(chan, m, ...) end
local function lua(code, ...) return rpc("nvim_exec_lua", code, { ... }) end
local function sync() rpc("nvim_eval", "1"); rpc("nvim_eval", "1") end
local function wait(ms) vim.wait(ms, function() return false end) end

wait(1500)
lua([[
  _G.__F = require("config.figures")
  _G.__prompts = {}
  vim.ui.input = function(opts, cb) table.insert(_G.__prompts, { prompt = opts.prompt, default = opts.default }); cb(opts.default) end
  vim.ui.select = function(items, opts, cb) table.insert(_G.__prompts, { select = items }); cb(items[1], 1) end
]])

local cases = {
  -- file (relative to T), line, cursor col (0-based byte; nil = 0), expected fig (relative to T) or "CREATE"/"NOOPEN", tier, why
  { "notes.tex", [[\includegraphics{figures/kernel}]], 5, "figures/kernel.fig", "spec", "the plain form" },
  { "notes.tex", [[  \includegraphics[width=0.5\textwidth]{figures/kernel}]], 3, "figures/kernel.fig", "spec", "options with a backslash and a dot" },
  { "notes.tex", [[\includegraphics[scale=0.8]{figures/kernel.pdf}]], 0, "figures/kernel.fig", "inferred", "an explicit .pdf extension" },
  { "notes.tex", [[\includegraphics{./figures/kernel}]], 0, "figures/kernel.fig", "inferred", "./ prefix" },
  { "chapter2/sec.tex", [[\includegraphics{figures/cyc}]], 0, "chapter2/figures/cyc.fig", "spec", "resolved against the .tex's folder (nested)" },
  { "chapter2/sec.tex", [[\includegraphics{../figures/kernel}]], 0, "figures/kernel.fig", "inferred", "../ from a nested .tex" },
  { "notes.tex", [[\includegraphics{chapter2/figures/cyc}]], 0, "chapter2/figures/cyc.fig", "spec", "…figures/NAME… inside a deeper path" },
  { "notes.tex", [[\includegraphics{figures/sub/deep}]], 0, "figures/sub/deep.fig", "inferred", "a subfolder of figures/" },
  { "notes.tex", [[\includegraphics*[trim=1 2 3 4, clip]{figures/kernel}]], 0, "figures/kernel.fig", "inferred", "starred form (graphicx)" },
  { "notes.tex", [[\includegraphics{figures/a}\hfill\includegraphics{figures/b}]], 40, "figures/b.fig", "gap", "two on a line, cursor on the second" },
  { "notes.tex", [[\includegraphics{figures/a}\hfill\includegraphics{figures/b}]], 3, "figures/a.fig", "gap", "two on a line, cursor on the first" },
  { "notes.tex", [[\includegraphics[width=\linewidth]{figures/kernel}% a comment]], 0, "figures/kernel.fig", "spec", "trailing comment" },
  { "notes.tex", [[\includegraphics{img/photo}]], 0, "NOOPEN:img/photo.fig", "inferred", "not a figures/ path: must not open img/photo.fig" },
  { "notes.md", [[\includegraphics{figures/kernel}]], 0, "figures/kernel.fig", "spec", ".md with includegraphics" },
  { "notes.md", [[![the kernel](figures/kernel.pdf)]], 0, "figures/kernel.fig", "gap", "markdown image syntax (FIGURES.md only names includegraphics)" },
  { "notes.tex", "\\includegraphics{" .. T .. "/figures/kernel}", 0, "figures/kernel.fig", "inferred", "absolute path" },
  { "notes.tex", [[\includegraphics{figures/kernel-2_b}]], 0, "figures/kernel-2_b.fig", "spec", "hyphen, digit, underscore in NAME" },
  { "notes.tex", [[\begin{center}\includegraphics{figures/kernel}\end{center}]], 30, "figures/kernel.fig", "spec", "inside an environment on one line" },
  { "notes.tex", [[Some prose with no figure.]], 0, "CREATE", "spec", "no includegraphics: asks for a name, creates, inserts" },
  { "chapter2/sec.tex", [[Prose.]], 0, "CREATE", "spec", "create from a nested .tex goes to chapter2/figures/" },
}

local results = {}
for i, c in ipairs(cases) do
  local file, line, col, want, tier, why = c[1], c[2], c[3] or 0, c[4], c[5], c[6]
  local path = T .. "/" .. file
  rpc("nvim_command", "silent! %bwipeout!")
  rpc("nvim_command", "edit " .. vim.fn.fnameescape(path))
  lua([[
    local line, col = ...
    vim.api.nvim_buf_set_lines(0, 0, -1, false, { "% test", line, "% end" })
    vim.api.nvim_win_set_cursor(0, { 2, col })
    _G.__F.launched = {}
    _G.__prompts = {}
  ]], line, col)
  local ok_before = vim.fn.globpath(T, "**/*.fig", false, true)
  rpc("nvim_input", "<Esc><Space>g")
  sync()
  wait(400)
  sync()
  local launched = lua("return vim.deepcopy(_G.__F.launched)")
  local prompts = lua("return vim.deepcopy(_G.__prompts)")
  local buf = lua("return vim.api.nvim_buf_get_lines(0, 0, -1, false)")
  local msgs = rpc("nvim_exec2", "messages", { output = true }).output
  local after = vim.fn.globpath(T, "**/*.fig", false, true)
  local opened = nil
  for _, argv in ipairs(launched) do
    for _, a in ipairs(type(argv) == "table" and argv or { argv }) do
      if type(a) == "string" and a:match("%.fig$") then opened = a end
    end
  end
  local created = {}
  for _, f in ipairs(after) do
    if not vim.tbl_contains(ok_before, f) then table.insert(created, f) end
  end
  local verdict
  local norm = function(p) return p and vim.fs.normalize(vim.uv.fs_realpath(vim.fs.dirname(p)) or vim.fs.dirname(p)) .. "/" .. vim.fs.basename(p) or nil end
  local realT = vim.uv.fs_realpath(T)
  if want == "CREATE" then
    verdict = (#prompts > 0 and opened ~= nil) and "pass" or "fail"
  elseif want:match("^NOOPEN:") then
    local bad = want:sub(8)
    verdict = (opened == nil or not opened:find(bad, 1, true)) and "pass" or "fail"
  else
    verdict = (opened and norm(opened) == realT .. "/" .. want) and "pass" or "fail"
  end
  table.insert(results, { i = i, file = file, line = line, col = col, want = want, tier = tier, why = why,
    opened = opened, prompts = prompts, created = created, buffer = buf, verdict = verdict,
    messages = msgs:sub(-300) })
  io.stdout:write(string.format("%-4s %-8s %-18s %-55s want %-28s got %s\n", verdict, tier, file, line:sub(1, 55), want, tostring(opened and opened:gsub(vim.pesc(realT) .. "/", ""):gsub(vim.pesc(T) .. "/", "") or "nil")))
end

-- next free number: with fig-1.fig and fig-3.fig present
lua([[ local T = ...; vim.fn.writefile({}, T .. "/figures/fig-1.fig"); vim.fn.writefile({}, T .. "/figures/fig-3.fig") ]], T)
rpc("nvim_command", "silent! %bwipeout!")
rpc("nvim_command", "edit " .. vim.fn.fnameescape(T .. "/notes.tex"))
lua([[ vim.api.nvim_buf_set_lines(0, 0, -1, false, { "prose" }); _G.__F.launched = {}; _G.__prompts = {} ]])
rpc("nvim_input", "<Esc><Space>g")
sync(); wait(400); sync()
local p = lua("return vim.deepcopy(_G.__prompts)")
table.insert(results, { i = "next-free", prompts = p, tier = "gap", why = "fig-1, fig-3 exist: 'the next free number' (2 = lowest free; 4 = after the highest)" })
io.stdout:write("next-free default with fig-1, fig-3 present: " .. vim.inspect(p) .. "\n")

vim.fn.writefile({ vim.json.encode(results) }, OUT)
pcall(rpc, "nvim_command", "qa!")
vim.wait(500)
vim.uv.kill(pid, "sigterm")
os.exit(0)
