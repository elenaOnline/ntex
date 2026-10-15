-- Powers, roots, fractions, operators: x^{2}, \sqrt{}, \frac{}{}, \log, \det …
--
-- One snippet per line:  math("sq", [[\sqrt{ <> }]], "default")
--   math   fires by itself in maths, at the start of a word
--   suffix fires right after what it belongs to: xsr → x^{2} (never inside a \command)
--   <> is a tabstop (Tab jumps to the next, then out). Last field: where it came from.
--   Full vocabulary: snippets/README.md. Save the file: it reloads.
local h = require("snips.helpers")
local math, suffix = h.math, h.suffix

return h.group("Operations", {
  -- trigger       output                 origin
  suffix("sr",     [[^{2}]],              "default"),
  suffix("cb",     [[^{3}]],              "default"),
  suffix("rd",     [[^{<>}]],             "changed"), -- "raise to (D) the power"
  suffix("_",      [[_{<>}]],             "default"),
  suffix("sts",    [[_\text{<>}]],        "default"),
  math("sq",       [[\sqrt{ <> }]],       "default"),
  math("//",       [[\frac{<>}{<>}]],     "default"),
  math("ee",       [[e^{ <> }]],          "yours"),
  suffix("invs",   [[^{-1}]],             "default"),
  math("exp",      [[\exp]],              "default"),
  math("log",      [[\log]],              "default"),
  math("ln",       [[\ln]],               "default"),
  suffix("conj",   [[^{*}]],              "default"),
  math("Re",       [[\mathrm{Re}]],       "default"),
  math("Im",       [[\mathrm{Im}]],       "default"),
  math("bf",       [[\mathbf{<>}]],       "default"),
  math("rm",       [[\mathrm{<>}]],       "default"),
  suffix("compl",  [[^{c}]],              "castel"), -- complement: Acompl → A^{c}

  -- linear algebra
  math("det",      [[\det]],              "default"),
  math("trace",    [[\mathrm{Tr}]],       "default"),
})
