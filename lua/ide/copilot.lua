-- GitHub Copilot ghost text in .tex and .md, with Neovim 0.12's built-in
-- inline completion (vim.lsp.inline_completion) and GitHub's own language
-- server (@github/copilot-language-server, installed locally in
-- ~/.local/share/texnvim/copilot; see README "IDE features").
--
--   Alt-l          accept the grey suggestion (Option-l in cmux arrives as
--                  Esc l, which Neovim reads as Alt-l too)
--   Alt-] / Alt-[  next / previous suggestion
--   Space a        Copilot on / off (says which). Off stops the server, so
--                  nothing is sent to GitHub.
--   Tab            never: Tab belongs to snippets and tabout
--
-- Nothing shows until you sign in once: open a .tex file, :LspCopilotSignIn
-- (from nvim-lspconfig's lsp/copilot.lua, pinned in lazy-lock.json).
--
-- prefs.lua: copilot = true/false (false: never started, Space a explains),
-- copilot_at_start = true/false (on when ntex starts).
local prefs = require("config.prefs")

local M = {}
M.FILETYPES = { "tex", "markdown" }
M.on = false
M.hinted = false
M.status = nil -- last status the server reported ("NotSignedIn", "OK", …)

local DIR = vim.fn.stdpath("data") .. "/copilot/node_modules/@github"

--- the server: the native binary from the npm package, or its node script
function M.cmd()
  local native = DIR .. "/copilot-language-server-darwin-arm64/copilot-language-server"
  if vim.fn.executable(native) == 1 then
    return { native, "--stdio" }
  end
  local js = DIR .. "/copilot-language-server/dist/language-server.js"
  if vim.uv.fs_stat(js) and vim.fn.executable("node") == 1 then
    return { "node", js, "--stdio" }
  end
  if vim.fn.executable("copilot-language-server") == 1 then
    return { "copilot-language-server", "--stdio" }
  end
  return nil
end

--- nvim-lspconfig's definition (sign in / out commands), read straight from
--- the pinned plugin without putting all of lspconfig on the runtimepath
local function upstream()
  local ok, cfg = pcall(function()
    local plugin = require("lazy.core.config").plugins["nvim-lspconfig"]
    return dofile(plugin.dir .. "/lsp/copilot.lua")
  end)
  return ok and type(cfg) == "table" and cfg or {}
end

local function hint_once(status)
  M.status = status
  if M.hinted or status == "OK" or status == "MaybeOk" or status == nil then
    return
  end
  vim.schedule(function()
    -- not after Space a turned it off (a late answer from the stopping server)
    if M.on and not M.hinted then
      M.hinted = true
      vim.notify("Copilot: run :LspCopilotSignIn", vim.log.levels.INFO)
    end
  end)
end

function M.available()
  -- TEXNVIM_NO_LSP=1: no language servers (Ntex Studio's headless bridge)
  return prefs.copilot ~= false and vim.env.TEXNVIM_NO_LSP ~= "1" and M.cmd() ~= nil
end

function M.setup()
  if not M.available() then
    return
  end
  local base = upstream()
  local on_attach = base.on_attach
  vim.lsp.config("copilot", vim.tbl_deep_extend("force", base, {
    cmd = M.cmd(),
    filetypes = M.FILETYPES,
    root_markers = { ".git" },
    -- no usage telemetry from the editor side (completions still go to GitHub)
    settings = { telemetry = { telemetryLevel = "off" } },
    handlers = {
      -- the server's status line ("not signed in", …): one quiet hint per
      -- session instead of a message every time
      didChangeStatus = function(_, params)
        if params and params.kind == "Error" and tostring(params.message or ""):lower():find("sign") then
          hint_once("NotSignedIn")
        end
      end,
      ["window/showMessage"] = function(err, params, ctx)
        local msg = params and params.message or ""
        if msg:lower():find("sign") then
          hint_once("NotSignedIn")
          return vim.NIL
        end
        return vim.lsp.handlers["window/showMessage"](err, params, ctx)
      end,
    },
    on_attach = function(client, bufnr)
      if on_attach then
        on_attach(client, bufnr) -- :LspCopilotSignIn / :LspCopilotSignOut
      end
      client:request("checkStatus", vim.empty_dict(), function(err, res)
        if not err and res then
          hint_once(res.status)
        end
      end, bufnr)
    end,
  }))
  M.set(prefs.copilot_at_start ~= false, true)
end

--- Copilot on or off: the server runs (and suggestions show) only while on
function M.set(on, quiet)
  if on and not M.available() then
    vim.notify(prefs.copilot == false and "Copilot is off in prefs.lua (copilot = false)"
      or "Copilot: the server isn't installed (see README, IDE features)", vim.log.levels.WARN)
    return
  end
  M.on = on
  vim.lsp.enable("copilot", on)
  vim.lsp.inline_completion.enable(on)
  if not on then
    for _, c in ipairs(vim.lsp.get_clients({ name = "copilot" })) do
      c:stop()
    end
  end
  if not quiet then
    local signed = (M.status == "NotSignedIn" and on) and " (not signed in: :LspCopilotSignIn)" or ""
    vim.notify("Copilot " .. (on and "on" or "off") .. signed, vim.log.levels.INFO)
  end
end

function M.toggle()
  M.set(not M.on)
end

function M.setup_keys()
  local map = vim.keymap.set
  map("n", "<leader>a", M.toggle, { desc = "Copilot ghost text on / off" })
  map("i", "<M-l>", function()
    vim.lsp.inline_completion.get()
  end, { desc = "Accept the Copilot suggestion" })
  map("i", "<M-]>", function()
    vim.lsp.inline_completion.select({ count = 1 })
  end, { desc = "Next Copilot suggestion" })
  map("i", "<M-[>", function()
    vim.lsp.inline_completion.select({ count = -1 })
  end, { desc = "Previous Copilot suggestion" })
end

return M
