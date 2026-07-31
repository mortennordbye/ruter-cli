#!/bin/bash
# Installs Ruter.app and symlinks `ruter` onto your PATH.
#
# Shipped inside the macOS release tarball as install.sh. Kept as a file in the repo
# rather than inlined into the release workflow so it can be linted and read on its own.
set -euo pipefail

SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP="${RUTER_APP_DIR:-$HOME/Applications/Ruter.app}"
BIN="${RUTER_BIN_DIR:-$HOME/.local/bin}"

rm -rf "$APP"
mkdir -p "$(dirname "$APP")" "$BIN"
cp -R "$SRC/Ruter.app" "$APP"

# Re-sign locally. The Location Services grant is keyed to the code signature, and the
# ad-hoc signature applied in CI does not carry its identity across a download and copy.
codesign --force --sign - "$APP" >/dev/null 2>&1 || true

# Browsers set the quarantine flag on downloads; curl does not. Clearing it either way
# saves a Gatekeeper prompt on a binary that is ad-hoc signed rather than notarised.
xattr -dr com.apple.quarantine "$APP" 2>/dev/null || true

ln -sf "$APP/Contents/MacOS/ruter" "$BIN/ruter"

echo "Installert. $BIN/ruter -> $APP/Contents/MacOS/ruter"
if ! printf '%s' ":$PATH:" | grep -q ":$BIN:"; then
  echo "OBS: $BIN ligger ikke i PATH."
fi
echo "Kjør 'ruter where' for å sjekke at posisjonen virker."
echo "Første gang kan macOS spørre om tilgang til stedstjenester."
