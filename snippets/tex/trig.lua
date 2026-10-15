-- Trigonometric functions: sin → \sin, then \sinx → \sin x
--
-- One snippet per line:  math("sin", [[\sin]], "default")
--   Never fires inside a word or a \command (\arcsin, \sinh type as is).
--   pat(...) is a Lua-pattern trigger: \sin then a letter gets a space.
--   Full vocabulary: snippets/README.md. Save the file: it reloads.
local h = require("snips.helpers")
local math, pat = h.math, h.pat

local TRIG = { arcsin = true, sin = true, arccos = true, cos = true, arctan = true, tan = true, csc = true, sec = true, cot = true }
local HYPERBOLIC = { sinh = true, cosh = true, tanh = true, coth = true }
-- \sin then a letter other than h (so \sinh still works)
local function trig_then_letter(_, _, c) return TRIG[c[1]] == true and c[2] ~= "h" end
local function hyperbolic_then_letter(_, _, c) return HYPERBOLIC[c[1]] == true end

return h.group("Trigonometry", {
  -- trigger        output             origin
  math("sin",       [[\sin]],          "default"),
  math("cos",       [[\cos]],          "default"),
  math("tan",       [[\tan]],          "default"),
  math("csc",       [[\csc]],          "default"),
  math("sec",       [[\sec]],          "default"),
  math("cot",       [[\cot]],          "default"),
  math("arcsin",    [[\arcsin]],       "default"),
  math("arccos",    [[\arccos]],       "default"),
  math("arctan",    [[\arctan]],       "default"),
  pat([[\(%a+)(%a)]], [[\%1 %2]],      "yours", { name = "\\sin‹letter›", boundary = "none", check = trig_then_letter, eg = "sinx", desc = "Add space after trig funcs. Skips letter h to allow sinh, cosh, etc." }),
  pat([[\(%a+)(%a)]], [[\%1 %2]],      "yours", { name = "\\sinh‹letter›", boundary = "none", check = hyperbolic_then_letter, eg = "sinhx", desc = "Add space after hyperbolic trig funcs" }),
  -- not built into MathJax / LaTeX
  math("arccsc",    [[\operatorname{arccsc}]], "default", { priority = 1 }),
  math("arcsec",    [[\operatorname{arcsec}]], "default", { priority = 1 }),
  math("arccot",    [[\operatorname{arccot}]], "default", { priority = 1 }),
})
