-- ntex snippets: LuaSnip, loaded from the files in ~/.config/texnvim/snippets/
-- with LuaSnip's from_lua loader. .tex uses them directly; .md (and plaintex)
-- through filetype_extend, with maths detection that works in both
-- (lua/snips/context.lua).
--
--   snippets/tex/*.lua   one file per topic, written with lua/snips/helpers.lua
--   snippets/README.md   how to add one
--
-- Saving a snippet file reloads just that file. A file with an error is not
-- loaded: its previous snippets stay active and :SnippetReport shows the
-- error. $TEXNVIM_SNIPPETS points at another snippets folder (the tests do this);
-- $TEXNVIM_NO_WATCH=1 turns off reloading on changes made by other programs.

local api = vim.api
local M = {}

M.generation = 0 -- bumped after every successful (re)load
M.errors = {} -- path → message, for files that failed to load

function M.dir()
  return vim.env.TEXNVIM_SNIPPETS or vim.fs.joinpath(vim.fn.stdpath("config"), "snippets")
end

function M.files()
  local out = vim.fn.globpath(vim.fs.joinpath(M.dir(), "tex"), "*.lua", false, true)
  table.sort(out)
  return out
end

local function is_snippet_file(path)
  local dir = vim.fs.normalize(vim.fs.joinpath(M.dir(), "tex"))
  path = vim.fs.normalize(path)
  return path:sub(1, #dir + 1) == dir .. "/" and path:match("%.lua$") ~= nil
end

local function short(path)
  return vim.fn.fnamemodify(path, ":~")
end

local real_loadfile = loadfile

--- Run a snippet file the way the loader will (same globals), without adding
--- anything. Returns true, or false and the error.
function M.check(path)
  local fn, err = real_loadfile(path)
  if not fn then
    return false, err
  end
  local ls = require("luasnip")
  local env = vim.tbl_extend("force", _G, ls.get_snip_env())
  setmetatable(env, getmetatable(ls.get_snip_env()))
  setfenv(fn, env)
  local ok, res = pcall(fn)
  if not ok then
    return false, res
  end
  if type(res) ~= "table" then
    return false, "the file must return a list of snippets (return h.group(\"…\", { … }))"
  end
  return true
end

-- LuaSnip stops loading the whole folder at the first file that errors. While
-- it loads, each file is checked first; one that fails is loaded as empty
-- instead (and reported), so one typo never switches off every other snippet.
local function with_guarded_loadfile(fn)
  _G.loadfile = function(path, ...)
    local chunk, err = real_loadfile(path, ...)
    if not (path and is_snippet_file(path)) then
      return chunk, err
    end
    local ok, why = M.check(path)
    if not ok then
      M.errors[path] = why or err
      return function()
        return {}
      end
    end
    M.errors[path] = nil
    return chunk
  end
  local ok, err = pcall(fn)
  _G.loadfile = real_loadfile
  if not ok then
    error(err)
  end
end

local function after_load()
  require("snips.visual").refresh()
  require("snips.context").invalidate()
  M.generation = M.generation + 1
end

--- (Re)load one snippet file after it changed. Keeps the old snippets if the
--- new version has an error.
function M.reload_file(path)
  path = vim.fs.normalize(vim.fn.fnamemodify(path, ":p"))
  local ok, err = M.check(path)
  if not ok then
    M.errors[path] = err
    -- one line, so it never turns into a "Press ENTER" prompt while typing
    vim.notify(("Snippets: %s has an error, its previous snippets are kept (:SnippetReport)"):format(vim.fs.basename(path)), vim.log.levels.ERROR)
    return false
  end
  M.errors[path] = nil
  with_guarded_loadfile(function()
    require("luasnip.loaders").reload_file(path)
  end)
  after_load()
  pcall(function()
    require("config.reference").on_sources_changed()
  end)
  return true
end

--- Reload every snippet file.
function M.reload_all()
  local good = true
  for _, f in ipairs(M.files()) do
    good = M.reload_file(f) and good
  end
  return good
end

-- hot reload: saved in ntex (BufWritePost) or by another program (fs_event)
local watcher
local pending = {}
local debounce

local function schedule_reload(path)
  pending[path] = true
  if debounce then
    debounce:stop()
  else
    debounce = vim.uv.new_timer()
  end
  debounce:start(150, 0, vim.schedule_wrap(function()
    local list = vim.tbl_keys(pending)
    pending = {}
    for _, p in ipairs(list) do
      if vim.uv.fs_stat(p) then
        M.reload_file(p)
      end
    end
  end))
end

local function watch()
  if watcher then
    watcher:stop()
  end
  -- TEXNVIM_NO_WATCH=1: only explicit reloads (Ntex Studio's headless bridge
  -- writes and reloads its private copy itself; a second, delayed reload
  -- would swap the snippets out from under a snippet it is typing)
  if vim.env.TEXNVIM_NO_WATCH == "1" then
    watcher = nil
    return
  end
  watcher = vim.uv.new_fs_event()
  if not watcher then
    return
  end
  local dir = vim.fs.joinpath(M.dir(), "tex")
  watcher:start(dir, {}, function(err, fname)
    if err or not fname or not fname:match("%.lua$") then
      return
    end
    vim.schedule(function()
      schedule_reload(vim.fs.normalize(vim.fs.joinpath(dir, vim.fs.basename(fname))))
    end)
  end)
end

function M.setup()
  local ls = require("luasnip")
  local types = require("luasnip.util.types")
  ls.setup({
    enable_autosnippets = true,
    update_events = { "TextChanged", "TextChangedI" },
    region_check_events = { "CursorMoved", "CursorMovedI" },
    delete_check_events = { "TextChanged", "TextChangedI" },
    loaders_store_source = true, -- file + line of every snippet, for :SnippetReport and the reference page
    ext_opts = {
      [types.insertNode] = {
        passive = { hl_group = "SnippetTabstop" },
      },
    },
  })
  api.nvim_set_hl(0, "SnippetTabstop", { default = true, underdotted = true, sp = "Gray" })
  ls.filetype_extend("markdown", { "tex" })
  ls.filetype_extend("plaintex", { "tex" })
  require("snips.mdmath").force_math = require("snips.settings").get().force_math_languages

  -- reloading is done here (checked first), not by the loader's own watchers
  with_guarded_loadfile(function()
    require("luasnip.loaders.from_lua").load({
      paths = { M.dir() },
      fs_event_providers = { autocmd = false, libuv = false },
    })
  end)
  after_load()
  for path, err in pairs(M.errors) do
    vim.schedule(function()
      vim.notify(("Snippets: %s has an error and was not loaded (:SnippetReport)\n%s"):format(vim.fs.basename(path), tostring(err)), vim.log.levels.ERROR)
    end)
  end
  watch()

  local group = api.nvim_create_augroup("ntex_snippets", { clear = true })
  api.nvim_create_autocmd("BufWritePost", {
    group = group,
    callback = function(ev)
      local f = vim.fs.normalize(vim.fn.fnamemodify(ev.file, ":p"))
      if is_snippet_file(f) then
        schedule_reload(f)
      end
    end,
  })
  local ctx_mod = require("snips.context")
  api.nvim_create_autocmd("FileType", {
    group = group,
    pattern = { "tex", "plaintex", "markdown" },
    callback = function(ev)
      require("snips.features").attach(ev.buf)
    end,
  })
  for _, buf in ipairs(api.nvim_list_bufs()) do
    if api.nvim_buf_is_loaded(buf) and ctx_mod.filetypes[vim.bo[buf].filetype] then
      require("snips.features").attach(buf)
    end
  end

  local conceal = require("snips.conceal")
  conceal.set(require("config.prefs").conceal == true, true)

  local function cmd(name, fn, desc, old)
    api.nvim_create_user_command(name, fn, { desc = desc })
    if old then
      api.nvim_create_user_command(old, fn, { desc = desc .. " (old name of :" .. name .. ")" })
    end
  end
  cmd("SnippetReport", function()
    require("snips.report").show()
  end, "Snippets: loaded per file, load errors", "LatexSuiteReport")
  cmd("SnippetEdit", function()
    M.edit()
  end, "Snippets: pick a snippet file to edit", "LatexSuiteEdit")
  cmd("SnippetReload", function()
    if M.reload_all() then
      vim.notify("Snippets reloaded", vim.log.levels.INFO)
    end
  end, "Snippets: reload every snippet file", "LatexSuiteReload")
  cmd("Conceal", function()
    conceal.toggle()
  end, "Pretty symbols on / off (F2)", "LatexSuiteConceal")
  vim.keymap.set({ "n", "i" }, "<F2>", conceal.toggle, { desc = "Toggle conceal (pretty math symbols)" })
end

--- Space e: pick a snippet file (or the README / decision log) and open it.
function M.edit()
  local items = {}
  for _, f in ipairs(M.files()) do
    items[#items + 1] = f
  end
  for _, extra in ipairs({ "README.md", "DECISIONS.md" }) do
    local p = vim.fs.joinpath(M.dir(), extra)
    if vim.uv.fs_stat(p) then
      items[#items + 1] = p
    end
  end
  vim.ui.select(items, {
    prompt = "Snippet file",
    format_item = function(p)
      local name = vim.fs.basename(p)
      local title = ""
      if p:match("%.lua$") then
        local first = (vim.fn.readfile(p, "", 1)[1] or ""):gsub("^%-%-%s*", "")
        title = "  " .. first
      end
      return name .. title
    end,
  }, function(choice)
    if choice then
      vim.cmd.edit(vim.fn.fnameescape(choice))
    end
  end)
end

--- Every loaded snippet with its metadata, in file order (report, reference page).
function M.list()
  local ls = require("luasnip")
  local out, seen = {}, {}
  for _, kind in ipairs({ "snippets", "autosnippets" }) do
    for _, snip in ipairs(ls.get_snippets("tex", { type = kind })) do
      if snip.ntex and not snip.invalidated and not seen[snip] then
        seen[snip] = true
        local src = snip._source or {}
        local m = vim.deepcopy(snip.ntex)
        m.file = src.file and vim.fs.basename(src.file) or "?"
        m.path = src.file
        m.line = src.line
        m.effective_priority = snip.priority
        m.trigger = snip.trigger
        out[#out + 1] = m
      end
    end
  end
  table.sort(out, function(a, b)
    if a.file ~= b.file then
      return a.file < b.file
    end
    if (a.line or 0) ~= (b.line or 0) then
      return (a.line or 0) < (b.line or 0)
    end
    return a.where < b.where -- env(): display before inline
  end)
  return out
end

return M
