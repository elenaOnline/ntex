-- A few keys that make Neovim friendlier for someone who lives in insert mode.
local map = vim.keymap.set

-- Save with Ctrl-s from any mode (and stay in insert mode if you were there)
map({ "n", "v" }, "<C-s>", "<cmd>write<cr>", { desc = "Save" })
map("i", "<C-s>", "<cmd>write<cr>", { desc = "Save" })
-- Undo without leaving insert mode (Ctrl-z). Redo: Esc then Ctrl-r.
map("i", "<C-z>", "<C-o>u", { desc = "Undo" })
map("n", "<C-z>", "u", { desc = "Undo (instead of suspending Neovim)" })
-- Esc also clears search highlighting
map("n", "<Esc>", "<cmd>nohlsearch<cr><Esc>")
-- Move by screen line on wrapped text
-- "x" = visual only: in select mode (a snippet placeholder) j/k must type
map({ "n", "x" }, "j", "v:count == 0 ? 'gj' : 'j'", { expr = true })
map({ "n", "x" }, "k", "v:count == 0 ? 'gk' : 'k'", { expr = true })
map("i", "<Down>", "<C-o>gj")
map("i", "<Up>", "<C-o>gk")

-- <leader> is Space
map("n", "<leader>w", "<cmd>write<cr>", { desc = "Save" })
map("n", "<leader>q", "<cmd>quit<cr>", { desc = "Quit window" })
map("n", "<leader>s", "<cmd>Scratch<cr>", { desc = "New scratch .tex" })
map("n", "<leader>e", "<cmd>SnippetEdit<cr>", { desc = "Pick a snippet file to edit" })
map("n", "<leader>r", "<cmd>SnippetReport<cr>", { desc = "Snippet report" })
map("n", "<leader>c", function() require("snips.conceal").toggle() end, { desc = "Toggle conceal" })
-- Space z (Harper's fixes), Space d / Space x (errors), Space a (Copilot) and
-- the finder keys (Space f F o / b) are in lua/config/ide.lua and lua/ide/.
map("n", "<leader>?", "<cmd>Reference<cr>", { desc = "Open the ntex reference page" })
