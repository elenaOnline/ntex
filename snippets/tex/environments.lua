-- Environments: \begin{…}…\end{…}, matrices, cases, identity matrices.
--
--   env("pmat", "pmatrix", "default")  pmat → \begin{pmatrix} … \end{pmatrix}:
--     on separate lines in display maths, on one line in inline maths.
--   math(..., { where = "display" }) only in display maths ("inline": only in $…$).
--   <1> twice = one tabstop typed in two places (the environment name).
--   Full vocabulary: snippets/README.md. Save the file: it reloads.
local h = require("snips.helpers")
local math, env, pat = h.math, h.env, h.pat

-- iden3 → a 3×3 identity matrix
local function identity(c)
  local n = tonumber(c[1]) or 0
  local rows = {}
  for r = 1, n do
    local row = {}
    for col = 1, n do
      row[col] = (r == col) and "1" or "0"
    end
    rows[r] = table.concat(row, " & ")
  end
  return "\\begin{pmatrix}\n" .. table.concat(rows, " \\\\\n") .. "\n\\end{pmatrix}"
end

return h.group("Environments", {
  -- trigger       output                                      origin
  math("beg", [[
\begin{<1>}
	<2>
\end{<1>}]],                                                  "yours", { where = "display" }),
  math("beg",      [[\begin{<1>} <2> \end{<1>}]],              "yours", { where = "inline" }),

  env("pmat",      "pmatrix",    "default"),
  env("bmat",      "bmatrix",    "default"),
  env("Bmat",      "Bmatrix",    "default"),
  env("vmat",      "vmatrix",    "default"),
  env("Vmat",      "Vmatrix",    "default"),
  env("matrix",    "matrix",     "default"),
  env("cases",     "cases",      "default"),
  env("align",     "align",      "default"),
  env("array",     "array",      "default"),

  pat("iden(%d)",  identity,     "default", { name = "iden‹n›", eg = "iden3", desc = "N x N identity matrix" }),
})
