-- The maths-typing settings, from prefs.lua `latex = { … }`, in the shape the
-- features (lua/snips/features.lua) and the snippet conditions use.
-- Nothing here reads files other than prefs.lua (through config.prefs).
local M = {}

local function set_of(list)
  local out = {}
  for _, v in ipairs(list or {}) do
    out[v] = true
  end
  return out
end

--- Build the settings table from a `latex` prefs table (defaults filled in).
function M.from(p)
  local s = { raw = p }
  s.snippets_enabled = p.snippets ~= false
  s.fire_after_letters = p.fire_after_letters == true
  s.remove_whitespace = p.remove_snippet_whitespace ~= false
  s.auto_delete_dollar = p.auto_delete_dollar ~= false
  s.autofraction = p.autofraction ~= false
  s.autofraction_symbol = p.autofraction_symbol or "\\frac"
  s.autofraction_breaking = p.autofraction_breaking_chars or "+-=\t"
  s.autofraction_excluded = {}
  for _, e in ipairs(p.autofraction_excluded or {}) do
    if type(e) == "table" and type(e[1]) == "string" and type(e[2]) == "string" then
      s.autofraction_excluded[#s.autofraction_excluded + 1] = { open = e[1], close = e[2] }
    end
  end
  s.matrix = p.matrix_shortcuts ~= false
  s.matrix_envs = set_of(p.matrix_envs)
  s.matrix_envs_tex = set_of(p.matrix_envs)
  for _, n in ipairs(p.matrix_envs_tex or {}) do
    s.matrix_envs_tex[n] = true
  end
  s.matrix_macros = set_of(p.matrix_macros)
  s.tabout = p.tabout ~= false
  s.tabout_eol_only = p.tabout_exit_only_at_eol ~= false
  s.tabout_closing = set_of(p.tabout_closing)
  s.enlarge = p.enlarge_brackets ~= false
  s.enlarge_space = p.enlarge_space ~= false
  s.enlarge_triggers = vim.deepcopy(p.enlarge_triggers or {})
  s.force_math_languages = vim.deepcopy(p.force_math_languages or { "math" })
  return s
end

local current
--- The live settings (prefs.lua is read once, at startup).
function M.get()
  if not current then
    current = M.from(require("config.prefs").latex or {})
  end
  return current
end

--- Tests use this to try a setting without editing prefs.lua.
function M.override(changes)
  local p = vim.deepcopy(require("config.prefs").latex or {})
  for k, v in pairs(changes or {}) do
    p[k] = v
  end
  current = M.from(p)
  require("snips.mdmath").force_math = current.force_math_languages
  return current
end

return M
