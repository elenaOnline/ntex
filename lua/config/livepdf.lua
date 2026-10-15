-- Live PDF: after a pause in typing, save the .tex file; the BufWritePost hook
-- in lua/plugins/core.lua then compiles it (tectonic) and Skim reloads.
-- Turn off in prefs.lua (live_pdf = false).
local prefs = require("config.prefs")
if not prefs.live_pdf then
  return
end

local timers = {}

local function save(buf)
  if not vim.api.nvim_buf_is_valid(buf) or not vim.bo[buf].modified then
    return
  end
  if vim.api.nvim_buf_get_name(buf) == "" or vim.bo[buf].buftype ~= "" or vim.bo[buf].readonly then
    return
  end
  vim.api.nvim_buf_call(buf, function()
    vim.cmd("silent! lockmarks update")
  end)
end

vim.api.nvim_create_autocmd({ "TextChanged", "TextChangedI" }, {
  group = vim.api.nvim_create_augroup("texnvim_live_pdf", { clear = true }),
  pattern = "*.tex",
  callback = function(ev)
    local buf = ev.buf
    local t = timers[buf]
    if not t then
      t = vim.uv.new_timer()
      timers[buf] = t
      vim.api.nvim_create_autocmd("BufWipeout", {
        buffer = buf,
        once = true,
        callback = function()
          t:stop()
          t:close()
          timers[buf] = nil
        end,
      })
    end
    t:stop()
    t:start(prefs.live_pdf_delay_ms, 0, vim.schedule_wrap(function()
      save(buf)
    end))
  end,
})
