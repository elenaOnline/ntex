-- When may a snippet fire? Used as LuaSnip `condition`s by lua/snips/helpers.lua.
--
-- Two questions, cheapest first:
--   1. boundary: is the trigger at a place where it can't be part of something
--      else? (the anti-mangling rules, below)
--   2. where: maths / inline / display / text, from lua/snips/context.lua.
--
-- Boundary rules (the reason `\subset`, `\approx`, `\emptyset` type as written):
--   "word"   (math, math_tab, text, env, pat)
--            - never right after a backslash: \sum, \sub… are commands;
--            - a trigger that starts with a letter never right after a letter:
--              `sub` does not fire in `psub`, `xx` not in `\approx`.
--              prefs.lua latex.fire_after_letters = true relaxes this to
--              "not inside a \command name" (Auu → A\cup again).
--   "suffix" (suffix, post, and the auto subscript)
--            may follow a letter (xsr → x^{2}, xhat → \hat{x}, x2 → x_{2}),
--            but not when that letter ends a \command name (\hbar, \alpha2).
--   "none"   no check (triggers that start with a backslash themselves).
local context = require("snips.context")
local settings = require("snips.settings")

local M = {}

local BS = 92 -- "\"

local function is_letter(b)
  return b and ((b >= 65 and b <= 90) or (b >= 97 and b <= 122))
end

--- Is the byte at `pos` (1-based) preceded by an odd number of backslashes,
--- i.e. is it the first character of a command name?
function M.after_backslash(line, pos)
  local n, k = 0, pos - 1
  while k >= 1 and line:byte(k) == BS do
    n = n + 1
    k = k - 1
  end
  return n % 2 == 1
end

--- Is the letter at `pos` (1-based) part of a \command name? Walks back over
--- letters; a command if that run starts right after a (non-escaped) backslash.
function M.in_command(line, pos)
  local k = pos
  while k > 1 and is_letter(line:byte(k - 1)) do
    k = k - 1
  end
  return M.after_backslash(line, k)
end

--- 1-based position where the matched trigger starts in line_to_cursor.
local function start_of(line, match)
  return #line - #match + 1
end

--- boundary check for a trigger `match` ending at the cursor.
--- kind: "word" | "suffix" | "none"; lead: offset of the trigger word inside
--- the match (post-fix patterns capture a letter first: lead = 1).
function M.boundary(kind, line, match, lead)
  if kind == "none" then
    return true
  end
  local from = start_of(line, match) + (lead or 0)
  local first = line:byte(from)
  if kind == "suffix" then
    if is_letter(first) and is_letter(line:byte(from - 1)) then
      return not M.in_command(line, from - 1)
    end
    return not M.after_backslash(line, from)
  end
  -- word
  if M.after_backslash(line, from) then
    return false
  end
  if is_letter(first) and is_letter(line:byte(from - 1)) then
    if settings.get().fire_after_letters then
      return not M.in_command(line, from - 1)
    end
    return false
  end
  return true
end

-- Arguments in text (not maths) where text snippets like mk/dm must stay off:
-- file names, labels, environment names, package names …
local TEXT_OFF_ARGS = {
  begin = true, ["end"] = true, label = true, ref = true, eqref = true, cref = true, Cref = true,
  autoref = true, cite = true, citep = true, citet = true, usepackage = true, documentclass = true,
  input = true, include = true, includegraphics = true, url = true, href = true, newcommand = true,
  renewcommand = true, newenvironment = true, bibliography = true, bibliographystyle = true,
}

local function in_text_off_arg(line_to_cursor)
  local name = line_to_cursor:match("\\(%a+)%*?%b[]{[^{}]*$") or line_to_cursor:match("\\(%a+)%*?{[^{}]*$")
  return name ~= nil and TEXT_OFF_ARGS[name] == true
end

--- where: "math" | "inline" | "display" | "text"
function M.where(where, ctx, line)
  local mode = ctx.mode
  if mode.comment then
    return false
  end
  if where == "text" then
    return mode.text and not (ctx.ft == "tex" and in_text_off_arg(line))
  end
  if not context.strictly_in_math(mode) then
    return false
  end
  if where == "inline" then
    return mode.inlineMath == true
  elseif where == "display" then
    return mode.blockMath == true or mode.codeMath == true
  end
  return true
end

--- line: extra rules about the text before the trigger on this line
---   "start"      only spaces before it
---   "after text" something other than spaces before it
---   "list item"  the line is a list item: `- `, `* `, `+ `, `1. `, `1) ` (after spaces or `>`)
function M.line(rule, line, match)
  local before = line:sub(1, #line - #match)
  if rule == "start" then
    return before:match("^%s*$") ~= nil
  elseif rule == "after text" then
    return before:match("%S") ~= nil
  elseif rule == "list item" then
    return M.list_marker(before) ~= nil
  end
  return true
end

--- For a line like "  - some text", return marker ("-"), the spaces after it
--- and everything before the marker.
function M.list_marker(before)
  local lead, marker, ws = before:match("^([ \t]*>*[ \t]*)(%d+[.)])([ \t]+)")
  if not marker then
    lead, marker, ws = before:match("^([ \t]*>*[ \t]*)([-*+])([ \t]+)")
  end
  if marker then
    return marker, ws, lead
  end
end

return M
