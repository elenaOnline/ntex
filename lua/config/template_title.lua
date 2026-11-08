-- The {{TITLE}} a new .tex file gets from its name: _ and - become spaces, and
-- LaTeX's special characters are escaped, so a name like "fluids & waves"
-- or "50% done" can't break the first compile.
local escapes = {
  ["\\"] = "\\textbackslash{}",
  ["~"] = "\\textasciitilde{}",
  ["^"] = "\\textasciicircum{}",
}

return function(name)
  local spaced = name:gsub("[_-]", " ")
  return (spaced:gsub("[\\&%%%$#{}~^]", function(c)
    return escapes[c] or ("\\" .. c)
  end))
end
