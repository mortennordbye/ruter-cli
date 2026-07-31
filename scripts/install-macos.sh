#!/bin/bash
# Install `ruter` on macOS so Core Location actually works.
#
# A bare CLI binary gets no Location Services grant: macOS attributes the
# request to whatever process launched it (your terminal, or whatever is
# wrapping it), and if that process has no location usage description the
# prompt is suppressed and Core Location returns nothing forever.
#
# Putting the binary inside a minimal .app bundle gives it its own TCC identity,
# so it can be granted access on its own terms. The bundle is not a GUI app;
# it is just the directory layout macOS needs to recognise an identity.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="${RUTER_APP_DIR:-$HOME/Applications/Ruter.app}"
BIN_DIR="${RUTER_BIN_DIR:-$HOME/.local/bin}"

cd "$REPO_ROOT"

echo "==> Bygger release-binærfil"
cargo build --release --locked

echo "==> Lager $APP_DIR"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
cp macos/Info.plist "$APP_DIR/Contents/Info.plist"
cp target/release/ruter "$APP_DIR/Contents/MacOS/ruter"

# The TCC grant is keyed on the code signature, so an unsigned binary would lose
# its permission on every rebuild. Ad-hoc signing is enough to keep it stable.
echo "==> Signerer"
codesign --force --sign - "$APP_DIR" >/dev/null 2>&1

echo "==> Symlenker $BIN_DIR/ruter"
mkdir -p "$BIN_DIR"
ln -sf "$APP_DIR/Contents/MacOS/ruter" "$BIN_DIR/ruter"

echo
echo "Ferdig. $BIN_DIR/ruter -> $APP_DIR/Contents/MacOS/ruter"
if ! printf '%s' ":$PATH:" | grep -q ":$BIN_DIR:"; then
  echo "OBS: $BIN_DIR ligger ikke i PATH."
fi
echo
echo "Kjør 'ruter where' for å sjekke at posisjonen virker."
echo "Første gang kan macOS spørre om tilgang til stedstjenester."
