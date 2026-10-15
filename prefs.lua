-- Your ntex preferences. Edit, save, restart ntex.
-- (Snippets live in ~/.config/texnvim/snippets/ — see snippets/README.md.)
return {
  -- Rendered math images in the text (snacks.nvim). Off: too distracting.
  -- One-off override from the shell: TEXNVIM_IMAGES=1 ntex file.tex
  images = false,

  -- Pretty symbols (\alpha → α, \mathbb{R} → ℝ) when ntex starts; the cursor
  -- line always shows the source. F2 toggles.
  conceal = true,

  -- Live PDF: when you pause typing in a .tex file, save it and recompile, so
  -- Skim (with "Check for file changes" on) updates as you work. A compile
  -- that fails mid-equation leaves the last good PDF in place.
  live_pdf = true,
  live_pdf_delay_ms = 400, -- pause length before it saves + compiles

  -- Where \lv shows the PDF (\ls always opens Skim, with SyncTeX):
  --   "tdf"     split pane in cmux running tdf; follows every compile, keeps your page
  --   "browser" cmux browser split; sharp + selectable text; reloads after each compile
  --   "skim"    Skim.app
  -- Outside cmux it's always Skim.
  pdf_viewer = "tdf",

  -- Conceal sub/superscripts too (x_{i} → xᵢ). Those glyphs are small in most
  -- fonts; set false to keep _{…}/^{…} as typed while still concealing symbols.
  conceal_scripts = true,

  -- IDE features (README "IDE features") ------------------------------------
  -- Compile errors as underlines + a note at the end of the line, on the
  -- right line (\input files too), after every compile. Space d: in full.
  compile_errors = true,
  -- Overfull / underfull \hbox warnings: false hides them; true shows them as
  -- faint hints (underline only).
  box_warnings = false,

  -- Harper: grammar and spelling in .tex and .md (brew install harper). Runs
  -- on your Mac only. Replaces Vim's spell check where it runs. Space z fixes.
  harper = true,
  harper_dialect = "American", -- "American", "British", "Australian", "Canadian", "Indian"

  -- GitHub Copilot ghost text (Alt-l accepts; Tab never does). Needs a one-time
  -- :LspCopilotSignIn. While on, the file you edit is sent to GitHub.
  copilot = false, -- true: start it (needs :LspCopilotSignIn; sends the file you edit to GitHub)
  copilot_at_start = true, -- on when ntex starts; Space a switches on / off

  -- Maths typing features (these were the Latex Suite settings; values are
  -- the ones your Obsidian import had on 2026-09-30).
  latex = {
    snippets = true, -- false switches every snippet off

    -- Letter triggers (uu, xx, sub, …) fire only at the start of a word, so
    -- they never fire inside \subset, \approx, … true also lets them fire
    -- right after a letter (Auu → A\cup), but still never inside a \command.
    fire_after_letters = false,

    -- In inline maths, drop the trailing space a snippet would type.
    remove_snippet_whitespace = true,

    -- Backspace in an empty $|$ deletes both dollars.
    auto_delete_dollar = true,

    -- Typing / after something in maths makes it a fraction: a/ → \frac{a}{…}
    autofraction = true,
    autofraction_symbol = "\\frac",
    autofraction_breaking_chars = "+-=\t", -- where the numerator starts (plus space $ ( [ {)
    autofraction_excluded = { { "^{", "}" }, { "\\pu{", "}" } }, -- / stays a slash inside these

    -- Tab / Enter / Shift-Enter in matrices: " & ", " \\" + new row, leave.
    matrix_shortcuts = true,
    matrix_envs = { "pmatrix", "cases", "align", "gather", "bmatrix", "Bmatrix", "vmatrix", "Vmatrix", "array", "matrix" },
    -- in .tex files also these:
    matrix_envs_tex = {
      "align*", "gather*", "aligned", "alignedat", "gathered", "split", "alignat", "alignat*",
      "flalign", "flalign*", "eqnarray", "eqnarray*", "pmatrix*", "bmatrix*", "smallmatrix", "dcases",
    },
    matrix_macros = { "eqalign" },

    -- Tab steps past the next closing bracket, then out of the maths.
    tabout = true,
    tabout_exit_only_at_eol = true, -- leave the maths only when nothing but spaces follows
    tabout_closing = { ")", "]", "\\rbrack", "\\}", "\\rbrace", "\\rangle", "\\rvert", "\\rVert", "\\rfloor", "\\rceil", "\\urcorner", "}" },

    -- Brackets around these become \left( … \right) once a snippet types them.
    enlarge_brackets = true,
    enlarge_space = true, -- "\left( " and " \right)" with a space
    enlarge_triggers = { "sum", "int", "frac", "prod", "bigcup", "bigcap" },

    -- Fenced code blocks in .md files that count as display maths.
    force_math_languages = { "math" },
  },

  -- Ntex Studio (Space n): making snippets and blocks on the fly.
  -- studio/README.md has the keys. Read when the studio starts.
  studio = {
    hide_after_save = true, -- ⌘↵ saves and hides the studio, back to ntex
    typeface = "Computer Modern", -- or "Palatino"
    render_preview = true, -- render the output with tectonic as you type
    -- "Sort into my files" asks this model where each inbox snippet belongs
    sort_model = "claude-haiku-5-5",
    sort_effort = "high",
    -- Figures: the Ask box rewrites the .fig from your instruction.
    -- "claude-…" models run on your Claude plan, "gpt-…" ones on your ChatGPT
    -- plan (through codex). Chosen by studio/figures/eval/REPORT.md.
    figure_model = "claude-sonnet-5-5", -- the normal one: right 100% of the time in the eval, ~4.5 s
    figure_effort = "low", -- "low" | "medium" | "high"
    figure_fast_model = "gpt-6-luna", -- ⚡ fast: ~4 s, right ~95% of the time
    figure_fast_effort = "low",
    figure_fast = false, -- start with ⚡ fast on?
  },
}
