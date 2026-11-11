# Ntex Studio

Make snippets, Tab shortcuts and new theorem-like blocks without leaving the
lecture. Open it, type, press ⌘↵, and you're back in ntex with the snippet
already working.

## Open it

| From | Does |
| --- | --- |
| Space n in ntex | a new snippet |
| Space n on a selection (visual mode) | a new snippet whose output is the selection |
| Space N in ntex | the inbox |
| `:SnippetStudio [snippet\|block\|inbox\|library\|look]` | that page |
| ⌘4 or ⌘F in the studio | the library: every snippet you have, searchable |
| ⌘5 in the studio | the editor look: palette, section rules, block colors, nesting lines |
| ⌘6 in the studio | the notes look: typeface, boxes and colors of the PDF |
| `ntex-studio` in a shell | the studio (`--page`, `--output-file`, `--context math\|text`) |

It stays running after ⌘W / Esc, so the next Space n is instant. ⌘Q quits it.

## The editor look (page 5, ⌘5)

How ntex colors .tex files while you type. **Palette**: Slate (default),
Code, Code Dark, Alabaster (the look before). **Standard**: twelve common editor
themes to compare against (VS Code Light+/Dark+, GitHub Light, One Light,
Solarized Light/Dark, Catppuccin Latte/Mocha, Tokyo Night, Gruvbox Dark,
Newpaper Light/Dark), in
their own colors with your structure settings on top. **Sections**: none, a rule
above each heading, or a tinted band. **Blocks**: plain, names colored by
kind, or names plus a bar in the gutter. **Begin/end**: color only the
environment name, or the whole `\begin{…}[…]` line. **Guides**: nesting
lines. **Black ink**: only for what prints, or for commands too.

Each change is saved to `~/.config/texnvim/look.json` at once, and every open
ntex repaints. Code Dark and the standard themes also switch the terminal's background and
text color while ntex runs; one of your light palettes (or quitting ntex)
gives the white back. In ntex,
`:NtexLook` shows the settings and `:NtexLook palette code` changes one.

**Colors**: every color of the palette in use (page, text, math, commands,
braces, comments, the block colors …) as a chip. Pick one for H, S and V
sliders and a hex field (Return applies). Edits belong to that palette and
repaint ntex live; *Reset this color* and *Reset all of …* undo them.

## Saved looks, structure, the notes look (pages 5 and 6)

**Saved looks** (top of the Look page, and of the Notes page): every look is
a button that puts it back. Slate (default), Alabaster (stock) and the notes
Theme default are always there and protected. *Save current
as* (a name), *Rename* and *Delete* work on your own looks. They live in
`~/.config/texnvim/looks.json`.

**Editor structure** (Look page): section rule glyph, subsection rule,
section numbers, gutter bar glyph and side, nesting line glyph and which
blocks get one, bold titles, and the shades of whole begin/end lines.

**Notes** (page 6, ⌘6): how the PDF looks. Typeface (Charter, Palatino,
Times, Utopia, Libertinus, STIX Two, Fourier) and line spacing, the
theorem-like boxes (fill, outline, corners, head color, bar width) and every
color of the notes palette (H, S, V, hex). Studio writes
`templates/tex/ntex-notes-fonts.tex` and `ntex-notes-look.tex`, which
ntex-base.sty reads; each exists only while it differs from the theme, so the
defaults give exactly the theme's PDF. A note shows a change at its next
compile.

Each kind of box (theorem, lemma, proposition, corollary, definition,
example, remark, proof, exercise) can also have its own bar, fill, outline,
corners, head color and body font on top of the shared shape, and the proof
its own end mark (□ ■ ∎ or none): `kinds` and `qed` in `notes-look.json`.
Proof and exercise start with their own settings (the theme's look for them),
so the shared shape doesn't reach them until they're cleared. Powerset's
Styles page edits these; the Notes page edits the shared shape.

## Make a snippet (page 1, ⌘1)

1. **Type the trigger** exactly as you will in ntex. Every key counts,
   including space and `\`. Tab makes it a Tab shortcut (a ⇥ appears).
   Return moves to the output.
2. **Type the output.** Some keys insert tokens:

   | Key | Inserts |
   | --- | --- |
   | Tab | a tab stop (with text selected, that text becomes its default) |
   | ⇧Tab | a mirror of the last tab stop, typed in both places at once (press again to pick another) |
   | ⌘E | where the cursor ends up |
   | ⌘⇧V | the selection (for "wraps a selection" snippets) |
   | ⌥1 … ⌥9 | the captured letter / pattern capture |

   Pasting `\frac{<>}{<>}`, `\frac{$1}{$2}$0` or `${1:x}` turns into tokens.
3. **Check it.** The studio guesses maths or text, and by itself or on Tab. The
   *(guessed)* tag disappears once you choose. It also:
   - **tries the snippet in a hidden ntex** with all your snippets loaded
     (Example 1.3: what typing it really gives);
   - **renders the output** with tectonic;
   - **warns** about clashes, shadowed triggers and English words.

   More options holds the rest: suffix, post, pattern, visual or raw Lua
   kinds; inline/display only; line rules; .tex/.md only; priority; a custom
   condition.
4. **⌘↵** saves it to `snippets/tex/inbox.lua`. Every open ntex reloads it at
   once. ⌘Z (outside a text field) undoes the last save, delete or sort.

## Make a block (page 2, ⌘2)

Pick a title ("Fun fact"), what it looks like (theorem, lemma, definition,
example or remark), numbering, body font, and a colour from your palette or a
new hue. The preview is your theme, rendered. ⌘↵ adds it to
`templates/tex/ntex-blocks.sty`, which `ntex-notes.sty` loads, so
`\begin{funfact}` works in the next compile. Its Tab shortcut (e.g.
`fun` + Tab at the start of a line) goes to the inbox. Your blocks are listed
under the form, with Edit and Remove.

To share a .tex with someone, send `ntex-blocks.sty` along with
`ntex-notes.sty`.

## The inbox (page 3, ⌘3)

New snippets land in `snippets/tex/inbox.lua`. It's an ordinary snippet
file, so you can edit it by hand. **Sort into my files** (⌘⇧S) asks Claude
Haiku 5.5 where each one belongs: it reads your topic files, then the studio
moves the snippets itself. It backs up first, checks every file loads, and
puts everything back if anything fails. One ⌘Z undoes the whole sort. A sort
takes a few seconds and costs a fraction of a cent. Backups are kept in
`~/.local/state/ntex-studio/backups/`.

## The library (page 4, ⌘4)

Every snippet in `snippets/tex/`, as **you type → you get**, grouped by file
(each file's group title, e.g. *Calculus*). "You get" is the output rendered
like page 1's preview; the first visit renders them one at a time in the
background (a few minutes for all of them), after that they're cached.

- **Search** as you type (⌘F from any page): fuzzy over the trigger (`dnt`
  finds `dint`), the LaTeX, the description and the file. Several words must
  all match. The categories under it narrow the list to one file.
- ↑/↓ choose, **Return** (or a double click, or Edit) opens the snippet on
  page 1 with everything page 1 has: Try it (in a hidden ntex that has the
  old version taken out), the render, the checks, More options.
- **⌘↵ there saves it back into its own file, in place**, and you land back
  on the library. Only that snippet changes: if you changed just the trigger
  or the output, only those two strings are swapped and the columns of the
  line stay aligned; any other change rewrites that one call (and adds the
  helper to the file's `local` line if it now needs one). The file is
  checked by the hidden ntex first; one that would not load is not written.
  ⌘Z undoes the save. Esc goes back without saving.
- Snippets the editor can't represent (an `env(…)`, a pattern with a `name`,
  a computed output like `dm`) open as their Lua, with the same in-place save.

## Settings

`studio = { … }` in `~/.config/texnvim/prefs.lua`:
- `hide_after_save`
- `typeface` ("Computer Modern" or "Palatino")
- `render_preview`
- `sort_model`, `sort_effort`

## Build, test

```sh
~/.config/texnvim/studio/build.sh      # fonts from your tectonic bundle + release build + Ntex Studio.app
cd ~/.config/texnvim/studio && cargo test --workspace   # headless: nothing opens on screen
```

`build.sh` wraps the release binary in `target/release/Ntex Studio.app`
(bundle id `io.github.ntex.studio`, icon from `macos/AppIcon.iconset`, whose 16 and 32 px sizes drop the handle; `macos/AppIcon.png` is the 1024 px master), so
the Dock and ⌘-Tab show Studio's name and icon. `ntex-studio` and ntex run the
binary inside it, and fall back to `target/release/ntex-studio` without it.

The screens are tested by rendering them offscreen. `evidence/` holds the
latest renders, and `verification/` holds the adversary's acceptance cases,
measured in a real headless ntex. How it's built is in `ARCHITECTURE.md` and
every control in `UX.md`.
