<div align="center">

# 🚋 ruter-cli

### Neste avgang med buss, trikk, T-bane og tog — fra der du står, i terminalen.

[![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)](https://www.rust-lang.org) [![Ratatui](https://img.shields.io/badge/Ratatui-2E3440?logo=rust&logoColor=white)](https://ratatui.rs) [![Entur](https://img.shields.io/badge/Entur%20API-1A1A1A?logo=graphql&logoColor=E10098)](https://developer.entur.org/) [![macOS](https://img.shields.io/badge/macOS-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/)

[![CI](https://github.com/mortennordbye/ruter-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/mortennordbye/ruter-cli/actions/workflows/ci.yml) [![Scorecard](https://github.com/mortennordbye/ruter-cli/actions/workflows/scorecard.yml/badge.svg)](https://github.com/mortennordbye/ruter-cli/actions/workflows/scorecard.yml) [![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/mortennordbye/ruter-cli/badge)](https://securityscorecards.dev/viewer/?uri=github.com/mortennordbye/ruter-cli)

[![License](https://img.shields.io/github/license/mortennordbye/ruter-cli?style=flat-square)](LICENSE) [![Last Commit](https://img.shields.io/github/last-commit/mortennordbye/ruter-cli?style=flat-square)](https://github.com/mortennordbye/ruter-cli/commits/main) [![Stars](https://img.shields.io/github/stars/mortennordbye/ruter-cli?style=flat-square)](https://github.com/mortennordbye/ruter-cli/stargazers)

Bygget på [Entur](https://developer.entur.org/) sitt åpne Journey Planner-API, så det dekker hele
Norge. Navnet kommer av at det er skrevet for daglig bruk i Oslo, der Ruter er operatøren.

</div>

---

```
  Storo, Oslo  →  Dronningens gate 40, Oslo                          11:59
  ════════════════════════════════════════════════════════════════════════
  ● om 3 min     12:02 → 12:20   18 min   direkte
      ↳ gå 1 min                                       → Storo
      [18 ] trikk  Storo spor A            12:03 ●     → Jernbanetorget
      ↳ gå 1 min                                       → Dronningens gate 40, Oslo
  ────────────────────────────────────────────────────────────────────────
  ● om 6 min     12:05 → 12:24   19 min   direkte
      ↳ gå 3 min                                       → Storo
      [ 5 ] T-bane Storo spor 1            12:08 ●     → Jernbanetorget
      ↳ gå 4 min                                       → Dronningens gate 40, Oslo
  ────────────────────────────────────────────────────────────────────────
```

## Kom i gang

### Installer

```sh
curl -fsSL https://raw.githubusercontent.com/mortennordbye/ruter-cli/main/scripts/install.sh | sh
```

Skriptet finner siste release, laster ned riktig binærfil for maskinen din, sjekker
sjekksummen og installerer til `~/.local/bin/ruter`. macOS får i tillegg `Ruter.app`
i `~/Applications`, fordi det er det som gir verktøyet egen tilgang til stedstjenester
— se forklaringen under.

Miljøvariabler hvis du vil styre hvor ting havner: `RUTER_BIN_DIR`, `RUTER_APP_DIR`,
og `RUTER_VERSION` for å pinne en bestemt tag.

### Oppgrader

```sh
ruter upgrade           # sjekker og installerer nyeste versjon
ruter upgrade --check   # bare si fra om det finnes noe nyere
```

### Manuelt fra Releases

Hver release har binærfiler under [Releases](https://github.com/mortennordbye/ruter-cli/releases):
én universal build for macOS (Apple Silicon og Intel i samme fil), og Linux x86_64 og aarch64.

```sh
tar -xzf ruter-vX.Y.Z-macos-universal.tar.gz
cd ruter-vX.Y.Z-macos-universal
./install.sh
```

Linux-arkivene inneholder bare binærfilen; legg den hvor du vil.

Binærfilene er ad-hoc-signert, ikke notarisert. Lastet ned med `curl` er det uproblematisk;
laster du ned via nettleser setter den karanteneflagget, og `install.sh` fjerner det.

### Bygge selv

```sh
./scripts/install-macos.sh          # macOS — bygger og setter opp app-bundlet
cargo install --path .              # Linux, eller hvis du ikke trenger GPS
```

### Så

```sh
ruter config add hjem "Dronningens gate 40, Oslo"
ruter hjem
```

`config add` slår opp adressen i Entur sin geokoder og lagrer koordinatene, så du slipper å
skrive inn lengde- og breddegrad selv.

## Bruk

| Kommando | Hva den gjør |
| --- | --- |
| `ruter hjem` | reise fra der du er nå til stedet du har lagret som `hjem` |
| `ruter hjem --watch` | samme, men oppdaterer seg selv |
| `ruter near` | avganger fra holdeplasser i nærheten |
| `ruter --from jobb hjem` | reise mellom to lagrede steder |
| `ruter Oslo S` | destinasjonen kan også være en adresse eller `59.91,10.75` |
| `ruter Brekkelia 3D` | adresser med mellomrom trenger ikke anførselstegn |
| `ruter hjem --json` | rå JSON, for skripting |
| `ruter where` | hvor den tror du er, og hvilken kilde den brukte |
| `ruter upgrade` | sjekk om det finnes en nyere versjon, og installer den |

Nyttige flagg: `-n` antall resultater, `--modes bus,tram` for å begrense transportmidler,
`--no-gps` / `--no-ip` for å skru av posisjonskilder, `--color auto\|always\|never`.

`ruter config list`, `ruter config remove <navn>` og `ruter config path` finnes også.

## Faste reiseveier

Reiseplanleggeren velger normalt den raskeste veien. Vil du alltid reise via bestemte
holdeplasser, lagrer du en reisevei:

```sh
ruter route add sognsvann \
  --from "Dronningens gate 40, Oslo" \
  --to "Sognsvann, Oslo" \
  --via "Ullevål stadion, Oslo" --via "Solvang, Oslo"
```

Kjør den etterpå med navnet:

```sh
ruter sognsvann
ruter sognsvann --watch
ruter sognsvann --from jobb    # samme reisevei, annet startpunkt
```

`--via` gjentas én gang per holdeplass, i den rekkefølgen du passerer dem. Dropper du
`--from`, starter reiseveien der du er akkurat nå.

Holdeplassene lagres med Entur-ID, ikke med teksten du skrev. «Solvang» treffer 25
holdeplasser i Norge, og den i Oslo kommer først på tiende plass — hadde navnet blitt
slått opp på nytt hver gang, ville reiseveien før eller siden endt et helt annet sted.
Av samme grunn står det «Solvang, Oslo» og ikke bare «Solvang» i eksempelet over.

`ruter route list` og `ruter route remove <navn>` finnes også. Et navn kan ikke være både
et lagret sted og en reisevei; `ruter <navn>` slår opp reiseveier først, så begge deler
ville gjort stedet uleselig.

## Hvor den tror du er

Kildene prøves i denne rekkefølgen, og den første som svarer vinner:

1. `--from` — enten et lagret sted, `lat,lon`, eller en adresse
2. macOS Core Location (GPS/WiFi)
3. IP-oppslag via ipinfo.io
4. `default_origin` fra konfigurasjonen

Kjør `ruter where` for å se hva den faktisk fant, og hvorfor.

### Hvorfor GPS trenger et app-bundle på macOS

En vanlig kommandolinje-binærfil har ingen bundle-identitet. `Bundle.main.bundleIdentifier` er
`nil`, og da har macOS ingenting å knytte en tillatelse til: forespørselen blir i stedet tilskrevet
prosessen som startet den. Har ikke *den* prosessen en `NSLocationUsageDescription`, blir dialogen
undertrykt i stillhet, og Core Location returnerer aldri noe. Ingen feilmelding, bare tomt.

To ting løser dette, og `scripts/install-macos.sh` gjør begge:

1. `build.rs` limer `macos/Info.plist` inn i `__TEXT,__info_plist`-seksjonen i binærfilen, som gir
   den en `CFBundleIdentifier`.
2. Skriptet installerer binærfilen i et lite `Ruter.app`-bundle og symlenker `~/.local/bin/ruter`
   dit, slik at den får sin egen oppføring i Stedstjenester i stedet for å arve terminalens.

Bundlet er ikke en GUI-app — det er bare mappestrukturen macOS krever for å kjenne igjen en
identitet. Symlenken funker fint; `ruter` oppfører seg som en helt vanlig kommando.

Etter installasjon dukker «ruter» opp under *Systeminnstillinger → Personvern og sikkerhet →
Stedstjenester*. Uten GPS venter `ruter` i ti sekunder før den faller tilbake til IP; `--no-gps`
hopper over forsøket. Er tilgangen allerede avslått, gir `ruter` opp med én gang i stedet for å
vente ut tidsavbruddet. Første gang dukker tilgangsdialogen opp, og da venter `ruter` i opptil
ett minutt på svar — de sekundene teller ikke mot tidsavbruddet på selve posisjonen.

Går GPS likevel ikke gjennom, viser `ruter where` hva som blokkerer: om stedstjenester i det hele
tatt er på, hvilken tilgang `ruter` har, om den kjører som app-bundle, og hva Core Location svarte
sist.

**IP-oppslag er grovt.** På én maskin ga IP-oppslaget Stortorvet mens GPS ga en adresse 5 km
unna, altså helt andre holdeplasser. To ulike IP-tjenester ga dessuten svar som lå 55 km fra
hverandre på samme tilkobling. Derfor merkes resultatet alltid med en advarsel når posisjonen
kommer derfra.

## Konfigurasjon

`~/.config/ruter/config.toml` (følger `$XDG_CONFIG_HOME` hvis den er satt):

```toml
client_name = "nordbye-ruter-cli"   # sendes som ET-Client-Name
default_destination = "hjem"
default_origin = "jobb"
num_results = 5
max_walk_minutes = 15
modes = ["bus", "tram", "metro", "rail", "water"]
watch_interval_secs = 30

[places.hjem]
label = "Dronningens gate 40, Oslo"
lat = 59.912517
lon = 10.74882

[routes.sognsvann]
label = "Dronningens gate 40, Oslo → Sognsvann, Oslo"
from = { label = "Dronningens gate 40, Oslo", lat = 59.912517, lon = 10.74882 }
to = { label = "Sognsvann, Oslo", lat = 59.96732, lon = 10.73375 }
via = [
  { label = "Ullevål stadion, Oslo", id = "NSR:StopPlace:58265" },
  { label = "Solvang, Oslo", id = "NSR:StopPlace:6162" },
]
```

Sett gjerne din egen `client_name`. Entur krever at klienter identifiserer seg, og anonyme
forespørsler blir rate-limitet hardere.

---

## Kodestruktur

```text
ruter-cli/
├── src/
│   ├── main.rs        # CLI-dispatch, exit-koder, stdout
│   ├── cli.rs         # clap-definisjoner
│   ├── config.rs      # TOML-config og lagrede steder
│   ├── location.rs    # posisjonskjeden, inkl. Core Location
│   ├── watch.rs       # --watch (ratatui) og bakgrunnstråden
│   ├── upgrade.rs     # ruter upgrade: versjonssjekk mot GitHub
│   ├── entur/         # Journey Planner v3-klient og geokoder
│   └── render/        # avgangstavla: farger, merker, tidsformat
├── macos/             # Info.plist, limes inn i binærfilen av build.rs
├── scripts/           # install.sh (curl-installer), install-macos.sh (bygg selv)
└── tests/fixtures/    # innspilte API-svar; testene går aldri på nett
```

---

## Workflows

| Workflow | Trigger | Hva den gjør |
| -------- | ------- | ------------ |
| CI | push, PR | `cargo fmt --check`, `clippy -D warnings`, `cargo test` på Linux og macOS |
| Dependency Review | PR | blokkerer avhengigheter med kjente sårbarheter |
| Scorecard | push, ukentlig | OpenSSF-vurdering av forsyningskjeden → Security-fanen |
| Release Please | push til main | lager release-PR med CHANGELOG og versjonsbump |
| Release Binaries | kalles av Release Please | bygger macOS universal + Linux x86_64/aarch64, henger dem på releasen og tar den ut av draft til slutt |

CI kjører på både `ubuntu-latest` og `macos-latest` med vilje: Core Location-koden i
`src/location.rs` er `cfg`-gjemt bak macOS, så Linux-jobben beviser at `objc2`-avhengigheten
faktisk er valgfri, og macOS-jobben er den eneste som i det hele tatt kompilerer den koden.

---

## Utvikling

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
```

Testene kjører mot innspilte API-svar i `tests/fixtures/`, så de trenger ikke nett.

Commits følger [Conventional Commits](https://www.conventionalcommits.org/) — release-please
regner ut versjonsnummer fra dem, og dette repoet har ingen automatisk sjekk av PR-titler,
så det er verdt å være nøye. `feat:` gir minor bump, `fix:` patch, `feat!:` major.

Se [CLAUDE.md](CLAUDE.md) og [AGENTS.md](AGENTS.md) for arbeidsflyt med kodeagenter.

---

## Lisens

MIT. Rutedata kommer fra Entur under [NLOD](https://data.norge.no/nlod/no/2.0).
Dette er et hobbyprosjekt uten tilknytning til Ruter AS eller Entur AS.

---

<div align="center">

### ⭐ Star this repo if you find it useful ⭐

<a href="https://www.star-history.com/#mortennordbye/ruter-cli&Date">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=mortennordbye/ruter-cli&type=Date&theme=dark" />
    <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=mortennordbye/ruter-cli&type=Date" />
    <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=mortennordbye/ruter-cli&type=Date" width="600" />
  </picture>
</a>

Made by [Morten Victor Nordbye](https://github.com/mortennordbye)

</div>
