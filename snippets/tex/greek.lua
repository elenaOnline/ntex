-- Greek letters. (Latex Suite's @a, @b, … are off: see DECISIONS.md, "Still turned off".)
--
-- One snippet per line:  math("lmd", [[\lambda]], "yours")
--   trigger → output, in maths, by itself. Last field: where it came from.
--   Never fires inside a word or a \command: \varphi and \epsilon type as is.
--   Full vocabulary: snippets/README.md. Save the file: it reloads.
local h = require("snips.helpers")
local math = h.math

return h.group("Greek", {
  -- trigger       output             origin
  math("phi",      [[\varphi]],       "yours"),
  math("lmd",      [[\lambda]],       "yours"),
  math("pi",       [[\pi]],           "castel"),
  math("zeta",     [[\zeta]],         "castel"),
})
