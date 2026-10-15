-- texlab (LaTeX language server): completion of \ref, \cite, \begin, your own
-- \newcommand macros, labels across files; diagnostics; go-to-definition.
-- Uses Neovim's built-in LSP client (vim.lsp.config / vim.lsp.enable).
vim.lsp.config("texlab", {
  cmd = { "texlab" },
  filetypes = { "tex", "plaintex", "bib" },
  root_markers = { ".latexmkrc", ".texlabroot", "texlabroot", "Tectonic.toml", ".git" },
  settings = {
    texlab = {
      -- building/viewing is VimTeX's job (tectonic + Skim)
      build = { onSave = false, forwardSearchAfter = false },
      chktex = { onOpenAndSave = false, onEdit = false },
      diagnosticsDelay = 300,
      -- "Unused label" would underline every \label you haven't \ref'd yet
      diagnostics = { ignoredPatterns = { "^Unused label" } },
      formatterLineLength = 0,
    },
  },
  capabilities = (function()
    local ok, blink = pcall(require, "blink.cmp")
    return ok and blink.get_lsp_capabilities() or nil
  end)(),
})
-- TEXNVIM_NO_LSP=1 (Ntex Studio's headless bridge): no language servers
if vim.env.TEXNVIM_NO_LSP ~= "1" then
  vim.lsp.enable("texlab")
end
-- How diagnostics look (texlab's, tectonic's, Harper's): lua/ide/diagnostics.lua
