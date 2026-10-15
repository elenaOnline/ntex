-- Chemistry (mhchem): \ce{…}, \pu{…}, isotopes.
--
-- One snippet per line:  math("cee", [[\ce{ <> }]], "default")
--   Inside \ce{…} and \pu{…} no maths snippets fire (mhchem has its own
--   syntax: H2O, A -> B). Full vocabulary: snippets/README.md.
local h = require("snips.helpers")
local math = h.math

return h.group("Chemistry", {
  -- trigger       output                          origin
  math("pu",       [[\pu{ <> }]],                  "default"),
  math("cee",      [[\ce{ <> }]],                  "default"),
  math("he4",      [[{}^{4}_{2}He ]],              "default"),
  math("he3",      [[{}^{3}_{2}He ]],              "default"),
  math("iso",      [[{}^{<:4>}_{<:2>}<:He>]],      "default"),
})
