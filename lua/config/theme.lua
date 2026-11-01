-- Colors. A palette (lua/config/palettes.lua, picked in look.json: see
-- lua/config/look.lua and Ntex Studio's Look page) drawn on top of Modus, so
-- every UI group Modus defines (menus, floats, diagnostics, the finder) keeps
-- its AAA contrast and only the page, the syntax and the bars change. Light
-- palettes sit on Modus Operandi plus the Alabaster page below; dark ones
-- ("code-dark") on Modus Vivendi, and switch 'background' to dark themselves.
--
--   "slate"      (default, chosen 2026-10-18) one cool blue-gray for all the
--                markup, near-black indigo math, ink only for printed text
--   "code"       strong code-theme roles: blue math, magenta math commands,
--                red markup, gray italic comments
--   "code-dark"  Code on a dark page
--   "alabaster"  the look before (2026-10-15): tonsky's minimal highlighting
--
-- Standard themes, to compare against (palettes.lua M.standard; plugins in
-- lua/plugins/themes.lua): their own LaTeX colors, ntex's structure marks on
-- top. vscode-light/-dark, github-light, one-light, solarized-light/-dark,
-- catppuccin-latte/-mocha, tokyonight, gruvbox-dark, newpaper-light/-dark.
--
-- :NtexTheme NAME tries one in this ntex only (also the older "commutator",
-- the palette of the group theory game, and plain "modus").
--
-- Kept from before: concealed symbols (α, ℝ, ∑, subscripts) use the `Conceal`
-- group, which most themes paint pale grey, so here they take the math color.
--
-- The Ntex* groups at the end color the status bar and winbar
-- (lua/config/statusline.lua), in every palette.
local M = {}

M.palettes = {
  alabaster = {
    bg = "#ffffff", -- pure white paper; matches the terminal and cmux chrome
    fg = "#000000",
    math = "#325cc0", -- Alabaster "definition" blue: math
    mathcmd = "#325cc0",
    cmd = "#000000", -- commands stay black
    env = "#7a3e9d", -- Alabaster "constant" purple: environment names
    comment = "#aa3731", -- comments: red, not faded
    string = "#448c27",
    punct = "#777777",
    dim = "#5c5c5c",
    cursorline = "#ebebeb",
    cursorlinenr = "#e3e3e3",
    linenr = "#9a9a9a",
    visual = "#bfdbfe",
    status = "#dddddd",
    status_nc = "#eeeeee",
    ghost = "#8a8a8a",
    ok = "#2f6b16", -- darker than Alabaster's green so it reads on the grey bar
    busy = "#8a5a00",
    err = "#aa3731",
    warn = "#8a5a00",
    hint = "#0d6e8f",
  },
  -- From Commutator's :root tokens (site/…/commutator/assets/index-*.css):
  -- --navy #1b2340, --navy-2 #545b73, --navy-3 #8a90a3, --grid #d5d8e0,
  -- the *-text role colors (AA on white) and #e3e5ec, its panel border.
  commutator = {
    bg = "#ffffff", -- --paper
    fg = "#1b2340", -- --navy: ink
    math = "#1f4fd1", -- --structure-text
    mathcmd = "#5528c2", -- --scope: \sum, \frac … (operators bind a scope)
    cmd = "#545b73", -- --navy-2: text commands recede behind the prose
    env = "#5528c2", -- --scope: an environment is a scope
    comment = "#a61c5a", -- --inverse-text
    string = "#11806c", -- --equation-text
    punct = "#8a90a3", -- --navy-3: $ & { } recede like the game's grid
    dim = "#545b73",
    cursorline = "#eef0f5",
    cursorlinenr = "#e3e5ec",
    linenr = "#8a90a3",
    visual = "#dbe5ff", -- a tint of --structure
    status = "#e3e5ec",
    status_nc = "#f1f2f6",
    ghost = "#8a90a3",
    ok = "#0e6e5d", -- --equation-text, deepened for the grey bar
    busy = "#8a5a00", -- --identity-text
    err = "#c2410c", -- --contradiction-text
    warn = "#8a5a00", -- --identity-text
    hint = "#1f4fd1", -- --structure-text
  },
}

local function palette_name()
  local name = M.override or require("config.look").get().palette
  local P = require("config.palettes")
  if P.palettes[name] or P.standard[name] or M.palettes[name] or name == "modus" then
    return name
  end
  return "slate"
end

-- the palettes.lua palette in use (nil for a standard theme and the older commutator / modus)
local function editor_palette()
  return require("config.palettes").palettes[palette_name()]
end

-- the standard theme in use (palettes.lua M.standard), or nil
local function standard()
  return require("config.palettes").standard[palette_name()]
end

local function scheme()
  local std = standard()
  return std and std.scheme or "modus"
end

-- :colorscheme for the palette in use (a standard theme may need setting up first)
local function load_scheme()
  local std = standard()
  if std and std.before then
    std.before()
  end
  vim.cmd.colorscheme(scheme())
end

-- the 'background' the palette needs (nil: follow the terminal)
local function wanted_background()
  local p = editor_palette() or standard()
  return p and (p.dark and "dark" or "light") or nil
end

-- The terminal's own colors (cmux's padding and chrome) follow the page:
-- OSC 11 / 10 set its background and foreground while ntex runs; on white
-- paper (like the terminal's own), on suspend and on quit,
-- OSC 111 / 110 give it its configured colors back.
local term_set = false
local function terminal_reset()
  if term_set then
    term_set = false
    pcall(vim.api.nvim_ui_send, "\27]111\27\\\27]110\27\\")
  end
end
local function terminal()
  local np, std = editor_palette(), standard()
  local page = (np or std) and require("config.palettes").page() or nil
  -- white paper (her terminal's own colors): nothing to set
  if not page or (vim.o.background == "light" and page.bg == "#ffffff") then
    return terminal_reset()
  end
  term_set = true
  pcall(vim.api.nvim_ui_send, "\27]11;" .. page.bg .. "\27\\\27]10;" .. page.fg .. "\27\\")
end

local function paint(hl, p)
  local plain = { fg = p.fg }
  -- page and UI
  hl.Normal = { fg = p.fg, bg = p.bg }
  hl.NormalNC = { fg = p.fg, bg = p.bg }
  hl.EndOfBuffer = { fg = p.linenr, bg = p.bg }
  hl.SignColumn = { bg = p.bg }
  hl.FoldColumn = { fg = p.linenr, bg = p.bg }
  hl.LineNr = { fg = p.linenr, bg = p.bg }
  hl.LineNrAbove = { fg = p.linenr, bg = p.bg }
  hl.LineNrBelow = { fg = p.linenr, bg = p.bg }
  hl.CursorLine = { bg = p.cursorline }
  hl.CursorLineNr = { fg = p.fg, bg = p.cursorlinenr, bold = true }
  hl.Visual = { bg = p.visual }
  hl.StatusLine = { fg = p.fg, bg = p.status }
  hl.StatusLineNC = { fg = p.dim, bg = p.status_nc }
  hl.WinBar = { fg = p.dim, bg = p.bg }
  hl.WinBarNC = { fg = p.linenr, bg = p.bg }
  hl.WinSeparator = { fg = "#cfcfcf", bg = p.bg }
  hl.NonText = { fg = "#b5b5b5" }
  hl.MatchParen = { bg = "#d6e4ff", bold = true }

  -- generic syntax (other filetypes): four classes, as in Alabaster
  for _, g in ipairs({ "Statement", "Keyword", "Conditional", "Repeat", "Operator", "Identifier",
    "Type", "StorageClass", "Structure", "PreProc", "Include", "Define", "Macro", "Special",
    "Label", "Exception", "Tag", "Title", "@variable", "@keyword", "@keyword.function",
    "@keyword.return", "@keyword.operator", "@type", "@type.builtin", "@operator", "@property",
    "@variable.member", "@function.call", "@function.method.call", "@module", "@label", "@tag",
    "@markup.heading" }) do
    hl[g] = plain
  end
  for _, g in ipairs({ "Comment", "@comment", "SpecialComment" }) do
    hl[g] = { fg = p.comment }
  end
  for _, g in ipairs({ "String", "Character", "@string", "@character" }) do
    hl[g] = { fg = p.string }
  end
  for _, g in ipairs({ "Constant", "Number", "Boolean", "Float", "@constant", "@number",
    "@boolean", "@constant.builtin" }) do
    hl[g] = { fg = p.env }
  end
  for _, g in ipairs({ "Function", "@function", "@function.method", "@constructor" }) do
    hl[g] = { fg = p.math }
  end
  for _, g in ipairs({ "Delimiter", "@punctuation", "@punctuation.bracket",
    "@punctuation.delimiter", "@punctuation.special" }) do
    hl[g] = { fg = p.punct }
  end

  -- LaTeX (VimTeX syntax groups)
  hl.Conceal = { fg = p.math, bg = "NONE" }
  for _, g in ipairs({ "texMathZone", "texMathZoneX", "texMathZoneXX", "texMathZoneTI",
    "texMathZoneTD", "texMathZoneLI", "texMathZoneLD", "texMathZoneEnv", "texMathZoneEnsured",
    "texMathSuperSub", "texMathSymbol", "texMathOper", "texMathArg", "texMathEnvArgName",
    "@markup.math" }) do
    hl[g] = { fg = p.math }
  end
  hl.texMathCmd = { fg = p.mathcmd }
  hl.texMathCmdEnv = { fg = p.mathcmd }
  hl.texMathDelimZone = { fg = p.punct }
  hl.texMathDelim = { fg = p.math }
  hl.texCmd = { fg = p.cmd }
  hl.texCmdEnv = { fg = p.cmd }
  hl.texCmdPart = { fg = p.cmd }
  hl.texPartArgTitle = { fg = p.fg, bold = true }
  hl.texEnvArgName = { fg = p.env }
  hl.texDelim = { fg = p.punct }
  hl.texComment = { fg = p.comment }
  hl.texCommentTodo = { fg = p.comment, bold = true }
  hl.texOpt = { fg = p.dim }
  hl.texArg = plain
  hl.texRefArg = { fg = p.env }
  hl.texFileArg = { fg = p.string }
  hl.texSpecialChar = { fg = p.punct }

  -- Copilot's ghost text: dim and italic, clearly not yet typed
  hl.ComplHint = { fg = p.ghost, italic = true }
  hl.ComplHintMore = { fg = p.ghost }
end

-- status bar / winbar groups, for any palette
local function bars(hl, c, p)
  local sbg = p and p.status or c.bg_status_line_active or c.bg_dim
  local wbg = p and p.bg or c.bg_main
  local col = p and {
    ok = p.ok, busy = p.busy, err = p.err, warn = p.warn, hint = p.hint, dim = p.dim, math = p.math, sep = "#b0b0b0",
  } or {
    ok = c.green, busy = c.yellow_warmer or c.yellow, err = c.red, warn = c.yellow_warmer or c.yellow,
    hint = c.cyan, dim = c.fg_dim, math = c.blue, sep = c.fg_dim,
  }
  hl.NtexOk = { fg = col.ok, bg = sbg, bold = true }
  hl.NtexBusy = { fg = col.busy, bg = sbg, bold = true }
  hl.NtexErr = { fg = col.err, bg = sbg, bold = true }
  hl.NtexWarn = { fg = col.warn, bg = sbg, bold = true }
  hl.NtexHint = { fg = col.hint, bg = sbg, bold = true }
  hl.NtexInfo = { fg = col.dim, bg = sbg, bold = true }
  hl.NtexSnip = { fg = col.math, bg = sbg, bold = true }
  hl.NtexStatusDim = { fg = col.dim, bg = sbg }
  hl.NtexWinbar = { fg = col.dim, bg = wbg }
  hl.NtexWinbarSep = { fg = col.sep, bg = wbg }
  hl.NtexWinbarMath = { fg = col.math, bg = wbg, bold = true }
end

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
      local np = editor_palette()
      local p
      if np then
        p = not np.dark and M.palettes.alabaster or nil -- the light page under the palette
      elseif vim.o.background == "light" then
        p = M.palettes[palette_name()] -- commutator; nil for plain modus
      end
      if p then
        paint(hl, p)
      end
      if np and np.dark then
        bars(hl, c, {
          status = np.status, bg = np.bg, dim = np.dim, math = np.math, ok = c.green, err = c.red, hint = c.cyan,
          busy = c.yellow_warmer or c.yellow, warn = c.yellow_warmer or c.yellow,
        })
      else
        bars(hl, c, p)
      end
    end,
  })
  local group = vim.api.nvim_create_augroup("texnvim_theme", { clear = true })
  -- the palette goes on after Modus has set its groups
  vim.api.nvim_create_autocmd("ColorScheme", {
    group = group,
    callback = function()
      local np, std = editor_palette(), standard()
      local colors = require("config.look").colors(palette_name())
      if std then
        require("config.palettes").apply_standard(colors)
      elseif np and vim.g.colors_name == "modus" then
        require("config.palettes").apply(palette_name(), { ink = require("config.look").get().ink, colors = colors })
      end
      terminal()
    end,
  })
  M.reload()

  -- the terminal reports a theme (at startup, and when it changes): a
  -- palette keeps its own light or dark
  vim.api.nvim_create_autocmd("OptionSet", {
    group = group,
    pattern = "background",
    callback = function()
      local want = wanted_background()
      if want and vim.o.background ~= want then
        vim.o.background = want -- (no OptionSet from inside an autocmd)
      end
      load_scheme()
    end,
  })
  vim.api.nvim_create_autocmd({ "VimLeavePre", "VimSuspend" }, {
    group = group,
    callback = terminal_reset,
  })
  vim.api.nvim_create_autocmd("VimResume", {
    group = group,
    callback = terminal,
  })

  -- :NtexTheme code — try a palette in this ntex only (look.json, or Ntex
  -- Studio's Look page, makes it stick). No argument: say which one is on.
  vim.api.nvim_create_user_command("NtexTheme", function(o)
    if o.args ~= "" then
      M.override = o.args
      M.reload()
    end
    vim.notify("ntex palette: " .. palette_name(), vim.log.levels.INFO)
  end, {
    nargs = "?",
    complete = function()
      local names = { "slate", "code", "code-dark", "alabaster" }
      vim.list_extend(names, vim.tbl_keys(require("config.palettes").standard))
      vim.list_extend(names, { "commutator", "modus" })
      return names
    end,
    desc = "Switch ntex's palette for this session",
  })
end

--- Repaint with the palette now in effect (look.json or :NtexTheme).
function M.reload()
  local want = wanted_background()
  if want and vim.o.background ~= want then
    vim.o.background = want
  end
  local ok, err = pcall(load_scheme)
  if not ok then
    vim.notify("ntex: the " .. palette_name() .. " theme didn't load (" .. tostring(err) .. "); using Slate", vim.log.levels.WARN)
    M.override = "slate"
    vim.cmd.colorscheme("modus")
  end
end

return M
