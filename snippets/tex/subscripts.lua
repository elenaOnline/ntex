-- Automatic subscripts: x2 → x_{2}, \alpha3 → \alpha_{3}, x_{2}4 → x_{24}, \dot{x}3 → \dot{x}_{3}
--
--   pat(pattern, output, origin, opts): the trigger is a Lua pattern
--   (%a a letter, %d a digit, %a+ letters, ( ) captures → %1 %2 in the output).
--   The letter must not end a \command: \leq2 and \sqrt2 type as is; only
--   Greek letters get a subscript (\alpha2 → \alpha_{2}).
--   Full vocabulary: snippets/README.md. Save the file: it reloads.
local h = require("snips.helpers")
local math, pat = h.math, h.pat

-- ${GREEK} and ${ACCENT} from Latex Suite's snippet variables, as sets
local function set(words)
  local out = {}
  for w in words:gmatch("%S+") do
    out[w] = true
  end
  return out
end
local GREEK = set("alpha beta gamma Gamma delta Delta epsilon varepsilon zeta eta theta vartheta Theta iota kappa lambda Lambda mu nu xi omicron pi rho varrho sigma Sigma tau upsilon Upsilon phi varphi Phi chi psi omega Omega")
local ACCENT = set("dot ddot hat bar tilde vec underline overline mathbf mathcal mathrm mathbb")

-- a single letter, or \ and a Greek name
local function letter(x)
  return x:match("^%a$") ~= nil or (x:sub(1, 1) == "\\" and GREEK[x:sub(2)] == true)
end
local function greek(_, _, c) return GREEK[c[1]] == true end
local function accent(_, _, c) return ACCENT[c[1]] == true and letter(c[2]) end
local function accent2(_, _, c) return ACCENT[c[1]] == true and ACCENT[c[2]] == true and letter(c[3]) end

return h.group("Subscripts", {
  -- trigger                                  output                       origin
  pat("(%a)(%d)",                             [[%1_{%2}]],                 "yours",   { name = "‹letter›‹digit›", priority = -1, boundary = "suffix", eg = "x2" }),
  pat([[\(%a+)(%d)]],                         [[\%1_{%2}]],                "yours",   { name = "\\‹greek›‹digit›", priority = -1, boundary = "none", check = greek, eg = "\\alpha2" }),
  pat("(%a)_{(%d+)}(%d)",                     [[%1_{%2%3}]],               "changed", { name = "x_{‹digits›}‹digit›", priority = -1, boundary = "suffix", eg = "x24" }),
  pat([[\(%a+)_{(%d+)}(%d)]],                 [[\%1_{%2%3}]],              "changed", { name = "\\‹greek›_{‹digits›}‹digit›", priority = -1, boundary = "none", check = greek, eg = "\\alpha_{2}4" }),
  pat([[\(%a+){(\?%a+)}(%d)]],                [[\%1{%2}_{%3}]],            "yours",   { name = "\\‹accent›{x}‹digit›", priority = -1, boundary = "none", check = accent, eg = "xdot3" }),
  pat([[\(%a+){(\?%a+)}_{(%d+)}(%d)]],        [[\%1{%2}_{%3%4}]],          "yours",   { name = "\\‹accent›{x}_{‹digits›}‹digit›", priority = -1, boundary = "none", check = accent, eg = "xdot34" }),
  pat([[\(%a+){\(%a+){(\?%a+)}}(%d)]],        [[\%1{\%2{%3}}_{%4}]],       "yours",   { name = "\\‹accent›{\\‹accent›{x}}‹digit›", priority = -1, boundary = "none", check = accent2, eg = "\\dot{\\vec{a}}3" }),
  pat([[\(%a+){\(%a+){(\?%a+)}}_{(%d+)}(%d)]], [[\%1{\%2{%3}}_{%4%5}]],    "yours",   { name = "\\‹accent›{\\‹accent›{x}}_{‹digits›}‹digit›", priority = -1, boundary = "none", check = accent2, eg = "\\dot{\\vec{a}}_{3}4" }),

  math("xjj",      [[x_{j}]],       "default"),
  math("xp1",      [[x_{n+1}]],     "default"),
  math("ynn",      [[y_{n}]],       "default"),
  math("yii",      [[y_{i}]],       "default"),
  math("yjj",      [[y_{j}]],       "default"),
})
