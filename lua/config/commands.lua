-- :Scratch and the template for new .tex files.
local api = vim.api

-- Where :Scratch puts files. Change it here, or per call: :Scratch ~/somewhere
vim.g.scratch_dir = vim.g.scratch_dir or vim.fs.joinpath(vim.env.HOME, "scratch")

local template_path = vim.fs.joinpath(vim.fn.stdpath("config"), "templates", "article.tex")

--- Fill an empty .tex buffer from templates/article.tex
local function apply_template(buf)
  if vim.b[buf].texnvim_template_done then
    return
  end
  vim.b[buf].texnvim_template_done = true
  local lines = api.nvim_buf_get_lines(buf, 0, -1, false)
  if #lines > 1 or (lines[1] or "") ~= "" then
    return
  end
  local ok, tpl = pcall(vim.fn.readfile, template_path)
  if not ok then
    return
  end
  local name = vim.fn.fnamemodify(api.nvim_buf_get_name(buf), ":t:r")
  local cursor = nil
  for i, l in ipairs(tpl) do
    l = l:gsub("{{TITLE}}", (name:gsub("[_-]", " ")))
    l = l:gsub("{{DATE}}", os.date("%Y-%m-%d"))
    if l:find("{{CURSOR}}", 1, true) then
      cursor = i
      l = l:gsub("{{CURSOR}}", "")
    end
    tpl[i] = l
  end
  api.nvim_buf_set_lines(buf, 0, -1, false, tpl)
  if cursor and api.nvim_get_current_buf() == buf then
    api.nvim_win_set_cursor(0, { cursor, 0 })
  end
  vim.bo[buf].modified = true
end

api.nvim_create_autocmd({ "BufNewFile", "BufReadPost" }, {
  group = api.nvim_create_augroup("texnvim_template", { clear = true }),
  pattern = "*.tex",
  callback = function(ev)
    apply_template(ev.buf)
  end,
})

--- :Scratch [dir]  — a fresh, timestamped .tex file (default dir: ~/scratch)
api.nvim_create_user_command("Scratch", function(opts)
  local dir = opts.args ~= "" and vim.fn.expand(opts.args) or vim.g.scratch_dir
  vim.fn.mkdir(dir, "p")
  local name = os.date("scratch-%Y-%m-%d-%H%M%S") .. ".tex"
  vim.cmd.edit(vim.fn.fnameescape(vim.fs.joinpath(dir, name)))
  vim.cmd("startinsert")
end, { nargs = "?", complete = "dir", desc = "Open a new scratch .tex file" })

-- Saving a file into a folder that doesn't exist yet (e.g. `ntex ~/new/dir/x.tex`)
-- creates the folder instead of failing with E212.
vim.api.nvim_create_autocmd("BufWritePre", {
  group = vim.api.nvim_create_augroup("texnvim_mkdir_on_save", { clear = true }),
  callback = function(ev)
    if ev.match:match("^%w+://") then
      return
    end
    local dir = vim.fn.fnamemodify(ev.match, ":p:h")
    if vim.fn.isdirectory(dir) == 0 then
      vim.fn.mkdir(dir, "p")
    end
  end,
})
