#!/bin/sh
# Build Ntex Studio (the release binary that ~/.local/bin/ntex-studio runs).
#
#   ~/.config/texnvim/studio/build.sh            fonts (if missing) + release build
#   ~/.config/texnvim/studio/build.sh --fonts    only the fonts
#   ~/.config/texnvim/studio/build.sh --bundle   only Ntex Studio.app (after a build)
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

# target/release/Ntex Studio.app around a copy of the release binary, so the
# Dock, ⌘-Tab and Finder show Studio's name and icon. The .icns is made on
# every build from macos/AppIcon.iconset, else from macos/AppIcon.png (1024 px).
# ntex-studio (~/.local/bin) and ntex run the binary inside the bundle, and
# fall back to target/release/ntex-studio when the bundle is missing.
bundle() {
  TARGET="${CARGO_TARGET_DIR:-$HERE/target}/release"
  APP="$TARGET/Ntex Studio.app"
  VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$HERE/Cargo.toml" | head -n 1)
  MINOS=$(vtool -show-build "$TARGET/ntex-studio" 2>/dev/null | awk '$1 == "minos" { print $2; exit }')
  ICONSET="$TARGET/AppIcon.iconset"
  rm -rf "$APP.part" "$ICONSET"
  mkdir -p "$APP.part/Contents/MacOS" "$APP.part/Contents/Resources"
  cp "$TARGET/ntex-studio" "$APP.part/Contents/MacOS/ntex-studio"
  sed -e "s/@VERSION@/$VERSION/g" -e "s/@MINOS@/${MINOS:-11.0}/g" "$HERE/macos/Info.plist" > "$APP.part/Contents/Info.plist"
  if [ -d "$HERE/macos/AppIcon.iconset" ]; then
    # the checked-in iconset: its 16 and 32 px sizes are drawn without the
    # handle, so keep them rather than scaling the 1024 down
    cp -R "$HERE/macos/AppIcon.iconset" "$ICONSET"
  else
    mkdir -p "$ICONSET"
    for px in 16 32 128 256 512; do
      sips -z $px $px "$HERE/macos/AppIcon.png" --out "$ICONSET/icon_${px}x${px}.png" >/dev/null
      sips -z $((px * 2)) $((px * 2)) "$HERE/macos/AppIcon.png" --out "$ICONSET/icon_${px}x${px}@2x.png" >/dev/null
    done
  fi
  iconutil -c icns "$ICONSET" -o "$APP.part/Contents/Resources/AppIcon.icns"
  # ad-hoc signature over the whole bundle (Info.plist and icon included)
  codesign --force --sign - "$APP.part" 2>/dev/null
  rm -rf "$ICONSET" "$APP"
  mv "$APP.part" "$APP"
  # so Finder and the Dock pick up a changed icon
  touch "$APP"
  echo "build.sh: bundled $APP"
}

fonts
[ "${1:-}" = "--fonts" ] && exit 0
if [ "${1:-}" = "--bundle" ]; then bundle; exit 0; fi

cd "$HERE"
cargo build --release --workspace --bins   # ntex-studio and ntex-fig
bundle
echo "build.sh: built $HERE/target/release/Ntex Studio.app (run it with: ntex-studio, or Space n in ntex)"
