-- Plugin list. Exact versions are pinned in lazy-lock.json (committed).
return {
  -- Colours: Modus (AAA contrast, light/dark follows the terminal). lua/config/theme.lua
  {
    "miikanissi/modus-themes.nvim",
    lazy = false,
    priority = 1000,
    config = function()
      require("config.theme").setup()
    end,
  },

  -- VimTeX: compile (tectonic), view (Skim), syntax, conceal, math zones
  {
    "lervag/vimtex",
    lazy = false, -- VimTeX must not be lazy-loaded
    init = function()
      vim.g.vimtex_compiler_method = "tectonic"
      vim.g.vimtex_compiler_tectonic = {
        out_dir = "",
        hooks = {},
        -- -Z search-path: documents find the notes theme (\usepackage{ntex-notes})
        -- in templates/tex/ wherever the .tex file lives
        options = {
          "--keep-logs", "--synctex",
          "-Z", "search-path=" .. vim.fn.stdpath("config") .. "/templates/tex",
        },
      }
      vim.g.vimtex_view_method = "skim"
      -- TEXNVIM_NO_VIEWER=1 (tests): VimTeX's viewer is off entirely (\lv, \ls,
      -- the reload after a compile), so nothing ever talks to Skim.
      if vim.env.TEXNVIM_NO_VIEWER == "1" then
        vim.g.vimtex_view_enabled = 0
      end
      -- The viewer only opens on an explicit \lv (:VimtexView), never after a compile.
      vim.g.vimtex_view_automatic = 0
      vim.g.vimtex_view_skim_sync = 0
      vim.g.vimtex_view_skim_activate = 1
      vim.g.vimtex_view_skim_reading_bar = 0
      -- The snippets own insert mode; VimTeX's `-style imaps would clash.
      vim.g.vimtex_imaps_enabled = 0
      -- With live PDF (prefs.lua) the file compiles on every pause, often
      -- mid-equation, so the error list stays closed; \le shows it.
      -- Otherwise it opens after a failed compile without stealing the cursor.
      local live = require("config.prefs").live_pdf
      vim.g.vimtex_quickfix_mode = live and 0 or 2
      vim.g.vimtex_compiler_silent = live and 1 or 0
      vim.g.vimtex_quickfix_open_on_warning = 0
      vim.g.vimtex_complete_enabled = 0 -- completion comes from texlab via blink.cmp
      vim.g.vimtex_syntax_enabled = 1
      -- Conceal starts off here; lua/snips/conceal.lua sets it from prefs.lua
      -- at startup and rewrites it when <F2> toggles.
      vim.g.vimtex_syntax_conceal = require("snips.conceal").tex_option(false)
      vim.g.vimtex_mappings_enabled = 1
      -- \lv is ours (lua/config/pdfview.lua: cmux pane or Skim); \ls = Skim
      vim.g.vimtex_mappings_disable = { n = { "<localleader>lv" } }
      vim.g.vimtex_log_ignore = { "latexmk is not executable" }
    end,
    config = function()
      -- After every successful compile VimTeX asks Skim (via osascript) to
      -- reload the PDF. That Apple Event *launches* Skim if it isn't running,
      -- so a save would pop Skim up. Only do the reload when Skim is already
      -- open; otherwise Skim opens only on \lv / :VimtexView.
      local grp = vim.api.nvim_create_augroup("texnvim_vimtex_viewer", { clear = true })
      vim.api.nvim_create_autocmd("User", {
        group = grp,
        pattern = "VimtexEventInitPost",
        callback = function()
          pcall(vim.api.nvim_clear_autocmds, { group = "vimtex_viewer", event = "User", pattern = "VimtexEventCompileSuccess" })
        end,
      })
      vim.api.nvim_create_autocmd("User", {
        group = grp,
        pattern = "VimtexEventCompileSuccess",
        callback = function()
          if vim.env.TEXNVIM_NO_VIEWER == "1" then
            return
          end
          vim.system({ "pgrep", "-xq", "Skim" }, {}, function(res)
            if res.code == 0 then
              vim.schedule(function()
                pcall(vim.fn["vimtex#view#compiler_callback"])
              end)
            end
          end)
        end,
      })

      -- a save that arrived while compiling: compile once more when it's done
      vim.api.nvim_create_autocmd("User", {
        group = grp,
        pattern = { "VimtexEventCompileSuccess", "VimtexEventCompileFailed" },
        callback = function(ev)
          local buf = vim.api.nvim_get_current_buf()
          if vim.b[buf].texnvim_recompile then
            vim.b[buf].texnvim_recompile = false
            vim.schedule(function()
              pcall(vim.cmd, "silent VimtexCompile")
            end)
          end
        end,
      })

      -- compile on save (single shot; tectonic has no continuous mode here)
      vim.api.nvim_create_autocmd("BufWritePost", {
        group = vim.api.nvim_create_augroup("texnvim_compile_on_save", { clear = true }),
        pattern = "*.tex",
        callback = function(ev)
          if vim.env.TEXNVIM_NO_COMPILE == "1" then
            return
          end
          -- only compile a file that really made it to disk (a failed write
          -- still fires BufWritePost); never let a compile error bury the save
          if vim.fn.filereadable(ev.match) == 0 then
            return
          end
          if vim.b[ev.buf].vimtex and vim.fn.exists(":VimtexCompile") == 2 then
            -- :VimtexCompile *stops* a running compile, so if one is running,
            -- queue another for when it finishes (see VimtexEventCompile* below)
            local running = vim.api.nvim_buf_call(ev.buf, function()
              local ok, r = pcall(vim.fn.eval, "b:vimtex.compiler.is_running()")
              return ok and r == 1
            end)
            if running then
              vim.b[ev.buf].texnvim_recompile = true
              return
            end
            local ok, err = pcall(vim.cmd, "silent VimtexCompile")
            if not ok then
              vim.notify("texnvim: compile didn't start: " .. tostring(err), vim.log.levels.WARN)
            end
          end
        end,
      })
    end,
  },

  -- LuaSnip: the snippet engine. Set up by lua/snips (snippets in snippets/tex/*.lua;
  -- triggers are plain text or Lua patterns, so jsregexp isn't needed).
  {
    "L3MON4D3/LuaSnip",
    lazy = false,
  },

  -- Completion: texlab (\ref, \cite, macros), LuaSnip, paths, buffer words.
  {
    "saghen/blink.cmp",
    version = "1.*", -- v2 (main) needs blink.lib; stay on the stable 1.x line
    lazy = false,
    dependencies = { "L3MON4D3/LuaSnip" },
    opts = {
      -- Keys are deliberate: Tab/Shift-Tab belong to snippets & tabout and Enter
      -- to matrix rows (lua/snips/features.lua). Enter accepts a completion
      -- only after you picked one with ↓/↑ or Ctrl-n/Ctrl-p.
      keymap = {
        preset = "none",
        ["<C-space>"] = { "show", "show_documentation", "hide_documentation" },
        ["<C-e>"] = { "hide", "fallback" },
        ["<C-y>"] = { "select_and_accept", "fallback" },
        ["<Down>"] = { "select_next", "fallback" },
        ["<Up>"] = { "select_prev", "fallback" },
        ["<C-n>"] = { "select_next", "fallback" },
        ["<C-p>"] = { "select_prev", "fallback" },
        ["<C-b>"] = { "scroll_documentation_up", "fallback" },
        ["<C-f>"] = { "scroll_documentation_down", "fallback" },
      },
      snippets = { preset = "luasnip" },
      fuzzy = { implementation = "lua" }, -- no prebuilt binary to download
      completion = {
        list = { selection = { preselect = false, auto_insert = false } },
        documentation = { auto_show = true, auto_show_delay_ms = 300 },
        menu = {
          -- Pop up by itself only where it helps: after a backslash, inside
          -- \ref{ \cite{ \label{ \begin{ …, or a path. Ctrl-Space shows it anywhere.
          auto_show = function(ctx)
            local before = ctx.line:sub(1, ctx.cursor[2])
            if before:match("\\%a*$") then
              return true
            end
            if before:match("\\%a*{[^}]*$") then
              return true
            end
            if before:match("%.?%.?/[%w_%-%.]*$") then
              return true
            end
            return false
          end,
        },
      },
      sources = {
        default = { "lsp", "snippets", "path", "buffer" },
      },
      signature = { enabled = false },
      cmdline = { enabled = true },
    },
  },

  -- Treesitter parsers: latex (snacks math detection) + markdown/markdown_inline.
  -- Highlighting is enabled for markdown only; tex uses VimTeX's syntax.
  {
    "nvim-treesitter/nvim-treesitter",
    branch = "main",
    lazy = false,
    build = ":TSUpdate",
    config = function()
      local want = { "latex", "markdown", "markdown_inline" }
      local have = require("nvim-treesitter").get_installed("parsers")
      local missing = vim.tbl_filter(function(p)
        return not vim.tbl_contains(have, p)
      end, want)
      if #missing > 0 then
        require("nvim-treesitter").install(missing)
      end
      vim.api.nvim_create_autocmd("FileType", {
        group = vim.api.nvim_create_augroup("texnvim_treesitter", { clear = true }),
        pattern = { "markdown" },
        callback = function(ev)
          pcall(vim.treesitter.start, ev.buf, "markdown")
        end,
      })
    end,
  },

  -- nvim-lspconfig: only its lsp/copilot.lua is used (the Copilot server
  -- definition with :LspCopilotSignIn / :LspCopilotSignOut), read by
  -- lua/ide/copilot.lua. lazy = true keeps its ~300 server configs off the
  -- runtimepath, so texlab and harper-ls use exactly the settings in lua/.
  {
    "neovim/nvim-lspconfig",
    lazy = true,
  },

  -- snacks.nvim: image (renders LaTeX math inline, kitty graphics) and picker
  -- (the file finder, Space f / F / o / / / b / x; lua/ide/finder.lua).
  {
    "folke/snacks.nvim",
    lazy = false,
    priority = 1000,
    opts = function()
      -- default from prefs.lua (images = false); TEXNVIM_IMAGES=1/0 overrides
      local images = require("config.prefs").images == true
      if vim.env.TEXNVIM_IMAGES == "1" then
        images = true
      elseif vim.env.TEXNVIM_IMAGES == "0" then
        images = false
      end
      -- kitty graphics do not pass through tmux inside cmux
      if vim.env.TMUX and vim.env.TEXNVIM_IMAGES ~= "1" then
        images = false
      end
      return {
        -- the finder: small, no icons, Modus colours (theme.lua); vim.ui.select
        -- (Space z suggestions, Space e) uses the same small list
        picker = require("ide.finder").picker_opts(),
        image = {
          enabled = images,
          doc = {
            enabled = true,
            inline = true, -- needs unicode placeholders (Ghostty/cmux have them)
            float = true, -- fallback: show the rendered math next to the cursor
            max_width = 80,
            max_height = 20,
          },
          math = {
            enabled = true,
            latex = {
              font_size = "large",
              packages = { "amsmath", "amssymb", "amsfonts", "amscd", "mathtools" },
            },
          },
        },
      }
    end,
  },
}
