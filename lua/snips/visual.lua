-- Visual snippets: select text in maths, type a key, the selection is wrapped
-- (snippets/tex/visual.lua: vis("S", [[\sqrt{ %V }]]) and friends).
--
-- You select with the mouse or Shift+arrows, which gives *select mode* in
-- this config (what LuaSnip placeholders are too). Typing a visual key in
-- select mode inside maths expands that snippet around the selection (as
-- LuaSnip's LS_SELECT_RAW); any other key, or outside maths, replaces the
-- selection as usual. From ordinary visual mode (v), press <C-g> first.
-- The keys are read from the loaded snippet files, so adding a vis(...) line
-- and saving is enough.

local api = vim.api
local ls = require("luasnip")
local context = require("snips.context")
local settings = require("snips.settings")

local M = {}

M.by_key = {} -- key → list of visual snippets (highest priority first)
local mapped = {}

local function selection()
  local mode = vim.fn.mode()
  local vtype = ({ s = "v", S = "V", ["\19"] = "\22" })[mode]
  if not vtype or vtype == "\22" then
    return nil
  end
  local p1, p2 = vim.fn.getpos("v"), vim.fn.getpos(".")
  local regions = vim.fn.getregionpos(p1, p2, { type = vtype, exclusive = vim.o.selection == "exclusive" })
  if #regions == 0 then
    return nil
  end
  local first, last = regions[1], regions[#regions]
  local srow, scol = first[1][2] - 1, first[1][3] - 1
  local erow, ecol = last[2][2] - 1, last[2][3]
  local eline = api.nvim_buf_get_lines(0, erow, erow + 1, false)[1] or ""
  if vtype == "V" then
    scol = 0
    ecol = #eline
  end
  if ecol > #eline then
    ecol = #eline
  end
  local lines = api.nvim_buf_get_text(0, srow, scol, erow, ecol, {})
  return { from = { srow, scol }, to = { erow, ecol }, lines = lines }
end

function M.handle(key)
  local sel = selection()
  local snips = M.by_key[key]
  if sel and snips and settings.get().snippets_enabled and table.concat(sel.lines, "\n") ~= "" then
    -- the maths check is made at the end of the selection
    local ctx = context.get_at(api.nvim_get_current_buf(), sel.to[1], sel.to[2])
    if ctx and context.strictly_in_math(ctx.mode) then
      local snip = snips[1]
      vim.o.undolevels = vim.o.undolevels
      ls.snip_expand(snip, {
        clear_region = { from = sel.from, to = sel.to },
        pos = sel.from,
        expand_params = {
          env_override = { LS_SELECT_RAW = sel.lines, NTEX_INLINE = ctx.mode.inlineMath and "1" or "" },
        },
      })
      return
    end
  end
  -- not a visual snippet here: behave like normal select-mode typing
  api.nvim_feedkeys(key, "n", false)
end

--- (Re)read the visual snippets from LuaSnip and map their keys in select mode.
function M.refresh()
  local by_key = {}
  local seen = {}
  for _, ft in ipairs({ "tex" }) do
    for _, snip in ipairs(ls.get_snippets(ft, { type = "snippets" })) do
      local m = snip.ntex
      if m and m.kind == "visual" and not snip.invalidated and not seen[snip] then
        seen[snip] = true
        by_key[m.trig] = by_key[m.trig] or {}
        table.insert(by_key[m.trig], snip)
      end
    end
  end
  for _, list in pairs(by_key) do
    table.sort(list, function(a, b)
      return (a.priority or 0) > (b.priority or 0)
    end)
  end
  M.by_key = by_key
  for key in pairs(mapped) do
    if not by_key[key] then
      pcall(vim.keymap.del, "s", key)
      mapped[key] = nil
    end
  end
  for key in pairs(by_key) do
    if not mapped[key] then
      vim.keymap.set("s", key, function()
        M.handle(key)
      end, { desc = "ntex visual snippet " .. key })
      mapped[key] = true
    end
  end
end

return M
