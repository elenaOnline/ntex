-- Conceal (pretty symbols).
--   .tex : VimTeX's syntax conceal (\alpha → α, \mathbb{R} → ℝ, …)
--   .md  : a small decoration provider using Latex Suite's symbol tables
--          (tools/conceal_maps.json, from its conceal_maps.ts), applied
--          only inside $…$ / $$…$$.
-- The line under the cursor always shows the source (concealcursor="").
-- Start state: prefs.lua `conceal`. Toggle: <F2> (normal or insert) or :Conceal.

local api = vim.api
local mdmath = require("snips.mdmath")

local M = {}

local ns = api.nvim_create_namespace("ntex_conceal")
M.enabled = false

local maps
local function load_maps()
  if maps then
    return maps
  end
  local path = vim.fn.stdpath("config") .. "/tools/conceal_maps.json"
  local ok, data = pcall(function()
    return vim.json.decode(table.concat(vim.fn.readfile(path), "\n"))
  end)
  maps = { cmd = {}, font = {}, sup = {}, sub = {} }
  if not ok then
    return maps
  end
  for k, v in pairs(data.cmd_symbols or {}) do
    maps.cmd[k] = v
  end
  for k, v in pairs(data.greek or {}) do
    maps.cmd[k] = v
  end
  maps.font.mathbb = data.mathbb or {}
  maps.font.mathcal = data.mathscrcal or {}
  maps.font.mathscr = data.mathscrcal or {}
  maps.font.mathfrak = data.mathfrak or {}
  maps.sup = data.map_super or {}
  maps.sub = data.map_sub or {}
  return maps
end

local function mark(buf, row, s, e, char)
  pcall(api.nvim_buf_set_extmark, buf, ns, row, s, { end_col = e, conceal = char, ephemeral = true })
end

local function conceal_span(buf, row, line, s, e)
  local m = load_maps()
  local seg = line:sub(s + 1, e)
  local i = 1
  while i <= #seg do
    local c = seg:sub(i, i)
    if c == "\\" then
      local name = seg:match("^\\(%a+)", i)
      if name then
        local stop = i + #name
        local font = m.font[name]
        if font then
          local letter = seg:match("^{(%a)}", stop + 1)
          if letter and font[letter] then
            mark(buf, row, s + i - 1, s + stop + 3, font[letter])
            i = stop + 4
            goto continue
          end
        end
        local sym = m.cmd[name]
        -- only if the macro name ends here (not a prefix of a longer word)
        if sym and not seg:sub(stop + 1, stop + 1):match("%a") then
          mark(buf, row, s + i - 1, s + stop, sym)
        end
        i = stop + 1
      else
        i = i + 2
      end
    elseif c == "^" or c == "_" then
      local map = c == "^" and m.sup or m.sub
      local braced = seg:match("^{(.)}", i + 1)
      if braced and map[braced] then
        mark(buf, row, s + i - 1, s + i + 3, map[braced])
        i = i + 4
      else
        local ch = seg:sub(i + 1, i + 1)
        if ch ~= "" and map[ch] and ch ~= "{" and ch ~= "\\" then
          mark(buf, row, s + i - 1, s + i + 1, map[ch])
          i = i + 2
        else
          i = i + 1
        end
      end
    else
      i = i + 1
    end
    ::continue::
  end
end

api.nvim_set_decoration_provider(ns, {
  on_win = function(_, _, buf)
    return M.enabled and vim.bo[buf].filetype == "markdown"
  end,
  on_line = function(_, _, buf, row)
    local line = api.nvim_buf_get_lines(buf, row, row + 1, false)[1]
    if not line or not line:find("[\\^_]") then
      return
    end
    local ok, spans = pcall(mdmath.spans, buf, row, line)
    if not ok then
      return
    end
    for _, sp in ipairs(spans) do
      conceal_span(buf, row, line, sp[1], sp[2])
    end
  end,
})

M.TEX_CONCEAL = {
  accents = 1, ligatures = 1, cites = 1, fancy = 1, texTabularChar = 1, spacing = 1, greek = 1,
  math_bounds = 1, math_delimiters = 1, math_fracs = 1, math_super_sub = 1, math_symbols = 1,
  sections = 0, styles = 1,
}

--- g:vimtex_syntax_conceal for the given state (VimTeX reads it whenever it
--- (re)loads tex syntax).
function M.tex_option(enabled)
  local t = {}
  for k, v in pairs(M.TEX_CONCEAL) do
    t[k] = enabled and v or 0
  end
  local okp, prefs = pcall(require, "config.prefs")
  if okp and prefs.conceal_scripts == false then
    t.math_super_sub = 0
  end
  return t
end

local function apply_tex(enabled)
  vim.g.vimtex_syntax_conceal = M.tex_option(enabled)
  for _, buf in ipairs(api.nvim_list_bufs()) do
    if api.nvim_buf_is_loaded(buf) and vim.bo[buf].filetype == "tex" then
      api.nvim_buf_call(buf, function()
        vim.cmd("syntax clear")
        vim.bo[buf].syntax = "tex"
      end)
    end
  end
end

function M.set(enabled, quiet)
  M.enabled = enabled
  apply_tex(enabled)
  vim.cmd("redraw!")
  if not quiet then
    vim.notify("Conceal " .. (enabled and "on" or "off"), vim.log.levels.INFO)
  end
end

function M.toggle()
  M.set(not M.enabled)
end

return M
