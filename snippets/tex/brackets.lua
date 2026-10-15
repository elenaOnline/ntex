-- Brackets: ( ) [ ] { } pairs, \langle \rangle, |…|, \left( \right) …
--
-- One snippet per line:  math("avg", [[\langle <> \rangle ]], "default")
--   <> is where the cursor goes; Tab leaves. Typing ) ] } steps over the one
--   that is already there. A trailing space is dropped in inline maths.
--   math_tab needs Tab after the trigger. Full vocabulary: snippets/README.md.
local h = require("snips.helpers")
local math, math_tab = h.math, h.math_tab

return h.group("Brackets", {
  -- trigger       output                          origin
  math("gen",      [[\langle <> \rangle ]],        "default"),
  math("norm",     [[\lvert <> \rvert ]],          "default", { priority = 1 }),
  math("Norm",     [[\lVert <> \rVert ]],          "default", { priority = 1 }),
  math("ceil",     [[\lceil <> \rceil ]],          "default"),
  math("floor",    [[\lfloor <> \rfloor ]],        "default"),
  math("mod",      [[/]],                          "changed"), -- Latex Suite: |…|
  math("(",        "(<>)",                         "default"),
  math("{",        "{<>}",                         "default"),
  math("[",        "[<>]",                         "default"),
  math("lr(",      [[\left( <> \right) ]],         "default"),
  math("lr{",      [[\left\{ <> \right\} ]],       "default"),
  math("lr[",      [[\left[ <> \right] ]],         "default"),
  math("lr|",      [[\left| <> \right| ]],         "default"),
  math("lra",      [[\left< <> \right> ]],         "default"),

  -- added from Gilles Castel's set (DECISIONS.md, "New from Castel": delete a line to veto it)
  math_tab("lr",   [[\left( <> \right) ]],         "castel"),
  math_tab("lrb",  [[\left\{ <> \right\} ]],       "castel"),
})
