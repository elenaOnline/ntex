// Words for the Settings & files, Commands and Troubleshooting sections.
// Values (what is set now, which file exists) are filled in by the build.

// Plain-English meaning of each `latex = { … }` key in prefs.lua (these
// were the Latex Suite settings; same values as your Obsidian import had).
export const LATEX_MEANING = {
  snippets: "Snippets on at all.",
  fire_after_letters: "true: letter triggers (uu, xx, sub …) also fire right after a letter (Auu → A\\cup), as in Latex Suite; never inside a \\command either way.",
  remove_snippet_whitespace: "In inline maths, drop the trailing space a snippet would type.",
  auto_delete_dollar: "Backspace in an empty $|$ deletes both dollars.",
  autofraction: "Typing / in maths makes a fraction of what is before it.",
  autofraction_symbol: "The command auto-fraction types.",
  autofraction_breaking_chars: "Where the numerator starts when looking back from / (space, $, (, [, { always count).",
  autofraction_excluded: "Inside these pairs / stays a slash.",
  matrix_shortcuts: "Tab / Enter / Shift-Enter act on matrix rows and columns.",
  matrix_envs: "Environments where the matrix keys work.",
  matrix_envs_tex: "In .tex files the matrix keys also work in these.",
  matrix_macros: "Commands whose argument counts as a matrix.",
  tabout: "Tab steps past the next closing bracket, then out of the maths.",
  tabout_exit_only_at_eol: "Tab leaves the maths only when nothing but spaces follows.",
  tabout_closing: "What counts as a closing bracket for Tab.",
  enlarge_brackets: "Brackets around \\sum, \\frac, … become \\left( \\right) once a snippet types one of these.",
  enlarge_space: "A space after \\left( and before \\right).",
  enlarge_triggers: "Commands that make the brackets around them grow.",
  force_math_languages: "Fenced code blocks (```math) in .md files that count as display maths.",
};

// Meanings for prefs.lua keys when the file has no comment for one.
export const PREF_MEANING = {
  images: "Rendered maths images in the text (snacks.nvim).",
  conceal: "Pretty symbols when ntex starts: true / false. F2 toggles.",
  latex: "Maths typing features (the old Latex Suite settings): see the table below.",
  live_pdf: "Save and compile when you pause typing.",
  live_pdf_delay_ms: "How long a pause, in milliseconds.",
  pdf_viewer: "Where \\lv shows the PDF: \"tdf\", \"browser\" or \"skim\".",
  conceal_scripts: "Also conceal sub/superscripts (x_{i} → xᵢ).",
  compile_errors: "Compile errors as underlines + a note at the end of the line, after every compile.",
  box_warnings: "Overfull / underfull \\hbox warnings: false hides them, true shows them as faint hints.",
  harper: "Harper grammar and spelling in .tex and .md (replaces Vim's spell check there).",
  harper_dialect: "Harper's English: \"American\", \"British\", \"Australian\", \"Canadian\", \"Indian\".",
  copilot: "GitHub Copilot ghost text at all (false: never started).",
  copilot_at_start: "Copilot on when ntex starts; Space a switches it.",
};

// Commands that aren't in the live list (built-in or lazy-loaded), and the
// wording for the ones that are. key → [what, keys, args]
export const COMMAND_INFO = {
  Reference: ["open this page, rebuilding it first if a source changed (in cmux: a browser split to the right, cursor stays in Neovim)", "Space ?"],
  Scratch: ["new timestamped scratch .tex in ~/scratch, or in the folder you give", "Space s", "[dir]"],
  SnippetReport: ["the loaded snippets, file by file, and any load error", "Space r"],
  SnippetEdit: ["pick a snippet file (or README / DECISIONS) and open it", "Space e"],
  SnippetReload: ["reload every snippet file now (saving one already reloads it)"],
  SnippetStudio: ["open Ntex Studio on a page: snippet (default), block or inbox. Build it once with ~/.config/texnvim/studio/build.sh", "Space n", "[page]"],
  Figure: ["Ntex Figures: the same as Space g; with a name, make (or open) figures/NAME.fig and include it at the cursor", "Space g", "[name]"],
  Conceal: ["pretty symbols on / off", "F2"],
  LatexSuiteReport: ["old name of :SnippetReport"],
  LatexSuiteEdit: ["old name of :SnippetEdit"],
  LatexSuiteReload: ["old name of :SnippetReload"],
  LatexSuiteConceal: ["old name of :Conceal"],
  Lazy: ["plugin manager window; `:Lazy restore` puts every plugin back to lazy-lock.json"],
  checkhealth: ["self-check, e.g. `:checkhealth snacks luasnip`. `:checkhealth vimtex` asks macOS about Skim, which can show a permission prompt", "", "[plugin]"],
  LspCopilotSignIn: ["sign in to GitHub Copilot once: copies a one-time code, offers to open github.com/login/device; paste the code there and approve. Run it in a .tex / .md file"],
  LspCopilotSignOut: ["sign Copilot out of GitHub (on this Mac)"],
  LuaSnipUnlinkCurrent: ["forget the current snippet, if Tab keeps jumping back into an old one"],
  VimtexCompile: ["compile now (tectonic)", "\\ll"],
  VimtexView: ["open the PDF in Skim at the cursor", "\\ls"],
  VimtexErrors: ["show the compile errors", "\\le"],
  VimtexClean: ["delete build files (`:VimtexClean!` also the PDF)", "\\lc"],
  VimtexTocOpen: ["table of contents", "\\lt"],
  VimtexTocToggle: ["table of contents on / off", "\\lT"],
  VimtexStop: ["stop compiling", "\\lk"],
  VimtexStopAll: ["stop all compilers", "\\lK"],
  VimtexStatus: ["compiler status", "\\lg"],
  VimtexInfo: ["information about the document", "\\li"],
  VimtexLog: ["VimTeX's message log", "\\lq"],
  VimtexReload: ["reload VimTeX", "\\lx"],
  VimtexReloadState: ["reload VimTeX's state for this document", "\\lX"],
  VimtexCompileOutput: ["the compiler's raw output", "\\lo"],
  VimtexCompileSS: ["compile once", "\\lS"],
  VimtexCompileSelected: ["compile only the selected lines", "\\lL"],
  VimtexCountWords: ["count the words in the document"],
  VimtexCountLetters: ["count the letters in the document"],
  VimtexDocPackage: ["documentation for the package under the cursor", "K"],
  VimtexContextMenu: ["actions for the command under the cursor", "\\la"],
  VimtexClearCache: ["clear VimTeX's caches (e.g. after moving files)"],
  VimtexToggleMain: ["treat this file as the main document, or not"],
  VimtexInverseSearch: ["used by Skim to jump from the PDF to the source (⌘⇧-click)"],
};

// Files that matter. {path, what, edit, applies}; `check` = path to test for existence
export function filesInfo({ keys, code, cfg, snippetFiles }) {
  return [
    { id: "f-snippets", path: `${cfg}/snippets/tex/*.lua`, what: `your snippets, one file per topic, one snippet per line (${snippetFiles.map((f) => f.replace(/\.lua$/, "")).join(", ")})`, edit: `${keys("Space e")} picks a file; save`, applies: "live", check: `${cfg}/snippets/tex` },
    { id: "f-snippets-readme", path: `${cfg}/snippets/README.md`, what: "how a snippet line reads, how to add one, the whole vocabulary", edit: "read it", applies: "—", check: `${cfg}/snippets/README.md` },
    { id: "f-decisions", path: `${cfg}/snippets/DECISIONS.md`, what: "every decision of the 2026-09-30 migration (also the Migration decisions section here); decisions.json next to it is what this page reads", edit: "read it; tools/migration/ rebuilds it", applies: "rebuild", check: `${cfg}/snippets/DECISIONS.md` },
    { id: "f-prefs", path: `${cfg}/prefs.lua`, what: `your preferences: images, conceal, live PDF, PDF viewer, and ${code("latex = { … }")} (auto-fraction, tabout, matrix keys, …)`, edit: "open, change a value, save", applies: "restart ntex" },
    { id: "f-helpers", path: `${cfg}/lua/snips/helpers.lua`, what: "the snippet vocabulary (math, post, pat, env, vis, …): what a line in a snippet file turns into", edit: "rarely", applies: "restart ntex" },
    { id: "f-cond", path: `${cfg}/lua/snips/cond.lua`, what: "when a snippet may fire: word starts, \\commands, maths / text", edit: "rarely", applies: "restart ntex" },
    { id: "f-features", path: `${cfg}/lua/snips/features.lua`, what: "Tab, Enter, auto-fraction, tabout, matrix keys, enlarging brackets", edit: "rarely; the settings are in prefs.lua", applies: "restart ntex" },
    { id: "f-options", path: `${cfg}/lua/config/options.lua`, what: "Vim options: line numbers, relative numbers, wrap, spell languages, clipboard, mouse", edit: "open, change, save", applies: "restart ntex" },
    { id: "f-keymaps", path: `${cfg}/lua/config/keymaps.lua`, what: "Ctrl-s, Ctrl-z, the Space shortcuts", edit: "open, change, save", applies: "restart ntex" },
    { id: "f-ide", path: `${cfg}/lua/ide/`, what: "IDE features: diagnostics.lua (how errors look), compile_errors.lua (tectonic log → errors), harper.lua, copilot.lua, finder.lua; loaded by lua/config/ide.lua", edit: "the switches are in prefs.lua", applies: "restart ntex" },
    { id: "f-copilot", path: "~/.local/share/texnvim/copilot/", what: "GitHub's Copilot language server (npm, local to ntex)", edit: `update: ${code("npm install --prefix ~/.local/share/texnvim/copilot @github/copilot-language-server@latest")}`, applies: "restart ntex" },
    { id: "f-theme", path: `${cfg}/lua/config/theme.lua`, what: "colours (Modus) and the conceal colour", edit: "on_highlights in that file", applies: "restart ntex" },
    { id: "f-macos", path: `${cfg}/lua/config/macos_keys.lua`, what: "Option-Left / Right / Backspace with macOS word rules", edit: "rarely", applies: "restart ntex" },
    { id: "f-pdfview", path: `${cfg}/lua/config/pdfview.lua`, what: `${code("\\lv")} / ${code("\\ls")}: PDF in a cmux pane or Skim`, edit: "rarely; the viewer choice is in prefs.lua", applies: "restart ntex" },
    { id: "f-plugins", path: `${cfg}/lua/plugins/core.lua`, what: "plugins and their settings: VimTeX, LuaSnip, completion keys, treesitter, snacks", edit: "open, change, save", applies: "restart ntex" },
    { id: "f-figures", path: `${cfg}/lua/config/figures.lua`, what: `Ntex Figures in ntex: ${keys("Space g")}, .fig files (syntax/ntexfig.vim; saving runs ntex-fig build, its messages become errors on their lines), a figure's PDF changing recompiles the .tex. The shorthand: studio/figures/FIGURES.md`, edit: "rarely", applies: "restart ntex" },
    { id: "f-template", path: `${cfg}/templates/article.tex`, what: "what a new .tex file starts with", edit: "open, change, save", applies: "next new file" },
    { id: "f-lock", path: `${cfg}/lazy-lock.json`, what: "exact plugin versions", edit: `${keys([":Lazy restore", "Enter"])} returns to it; ${keys([":Lazy update", "Enter"])} rewrites it`, applies: "restart ntex" },
    { id: "f-ghostty", path: "~/.config/ghostty/config", what: "cmux terminal: font (JuliaMono), keybinds", edit: "open, change, save", applies: "cmux reload-config" },
    { id: "f-launcher", path: "~/.local/bin/ntex", what: "the ntex command (NVIM_APPNAME=texnvim)", edit: "rarely", applies: "next ntex" },
    { id: "f-tests", path: `${cfg}/tests/`, what: "run.sh, cases.lua, the headless driver", edit: "add cases to cases.lua", applies: "when run" },
    { id: "f-reference", path: `${cfg}/reference/`, what: "this page: build.mjs, content/*.mjs (edit the words there), verify.lua", edit: `content/*.mjs, then ${keys([":Reference", "Enter"])}`, applies: "rebuild" },
    { id: "f-latex-suite", path: "~/.config/latex-suite/", what: "Obsidian Latex Suite snippets and settings: ntex no longer reads them (Superset's trial still does)", edit: "leave as is", applies: "not ntex" },
  ];
}

export function troubleInfo({ keys, code }) {
  return [
    {
      id: "t-e212", title: "“E212: Can't open file for writing” when saving", k: "save error folder directory",
      body: `<p>This happened when the folder didn't exist yet (e.g. ${code("ntex ~/new/dir/x.tex")}). <strong>Fixed:</strong> ntex now creates missing folders when you save (${code("lua/config/commands.lua")}). If you still see it, the folder isn't writable, or the path has a typo: ${keys([":saveas ~/somewhere/else.tex", "Enter"])}.</p>`,
    },
    {
      id: "t-pdf", title: "The PDF doesn't update", k: "skim reload compile live",
      body: `<ul>
<li><strong>Skim</strong>: Skim → Settings… (⌘,) → <em>Sync</em> → tick <em>Check for file changes</em>.</li>
<li><strong>Did it compile?</strong> With live PDF a failed compile keeps the last good PDF and the error list stays closed: ${keys("\\le")} shows it, ${keys("\\ll")} compiles now.</li>
<li>The tdf pane reloads itself; a browser pane reloads after each successful compile; ${keys("\\lv")} again re-opens a closed pane.</li>
<li>Live PDF needs <code>live_pdf = true</code> in prefs.lua; ${keys("Ctrl-s")} always saves and compiles.</li>
</ul>`,
    },
    {
      id: "t-errors", title: "Compile errors don't show in the text", k: "errors diagnostics underline compile tectonic log missing",
      body: `<ul>
<li><strong>Typing?</strong> An error on the line you are typing in waits until you press ${keys("Esc")} (live PDF compiles mid-formula). Errors elsewhere show at once; grammar and texlab notes update when you leave insert mode.</li>
<li><strong>Did it compile?</strong> Errors come after a compile: pause typing, ${keys("Ctrl-s")} or ${keys("\\ll")}. ${keys("Space x")} lists every error; ${keys("Space d")} shows this line's in full.</li>
<li><strong>Is it on?</strong> <code>compile_errors = true</code> in prefs.lua. Overfull / underfull boxes are hidden unless <code>box_warnings = true</code>.</li>
<li><strong>A fragment, not a document?</strong> A file without <code>\\documentclass</code> (an <code>\\input</code> chapter) compiles through its main file; open the main file once so VimTeX finds it.</li>
<li>TeX stops at the first error, so only one shows at a time; fix it and the next appears.</li>
<li>Still nothing: ${keys([":checkhealth vim.lsp", "Enter"])} for texlab / Harper; ${keys("\\lo")} shows tectonic's raw output.</li>
</ul>`,
    },
    {
      id: "t-harper", title: "Harper flags something that is fine", k: "harper grammar spelling false positive dictionary ignore",
      body: `<p>Put the cursor on it and press ${keys("Space z")}: <em>Add to dictionary</em> (names like Borel, Lipschitz) or <em>Ignore</em>. Notes inside maths are dropped by ntex (Harper reads <code>\\[…\\]</code> and <code>\\(…\\)</code> as prose). Known left-overs: proper names (until you add them), and rules that see a sentence with its maths removed. A whole rule can be switched off in <code>lua/ide/harper.lua</code> (<code>linters = { RuleName = false }</code>; the rule name is in the ${keys("Space d")} float). <code>harper = false</code> in prefs.lua goes back to Vim's spell check; <code>harper_dialect</code> picks the English.</p>`,
    },
    {
      id: "t-copilot", title: "Copilot shows nothing / says to sign in", k: "copilot sign in ghost text ai github",
      body: `<ol>
<li>Open a .tex or .md file, then ${keys([":LspCopilotSignIn", "Enter"])}. It copies a one-time code and asks to open the browser: answer ${keys("y")}, paste the code on github.com/login/device, approve. “Signed in as …” appears in ntex.</li>
<li>Copilot needs a Copilot plan on your GitHub account: Copilot Free (a monthly allowance of completions) or, free with GitHub Education, the Student plan (unlimited). github.com/settings/copilot shows which you have.</li>
<li>Still nothing: ${keys("Space a")} says whether it is on; suggestions appear after a short pause in insert mode, grey and italic; ${keys("Alt-l")} accepts. ${keys([":checkhealth vim.lsp", "Enter"])} lists the running servers.</li>
<li>While on, the text around the cursor goes to GitHub. ${keys("Space a")} turns it off for the session (the server stops); <code>copilot_at_start = false</code> starts ntex with it off; ${keys([":LspCopilotSignOut", "Enter"])} signs out.</li>
</ol>`,
    },
    {
      id: "t-cmux-auth", title: "<code>cmux reload-config</code> asks for a password", k: "cmux socket auth reload config",
      body: `<p>The cmux command talks to the running app through its socket. Inside a cmux terminal it is trusted; outside (Terminal.app, an ssh session, a script) it needs the socket password (<code>--password</code>, <code>CMUX_SOCKET_PASSWORD</code>, or the one saved in cmux's Settings). Simplest: run it from a cmux terminal, or restart cmux.</p>`,
    },
    {
      id: "t-option", title: "Option-Left / Option-Right don't jump words", k: "alt meta option arrow esc ghostty",
      body: `<p>cmux (Ghostty) sends Option-Left / Option-Right as <code>Esc b</code> / <code>Esc f</code> (its built-in <code>alt+arrow_left=esc:b</code> keybinds); ntex maps both those and the <code>&lt;M-Left&gt;</code> forms, and Option-Backspace. If they stop working, look in <code>~/.config/ghostty/config</code> for a <code>keybind = alt+arrow_…</code> line that overrides the defaults, then <code>cmux reload-config</code>. On the <kbd class="key">:</kbd> line they fall back to Vim's own word keys.</p>`,
    },
    {
      id: "t-images", title: "Turn rendered maths images on or off", k: "snacks images render",
      body: `<p>${code("images = true")} (or <code>false</code>) in prefs.lua, then restart; once only: ${code("TEXNVIM_IMAGES=1 ntex file.tex")}. Needs tectonic, ImageMagick (<code>magick</code>) and Ghostscript (<code>gs</code>). Images never show inside tmux. The line you are on always shows the source.</p>`,
    },
    {
      id: "t-conceal", title: "Concealed symbols: colours, or turning them off", k: "conceal colour pretty symbols f2",
      body: `<p>${keys("F2")} switches conceal for this session. The start state is <code>conceal</code> in prefs.lua (<code>nil</code> = follow the shared <code>concealEnabled</code>); <code>conceal_scripts = false</code> keeps <code>_{i}</code> as typed. Colours: <code>on_highlights</code> in <code>lua/config/theme.lua</code>; concealed symbols use the maths blue so they don't look faint.</p>`,
    },
    {
      id: "t-snippet", title: "A snippet doesn't fire", k: "snippet not expanding trigger context math",
      body: `<ol>
<li><strong>Are you in maths?</strong> Most snippets are maths-only. In .tex that means inside <code>$…$</code>, <code>\\(…\\)</code>, <code>\\[…\\]</code> or a maths environment, and not inside <code>\\text{…}</code>. Text-only ones (<code>mk</code>, <code>dm</code>) are the opposite.</li>
<li><strong>Does it need Tab?</strong> See the How column in <a href="#snippets">Your snippets</a>.</li>
<li><strong>Is another snippet winning?</strong> A longer or higher-priority trigger that ends the same way fires first; the Notes column says so, and rows marked <span class="tag flag">check</span> didn't expand as their text suggests.</li>
<li><strong>Right after a letter?</strong> Triggers that start with a letter only fire at the start of a word: ${code("A uu")} gives ${code("A \\cup")}, ${code("Auu")} stays. (So ${code("\\subset")}, ${code("\\approx")} … type as written.) To change that: ${code("fire_after_letters = true")} in prefs.lua, in ${code("latex = { … }")}.</li>
<li><strong>Inside ${code("\\ce{}")}, ${code("\\label{}")}, a comment?</strong> Nothing fires there on purpose.</li>
<li><strong>Did it load?</strong> ${keys("Space r")} (:SnippetReport) lists every snippet by file. A snippet file with an error keeps its previous snippets and the error is shown there.</li>
</ol>`,
    },
    {
      id: "t-superset", title: "Superset and Obsidian: the old snippets folder", k: "superset latex-suite obsidian snippets.js trial",
      body: `<p>Since 2026-09-30 ntex has its own snippets (${code("~/.config/texnvim/snippets/tex/*.lua")}) and no longer reads <code>~/.config/latex-suite/</code>. That folder is untouched: the Superset trial branch still reads it. So a snippet you add in ntex is not in Superset (and the other way round). The Superset branch is a trial: <strong>ask before merging or releasing it.</strong></p>`,
    },
    {
      id: "t-rollback", title: "Going back to the Latex Suite setup", k: "rollback undo migration pre-luasnip git",
      body: `<p>Everything before the move to LuaSnip is tagged in the config's git repo: <code>git -C ~/.config/texnvim checkout pre-luasnip</code>, then restart ntex (it reads <code>~/.config/latex-suite/</code> again). <code>git -C ~/.config/texnvim checkout main</code> comes back. Every decision of the move, with before/after examples: the Migration decisions section, or <code>snippets/DECISIONS.md</code>.</p>`,
    },
    {
      id: "t-plugins", title: "Something broke after a plugin update", k: "lazy restore plugins lock",
      body: `<p>${keys([":Lazy restore", "Enter"])} puts every plugin back to the version in <code>lazy-lock.json</code>, then restart ntex. The lock file is in the config's git repo: <code>git -C ~/.config/texnvim log -p lazy-lock.json</code> shows what changed.</p>`,
    },
    {
      id: "t-tests", title: "Checking that everything still works", k: "tests run.sh",
      body: `<p><code>~/.config/texnvim/tests/run.sh</code> runs every suite headless (about a minute; no windows, never touches your snippets folder). One suite: <code>run.sh mine</code> (your snippets, hot reload), <code>defaults</code> (the Latex Suite defaults you kept, and the ones that stay off), <code>mangling</code> (234 commands typed as written), <code>english</code> (ordinary text stays), <code>decisions</code> (every example in the decision log), <code>perf</code>, <code>system</code> (health, template, completion; compiling only with <code>TEXNVIM_TEST_COMPILE=1</code>), <code>reference</code> (this page builds and lists every snippet).</p>`,
    },
    {
      id: "t-reference", title: "This page is out of date, or didn't rebuild", k: "reference page build stale",
      body: `<p>Saving a snippet file rebuilds it in the background (you only hear about it if that fails). ${keys([":Reference", "Enter"])} rebuilds when anything is newer than the page, then opens it. From a terminal: <code>node ~/.config/texnvim/reference/build.mjs</code> prints what went wrong; <code>--full</code> re-types every example. Examples that didn't expand as expected are listed in <code>reference/build-report.txt</code>.</p>`,
    },
  ];
}
