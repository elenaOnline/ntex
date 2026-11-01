-- The editor look: which palette paints the page (lua/config/palettes.lua)
-- and how structure shows (lua/config/structure.lua). Chosen 2026-10-18 in
-- the ntex Syntax Lab; the defaults below are that pick.
--
-- The settings live in ~/.config/texnvim/look.json, which Ntex Studio's Look
-- page writes (⌘5). Every running ntex watches the file and follows at once.
-- From inside ntex:  :NtexLook                 show the settings
--                    :NtexLook palette code    change one (saved to look.json)
--
--   palette   "slate" | "code" | "code-dark" | "alabaster" (the look before),
--             or a standard theme to compare: "vscode-light" | "vscode-dark" |
--             "github-light" | "one-light" | "solarized-light" |
--             "solarized-dark" | "catppuccin-latte" | "catppuccin-mocha" |
--             "tokyonight" | "gruvbox-dark" | "newpaper-light" |
--             "newpaper-dark" (their own colors; ink n/a)
--   sections  "none" | "rule" | "band"
--   kinds     "none" | "color" | "bar"       (bar: a gutter bar beside blocks)
--   lines     "name" | "whole"               (color the whole \begin{…} line)
--   guides    true | false                   (nesting lines)
--   ink       "content" | "all"              (black only for printed text, or
--                                              for commands too, as before)
--   structure the fine settings (rule glyph, subsection rule, bar glyph and
--             side, guide glyph and scope, shades …: structure.lua M.FINE)
--   colors    { palette = { role = "#rrggbb" } }  the Look page's HSV edits,
--             painted over that palette (roles: palettes.lua M.ROLES)
--
-- A dark palette switches Neovim to background=dark (Modus Vivendi under the
-- menus and floats). Code Dark and every standard theme set the terminal's
-- own background and foreground to the page's (OSC 11 / 10), so cmux's
-- padding matches; a custom light palette, or quitting ntex, gives the
-- terminal its own white back (OSC 111 / 110).
local M = {}

M.DEFAULTS = { palette = "slate", sections = "rule", kinds = "bar", lines = "whole", guides = true, ink = "content" }
M.CHOICES = {
  palette = vim.list_extend({ "slate", "code", "code-dark", "alabaster" }, require("config.palettes").STANDARD_ORDER),
  sections = { "none", "rule", "band" },
  kinds = { "none", "color", "bar" },
  lines = { "name", "whole" },
  guides = { true, false },
  ink = { "content", "all" },
}
M.KEYS = { "palette", "sections", "kinds", "lines", "guides", "ink" }

-- TEXNVIM_LOOK: another file (the tests use a temp one)
M.path = vim.env.TEXNVIM_LOOK or vim.fs.joinpath(vim.fn.stdpath("config"), "look.json")

local function valid(key, v)
  for _, c in ipairs(M.CHOICES[key] or {}) do
    if c == v then
      return true
    end
  end
  return false
end

--- The settings in look.json over the defaults (unknown keys and values are ignored).
function M.read()
  local look = vim.deepcopy(M.DEFAULTS)
  look.colors = {}
  look.structure = vim.deepcopy(require("config.structure").FINE)
  local fd = io.open(M.path, "r")
  if not fd then
    return look
  end
  local text = fd:read("*a")
  fd:close()
  local ok, t = pcall(vim.json.decode, text)
  if ok and type(t) == "table" then
    for _, k in ipairs(M.KEYS) do
      if valid(k, t[k]) then
        look[k] = t[k]
      end
    end
    -- "structure": the fine structure settings (structure.lua M.FINE); a value
    -- of the wrong type, or a glyph longer than one character, is ignored
    if type(t.structure) == "table" then
      for k, d in pairs(look.structure) do
        local v = t.structure[k]
        if type(v) == type(d) and (type(v) ~= "string" or vim.fn.strchars(v) <= 6) then
          look.structure[k] = v
        end
      end
    end
    -- "colors": { palette = { role = "#rrggbb" } }, the Look page's color edits
    if type(t.colors) == "table" then
      for pal, roles in pairs(t.colors) do
        if valid("palette", pal) and type(roles) == "table" then
          for role, c in pairs(roles) do
            if type(c) == "string" and c:match("^#%x%x%x%x%x%x$") and vim.tbl_contains(require("config.palettes").ROLES, role) then
              look.colors[pal] = look.colors[pal] or {}
              look.colors[pal][role] = c:lower()
            end
          end
        end
      end
    end
  end
  return look
end

--- The color edits for `palette` (role → "#rrggbb"; empty when none).
function M.colors(palette)
  return (M.current.colors or {})[palette] or {}
end

M.current = M.read()

function M.get()
  return M.current
end

--- Write look.json (atomically: Studio and other ntex read it whole).
function M.save(look)
  local keys = {}
  for _, k in ipairs(M.KEYS) do
    keys[#keys + 1] = ("  %s: %s"):format(vim.json.encode(k), vim.json.encode(look[k]))
  end
  if look.structure and not vim.deep_equal(look.structure, require("config.structure").FINE) then
    keys[#keys + 1] = '  "structure": ' .. vim.json.encode(look.structure)
  end
  if look.colors and next(look.colors) then
    keys[#keys + 1] = '  "colors": ' .. vim.json.encode(look.colors)
  end
  local tmp = M.path .. ".tmp"
  local fd = io.open(tmp, "w")
  if not fd then
    return false
  end
  fd:write("{\n" .. table.concat(keys, ",\n") .. "\n}\n")
  fd:close()
  return vim.uv.fs_rename(tmp, M.path) ~= nil
end

--- Paint and mark with `look` (default: look.json as it is now).
function M.apply(look)
  M.current = look or M.read()
  require("config.structure").setup(vim.tbl_extend("force", {
    sections = M.current.sections,
    kinds = M.current.kinds,
    lines = M.current.lines,
    guides = M.current.guides,
  }, M.current.structure or {}))
  require("config.theme").reload()
end

local watcher
local function watch()
  if watcher then
    return
  end
  watcher = vim.uv.new_fs_event()
  if not watcher then
    return
  end
  -- the folder, not the file: Studio replaces look.json by renaming over it
  local dir, name = vim.fs.dirname(M.path), vim.fs.basename(M.path)
  local pending = false
  watcher:start(dir, {}, function(err, file)
    if err or (file and file ~= name) or pending then
      return
    end
    pending = true
    vim.defer_fn(function()
      pending = false
      local new = M.read()
      if not vim.deep_equal(new, M.current) then
        M.apply(new)
      end
    end, 50)
  end)
end

function M.setup()
  require("config.structure").setup(vim.tbl_extend("force", {
    sections = M.current.sections,
    kinds = M.current.kinds,
    lines = M.current.lines,
    guides = M.current.guides,
  }, M.current.structure or {}))
  watch()

  vim.api.nvim_create_user_command("NtexLook", function(o)
    local key, value = o.fargs[1], o.fargs[2]
    if not key then
      local parts = {}
      for _, k in ipairs(M.KEYS) do
        parts[#parts + 1] = k .. " = " .. tostring(M.current[k])
      end
      vim.notify("ntex look: " .. table.concat(parts, ", ") .. "\n(" .. M.path .. ")", vim.log.levels.INFO)
      return
    end
    if not M.CHOICES[key] then
      vim.notify("NtexLook: the settings are " .. table.concat(M.KEYS, ", "), vim.log.levels.ERROR)
      return
    end
    local v = value
    if key == "guides" then
      v = ({ ["true"] = true, on = true, ["false"] = false, off = false })[value or ""]
    end
    if not valid(key, v) then
      vim.notify(("NtexLook: %s is one of %s"):format(key, table.concat(vim.tbl_map(tostring, M.CHOICES[key]), ", ")), vim.log.levels.ERROR)
      return
    end
    local new = vim.deepcopy(M.current)
    new[key] = v
    M.save(new)
    M.apply(new)
  end, {
    nargs = "*",
    complete = function(_, line)
      local args = vim.split(line, "%s+", { trimempty = true })
      if #args <= 1 or (#args == 2 and not line:match("%s$")) then
        return M.KEYS
      end
      return vim.tbl_map(tostring, M.CHOICES[args[2]] or {})
    end,
    desc = "Show or change the ntex editor look (palette, sections, kinds, lines, guides, ink)",
  })
end

return M
