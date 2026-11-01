-- Structure marks for .tex buffers: make section breaks, the kind of each
-- block, and how blocks nest visible while typing, on top of the syntax
-- colors in theme.lua. Everything here is extmarks and the status column:
-- nothing is written into the file.
--
--   sections  "rule"  a thin rule above each \section (and a short one above
--                     \subsection), with the section number at the end of
--                     the heading line
--             "band"  a tinted full-width band behind each heading line
--   kinds     "color" environment names colored by kind (below), structural
--                     ones (itemize, center, document …) dimmed
--             "bar"   the same, plus a bar in the gutter beside every line of
--                     a theorem-like block, in its kind's color
--   lines     "name"  only the environment name (and its title) is colored
--             "whole" the whole \begin{…}[…] / \end{…} in its kind: \begin a
--                     deeper shade, braces and brackets a gray tinted toward
--                     the kind's hue; math environments take the math blue
--   guides    true    a thin vertical line at the \begin's indent for every
--                     open environment, so nesting is visible (drawn where
--                     the line's own indentation leaves room for it)
--
-- Kinds: statement (theorem, lemma …), definition, example (example,
-- remark …), proof (proof, solution), problem (problem, exercise),
-- layout (lists, center, figure, document), math (align, cases, matrices).
-- Colors: the NtexKind* / NtexSection* groups in theme.lua.
local M = {}

local api = vim.api
local ns = api.nvim_create_namespace("ntex_structure")

M.opts = { sections = "rule", kinds = "bar", lines = "name", guides = false }

-- The fine structure settings (Ntex Studio's Look page, "Editor structure";
-- look.json "structure"). Defaults are the look as designed.
M.FINE = {
  rule_glyph = "─", -- the section rule: ─ ━ ┄ ═ ·
  subrule = "short", -- the subsection rule: "none" | "short" | "full"
  subrule_len = 24, -- columns, when short
  numbers = "end", -- the §2.1 number: "end" (of the heading line) | "none"
  bar_glyph = "▎", -- the gutter bar: ▏ ▎ ▌ ┃ █
  bar_side = "right", -- of the line number: "right" | "left"
  guide_glyph = "│", -- nesting lines: │ ┊ ╎ ▏
  guide_scope = "all", -- "all" | "blocks" (no lines for lists and other layout)
  title_bold = true, -- block titles ([Kernel]) bold
  begin_shade = 38, -- % darker \begin / \end than the kind (whole line)
  brace_tint = 35, -- % of the kind's hue in { } [ ] (whole line)
  guide_fade = 62, -- % toward the page of the nesting lines
}
for k, v in pairs(M.FINE) do
  M.opts[k] = v
end

M.KINDS = {
  statement = { "theorem", "lemma", "proposition", "corollary", "claim", "conjecture", "fact" },
  definition = { "definition", "notation" },
  example = { "example", "remark", "note", "observation" },
  proof = { "proof", "solution" },
  problem = { "problem", "exercise", "question" },
  layout = { "document", "itemize", "enumerate", "description", "center", "figure", "table",
    "tabular", "minipage", "flushleft", "flushright", "quote" },
  math = { "align", "equation", "gather", "multline", "flalign", "alignat", "aligned", "gathered",
    "split", "cases", "dcases", "array", "matrix", "pmatrix", "bmatrix", "Bmatrix", "vmatrix",
    "Vmatrix", "smallmatrix" },
}
local kind_of = {}
for kind, names in pairs(M.KINDS) do
  for _, n in ipairs(names) do
    kind_of[n] = kind
    kind_of[n .. "*"] = kind
  end
end
local GROUP = {
  statement = "NtexKindStatement",
  definition = "NtexKindDefinition",
  example = "NtexKindExample",
  proof = "NtexKindProof",
  problem = "NtexKindProblem",
  layout = "NtexKindLayout",
  math = "NtexKindMath",
}
local BARRED = { statement = true, definition = true, example = true, proof = true, problem = true }

local LEVELS = { section = 1, subsection = 2, subsubsection = 3 }

-- per-buffer: kind of the innermost barred block on each line (1-based)
M.bars = {}

local function strip_comment(line)
  local i = 1 -- cut at the first % not preceded by a backslash
  while true do
    local p = line:find("%", i, true)
    if not p then
      return line
    end
    if p == 1 or line:sub(p - 1, p - 1) ~= "\\" then
      return line:sub(1, p - 1)
    end
    i = p + 1
  end
end

local function text_width(win)
  local info = vim.fn.getwininfo(win)[1]
  return info and (info.width - info.textoff) or 80
end

local function mark(buf, r, s, e, group)
  api.nvim_buf_set_extmark(buf, ns, r, s, { end_col = e, hl_group = group, priority = 150 })
end

-- \begin{name}[opt]{arg} / \end{name}, starting at byte s (1-based) of line
local function color_env(buf, r, line, s, kw, name, kind, o)
  local group = GROUP[kind]
  local brace = line:find("{", s, true) -- 1-based col of '{'
  local name_end = brace + #name -- 0-based end col of the name
  if o.lines == "whole" then
    mark(buf, r, s - 1, s + #kw, group .. "Cmd")
    mark(buf, r, brace - 1, brace, group .. "Delim")
    mark(buf, r, name_end, name_end + 1, group .. "Delim")
  end
  if kind ~= "math" or o.lines == "whole" then
    mark(buf, r, brace, name_end, group)
  end
  if kw ~= "begin" or kind == "math" or kind == "layout" then
    return
  end
  -- \begin{lemma}[Title] or \begin{lemma}{Title}: the title in the kind's color
  local after = name_end + 2
  local rest = line:sub(after)
  local opt = rest:match("^%s*(%b[])") or rest:match("^%s*(%b{})")
  if opt then
    local os_ = line:find(opt, after, true) - 1 -- 0-based
    if o.lines == "whole" then
      mark(buf, r, os_, os_ + 1, group .. "Delim")
      mark(buf, r, os_ + #opt - 1, os_ + #opt, group .. "Delim")
      mark(buf, r, os_ + 1, os_ + #opt - 1, group .. "Title")
    else
      mark(buf, r, os_, os_ + #opt, group .. "Title")
    end
  end
end

local function indent_of(line)
  return vim.fn.strdisplaywidth(line:match("^%s*"))
end

function M.refresh(buf)
  buf = buf or api.nvim_get_current_buf()
  if not api.nvim_buf_is_valid(buf) then
    return
  end
  api.nvim_buf_clear_namespace(buf, ns, 0, -1)
  local o = M.opts
  local win = vim.fn.bufwinid(buf)
  local width = win ~= -1 and text_width(win) or 80
  local lines = api.nvim_buf_get_lines(buf, 0, -1, false)
  local count = { 0, 0, 0 }
  local stack = {} -- open barred blocks: kind
  local open = {} -- open environments for guides: { name, kind, col }
  local bars = {}
  for row, raw in ipairs(lines) do
    local line = strip_comment(raw)
    local r = row - 1
    -- guides for environments open above this line (before this line's
    -- own \begin / \end change the stack)
    if o.guides and #open > 0 then
      local blank = raw:match("^%s*$") ~= nil
      local ind = blank and math.huge or indent_of(raw)
      local closes = {}
      for name in line:gmatch("\\end%s*{([^}]*)}") do
        closes[name] = true
      end
      for _, e in ipairs(open) do
        if (o.guide_scope ~= "blocks" or e.kind ~= "layout") and e.col < ind and not (closes[e.name] and indent_of(raw) <= e.col) then
          api.nvim_buf_set_extmark(buf, ns, r, 0, {
            virt_text = { { o.guide_glyph, GROUP[e.kind] .. "Guide" } },
            virt_text_win_col = e.col,
            virt_text_pos = "overlay",
            hl_mode = "combine",
            priority = 10,
          })
        end
      end
    end
    -- sections
    for s, kw, star, arg in line:gmatch("()\\(%a+)(%*?)%s*(%b{})") do
      local lvl = LEVELS[kw]
      if lvl and o.sections ~= "none" then
        local num = ""
        if star == "" then
          count[lvl] = count[lvl] + 1
          for k = lvl + 1, 3 do
            count[k] = 0
          end
          num = table.concat(count, ".", 1, lvl)
        end
        if o.sections == "rule" then
          if lvl == 1 or (lvl == 2 and o.subrule ~= "none") then
            local len = (lvl == 1 or o.subrule == "full") and width or math.min(width, o.subrule_len)
            local virt = { { string.rep(o.rule_glyph, len), lvl == 1 and "NtexSectionRule" or "NtexSubsectionRule" } }
            api.nvim_buf_set_extmark(buf, ns, r, 0, { virt_lines = { virt }, virt_lines_above = true })
          end
        elseif o.sections == "band" then
          api.nvim_buf_set_extmark(buf, ns, r, 0, {
            line_hl_group = lvl == 1 and "NtexSectionBand" or "NtexSubsectionBand",
            priority = 5,
          })
        end
        if num ~= "" and o.numbers ~= "none" then
          api.nvim_buf_set_extmark(buf, ns, r, 0, {
            virt_text = { { "§" .. num, "NtexSectionNum" } },
            virt_text_pos = "eol",
          })
        end
        local title_s = line:find("{", s + #kw + #star + 1, true)
        if title_s then
          mark(buf, r, title_s, title_s + #arg - 2, lvl == 1 and "NtexSectionTitle" or "NtexSubsectionTitle")
        end
      end
    end
    -- environments
    for s, kw, arg in line:gmatch("()\\(%a+)%s*(%b{})") do
      if kw == "begin" or kw == "end" then
        local name = arg:sub(2, -2)
        local kind = kind_of[name]
        if kind and o.kinds ~= "none" then
          color_env(buf, r, line, s, kw, name, kind, o)
        end
        if kind and o.guides and name ~= "document" then
          if kw == "begin" then
            open[#open + 1] = { name = name, kind = kind, col = vim.fn.strdisplaywidth(raw:sub(1, s - 1)) }
          else
            for k = #open, 1, -1 do
              if open[k].name == name then
                for j = #open, k, -1 do
                  open[j] = nil
                end
                break
              end
            end
          end
        end
        if kind and BARRED[kind] then
          if kw == "begin" then
            stack[#stack + 1] = kind
          else
            for k = #stack, 1, -1 do
              if stack[k] == kind then
                -- the \end line still belongs to the block
                bars[row] = bars[row] or kind
                table.remove(stack, k)
                break
              end
            end
          end
        end
      end
    end
    if not bars[row] and #stack > 0 then
      bars[row] = stack[#stack]
    end
  end
  M.bars[buf] = bars
end

-- statuscolumn: the line number, then the block bar (or a space)
function M.column()
  local buf = api.nvim_get_current_buf()
  local kind = (M.bars[buf] or {})[vim.v.lnum]
  local bar = kind and ("%#" .. GROUP[kind] .. "Bar#" .. M.opts.bar_glyph .. "%*") or " "
  if M.opts.bar_side == "left" then
    return bar .. "%=%l "
  end
  return "%=%l" .. bar
end

local timer
local function schedule(buf)
  if timer then
    timer:stop()
  end
  timer = vim.defer_fn(function()
    M.refresh(buf)
  end, 120)
end

local function attach(buf)
  local win = vim.fn.bufwinid(buf)
  local col = "%{%v:lua.require'config.structure'.column()%}"
  if win ~= -1 then
    if M.opts.kinds == "bar" then
      vim.wo[win].statuscolumn = col
    elseif vim.wo[win].statuscolumn == col then
      vim.wo[win].statuscolumn = ""
    end
  end
  M.refresh(buf)
end

function M.setup(opts)
  M.opts = vim.tbl_extend("force", M.opts, opts or {})
  local g = api.nvim_create_augroup("ntex_structure", { clear = true })
  api.nvim_create_autocmd({ "FileType" }, {
    group = g,
    pattern = "tex",
    callback = function(ev)
      vim.schedule(function()
        attach(ev.buf)
      end)
    end,
  })
  api.nvim_create_autocmd({ "BufWinEnter" }, {
    group = g,
    pattern = "*.tex",
    callback = function(ev)
      attach(ev.buf)
    end,
  })
  api.nvim_create_autocmd({ "TextChanged", "TextChangedI" }, {
    group = g,
    pattern = "*.tex",
    callback = function(ev)
      schedule(ev.buf)
    end,
  })
  api.nvim_create_autocmd({ "WinResized", "VimResized" }, {
    group = g,
    callback = function()
      for _, w in ipairs(api.nvim_list_wins()) do
        local b = api.nvim_win_get_buf(w)
        if vim.bo[b].filetype == "tex" then
          M.refresh(b)
        end
      end
    end,
  })
  for _, b in ipairs(api.nvim_list_bufs()) do
    if api.nvim_buf_is_loaded(b) and vim.bo[b].filetype == "tex" then
      attach(b)
    end
  end
end

return M
