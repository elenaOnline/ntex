-- Standard editor themes, for comparing against the ntex palettes (Ntex
-- Studio's Look page, :NtexLook palette NAME). Loaded only when picked:
-- lazy.nvim loads a colorscheme plugin on :colorscheme. The palette names
-- and how ntex's structure marks sit on top: lua/config/palettes.lua.
return {
  { "Mofiqul/vscode.nvim", lazy = true }, -- VS Code Light+ / Dark+ (LaTeX Workshop's default look)
  { "projekt0n/github-nvim-theme", name = "github-theme", lazy = true }, -- GitHub Light
  { "olimorris/onedarkpro.nvim", lazy = true }, -- One Light (Atom's, the base of many)
  { "maxmx03/solarized.nvim", lazy = true }, -- Solarized Light / Dark
  { "catppuccin/nvim", name = "catppuccin", lazy = true }, -- Catppuccin Latte / Mocha
  { "folke/tokyonight.nvim", lazy = true }, -- Tokyo Night
  { "ellisonleao/gruvbox.nvim", lazy = true }, -- Gruvbox Dark
  { "yorik1984/newpaper.nvim", lazy = true }, -- Newpaper Light / Dark (its own LaTeX highlighting)
}
