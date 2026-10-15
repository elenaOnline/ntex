-- Ntex Studio: a small native app for making snippets, Tab shortcuts and
-- theorem-like blocks on the fly (~/.config/texnvim/studio/). It writes
-- snippets/tex/inbox.lua, which ntex hot-reloads, so a saved snippet works
-- here at once.
--
--   Space n          (normal) open the studio on its snippet page
--   Space n          (visual) the same, with the selection as the output
--   Space N          the inbox (snippets waiting to be sorted into your files)
--   :SnippetStudio [snippet|block|inbox]
--
-- The studio runs detached: it keeps running when ntex quits, and the next
-- Space n just brings it forward. Build it once with
-- ~/.config/texnvim/studio/build.sh. TEXNVIM_NO_VIEWER=1 (the tests) never
-- launches it.
local M = {}

local studio_dir = vim.fs.joinpath(vim.fn.stdpath("config"), "studio")
M.binary = vim.fs.joinpath(studio_dir, "target", "release", "ntex-studio")
M.PAGES = { "snippet", "block", "inbox" }

M.launched = {} -- argv of every launch (and would-be launch), newest last; tests read it

local function selection_text()
  local mode = vim.fn.mode()
  local vtype = (mode == "v" or mode == "V" or mode == "\22") and mode or vim.fn.visualmode()
  if vtype == "" then
    vtype = "v"
  end
  local ok, lines = pcall(vim.fn.getregion, vim.fn.getpos("v"), vim.fn.getpos("."), { type = vtype })
  if not ok or not lines then
    return nil
  end
  return table.concat(lines, "\n")
end

-- maths or text where the selection starts (snips.context, as the snippets see it)
local function context_at_selection()
  local ok, res = pcall(function()
    local ctx = require("snips.context")
    local a, b = vim.fn.getpos("v"), vim.fn.getpos(".")
    local first = (a[2] < b[2] or (a[2] == b[2] and a[3] <= b[3])) and a or b
    -- the context sees the character before col: look just inside the selection
    local c = ctx.get_at(vim.api.nvim_get_current_buf(), first[2] - 1, first[3])
    if not c then
      return nil
    end
    return ctx.in_math(c.mode) and not c.mode.textEnv and "math" or "text"
  end)
  return ok and res or nil
end

local function filetype()
  local ft = require("snips.context").filetypes[vim.bo.filetype]
  return ft -- "tex" | "markdown" | nil
end

local seq = 0
local function write_temp(text)
  seq = seq + 1
  local dir = vim.uv.os_tmpdir() or "/tmp"
  local path = vim.fs.joinpath(dir, ("ntex-studio-selection-%d-%d.txt"):format(vim.fn.getpid(), seq))
  local fd = io.open(path, "w")
  if not fd then
    return nil
  end
  fd:write(text)
  fd:close()
  return path
end

--- The command line for a launch. opts: page = "snippet"|"block"|"inbox",
--- output_file = path, context = "math"|"text", ft = "tex"|"markdown".
function M.command(opts)
  opts = opts or {}
  local argv = { M.binary }
  if opts.page then
    vim.list_extend(argv, { "--page", opts.page })
  end
  if opts.output_file then
    vim.list_extend(argv, { "--output-file", opts.output_file })
  end
  if opts.context then
    vim.list_extend(argv, { "--context", opts.context })
  end
  if opts.ft then
    vim.list_extend(argv, { "--ft", opts.ft })
  end
  return argv
end

--- Launch (or bring forward) the studio. Returns true if it was launched.
function M.launch(opts)
  local argv = M.command(opts)
  M.launched[#M.launched + 1] = argv
  if vim.env.TEXNVIM_NO_VIEWER == "1" then
    if opts and opts.output_file then
      os.remove(opts.output_file)
    end
    vim.notify("Ntex Studio: not opening (TEXNVIM_NO_VIEWER=1)", vim.log.levels.INFO)
    return false
  end
  if vim.fn.executable(M.binary) ~= 1 then
    if opts and opts.output_file then
      os.remove(opts.output_file)
    end
    vim.notify("Ntex Studio isn't built: ~/.config/texnvim/studio/build.sh", vim.log.levels.WARN)
    return false
  end
  -- detached, no pipes: it outlives this ntex, and never writes into a closed one
  local ok, err = pcall(vim.system, argv, { detach = true, stdin = false, stdout = false, stderr = false })
  if not ok then
    vim.notify("Ntex Studio didn't start: " .. tostring(err), vim.log.levels.ERROR)
    return false
  end
  return true
end

--- Space n / :SnippetStudio: open a page.
function M.open(page)
  return M.launch({ page = page or "snippet", ft = filetype() })
end

--- Space n in visual mode: the selection becomes the new snippet's output.
function M.open_with_selection()
  local text = selection_text()
  local context = context_at_selection()
  local ft = filetype()
  vim.api.nvim_feedkeys(vim.keycode("<Esc>"), "nx", false)
  local opts = { page = "snippet", context = context, ft = ft }
  if text and text ~= "" then
    opts.output_file = write_temp(text)
  end
  return M.launch(opts)
end

function M.setup()
  vim.api.nvim_create_user_command("SnippetStudio", function(o)
    local page = o.args ~= "" and o.args or "snippet"
    if not vim.tbl_contains(M.PAGES, page) then
      vim.notify("SnippetStudio: the pages are snippet, block and inbox", vim.log.levels.ERROR)
      return
    end
    M.open(page)
  end, {
    nargs = "?",
    complete = function()
      return M.PAGES
    end,
    desc = "Open Ntex Studio (make a snippet, a block; the inbox)",
  })
  vim.keymap.set("n", "<leader>n", function()
    M.open("snippet")
  end, { desc = "Ntex Studio: make a new snippet" })
  vim.keymap.set("x", "<leader>n", function()
    M.open_with_selection()
  end, { desc = "Ntex Studio: make a snippet from the selection" })
  vim.keymap.set("n", "<leader>N", function()
    M.open("inbox")
  end, { desc = "Ntex Studio: the inbox (snippets waiting to be sorted)" })
end

return M
