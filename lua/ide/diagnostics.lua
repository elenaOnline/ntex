-- How errors, warnings and grammar notes look (every source: tectonic compile
-- errors, texlab while you type, Harper).
--
--   underline        under the exact text (errors red, warnings amber, grammar
--                    and spelling a quiet blue)
--   sign             E / W in the line-number column, worst first. The sign
--                    takes the number's place, so the text never shifts.
--   end-of-line note the message, shortened, after the line (errors and
--                    warnings only; grammar notes stay as underlines)
--   Space d          the line's messages in full, in a small float
--   ]d / [d          next / previous one (the float opens on arrival)
--   Space x          all of them in a list (lua/ide/finder.lua)
--
-- Why the end-of-line note and not virtual_lines (the full message under the
-- cursor line): lines wrap here and a LaTeX paragraph is often one long line,
-- so a block of text appearing under the cursor line pushes the rest of the
-- screen down every time the cursor lands on a flagged line, and with Harper
-- that is most lines. The end-of-line note never moves text; Space d (or ]d)
-- shows the whole message.
--
-- Grammar / texlab notes update when you leave insert mode (no underline
-- flickering under the word you are typing); compile errors show at once
-- (lua/ide/compile_errors.lua).
local M = {}

local S = vim.diagnostic.severity

local function short(d)
  local msg = vim.split(d.message, "\n", { plain = true })[1]
  if vim.fn.strchars(msg) > 60 then
    msg = vim.fn.strcharpart(msg, 0, 59) .. "…"
  end
  return msg
end

function M.config()
  return {
    underline = true,
    severity_sort = true,
    update_in_insert = false,
    signs = {
      severity = { min = S.WARN },
      text = { [S.ERROR] = "E", [S.WARN] = "W", [S.INFO] = "I", [S.HINT] = "H" },
    },
    virtual_text = {
      severity = { min = S.WARN },
      spacing = 2,
      prefix = "●",
      source = false,
      format = short,
    },
    virtual_lines = false,
    float = {
      border = "rounded",
      source = "if_many",
      header = "",
      scope = "line",
      max_width = 80,
    },
    jump = {
      -- ]d / [d: show the message where you land (closes when you move)
      on_jump = function(d, bufnr)
        if d then
          vim.diagnostic.open_float({ bufnr = bufnr, scope = "cursor", focus = false })
        end
      end,
    },
  }
end

function M.setup()
  vim.diagnostic.config(M.config())
  -- signs go in the number column: with relative numbers on, a separate sign
  -- column would appear and shift the text whenever an error comes and goes
  vim.o.signcolumn = "number"
  local map = vim.keymap.set
  map("n", "<leader>d", function()
    vim.diagnostic.open_float({ scope = "line" })
  end, { desc = "Show this line's errors / warnings / grammar notes in full" })
end

return M
