-- :SnippetReport (Space r) — what loaded, file by file, and any load errors.
local M = {}

local function short(p)
  return p and vim.fn.fnamemodify(p, ":~") or "?"
end

function M.lines()
  local snips = require("snips")
  local list = snips.list()
  local out = {}
  local function add(s)
    out[#out + 1] = s
  end
  add("# Snippet report")
  add("")
  add("folder: " .. short(snips.dir()) .. "   (Space e opens a file; saving reloads it)")
  local errs = vim.tbl_keys(snips.errors)
  table.sort(errs)
  if #errs > 0 then
    add("")
    add("LOAD ERRORS (the previous version of each file is still active, if there was one):")
    for _, p in ipairs(errs) do
      add("  " .. vim.fs.basename(p) .. ":")
      for _, l in ipairs(vim.split(tostring(snips.errors[p]), "\n")) do
        add("    " .. l)
      end
    end
  end
  local by_file, files = {}, {}
  local totals = { auto = 0, tab = 0, visual = 0 }
  for _, m in ipairs(list) do
    if not by_file[m.file] then
      by_file[m.file] = {}
      files[#files + 1] = m.file
    end
    table.insert(by_file[m.file], m)
    totals[m.how] = (totals[m.how] or 0) + 1
  end
  add("")
  add(("loaded: %d snippets (%d by themselves, %d on Tab, %d visual) from %d files"):format(#list, totals.auto, totals.tab, totals.visual, #files))
  for _, f in ipairs(files) do
    local ms = by_file[f]
    add("")
    add(("## %s — %s (%d)"):format(f, ms[1].group or "", #ms))
    for _, m in ipairs(ms) do
      local body = m.body and m.body:gsub("\n", "⏎") or "(computed)"
      add(("  %4s  %-14s %-28s %-6s %-8s %s"):format(m.line or "?", m.trig, body:sub(1, 28), m.how, m.where, m.origin))
    end
  end
  return out
end

function M.show()
  local lines = M.lines()
  vim.cmd("botright new")
  local buf = vim.api.nvim_get_current_buf()
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
  vim.bo[buf].buftype = "nofile"
  vim.bo[buf].bufhidden = "wipe"
  vim.bo[buf].modifiable = false
  vim.bo[buf].filetype = "markdown"
  vim.api.nvim_buf_set_name(buf, "Snippet report")
  vim.keymap.set("n", "q", "<cmd>close<cr>", { buffer = buf })
end

return M
