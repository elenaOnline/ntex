-- ntex editor palettes (final, chosen 2026-10-18 in the ntex Syntax Lab):
-- "slate", "code", "code-dark"; "alabaster" is the look before this change,
-- kept for reference. Each palette paints the LaTeX syntax groups, the page
-- and bars, and the NtexKind*/NtexSection* groups structure.lua uses.
--
-- ink = "content" (chosen): black (or near-white on dark) only for text that
-- prints; commands, environments, options and braces take `markup`/`quiet`.
-- ink = "all": markup takes the ink color too (the old look).
--
-- Dark palettes (dark = true) expect vim.o.background = "dark" so the Modus
-- base underneath (menus, floats, diagnostics) is Vivendi.
local set = vim.api.nvim_set_hl

local function rgb(h)
  return tonumber(h:sub(2, 3), 16), tonumber(h:sub(4, 5), 16), tonumber(h:sub(6, 7), 16)
end
local function mix(a, b, t) -- t of b into a
  local r1, g1, b1 = rgb(a)
  local r2, g2, b2 = rgb(b)
  local f = function(x, y)
    return math.floor(x + (y - x) * t + 0.5)
  end
  return string.format("#%02x%02x%02x", f(r1, r2), f(g1, g2), f(b1, b2))
end

local LIGHT_KINDS = {
  Statement = "#7a3e9d", Definition = "#2e7d32", Example = "#8a5a00",
  Proof = "#56647a", Problem = "#0d6e8f", Layout = "#8a8a8a",
}

local M = {}
local P = {}
M.palettes = P

-- Alabaster: round 2 as it stands (her current pick), for reference
P.alabaster = {
  bg = "#ffffff", ink = "#000000", math = "#325cc0", mathcmd = "#325cc0", mathdelim = "#777777",
  markup = "#6b7180", quiet = "#a3a8b2", comment = "#aa3731", file = "#a3a8b2",
  kinds = LIGHT_KINDS,
}


-- Slate: everything that is not content takes one cool hue family, so
-- "blue-gray = machinery" is a single rule; math is a near-black indigo.
P.slate = {
  bg = "#ffffff", ink = "#111111", math = "#2a2f6b", mathcmd = "#5a5fa8", mathdelim = "#b5b8d6",
  markup = "#7b8fae", quiet = "#c3cddc", comment = "#a8455a", file = "#9fb0c8",
  kinds = LIGHT_KINDS,
}

-- Code: strong, saturated roles like a code theme. Math letters blue, math
-- commands and operators magenta, markup commands red, file names navy,
-- comments gray italic (the code convention), prose stays black.
P.code = {
  bg = "#ffffff", ink = "#1f2328", math = "#0550ae", mathcmd = "#bf1d78", mathdelim = "#8c959f",
  markup = "#cf222e", quiet = "#8c959f", comment = "#6e7781", comment_italic = true, file = "#0a3069",
  kinds = LIGHT_KINDS,
}


-- Code Dark: Code's roles on a dark page (GitHub-dark-like ink), with the
-- block hues lifted to read on dark.
P["code-dark"] = {
  dark = true,
  bg = "#0f1318", ink = "#e6edf3", math = "#79c0ff", mathcmd = "#f78fc6", mathdelim = "#6e7681",
  markup = "#ff7b72", quiet = "#6e7681", comment = "#8b949e", comment_italic = true, file = "#a5d6ff",
  kinds = {
    Statement = "#c7a0e6", Definition = "#7fd08a", Example = "#e3b55c",
    Proof = "#a8b4c8", Problem = "#5cc8e6", Layout = "#7a7f8a",
  },
  linenr = "#4d5566", cursorline = "#1c222b", cursorlinenr = "#252c37", status = "#232a34",
  dim = "#9aa4b2", rule = "#39414d", subrule = "#2a313b", num = "#5d6778",
}


-- Standard editor themes, as their Neovim ports draw LaTeX, for comparing
-- against the palettes above. Only the structure marks go on top, colored
-- from each theme's own groups (M.apply_standard); ink = "content" does not
-- apply. `scheme` is the :colorscheme name, `dark` sets 'background' first.
M.standard = {
  ["vscode-light"] = { label = "VS Code Light+", scheme = "vscode", dark = false },
  ["vscode-dark"] = { label = "VS Code Dark+", scheme = "vscode", dark = true },
  ["github-light"] = { label = "GitHub Light", scheme = "github_light", dark = false },
  ["one-light"] = { label = "One Light", scheme = "onelight", dark = false },
  ["solarized-light"] = { label = "Solarized Light", scheme = "solarized", dark = false },
  ["solarized-dark"] = { label = "Solarized Dark", scheme = "solarized", dark = true },
  ["catppuccin-latte"] = { label = "Catppuccin Latte", scheme = "catppuccin-latte", dark = false },
  ["catppuccin-mocha"] = { label = "Catppuccin Mocha", scheme = "catppuccin-mocha", dark = true },
  ["tokyonight"] = { label = "Tokyo Night", scheme = "tokyonight-night", dark = true },
  ["gruvbox-dark"] = { label = "Gruvbox Dark", scheme = "gruvbox", dark = true },
  -- newpaper keeps a style of its own (default light) and brings its own
  -- LaTeX syntax extras (after/syntax/tex.vim): set the style before each load
  ["newpaper-light"] = { label = "Newpaper Light", scheme = "newpaper", dark = false, before = function()
    require("newpaper").setup({ style = "light" })
  end },
  ["newpaper-dark"] = { label = "Newpaper Dark", scheme = "newpaper", dark = true, before = function()
    require("newpaper").setup({ style = "dark" })
  end },
}

M.STANDARD_ORDER = { "vscode-light", "vscode-dark", "github-light", "one-light", "solarized-light",
  "solarized-dark", "catppuccin-latte", "catppuccin-mocha", "tokyonight", "gruvbox-dark", "newpaper-light",
  "newpaper-dark" }

-- the nesting lines' share of the page (dark pages a little less), 0–0.95
local function fade(pct, dark)
  local f = (pct - (dark and 7 or 0)) / 100
  return f > 0.95 and 0.95 or f
end

local function fg_of(group, fallback)
  local h = vim.api.nvim_get_hl(0, { name = group, link = false })
  return h.fg and string.format("#%06x", h.fg) or fallback
end
local function bg_of(group, fallback)
  local h = vim.api.nvim_get_hl(0, { name = group, link = false })
  return h.bg and string.format("#%06x", h.bg) or fallback
end

--- The page colors of the colorscheme now loaded: { bg, fg } (for the terminal).
function M.page()
  local dark = vim.o.background == "dark"
  return { bg = bg_of("Normal", dark and "#000000" or "#ffffff"), fg = fg_of("Normal", dark and "#ffffff" or "#000000") }
end

-- Every color a palette is made of, as the Look page edits them (look.json
-- "colors": per palette, role → "#rrggbb", painted over the palette). The
-- kinds are the block colors; the rest are page and syntax roles.
M.ROLES = { "bg", "ink", "math", "mathcmd", "mathdelim", "markup", "quiet", "comment", "file",
  "Statement", "Definition", "Example", "Proof", "Problem", "Layout" }
local KIND = { Statement = true, Definition = true, Example = true, Proof = true, Problem = true, Layout = true }

-- the syntax groups each role paints (fg)
local GROUPS = {
  math = { "texMathZone", "texMathZoneX", "texMathZoneXX", "texMathZoneTI", "texMathZoneTD",
    "texMathZoneLI", "texMathZoneLD", "texMathZoneEnv", "texMathZoneEnsured", "texMathSuperSub",
    "texMathArg", "texMathEnvArgName", "texDeclmathoperArgBody", "texMathTagArg", "Conceal" },
  mathcmd = { "texMathCmd", "texMathCmdEnv", "texMathSymbol", "texMathOper", "texCmdGreek",
    "texMathCmdText", "texMathDelim", "texTabularChar" },
  mathdelim = { "texMathDelimZone" },
  markup = { "texCmd", "texCmdEnv", "texCmdPart", "texCmdItem", "texCmdClass",
    "texCmdPackage", "texCmdTitle", "texCmdAuthor", "texCmdStyle", "texCmdStyleBold",
    "texCmdStyleItal", "texCmdInput", "texCmdDeclmathoper", "texCmdNewcmd", "texCmdRef",
    "texCmdCRef", "texDeclmathoperArgName", "texLength", "texSpecialChar", "texCmdSize", "texEnvArgName" },
  quiet = { "texDelim", "texOpt", "texEnvOpt", "texOptEqual", "texTabularCol", "texItemLabelDelim" },
  file = { "texFileArg", "texFilesArg" },
  comment = { "texComment", "texCommentAcronym" },
  ink = { "texTitleArg", "texAuthorArg", "texGroup", "texItemLabelConcealed", "SpecialChar", "texArg",
    "texMathTextConcArg", "texStyleBold", "texPartArgTitle" },
}
M.GROUPS = GROUPS

-- recolor a group, keeping its other attributes (bold, italic …)
local function recolor(group, key, color)
  local h = vim.api.nvim_get_hl(0, { name = group, link = false })
  h[key] = color
  set(0, group, h)
end

local function hexok(c)
  return type(c) == "string" and c:match("^#%x%x%x%x%x%x$") ~= nil
end

--- The palette last painted, before the look.json colors: role → color (the Look page's "reset" values).
M.base = {}

--- After a standard theme's :colorscheme: the structure marks in its own
--- colors (statement = its keyword color, definition = strings, example =
--- numbers, proof = comments, problem = types, layout = line numbers, math =
--- its math zone), the status bar groups, and concealed symbols in its math
--- color (most themes paint Conceal pale, which hides α, ℝ).
function M.apply_standard(colors)
  colors = colors or {}
  local F = require("config.structure").opts
  local dark = vim.o.background == "dark"
  local page = M.page()
  local base = {
    bg = page.bg, ink = page.fg,
    math = fg_of("texMathZone", fg_of("Special", page.fg)),
    quiet = fg_of("texDelim", fg_of("Delimiter", fg_of("LineNr", page.fg))),
  }
  base.mathcmd = fg_of("texMathCmd", base.math)
  base.mathdelim = fg_of("texMathDelimZone", base.quiet)
  base.markup = fg_of("texCmd", page.fg)
  base.comment = fg_of("texComment", fg_of("Comment", page.fg))
  base.file = fg_of("texFileArg", page.fg)
  base.Statement = fg_of("Keyword", fg_of("Statement", page.fg))
  base.Definition = fg_of("String", page.fg)
  base.Example = fg_of("Number", fg_of("Constant", page.fg))
  base.Proof = fg_of("Comment", page.fg)
  base.Problem = fg_of("Type", page.fg)
  base.Layout = fg_of("LineNr", base.quiet)
  M.base = base
  local c = vim.tbl_extend("force", {}, base)
  for role, v in pairs(colors) do
    if c[role] and hexok(v) then
      c[role] = v
    end
  end
  -- the overridden roles, on the theme's own groups
  for role, v in pairs(colors) do
    if hexok(v) and GROUPS[role] then
      for _, g in ipairs(GROUPS[role]) do
        recolor(g, "fg", v)
      end
    end
  end
  if colors.bg or colors.ink then
    for _, g in ipairs({ "Normal", "NormalNC" }) do
      set(0, g, { fg = c.ink, bg = c.bg })
    end
    for _, g in ipairs({ "EndOfBuffer", "SignColumn", "LineNr", "LineNrAbove", "LineNrBelow", "FoldColumn" }) do
      recolor(g, "bg", c.bg)
    end
  end
  local bg, ink, math, quiet = c.bg, c.ink, c.math, c.quiet
  local kinds = {
    Statement = c.Statement, Definition = c.Definition, Example = c.Example,
    Proof = c.Proof, Problem = c.Problem, Layout = c.Layout, Math = math,
  }
  for k, c in pairs(kinds) do
    set(0, "NtexKind" .. k, { fg = c })
    set(0, "NtexKind" .. k .. "Title", { fg = c, bold = k ~= "Layout" and F.title_bold })
    set(0, "NtexKind" .. k .. "Bar", { fg = c, bg = bg_of("LineNr", nil) })
    set(0, "NtexKind" .. k .. "Cmd", { fg = mix(c, dark and bg or "#000000", F.begin_shade / 100 * (dark and 0.79 or 1)) })
    set(0, "NtexKind" .. k .. "Delim", { fg = mix(quiet, c, F.brace_tint / 100) })
    set(0, "NtexKind" .. k .. "Guide", { fg = mix(c, bg, fade(F.guide_fade, dark)) })
  end
  local lnr = fg_of("LineNr", quiet)
  set(0, "NtexKindLayoutGuide", { fg = mix(lnr, bg, 0.4) })
  set(0, "NtexSectionRule", { fg = mix(lnr, bg, 0.3) })
  set(0, "NtexSubsectionRule", { fg = mix(lnr, bg, 0.55) })
  set(0, "NtexSectionNum", { fg = lnr })
  set(0, "NtexSectionTitle", { fg = ink, bold = true })
  set(0, "NtexSubsectionTitle", { fg = ink, bold = true })
  set(0, "NtexSectionBand", { bg = mix(bg, kinds.Statement, dark and 0.12 or 0.07) })
  set(0, "NtexSubsectionBand", { bg = mix(bg, kinds.Statement, dark and 0.06 or 0.035) })
  set(0, "Conceal", { fg = math })

  -- status bar / winbar (lua/config/statusline.lua)
  local sbg = bg_of("StatusLine", mix(bg, ink, 0.12))
  local col = {
    Ok = fg_of("DiagnosticOk", kinds.Definition), Busy = fg_of("DiagnosticWarn", kinds.Example),
    Err = fg_of("DiagnosticError", ink), Warn = fg_of("DiagnosticWarn", kinds.Example),
    Hint = fg_of("DiagnosticHint", kinds.Problem), Info = fg_of("Comment", ink), Snip = math,
  }
  for k, c in pairs(col) do
    set(0, "Ntex" .. k, { fg = c, bg = sbg, bold = true })
  end
  set(0, "NtexStatusDim", { fg = fg_of("Comment", ink), bg = sbg })
  set(0, "NtexWinbar", { fg = fg_of("Comment", ink), bg = bg })
  set(0, "NtexWinbarSep", { fg = lnr, bg = bg })
  set(0, "NtexWinbarMath", { fg = math, bg = bg, bold = true })
end

--- Paint palette `name` (opts.ink: "content" | "all"). Call it from the
--- colorscheme's on_highlights, after the Modus base is set.
function M.apply(name, opts)
  opts = opts or {}
  local F = require("config.structure").opts
  local p = vim.tbl_extend("force", {}, P[name])
  p.kinds = vim.tbl_extend("force", {}, P[name].kinds)
  M.base = vim.tbl_extend("force", {}, p.kinds)
  for _, r in ipairs(M.ROLES) do
    if not KIND[r] then
      M.base[r] = p[r]
    end
  end
  local colors = opts.colors or {}
  for role, v in pairs(colors) do
    if hexok(v) then
      if KIND[role] then
        p.kinds[role] = v
      elseif M.base[role] then
        p[role] = v
      end
    end
  end
  if opts.ink == "all" and not colors.markup then
    p.markup = p.ink
  end
  local dark = p.dark
  local bg = p.bg
  local kinds = vim.tbl_extend("force", {}, p.kinds, { Math = p.math })

  -- page and bars
  set(0, "Normal", { fg = p.ink, bg = bg })
  set(0, "NormalNC", { fg = p.ink, bg = bg })
  set(0, "EndOfBuffer", { fg = p.linenr or "#9a9a9a", bg = bg })
  set(0, "LineNr", { fg = p.linenr or "#9a9a9a", bg = bg })
  set(0, "LineNrAbove", { fg = p.linenr or "#9a9a9a", bg = bg })
  set(0, "LineNrBelow", { fg = p.linenr or "#9a9a9a", bg = bg })
  set(0, "CursorLine", { bg = p.cursorline or "#ebebeb" })
  set(0, "CursorLineNr", { fg = p.ink, bg = p.cursorlinenr or "#e3e3e3", bold = true })
  set(0, "StatusLine", { fg = p.ink, bg = p.status or "#dddddd" })
  set(0, "WinBar", { fg = p.dim or "#5c5c5c", bg = bg })
  set(0, "NtexWinbar", { fg = p.dim or "#5c5c5c", bg = bg })
  set(0, "NtexWinbarSep", { fg = p.rule or "#b0b0b0", bg = bg })
  set(0, "NtexWinbarMath", { fg = p.math, bg = bg, bold = true })
  set(0, "NtexStatusDim", { fg = p.dim or "#5c5c5c", bg = p.status or "#dddddd" })
  set(0, "SpellBad", { undercurl = true, sp = dark and "#a05a5a" or "#d08080" })
  set(0, "Conceal", { fg = p.math })

  -- LaTeX syntax
  for _, g in ipairs({ "texMathZone", "texMathZoneX", "texMathZoneXX", "texMathZoneTI", "texMathZoneTD",
    "texMathZoneLI", "texMathZoneLD", "texMathZoneEnv", "texMathZoneEnsured", "texMathSuperSub",
    "texMathArg", "texMathEnvArgName", "texDeclmathoperArgBody", "texMathTagArg" }) do
    set(0, g, { fg = p.math })
  end
  for _, g in ipairs({ "texMathCmd", "texMathCmdEnv", "texMathSymbol", "texMathOper", "texCmdGreek",
    "texMathCmdText", "texMathDelim" }) do
    set(0, g, { fg = p.mathcmd })
  end
  if name:match("^code") then
    set(0, "texMathDelim", { fg = p.ink })
    set(0, "texMathOper", { fg = p.mathcmd })
  end
  set(0, "texMathDelimZone", { fg = p.mathdelim })
  set(0, "texMathTextConcArg", { fg = p.ink })
  for _, g in ipairs({ "texCmd", "texCmdEnv", "texCmdPart", "texCmdItem", "texCmdClass",
    "texCmdPackage", "texCmdTitle", "texCmdAuthor", "texCmdStyle", "texCmdStyleBold",
    "texCmdStyleItal", "texCmdInput", "texCmdDeclmathoper", "texCmdNewcmd", "texCmdRef",
    "texCmdCRef", "texDeclmathoperArgName", "texLength", "texSpecialChar", "texCmdSize" }) do
    set(0, g, { fg = p.markup })
  end
  for _, g in ipairs({ "texDelim", "texOpt", "texEnvOpt", "texOptEqual", "texTabularCol",
    "texItemLabelDelim" }) do
    set(0, g, { fg = p.quiet })
  end
  set(0, "texFileArg", { fg = p.file })
  set(0, "texFilesArg", { fg = p.file })
  set(0, "texEnvArgName", { fg = p.markup })
  for _, g in ipairs({ "texTitleArg", "texAuthorArg", "texGroup", "texItemLabelConcealed", "SpecialChar", "texArg" }) do
    set(0, g, { fg = p.ink })
  end
  set(0, "texStyleBold", { fg = p.ink, bold = true })
  set(0, "texPartArgTitle", { fg = p.ink, bold = true })
  set(0, "texTabularChar", { fg = p.mathcmd })
  set(0, "texComment", { fg = p.comment, italic = p.comment_italic or false })
  set(0, "texCommentAcronym", { fg = p.comment, italic = p.comment_italic or false })

  -- structure.lua groups
  for k, c in pairs(kinds) do
    set(0, "NtexKind" .. k, { fg = c })
    set(0, "NtexKind" .. k .. "Title", { fg = c, bold = k ~= "Layout" and F.title_bold })
    set(0, "NtexKind" .. k .. "Bar", { fg = c })
    set(0, "NtexKind" .. k .. "Cmd", { fg = mix(c, dark and bg or "#000000", F.begin_shade / 100 * (dark and 0.79 or 1)) })
    set(0, "NtexKind" .. k .. "Delim", { fg = mix(p.quiet, c, F.brace_tint / 100) })
    set(0, "NtexKind" .. k .. "Guide", { fg = mix(c, bg, fade(F.guide_fade, dark)) })
  end
  set(0, "NtexKindLayoutGuide", { fg = mix(p.quiet, bg, 0.3) })
  set(0, "NtexSectionRule", { fg = p.rule or "#c4c4c4" })
  set(0, "NtexSubsectionRule", { fg = p.subrule or "#dadada" })
  set(0, "NtexSectionNum", { fg = p.num or "#9a9a9a" })
  set(0, "NtexSectionTitle", { fg = p.ink, bold = true })
  set(0, "NtexSubsectionTitle", { fg = p.ink, bold = true })
  set(0, "NtexSectionBand", { bg = dark and mix(bg, kinds.Statement, 0.12) or "#eef1f7" })
  set(0, "NtexSubsectionBand", { bg = dark and mix(bg, kinds.Statement, 0.06) or "#f6f7fa" })
end

return M
