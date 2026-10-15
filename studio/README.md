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
| `:SnippetStudio [snippet\|block\|inbox]` | that page |
| `ntex-studio` in a shell | the studio (`--page`, `--output-file`, `--context math\|text`) |

It stays running after ⌘W / Esc, so the next Space n is instant. ⌘Q quits it.

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

## Settings

`studio = { … }` in `~/.config/texnvim/prefs.lua`:
- `hide_after_save`
- `typeface` ("Computer Modern" or "Palatino")
- `render_preview`
- `sort_model`, `sort_effort`

## Build, test

```sh
~/.config/texnvim/studio/build.sh      # fonts from your tectonic bundle + release build
cd ~/.config/texnvim/studio && cargo test --workspace   # headless: nothing opens on screen
```

The screens are tested by rendering them offscreen. `evidence/` holds the
latest renders, and `verification/` holds the adversary's acceptance cases,
measured in a real headless ntex. How it's built is in `ARCHITECTURE.md` and
every control in `UX.md`.
