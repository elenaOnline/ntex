-- macOS text-editing keys, the way they behave in any Mac text field:
--   Option-Left       start of the previous word
--   Option-Right      end of the next word
--   Option-Backspace  delete back to the start of the previous word
-- Words are letters/digits/underscore (plus ' ’ . between letters or digits, so
-- "don't" and "3.14" are one word); everything else — spaces, \ { } $ ^ _ … —
-- separates words, and line breaks are crossed like in TextEdit.
-- Works in insert, normal and visual mode; on the : command line it maps to
-- Vim's own word keys. Ghostty (and so cmux) sends Option-Left/Right as
-- Esc-b / Esc-f by default (its built-in `alt+arrow_left=esc:b` keybinds),
-- which Neovim reads as <M-b> / <M-f>; other terminals send <M-Left> etc.
-- Both are mapped. Option-Backspace arrives as <M-BS>.
local M = {}
local api = vim.api

local function chars(line)
  -- list of { byte_start (0-based), char }
  local out, i = {}, 1
  while i <= #line do
    local b = line:byte(i)
    local len = b < 0x80 and 1 or b < 0xE0 and 2 or b < 0xF0 and 3 or 4
    out[#out + 1] = { i - 1, line:sub(i, i + len - 1) }
    i = i + len
  end
  return out
end

local function is_word(ch)
  return ch ~= nil and (ch:match("^[%w_]$") ~= nil or #ch > 1 and ch ~= "’")
end

local function is_mid(ch)
  return ch == "'" or ch == "’" or ch == "."
end

-- index of the char at byte col (1-based into cs), or #cs+1 at end of line
local function index_at(cs, col)
  for i, c in ipairs(cs) do
    if c[1] >= col then
      return i
    end
  end
  return #cs + 1
end

local function word_at(cs, i)
  local c = cs[i] and cs[i][2]
  if is_word(c) then
    return true
  end
  return is_mid(c) and is_word(cs[i - 1] and cs[i - 1][2]) and is_word(cs[i + 1] and cs[i + 1][2])
end

--- position after moving one macOS word forward from (row 1-based, col byte)
function M.forward(buf, row, col)
  local nrows = api.nvim_buf_line_count(buf)
  local phase = "skip_space"
  while true do
    local line = api.nvim_buf_get_lines(buf, row - 1, row, false)[1] or ""
    local cs = chars(line)
    local i = index_at(cs, col)
    while i <= #cs do
      local w = word_at(cs, i)
      if phase == "skip_space" and w then
        phase = "in_word"
      elseif phase == "in_word" and not w then
        return row, cs[i][1]
      end
      i = i + 1
    end
    if phase == "in_word" or row >= nrows then
      return row, #line
    end
    row, col = row + 1, 0
  end
end

--- position after moving one macOS word backward
function M.backward(buf, row, col)
  local phase = "skip_space"
  while true do
    local line = api.nvim_buf_get_lines(buf, row - 1, row, false)[1] or ""
    local cs = chars(line)
    local i = index_at(cs, col) - 1
    while i >= 1 do
      local w = word_at(cs, i)
      if phase == "skip_space" and w then
        phase = "in_word"
      elseif phase == "in_word" and not w then
        return row, cs[i + 1][1]
      end
      i = i - 1
    end
    if phase == "in_word" or row <= 1 then
      return row, 0
    end
    row = row - 1
    col = #(api.nvim_buf_get_lines(buf, row - 1, row, false)[1] or "")
  end
end

local function cursor()
  local pos = api.nvim_win_get_cursor(0)
  return pos[1], pos[2]
end

local function normal_like()
  local m = api.nvim_get_mode().mode
  return not (m:sub(1, 1) == "i" or m:sub(1, 1) == "R")
end

-- in normal/visual mode the cursor sits *on* a char: "end of word" is the
-- word's last char, not the position after it
local function set(row, col, forward)
  if forward and normal_like() then
    local line = api.nvim_buf_get_lines(0, row - 1, row, false)[1] or ""
    if col > 0 and col >= #line then
      col = #line - 1
    elseif col > 0 then
      col = col - 1
      while col > 0 and bit.band(line:byte(col + 1), 0xC0) == 0x80 do
        col = col - 1
      end
    end
  end
  api.nvim_win_set_cursor(0, { row, math.max(col, 0) })
end

function M.word_right()
  local r, c = cursor()
  if normal_like() then
    -- normal mode: start from *after* the char under the cursor
    local line = api.nvim_get_current_line()
    local cs = chars(line)
    local i = index_at(cs, c)
    c = cs[i + 1] and cs[i + 1][1] or #line
  end
  local nr, nc = M.forward(0, r, c)
  set(nr, nc, true)
end

function M.word_left()
  local r, c = cursor()
  local nr, nc = M.backward(0, r, c)
  set(nr, nc, false)
end

function M.delete_word_left()
  local r, c = cursor()
  local nr, nc = M.backward(0, r, c)
  if nr == r and nc == c then
    return
  end
  api.nvim_buf_set_text(0, nr - 1, nc, r - 1, c, { "" })
  api.nvim_win_set_cursor(0, { nr, nc })
end

-- ⌘C / ⌘X / ⌘V, the way they work in any Mac text field. Only keys Vim
-- itself never uses are added; y, d, p, x … and the clipboard setting are as
-- before.
--   ⌘C  copy the selection (mouse, Shift-arrows, v / V) to the Mac clipboard;
--       the selection stays, as in any Mac app
--   ⌘X  cut the selection to the Mac clipboard
--   ⌘V  pasted by the terminal (cmux), so it works in every mode already; the
--       one change: pasting over a selection no longer puts the replaced text
--       on the clipboard, so ⌘V twice pastes the same thing twice
-- For ⌘C to reach Neovim, cmux 0.64.x needs `keybind = super+c=unbind` in
-- ~/Library/Application Support/com.cmuxterm.app/config.ghostty (not needed
-- from cmux 0.65.1).
function M.setup_clipboard()
  local o = { silent = true }
  local map = vim.keymap.set
  map("x", "<D-c>", '"+ygv', vim.tbl_extend("force", o, { desc = "macOS: copy" }))
  map("s", "<D-c>", '<C-g>"+ygv<C-g>', vim.tbl_extend("force", o, { desc = "macOS: copy" }))
  map("x", "<D-x>", '"+d', vim.tbl_extend("force", o, { desc = "macOS: cut" }))
  map("s", "<D-x>", '<C-g>"+d', vim.tbl_extend("force", o, { desc = "macOS: cut" }))

  -- Neovim's paste handler deletes a selection before pasting over it, and
  -- that delete lands on the clipboard (clipboard = unnamedplus). Put the
  -- registers back once the selection is gone.
  local paste = vim.paste
  vim.paste = function(lines, phase)
    local mode = vim.api.nvim_get_mode().mode
    if phase > 1 or not mode:find("^[vVsS\22\19]") then
      return paste(lines, phase)
    end
    local saved = {}
    for _, r in ipairs({ "-", "+", '"' }) do
      saved[#saved + 1] = { r, vim.fn.getreginfo(r) }
    end
    local ok = paste(lines, phase)
    for _, s in ipairs(saved) do
      pcall(vim.fn.setreg, s[1], s[2])
    end
    return ok
  end
end

-- ⌘Z / ⌘⇧Z undo and redo, as in any Mac app. u and Ctrl-R are unchanged.
-- In insert mode you stay in insert mode; <C-g>u first closes the text typed
-- so far as its own undo step, so ⌘Z takes back what was typed since the last
-- break, not the whole insert session. With a selection, ⌘Z drops the
-- selection and undoes.
function M.setup_undo()
  local o = { silent = true }
  local map = vim.keymap.set
  local undo = vim.tbl_extend("force", o, { desc = "macOS: undo" })
  local redo = vim.tbl_extend("force", o, { desc = "macOS: redo" })
  map("n", "<D-z>", "<Cmd>undo<CR>", undo)
  map("i", "<D-z>", "<C-g>u<Cmd>undo<CR>", undo)
  map({ "x", "s" }, "<D-z>", "<Esc>u", undo)
  for _, lhs in ipairs({ "<D-S-z>", "<D-Z>" }) do
    map("n", lhs, "<Cmd>redo<CR>", redo)
    map("i", lhs, "<Cmd>redo<CR>", redo)
    map({ "x", "s" }, lhs, "<Esc><C-r>", redo)
  end
end

function M.setup()
  M.setup_clipboard()
  M.setup_undo()
  local o = { silent = true }
  for _, lhs in ipairs({ "<M-Left>", "<A-Left>", "<M-b>" }) do
    vim.keymap.set({ "i", "n", "x" }, lhs, M.word_left, vim.tbl_extend("force", o, { desc = "macOS: word left" }))
    vim.keymap.set("c", lhs, "<S-Left>", { desc = "macOS: word left" })
  end
  for _, lhs in ipairs({ "<M-Right>", "<A-Right>", "<M-f>" }) do
    vim.keymap.set({ "i", "n", "x" }, lhs, M.word_right, vim.tbl_extend("force", o, { desc = "macOS: word right" }))
    vim.keymap.set("c", lhs, "<S-Right>", { desc = "macOS: word right" })
  end
  for _, lhs in ipairs({ "<M-BS>", "<A-BS>", "<M-C-H>" }) do
    vim.keymap.set({ "i", "n" }, lhs, M.delete_word_left, vim.tbl_extend("force", o, { desc = "macOS: delete word left" }))
    vim.keymap.set("c", lhs, "<C-w>", { desc = "macOS: delete word left" })
  end
end

return M
