local o = vim.opt

-- Writing-friendly display
o.wrap = true
o.linebreak = true -- wrap at word boundaries, not mid-word
o.breakindent = true
o.smoothscroll = true
o.number = true          -- show the current line's real number
o.relativenumber = true  -- other lines show their distance from the cursor
o.signcolumn = "no"
o.cursorline = true
o.scrolloff = 4
o.laststatus = 2
o.showmode = true -- always show -- INSERT -- etc.
o.ruler = true
o.termguicolors = true
o.mouse = "a"
o.clipboard = "unnamedplus" -- yank/paste with the macOS clipboard
o.undofile = true -- undo survives closing the file
o.updatetime = 300
o.timeoutlen = 600
o.splitright = true
o.splitbelow = true
o.confirm = true -- ask instead of failing on :q with unsaved changes
o.ignorecase = true
o.smartcase = true
o.expandtab = true
o.shiftwidth = 2
o.tabstop = 2
o.completeopt = { "menu", "menuone", "noselect" }
o.shortmess:append("cI")

-- Selecting with the mouse or Shift+arrows gives *select mode*, where typing
-- replaces the selection (like any normal editor). Latex Suite's visual
-- snippets hook into this: select, then type ( [ { U O B C K S.
o.selectmode = { "mouse", "key" }
o.keymodel = { "startsel", "stopsel" }

-- Spell check prose only. VimTeX's syntax marks math/commands as no-spell,
-- and the markdown syntax does the same for code.
o.spelllang = { "en_gb", "en_us" }

-- Concealment: pretty symbols everywhere except the line you are on.
o.conceallevel = 2
o.concealcursor = ""

vim.api.nvim_create_autocmd("FileType", {
  pattern = { "tex", "plaintex", "markdown" },
  callback = function()
    vim.opt_local.spell = true
    vim.opt_local.conceallevel = 2
    vim.opt_local.concealcursor = ""
    vim.opt_local.textwidth = 0
  end,
})

-- Colours come from lua/config/theme.lua (Modus), loaded with the plugins.
