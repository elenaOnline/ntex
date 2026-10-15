-- Harper: grammar and spelling for .tex and .md (harper-ls, `brew install
-- harper`). Runs locally; nothing leaves the Mac.
--
-- Notes show as quiet underlines (no sign, no end-of-line text); ]d jumps to
-- the next one and shows it, Space z offers Harper's fixes for the word under
-- the cursor (replace, add to your dictionary, ignore), Space x lists them all.
-- Vim's own spell check is switched off where Harper runs, so a word is never
-- underlined twice; it comes back if harper-ls isn't installed or stops.
--
-- Maths: Harper skips $…$ and maths environments (equation, align, …) in
-- .tex, and $…$ / $$…$$ in .md, but it reads \[…\], $$…$$ and \(…\) in .tex as
-- prose and flags the f in f(x). So every note inside maths is dropped here
-- (the same maths detection the snippets use: VimTeX's syntax in .tex, the
-- $-scanner in .md); prose inside \text{…} is still checked.
--
-- prefs.lua: harper = true/false, harper_dialect = "American" (or "British",
-- "Australian", "Canadian", "Indian").
local prefs = require("config.prefs")

local M = {}
M.dropped = 0 -- notes dropped because they were inside maths (tests read this)

local FILETYPES = { "tex", "markdown" }

function M.enabled()
  -- TEXNVIM_NO_LSP=1: no language servers (Ntex Studio's headless bridge)
  return prefs.harper ~= false and vim.env.TEXNVIM_NO_LSP ~= "1" and vim.fn.executable("harper-ls") == 1
end

local function in_math(buf, lnum, col)
  local ok, res = pcall(function()
    local ctx = require("snips.context")
    local c
    vim.api.nvim_buf_call(buf, function()
      -- the context sees the character *before* col; look at the first
      -- character of the flagged word
      c = ctx.get_at(buf, lnum, col + 1)
    end)
    return c and ctx.in_math(c.mode) and not c.mode.textEnv
  end)
  return ok and res
end

--- drop notes inside maths, then hand over to Neovim's normal handler
local function on_publish(err, result, ctx)
  if result and result.diagnostics and #result.diagnostics > 0 then
    local buf = vim.uri_to_bufnr(result.uri)
    if vim.api.nvim_buf_is_loaded(buf) then
      local client = vim.lsp.get_client_by_id(ctx.client_id)
      local enc = client and client.offset_encoding or "utf-16"
      local keep = {}
      for _, d in ipairs(result.diagnostics) do
        local row = d.range.start.line
        local line = vim.api.nvim_buf_get_lines(buf, row, row + 1, false)[1] or ""
        local okc, col = pcall(vim.str_byteindex, line, enc, d.range.start.character, false)
        local drop = okc and in_math(buf, row, col)
        if okc and not drop and d.range["end"].line == row then
          -- "for all $x$, and": with the maths removed Harper sees "for all ,"
          -- and flags the space before $. A flagged run of spaces next to
          -- maths is that artefact.
          local oke, ecol = pcall(vim.str_byteindex, line, enc, d.range["end"].character, false)
          if oke and line:sub(col + 1, ecol):match("^%s+$") then
            drop = in_math(buf, row, ecol) or (col > 0 and in_math(buf, row, col - 1))
          end
        end
        if not drop then
          keep[#keep + 1] = d
        else
          M.dropped = M.dropped + 1
        end
      end
      result.diagnostics = keep
    end
  end
  return vim.lsp.diagnostic.on_publish_diagnostics(err, result, ctx)
end

local function set_spell(buf, on)
  for _, win in ipairs(vim.fn.win_findbuf(buf)) do
    vim.wo[win][0].spell = on
  end
end

function M.setup()
  if not M.enabled() then
    return
  end
  vim.lsp.config("harper_ls", {
    cmd = { "harper-ls", "--stdio" },
    filetypes = FILETYPES,
    root_markers = { ".harper-dictionary.txt", ".git" },
    handlers = { ["textDocument/publishDiagnostics"] = on_publish },
    settings = {
      ["harper-ls"] = {
        dialect = prefs.harper_dialect or "American",
        diagnosticSeverity = "hint", -- quiet underline; errors and warnings stay the loud ones
        isolateEnglish = false,
        linters = {
          -- "Let $X$ be compact": the maths is invisible to Harper, which then
          -- reads "Let be" and suggests "let's"
          LetsConfusion = false,
        },
        markdown = { IgnoreLinkTitle = true },
      },
    },
  })
  vim.lsp.enable("harper_ls")

  local grp = vim.api.nvim_create_augroup("texnvim_harper", { clear = true })
  -- spell off as soon as the file opens (no flash of double underlines);
  -- options.lua's FileType autocmd ran first and turned it on
  vim.api.nvim_create_autocmd("FileType", {
    group = grp,
    pattern = FILETYPES,
    callback = function(ev)
      vim.schedule(function()
        if vim.api.nvim_buf_is_valid(ev.buf) then
          set_spell(ev.buf, false)
        end
      end)
    end,
  })
  vim.api.nvim_create_autocmd("BufWinEnter", {
    group = grp,
    callback = function(ev)
      if vim.tbl_contains(FILETYPES, vim.bo[ev.buf].filetype) then
        vim.wo.spell = false
      end
    end,
  })
  -- harper-ls stopped or crashed: Vim's spell check comes back
  vim.api.nvim_create_autocmd("LspDetach", {
    group = grp,
    callback = function(ev)
      local client = vim.lsp.get_client_by_id(ev.data.client_id)
      if client and client.name == "harper_ls" and vim.tbl_contains(FILETYPES, vim.bo[ev.buf].filetype) then
        vim.schedule(function()
          if vim.api.nvim_buf_is_valid(ev.buf) and #vim.lsp.get_clients({ bufnr = ev.buf, name = "harper_ls" }) == 0 then
            set_spell(ev.buf, true)
          end
        end)
      end
    end,
  })
end

--- Space z: Harper's fixes for the note under the cursor; without Harper,
--- Vim's first spelling suggestion (the old Space z)
function M.fix()
  local buf = vim.api.nvim_get_current_buf()
  local client = vim.lsp.get_clients({ bufnr = buf, name = "harper_ls" })[1]
  if not client then
    vim.cmd("normal! 1z=")
    return
  end
  local row = vim.api.nvim_win_get_cursor(0)[1] - 1
  local col = vim.api.nvim_win_get_cursor(0)[2]
  local here = vim.tbl_filter(function(d)
    return d.lnum == row and d.col <= col and col < math.max(d.end_col or d.col + 1, d.col + 1)
  end, vim.diagnostic.get(buf, { namespace = vim.lsp.diagnostic.get_namespace(client.id) }))
  if #here == 0 then
    vim.notify("Harper: nothing to fix under the cursor (]d jumps to the next note)", vim.log.levels.INFO)
    return
  end
  vim.lsp.buf.code_action({
    filter = function(_, client_id)
      return client_id == client.id
    end,
  })
end

-- harper-ls doesn't exit when Neovim closes its pipe, so every quit left an
-- orphan process behind. Stop all language servers (force) on the way out.
vim.api.nvim_create_autocmd("VimLeavePre", {
  group = vim.api.nvim_create_augroup("texnvim_lsp_exit", { clear = true }),
  callback = function()
    for _, client in ipairs(vim.lsp.get_clients()) do
      pcall(function() client:stop(true) end)
    end
  end,
})

return M
