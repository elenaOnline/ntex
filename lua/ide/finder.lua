-- The file finder: snacks.picker (snacks.nvim is already here for images).
--
--   Space f   files in this file's folder (no file yet: the working folder)
--   Space F   files anywhere under ~ (not ~/Library, .git, node_modules)
--   Space o   recent files
--   Space /   search the text of the files in this file's folder (ripgrep)
--   Space b   open buffers
--   Space x   all errors / warnings / grammar notes (lua/ide/diagnostics.lua)
--
-- Inside a picker: type to filter, Enter opens, Esc closes, Ctrl-v / Ctrl-s
-- open in a split, Alt-p shows/hides the preview.
-- Needs fd and rg (brew install fd ripgrep); without them snacks uses `find`
-- and `grep`, which are slower.
local M = {}

-- Small floating list at the top (VS Code's Cmd-P shape): no preview, no
-- icons, so it opens at once and stays out of the way. Text search and the
-- error list use a bottom panel with a preview instead.
function M.picker_opts()
  return {
    enabled = true,
    ui_select = true, -- vim.ui.select (Space z suggestions, Space e) uses the same list
    layout = { preset = "vscode" },
    icons = { files = { enabled = false }, git = { enabled = false } },
    formatters = { file = { filename_first = true, truncate = "left" } },
    sources = {
      grep = { layout = { preset = "ivy" } },
      diagnostics = { layout = { preset = "ivy" }, filter = { cwd = false } },
    },
  }
end

--- the current file's folder, or the working folder for an unnamed buffer
function M.here()
  local name = vim.api.nvim_buf_get_name(0)
  if name ~= "" and vim.bo.buftype == "" then
    local dir = vim.fs.dirname(name)
    if vim.uv.fs_stat(dir) then
      return dir
    end
  end
  return vim.uv.cwd()
end

local function tilde(p)
  return (p:gsub("^" .. vim.pesc(vim.env.HOME), "~"))
end

function M.files_here()
  local dir = M.here()
  Snacks.picker.files({ cwd = dir, title = "Files in " .. tilde(dir) })
end

function M.files_home()
  Snacks.picker.files({
    cwd = vim.env.HOME,
    title = "Files under ~",
    exclude = { "Library", ".git", "node_modules" },
  })
end

function M.recent()
  Snacks.picker.recent({ title = "Recent files" })
end

function M.grep_here()
  local dir = M.here()
  Snacks.picker.grep({ cwd = dir, title = "Search in " .. tilde(dir) })
end

function M.buffers()
  Snacks.picker.buffers({ title = "Open files" })
end

function M.diagnostics()
  if #vim.diagnostic.get() == 0 then
    vim.notify("No errors, warnings or grammar notes", vim.log.levels.INFO)
    return
  end
  Snacks.picker.diagnostics({ title = "Errors, warnings, grammar" })
end

function M.setup()
  local map = vim.keymap.set
  map("n", "<leader>f", M.files_here, { desc = "Find a file in this file's folder" })
  map("n", "<leader>F", M.files_home, { desc = "Find a file anywhere under ~" })
  map("n", "<leader>o", M.recent, { desc = "Recent files" })
  map("n", "<leader>/", M.grep_here, { desc = "Search text in this file's folder" })
  map("n", "<leader>b", M.buffers, { desc = "Open files (buffers)" })
  map("n", "<leader>x", M.diagnostics, { desc = "All errors, warnings and grammar notes (picker)" })
end

return M
