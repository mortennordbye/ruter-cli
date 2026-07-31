# ruter-cli

Neste avgang med buss, trikk, T-bane og tog — fra der du står, i terminalen.

Bygget på [Entur](https://developer.entur.org/) sitt åpne Journey Planner-API, så det dekker hele
Norge. Navnet kommer av at det er skrevet for daglig bruk i Oslo, der Ruter er operatøren.

```
  Oslo S  →  Hjemme   14:14

  ● om 2 min     14:16 → 14:34   18 min   direkte
      ↳ gå 3 min → Jernbanetorget
      [18 ] trikk   Jernbanetorget spor F 14:19 ●     → Holbergs plass
      ↳ gå 10 min → Ullevålsveien 15, Oslo

  ● om 5 min     14:19 → 14:38   19 min   direkte
      ↳ gå 2 min → Jernbanetorget
      [37 ] buss    Jernbanetorget spor A 14:21 ● +1  → Stensberggata
      ↳ gå 4 min → Ullevålsveien 15, Oslo
```

## Kom i gang

```sh
cargo install --path .

ruter config add hjem "Ullevålsveien 15, Oslo"
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
| `ruter "Oslo S"` | destinasjonen kan også være en adresse eller `59.91,10.75` |
| `ruter hjem --json` | rå JSON, for skripting |

Nyttige flagg: `-n` antall resultater, `--modes bus,tram` for å begrense transportmidler,
`--no-gps` / `--no-ip` for å skru av posisjonskilder, `--color auto\|always\|never`.

`ruter config list`, `ruter config remove <navn>` og `ruter config path` finnes også.

## Hvor den tror du er

Kildene prøves i denne rekkefølgen, og den første som svarer vinner:

1. `--from` — enten et lagret sted, `lat,lon`, eller en adresse
2. macOS Core Location (GPS/WiFi)
3. IP-oppslag via ipinfo.io
4. `default_origin` fra konfigurasjonen

**Om GPS på macOS:** stedstjenester tildeles terminalappen som startet prosessen, ikke selve
binærfilen. Du må gi f.eks. Ghostty eller Terminal tilgang under *Systeminnstillinger → Personvern
og sikkerhet → Stedstjenester*, og starte den på nytt. Uten dette venter `ruter` i fire sekunder
før den faller tilbake til IP — bruk `--no-gps` hvis du vil hoppe over forsøket.

**IP-oppslag er grovt.** Under utviklingen ga to ulike tjenester svar som lå 55 km fra hverandre på
samme tilkobling. Derfor merkes alltid resultatet med en advarsel når posisjonen kommer derfra.

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
label = "Ullevålsveien 15, Oslo"
lat = 59.920331
lon = 10.742684
```

Sett gjerne din egen `client_name`. Entur krever at klienter identifiserer seg, og anonyme
forespørsler blir rate-limitet hardere.

## Utvikling

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

Testene kjører mot innspilte API-svar i `tests/fixtures/`, så de trenger ikke nett.

## Lisens

MIT. Rutedata kommer fra Entur under [NLOD](https://data.norge.no/nlod/no/2.0).
Dette er et hobbyprosjekt uten tilknytning til Ruter AS eller Entur AS.
