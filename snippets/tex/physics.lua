-- Physics and quantum mechanics: k_{B}T, \dagger, \otimes, bras and kets.
--
-- One snippet per line:  math("ket", [[\ket{<>} ]], "default")
--   suffix fires right after what it belongs to: Adag → A^{\dagger}.
--   <1:\psi> twice = one tabstop typed in two places. Full vocabulary: snippets/README.md.
local h = require("snips.helpers")
local math, suffix = h.math, h.suffix

return h.group("Physics", {
  -- trigger       output                                   origin
  math("kbt",      [[k_{B}T]],                              "default"),
  math("msun",     [[M_{\odot}]],                           "default"),

  -- quantum mechanics
  suffix("dag",    [[^{\dagger}]],                          "default"),
  math("o+",       [[\oplus ]],                             "default"),
  math("ox",       [[\otimes ]],                            "changed"),
  math("bra",      [[\bra{<>} ]],                           "default"),
  math("ket",      [[\ket{<>} ]],                           "default"),
  math("brk",      [[\braket{ <> | <> } ]],                 "default"),
  math("outer",    [[\ket{<1:\psi>} \bra{<1>} ]],           "default"),
})
