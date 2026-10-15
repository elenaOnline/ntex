-- Colours: Modus (operandi light / vivendi dark, WCAG AAA ≥ 7:1), following
-- the terminal's light/dark. Neovim asks the terminal at startup and when it
-- reports a theme change, which sets 'background'; this re-applies the theme.
--
-- The one LaTeX-specific fix: concealed symbols (α, ℝ, ∑, subscripts) are
-- drawn with the `Conceal` group, which most themes paint pale grey — they'd
-- look fainter than the source they replace. Here they use the math colour.
local M = {}

function M.setup()
  require("modus-themes").setup({
    style = "auto", -- operandi when background=light, vivendi when dark
    styles = { comments = { italic = true }, keywords = {} },
    on_highlights = function(hl, c)
      local math = c.blue -- #0031a9 on white, #2fafff on black: both AAA
      hl.Conceal = { fg = math, bg = "NONE" }
      hl.texMathZone = { fg = math }
      hl.texMathZoneX = { fg = math }
      hl.texMathZoneXX = { fg = math }
      hl.texMathZoneTI = { fg = math }
      hl.texMathZoneTD = { fg = math }
      hl.texMathDelimZone = { fg = c.fg_dim }
      hl.texMathSuperSub = { fg = math }
      hl.texMathCmd = { fg = c.magenta_cooler }
      hl.texMathSymbol = { fg = math }
      hl.texMathOper = { fg = c.fg_main }
      hl.texCmd = { fg = c.magenta_cooler }
      hl.texEnvArgName = { fg = c.green }
      -- Copilot's ghost text: dim and italic, clearly not yet typed
      hl.ComplHint = { fg = c.fg_dim, italic = true }
      hl.ComplHintMore = { fg = c.fg_dim }
      -- markdown math ($…$) in .md files uses treesitter groups
      hl["@markup.math"] = { fg = math }
    end,
  })
  vim.cmd.colorscheme("modus")

  vim.api.nvim_create_autocmd("OptionSet", {
    group = vim.api.nvim_create_augroup("texnvim_theme", { clear = true }),
    pattern = "background",
    callback = function()
      vim.cmd.colorscheme("modus")
    end,
  })
end

return M
