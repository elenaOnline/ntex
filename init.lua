-- texnvim: Neovim for fast live LaTeX, with Castel-style LuaSnip snippets.
-- Launched with `ntex` (NVIM_APPNAME=texnvim). See README.md in this folder.

vim.g.mapleader = " " -- <leader> is Space
vim.g.maplocalleader = "\\" -- VimTeX keys stay \ll (compile), \lv (view), …

-- TEXNVIM_NO_VIEWER=1 (the tests set it): no PDF viewer, no Skim, no browser,
-- no `open`, ever. Each viewer call site checks it too; this is the safety net
-- for anything else that would call vim.ui.open (e.g. an LSP showDocument).
if vim.env.TEXNVIM_NO_VIEWER == "1" then
  vim.ui.open = function(path)
    _G.texnvim_blocked_opens = (_G.texnvim_blocked_opens or 0) + 1
    return nil, "TEXNVIM_NO_VIEWER=1: not opening " .. tostring(path)
  end
end

-- TEXNVIM_NO_LSP=1 (Ntex Studio's headless bridge sets it): no language
-- servers at all, so texlab, harper-ls and copilot-language-server never start
-- (and can't be left orphaned). lua/config/lsp.lua and lua/ide/harper.lua /
-- copilot.lua check it too; this is the safety net for anything else.
if vim.env.TEXNVIM_NO_LSP == "1" then
  vim.lsp.enable = function() end
  vim.lsp.start = function()
    return nil
  end
end

require("config.options")
require("config.lazy") -- plugins (lazy.nvim, pinned by lazy-lock.json)
require("config.lsp") -- texlab via vim.lsp.config / vim.lsp.enable
require("config.ide") -- errors inline, Harper, Copilot, file finder (lua/ide/)
require("config.keymaps")
require("config.macos_keys").setup() -- Option-Left/Right/Backspace like macOS
require("config.commands") -- :Scratch, new-file template
require("config.livepdf") -- save + compile when you pause typing (prefs.lua)
require("config.pdfview").setup() -- \\lv: PDF in a cmux pane (prefs.lua pdf_viewer)
require("snips").setup() -- snippets (snippets/tex/*.lua), maths typing features, conceal
require("config.reference").setup() -- :Reference / Space ?: the reference page (reference/)
require("config.studio").setup() -- :SnippetStudio / Space n, Space N: Ntex Studio (studio/)
require("config.figures").setup() -- :Figure / Space g: Ntex Figures (studio/figures/FIGURES.md)
