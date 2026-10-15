-- .fig files (Ntex Figures shorthand; lua/config/figures.lua, studio/figures/FIGURES.md)
vim.bo.commentstring = "% %s"
vim.bo.comments = ":%"
vim.bo.expandtab = true
vim.bo.shiftwidth = 2
vim.bo.tabstop = 2
-- grid blocks are ASCII art: never re-wrap them
vim.bo.textwidth = 0
vim.wo[0][0].wrap = false
vim.b.undo_ftplugin = "setlocal commentstring< comments< expandtab< shiftwidth< tabstop< textwidth< wrap<"
