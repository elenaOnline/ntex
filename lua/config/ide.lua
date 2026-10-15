-- IDE features: errors where they happen, grammar (Harper), Copilot ghost
-- text, a file finder. Each part lives in lua/ide/ and has its switch in
-- prefs.lua; README "IDE features" explains them.
--
--   lua/ide/diagnostics.lua     how errors / warnings / notes look; Space d
--   lua/ide/compile_errors.lua  tectonic's log → diagnostics after every compile
--   lua/ide/harper.lua          harper-ls for .tex / .md; Space z
--   lua/ide/copilot.lua         Copilot ghost text; Alt-l, Alt-] / Alt-[, Space a
--   lua/ide/finder.lua          snacks.picker; Space f F o / b x
--
-- (Options live here, not in options.lua: signcolumn is set in diagnostics.lua.)
require("ide.diagnostics").setup()
require("ide.compile_errors").setup()
require("ide.harper").setup()
require("ide.copilot").setup()
require("ide.copilot").setup_keys()
require("ide.finder").setup()

vim.keymap.set("n", "<leader>z", function()
  require("ide.harper").fix()
end, { desc = "Fix the word under the cursor (Harper's suggestions; Vim's spelling without Harper)" })
