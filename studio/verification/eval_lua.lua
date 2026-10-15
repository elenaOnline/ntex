-- nvim --clean --headless -l eval_lua.lua in.json out.json
-- in.json: ["<lua expression>", …] → out.json: [{ok=bool, value=string|nil, err=string|nil}, …]
local inp = vim.json.decode(table.concat(vim.fn.readfile(arg[1]), "\n"))
local out = {}
for _, src in ipairs(inp) do
  local fn, err = loadstring("return " .. src)
  if not fn then
    out[#out + 1] = { ok = false, err = err }
  else
    local ok, v = pcall(fn)
    if ok and type(v) == "string" then
      out[#out + 1] = { ok = true, value = v }
    else
      out[#out + 1] = { ok = false, err = tostring(v) }
    end
  end
end
local fd = assert(io.open(arg[2], "w"))
fd:write(vim.json.encode(out))
fd:close()
