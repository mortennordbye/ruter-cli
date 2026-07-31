#!/bin/sh
# Installerer `ruter` fra siste release.
#
#   curl -fsSL https://raw.githubusercontent.com/mortennordbye/ruter-cli/main/scripts/install.sh | sh
#
# Kjøres også av `ruter upgrade`, som setter RUTER_VERSION til versjonen den fant.
#
# Miljøvariabler:
#   RUTER_VERSION   tag som skal installeres, f.eks. v0.3.0 (standard: siste release)
#   RUTER_BIN_DIR   hvor symlenken/binærfilen havner (standard: ~/.local/bin)
#   RUTER_APP_DIR   hvor Ruter.app havner på macOS (standard: ~/Applications/Ruter.app)
#
# POSIX sh på toppnivå, slik at `| sh` virker uansett hva brukeren har. macOS-delen
# håndteres av install.sh inni arkivet, som er bash og gjør app-bundle, signering og
# karantene — den logikken skal bare finnes ett sted.
set -eu

REPO="mortennordbye/ruter-cli"
BIN_DIR="${RUTER_BIN_DIR:-$HOME/.local/bin}"

die() {
  echo "feil: $*" >&2
  exit 1
}

need() {
  command -v "$1" >/dev/null 2>&1 || die "mangler $1, som trengs for å installere"
}

need curl
need tar

case "$(uname -s)" in
  Darwin) platform="macos-universal" ;;
  Linux)
    case "$(uname -m)" in
      x86_64 | amd64) platform="linux-x86_64" ;;
      aarch64 | arm64) platform="linux-aarch64" ;;
      *) die "ingen ferdigbygd binærfil for $(uname -m). Bygg selv: cargo install --git https://github.com/$REPO" ;;
    esac
    ;;
  *) die "ruter støtter bare macOS og Linux" ;;
esac

# GitHub omdirigerer /releases/latest til den faktiske taggen. Å lese Location-headeren
# unngår både JSON-parsing og API-rategrensen på uautentiserte kall.
version="${RUTER_VERSION:-}"
if [ -z "$version" ]; then
  version=$(
    curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest" |
      sed 's|.*/tag/||'
  ) || die "fant ikke siste versjon"
  [ -n "$version" ] || die "fant ikke siste versjon"
fi

name="ruter-$version-$platform"
url="https://github.com/$REPO/releases/download/$version/$name.tar.gz"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM

echo "==> Laster ned $name"
curl -fsSL -o "$tmp/$name.tar.gz" "$url" ||
  die "kunne ikke laste ned $url"
curl -fsSL -o "$tmp/$name.tar.gz.sha256" "$url.sha256" ||
  die "kunne ikke laste ned sjekksummen"

echo "==> Sjekker sjekksum"
(
  cd "$tmp"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c "$name.tar.gz.sha256" >/dev/null
  else
    need sha256sum
    sha256sum -c "$name.tar.gz.sha256" >/dev/null
  fi
) || die "sjekksummen stemmer ikke — avbryter"

tar -xzf "$tmp/$name.tar.gz" -C "$tmp"

if [ "$platform" = "macos-universal" ]; then
  echo "==> Installerer Ruter.app"
  need bash
  RUTER_BIN_DIR="$BIN_DIR" bash "$tmp/$name/install.sh"
else
  echo "==> Installerer $BIN_DIR/ruter"
  mkdir -p "$BIN_DIR"
  install -m 755 "$tmp/$name/ruter" "$BIN_DIR/ruter"
  echo "Installert. $BIN_DIR/ruter"
  case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) echo "OBS: $BIN_DIR ligger ikke i PATH." ;;
  esac
fi

echo
echo "ruter $version er installert."
echo "Kom i gang:  ruter config add hjem \"Storgata 1, Oslo\""
