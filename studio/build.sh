#!/bin/sh
# Build Ntex Studio (the release binary that ~/.local/bin/ntex-studio runs).
#
#   ~/.config/texnvim/studio/build.sh            fonts (if missing) + release build
#   ~/.config/texnvim/studio/build.sh --fonts    only the fonts
#
# The fonts are the TeX ones, copied out of your tectonic bundle into
# assets/fonts/ (once; they're bundled into the binary). Latin Modern for the
# default look, TeX Gyre Pagella for prefs `typeface = "Palatino"`.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
FONTS="$HERE/assets/fonts"

fonts() {
  mkdir -p "$FONTS"
  for name in \
    lmroman10-regular lmroman10-italic lmroman10-bold lmroman10-bolditalic \
    lmromancaps10-regular lmmono10-regular lmmono10-italic latinmodern-math \
    texgyrepagella-regular texgyrepagella-italic texgyrepagella-bold texgyrepagella-bolditalic \
    XCharter-Roman XCharter-Italic XCharter-Bold XCharter-BoldItalic
  do
    out="$FONTS/$name.otf"
    [ -s "$out" ] && continue
    if ! command -v tectonic >/dev/null 2>&1; then
      echo "build.sh: tectonic isn't installed (brew install tectonic); it supplies the fonts" >&2
      exit 1
    fi
    echo "build.sh: $name.otf from the tectonic bundle"
    # tectonic needs a working directory it may write its cache notes in
    if ! (cd "${TMPDIR:-/tmp}" && tectonic -X bundle cat "$name.otf") > "$out.part" 2>/dev/null || [ ! -s "$out.part" ]; then
      rm -f "$out.part"
      echo "build.sh: $name.otf is not in the tectonic bundle" >&2
      exit 1
    fi
    mv "$out.part" "$out"
  done
}

fonts
[ "${1:-}" = "--fonts" ] && exit 0

cd "$HERE"
cargo build --release --workspace --bins   # ntex-studio and ntex-fig
echo "build.sh: built $HERE/target/release/ntex-studio (run it with: ntex-studio, or Space n in ntex)"
