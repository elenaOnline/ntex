# Terminal extras (optional)

ntex runs in any terminal. These are the small additions the author uses; copy
the lines you want.

- **Font.** [JuliaMono](https://juliamono.netlify.app) has proper Unicode maths
  (sub/superscripts, ∑ ∫ ∈ ℝ), so concealed LaTeX reads at full size.
  `brew install --cask font-juliamono`.
- **[cmux](https://github.com/manaflow-ai/cmux)** (on libghostty): `\lv` opens
  the PDF in a cmux split (tdf or the built-in browser), and `:Reference` opens
  in a browser split. `bin/ntex` tells snacks.nvim it is in a Ghostty-class
  terminal so rendered maths (off by default) uses kitty graphics.
- **Ghostty.app** reads the same `ghostty` config as cmux.
- **Tern**: `\lv` opens a Tern browser split by default (`tern_pdf_viewer` in
  `prefs.lua`).

| File | Goes to |
| --- | --- |
| `ghostty.config` | `~/.config/ghostty/config` (read by cmux and Ghostty.app) |
| `cmux-config.ghostty` | `~/Library/Application Support/com.cmuxterm.app/config.ghostty` (cmux only) |
| `cmux.json` | `~/.config/cmux/cmux.json` (lets `cmux` commands from ntex reach the app) |
| `tern-settings.json` | merge into `~/Library/Application Support/Tern/settings.json` |

After editing cmux's config: `cmux reload-config` from a cmux terminal.
