-- Test cases. "‸" is the cursor. Each case runs in a .md and a .tex buffer.
local M = {}

local function inl(ft, s)
  return "$" .. s .. "$"
end
local function disp(ft, s)
  if ft == "tex" then
    return "\\[\n" .. s .. "\n\\]"
  end
  return "$$\n" .. s .. "\n$$"
end

local FTS = { "md", "tex" }

-- display-math delimiters: ntex rewrites $$…$$ to \[…\] in .tex files
local function dm_open(ft) return ft == "tex" and "\\[" or "$$" end
local function dm_close(ft) return ft == "tex" and "\\]" or "$$" end

-- Your snippets (snippets/tex/*.lua) ---------------------------------------------
local function mine(T)
  for _, ft in ipairs(FTS) do
    local I = function(s) return inl(ft, s) end
    local D = function(s) return disp(ft, s) end
    io.write(("\n-- %s: inside $…$\n"):format(ft))
    T.case("x2 → x_{2}", ft, I("‸"), { "x2", I("x_{2}‸") })
    T.case("// → \\frac{‸}{}, Tab → second slot, Tab → out", ft, I("‸"), {
      "//", I("\\frac{‸}{}"),
      "<Tab>", I("\\frac{}{‸}"),
      "<Tab>", I("\\frac{}{}‸"),
    })
    T.case("sr → ^{2}", ft, I("‸"), { "xsr", I("x^{2}‸") })
    T.case("xhat → \\hat{x}", ft, I("‸"), { "xhat", I("\\hat{x}‸") })
    T.case("sin → \\sin, then x → \\sin x", ft, I("‸"), { "sin", I("\\sin‸"), "x", I("\\sin x‸") })
    T.case("a/b auto-fraction → \\frac{a}{b}", ft, I("‸"), { "a/", I("\\frac{a}{‸}"), "b", I("\\frac{a}{b‸}") })
    T.case("(a+b)/ auto-fraction drops outer parens", ft, I("x=‸"), { "(a+b", I("x=(a+b‸)"), ")/", I("x=\\frac{a+b}{‸}") })
    T.case("=> → \\implies", ft, I("‸"), { "=>", I("\\implies‸") })
    T.case("RR → \\mathbb{R}", ft, I("‸"), { "RR", I("\\mathbb{R}‸") })
    T.case("dint tabstops", ft, I("‸"), {
      "dint", { I("\\int_{0}^{1}  \\, dx"), mode = "s", sel = "0" },
      "<Tab>", { I("\\int_{0}^{1}  \\, dx"), mode = "s", sel = "1" },
      "<Tab>", I("\\int_{0}^{1} ‸ \\, dx"),
      "f<Tab>", { I("\\int_{0}^{1} f \\, dx"), mode = "s", sel = "x" },
      "<Tab>", I("\\int_{0}^{1} f \\, dx‸"),
    })
    T.case("uu → \\cup", ft, I("A ‸"), { "uu", I("A \\cup‸") })
    T.case("inn → \\in (beats nn; trailing space trimmed in inline math, as upstream)", ft, I("x ‸"), { "inn", I("x \\in‸") })
    T.case("nn → \\cap", ft, I("A ‸"), { "nn", I("A \\cap‸") })
    T.case("exx → \\exists", ft, I("‸"), { "exx", I("\\exists‸") })
    T.case("odp → \\langle $1 , $2 \\rangle", ft, I("‸"), {
      "odp", I("\\langle ‸ ,  \\rangle"),
      "a<Tab>", I("\\langle a , ‸ \\rangle"),
      "b<Tab>", I("\\langle a , b \\rangle‸"),
    })
    T.case("\\\\\\ → \\setminus{}", ft, I("A ‸"), { "\\\\\\", I("A \\setminus{}‸") })
    T.case("( auto-pairs, sum inside auto-enlarges", ft, I("‸"), { "(", I("(‸)"), "sum", I("\\left( \\sum‸ \\right)") })
    T.case("typing ) steps over )", ft, I("(a‸)"), { ")", I("(a)‸") })
    T.case("tabout: Tab leaves } then $", ft, I("\\sqrt{a‸}"), { "<Tab>", I("\\sqrt{a}‸"), "<Tab>", I("\\sqrt{a}") .. "‸" })
    T.case("Backspace in empty $$ deletes both", ft, "x " .. I("‸") .. " y", { "<BS>", "x ‸ y" })

    io.write(("\n-- %s: display math\n"):format(ft))
    T.case("pmat → pmatrix", ft, D("‸"), { "pmat", D("\\begin{pmatrix}\n‸\n\\end{pmatrix}") })
    T.case("beg → \\begin{…}\\end{…} with mirror", ft, D("‸"), {
      "beg", D("\\begin{‸}\n\t\n\\end{}"),
      "cases", D("\\begin{cases‸}\n\t\n\\end{cases}"),
      "<Tab>", D("\\begin{cases}\n\t‸\n\\end{cases}"),
    })
    T.case("matrix: Tab = &, Enter = \\\\, Shift-Enter leaves", ft, D("\\begin{pmatrix}\na‸\n\\end{pmatrix}"), {
      "<Tab>", D("\\begin{pmatrix}\na & ‸\n\\end{pmatrix}"),
      "b<CR>", D("\\begin{pmatrix}\na & b \\\\\n‸\n\\end{pmatrix}"),
      "c<S-CR>", D("\\begin{pmatrix}\na & b \\\\\nc\n\\end{pmatrix}‸"),
    })
    T.case("dm-style text inside \\text{} does not expand math snippets", ft, I("\\text{‸}"), { "sr", I("\\text{sr‸}") })

    io.write(("\n-- %s: visual (select mode)\n"):format(ft))
    T.select_from_marker(ft, I("‸a+b"), 3)
    T.type_keys("S")
    T.check(ft .. "  select a+b, type S → \\sqrt{ a+b } (result stays selected)", I("\\sqrt{ a+b }"), { mode = "s", sel = "\\sqrt{ a+b }" })
    T.type_keys("(")
    T.check(ft .. "  …then ( wraps again", I("(\\sqrt{ a+b })"), { mode = "s", sel = "(\\sqrt{ a+b })" })
    T.select_from_marker(ft, "plain ‸text here", 4)
    T.type_keys("S")
    T.check(ft .. "  S on a selection in text just types S", "plain S‸ here")

    io.write(("\n-- %s: text\n"):format(ft))
    T.case("mk → $‸$", ft, "Let ‸", { "mk", "Let $‸$", "x2", "Let $x_{2}‸$" })
    T.case("sr does not expand in text", ft, "the ‸", { "sr", "the sr‸" })
    T.case("xhat does not expand in text", ft, "‸", { "xhat", "xhat‸" })
    T.case("dm at line start → display block", ft, "Intro\n‸", { "dm", "Intro\n" .. dm_open(ft) .. "\n\t‸\n" .. dm_close(ft) })
    T.case("dm after text → display block on new line", ft, "so we get ‸", { "dm", "so we get \n" .. dm_open(ft) .. "\n\t‸\n" .. dm_close(ft) })
  end
  io.write("\n-- md: list display math\n")
  T.case("- item dm → indented block", "md", "- item ‸", { "dm", "- item \n  $$\n  \t‸\n  $$" })
end

-- Latex Suite defaults you kept, and defaults that stay off -------------------
local function defaults(T)
  for _, ft in ipairs(FTS) do
    local I = function(s) return inl(ft, s) end
    local D = function(s) return disp(ft, s) end
    io.write(("\n-- %s: Latex Suite defaults you kept\n"):format(ft))
    T.case("x2 → x_{2}", ft, I("‸"), { "x2", I("x_{2}‸") })
    T.case("x24 → x_{24}", ft, I("‸"), { "x24", I("x_{24}‸") })
    T.case("\\alpha3 → \\alpha_{3}", ft, I("‸"), { "\\alpha3", I("\\alpha_{3}‸") })
    T.case("xdot3 → \\dot{x}_{3}", ft, I("‸"), { "xdot3", I("\\dot{x}_{3}‸") })
    T.case("RR → \\mathbb{R}", ft, I("‸"), { "RR", I("\\mathbb{R}‸") })
    T.case("iden2 → 2×2 identity", ft, D("‸"), { "iden2", D("\\begin{pmatrix}\n1 & 0 \\\\\n0 & 1\n\\end{pmatrix}‸") })
    T.case("\\sum<Tab> → limits", ft, I("‸"), { "sum", I("\\sum‸"), "<Tab>", { I("\\sum_{i=1}^{N}"), mode = "s", sel = "i" } })
    T.case("int<Tab> → \\int … \\, dx", ft, I("‸"), { "int", I("\\int‸"), "<Tab>", I("\\int ‸ \\, dx") })
    T.case("lim → \\lim_{ n \\to \\infty }", ft, I("‸"), { "lim", { I("\\lim_{ n \\to \\infty }"), mode = "s", sel = "n" } })
    T.case("o+ keeps its space in display maths", ft, D("‸"), { "o+", D("\\oplus ‸") })
    T.case("o+ drops it in inline maths", ft, I("‸"), { "o+", I("\\oplus‸") })
    T.case("sin → \\sin; \\sinhx → \\sinh x", ft, I("‸"), { "sinh", I("\\sinh‸"), "x", I("\\sinh x‸") })
    T.case("arcsec → \\operatorname{arcsec}", ft, I("‸"), { "arcsec", I("\\operatorname{arcsec}‸") })
    T.case("pmat inline → one line", ft, I("‸"), { "pmat", I("\\begin{pmatrix}‸\\end{pmatrix}") })
    T.case("v,. → \\mathbf{v}; \\mu,. → \\boldsymbol{\\mu}", ft, I("‸"), { "v,. \\mu,.", I("\\mathbf{v} \\boldsymbol{\\mu}‸") })
    T.case("outer: mirrored \\psi", ft, I("‸"), { "outer", { I("\\ket{\\psi} \\bra{\\psi}"), mode = "s", sel = "\\psi" }, "\\phi", I("\\ket{\\phi‸} \\bra{\\phi}") })

    io.write(("\n-- %s: turned-off defaults stay off\n"):format(ft))
    T.case("@a stays @a", ft, I("‸"), { "@a", I("@a‸") })
    T.case("alpha stays alpha (no automatic backslash)", ft, I("‸"), { "alpha", I("alpha‸") })
    T.case("\\tox stays \\tox (no space after macros)", ft, I("‸"), { "\\tox", I("\\tox‸") })
    T.case("\\leq3 stays \\leq3", ft, I("‸"), { "\\leq3", I("\\leq3‸") })
    T.case("QQ → \\mathbb{Q} (yours again since 2026-10-05)", ft, I("‸"), { "QQ", I("\\mathbb{Q}‸") })
    T.case("deg stays deg", ft, I("‸"), { "deg", I("deg‸") })
    T.case("mk inside \\text{} does not expand", ft, I("\\text{if ‸}"), { "mk", I("\\text{if mk‸}") })
  end
end

-- Typing every symbol command as written --------------------------------------
local function mangling(T)
  local data = vim.json.decode(table.concat(vim.fn.readfile(vim.env.TEXNVIM_TEST_DATA .. "/symbols.json"), "\n"))
  local intended = data.intended or {}
  local esc = function(s) return (s:gsub("<", "<lt>")) end
  -- both settings of prefs.lua latex.fire_after_letters must keep commands intact
  local runs = { { ft = "md", relaxed = false }, { ft = "tex", relaxed = false }, { ft = "tex", relaxed = true } }
  for _, run in ipairs(runs) do
    local ft = run.ft
    T.lua([[require("snips.settings").override({ fire_after_letters = ... })]], run.relaxed)
    io.write(("\n-- %s: %d commands typed as written into $…$%s\n"):format(ft, #data.symbols, run.relaxed and " (fire_after_letters = true)" or ""))
    local changed, allowed = 0, 0
    for _, cmd in ipairs(data.symbols) do
      T.open(ft, "$‸$", true)
      T.type_keys(esc(cmd))
      local st = T.state()
      local got = table.concat(st.lines, "\n"):gsub("^%$", ""):gsub("%$$", "")
      local want = intended[cmd] and intended[cmd].out or cmd
      if intended[cmd] then
        allowed = allowed + 1
      end
      if got ~= want then
        changed = changed + 1
      end
      T.ok(("%-4s %s%-24s → %s%s"):format(ft, run.relaxed and "(after letters) " or "", cmd, got, intended[cmd] and ("   (on purpose: " .. intended[cmd].why .. ")") or ""), got == want,
        ("\n        expected %s"):format(vim.inspect(want)))
    end
    io.write(("   %s: %d of %d changed unexpectedly, %d change on purpose (allowlist)\n"):format(ft, changed, #data.symbols, allowed))
  end
  T.lua([[require("snips.settings").override({})]])
end

-- Ordinary English stays ordinary -----------------------------------------------
local PARAGRAPH = "Let us admit the landmark result: the subset of all sets is a set, and the inner sum is finite. "
  .. "Since the integral of sine and cosine over a period vanishes, the log of the determinant is limited; "
  .. "a bar, a dot and a hat are accents, vectors have norms, floors and ceilings. "
  .. "We begin with cases, align arrays and matrices, trace properties iff they are negative, "
  .. "and differ by an implication (quod erat demonstrandum). Experts like pi, stars, perpendicular lines "
  .. "and philosophers too; Oxford and Cambridge set exams on Monday. Ask Ms. Kim, Dr. Smith or the "
  .. "editor: sub-editors, subsets and supersets, ooh and aah, 2nd and 3rd place, e.g. and i.e. all stay."

local function english(T)
  local esc = function(s) return (s:gsub("<", "<lt>")) end
  for _, ft in ipairs(FTS) do
    io.write(("\n-- %s: English stays as typed\n"):format(ft))
    T.case("a paragraph in text", ft, "‸", { esc(PARAGRAPH), PARAGRAPH .. "‸" })
    T.case("the paragraph inside \\text{} in inline maths", ft, "$x \\text{‸}$", { esc(PARAGRAPH), "$x \\text{" .. PARAGRAPH .. "‸}$" })
    T.case("the paragraph inside \\text{} in display maths", ft, disp(ft, "\\text{‸}"), { esc(PARAGRAPH), disp(ft, "\\text{" .. PARAGRAPH .. "‸}") })
    T.case("mk and dm at a word boundary still work", ft, "say ‸", { "mk", "say $‸$" })
  end
  T.case("a list item with admit / landmark in it", "md", "- ‸", { "we admit the landmark", "- we admit the landmark‸" })
  T.case("a numbered item too", "md", "1. ‸", { "admit it", "1. admit it‸" })
  T.case("a % comment in .tex: nothing fires", "tex", "% ‸", { "mk dm uu xx sub", "% mk dm uu xx sub‸" })
  T.case("maths inside a % comment: nothing fires", "tex", "% $‸$", { "uu x2", "% $uu x2‸$" })
end

-- Every before/after in snippets/decisions.json, re-checked -----------------------
local function decisions(T)
  local path = vim.env.TEXNVIM_DECISIONS
  local ok_read, data = pcall(function()
    return vim.json.decode(table.concat(vim.fn.readfile(path), "\n"))
  end)
  if not ok_read or type(data) ~= "table" then
    T.ok("     decisions.json is readable (" .. tostring(path) .. ")", false)
    return
  end
  io.write(("\n-- %d entries in %s; every “now” typed again\n"):format(#data.entries, vim.fn.fnamemodify(path, ":~")))
  local n = 0
  for _, e in ipairs(data.entries) do
    for _, ex in ipairs(e.examples or {}) do
      if ex.after and ex.after.verified then
        n = n + 1
        local ft = ex.ft == "tex" and "tex" or "md"
        if ex.select ~= nil and ex.select ~= vim.NIL then
          T.select_from_marker(ft, ex.before, ex.select)
        else
          T.open(ft, ex.before, true)
        end
        T.type_keys(ex.keys)
        local st = T.state()
        local got = T.render(st)
        local want_sel = ex.after.sel ~= vim.NIL and ex.after.sel or nil
        local good = got == ex.after.text and st.mode == ex.after.mode and (st.sel or nil) == want_sel
        T.ok(("%-4s [%s] %s: %s → %s"):format(ex.ft, e.category, e.trigger, ex.keys, tostring(ex.after.text):gsub("\n", "⏎")), good,
          ("\n        got:      %s (mode %s, sel %s)"):format(vim.inspect(got), st.mode, vim.inspect(st.sel)))
      end
    end
  end
  io.write(("   %d examples re-typed\n"):format(n))
end

-- Hot reload ------------------------------------------------------------------------
local function hot_reload(T)
  io.write("\n-- hot reload (a copy of snippets/ in the temp folder)\n")
  local gen0 = T.lua([[return require("snips").generation]])
  local path = T.lua([[return require("snips").dir() .. "/tex/sets.lua"]])
  -- edit a snippet file the normal way: open it, add a line, :w
  T.rpc("nvim_input", "<Esc>")
  T.sync()
  T.lua([==[
    vim.cmd.edit(...)
    local buf = vim.api.nvim_get_current_buf()
    local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
    for i, l in ipairs(lines) do
      if l:match('^  math%("uu"') then
        table.insert(lines, i, '  math("zzq",      [[\\hotreload{<>}]],          "new"),')
        break
      end
    end
    vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
    vim.cmd("silent write")
  ]==], path)
  local ok = T.lua([[
    local g = ...
    return vim.wait(8000, function() return require("snips").generation > g end, 20)
  ]], gen0)
  T.ok("md   sets.lua saved → reloaded", ok == true)
  T.case("new trigger works after reload", "md", "$‸$", { "zzq", "$\\hotreload{‸}$" })
  T.case("…in .tex too", "tex", "$‸$", { "zzq", "$\\hotreload{‸}$" })

  -- an error keeps the previous snippets of that file working
  local gen1 = T.lua([[return require("snips").generation]])
  T.rpc("nvim_input", "<Esc>")
  T.sync()
  T.lua([[
    vim.cmd.edit(...)
    local buf = vim.api.nvim_get_current_buf()
    vim.api.nvim_buf_set_lines(buf, 0, 0, false, { "this is not lua (" })
    vim.cmd("silent write")
  ]], path)
  T.lua([[vim.wait(1500, function() return false end, 50)]])
  local after = T.lua([[local g = ...; local s = require("snips"); return { gen = s.generation, err = vim.tbl_count(s.errors) > 0 }]], gen1)
  T.ok("md   broken sets.lua → error reported, old snippets kept", after.gen == gen1 and after.err)
  T.case("old snippets of that file still expand", "md", "$‸$", { "zzq", "$\\hotreload{‸}$" })
  T.case("…all of them", "md", "$‸$", { "uu", "$\\cup‸$" })
  local report = T.lua([[return table.concat(require("snips.report").lines(), "\n")]])
  T.ok("     :SnippetReport shows the error", report:find("LOAD ERRORS", 1, true) ~= nil and report:find("sets.lua", 1, true) ~= nil)
  T.lua([[
    vim.cmd.edit(...)
    local buf = vim.api.nvim_get_current_buf()
    vim.api.nvim_buf_set_lines(buf, 0, 1, false, {})
    vim.cmd("silent write")
  ]], path)
  T.lua([[vim.wait(8000, function() return vim.tbl_count(require("snips").errors) == 0 end, 20)]])

  -- saved by another program (the folder is watched)
  local gen2 = T.lua([[return require("snips").generation]])
  local src = table.concat(vim.fn.readfile(path), "\n")
  src = src:gsub('\n  math%("uu"', '\n  math("zzw", [[\\external]], "new"),\n  math("uu"', 1)
  local fd = assert(io.open(path, "w"))
  fd:write(src)
  fd:close()
  local ok2 = T.lua([[
    local g = ...
    return vim.wait(8000, function() return require("snips").generation > g end, 20)
  ]], gen2)
  T.ok("md   sets.lua changed by another program → reloaded", ok2 == true)
  T.case("…and its new trigger works", "md", "$‸$", { "zzw", "$\\external‸$" })

  -- a new file in the folder
  local newfile = T.lua([[return require("snips").dir() .. "/tex/zz_new.lua"]])
  fd = assert(io.open(newfile, "w"))
  fd:write('local h = require("snips.helpers")\nreturn h.group("Test", { h.math("zzn", [[\\newfile]], "new") })\n')
  fd:close()
  local gen3 = T.lua([[return require("snips").generation]])
  T.lua([[require("snips").reload_file(...)]], newfile)
  local ok3 = T.lua([[local g = ...; return require("snips").generation > g]], gen3)
  T.ok("md   a new snippet file is picked up", ok3 == true)
  T.case("…and its trigger works", "md", "$‸$", { "zzn", "$\\newfile‸$" })
end

-- Ntex Studio: the %< %> escape, Space n / Space N / :SnippetStudio -----------------
local function studio(T)
  io.write("\n-- Ntex Studio (escapes in snippet bodies; launching, never for real in tests)\n")
  local toks = T.lua([[
    local t = require("snips.helpers").parse("a %<%> b %<3%> %%<c>")
    return t
  ]])
  T.ok("     helpers.parse: %< %> are literal < >", #toks == 1 and toks[1].text == "a <> b <3> %<c>", vim.inspect(toks))
  -- a snippet file using the escape, in the temp copy of snippets/
  local file = T.lua([[return require("snips").dir() .. "/tex/zz_escape.lua"]])
  local fd = assert(io.open(file, "w"))
  fd:write('local h = require("snips.helpers")\nreturn h.group("Test", {\n')
  fd:write('  h.math("zze", [[a %<%> b]], "new"),\n')
  fd:write('  h.math("zzf", [[%<<>%>]], "new"),\n')
  fd:write('  h.text("zzg", [[x %<1%> y<>]], "new"),\n')
  fd:write('})\n')
  fd:close()
  T.ok("md   a file with %< %> loads", T.lua([[return require("snips").reload_file(...)]], file) == true)
  T.case("%<%> → a literal <>", "md", "$‸$", { "zze", "$a <> b‸$" })
  T.case("%<<>%> → <‸> (a tabstop between literal angles)", "tex", "$‸$", { "zzf", "$<‸>$" })
  T.case("%<1%> in text stays <1>, the real tabstop still works", "tex", "Let ‸", { "zzg", "Let x <1> y‸" })
  os.remove(file)

  -- launching: TEXNVIM_NO_VIEWER=1 here, so nothing starts; the would-be
  -- command lines are recorded in require("config.studio").launched
  local function last()
    return T.lua([[local l = require("config.studio").launched; return l[#l] or {}]])
  end
  local has = T.lua([[return { cmd = vim.fn.exists(":SnippetStudio") == 2, n = vim.fn.maparg("<leader>n", "n") ~= "", x = vim.fn.maparg("<leader>n", "x") ~= "", N = vim.fn.maparg("<leader>N", "n") ~= "" }]])
  T.ok("     :SnippetStudio, Space n (normal + visual), Space N are defined", has.cmd and has.n and has.x and has.N, vim.inspect(has))
  T.open("tex", "x ‸")
  T.rpc("nvim_input", "<Esc>")
  T.sync()
  T.rpc("nvim_input", " n")
  T.sync()
  local a = last()
  T.ok("tex  Space n → ntex-studio --page snippet --ft tex (not launched)", a[1] and a[1]:match("ntex%-studio$") ~= nil and table.concat(a, " ", 2) == "--page snippet --ft tex", vim.inspect(a))
  T.rpc("nvim_input", " N")
  T.sync()
  T.ok("tex  Space N → --page inbox", table.concat(last(), " ", 2) == "--page inbox --ft tex", vim.inspect(last()))
  T.lua([[vim.cmd("SnippetStudio block")]])
  T.ok("     :SnippetStudio block → --page block", table.concat(last(), " ", 2):match("^%-%-page block") ~= nil, vim.inspect(last()))
  -- visual: the selection goes through a temp file, with the context
  T.open("tex", "so $‸a+b$ here")
  T.rpc("nvim_input", "<Esc>")
  T.sync()
  T.lua([[
    local r = vim.api.nvim_win_get_cursor(0)[1]
    vim.api.nvim_win_set_cursor(0, { r, 4 })
    vim.cmd("normal! v2l")
  ]])
  T.sync()
  T.rpc("nvim_input", " n")
  T.sync()
  local v = last()
  local args = table.concat(v, " ", 2)
  local out = args:match("%-%-output%-file (%S+)")
  T.ok("tex  visual Space n → --output-file, --context math, --ft tex", out ~= nil and args:find("--context math", 1, true) ~= nil and args:find("--ft tex", 1, true) ~= nil, args)
  T.ok("     …the selection file is under $TMPDIR and cleaned up when not launched", out ~= nil and out:find(T.lua([[return vim.uv.os_tmpdir()]]), 1, true) == 1 and vim.uv.fs_stat(out) == nil, tostring(out))
  local mode = T.lua([[return vim.fn.mode()]])
  T.ok("     …and visual mode is left", mode == "n", mode)
  T.rpc("nvim_input", "<Esc>")
  T.sync()
end

-- Ntex Figures (lua/config/figures.lua): Space g, .fig files, the live PDF follows ---
-- Never launches the window (TEXNVIM_NO_VIEWER=1: would-be launches are recorded in
-- figures.launched); ntex-fig is a fake on PATH; nothing compiles
-- (TEXNVIM_NO_COMPILE=1: recompiles are recorded in figures.recompiles).
local function figures(T)
  io.write("\n-- Ntex Figures (Space g, .fig files, figure PDFs recompile the .tex; never launched)\n")
  -- (realpath: on macOS $TMPDIR is under /var, a symlink to /private/var; buffer names are resolved)
  local root = T.lua([[local r = ...; vim.fn.mkdir(r, "p"); return vim.uv.fs_realpath(r)]], T.tmpdir .. "/figs")
  -- a fake ntex-fig: check reports BAD lines as errors and WOBBLY lines as
  -- warnings; build writes the PDF next to the .fig (atomically, like the real one)
  T.lua([[
    local root = ...
    local bin = root .. "/bin"
    vim.fn.mkdir(bin, "p")
    vim.fn.writefile({
      "#!/bin/sh",
      "echo \"$*\" >> \"$(dirname \"$0\")/calls.log\"",
      "case \"$1\" in",
      "check)",
      "  grep -n BAD \"$2\" | while IFS=: read n rest; do echo \"$2:$n:3: error: line $n: \\`BAD\\` is not a statement\"; done",
      "  grep -n WOBBLY \"$2\" | while IFS=: read n rest; do echo \"warning: line $n: unknown option \\`wobbly\\`\"; done",
      "  ;;",
      "new)",
      "  [ -n \"$FAKE_NO_NEW\" ] && exit 2",
      "  [ -e \"$2\" ] && exit 0",
      "  mkdir -p \"$(dirname \"$2\")\" && printf '%% Ntex Figures (fake ntex-fig new)\\nA -> B\\n' > \"$2\"",
      "  ;;",
      "build)",
      "  pdf=\"${2%.fig}.pdf\"",
      "  printf '%%PDF-1.4 fake %s %s\\n' \"$$\" \"$(date +%s)\" > \"$pdf.part\" && mv \"$pdf.part\" \"$pdf\"",
      "  ;;",
      "esac",
    }, bin .. "/ntex-fig")
    vim.uv.fs_chmod(bin .. "/ntex-fig", 493)
    vim.env.PATH = bin .. ":" .. vim.env.PATH
    _G.__fig_msgs = {}
    local orig = vim.notify
    vim.notify = function(m, l, o)
      table.insert(_G.__fig_msgs, tostring(m))
      return orig(m, l, o)
    end
  ]], root)
  local function calls()
    return T.lua([[local p = ... .. "/bin/calls.log"; return vim.fn.filereadable(p) == 1 and vim.fn.readfile(p) or {}]], root)
  end
  local function last()
    return T.lua([[local l = require("config.figures").launched; return l[#l] or {}]])
  end
  local function normal_keys(keys)
    T.rpc("nvim_input", "<Esc>")
    T.sync()
    T.rpc("nvim_input", keys)
    T.sync()
    T.lua([[vim.wait(200, function() return false end)]])
  end
  --- edit a real file under root (written first) with the cursor at row, col
  local function edit(rel, lines, row, col)
    return T.lua([[
      local path, lines, row, col = ...
      vim.fn.mkdir(vim.fs.dirname(path), "p")
      vim.fn.writefile(lines, path)
      vim.cmd("silent! %bwipeout!")
      vim.cmd.edit(path)
      vim.b.texnvim_template_done = true
      vim.api.nvim_win_set_cursor(0, { row or 1, col or 0 })
      return vim.bo.filetype
    ]], root .. "/" .. rel, lines, row, col)
  end

  -- the key, and no clash ----------------------------------------------------
  edit("doc/a.tex", { "x" })
  local maps = T.lua([[
    local out = {}
    for _, mode in ipairs({ "n", "v", "x", "s", "o", "i" }) do
      for _, m in ipairs(vim.api.nvim_get_keymap(mode)) do
        if m.lhs:sub(1, 2) == " g" then out[#out + 1] = mode .. "[" .. m.lhs .. "]" end
      end
      for _, m in ipairs(vim.api.nvim_buf_get_keymap(0, mode)) do
        if m.lhs:sub(1, 2) == " g" then out[#out + 1] = mode .. "[" .. m.lhs .. "] (buffer)" end
      end
    end
    return { maps = out, cmd = vim.fn.exists(":Figure") == 2 }
  ]])
  T.ok("     Space g is mapped (normal), nothing else starts with Space g; :Figure exists", #maps.maps == 1 and maps.maps[1] == "n[ g]" and maps.cmd, vim.inspect(maps))

  -- detection: the path on an \includegraphics line ---------------------------
  local det = T.lua([[
    local f = require("config.figures")
    local dir = ...
    local function at(line, col)
      local buf = vim.api.nvim_create_buf(false, true)
      vim.api.nvim_buf_set_lines(buf, 0, -1, false, { line })
      local r = f.figure_at_cursor(buf, 1, col or 0, dir)
      vim.api.nvim_buf_delete(buf, { force = true })
      return r or "nil"
    end
    return {
      plain = at("\\includegraphics{figures/kernel}"),
      pdf = at("\\includegraphics{figures/kernel.pdf}"),
      opts = at("  \\includegraphics[width=0.5\\textwidth, angle=0]{figures/kernel.pdf}"),
      dot = at("\\includegraphics[scale=2]{./figures/kernel}"),
      up = at("\\includegraphics{../figures/shared}"),
      sp = at("\\includegraphics [scale=2] {figures/kernel}"),
      png = at("\\includegraphics{figures/photo.png}"),
      other = at("\\includegraphics{img/kernel}"),
      second = at("\\includegraphics{figures/a} \\quad \\includegraphics{figures/b}", 40),
      md = at("![the cycle](figures/c6.pdf)"),
      sub = at("\\includegraphics{figures/sub/deep}"),
      subopt = at("\\includegraphics[width=3cm]{figures/sub/deep.pdf}"),
      deeper = at("\\includegraphics{./figures/a/b/c/d}"),
      escape = at("\\includegraphics{figures/../kernel}"),
      trailing = at("\\includegraphics{figures/sub/}"),
      subpng = at("\\includegraphics{figures/sub/photo.png}"),
    }
  ]], root .. "/doc")
  local K = root .. "/doc/figures/kernel.fig"
  T.ok("     {figures/kernel} → doc/figures/kernel.fig", det.plain == K, det.plain)
  T.ok("     {figures/kernel.pdf} → the same .fig", det.pdf == K, det.pdf)
  T.ok("     [width=0.5\\textwidth, angle=0]{figures/kernel.pdf} → the same", det.opts == K, det.opts)
  T.ok("     [scale=2]{./figures/kernel} → the same", det.dot == K, det.dot)
  T.ok("     \\includegraphics [scale=2] {figures/kernel} (spaces) → the same", det.sp == K, det.sp)
  T.ok("     {../figures/shared} → resolved against the .tex's folder", det.up == root .. "/figures/shared.fig", det.up)
  T.ok("     {figures/photo.png} and {img/kernel} are not figures", det.png == "nil" and det.other == "nil", det.png .. " " .. det.other)
  T.ok("     two on a line → the one under the cursor", det.second == root .. "/doc/figures/b.fig", det.second)
  T.ok("     .md ![…](figures/c6.pdf) → figures/c6.fig", det.md == root .. "/doc/figures/c6.fig", det.md)
  T.ok("     {figures/sub/deep} → doc/figures/sub/deep.fig", det.sub == root .. "/doc/figures/sub/deep.fig", det.sub)
  T.ok("     [width=3cm]{figures/sub/deep.pdf} → the same", det.subopt == root .. "/doc/figures/sub/deep.fig", det.subopt)
  T.ok("     {./figures/a/b/c/d} → any depth under figures/", det.deeper == root .. "/doc/figures/a/b/c/d.fig", det.deeper)
  T.ok("     {figures/../kernel}, {figures/sub/}, {figures/sub/photo.png} are not figures",
    det.escape == "nil" and det.trailing == "nil" and det.subpng == "nil", det.escape .. " " .. det.trailing .. " " .. det.subpng)

  -- Space g on an \includegraphics line: launches that figure (stubbed) ------
  edit("doc/a.tex", { "\\documentclass{article}", "\\begin{document}", "  \\includegraphics[width=4cm]{./figures/kernel.pdf}", "\\end{document}" }, 3, 4)
  local before = T.lua([[return vim.api.nvim_buf_get_lines(0, 0, -1, false)]])
  normal_keys(" g")
  T.lua([[local p = ...; vim.wait(3000, function() return vim.uv.fs_stat(p) ~= nil end, 20)]], root .. "/doc/figures/kernel.pdf")
  local a = last()
  T.ok("tex  Space g on the line → ntex-studio --figure <abs path> (not launched)", a[1] ~= nil and a[1]:match("ntex%-studio$") ~= nil and a[2] == "--figure" and a[3] == K, vim.inspect(a))
  local after = T.lua([[return vim.api.nvim_buf_get_lines(0, 0, -1, false)]])
  T.ok("     …the buffer is unchanged", vim.deep_equal(before, after), vim.inspect(after))
  local made = T.lua([[local p = ...; return vim.fn.filereadable(p) == 1 and vim.fn.readfile(p)[1] or "missing"]], K)
  T.ok("     …the missing .fig is made by `ntex-fig new <path>`", made:match("^%% Ntex Figures %(fake ntex%-fig new%)") ~= nil
    and vim.tbl_contains(calls(), "new " .. K), made .. " " .. vim.inspect(calls()))
  T.ok("     …and its missing PDF is built (ntex-fig build)", vim.tbl_contains(calls(), "build " .. K), vim.inspect(calls()))

  -- Space g on a subfolder figure: opens figures/sub/deep.fig, not a new fig-N --
  local D = root .. "/doc/figures/sub/deep.fig"
  edit("doc/s.tex", { "Text.", "  \\includegraphics[width=3cm]{figures/sub/deep.pdf}", "More." }, 2, 2)
  T.lua([[_G.__asked = nil; vim.ui.input = function(opts, cb) _G.__asked = opts; cb(opts.default) end]])
  before = T.lua([[return vim.api.nvim_buf_get_lines(0, 0, -1, false)]])
  normal_keys(" g")
  T.lua([[local p = ...; vim.wait(3000, function() return vim.uv.fs_stat(p) ~= nil end, 20)]], root .. "/doc/figures/sub/deep.pdf")
  local sub = T.lua([[local p = ...; return { asked = _G.__asked ~= nil, lines = vim.api.nvim_buf_get_lines(0, 0, -1, false),
    exists = vim.fn.filereadable(p) == 1, fig1 = vim.fn.filereadable(vim.fs.dirname(p) .. "/../fig-1.fig") == 1 }]], D)
  T.ok("tex  Space g on {figures/sub/deep.pdf} → ntex-studio --figure …/figures/sub/deep.fig", last()[3] == D, vim.inspect(last()))
  T.ok("     …no name asked, buffer unchanged, no fig-1 made; sub/deep.fig made (ntex-fig new) and built",
    not sub.asked and vim.deep_equal(before, sub.lines) and not sub.fig1 and sub.exists
      and vim.tbl_contains(calls(), "new " .. D) and vim.tbl_contains(calls(), "build " .. D), vim.inspect({ sub, calls() }))

  -- no ntex-fig `new` (an older or missing ntex-fig): the built-in cheat sheet --
  local fb = T.lua([[
    local f, root = require("config.figures"), ...
    local out = {}
    -- an ntex-fig whose `new` fails
    vim.env.FAKE_NO_NEW = "1"
    local p1 = root .. "/doc5/figures/old-cli.fig"
    out.failed = f.ensure_file(p1) and vim.fn.readfile(p1)[1] or "missing"
    -- spec change (interaction pass): the template is two lines naming the figure
    out.failed_tpl = vim.deep_equal(vim.fn.readfile(p1), vim.split(f.template_for(p1), "\n", { plain = true, trimempty = true }))
    vim.env.FAKE_NO_NEW = nil
    -- no ntex-fig at all
    local orig = f.fig_bin
    f.fig_bin = function() return nil end
    local p2 = root .. "/doc5/figures/nested/no-cli.fig"
    out.none = f.ensure_file(p2) and vim.fn.readfile(p2)[1] or "missing"
    f.fig_bin = orig
    -- an existing file is never touched
    local p3 = root .. "/doc5/figures/mine.fig"
    vim.fn.writefile({ "A -> B % mine" }, p3)
    f.ensure_file(p3)
    out.kept = vim.fn.readfile(p3)[1]
    return out
  ]], root)
  T.ok("     ntex-fig `new` fails → the built-in TEMPLATE", fb.failed_tpl and fb.failed == "% Ntex Figures — this file draws figures/old-cli.pdf. Save to rebuild; Space g opens the window.", vim.inspect(fb))
  T.ok("     no ntex-fig → the built-in TEMPLATE (subfolders made)", fb.none == "% Ntex Figures — this file draws figures/nested/no-cli.pdf. Save to rebuild; Space g opens the window.", vim.inspect(fb))
  T.ok("     an existing .fig is left as it is", fb.kept == "A -> B % mine", vim.inspect(fb))

  -- Space g elsewhere: asks for a name, creates the file, inserts the block --
  T.lua([[vim.fn.mkdir(... .. "/doc2/figures", "p"); vim.fn.writefile({}, ... .. "/doc2/figures/fig-3.fig")]], root)
  edit("doc2/b.tex", { "\\begin{proof}", "  Consider the square.", "\\end{proof}" }, 2, 5)
  T.lua([[
    _G.__asked = nil
    vim.ui.input = function(opts, cb) _G.__asked = opts; cb("My Kernel Map!.fig") end
  ]])
  normal_keys(" g")
  local asked = T.lua([[return _G.__asked or {}]])
  T.ok("tex  Space g elsewhere asks for a name, default fig-4 (fig-3 exists)", asked.default == "fig-4", vim.inspect(asked))
  T.check("tex  …inserts the block below the line, with its indentation", table.concat({
    "\\begin{proof}",
    "  Consider the square.",
    "  \\begin{center}",
    "    \\includegraphics{figures/my-kernel-map}",
    "  \\end{center}",
    "\\end{proof}",
  }, "\n"))
  local nf = root .. "/doc2/figures/my-kernel-map.fig"
  local st = T.lua([[local p = ...; return { exists = vim.fn.filereadable(p) == 1, first = (vim.fn.readfile(p)[1] or ""), cur = vim.api.nvim_win_get_cursor(0), mode = vim.fn.mode() }]], nf)
  T.ok("     …name sanitised to figures/my-kernel-map.fig, made by `ntex-fig new <path>`", st.exists
    and st.first:match("^%% Ntex Figures %(fake ntex%-fig new%)") ~= nil and vim.tbl_contains(calls(), "new " .. nf), vim.inspect({ st, calls() }))
  T.ok("     …cursor on the \\includegraphics line, normal mode", st.cur[1] == 4 and st.mode == "n", vim.inspect(st))
  T.ok("     …and launches it", last()[3] == nf, vim.inspect(last()))
  -- a blank line is replaced
  edit("doc2/c.tex", { "Text.", "", "More." }, 2, 0)
  T.lua([[vim.ui.input = function(opts, cb) _G.__asked = opts; cb(opts.default) end]])
  normal_keys(" g")
  T.check("tex  on a blank line: the block takes its place (default name fig-4)", table.concat({
    "Text.", "\\begin{center}", "  \\includegraphics{figures/fig-4}", "\\end{center}", "More." }, "\n"))
  -- cancelled: nothing happens
  T.lua([[vim.ui.input = function(opts, cb) cb(nil) end]])
  local n0 = #T.lua([[return require("config.figures").launched]])
  edit("doc2/d.tex", { "Text." }, 1, 0)
  normal_keys(" g")
  T.check("tex  cancelled (Esc at the prompt): nothing changes", "Text.")
  T.ok("     …nothing launched", #T.lua([[return require("config.figures").launched]]) == n0)
  -- .md works the same
  T.lua([[vim.ui.input = function(opts, cb) cb("cyc") end]])
  edit("doc2/n.md", { "# Notes", "Some text." }, 2, 0)
  normal_keys(" g")
  T.check("md   Space g in .md inserts the same block", table.concat({ "# Notes", "Some text.", "\\begin{center}", "  \\includegraphics{figures/cyc}", "\\end{center}" }, "\n"))
  -- :Figure NAME
  edit("doc2/e.tex", { "Text.", "" }, 2, 0)
  T.lua([[vim.cmd("Figure dihedral")]])
  T.sync()
  T.check("     :Figure dihedral → figures/dihedral.fig and the block", "Text.\n\\begin{center}\n  \\includegraphics{figures/dihedral}\n\\end{center}")

  -- unsaved buffer: one line, nothing made --------------------------------------
  T.lua([[vim.cmd("silent! %bwipeout!"); vim.cmd("enew"); vim.bo.filetype = "tex"; _G.__fig_msgs = {}]])
  n0 = #T.lua([[return require("config.figures").launched]])
  normal_keys(" g")
  local msgs = T.lua([[return _G.__fig_msgs]])
  T.ok("tex  unsaved buffer: says so in one line, launches nothing", #msgs == 1 and msgs[1]:find("save this file first", 1, true) ~= nil and not msgs[1]:find("\n")
    and #T.lua([[return require("config.figures").launched]]) == n0, vim.inspect(msgs))

  -- .fig buffers: filetype, syntax, diagnostics on save ------------------------
  local fig = root .. "/doc3/figures/sq.fig"
  local ft = edit("doc3/figures/sq.fig", { "% square", "A -f-> B", "BAD line here", "A -> C", "C -k-> D WOBBLY" }, 1, 0)
  local info = T.lua([[return { cs = vim.bo.commentstring, syn = vim.b.current_syntax,
    arrow = vim.fn.synIDattr(vim.fn.synID(2, 3, 1), "name"), comment = vim.fn.synIDattr(vim.fn.synID(1, 3, 1), "name") }]])
  T.ok("fig  .fig → filetype ntexfig, commentstring %, syntax on", ft == "ntexfig" and info.cs == "% %s" and info.syn == "ntexfig"
    and info.arrow == "ntexfigArrow" and info.comment == "ntexfigComment", ft .. " " .. vim.inspect(info))
  local diag = T.lua([[
    local f = require("config.figures")
    f.last_check = nil
    vim.cmd("silent write")
    vim.wait(5000, function() return f.last_check ~= nil and f.running.checks == 0 and f.running.builds == 0 end, 20)
    local out = {}
    for _, d in ipairs(vim.diagnostic.get(0, { namespace = f.ns })) do
      out[#out + 1] = { lnum = d.lnum + 1, col = d.col, sev = d.severity, msg = d.message }
    end
    table.sort(out, function(a, b) return a.lnum < b.lnum end)
    return { list = out, mode = vim.api.nvim_get_mode() }
  ]])
  local l = diag.list
  T.ok("fig  save → ntex-fig check: an error on line 3, a warning on line 5", #l == 2 and l[1].lnum == 3 and l[1].sev == 1 and l[1].col == 2
    and l[1].msg == "`BAD` is not a statement" and l[2].lnum == 5 and l[2].sev == 2 and l[2].msg == "unknown option `wobbly`", vim.inspect(l))
  T.ok("     …and ntex-fig build ran, in the background (no Press ENTER)", vim.tbl_contains(calls(), "build " .. fig) and diag.mode.blocking == false
    and T.lua([[return vim.uv.fs_stat(...) ~= nil]], root .. "/doc3/figures/sq.pdf"), vim.inspect({ calls(), diag.mode }))
  diag = T.lua([[
    local f = require("config.figures")
    vim.api.nvim_buf_set_lines(0, 2, 3, false, { "B -> D" })
    vim.api.nvim_buf_set_lines(0, 4, 5, false, { "C -k-> D" })
    f.last_check = nil
    vim.cmd("silent write")
    vim.wait(5000, function() return f.last_check ~= nil and f.running.checks == 0 end, 20)
    return #vim.diagnostic.get(0, { namespace = f.ns })
  ]])
  T.ok("     fixed and saved → the diagnostics clear", diag == 0, tostring(diag))
  local parsed = T.lua([[
    local f = require("config.figures")
    local p = f.parse_check
    local function one(t) local d = p(t, 10)[1]; return d and { d.lnum + 1, d.severity, d.message } or {} end
    local hidden = #p("x.fig:2:5: note: line 2: `A` is new here, so it became a node.", 10)
    f.show_notes = true
    local gcc = one("x.fig:2:5: note: guessed a position")
    f.show_notes = false
    return {
      hidden = hidden,
      gcc = gcc,
      plain = one("line 4: `->>` needs something to point at."),
      json = one('[{"severity":"Warning","span":{"line":7,"col":2},"message":"line 7: unknown option `wobbly`"}]'),
      real = one("/My Notes/figures/sq.fig:4:2: warning: line 4: unknown option `wobbly`"),
      none = #p("2 errors, 1 warning\n", 10),
    }
  ]])
  T.ok("     check output forms: ntex-fig's file:line:col: severity: (spaces in the path too), 'line N: …', JSON; summaries skipped, notes hidden unless show_notes",
    vim.deep_equal(parsed.plain, { 4, 1, "`->>` needs something to point at." }) and vim.deep_equal(parsed.json, { 7, 2, "unknown option `wobbly`" })
    and vim.deep_equal(parsed.gcc, { 2, 4, "guessed a position" }) and parsed.none == 0 and parsed.hidden == 0
    and vim.deep_equal(parsed.real, { 4, 2, "unknown option `wobbly`" }), vim.inspect(parsed))
  -- Space g in a .fig: its window
  normal_keys(" g")
  T.ok("fig  Space g in a .fig → ntex-studio --figure <this file>", last()[2] == "--figure" and last()[3] == fig, vim.inspect(last()))

  -- the live PDF follows figures ------------------------------------------------
  local tex = root .. "/doc4/main.tex"
  T.lua([[vim.fn.mkdir(... .. "/doc4/figures", "p")]], root)
  edit("doc4/main.tex", { "\\documentclass{article}", "\\begin{document}", "\\includegraphics{figures/w1}", "\\end{document}" })
  local w = T.lua([[
    local f, root = require("config.figures"), ...
    f.recompiles = {}
    local dir = root .. "/doc4/figures"
    local function wait_n(n, ms) vim.wait(ms or 2000, function() return #f.recompiles >= n end, 20) return #f.recompiles end
    local out = { watching = f.watches[dir] ~= nil }
    -- three quick writes of the figure's PDF (the window saving) → one recompile
    for i = 1, 3 do
      vim.fn.writefile({ "%PDF fake " .. i }, dir .. "/w1.pdf")
      vim.wait(30, function() return false end)
    end
    out.n1 = wait_n(1)
    vim.wait(600, function() return false end)
    out.n1b = #f.recompiles
    out.first = f.recompiles[1]
    -- a PDF the .tex doesn't include → nothing
    vim.fn.writefile({ "%PDF other" }, dir .. "/other.pdf")
    vim.wait(800, function() return false end)
    out.n2 = #f.recompiles
    return out
  ]], root)
  T.ok("tex  a .tex mentioning figures/ watches its figures/ folder", w.watching, vim.inspect(w))
  T.ok("     figure PDF written 3× quickly → one recompile of that .tex (debounced)", w.n1 == 1 and w.n1b == 1 and w.first and w.first.tex == tex
    and w.first.pdf == root .. "/doc4/figures/w1.pdf", vim.inspect(w))
  T.ok("     a PDF it doesn't include → no recompile", w.n2 == 1, vim.inspect(w))
  -- a .fig save → ntex-fig build → new PDF → the .tex recompiles (its buffer still open)
  local chain = T.lua([[
    local f, root = require("config.figures"), ...
    local hooked = {}
    f.recompile_hook = function(buf, pdf) hooked[#hooked + 1] = vim.api.nvim_buf_get_name(buf) end
    local n = #f.recompiles
    vim.cmd.edit(root .. "/doc4/figures/w1.fig")
    vim.api.nvim_buf_set_lines(0, 0, -1, false, { "A -> B" })
    vim.cmd("silent write")
    vim.wait(4000, function() return #f.recompiles > n end, 20)
    f.recompile_hook = nil
    return { n = #f.recompiles - n, hooked = hooked }
  ]], root)
  T.ok("fig  saving w1.fig → build → w1.pdf changes → main.tex recompiles (hook stub)", chain.n == 1 and chain.hooked[1] == tex, vim.inspect(chain))
  local gone = T.lua([[
    local f = require("config.figures")
    vim.cmd("silent! %bwipeout!")
    return vim.tbl_count(f.watches)
  ]])
  T.ok("     buffers closed → the watches stop", gone == 0, tostring(gone))
end

-- Per-keystroke cost ----------------------------------------------------------------
local function perf(T)
  io.write("\n-- per-keystroke timing\n")
  T.lua([[
    local ls = require("luasnip")
    if not _G.__ls_orig then
      _G.__ls_orig = ls.expand_auto
      ls.expand_auto = function(...)
        local t = vim.uv.hrtime()
        _G.__ls_orig(...)
        table.insert(_G.__ls_times, (vim.uv.hrtime() - t) / 1e3)
      end
    end
  ]])
  local text = "a + b = c d e f g h i j k l m n o p q r s t u v w y z + a + b + c + d + e + f + g"
  local stats = {}
  for _, ft in ipairs(FTS) do
    for _, where in ipairs({ "math", "text" }) do
      local start = where == "math" and "$‸$" or "‸"
      if ft == "tex" and where == "math" then
        start = "Some words before. $‸$ and after"
      end
      T.open(ft, start)
      T.lua([[_G.__ls_times = {}]])
      T.timings.e2e = {}
      T.type_keys(text)
      local us = T.lua([[return _G.__ls_times]])
      table.sort(us)
      local e2e = T.timings.e2e
      table.sort(e2e)
      local function pct(t, p)
        return t[math.max(1, math.ceil(#t * p))] or 0
      end
      local s = {
        ft = ft, where = where, n = #us,
        median_us = pct(us, 0.5), p95_us = pct(us, 0.95), max_us = us[#us] or 0,
        e2e_median_ms = pct(e2e, 0.5), e2e_p95_ms = pct(e2e, 0.95),
      }
      stats[#stats + 1] = s
      io.write(("  %-3s %-4s snippet check per key: median %.0f µs, p95 %.0f µs, max %.0f µs (n=%d); key→processed incl. RPC: median %.2f ms, p95 %.2f ms\n"):format(
        ft, where, s.median_us, s.p95_us, s.max_us, s.n, s.e2e_median_ms, s.e2e_p95_ms))
    end
  end
  local worst = 0
  for _, s in ipairs(stats) do
    worst = math.max(worst, s.p95_us)
  end
  T.ok("     p95 snippet check < 2 ms per key", worst < 2000, ("(p95 %.0f µs)"):format(worst))
  return stats
end

-- :checkhealth -------------------------------------------------------------------------
-- VimTeX's health check asks macOS about Skim (osascript), which can show a
-- permission dialog, so it is never run here; Skim is checked by path instead.
local function health(T)
  io.write("\n-- :checkhealth snacks luasnip (VimTeX's is skipped: it talks to Skim)\n")
  local skim = vim.uv.fs_stat("/Applications/Skim.app") ~= nil
  T.ok("     Skim.app is installed (path check instead of VimTeX's osascript)", skim)
  local vimtex = T.lua([[return vim.fn.exists("*vimtex#syntax#in_mathzone") == 1 or vim.fn.exists(":VimtexCompile") == 2 or pcall(vim.fn["vimtex#syntax#in_mathzone"])]])
  T.ok("     VimTeX is loaded", vimtex == true)
  local lines = T.lua([[
    vim.cmd("silent checkhealth snacks luasnip")
    for _, b in ipairs(vim.api.nvim_list_bufs()) do
      if vim.bo[b].filetype == "checkhealth" then
        return vim.api.nvim_buf_get_lines(b, 0, -1, false)
      end
    end
    return {}
  ]])
  -- only the parts this config uses: snacks.image, VimTeX, LuaSnip (the other
  -- snacks modules are deliberately not enabled and just say "setup {disabled}")
  local section = ""
  local n = 0
  local wanted = { ["Snacks.image"] = true, ["luasnip"] = true }
  local seen = {}
  for _, l in ipairs(lines) do
    local head = l:match("^(%S.-)%s+~$") or l:match("^(%S.-)~$")
    if head then
      section = vim.trim(head):gsub(":.*", "")
    end
    local sec = section:match("^(Snacks%.image)") or section:match("^(VimTeX)") or section:match("^(luasnip)") or section
    if wanted[sec] then
      seen[sec] = true
      if l:find("WARNING") or l:find("ERROR") then
        n = n + 1
        io.write(("  [%s] %s\n"):format(sec, vim.trim(l)))
      end
    end
  end
  io.write("  sections checked: " .. table.concat(vim.tbl_keys(seen), ", ") .. "\n")
  if n == 0 then
    io.write("  (no warnings or errors)\n")
  end
  T.ok("     checkhealth ran (" .. #lines .. " lines, " .. n .. " warnings/errors listed above)", #lines > 0)

  -- Render one formula through snacks' own pipeline (template → tectonic →
  -- PDF → ImageMagick/gs → PNG). Nothing is displayed; this only proves the
  -- conversion works on this Mac.
  io.write("\n-- snacks math rendering pipeline (no display)\n")
  local r = T.lua([[
    local img = { content = "$\\int_0^1 x^2 \\, dx = \\frac{1}{3}$", ext = "math.tex" }
    Snacks.image.doc.transforms.latex(img, { buf = vim.api.nvim_get_current_buf() })
    local dir = vim.fn.stdpath("cache") .. "/texnvim-test"
    vim.fn.mkdir(dir, "p")
    local src = dir .. "/probe-" .. os.time() .. ".math.tex"
    vim.fn.writefile(vim.split(img.content, "\n"), src)
    local done = false
    local conv = Snacks.image.convert.convert({ src = src, on_done = function() done = true end })
    local t0 = vim.uv.hrtime()
    conv:run()
    vim.wait(90000, function() return done end, 50)
    local file = conv.file
    local res = { done = done, err = conv:error(), file = file, ms = (vim.uv.hrtime() - t0) / 1e6 }
    res.exists = file and vim.uv.fs_stat(file) ~= nil
    if res.exists then
      res.size = vim.trim(vim.fn.system({ "magick", "identify", "-format", "%wx%h", file }))
      vim.fn.delete(file)
    end
    vim.fn.delete(src)
    return res
  ]])
  io.write(("  rendered: %s  %s  (%.0f ms)%s\n"):format(tostring(r.file), tostring(r.size), r.ms or 0, r.err and ("  error: " .. r.err) or ""))
  T.ok("     \\int_0^1 x^2 dx = 1/3 → PNG via tectonic + magick", r.done and r.exists and not r.err)
end

-- compile on save, viewer, template, :Scratch ----------------------------------------
local function system(T)
  local compile = vim.env.TEXNVIM_TEST_COMPILE == "1"
  io.write("\n-- template, :Scratch" .. (compile and ", compile on save" or " (compile on save skipped: set TEXNVIM_TEST_COMPILE=1)") .. "\n")
  local skim_before = vim.system({ "pgrep", "-xq", "Skim" }):wait().code == 0
  local dir = T.tmpdir .. "/compile"
  vim.fn.mkdir(dir, "p")
  T.rpc("nvim_input", "<Esc>")
  T.sync()
  local r = T.lua([[
    local dir = ...
    vim.cmd("silent! %bwipeout!")
    vim.cmd.edit(dir .. "/notes.tex")
    local lines = vim.api.nvim_buf_get_lines(0, 0, -1, false)
    local has_thm = false
    for _, l in ipairs(lines) do
      if l:find("\\usepackage{ntex-notes}", 1, true) then has_thm = true end -- theorems come from the theme
    end
    local first = lines[1]
    local cur = vim.api.nvim_win_get_cursor(0)
    vim.api.nvim_buf_set_lines(0, cur[1] - 1, cur[1], false, { "Let $x^2 + \\alpha_{3}$." })
    vim.cmd("silent write")
    local ok = false
    if vim.env.TEXNVIM_NO_COMPILE ~= "1" then
      ok = vim.wait(120000, function() return vim.uv.fs_stat(dir .. "/notes.pdf") ~= nil end, 200)
      vim.wait(1000, function() return false end, 100)
    end
    return { first = first, has_thm = has_thm, pdf = ok, written = vim.uv.fs_stat(dir .. "/notes.tex") ~= nil, view_automatic = vim.g.vimtex_view_automatic, method = vim.g.vimtex_compiler_method }
  ]], dir)
  T.ok("     new .tex gets the article template (" .. tostring(r.first) .. ")", r.first == "\\documentclass[11pt]{article}" and r.has_thm)
  T.ok("     :w writes the file", r.written)
  if compile then
    T.ok("     :w compiles with " .. tostring(r.method) .. " → notes.pdf", r.pdf)
    local skim_after = vim.system({ "pgrep", "-xq", "Skim" }):wait().code == 0
    T.ok("     Skim was not opened by the compile (view_automatic=" .. tostring(r.view_automatic) .. ")", skim_after == skim_before)
  end
  local sc = T.lua([[
    local dir = ...
    vim.cmd("Scratch " .. dir .. "/scratch")
    local name = vim.api.nvim_buf_get_name(0)
    return { name = name, first = vim.api.nvim_buf_get_lines(0, 0, 1, false)[1], ft = vim.bo.filetype }
  ]], dir)
  T.ok("     :Scratch opens a new .tex (" .. vim.fs.basename(sc.name) .. ")", sc.name:match("scratch%-%d+%-%d+%-%d+%-%d+%.tex$") ~= nil and sc.ft == "tex")

  io.write("\n-- conceal toggle (<F2>)\n")
  T.open("tex", "Text $\\alpha + \\beta‸$ end")
  local cc = T.lua([[
    local conceal = require("snips.conceal")
    local col = vim.api.nvim_get_current_line():find("\\alpha", 1, true)
    local function probe()
      vim.cmd("syntax sync fromstart")
      return vim.fn.synconcealed(1, col)
    end
    local before = probe()
    conceal.set(true, true)
    local on = probe()
    conceal.set(false, true)
    local off = probe()
    local pref = require("config.prefs").conceal
    return { default = conceal.enabled, before = before, on = on, off = off, pref = pref }
  ]])
  -- prefs.lua conceal decides the start state
  local want_on = cc.pref == true
  T.ok("     conceal starts " .. (want_on and "on" or "off") .. " (prefs.lua conceal = " .. tostring(cc.pref) .. ")",
    cc.before[1] == (want_on and 1 or 0))
  T.ok("     <F2> on: \\alpha shows as " .. tostring(cc.on[2]), cc.on[1] == 1 and cc.on[2] == "α")
  T.ok("     <F2> off again: source shown", cc.off[1] == 0)

  io.write("\n-- texlab completion through blink.cmp\n")
  T.open("tex", "\\documentclass{article}\n\\begin{document}\n\\section{A}\\label{sec:alpha}\n\\newcommand{\\myop}{x}\nSee ‸\n\\end{document}")
  local attached = T.lua([[
    return vim.wait(15000, function() return #vim.lsp.get_clients({ bufnr = 0, name = "texlab" }) > 0 end, 100)
  ]])
  T.ok("     texlab attached", attached)
  T.type_keys("\\ref{")
  local items = T.lua([[
    local blink = require("blink.cmp")
    local found = {}
    vim.wait(15000, function()
      local its = blink.get_items() or {}
      for _, it in ipairs(its) do
        if it.label and it.label:find("sec:alpha", 1, true) then
          found.label = it.label
        end
      end
      return found.label ~= nil
    end, 100)
    return { label = found.label, visible = blink.is_visible() }
  ]])
  T.ok("     \\ref{ offers sec:alpha (menu shown by itself: " .. tostring(items.visible) .. ")", items.label ~= nil)
  T.type_keys("<Esc>")
end

-- Option-Left / Option-Right / Option-Backspace, macOS word rules -----------
local function macos_keys(T)
  for _, ft in ipairs(FTS) do
    local I = function(s) return inl(ft, s) end
    io.write(("\n-- %s: macOS word keys (insert mode)\n"):format(ft))
    T.case("Option-Left → start of word", ft, "one two‸", { "<M-Left>", "one ‸two", "<M-Left>", "‸one two" })
    T.case("Option-Right → end of word", ft, "‸one two", { "<M-Right>", "one‸ two", "<M-Right>", "one two‸" })
    T.case("Option-Backspace deletes the word", ft, "one two‸", { "<M-BS>", "one ‸" })
    T.case("Option-Backspace: trailing spaces go with the word", ft, "one two  ‸", { "<M-BS>", "one ‸" })
    T.case("don't / 3.14 are one word", ft, "x don't‸ 3.14", { "<M-BS>", "x ‸ 3.14", "<M-Right>", "x  3.14‸" })
    T.case("LaTeX: \\ and { separate words", ft, I("\\frac{ab}‸"), { "<M-Left>", I("\\frac{‸ab}"), "<M-Left>", I("\\‸frac{ab}") })
    T.case("Option-Backspace in math keeps the backslash", ft, I("\\alpha + \\beta‸"), { "<M-BS>", I("\\alpha + \\‸") })
    T.case("Option-Left crosses a line break", ft, "one\n‸two", { "<M-Left>", "‸one\ntwo" })
    T.case("Option-Right crosses a line break", ft, "one‸\ntwo", { "<M-Right>", "one\ntwo‸" })
    -- what cmux/Ghostty actually send: Esc-b / Esc-f (<M-b> / <M-f>); must stay in insert mode
    T.case("cmux Option-Left (Esc-b) stays in insert", ft, "one two‸", { "<M-b>", "one ‸two", "X", "one X‸two" })
    T.case("cmux Option-Right (Esc-f) stays in insert", ft, "‸one two", { "<M-f>", "one‸ two", "X", "oneX‸ two" })
  end
  -- j/k must type into a selected snippet placeholder (select mode)
  for _, ft in ipairs(FTS) do
    local I = function(s) return inl(ft, s) end
    T.case("k types over the selected placeholder", ft, I("‸"), {
      "sum", I("\\sum‸"),
      "<Tab>", { I("\\sum_{i=1}^{N}"), mode = "s", sel = "i" },
      "k", I("\\sum_{k‸=1}^{N}"),
    })
  end
end

-- Castel's text environments: Tab-only at a line start in .tex (2026-10-01)
local function text_envs(T)
  io.write("\n-- tex: beg / enum / item / ali need Tab\n")
  T.case("enum<Tab> → enumerate", "tex", "‸", { "enum<Tab>", "\\begin{enumerate}\n\t\\item ‸\n\\end{enumerate}" })
  T.case("item<Tab> → itemize", "tex", "‸", { "item<Tab>", "\\begin{itemize}\n\t\\item ‸\n\\end{itemize}" })
  T.case("ali<Tab> → align*", "tex", "‸", { "ali<Tab>", "\\begin{align*}\n\t‸\n\\end{align*}" })
  T.case("beg<Tab> → name mirrored", "tex", "‸", { "beg<Tab>", { "\\begin{}\n\t\n\\end{}", mode = "i" }, "proof", "\\begin{proof‸}\n\t\n\\end{proof}" })
  T.case("typing 'items are' doesn't expand", "tex", "‸", { "items are", "items are‸" })
  T.case("typing 'began to' doesn't expand", "tex", "‸", { "began to", "began to‸" })
  T.case("'enum' mid-line + Tab doesn't expand (normal Tab)", "tex", "an enum‸", { "<Tab>", "an enum ‸" })
end

-- IDE features (lua/ide/): compile errors as diagnostics, texlab, Harper,
-- Copilot, the finder. Compiles with tectonic, so the driver runs this suite
-- with TEXNVIM_NO_VIEWER=1 (no Skim, no pane, no browser) and compiling on.
local function ide(T)
  local skim_before = vim.system({ "pgrep", "-xq", "Skim" }):wait().code == 0
  T.lua([[
    _G.__msgs = {}
    local orig = vim.notify
    vim.notify = function(m, l, o)
      table.insert(_G.__msgs, tostring(m))
      return orig(m, l, o)
    end
  ]])
  local guard = T.lua([[return { view = vim.g.vimtex_view_enabled, env = vim.env.TEXNVIM_NO_VIEWER, compile = vim.env.TEXNVIM_NO_COMPILE }]])
  T.ok("     TEXNVIM_NO_VIEWER=1: VimTeX's viewer is off (vimtex_view_enabled=" .. tostring(guard.view) .. ")", guard.env == "1" and guard.view == 0)

  -- compile errors ---------------------------------------------------------
  io.write("\n-- compile errors → diagnostics on the right line (tectonic)\n")
  local dir = T.tmpdir .. "/ide"
  local function compile(name, lines, extra)
    return T.lua([[
      local dir, name, lines, extra = ...
      local ce = require("ide.compile_errors")
      local root = dir .. "/" .. name
      vim.fn.mkdir(root, "p")
      for f, l in pairs(extra or {}) do vim.fn.writefile(l, root .. "/" .. f) end
      local path = root .. "/main.tex"
      if vim.api.nvim_buf_get_name(0) ~= path then
        vim.cmd("silent! %bwipeout!")
        vim.cmd.edit(path)
      end
      vim.b.texnvim_template_done = true
      vim.api.nvim_buf_set_lines(0, 0, -1, false, lines)
      local runs = ce.stats.runs
      local t0 = vim.uv.hrtime()
      vim.cmd("silent write")
      local done = vim.wait(90000, function() return ce.stats.runs > runs end, 50)
      local out = { done = done, ms = (vim.uv.hrtime() - t0) / 1e6, parse_ms = ce.stats.last_ms, list = {} }
      for _, b in ipairs(vim.api.nvim_list_bufs()) do
        for _, d in ipairs(vim.diagnostic.get(b, { namespace = ce.ns })) do
          local src = (vim.fn.readfile(vim.api.nvim_buf_get_name(b))[d.lnum + 1] or "")
          table.insert(out.list, { file = vim.fs.basename(vim.api.nvim_buf_get_name(b)), lnum = d.lnum + 1, sev = d.severity,
            msg = d.message, text = src:sub(d.col + 1, d.end_col or #src) })
        end
      end
      table.sort(out.list, function(a, b) return a.sev < b.sev end)
      return out
    ]], dir, name, lines, extra or {})
  end
  local function first(r, sev)
    for _, d in ipairs(r.list) do
      if d.sev == sev then return d end
    end
  end
  local function show(d)
    return d and ("%s:%d %q under %q"):format(d.file, d.lnum, d.msg, d.text) or "none"
  end
  local DOC = { "\\documentclass{article}", "\\usepackage{amsmath}", "\\begin{document}" }
  local function doc(body)
    local l = vim.deepcopy(DOC)
    vim.list_extend(l, body)
    l[#l + 1] = "\\end{document}"
    return l
  end
  local times = {}
  local cases = {
    { "undefined control sequence", "undef", doc({ "Hello.", "Let $x = \\foo y$ here." }), 5, "\\foo" },
    { "missing $", "dollar", doc({ "Hello.", "Then x^2 is positive.", "More." }), 5, "^" },
    { "unbalanced {", "brace", doc({ "Hello.", "\\textbf{bold text and more", "", "Next paragraph." }), 5, "\\textbf" },
    { "missing \\end{align}", "noend", doc({ "Hello.", "\\begin{align}", "  a &= b", "More text." }), 5, "\\begin{align}" },
  }
  for _, c in ipairs(cases) do
    local r = compile(c[2], c[3])
    times[#times + 1] = r.ms
    local d = first(r, 1)
    T.ok(("     %-28s → error on line %d: %s"):format(c[1], c[4], show(d)), r.done and d and d.lnum == c[4] and d.text == c[5])
  end
  -- fix the last one: a clean compile clears the errors
  local r = compile("noend", doc({ "Hello.", "\\begin{align}", "  a &= b", "\\end{align}", "More text." }))
  T.ok("     fixed → a clean compile clears them (" .. #r.list .. " left)", r.done and #r.list == 0)
  -- undefined reference: a warning, on the \ref
  r = compile("ref", doc({ "Hello.", "See \\ref{sec:nowhere} for more." }))
  local w = first(r, 2)
  T.ok("     undefined reference → warning on line 5: " .. show(w), r.done and w and w.lnum == 5 and w.text == "\\ref{sec:nowhere}" and not first(r, 1))
  -- an error in an \input file lands in that file
  r = compile("input", doc({ "Intro.", "\\input{chap}" }), { ["chap.tex"] = { "Chapter line one.", "Line two has \\bad here." } })
  local e = first(r, 1)
  T.ok("     error in an \\input file → chap.tex line 2: " .. show(e), r.done and e and e.file == "chap.tex" and e.lnum == 2 and e.text == "\\bad")
  -- overfull \hbox: hidden by default, a hint with box_warnings = true
  local box = doc({ "\\hbox to 1cm{wide wide wide wide wide wide text}", "Hello." })
  r = compile("box", box)
  T.ok("     overfull \\hbox hidden by default (prefs.lua box_warnings = false)", r.done and #r.list == 0, vim.inspect(r.list))
  T.lua([[require("config.prefs").box_warnings = true]])
  box[#box - 1] = "Hello again."
  r = compile("box", box)
  local h = first(r, 4)
  T.ok("     box_warnings = true → a hint on line 4: " .. show(h), r.done and h and h.lnum == 4 and #r.list == 1)
  T.lua([[require("config.prefs").box_warnings = false]])
  table.sort(times)
  io.write(("  compile + parse: median %.0f ms per compile (tectonic); log parse %.2f ms\n"):format(times[math.ceil(#times / 2)], r.parse_ms or 0))

  -- live PDF while typing: the error on the line being typed waits for Esc
  io.write("\n-- live PDF: an error on the line you are typing in waits for Esc\n")
  T.open("tex", table.concat(doc({ "Hello.", "Let $x = \\foo‸" }), "\n"))
  local st = T.lua([[
    local ce = require("ide.compile_errors")
    local runs = ce.stats.runs
    vim.api.nvim_input(" y$")
    local done = vim.wait(90000, function() return ce.stats.runs > runs end, 50)
    local row = vim.api.nvim_win_get_cursor(0)[1] - 1
    return { done = done, mode = vim.api.nvim_get_mode().mode, shown = #vim.diagnostic.get(0, { namespace = ce.ns, lnum = row }), held = #(ce.last[vim.api.nvim_get_current_buf()] or {}) }
  ]])
  T.ok(("     compiled after a pause, still in insert mode: error held back (shown %d, held %d)"):format(st.shown, st.held), st.done and st.mode == "i" and st.shown == 0 and st.held == 1)
  T.type_keys("<Esc>")
  local after = T.lua([[return #vim.diagnostic.get(0, { namespace = require("ide.compile_errors").ns, lnum = 4 })]])
  T.ok("     Esc → the error shows on that line", after == 1)

  -- the rest edits fragments that aren't whole documents: compiling them
  -- would only make VimTeX echo "failed mainfile detection" (a Press-ENTER
  -- prompt that eats the next key)
  T.lua([[vim.env.TEXNVIM_NO_COMPILE = "1"]])

  -- display ----------------------------------------------------------------
  io.write("\n-- how diagnostics show\n")
  local cfg = T.lua([[
    local c = vim.diagnostic.config()
    local nd = vim.fn.maparg("]d", "n", false, true)
    local pd = vim.fn.maparg("[d", "n", false, true)
    local src = function(m) return m.callback and debug.getinfo(m.callback, "S").source or (m.rhs or "") end
    return { sc = vim.wo.signcolumn, vt = type(c.virtual_text) == "table", sort = c.severity_sort, ul = c.underline,
      nd = src(nd), pd = src(pd) }
  ]])
  T.ok("     signs in the number column (signcolumn=" .. cfg.sc .. "), severity sorted, underline, end-of-line note", cfg.sc == "number" and cfg.sort == true and cfg.ul == true and cfg.vt)
  T.ok("     ]d / [d are Neovim's own (" .. vim.fs.basename(cfg.nd) .. ")", cfg.nd:find("defaults", 1, true) ~= nil and cfg.pd:find("defaults", 1, true) ~= nil)
  local fl = T.lua([[
    vim.fn.feedkeys(vim.keycode("<Space>d"), "x")
    local float
    for _, w in ipairs(vim.api.nvim_tabpage_list_wins(0)) do
      if vim.api.nvim_win_get_config(w).relative ~= "" then float = w end
    end
    local text = float and table.concat(vim.api.nvim_buf_get_lines(vim.api.nvim_win_get_buf(float), 0, -1, false), " ") or ""
    if float then vim.api.nvim_win_close(float, true) end
    return text
  ]])
  T.ok("     Space d → the line's message in a float: " .. fl, fl:find("Undefined control sequence", 1, true) ~= nil)

  -- texlab -----------------------------------------------------------------
  io.write("\n-- texlab while typing\n")
  T.open("tex", "\\documentclass{article}\n\\begin{document}\n\\section{A}\\label{sec:a}\n\\section{B}\\label{sec:a}\nSee \\ref{sec:missing}.‸\n\\end{document}")
  T.type_keys("<Esc>")
  local tl = T.lua([[
    local got = {}
    vim.wait(20000, function()
      got = {}
      for _, c in ipairs(vim.lsp.get_clients({ bufnr = 0, name = "texlab" })) do
        for _, d in ipairs(vim.diagnostic.get(0, { namespace = vim.lsp.diagnostic.get_namespace(c.id) })) do
          got[#got + 1] = (d.lnum + 1) .. ": " .. d.message
        end
      end
      return #got >= 3
    end, 100)
    table.sort(got)
    return got
  ]])
  local txt = table.concat(tl, "; ")
  T.ok("     duplicate label + undefined reference shown (" .. txt .. ")", txt:find("Duplicate label") and txt:find("Undefined reference") and not txt:find("Unused label"))

  -- Harper -----------------------------------------------------------------
  io.write("\n-- Harper (grammar and spelling)\n")
  local SAMPLE = table.concat({
    "\\documentclass{article}",
    "\\usepackage{amsmath,cleveref}",
    "\\begin{document}",
    "\\section{Compactness}\\label{sec:compact}",
    "Let $(X, d)$ be a metric space and let $K \\subseteq X$ be compact. By \\cref{thm:hb}, every",
    "sequence $(x_n)_{n \\in \\mathbb{N}}$ in $K$ has a convergent subsequence, so",
    "\\[",
    "  \\sup_{x \\in K} f(x) = \\max_{x \\in K} f(x) < \\infty .",
    "\\]",
    "We now prove teh claim, using \\eqref{eq:main} and the estimate",
    "\\begin{align}",
    "  \\|T x\\|_{L^2(\\Omega)} &\\le C \\|x\\|_{H^1_0(\\Omega)} \\label{eq:main}",
    "\\end{align}",
    "which holds for all $x \\in H^1_0(\\Omega)$, and \\( g(x) = f(x) \\) as well.",
    "\\end{document}‸",
  }, "\n")
  local function harper(ft, text)
    T.open(ft, text)
    T.type_keys("<Esc>")
    return T.lua([[
      local ctx = require("snips.context")
      local buf = vim.api.nvim_get_current_buf()
      local ok = vim.wait(20000, function() return #vim.lsp.get_clients({ bufnr = buf, name = "harper_ls" }) > 0 end, 50)
      local list = {}
      vim.wait(20000, function()
        local c = vim.lsp.get_clients({ bufnr = buf, name = "harper_ls" })[1]
        list = c and vim.diagnostic.get(buf, { namespace = vim.lsp.diagnostic.get_namespace(c.id) }) or {}
        for _, d in ipairs(list) do
          if d.message:find("definite article") or d.message:find("teh") then return true end
        end
        return false
      end, 100)
      local out = { attached = ok, spell = vim.wo.spell, flags = {}, in_math = {}, in_cmd = {} }
      for _, d in ipairs(list) do
        local line = vim.api.nvim_buf_get_lines(buf, d.lnum, d.lnum + 1, false)[1]
        local word = line:sub(d.col + 1, d.end_col)
        table.insert(out.flags, word)
        local c = ctx.get_at(buf, d.lnum, d.col + 1)
        if c and ctx.in_math(c.mode) then table.insert(out.in_math, word) end
        local before = line:sub(1, d.col)
        if before:match("\\%a*$") or before:match("\\%a+{[^}]*$") then table.insert(out.in_cmd, word) end
      end
      return out
    ]])
  end
  local hr = harper("tex", SAMPLE)
  local flags = table.concat(hr.flags, ", ")
  T.ok("     harper-ls attached to .tex, Vim spell off there (spell=" .. tostring(hr.spell) .. ")", hr.attached and hr.spell == false)
  T.ok("     flags “teh” in prose (all flags: " .. flags .. ")", vim.tbl_contains(hr.flags, "teh"))
  T.ok("     nothing flagged inside maths ($…$, \\[…\\], align, \\(…\\))", #hr.in_math == 0, table.concat(hr.in_math, ", "))
  T.ok("     nothing flagged in commands / labels / environment names", #hr.in_cmd == 0, table.concat(hr.in_cmd, ", "))
  io.write("  Harper flags on the sample paragraph: " .. flags .. "\n")
  local hm = harper("md", "# Notes\n\nLet $f \\colon X \\to \\mathbb{R}$ be continuous.\nWe now prove teh claim.\n\n$$\n\\sup_{x} f(x)\n$$‸")
  T.ok("     harper-ls attached to .md and flags “teh” (" .. table.concat(hm.flags, ", ") .. ")", hm.attached and vim.tbl_contains(hm.flags, "teh") and #hm.in_math == 0)
  local fix = T.lua([[
    local buf = vim.api.nvim_get_current_buf()
    local c = vim.lsp.get_clients({ bufnr = buf, name = "harper_ls" })[1]
    local d
    for _, x in ipairs(vim.diagnostic.get(buf, { namespace = vim.lsp.diagnostic.get_namespace(c.id) })) do
      if x.message:find("article") or vim.api.nvim_buf_get_lines(buf, x.lnum, x.lnum + 1, false)[1]:sub(x.col + 1, x.end_col) == "teh" then d = x end
    end
    vim.api.nvim_win_set_cursor(0, { d.lnum + 1, d.col })
    local params = vim.lsp.util.make_range_params(0, c.offset_encoding)
    params.context = { diagnostics = {}, triggerKind = 1 }
    local res = c:request_sync("textDocument/codeAction", params, 5000, buf)
    local titles = {}
    for _, a in ipairs(res and res.result or {}) do titles[#titles + 1] = a.title end
    return { titles = titles, map = vim.fn.maparg("<Space>z", "n", false, true).desc or "" }
  ]])
  T.ok("     Space z offers Harper's fixes (" .. table.concat(vim.list_slice(fix.titles, 1, 3), " / ") .. ")", #fix.titles > 0 and fix.map:find("Harper") ~= nil)

  -- Copilot ----------------------------------------------------------------
  -- skipped when prefs.lua has copilot = false or the server isn't installed
  local cp_ready = T.lua([[local co = require("ide.copilot"); return require("config.prefs").copilot ~= false and co.cmd() ~= nil]])
  if not cp_ready then
    io.write("\n-- Copilot: skipped (prefs.lua copilot = false, or the server isn't installed)\n")
  else
    io.write("\n-- Copilot (not signed in)\n")
    T.open("tex", "‸")
    local cp = T.lua([[
      local co = require("ide.copilot")
      local ok = vim.wait(20000, function() return #vim.lsp.get_clients({ bufnr = 0, name = "copilot" }) > 0 end, 50)
      vim.wait(15000, function() return co.status ~= nil end, 50)
      local hints = 0
      for _, m in ipairs(_G.__msgs) do if m:find("LspCopilotSignIn", 1, true) then hints = hints + 1 end end
      local keys = {}
      for _, k in ipairs({ { "i", "<M-l>" }, { "i", "<M-]>" }, { "i", "<M-[>" }, { "n", "<Space>a" } }) do
        keys[#keys + 1] = vim.fn.maparg(k[2], k[1]) ~= "" and k[2] or nil
      end
      local tab = vim.fn.maparg("<Tab>", "i", false, true)
      return { started = ok, status = co.status, on = co.on, hints = hints, keys = keys, inline = vim.lsp.inline_completion.is_enabled(),
        signin = vim.fn.exists(":LspCopilotSignIn") == 2, tab = tab.desc or tab.rhs or "", cmd = co.cmd()[1] }
    ]])
    T.ok("     server started unauthenticated (status " .. tostring(cp.status) .. "; " .. vim.fs.basename(cp.cmd) .. ")", cp.started and cp.status == "NotSignedIn")
    T.ok("     one quiet hint: “Copilot: run :LspCopilotSignIn” (" .. cp.hints .. "×), :LspCopilotSignIn exists", cp.hints == 1 and cp.signin)
    T.ok("     keys: " .. table.concat(cp.keys, " "), #cp.keys == 4)
    T.ok("     ghost text on at start (prefs.lua copilot_at_start), Tab is not Copilot's (" .. cp.tab .. ")", cp.on and cp.inline and not cp.tab:lower():find("copilot") and not cp.tab:lower():find("inline"))
    -- (Esc and Space within 'ttimeoutlen' would arrive as Alt-Space)
    T.type_keys("<Esc>")
    T.lua([[vim.wait(100)]])
    -- leader keys go in as one input (typed one by one, Space would time out)
    local mark = T.lua([[return #_G.__msgs]])
    T.rpc("nvim_input", " a")
    T.sync()
    local off = T.lua([[
      vim.wait(5000, function() return #vim.lsp.get_clients({ name = "copilot" }) == 0 end, 50)
      local mark = ...
      vim.wait(500)
      return { on = require("ide.copilot").on, clients = #vim.lsp.get_clients({ name = "copilot" }), inline = vim.lsp.inline_completion.is_enabled(), msg = table.concat(vim.list_slice(_G.__msgs, mark + 1), " | ") }
    ]], mark)
    T.ok("     Space a → off, server stopped (" .. tostring(off.msg) .. ")", off.on == false and off.clients == 0 and not off.inline and off.msg == "Copilot off", vim.inspect(off))
    T.rpc("nvim_input", " a")
    T.sync()
    local on = T.lua([[
      vim.wait(20000, function() return #vim.lsp.get_clients({ bufnr = 0, name = "copilot" }) > 0 end, 50)
      local hints = 0
      for _, m in ipairs(_G.__msgs) do if m:find("run :LspCopilotSignIn", 1, true) then hints = hints + 1 end end
      return { on = require("ide.copilot").on, clients = #vim.lsp.get_clients({ bufnr = 0, name = "copilot" }), inline = vim.lsp.inline_completion.is_enabled(), msg = _G.__msgs[#_G.__msgs], hints = hints }
    ]])
    T.lua([[vim.wait(1000)]])
    T.ok("     Space a → on again, server back (" .. tostring(on.msg) .. "), still one hint", on.on and on.clients == 1 and on.inline and on.hints == 1)
    T.case("Tab still expands snippets with Copilot on: enum<Tab>", "tex", "‸", { "enum<Tab>", "\\begin{enumerate}\n\t\\item ‸\n\\end{enumerate}" })
    T.case("Tab still expands \\sum's tabstops with Copilot on", "tex", "$‸$", { "sum", "$\\sum‸$", "<Tab>", { "$\\sum_{i=1}^{N}$", mode = "s", sel = "i" } })
    T.case("Alt-l with no suggestion types nothing", "tex", "abc‸", { "<M-l>", "abc‸" })
  end

  -- finder -----------------------------------------------------------------
  io.write("\n-- finder (snacks.picker)\n")
  T.open("tex", "Hello.‸")
  T.type_keys("<Esc>")
  -- the tests run with -i NONE (no history): give "recent" something to list
  T.lua([[vim.v.oldfiles = { ... }]], dir .. "/undef/main.tex")
  local empty = T.lua([[
    vim.diagnostic.reset()
    require("ide.finder").diagnostics()
    return { msg = _G.__msgs[#_G.__msgs], pickers = #Snacks.picker.get() }
  ]])
  T.ok("     Space x with nothing to list says so (" .. tostring(empty.msg) .. ")", empty.msg == "No errors, warnings or grammar notes" and empty.pickers == 0)
  -- something for Space x to list
  T.lua([[vim.diagnostic.set(vim.api.nvim_create_namespace("ide-test"), 0, { { lnum = 0, col = 0, message = "test note", severity = vim.diagnostic.severity.WARN } })]])
  for _, k in ipairs({ { " f", "files here" }, { " F", "files under ~" }, { " o", "recent" }, { " /", "grep here" }, { " b", "buffers" }, { " x", "diagnostics" } }) do
    T.lua([[vim.v.errmsg = ""]])
    local t0 = vim.uv.hrtime()
    T.rpc("nvim_input", k[1])
    local pk = T.lua([[
      local ok = vim.wait(5000, function() return #Snacks.picker.get() > 0 end, 2)
      return { ok = ok, title = (Snacks.picker.get()[1] or {}).title or "", err = vim.v.errmsg }
    ]])
    local ms = (vim.uv.hrtime() - t0) / 1e6
    T.lua([[for _, x in ipairs(Snacks.picker.get()) do x:close() end]])
    T.sync()
    local closed = T.lua([[
      local function floats()
        local n = 0
        for _, w in ipairs(vim.api.nvim_tabpage_list_wins(0)) do
          if vim.api.nvim_win_get_config(w).relative ~= "" then n = n + 1 end
        end
        return n
      end
      -- the picker closes its windows asynchronously
      vim.wait(3000, function() return #Snacks.picker.get() == 0 and floats() == 0 and vim.api.nvim_get_mode().mode == "n" end, 5)
      return { err = vim.v.errmsg, left = #Snacks.picker.get(), floats = floats(), mode = vim.api.nvim_get_mode().mode }
    ]])
    T.sync()
    T.ok(("     Space %s  %-12s opens in %3.0f ms (“%s”) and closes"):format(vim.trim(k[1]), k[2], ms, pk.title), pk.ok and closed.err == "" and closed.left == 0 and closed.floats == 0, vim.inspect({ closed, pk }))
  end

  -- nothing was shown -------------------------------------------------------
  local blocked = T.lua([[return _G.texnvim_blocked_opens or 0]])
  local skim_after = vim.system({ "pgrep", "-xq", "Skim" }):wait().code == 0
  T.ok("     no viewer / browser was asked for (" .. blocked .. " blocked), Skim state unchanged", blocked == 0 and skim_after == skim_before)
end

-- snippets inside command arguments: \section{…}, \item[…], \caption{…} (2026-10-05)
local function arg_contexts(T)
  io.write("\n-- tex: snippets inside \\section{…} and other arguments\n")
  T.case("\\section{…}: mk", "tex", "\\section{‸}", { "Groups mk", "\\section{Groups $‸$}" })
  T.case("\\section{…}: mk then x2", "tex", "\\section{‸}", { "The group mkx2", "\\section{The group $x_{2}‸$}" })
  T.case("\\section{$…$}: uu", "tex", "\\section{$‸$}", { "a uu", "\\section{$a \\cup‸$}" })
  T.case("\\section{$…$}: //", "tex", "\\section{$‸$}", { "//", "\\section{$\\frac{‸}{}$}" })
  T.case("\\subsection*{$…$}: iff", "tex", "\\subsection*{Order $‸$}", { "a iff", "\\subsection*{Order $a \\iff‸$}" })
  T.case("\\section[short]{$…$}: uu", "tex", "\\section[short]{$‸$}", { "a uu", "\\section[short]{$a \\cup‸$}" })
  T.case("\\item[$…$]: uu", "tex", "\\item[$‸$]", { "a uu", "\\item[$a \\cup‸$]" })
  T.case("\\caption{$…$}: x2", "tex", "\\caption{Plot of $‸$}", { "x2", "\\caption{Plot of $x_{2}‸$}" })
  T.case("\\textbf{…}: mk", "tex", "\\textbf{‸}", { "bold mk", "\\textbf{bold $‸$}" })
  T.case("\\section{…}: plain words stay", "tex", "\\section{‸}", { "Subgroups and cosets", "\\section{Subgroups and cosets‸}" })
  -- maths inside [...] options (VimTeX doesn't parse it; ntex falls back to a $-scan)
  T.case("\\begin{proof}[NTS: $…$]: uu", "tex", "\\begin{proof}[NTS: $‸$]", { "A uu", "\\begin{proof}[NTS: $A \\cup‸$]" })
  T.case("\\begin{proof}[…\\(…\\)]: x2", "tex", "\\begin{proof}[NTS: \\(‸\\)]", { "x2", "\\begin{proof}[NTS: \\(x_{2}‸\\)]" })
  T.case("\\item[$…$]: //", "tex", "\\item[$‸$]", { "//", "\\item[$\\frac{‸}{}$]" })
  T.case("[…] after the closing $: text again", "tex", "\\item[$a$ and ‸]", { "uu", "\\item[$a$ and uu‸]" })
  T.case("[1] citation then text: no maths", "tex", "see [1] ‸", { "uu", "see [1] uu‸" })
  T.case("escaped \\$ inside [ ]: no maths", "tex", "\\item[costs \\$‸]", { "5 uu", "\\item[costs \\$5 uu‸]" })
end

function M.run(suite, T)
  if suite == "mine" then
    mine(T)
    text_envs(T)
    arg_contexts(T)
    macos_keys(T)
    studio(T)
    hot_reload(T)
  elseif suite == "defaults" then
    defaults(T)
  elseif suite == "mangling" then
    mangling(T)
  elseif suite == "english" then
    english(T)
  elseif suite == "decisions" then
    decisions(T)
  elseif suite == "perf" then
    perf(T)
  elseif suite == "system" then
    health(T)
    system(T)
  elseif suite == "ide" then
    ide(T)
  elseif suite == "figures" then
    figures(T)
  end
end

return M
