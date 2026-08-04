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
#   RUTER_NO_PATH   sett til 1 for å la profilfilen være i fred; da bare varsles det
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

# Sørger for at BIN_DIR ligger i PATH, slik at `ruter` virker uten at brukeren må
# rydde i profilfilen selv.
#
# `ruter upgrade` kjører dette skriptet på nytt, så det må være idempotent. To ting
# sikrer det: linjen legges bare til når katalogen faktisk mangler i PATH, og bare
# når den ikke allerede står i profilfilen fra sist.
#
# Skallet vårt er et barn av brukerens, så PATH i det som kjører nå kan ikke endres
# herfra uansett — derfor sier vi tydelig fra hva som må til for å ta det i bruk.
ensure_on_path() {
  case ":$PATH:" in
    *":$BIN_DIR:"*) return 0 ;;
  esac

  if [ "${RUTER_NO_PATH:-}" = "1" ]; then
    echo "OBS: $BIN_DIR ligger ikke i PATH (RUTER_NO_PATH=1, så den er ikke endret)."
    return 0
  fi

  # ${SHELL##*/} framfor basename: ett eksternt program mindre å være avhengig av.
  case "${SHELL##*/}" in
    fish)
      echo "OBS: $BIN_DIR ligger ikke i PATH. Legg den til med:"
      echo "    fish_add_path $BIN_DIR"
      return 0
      ;;
    zsh) profile="$HOME/.zshrc" ;;
    bash)
      # Innlogging på macOS leser .bash_profile, ikke .bashrc.
      if [ "$(uname -s)" = "Darwin" ]; then
        profile="$HOME/.bash_profile"
      else
        profile="$HOME/.bashrc"
      fi
      ;;
    *) profile="$HOME/.profile" ;;
  esac

  line="export PATH=\"$BIN_DIR:\$PATH\""

  if [ -f "$profile" ] && grep -Fqs "$line" "$profile"; then
    echo "OBS: $BIN_DIR står allerede i $profile, men mangler i PATH her."
    echo "    Åpne et nytt terminalvindu, eller kjør:  . $profile"
    return 0
  fi

  # Subshell rundt det hele: feiler selve omdirigeringen, er det skallet som skriver
  # feilmeldingen, og da hjelper det ikke å dempe stderr for printf alene.
  if ! (printf '\n# lagt til av ruter-cli\n%s\n' "$line" >>"$profile") 2>/dev/null; then
    echo "OBS: klarte ikke skrive til $profile. Legg til selv:"
    echo "    $line"
    return 0
  fi

  echo "==> La $BIN_DIR i PATH via $profile"
  echo "    Gjelder nye terminalvinduer. For dette vinduet:  . $profile"
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
fi

# Utenfor if-en med vilje: macOS-grenen legger symlenken i samme BIN_DIR, så den
# trenger akkurat den samme sjekken. Før lå den bare i Linux-grenen, og en
# macOS-bruker uten ~/.local/bin i PATH fikk ikke et ord om hvorfor `ruter` manglet.
ensure_on_path

echo
echo "ruter $version er installert."

# `ruter upgrade` kjører dette skriptet, så onboarding-hintet ville ellers dukket opp
# ved hver eneste oppgradering — og se ut som om konfigurasjonen var borte.
config="${XDG_CONFIG_HOME:-$HOME/.config}/ruter/config.toml"
[ -f "$config" ] || echo "Kom i gang:  ruter config add hjem \"Dronningens gate 40, Oslo\""
