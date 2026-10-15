-- Derivatives, integrals, Taylor: \frac{\partial y}{\partial x}, \int … \, dx
--
-- One snippet per line:  math("dint", [[\int_{<:0>}^{<:1>} <> \, d<:x> ]], "changed")
--   <> tabstop, <:0> tabstop holding 0, <1:f> numbered (same number = mirror).
--   math fires by itself; math_tab needs Tab; pat(...) is a Lua-pattern trigger.
--   Full vocabulary: snippets/README.md. Save the file: it reloads.
local h = require("snips.helpers")
local math, math_tab, pat = h.math, h.math_tab, h.pat

return h.group("Calculus", {
  -- trigger        output                                                origin
  math_tab("par",   [[\frac{ \partial <:y> }{ \partial <:x> } ]],         "default"),
  pat("pa(%a)(%a)", [[\frac{ \partial %1 }{ \partial %2 } ]],             "default", { tab = true, name = "pa‹y›‹x›", eg = "payx" }),
  math("ddt",       [[\frac{d}{dt} ]],                                    "default"),

  math("int",       [[\int]],                                             "default", { priority = -1 }),
  math_tab([[\int]], [[\int <> \, d<:x> ]],                               "default"),
  math("dint",      [[\int_{<:0>}^{<:1>} <> \, d<:x> ]],                  "changed", { desc = "Definite integral" }),
  math("oint",      [[\oint]],                                            "default"),
  math("iint",      [[\iint]],                                            "default"),
  math("iiint",     [[\iiint]],                                           "default"),
  math("oinf",      [[\int_{0}^{\infty} <> \, d<:x> ]],                   "default"),
  math("infi",      [[\int_{-\infty}^{\infty} <> \, d<:x> ]],             "default"),

  math("tayl", [[<1:f>(<2:x> + <3:h>) = <1>(<2>) + <1>'(<2>)<3> + <1>''(<2>) \frac{<3>^{2}}{2!} + \dots]], "default", { desc = "Taylor expansion" }),
})
