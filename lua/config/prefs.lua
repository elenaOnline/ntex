-- Loads ~/.config/texnvim/prefs.lua (hand-edited) over these defaults.
-- `latex` is merged key by key, so prefs.lua only needs the keys it changes.
local defaults = {
  images = false,
  conceal = false,
  live_pdf = true,
  live_pdf_delay_ms = 800,
  pdf_viewer = "tdf",
  tern_pdf_viewer = "browser", -- \lv inside Tern: "browser" (sharp) | "tdf" | "skim"
  conceal_scripts = true,
  compile_errors = true,
  box_warnings = false,
  harper = true,
  harper_dialect = "American",
  copilot = true,
  copilot_at_start = true,
  latex = {
    snippets = true,
    fire_after_letters = false,
    remove_snippet_whitespace = true,
    auto_delete_dollar = true,
    autofraction = true,
    autofraction_symbol = "\\frac",
    autofraction_breaking_chars = "+-=\t",
    autofraction_excluded = { { "^{", "}" }, { "\\pu{", "}" } },
    matrix_shortcuts = true,
    matrix_envs = { "pmatrix", "cases", "align", "gather", "bmatrix", "Bmatrix", "vmatrix", "Vmatrix", "array", "matrix" },
    matrix_envs_tex = {
      "align*", "gather*", "aligned", "alignedat", "gathered", "split", "alignat", "alignat*",
      "flalign", "flalign*", "eqnarray", "eqnarray*", "pmatrix*", "bmatrix*", "smallmatrix", "dcases",
    },
    matrix_macros = { "eqalign" },
    tabout = true,
    tabout_exit_only_at_eol = true,
    tabout_closing = { ")", "]", "\\rbrack", "\\}", "\\rbrace", "\\rangle", "\\rvert", "\\rVert", "\\rfloor", "\\rceil", "\\urcorner", "}" },
    enlarge_brackets = true,
    enlarge_space = true,
    enlarge_triggers = { "sum", "int", "frac", "prod", "bigcup", "bigcap" },
    force_math_languages = { "math" },
  },
}

local path = vim.fn.stdpath("config") .. "/prefs.lua"
local prefs = vim.deepcopy(defaults)
local ok, user = pcall(dofile, path)
if ok and type(user) == "table" then
  for k, v in pairs(user) do
    if k == "latex" and type(v) == "table" then
      for lk, lv in pairs(v) do
        prefs.latex[lk] = lv
      end
    else
      prefs[k] = v
    end
  end
elseif vim.uv.fs_stat(path) then
  vim.schedule(function()
    vim.notify("texnvim: prefs.lua has an error, using defaults:\n" .. tostring(user), vim.log.levels.WARN)
  end)
end

-- prefs._defaults: the defaults above (the reference page shows them; pairs() skips it)
return setmetatable(prefs, { __index = { _defaults = defaults } })
