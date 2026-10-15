-- Visual snippets: select text in maths (mouse, or Shift+arrows; from v press
-- Ctrl-g), then type the key. %V is the selection. With no <> tabstop the
-- result stays selected, so you can wrap again: select a+b, S, then ( .
--
--   vis("S", [[\sqrt{ %V }]], "changed")
--   Outside maths the key just replaces the selection. Full vocabulary: snippets/README.md.
local h = require("snips.helpers")
local vis = h.vis

return h.group("Visual", {
  -- key        output                              origin
  vis("U",      [[\underbrace{ %V }_{ <> }]],       "changed"),
  vis("O",      [[\overbrace{ %V }^{ <> }]],        "changed"),
  vis("B",      [[\underset{ <> }{ %V }]],          "changed"),
  vis("C",      [[\cancel{ %V }]],                  "changed"),
  vis("K",      [[\cancelto{ <> }{ %V }]],          "changed"),
  vis("S",      [[\sqrt{ %V }]],                    "changed"),
  vis("(",      [[(%V)]],                           "changed"),
  vis("[",      "[%V]",                             "changed"),
  vis("{",      [[{%V}]],                           "changed"),

  -- added from Gilles Castel's set (DECISIONS.md, "New from Castel": delete a line to veto it)
  vis("/",      [[\frac{%V}{<>}]],                  "castel"),
})
