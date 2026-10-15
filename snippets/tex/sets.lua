-- Set operations and logic: ∪ ∩ ⊆ ∈ ∃ ∀ ⟹ …
--
-- One snippet per line:  math("uu", [[\cup]], "yours")
--   trigger "uu" → \cup, in maths, by itself. <> is a tabstop (Tab jumps to the
--   next, then out). Last field: where it came from. Save the file: it reloads.
--   Letter triggers never fire inside a word or a \command (\subset types as is).
--   Full vocabulary: snippets/README.md.
local h = require("snips.helpers")
local math = h.math

return h.group("Set operations", {
  -- trigger       output                          origin
  math("CC",       [[\mathcal{C}]],                "changed"), -- Latex Suite: \mathbb{C}
  math("Nat",      [[\mathbb{N}]],                 "yours"),
  math("FF",       [[\mathbb{F}]],                 "yours"),
  math("disj",     [[\mathcal{D}]],                "yours"),
  math("pow",      [[\mathcal{P}]],                "yours"),
  math("uu",       [[\cup]],                       "yours"),
  math("UU",       [[\bigcup_{<>}]],               "yours"),
  math("NN",       [[\bigcap_{<>}]],               "changed"), -- Latex Suite: \mathbb{N}
  math("exx",      [[\exists]],                    "yours"),
  math("fall",     [[\forall]],                    "yours"),
  math("psub",     [[\subset]],                    "yours"),
  math("diff",     [[\setminus]],                  "yours"),
  math("imp",      [[\implies]],                   "yours"),
  math("qed",      [[\ \blacksquare]],             "yours"),
  math("nn",       [[\cap]],                       "yours", { priority = -1 }),
  math("iff",      [[\iff]],                       "yours"),
  math("ninn",     [[\notin]],                     "yours"),
  math("st.",      [[\text{ s.t. }]],              "yours"),
  math("odp",      [[\langle <> , <> \rangle ]],   "yours"),
  math("sub",      [[\subseteq]],                  "yours"),
  math("neg",      [[\neg]],                       "yours"),

  math("and",      [[\land{}]],                    "changed"), -- Latex Suite: \cap
  math("orr",      [[\lor{}]],                     "changed"), -- Latex Suite: \cup
  math("inn",      [[\in ]],                       "changed"),
  math([[\\\]],    [[\setminus{}]],                "changed"),
  math("sup=",     [[\supseteq{}]],                "changed"),
  math("eset",     [[\varnothing{}]],              "changed"), -- Latex Suite: \emptyset
  math("set",      [[\{ <> \}]],                   "changed"),
  math("cset",     [[\{ <> : <>\}]],               "yours"),
})
