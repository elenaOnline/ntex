-- \lv — show the PDF. Where, is prefs.lua `pdf_viewer`:
--   "tdf"     a cmux split running tdf (terminal PDF viewer; reloads itself
--             when the PDF changes, keeps your page)            [default in cmux]
--   "browser" a cmux browser split (sharp, selectable text); reloaded after
--             every successful compile
--   "skim"    Skim.app, with SyncTeX (\lv jumps to the cursor's spot)
-- In Tern it is prefs.lua `tern_pdf_viewer` instead (default "browser"):
--   "browser" a Tern browser split: WebKit's own PDF view, sharp on Retina;
--             reloaded after every successful compile
--   "tdf"     a Tern split running tdf. Blurry: Tern tells programs the 1x
--             pixel size of the window, so tdf draws the page at half the
--             Retina resolution and Tern scales it up
--   "skim"    Skim.app
-- Elsewhere, or if cmux/Tern refuses, it falls back to Skim.
-- \ls always opens Skim (SyncTeX forward search).
local prefs = require("config.prefs")
local M = { panes = {} } -- pdf path -> { kind = "tdf"|"browser"|"tern"|"tern_browser", ref = "surface:N" | block id }

local function in_cmux()
  return (vim.env.CMUX_WORKSPACE_ID or vim.env.CMUX_SOCKET_PATH or vim.env.CMUX_SURFACE_ID) ~= nil
end

local function in_tern()
  return not in_cmux() and vim.env.TERM_PROGRAM == "tern" and vim.fn.executable("tern") == 1
end

local function tern(args)
  local res = vim.system(vim.list_extend({ "tern" }, args), { text = true }):wait(5000)
  return res.code == 0, (res.stdout or "") .. (res.stderr or "")
end

-- the Tern blocks of the shown tab: id -> block (focused, live, …)
local function tern_blocks()
  local ok, out = tern({ "ls", "--json" })
  local found = {}
  local okj, data = pcall(vim.json.decode, ok and out or "")
  if not okj or type(data) ~= "table" then
    return found
  end
  for _, s in ipairs(data.sessions or {}) do
    for _, t in ipairs(s.tabs or {}) do
      for _, b in ipairs(t.blocks or {}) do
        b.shown = s.shown and t.shown
        found[tostring(b.id)] = b
      end
    end
  end
  return found
end

-- `tern browser OP`: its answer's "ok" table, or nil
local function tern_browser(op)
  local ok, out = tern({ "browser", vim.json.encode(op) })
  local okj, res = pcall(vim.json.decode, ok and out or "")
  return okj and type(res) == "table" and type(res.ok) == "table" and res.ok or nil, out
end

-- reload a Tern browser block, then hand it back (automation marks it "driven")
local function tern_reload(block)
  tern_browser({ op = "nav", block = tonumber(block), go = "reload" })
  tern_browser({ op = "release", block = tonumber(block) })
end

local function tern_self()
  if vim.env.TERN_PANE and vim.env.TERN_PANE ~= "" then
    return vim.env.TERN_PANE
  end
  for id, b in pairs(tern_blocks()) do
    if b.shown and b.focused then
      return id
    end
  end
end

local function pdf_of(buf)
  return vim.api.nvim_buf_call(buf, function()
    local ok, out = pcall(vim.fn.eval, "b:vimtex.out()")
    if ok and type(out) == "string" and out ~= "" then
      return vim.fn.fnamemodify(out, ":p")
    end
    return vim.fn.expand("%:p:r") .. ".pdf"
  end)
end

local function cmux(args)
  local res = vim.system(vim.list_extend({ "cmux" }, args), { text = true }):wait(5000)
  return res.code == 0, (res.stdout or "") .. (res.stderr or "")
end

local function skim(msg)
  if msg then
    vim.notify("texnvim: " .. msg .. " — opening in Skim instead", vim.log.levels.WARN)
  end
  vim.cmd("VimtexView")
end

local function pane_alive(p)
  if p.kind == "tern_browser" then
    for _, b in ipairs((tern_browser({ op = "blocks" }) or {}).blocks or {}) do
      if tostring(b.block) == p.ref then
        return true
      end
    end
    return false
  elseif p.kind == "tern" then
    local b = tern_blocks()[p.ref]
    return b ~= nil and b.live ~= false and b.exited == vim.NIL
  end
  local ok = cmux({ "identify", "--surface", p.ref, "--no-caller" })
  return ok
end

local function no_viewer()
  if vim.env.TEXNVIM_NO_VIEWER == "1" then
    vim.notify("texnvim: viewer disabled (TEXNVIM_NO_VIEWER=1)", vim.log.levels.INFO)
    return true
  end
  return false
end

function M.view()
  if no_viewer() then
    return
  end
  local buf = vim.api.nvim_get_current_buf()
  local kind = prefs.pdf_viewer or "tdf"
  local tern_here = in_tern()
  if tern_here then
    kind = prefs.tern_pdf_viewer or "browser"
  end
  if kind == "skim" or not (in_cmux() or tern_here) then
    return skim()
  end
  local pdf = pdf_of(buf)
  if vim.fn.filereadable(pdf) == 0 then
    vim.notify("texnvim: no PDF yet — it appears after the first compile (pause typing, or Ctrl-s)", vim.log.levels.INFO)
    return
  end

  local p = M.panes[pdf]
  if p and pane_alive(p) then
    if p.kind == "browser" then
      cmux({ "browser", p.ref, "reload" })
    elseif p.kind == "tern_browser" then
      tern_reload(p.ref)
    end
    return
  end

  local ok, out
  if tern_here and kind == "browser" then
    local me = tern_self()
    if not me then
      return skim("couldn't find this Tern pane")
    end
    local res, err = tern_browser({ op = "open", owner = tonumber(me), url = "file://" .. pdf })
    if not (res and res.block) then
      return skim("Tern couldn't open a browser pane: " .. vim.trim(err))
    end
    local block = tostring(res.block)
    tern({ "dock", block, "right" }) -- it opens floating over this pane
    tern_browser({ op = "release", block = res.block })
    tern({ "focus", me })
    M.panes[pdf] = { kind = "tern_browser", ref = block }
    return
  elseif tern_here then
    local tdf = vim.fn.exepath("tdf")
    if tdf == "" then
      return skim("tdf isn't installed (brew install tdf)")
    end
    local me = tern_self()
    if not me then
      return skim("couldn't find this Tern pane")
    end
    ok, out = tern({ "split", me, "right", "--keep-open", "--json", "--", tdf, pdf })
    if not ok then
      return skim("Tern couldn't open a pane: " .. vim.trim(out))
    end
    tern({ "focus", me }) -- keep typing here, like cmux's --focus false
    local okj, res = pcall(vim.json.decode, out)
    if okj and type(res) == "table" and res.block then
      M.panes[pdf] = { kind = "tern", ref = tostring(res.block) }
    end
    return
  elseif kind == "browser" then
    ok, out = cmux({ "browser", "open-split", "file://" .. pdf, "--focus", "false" })
  else
    local tdf = vim.fn.exepath("tdf")
    if tdf == "" then
      return skim("tdf isn't installed (brew install tdf)")
    end
    ok, out = cmux({ "new-split", "right", "--command", tdf .. " " .. vim.fn.shellescape(pdf), "--focus", "false" })
  end
  if not ok then
    return skim("cmux couldn't open a pane: " .. vim.trim(out))
  end
  local ref = out:match("(surface:%d+)")
  if ref then
    M.panes[pdf] = { kind = kind, ref = ref }
  end
end

-- after a successful compile: a browser pane needs a reload (tdf reloads itself)
function M.on_compiled(buf)
  if vim.env.TEXNVIM_NO_VIEWER == "1" then
    return
  end
  local p = M.panes[pdf_of(buf)]
  if p and p.kind == "browser" then
    vim.system({ "cmux", "browser", p.ref, "reload" })
  elseif p and p.kind == "tern_browser" then
    tern_reload(p.ref)
  end
end

function M.setup()
  vim.api.nvim_create_autocmd("FileType", {
    group = vim.api.nvim_create_augroup("texnvim_pdfview", { clear = true }),
    pattern = "tex",
    callback = function(ev)
      vim.keymap.set("n", "<localleader>lv", M.view, { buffer = ev.buf, desc = "Show PDF (prefs.lua pdf_viewer)" })
      vim.keymap.set("n", "<localleader>ls", function()
        if not no_viewer() then
          vim.cmd("VimtexView")
        end
      end, { buffer = ev.buf, desc = "Show PDF in Skim (SyncTeX)" })
    end,
  })
  vim.api.nvim_create_autocmd("User", {
    group = "texnvim_pdfview",
    pattern = "VimtexEventCompileSuccess",
    callback = function()
      M.on_compiled(vim.api.nvim_get_current_buf())
    end,
  })
end

return M
