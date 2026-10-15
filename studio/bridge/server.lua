-- Ntex Studio's bridge: a long-lived headless ntex the studio talks to.
--
--   nvim --clean --headless -l studio/bridge/server.lua
--
-- Like tests/driver.lua, this *controller* Neovim starts the real config in a
-- child (`nvim --embed --headless -n -i NONE`, NVIM_APPNAME=texnvim) and types
-- into it one key at a time over RPC: every key is fully processed
-- (autosnippets included) before the next one is sent. No window, no viewer,
-- no compile, no language server, no reference rebuild.
--
-- Environment (set by the studio, io/src/bridge.rs):
--   NTEX_BRIDGE_DIR      a private temp folder; removed on quit. Holds
--                        snippets/ (the copy the child loads), work/ (the
--                        scratch buffers) and check/ (files being checked).
--   NTEX_BRIDGE_SOURCE   her real snippets folder (copied, never written)
--   NTEX_BRIDGE_APPNAME  the config (default texnvim)
--
-- Protocol: JSON lines. stdin gets requests, stdout gets answers (and one
-- {"event":"ready"} line once the child has loaded); stderr is a log.
--   {"id":1,"op":"list"}                          → {"id":1,"ok":true,"snippets":[…]}
--   {"id":2,"op":"check","name":"inbox.lua","text":"…"}
--                                                 → {"id":2,"ok":true} | {"id":2,"ok":false,"error":"inbox.lua:12: …"}
--   {"id":3,"op":"try",entry,ft,place,keys,tab,fills}
--                                                 → {"id":3,"ok":true,"result":{expanded,expanded_other,steps,…}}
--   {"id":4,"op":"refresh"}                       → {"id":4,"ok":true}
--   {"id":5,"op":"quit"}                          → {"id":5,"ok":true}, then exit
-- Any failure: {"id":n,"ok":false,"error":"…"}.
--
-- Self-heal: if a try sees a Lua error in the child (v:errmsg set by an
-- autocmd/mapping error, an ERROR-level vim.notify, anything mentioning
-- LuaSnip), its result still comes back ok:true with result.error set; then
-- {"event":"restart","reason":"…"} and the server exits (code 0). The studio
-- starts a fresh one and resends the requests it hasn't had answers to.

io.stdout:setvbuf("no")
local uv = vim.uv
local t_start = uv.hrtime()

local ROOT = assert(vim.env.NTEX_BRIDGE_DIR, "NTEX_BRIDGE_DIR is not set")
local SOURCE = assert(vim.env.NTEX_BRIDGE_SOURCE, "NTEX_BRIDGE_SOURCE is not set")
local APP = vim.env.NTEX_BRIDGE_APPNAME or "texnvim"
local COPY = ROOT .. "/snippets"
local WORK = ROOT .. "/work"
local CHECK = ROOT .. "/check"
local TRY_NAME = "zz_studio_try.lua"
local TRY_FILE = COPY .. "/tex/" .. TRY_NAME
local CURSOR = "‸"

local function log(...)
  io.stderr:write("bridge: " .. table.concat(vim.tbl_map(tostring, { ... }), " ") .. "\n")
end

local function send(obj)
  io.stdout:write(vim.json.encode(obj) .. "\n")
end

for _, d in ipairs({ COPY .. "/tex", WORK, CHECK }) do
  vim.fn.mkdir(d, "p")
end

local function read_file(p)
  local fd = io.open(p, "rb")
  if not fd then
    return nil
  end
  local s = fd:read("*a")
  fd:close()
  return s
end

local function write_file(p, s)
  local tmp = p .. ".part"
  local fd = assert(io.open(tmp, "wb"))
  fd:write(s)
  fd:close()
  assert(os.rename(tmp, p))
end

local function lua_files(dir)
  local out = {}
  for name, kind in vim.fs.dir(dir) do
    if (kind == "file" or kind == "link") and name:match("%.lua$") and name ~= TRY_NAME then
      out[name] = true
    end
  end
  return out
end

-- copy snippets/tex/*.lua into the private folder; returns the names that
-- changed (new or different) and the names that are gone from the source
local function sync_copy()
  local src_tex, dst_tex = SOURCE .. "/tex", COPY .. "/tex"
  local src, dst = lua_files(src_tex), lua_files(dst_tex)
  local changed, gone = {}, {}
  for name in pairs(src) do
    local s = read_file(src_tex .. "/" .. name)
    if s and read_file(dst_tex .. "/" .. name) ~= s then
      write_file(dst_tex .. "/" .. name, s)
      changed[#changed + 1] = name
    end
  end
  for name in pairs(dst) do
    if not src[name] then
      gone[#gone + 1] = name
    end
  end
  table.sort(changed)
  table.sort(gone)
  return changed, gone
end

sync_copy()

---------------------------------------------------------------------------
-- the child: the real config

local child_env = {
  NVIM_APPNAME = APP,
  TEXNVIM_SNIPPETS = COPY,
  TEXNVIM_NO_VIEWER = "1",
  TEXNVIM_NO_COMPILE = "1",
  TEXNVIM_NO_REFERENCE = "1",
  TEXNVIM_NO_LSP = "1", -- no texlab, harper-ls, copilot-language-server
  TEXNVIM_NO_WATCH = "1", -- this script reloads the copy itself
  TEXNVIM_IMAGES = "0",
  HOME = vim.env.HOME,
  PATH = vim.env.PATH,
  TMPDIR = vim.env.TMPDIR,
  TERM = "xterm-256color",
}

local child_dead = false
local chan = vim.fn.jobstart({ "nvim", "--embed", "--headless", "-n", "-i", "NONE" }, {
  rpc = true,
  env = child_env,
  clear_env = true,
  cwd = WORK,
  on_exit = function(_, code, signal)
    child_dead = true
    io.stderr:write(("bridge: the child nvim exited (code %s, signal %s)\n"):format(tostring(code), tostring(signal)))
  end,
})
assert(chan > 0, "could not start the child nvim")
local child_pid = vim.fn.jobpid(chan)

-- watchdog: a child that stops answering (a bug, a prompt) is killed rather
-- than hanging the studio; the request then fails and the studio restarts us
local deadline = nil
local watchdog = uv.new_timer()
watchdog:start(1000, 1000, function()
  if deadline and uv.now() > deadline then
    io.stderr:write("bridge: the child nvim did not answer for 20 s, killing it\n")
    uv.kill(child_pid, "sigkill")
    child_dead = true
    deadline = nil
  end
end)

local function rpc(method, ...)
  deadline = uv.now() + 20000
  local ok, res = pcall(vim.rpcrequest, chan, method, ...)
  deadline = nil
  if not ok then
    error(res, 0)
  end
  return res
end
local function lua(code, ...)
  return rpc("nvim_exec_lua", code, { ... })
end
local function sync()
  -- non-fast requests run only once typeahead is consumed; the second one
  -- also lets vim.schedule() callbacks queued by the first key run
  rpc("nvim_eval", "1")
  rpc("nvim_eval", "1")
end

-- Is the child still answering? on_exit only runs while the event loop does,
-- and this script spends its idle time blocked reading stdin, so a child that
-- died between requests is only noticed by asking it (2026-10-08: a dead child
-- turned every later request into "Invalid channel: 3").
local function child_alive()
  if child_dead then
    return false
  end
  local ok = pcall(vim.rpcrequest, chan, "nvim_eval", "1")
  if not ok then
    child_dead = true
  end
  return ok
end

local function cleanup()
  pcall(vim.rpcrequest, chan, "nvim_command", "qa!")
  pcall(vim.fn.jobstop, chan)
  pcall(vim.fn.jobwait, { chan }, 1000)
  if child_pid then
    pcall(uv.kill, child_pid, "sigkill")
  end
  vim.fn.delete(ROOT, "rf")
end

-- child-side helpers, installed once
lua([==[
  local B = {}
  _G.ntex_bridge = B
  B.expanded = {}
  -- every expansion, from LuaSnip's own event (the snippet being expanded is
  -- a copy that keeps snip.ntex and snip._source)
  vim.api.nvim_create_autocmd("User", {
    pattern = "LuasnipPreExpand",
    callback = function()
      local snip = require("luasnip").session.event_node
      if not snip then
        return
      end
      local m = snip.ntex or {}
      local src = snip._source or {}
      B.expanded[#B.expanded + 1] = {
        trigger = m.trig or snip.trigger or snip.name or "?",
        file = src.file and vim.fs.basename(src.file) or "?",
      }
    end,
  })
  -- Lua errors while typing: an error in an autocmd or mapping (LuaSnip's
  -- expansion runs in both) sets v:errmsg; LuaSnip and the config also report
  -- through vim.notify. Either may leave LuaSnip's state corrupt, so the
  -- controller restarts after a try that saw one. Wrapped once, here, after
  -- the config has loaded (whatever vim.notify is by then still runs).
  B.errors = {}
  local notify = vim.notify
  vim.notify = function(msg, level, opts)
    local s = tostring(msg)
    if (type(level) == "number" and level >= vim.log.levels.ERROR) or s:find("LuaSnip", 1, true) then
      B.errors[#B.errors + 1] = s
    end
    return notify(msg, level, opts)
  end
  function B.clear_errors()
    vim.v.errmsg = ""
    B.errors = {}
  end
  --- what went wrong since clear_errors(), or nil
  function B.take_errors()
    local out = {}
    -- only Lua errors: LuaSnip itself sets v:errmsg on every expansion with
    -- `silent! call repeat#set(…)` (E117 without vim-repeat)
    local em = vim.v.errmsg
    if em ~= "" and (em:find("E5108", 1, true) or em:find("[Ll]ua") or em:find("stack traceback", 1, true)) then
      out[#out + 1] = em
    end
    for _, e in ipairs(B.errors) do
      out[#out + 1] = e
    end
    B.clear_errors()
    return #out > 0 and table.concat(out, "\n") or nil
  end

  -- no undo files, no live-PDF saves of the scratch buffers
  vim.o.undofile = false
  pcall(vim.api.nvim_del_augroup_by_name, "texnvim_live_pdf")

  B.bufs = {}
  --- the scratch buffer for ft ("tex"|"markdown"), reused (VimTeX's syntax
  --- breaks after ~220 .tex buffers in one session); text with ‸; insert mode
  --- is entered by the controller
  function B.open(work, ft, lines, r, c)
    pcall(function() require("luasnip").unlink_current() end)
    local buf = B.bufs[ft]
    if not (buf and vim.api.nvim_buf_is_valid(buf)) then
      vim.cmd.edit(vim.fn.fnameescape(work .. "/try." .. (ft == "tex" and "tex" or "md")))
      buf = vim.api.nvim_get_current_buf()
      vim.b[buf].texnvim_template_done = true
      B.bufs[ft] = buf
    elseif vim.api.nvim_get_current_buf() ~= buf then
      vim.cmd("silent! buffer " .. buf)
    end
    vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
    vim.bo[buf].modified = false
    vim.api.nvim_win_set_cursor(0, { r, c })
    require("snips.context").invalidate()
    B.expanded = {}
    return #lines[r]
  end

  function B.state()
    local cur = vim.api.nvim_win_get_cursor(0)
    local mode = vim.fn.mode()
    local sel, sel_from = nil, nil
    if mode == "s" or mode == "v" then
      local a, b = vim.fn.getpos("v"), vim.fn.getpos(".")
      if a[2] > b[2] or (a[2] == b[2] and a[3] > b[3]) then
        a, b = b, a
      end
      sel = table.concat(vim.fn.getregion(a, b, { type = "v" }), "\n")
      sel_from = { a[2], a[3] - 1 }
    end
    return { lines = vim.api.nvim_buf_get_lines(0, 0, -1, false), row = cur[1], col = cur[2], mode = mode, sel = sel, sel_from = sel_from }
  end

  B.fields = {}
  function B.field_state(id)
    local f = assert(B.fields[id], "unknown field: " .. tostring(id))
    assert(vim.api.nvim_get_current_buf() == f.buf, "field is not current")
    local st = B.state()
    assert(#st.lines == 1, "a snippet field must stay on one line")
    local selected = vim.NIL
    if st.mode == "s" and st.sel_from then
      selected = { st.sel_from[2], st.sel_from[2] + #st.sel }
    end
    local state = {
      text = st.lines[1], cursor = st.col, selected = selected,
      in_snippet = require("luasnip").jumpable(1),
    }
    f.state = state
    return state
  end

  -- Save the active field's insert/selection position before switching. The
  -- LuaSnip session is buffer-local; do not unlink it on a field switch.
  function B.field_switch_needed(id)
    local buf = vim.api.nvim_get_current_buf()
    local target = B.fields[id]
    if target and target.buf == buf then return false end
    for other, f in pairs(B.fields) do
      if f.buf == buf then B.field_state(other); break end
    end
    return true
  end

  function B.field_activate(work, id, text, cursor)
    local f = B.fields[id]
    if not f then
      assert(text ~= nil, "unknown field: " .. tostring(id))
      local buf = vim.api.nvim_create_buf(false, true)
      vim.api.nvim_buf_set_name(buf, work .. "/field-" .. id .. ".tex")
      vim.bo[buf].buftype = "nofile"
      vim.bo[buf].bufhidden = "hide"
      vim.bo[buf].swapfile = false
      vim.bo[buf].undofile = false
      vim.b[buf].texnvim_template_done = true
      f = { buf = buf, state = { text = "", cursor = 0, selected = vim.NIL } }
      B.fields[id] = f
      vim.api.nvim_set_current_buf(buf)
      vim.bo[buf].filetype = "tex"
    else
      vim.api.nvim_set_current_buf(f.buf)
    end
    if text ~= nil then
      assert(not text:find("[\r\n]"), "a snippet field must have one line")
      pcall(function() require("luasnip").unlink_current() end)
      vim.api.nvim_buf_set_lines(f.buf, 0, -1, false, { text })
      -- Clamp to a UTF-8 byte boundary, including the insert position at EOL.
      cursor = math.max(0, math.min(cursor or #text, #text))
      while cursor > 0 and cursor < #text and text:byte(cursor + 1) >= 128 and text:byte(cursor + 1) < 192 do
        cursor = cursor - 1
      end
      f.state = { text = text, cursor = cursor, selected = vim.NIL }
      vim.bo[f.buf].modified = false
    end
    local st = f.state
    vim.api.nvim_win_set_cursor(0, { 1, st.cursor })
    require("snips.context").invalidate()
    if st.selected ~= vim.NIL and st.selected then
      local a, b = st.selected[1], st.selected[2]
      vim.api.nvim_win_set_cursor(0, { 1, a })
      vim.cmd("normal! v")
      local last = b - 1
      while last > a and st.text:byte(last + 1) >= 128 and st.text:byte(last + 1) < 192 do last = last - 1 end
      vim.api.nvim_win_set_cursor(0, { 1, last })
      return "<C-g>"
    end
    return st.cursor >= #st.text and #st.text > 0 and "A" or "i"
  end

  function B.field_close(id)
    local f = B.fields[id]
    if f then
      vim.api.nvim_buf_call(f.buf, function()
        pcall(function() require("luasnip").unlink_current() end)
      end)
      vim.api.nvim_buf_delete(f.buf, { force = true })
      B.fields[id] = nil
    end
  end

  --- load the try file beside her snippets (checked first)
  function B.load_try(path)
    local snips = require("snips")
    local ok, err = snips.check(path)
    if not ok then
      return tostring(err)
    end
    if not snips.reload_file(path) then
      return tostring(snips.errors[vim.fs.normalize(path)] or "did not load")
    end
    return nil
  end

  --- unload the try file's snippets: empty it, reload, delete it
  function B.unload_try(path)
    pcall(function() require("luasnip").unlink_current() end)
    if vim.uv.fs_stat(path) then
      local fd = io.open(path, "w")
      fd:write("return {}\n")
      fd:close()
      require("snips").reload_file(path)
      os.remove(path)
    end
    pcall(function() require("luasnip").clean_invalidated({ inv_limit = 100 }) end)
  end

  --- why the try snippet didn't fire (best effort), from its metadata and
  --- the context at the cursor
  function B.reason(trigger)
    local ok, res = pcall(function()
      local ls = require("luasnip")
      local mine
      for _, kind in ipairs({ "snippets", "autosnippets" }) do
        for _, s in ipairs(ls.get_snippets("tex", { type = kind })) do
          if s.ntex and not s.invalidated and s._source and s._source.file and vim.fs.basename(s._source.file) == "zz_studio_try.lua" then
            mine = mine or s
          end
        end
      end
      if not mine then
        return "the snippet didn't load"
      end
      local m = mine.ntex
      local ctx = require("snips.context").get()
      if not ctx then
        return nil
      end
      local cmod = require("snips.context")
      local in_math = cmod.strictly_in_math(ctx.mode)
      if m.where ~= "text" and not in_math then
        return "the cursor wasn't in maths"
      end
      if m.where == "text" and not ctx.mode.text then
        return "the cursor wasn't in text"
      end
      if m.ft and ctx.ft ~= m.ft then
        return "it only fires in ." .. (m.ft == "tex" and "tex" or "md") .. " files"
      end
      if m.line_rule then
        local line = vim.api.nvim_get_current_line():sub(1, vim.api.nvim_win_get_cursor(0)[2])
        local tr = trigger or ""
        if line:sub(-#tr) == tr and not require("snips.cond").line(m.line_rule, line, tr) then
          return ({ start = "it only fires at the start of a line", ["after text"] = "it only fires after other text on the line", ["list item"] = "it only fires in a list item" })[m.line_rule]
        end
      end
      return nil
    end)
    return ok and res or nil
  end
]==])

---------------------------------------------------------------------------
-- typing

--- split literal text into keys for nvim_input ("<" must be "<lt>")
local function literal_keys(text)
  local out = {}
  for ch in text:gmatch("[%z\1-\127\194-\244][\128-\191]*") do
    if ch == "<" then
      out[#out + 1] = "<lt>"
    elseif ch == "\n" then
      out[#out + 1] = "<CR>"
    elseif ch == "\t" then
      out[#out + 1] = "<Tab>"
    else
      out[#out + 1] = ch
    end
  end
  return out
end

local function press(keys)
  for _, k in ipairs(keys) do
    rpc("nvim_input", k)
    sync()
  end
end

--- nvim notation, with a sync after each token (as tests/driver.lua does).
local function key_tokens(keys)
  local out, i = {}, 1
  while i <= #keys do
    local key = keys:match("^<[%w%-]+>", i)
    if key then
      i = i + #key
    else
      key = assert(keys:match("^[%z\1-\127\194-\244][\128-\191]*", i), "invalid UTF-8 key")
      i = i + #key
      if key == "<" then key = "<lt>" end
    end
    out[#out + 1] = key
  end
  return out
end

local function split_marker(text)
  local lines = vim.split(text, "\n", { plain = true })
  for r, l in ipairs(lines) do
    local c = l:find(CURSOR, 1, true)
    if c then
      lines[r] = l:sub(1, c - 1) .. l:sub(c + #CURSOR)
      return lines, r, c - 1
    end
  end
  lines[#lines + 1] = ""
  return lines, #lines, 0
end

-- where to type: text around ‸ (the prefix/suffix are stripped from what is
-- shown, so a step reads "\frac{‸}{}" rather than "$\frac{‸}{}$")
local function place_text(place, ft)
  local md = ft == "markdown"
  if place == "inline" then
    return "$‸$"
  elseif place == "display" then
    return md and "$$\n‸\n$$" or "\\[\n‸\n\\]"
  elseif place == "text-start" then
    return "‸"
  elseif place == "list-item" then
    return md and "- ‸" or "\\begin{itemize}\n\\item ‸\n\\end{itemize}"
  elseif place == "visual" then
    return "$‸x+y$"
  else -- "text": after some words on the line
    return "Let ‸"
  end
end

local VISUAL_SAMPLE = "x+y"

local function open(ft, text, select_len)
  local lines, r, c = split_marker(text)
  rpc("nvim_input", "<Esc>")
  sync()
  local len = lua("return _G.ntex_bridge.open(...)", WORK, ft, lines, r, c)
  sync()
  if select_len then
    lua([[
      local r, c, n = ...
      vim.api.nvim_win_set_cursor(0, { r, c })
      vim.cmd("normal! v" .. (n > 1 and ((n - 1) .. "l") or ""))
    ]], r, c, select_len)
    sync()
    rpc("nvim_input", "<C-g>")
    sync()
  elseif c >= len and len > 0 then
    rpc("nvim_input", "A")
    sync()
  else
    rpc("nvim_input", "i")
    sync()
  end
end

--- the buffer as text with ‸ at the cursor (insert mode) or at the start of
--- the selection (select mode), the place's prefix/suffix stripped
local function render(st, prefix, suffix)
  local lines = vim.deepcopy(st.lines)
  local row, col = st.row, st.col
  if st.sel_from then
    row, col = st.sel_from[1], st.sel_from[2]
  end
  if st.mode == "i" or st.sel_from then
    local l = lines[row] or ""
    lines[row] = l:sub(1, col) .. CURSOR .. l:sub(col + 1)
  end
  local whole = table.concat(lines, "\n")
  if #whole >= #prefix + #suffix and whole:sub(1, #prefix) == prefix and (suffix == "" or whole:sub(-#suffix) == suffix) then
    whole = whole:sub(#prefix + 1, #whole - #suffix)
  end
  return whole
end

-- set by a try that saw a Lua error in the child: the main loop answers,
-- says {"event":"restart"} and exits, and the studio starts a fresh server
-- before the next request is handled
local restart_reason = nil

local function field(req)
  local id = assert(req.field, "missing field id")
  if req.op == "field_close" then
    lua("_G.ntex_bridge.field_close(...)", id)
    return { ok = true, field = id }
  end
  local reset = req.op == "field_open" or req.op == "field_set"
  if reset then
    assert(type(req.text) == "string" and not req.text:find("[\r\n]"), "a snippet field must have one line")
  end
  local switch = lua("return _G.ntex_bridge.field_switch_needed(...)", id)
  if switch or reset then
    rpc("nvim_input", "<Esc>")
    sync()
    local enter = lua("return _G.ntex_bridge.field_activate(...)", WORK, id, reset and req.text or nil, req.cursor)
    rpc("nvim_input", enter)
    sync()
  end
  lua("_G.ntex_bridge.clear_errors()")
  local escaped = false
  if req.op == "field_keys" then
    for _, key in ipairs(key_tokens(req.keys or "")) do
      rpc("nvim_input", key)
      sync()
      local mode = rpc("nvim_get_mode").mode
      if mode ~= "i" and mode ~= "s" then
        escaped = true
        lua([[pcall(function() require("luasnip").unlink_current() end)]])
        rpc("nvim_input", "<Esc>")
        sync()
        rpc("nvim_input", "a")
        sync()
      end
    end
  end
  local ok, st = pcall(lua, "return _G.ntex_bridge.field_state(...)", id)
  if not ok then
    -- A broken snippet (including one that violates the one-line field
    -- contract) must not strand the field in a corrupt child. Restore its
    -- last acknowledged text/cursor through the usual planned restart.
    restart_reason = tostring(st)
    return { ok = false, field = id, error = tostring(st) }
  end
  if escaped then st.in_snippet = false end
  local err = lua("return _G.ntex_bridge.take_errors()")
  if err and err ~= vim.NIL then
    restart_reason = tostring(err)
    return { ok = false, field = id, state = st, error = tostring(err) }
  end
  return { ok = true, field = id, state = st }
end

--- the first line of an error, without the "E5108: Error executing lua" and
--- file-path noise, for TryResult.error
local function short_error(s)
  s = tostring(s)
  local first = nil
  for l in s:gmatch("[^\n]+") do
    l = vim.trim(l)
    if l ~= "" and not l:match("^Error detected while processing") and not l:match("^stack traceback") then
      first = l
      break
    end
  end
  first = (first or s):gsub("^E%d+: ", ""):gsub("^Error executing [Ll]ua[^:]*: ", ""):gsub("^Lua: ", "")
  -- "…/zz_studio_try.lua:5: boom" → "boom (in the draft)"; other files by name
  local in_draft = first:match("^%S-" .. vim.pesc(TRY_NAME) .. ":%d+: (.*)$")
  if in_draft then
    first = in_draft .. " (in the draft)"
  else
    first = first:gsub("^%S-/([%w_%-%.]+%.lua:%d+: )", "%1")
  end
  return (first:sub(1, 300))
end

local function try(req)
  local ft = req.ft == "markdown" and "markdown" or "tex"
  local place = req.place or "inline"
  local entry = req.entry or ""
  local t0 = uv.hrtime()

  -- editing a snippet in one of her files: load that file without it while
  -- trying, so the old version doesn't shadow the draft (put back below)
  local hidden = nil
  if type(req.hide) == "table" and type(req.hide.file) == "string" and req.hide.file:match("^[%w_%-%.]+%.lua$")
    and type(req.hide.text) == "string" then
    hidden = COPY .. "/tex/" .. req.hide.file
    write_file(hidden, req.hide.text)
    local herr = lua("return _G.ntex_bridge.load_try(...)", hidden)
    if herr and herr ~= vim.NIL then
      write_file(hidden, read_file(SOURCE .. "/tex/" .. req.hide.file) or "return {}\n")
      lua("return _G.ntex_bridge.load_try(...)", hidden)
      hidden = nil
    end
  end
  local function unhide()
    if hidden then
      write_file(hidden, read_file(SOURCE .. "/tex/" .. req.hide.file) or "return {}\n")
      pcall(lua, "return _G.ntex_bridge.load_try(...)", hidden)
      hidden = nil
    end
  end

  write_file(TRY_FILE, table.concat({
    "-- Ntex Studio: a draft being tried (deleted right after)",
    'local h = require("snips.helpers")',
    "local math, math_tab, text, suffix, post, pat, vis, env = h.math, h.math_tab, h.text, h.suffix, h.post, h.pat, h.vis, h.env",
    'return h.group("Studio try", {',
    entry,
    "})",
    "",
  }, "\n"))
  local err = lua("return _G.ntex_bridge.load_try(...)", TRY_FILE)
  if err and err ~= vim.NIL then
    lua("_G.ntex_bridge.unload_try(...)", TRY_FILE)
    unhide()
    err = tostring(err):gsub("^.-" .. vim.pesc(TRY_NAME) .. ":", "line ")
    return { expanded = false, steps = {}, error = "the entry doesn't load: " .. err }
  end

  local steps = {} -- kept if the try fails half-way
  local ok, res = pcall(function()
    local text = place_text(place, ft)
    local k = text:find(CURSOR, 1, true)
    local prefix, suffix = text:sub(1, k - 1), text:sub(k + #CURSOR)
    local visual = place == "visual"
    if visual then
      suffix = suffix:sub(#VISUAL_SAMPLE + 1)
    end
    open(ft, text, visual and #VISUAL_SAMPLE or nil)
    lua("_G.ntex_bridge.clear_errors()")

    local function step(label)
      local st = lua("return _G.ntex_bridge.state()")
      local s = { keys = label, buffer = render(st, prefix, suffix) }
      if st.sel and st.sel ~= vim.NIL and st.mode == "s" then
        s.selected = st.sel
      end
      steps[#steps + 1] = s
    end

    press(literal_keys(req.keys or ""))
    -- why it wouldn't fire, judged with the trigger still at the cursor
    -- (before Tab turns it into spaces); only used if nothing expands
    local why = nil
    if #lua("return _G.ntex_bridge.expanded") == 0 then
      why = lua("return _G.ntex_bridge.reason(...)", req.keys)
    end
    if req.tab then
      press({ "<Tab>" })
    end
    step((req.keys or "") .. (req.tab and " ⇥" or ""))

    local exp = lua("return _G.ntex_bridge.expanded")
    local result = { expanded = #exp > 0, steps = steps }
    if #exp > 0 then
      result.expanded_trigger = exp[1].trigger
      result.expanded_file = exp[1].file
    end
    for _, e in ipairs(exp) do
      if e.file ~= TRY_NAME then
        result.expanded_other = ("%s from %s"):format(e.trigger, e.file)
        result.expanded_trigger = e.trigger
        result.expanded_file = e.file
        break
      end
    end
    if #exp == 0 then
      if why and why ~= vim.NIL then
        result.reason = why
      end
    else
      for _, fill in ipairs(req.fills or {}) do
        local fk = literal_keys(fill)
        fk[#fk + 1] = "<Tab>"
        press(fk)
        step((fill ~= "" and (fill .. " ") or "") .. "⇥")
      end
    end
    return result
  end)

  -- leave the buffer and LuaSnip clean for the next try, unload the draft
  pcall(rpc, "nvim_input", "<Esc>")
  pcall(sync)
  pcall(lua, "_G.ntex_bridge.unload_try(...)", TRY_FILE)
  unhide()
  -- a Lua error in the child while typing (or the try itself failing): the
  -- child may be corrupt (LuaSnip's state half-updated), so report it and
  -- restart before the next request
  local okc, seen = pcall(lua, "return _G.ntex_bridge.take_errors()")
  local problem = nil
  if not ok then
    problem = tostring(res)
  elseif not okc then
    problem = "the child nvim stopped answering: " .. tostring(seen)
  elseif seen and seen ~= vim.NIL then
    problem = tostring(seen)
  end
  if not ok then
    res = { expanded = false, steps = steps }
  end
  if problem then
    restart_reason = problem
    log("a Lua error while trying " .. vim.inspect(req.keys) .. ", restarting: " .. problem:gsub("\n", " | "):sub(1, 400))
    res.error = "a Lua error while typing: " .. short_error(problem)
  end
  res.ms = (uv.hrtime() - t0) / 1e6
  return res
end

---------------------------------------------------------------------------
-- the other requests

local function list()
  local raw = lua([[
    local out = {}
    for _, m in ipairs(require("snips").list()) do
      out[#out + 1] = {
        trigger = m.trig or m.trigger, -- as written in the file (post: "hat", not its pattern)
        pattern = m.pattern,
        kind = m.kind,
        ["where"] = m.where,
        how = m.how,
        body = m.body,
        file = m.file,
        line = m.line,
        origin = m.origin or "",
        group = m.group,
        line_rule = m.line_rule,
        ft = m.ft,
        priority = m.priority or 0,
        desc = m.desc,
      }
    end
    return out
  ]])
  return type(raw) == "table" and raw or {} -- {} encodes as []
end

local function check(name, text)
  if type(name) ~= "string" or not name:match("^[%w_%-%.]+%.lua$") then
    return false, "not a snippet file name: " .. tostring(name)
  end
  local path = CHECK .. "/" .. name
  write_file(path, text or "")
  local res = lua([[
    local ok, err = require("snips").check(...)
    return { ok = ok == true, err = err and tostring(err) or nil }
  ]], path)
  os.remove(path)
  if res.ok then
    return true
  end
  local err = tostring(res.err or "did not load")
  local fixed, n = err:gsub("^.-" .. vim.pesc(name) .. ":", name .. ":", 1)
  if n == 0 then
    fixed = name .. ": " .. err
  end
  return false, fixed
end

local function refresh()
  local changed, gone = sync_copy()
  local errors = lua([[
    local copy, changed, gone = ...
    local snips = require("snips")
    for _, name in ipairs(gone) do
      -- unload, then delete
      local p = copy .. "/tex/" .. name
      local fd = io.open(p, "w")
      if fd then
        fd:write("return {}\n")
        fd:close()
        snips.reload_file(p)
      end
      os.remove(p)
    end
    for _, name in ipairs(changed) do
      snips.reload_file(copy .. "/tex/" .. name)
    end
    local errs = {}
    for p, e in pairs(snips.errors) do
      errs[#errs + 1] = vim.fs.basename(p) .. ": " .. tostring(e)
    end
    return errs
  ]], COPY, changed, gone)
  return changed, gone, errors
end

---------------------------------------------------------------------------
-- start-up: wait for the config, warm VimTeX's syntax, say ready

local ok_start, start_err = pcall(function()
  local n = lua([[return #require("snips").list()]])
  open("tex", "$‸$")
  press(literal_keys("x"))
  rpc("nvim_input", "<Esc>")
  sync()
  open("markdown", "$‸$")
  rpc("nvim_input", "<Esc>")
  sync()
  send({ event = "ready", ms = (uv.hrtime() - t_start) / 1e6, snippets = n, pid = uv.os_getpid(), child = child_pid })
end)
if not ok_start then
  send({ event = "failed", error = tostring(start_err) })
  cleanup()
  os.exit(1)
end

---------------------------------------------------------------------------
-- the loop: one request per line until "quit" or end of input

local function handle(req)
  local op = req.op
  if op == "list" then
    return { ok = true, snippets = list() }
  elseif op == "check" then
    -- ok = the check ran; loads = the file would load (a load error is an
    -- answer, not a bridge failure)
    local ok, err = check(req.name, req.text)
    return { ok = true, loads = ok, error = (not ok) and err or nil }
  elseif op == "try" then
    return { ok = true, result = try(req) }
  elseif op == "refresh" then
    local changed, gone, errors = refresh()
    return { ok = true, changed = changed, gone = gone, errors = errors }
  elseif op == "ping" then
    return { ok = true }
  elseif op == "field_open" or op == "field_keys" or op == "field_set" or op == "field_close" then
    return field(req)
  end
  return { ok = false, error = "unknown op: " .. tostring(op) }
end

local exit_code = 0
for line in io.stdin:lines() do
  if line:match("%S") then
    local okj, req = pcall(vim.json.decode, line)
    if not okj or type(req) ~= "table" then
      send({ ok = false, error = "bad request: " .. line:sub(1, 200) })
    elseif req.op == "quit" then
      send({ id = req.id, ok = true })
      break
    else
      -- a dead child can't answer: leave this request unanswered and ask for
      -- a fresh server; the studio sends it again there
      if not child_alive() then
        log("the child nvim is gone; asking for a restart")
        send({ event = "restart", reason = "the hidden ntex stopped" })
        break
      end
      local okh, res = pcall(handle, req)
      if not okh and not child_alive() then
        log("the child nvim died during " .. tostring(req.op) .. ": " .. tostring(res):sub(1, 300))
        send({ event = "restart", reason = "the hidden ntex stopped" })
        break
      end
      if not okh then
        res = { ok = false, error = tostring(res) }
      end
      res.id = req.id
      send(res)
      -- a child that saw a Lua error may be corrupt: a planned restart (the
      -- studio requeues what we haven't answered and starts a fresh server)
      if restart_reason then
        send({ event = "restart", reason = restart_reason:sub(1, 500) })
        break
      end
      -- a dead child can't answer anything again: exit, the studio restarts us
      if vim.fn.jobwait({ chan }, 0)[1] ~= -1 then
        log("the child nvim exited")
        exit_code = 1
        break
      end
    end
  end
end

cleanup()
os.exit(exit_code)
