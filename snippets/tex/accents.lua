-- Accents and bold: \hat{x}, \bar{x}, \vec{v}, \mathbf{v} …
--
--   post("hat", [[\hat{%1}]], "default")   xhat → \hat{x}: grabs the letter typed
--     just before the trigger (%1). Never grabs the end of a \command (\hbar, \widehat).
--   math("hat", [[\hat{<>}]], "default")   hat on its own → \hat{|}
--   pat(...) is a Lua-pattern trigger; here \mu,. → \boldsymbol{\mu}.
--   Full vocabulary: snippets/README.md. Save the file: it reloads.
local h = require("snips.helpers")
local math, post, pat = h.math, h.post, h.pat

-- ${GREEK} from Latex Suite's snippet variables, as a set
local GREEK = {}
for name in ("alpha beta gamma Gamma delta Delta epsilon varepsilon zeta eta theta vartheta Theta iota kappa lambda Lambda mu nu xi omicron pi rho varrho sigma Sigma tau upsilon Upsilon phi varphi Phi chi psi omega Omega"):gmatch("%S+") do
  GREEK[name] = true
end
local function greek(_, _, c)
  return GREEK[c[1]] == true
end
-- "cdot" is always \cdot, never \dot{c} (as in Latex Suite, where cdot won)
local function not_c(_, _, c)
  return c[1] ~= "c"
end

return h.group("Accents", {
  -- trigger       output                     origin
  post("hat",      [[\hat{%1}]],              "default"),
  post("bar",      [[\bar{%1}]],              "default"),
  post("dot",      [[\dot{%1}]],              "default", { priority = -1, check = not_c }),
  post("ddot",     [[\ddot{%1}]],             "default", { priority = 1 }),
  post("tilde",    [[\tilde{%1}]],            "default"),
  post("und",      [[\underline{%1}]],        "default"),
  post("vec",      [[\vec{%1}]],              "default"),
  post(",.",       [[\mathbf{%1}]],           "default"),
  post(".,",       [[\mathbf{%1}]],           "default"),
  pat([[\(%a+),%.]], [[\boldsymbol{\%1}]],    "default", { name = "\\greek,.", check = greek, boundary = "none", len = 10, eg = "\\mu,." }),
  pat([[\(%a+)%.,]], [[\boldsymbol{\%1}]],    "default", { name = "\\greek.,", check = greek, boundary = "none", len = 10, eg = "\\mu.," }),

  math("hat",      [[\hat{<>}]],              "default"),
  math("bar",      [[\bar{<>}]],              "default"),
  math("dot",      [[\dot{<>}]],              "default", { priority = -1 }),
  math("ddot",     [[\ddot{<>}]],             "default"),
  math("cdot",     [[\cdot]],                 "default"),
  math("tilde",    [[\tilde{<>}]],            "default"),
  math("und",      [[\underline{<>}]],        "default"),
  math("vec",      [[\vec{<>}]],              "default"),
  math("pmod",     [[\pmod{<:n>}]],           "default", { desc = "Parenthesized modulo (\\pmod{n})" }),
})
