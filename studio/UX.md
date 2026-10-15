# Ntex Studio: UX

The author opens this mid-lecture, so speed is the product. The keyboard-only path
for a new snippet must be: **Space n → type trigger → Return → type output
(Tab for tab stops) → ⌘↵**, and she's back in ntex with the snippet live.
Everything else (options, checks, renders, the inbox, blocks) supports that
path and never gets in its way. The best comparisons are Raycast's snippet
creator for speed, Obsidian Latex Suite's snippet syntax for power, and a
well-set LaTeX document for looks.

Quality bar: input-to-frame under 16 ms on every keystroke. First frame after
launch under 300 ms; fonts are bundled, so nothing waits on the bridge. The
bridge, renders and agent never block the UI.

## The look: "set in LaTeX"

Every surface should look as if it came out of `pdflatex` with her notes theme
(`templates/tex/ntex-notes.sty`). The page is the window. Avoid web-UI clichés:
no rounded cards, no drop shadows, no grey app chrome, no system font, no pill
buttons.

| Element | LaTeX it imitates | Spec |
| --- | --- | --- |
| Window | the page | white paper `#FFFFFF`, ink `#1E1E28`; transparent title bar (traffic lights on the paper); margins 44 px left/right, 28 px top under the running head |
| Running head | `fancyhdr` | left: *Ntex Studio* (italic serif, slate); right: the three pages in small caps, `Snippet · Block · Inbox³` (superscript count). The current page is ink and bold, the others slate. 0.4 pt rule under it (1 px ink at 60 %). Clicking a page switches to it; ⌘1/⌘2/⌘3 too. |
| Page title | `\section` with her titlesec | `1  New snippet`: 21 px bold serif, the number in theme blue `#1E47B8` |
| Groups | her tcolorbox theorem blocks | tinted fill, 3 px rule on the left, no radius, padding 11/10/7/7 px. Run-in head **Definition 1.1** (Trigger). in the head tone, bold head, upright name in parentheses, period. Box kinds and tones exactly as ntex-notes.sty (theorem blue, lemma violet, definition teal, example maroon, remark green, proof grey 1 px rule on F4F4F4). |
| Generated code | the proof block | `Proof (Generated Lua).` head in *italic* (amsthm proof style), code in LM Mono 13 px, a □ (`\qedsymbol`, grey) at the bottom right |
| Body text | `\normalsize` Latin Modern Roman 10 | 15 px, line height 1.25 |
| Labels | `\textsc` | Latin Modern Roman Caps 15 px (small caps) for field labels: `Trigger`, `Fires`, `Where` |
| Code / LaTeX source | `\texttt` | Latin Modern Mono 14 px (trigger field 26 px) |
| Text fields | fill-in blanks | no box. The text sits on a 1 px slate rule, which turns 1.5 px blue when focused. Placeholder italic slate. |
| Buttons | `\fbox` / `\doublebox` | 1 px ink border, square corners, padding 3×9, small-caps label, optional keycap. Primary uses a double rule (`\doublebox`, 1 px + 1 px with a 2 px gap). Hover fills theorem-back `#E0E7FB`, press darkens 6 %, disabled is 40 % ink. A focus ring is a 1.5 px blue outline 2 px outside. |
| Radio | an inline enumerate | `● by itself   ○ on Tab`. ● is ink, ○ slate (LM Math glyphs, U+25CF/U+25CB). A guessed value adds *(guessed)* in italic slate. |
| Checkbox | `$\square$` / `$\boxtimes$` | ☐ / ☒ + label |
| Keycaps | `menukeys` `\keys{}` | LM Mono 12 px, 1 px slate border, radius 2, padding 0×4, baseline-aligned |
| Tables | `booktabs` | heavy rule 1.2 px top/bottom, 0.6 px mid rule, no vertical rules, header in small caps |
| Warnings | the LaTeX log | `Studio Warning: ‘ff’ starts 12 English words (offer, effect, …).` in italic slate. A blocking error looks like TeX's `! Trigger is empty.`, in error ink `#A9182D`, mono. |
| Hints | footnotes | at the bottom of the page: a 1/3-width 0.4 pt rule, then numbered 12 px notes with superscript numbers. They hold the keys for whatever is focused, e.g. `¹ Tab inserts a tab stop · ⇧Tab a mirror · ⌘E the exit point.` |
| Toasts | a margin note | small italic text in the right margin area, or the footer line when narrow: *Saved ‘ff’ to the inbox — live in ntex now. ⌘Z undoes.* Fades after 4 s. |
| Token chips | small `\fbox`es in the source | see below |

Typeface: `prefs.typeface = "Palatino"` swaps the serif for TeX Gyre Pagella,
to match her notes. The default, Computer Modern, says "LaTeX" the loudest.

Colour use is limited to the theme's: blue for structure, the box tones, and
error ink. Nothing else.

Window: 860 × 960 default, min 620 × 560, resizable, remembers its frame
(`state/window.json`). The page content scrolls vertically. The running head
and footnotes stay pinned.

## Page 1: New snippet (⌘1)

```
Ntex Studio                                  SNIPPET · Block · Inbox³
───────────────────────────────────────────────────────────────────────
1  New snippet

┃ Definition 1.1 (Trigger).
┃   ff ⇥                                    ← TriggerField, 26 px mono
┃   ─────────────────────────
┃   Fires  ● by itself  ○ on Tab (guessed)   In  ● maths  ○ text (guessed)

┃ Theorem 1.2 (Output).
┃   [⇥ tab stop] [⇧⇥ mirror] [⌘E end] [⌘⇧V selection] [⌥1 letter]   ← only allowed ones
┃   \frac{①}{②}                              ← TokenEditor
┃   Rendered:  (image: ½ with blue boxes)    ← right-aligned, fades in

┃ Example 1.3 (Try it in ntex).
┃   In  $…$  of a .tex file:
┃   ─────────────────────────────── (booktabs)
┃   You type        You get
┃   ───────────────────────────────
┃   ff              \frac{‸}{}
┃   a ⇥             \frac{a}{‸}
┃   b ⇥             \frac{a}{b}‸
┃   ───────────────────────────────

┃ Remark (Checks).      ← only when there are findings
┃   Studio Warning: ‘ff’ is already a snippet: \frac{…} in operations.lua (yours).  [Use priority 1]
┃   ! Output is empty.

More options. ▸                               ← \paragraph, collapsed
   Kind   ● plain  ○ follows a letter  ○ grabs the letter before  ○ pattern  ○ wraps a selection  ○ Lua
   Maths  ● anywhere  ○ only $…$  ○ only display
   Line   ● anywhere  ○ at the start  ○ after text  ○ in a list item    (text only)
   Files  ● .tex and .md  ○ .tex  ○ .md
   Priority  0̲    Description  _______    Example keys  _______
   Condition  function(line, trigger, captures) ______ end   (mono, multi-line)

┃ Proof (Generated Lua).
┃   math("ff", [[\frac{<>}{<>}]], "yours"),                         □

                    [ Clear  ⌘N ]   ╔ Save to inbox  ⌘↵ ╗
───────────
¹ Type the trigger exactly as you will in ntex. Tab makes it expand on Tab.
```

Behaviour:

- **Opening.** Focus goes to the trigger field, which is empty, or keeps the
  draft if it wasn't saved. Opening with a selection from ntex (Space n in
  visual mode) prefills the output with it as text, focuses the trigger, and
  guesses the context from where her cursor was.
- **Trigger field.** Every printable key is literal: space, `\`, `/`, digits,
  `(`. Tab toggles "on Tab", shown as a ⇥ keycap after the text. It also sets
  Fires to "on Tab" and pins it. Return or ↓ moves focus to the output.
  ⌥⌫ deletes a word and ⌘⌫ everything. Esc hides the window if the draft is
  empty, otherwise it blurs. For Kind=Pattern the field takes a Lua pattern;
  for Kind=Visual it takes one key.
- **Output editor.** Multi-line, mono. The keys:

  | Key | Does |
  | --- | --- |
  | Tab | inserts the next tab stop. With text selected, the selection becomes that tab stop's default. |
  | ⇧Tab | inserts a mirror of the last tab stop before the caret. Pressed again right away, it cycles through the tab stops. |
  | ⌘E | the exit point (at most one; inserting moves it) |
  | ⌘⇧V | the selection (visual kind; offers to switch kind otherwise) |
  | ⌥1…⌥9 | captures (post/pattern) |
  | Return | a newline. Auto-indent copies the previous line's leading tabs and spaces. |
  | ⌘↵ | save |
  | Esc | blur |

  The toolbar buttons do the same things and show their keys.

  **Chips:** a tab stop is a small box with a theme-blue 1 px border and fill
  `#E0E7FB`, holding its number in LM Mono 11 px (`1`). With a default, the
  chip reads `1 x`, the default text in mono ink. Mirrors are the same chip
  with a dashed border. The exit chip is green (remark tones) with `end`.
  The selection chip is violet with `sel`. A capture chip is maroon with
  `letter` (post) or `%1` (pattern). The caret steps over a chip as one
  character. Backspace deletes a whole chip, double-click selects its
  default text for editing, and hover shows a tooltip ("Tab stop 1 — typed
  in 2 places").

  **Paste** of helper syntax (`\frac{<>}{<>}`), UltiSnips or VS Code syntax
  (`\frac{$1}{$2}$0`, `${1:x}`, `${VISUAL}`) becomes chips. Plain LaTeX stays
  text. Copy writes helper syntax.
  Tab stops renumber 1, 2, 3 in reading order after every edit
  (`codegen::renumber`).
- **Guesses** (`infer::guess`) fill every un-pinned field on each change. The
  *(guessed)* tag disappears once she sets the field. Clicking or keying a
  radio pins it.
- **Example (Try it).** This runs in a real headless ntex: 250 ms after the
  last edit, if the draft has no Error findings. While it runs, a faint
  *trying…* shows in place of the table, without flicker: the old table stays
  until the new one arrives. If a different snippet expands, say so plainly:
  *ntex expanded ‘ffx’ from operations.lua instead.* If nothing expands, show
  *Nothing expanded* with the reason if known (for example, a Tab snippet
  where a line-start rule wasn't met). Clicking the "In $…$ of a .tex file"
  line cycles the place: inline, display, text and so on, as allowed.
- **Rendered.** 600 ms after the last edit, via tectonic. It fades in at 150
  ms. A TeX error shows as a single slate log line (`! Missing $ inserted.`),
  not as an Error finding. It's off when `prefs.render_preview = false`.
- **Checks** live in a Remark box that appears and disappears without moving
  the save button too much: it reserves no space when there are no findings.
  The page scrolls, and the buttons stay at the bottom of the content. A
  finding with a fix shows a small `\fbox` button, and clicking it applies the
  fix.
- **Save (⌘↵).** If there's any Error, the first error flashes: the box rule
  pulses blue twice and focus moves to the offending field. Otherwise:
  1. Compute the file with `inbox::with_draft`, then run
     `Bridge::check_file`. If the bridge isn't ready yet, save anyway: the
     loader's guard keeps the old file active if it's broken, and say so.
  2. Back up, write, toast, and clear the form.
  3. If `prefs.hide_after_save`, hide the app with `cx.hide()`, so macOS
     returns focus to the terminal.
  4. ⌘Z on the page (not in a text field) undoes the last save, delete or
     sort.
- **Clear (⌘N)** resets the draft. The cleared draft can be restored with ⌘Z.

## Page 2: New block (⌘2)

```
2  New block

┃ Definition 2.1 (Name).
┃   Title  Fun fact________      Environment  funfact (mono, follows the title until edited)

┃ Theorem 2.2 (Looks like).
┃   Like  ○ theorem  ○ lemma  ○ definition  ○ example  ● remark
┃   Numbered  ● no  ○ with theorems (Fun fact 1.4)  ○ on its own (Fun fact 1)
┃   Body  ○ italic  ● upright
┃   Colour  [Blue][Violet][Teal][Maroon][Green][Grey][New hue…]   ← swatches: three stacked tones
┃           Hue ━━━━━●━━━━━ 145°   (only for New hue)
┃           body 14.1 : 1 · head 6.0 : 1   ✓

┃ Example 2.3 (Preview).
┃   (image: a Remark box beside the new Fun fact box, rendered by tectonic with her theme)

┃ Lemma 2.4 (Snippet).
┃   Trigger  fun ⇥  (Tab at the start of a line, in .tex)     ☒ asks for a title
┃   → \begin{funfact}[‸] … \end{funfact}

┃ Proof (Generated LaTeX).
┃   % ── block: funfact ── … □

                                   ╔ Add to my template  ⌘↵ ╗

Your blocks
  ─────────────────────────────── booktabs: Block | Environment | Colour | Snippet | [Edit] [Remove]
```

- "Like" sets numbering, body and colour unless she has touched them: the
  same pin/guess model as snippets. For example, theorem gives italic body,
  numbered with theorems, and blue.
- Swatches: one per palette entry, each three stacked tones (rule, back, head),
  name in small caps under it. New hue opens the slider, and the tones come
  from `colour::tones_for_hue`.
- Contrast is shown live. Below the bar it's a warning, not a block.
- Save writes ntex-blocks.sty (making it and the `\RequirePackage` if needed)
  and puts the snippet in the inbox. The toast reads: *Added ‘Fun fact’ —
  \begin{funfact} works in every new compile; snippet ‘fun’ ⇥ is in the
  inbox.*
- Edit loads a block back into the form. Remove asks inline
  ("Remove ‘Fun fact’? Documents using \begin{funfact} will stop compiling.
  [Remove] [Keep]").

## Page 3: Inbox (⌘3)

```
3  Inbox — 3 snippets waiting

  ─────────────────────────────────────────────────────── booktabs
  Trigger   Output                    Fires      Made
  ───────────────────────────────────────────────────────
  ff        \frac{□}{□}               by itself  14:32     ← row: Return edits, ⌫ deletes (undoable)
  fun ⇥     \begin{funfact}…          Tab        14:40
  ───────────────────────────────────────────────────────

                        ╔ Sort into my files  ⌘⇧S ╗   Haiku 5.5 · high

┃ Proof (Sorting).                         ← while/after sorting
┃   reading operations.lua …               (streamed progress, italic slate)
┃   ff     → operations.lua, after //      fractions live there
┃   fun ⇥  → text.lua, after beg           the other Tab environments
┃   Moved 2 snippets.  [Undo ⌘Z]                                       □
```

- ↑/↓ moves the row focus. Return edits, which opens the entry on page 1
  with its id so saving replaces it. ⌫ deletes.
- Sorting runs with the button disabled and a Cancel button beside it. The
  plan is applied automatically when it arrives (one undo). If the check
  fails, everything is restored and the log says why.
- An empty inbox shows *Nothing waiting. New snippets land here first; sort
  them into your files when you have a minute.*

## Page 4: Library (⌘4, ⌘F)

```
4  Library — 234 snippets in 15 files
Search  ________________________________________   6 found
All²³⁴ · Accents²⁰ · Brackets¹⁶ · Calculus¹² · …        ← small caps, superscript counts; click narrows
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━  (booktabs)
   You type      You get (rendered)                Fires   (In, while searching)
Calculus  calculus.lua                                   ← blue bold head, mono slate file
 ▸ dint          ∫₀¹ □ dx                         maths, by itself
     \int_{0}^{1} □ \, dx                                 [Edit ↵]
     Definite integral · calculus.lua, line 18 · origin: changed
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

- The search field has focus on arrival; typing filters at once (fuzzy, words
  ANDed, best first; without a query the list is in file order under category
  heads). ↑/↓ move the ▸ even while typing; Return edits; Esc clears the
  search, then hides.
- The selected row opens: its LaTeX in mono, then description, example keys,
  file and line, origin, and Edit (Edit the Lua for entries page 1 can't
  represent).
- "You get" is the rendered output (10 pt at 1.5×, at most 64 px tall in a
  row); its LaTeX shows until a render exists. The list scrolls by itself so
  the search stays in view.
- Edit opens page 1 titled *Edit snippet*, with a slate line *In Calculus
  (calculus.lua, line 18). ⌘↵ saves it there, in place; Esc goes back to the
  library.* Buttons: Cancel (Esc) and Save changes (⌘↵). After the save the
  library comes back on the same row, with the toast *Updated ‘dint’ in
  Calculus — live in ntex now. ⌘Z undoes.*

## Global keys

| Key | Does |
| --- | --- |
| ⌘1 ⌘2 ⌘3 | pages (in a figure window ⌘1 is *actual size*, ⌘0 fits) |
| ⌘↵ | save (snippet / block) |
| ⌘N | clear |
| ⌘Z / ⌘⇧Z | in a field: text undo; elsewhere: undo the last save, delete or sort |
| ⌘⇧S | sort inbox |
| ⌘W, Esc on an empty draft | hide the window (the app keeps running; the next Space n is instant) |
| ⌘Q | quit (stops the bridge) |
| Tab / ⇧Tab | outside the trigger field and output editor: move between controls, in visual order |

Accessibility: every control is focusable, has a visible focus ring, and
shows its keys in the footnotes. Click targets are at least 24 px high.

## Copy

Plain English with LaTeX's dry voice. Short labels, no exclamation marks.
Name the file and the trigger in messages. Call the dumping ground "the
inbox", the topic files "your files", and Tab-triggered snippets "Tab
shortcuts" in hints.

## Figure labels (figure-qol class C)

- A label's field shows maths without `$…$`; a small `text` chip (an \fbox,
  filled when on) switches that label to text mode. Ways in: click the label
  of the selected object, double-click, ↵. Tab / ⇧Tab: every label in reading
  order. ↵ commits, Esc reverts. The footnote under the field says which mode
  it is in.

## Figure objects (figure-qol class A)

- One grammar for a drag of the selection: plain = the whole selection moves,
  each object snapping as before; **⌥** (held as the drag starts) leaves the
  originals and drags copies, arrows between the copies included, one undo
  step; **⇧** locks to horizontal or vertical (⇧ decides on release: a ⇧-drag
  moves the selection plus the object pressed, axis-locked; a ⇧-click without
  movement toggles it in the selection); **⌘** turns snapping off (it
  replaces the old ⌥ half-cell and ⇧ free-placement / free-angle meanings).
- ⌘C / ⌘X / ⌘V copy, cut and paste objects as `.fig` shorthand on the system
  clipboard (it pastes into Neovim, or another figure window); pasting text
  that is not shorthand says so and changes nothing. A paste never lands
  exactly on the originals: one cell right and down for nodes, half a cell for
  shapes, each further paste another step. ⌘D repeats the last ⌥-drag's
  offset (else one cell). Every footnote is one line at the default window
  width (1180 px); QH1 measures them. Distribute (three or more) sits beside
  Align in the panel and in the right-click menu. A turned shape's selection
  box turns with it.
