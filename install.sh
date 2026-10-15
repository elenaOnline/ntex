#!/bin/sh
# install.sh — put ntex where it expects to live and link its launchers.
#   ./install.sh           link ~/.config/texnvim → this checkout (if it isn't there already)
#                          and ~/.local/bin/{ntex,ntex-studio,ntex-fig} → bin/
#   ./install.sh --studio  also build Ntex Studio + Ntex Figures (macOS, Rust, tectonic)
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
CONF="${XDG_CONFIG_HOME:-$HOME/.config}/texnvim"
BIN="$HOME/.local/bin"

if [ "$HERE" != "$CONF" ]; then
  if [ -e "$CONF" ] || [ -L "$CONF" ]; then
    echo "install.sh: $CONF already exists; move it away or clone this repo there." >&2
    exit 1
  fi
  mkdir -p "$(dirname "$CONF")"
  ln -s "$HERE" "$CONF"
  echo "linked $CONF → $HERE"
fi

mkdir -p "$BIN"
for b in ntex ntex-studio ntex-fig; do
  ln -sf "$CONF/bin/$b" "$BIN/$b"
  echo "linked $BIN/$b"
done
case ":$PATH:" in *":$BIN:"*) ;; *) echo "note: add $BIN to your PATH" ;; esac

for tool in nvim:neovim tectonic:tectonic; do
  cmd=${tool%%:*}; pkg=${tool#*:}
  command -v "$cmd" >/dev/null 2>&1 || echo "missing: $cmd (brew install $pkg)"
done

if [ "${1:-}" = "--studio" ]; then
  "$CONF/studio/build.sh"
fi
echo "done. Try: ntex scratch.tex   (first start installs the plugins)"
