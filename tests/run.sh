#!/bin/sh
# Headless tests for ntex. No windows are opened: the config runs in
# `nvim --embed --headless` and is typed into over RPC.
#
#   ~/.config/texnvim/tests/run.sh               all suites
#   ~/.config/texnvim/tests/run.sh mine         one suite
#
# Suites: mine (your snippets, .md and .tex, hot reload), defaults (the Latex
# Suite defaults you kept, and the ones that stay off), mangling (234 commands
# typed as written come out unchanged), english (ordinary text stays as typed),
# decisions (every after-example in snippets/decisions.json), perf
# (per-keystroke cost), system (health, math rendering, template, :Scratch,
# conceal, completion), ide (compile errors as diagnostics, texlab, Harper,
# Copilot unauthenticated, the finder; compiles with tectonic, viewer off),
# figures (Ntex Figures: Space g, .fig syntax + diagnostics from a fake
# ntex-fig, figure PDFs recompiling the .tex; never launched, never compiled),
# reference (the reference page builds).
#
# The snippets folder is *copied* to a temp folder first, so the hot-reload
# test edits the copy, never ~/.config/texnvim/snippets. Nothing compiles
# (TEXNVIM_NO_COMPILE=1) unless TEXNVIM_TEST_COMPILE=1, except in `ide`.
set -u
# Never a viewer during tests: no Skim reload after a compile, no \lv / \ls,
# no cmux pane, no browser, no `open` (init.lua, core.lua, pdfview.lua,
# reference.lua all check it). Every suite, every child Neovim.
TEXNVIM_NO_VIEWER=1
export TEXNVIM_NO_VIEWER
HERE=$(cd "$(dirname "$0")" && pwd)
CFG=$(dirname "$HERE")
APP=$(basename "$CFG") # texnvim (or texnvim-next while developing)
SUITES=${*:-mine defaults mangling english decisions perf system ide figures reference}
TMP=$(mktemp -d "${TMPDIR:-/tmp}/texnvim-tests.XXXXXX")
trap 'rm -rf "$TMP"' EXIT INT TERM

# the symbol list (and its allowlist) from the reference page, for `mangling`
node --input-type=module -e '
const m = await import(process.argv[1]);
process.stdout.write(JSON.stringify({ symbols: m.SYMBOLS.map((s) => s[0]), intended: m.INTENDED || {} }));
' "$CFG/reference/content/symbols.mjs" > "$TMP/symbols.json" || { echo "could not read reference/content/symbols.mjs"; exit 1; }

reference_suite() {
  out="$TMP/reference"
  mkdir -p "$out"
  [ -f "$CFG/reference/examples.json" ] && cp "$CFG/reference/examples.json" "$out/"
  echo
  echo "== suite: reference  (config: $CFG)"
  if TEXNVIM_APPNAME="$APP" node "$CFG/reference/build.mjs" --check --out "$out"; then
    echo "  ok    build succeeds and every loaded snippet is on the page"
    printf '{"pass":1,"fail":0,"failures":[],"suite":"reference"}' > "$TMP/reference.json"
  else
    echo "  FAIL  reference build (see above)"
    printf '{"pass":0,"fail":1,"failures":["reference build"],"suite":"reference"}' > "$TMP/reference.json"
    return 1
  fi
}

status=0
for suite in $SUITES; do
  if [ "$suite" = reference ]; then
    reference_suite || status=1
    continue
  fi
  cp -R "$CFG/snippets" "$TMP/snippets-$suite"
  TEXNVIM_APPNAME="$APP" TEXNVIM_SNIPPETS="$TMP/snippets-$suite" TEXNVIM_TEST_DATA="$TMP" TEXNVIM_DECISIONS="$CFG/snippets/decisions.json" \
    nvim --clean --headless -l "$HERE/driver.lua" "$suite" "$TMP/$suite.json" || status=1
done

echo
echo "== summary"
for suite in $SUITES; do
  if [ -f "$TMP/$suite.json" ]; then
    printf '  %-9s %s\n' "$suite" "$(cat "$TMP/$suite.json")"
  else
    printf '  %-9s (did not finish)\n' "$suite"
    status=1
  fi
done

# language servers must exit with Neovim: no orphaned harper-ls / copilot / texlab
# (parent = launchd) may be left behind by the test run
sleep 2
orphans=$(for p in $(pgrep -f "harper-ls|copilot-language-server|texlab" 2>/dev/null); do
  [ "$(ps -o ppid= -p "$p" 2>/dev/null | tr -d ' ')" = "1" ] && echo "$p"; done)
if [ -n "$orphans" ]; then
  printf '  %-9s FAIL: orphaned language servers left running: %s\n' "orphans" "$(echo $orphans)"
  status=1
else
  printf '  %-9s %s\n' "orphans" "none left running"
fi
exit $status
