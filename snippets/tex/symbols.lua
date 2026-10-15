-- Symbols, relations, arrows, big operators, blackboard / calligraphic letters.
--
-- One snippet per line:  math("->", [[\to]], "default")
--   math     fires by itself in maths       math_tab   needs Tab after the trigger
--   <> tabstop, <:i> tabstop holding i. Last field: where it came from.
--   Full vocabulary: snippets/README.md. Save the file: it reloads.
local h = require("snips.helpers")
local math, math_tab = h.math, h.math_tab

return h.group("Symbols", {
  -- trigger        output                                origin
  math("ooo",       [[\infty]],                           "default"),
  math("sum",       [[\sum]],                             "default"),
  math("prod",      [[\prod]],                            "default"),
  math_tab([[\sum]],  [[\sum_{<:i>=<:1>}^{<:N>} ]],       "default"),
  math_tab([[\prod]], [[\prod_{<:i>=<:1>}^{<:N>} ]],      "default"),
  math("lim",       [[\lim_{ <:n> \to <:\infty> } ]],     "default"),
  math("+-",        [[\pm]],                              "default"),
  math("-+",        [[\mp]],                              "default"),
  math("...",       [[\dots]],                            "default"),
  math("nabl",      [[\nabla]],                           "default"),
  math("xx",        [[\times]],                           "default"),
  math("**",        [[\cdot]],                            "default"),
  math("para",      [[\parallel]],                        "default"),

  math("===",       [[\equiv]],                           "default"),
  math("!=",        [[\neq]],                             "default"),
  math(">=",        [[\geq]],                             "default"), math("geq",        [[\geq]],                             "default"),
  math("leq",        [[\leq]],                             "default"), math("<=",        [[\leq]],                             "default"),
  math(">>",        [[\gg]],                              "default"),
  math("<<",        [[\ll]],                              "default"),
  math("simm",      [[\sim]],                             "default"),
  math("sim=",      [[\simeq]],                           "default"),
  math("prop",      [[\propto]],                          "default"),

  math("<->",       [[\leftrightarrow ]],                 "default"),
  math("->",        [[\to]],                              "default"),
  math("!>",        [[\mapsto]],                          "default"),
  math("=>",        [[\implies]],                         "default"),
  math("=<",        [[\impliedby]],                       "default"),

 math("Nat",        [[\mathcal{N}]],                      "default"), math("LL",        [[\mathcal{L}]],                      "default"),
  math("HH",        [[\mathcal{H}]],                      "default"),
  math("RR",        [[\mathbb{R}]],                       "default"),
  math("QQ",        [[\mathbb{Q}]],                       "default"), math("ZZ",        [[\mathbb{Z}]],                       "default"),
  -- CC and NN are in sets.lua (yours); the second CC (\mathbb{C}) and NN
  -- (\mathbb{N}) in snippets.js never fired and were dropped (DECISIONS.md).

  -- added from Gilles Castel's set (DECISIONS.md, "New from Castel": delete a line to veto it)
  math("lll",       [[\ell]],                             "castel"),
  math("mcal",      [[\mathcal{<>}]],                     "castel"),
  math("star",      [[\star]],                            "castel"),
  math("perp",      [[\perp]],                            "castel"),
  math("~~",        [[\sim ]],                            "castel"),
  math("<!",        [[\triangleleft ]],                   "castel"),
  math("<>",        [[\diamond ]],                        "castel"),
  math("DD",        [[\mathbb{D}]],                       "castel"),
})
