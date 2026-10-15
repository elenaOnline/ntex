local dir = arg[1]
local out = {}
local function helper(name) return function(trig, ...) return { helper = name, trig = trig } end end
local stub = setmetatable({ group = function(title, list) return { title = title, list = list } end,
  display_delims = function() return "\\[", "\\]" end }, { __index = function(t, k) return helper(k) end })
package.loaded["snips.helpers"] = stub
package.loaded["snips.cond"] = { list_marker = function() end }
for _, f in ipairs(vim.fn.globpath(dir, "*.lua", false, true)) do
  local g = dofile(f)
  local trigs = {}
  for _, e in ipairs(g.list) do trigs[#trigs + 1] = e.trig end
  out[vim.fs.basename(f)] = { title = g.title, n = #g.list, triggers = trigs }
end
io.write(vim.json.encode(out))
