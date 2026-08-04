mod cli;
mod config;
mod doctor;
mod entur;
mod location;
mod render;
mod upgrade;
mod watch;

use anyhow::{Context, Result, bail};
use chrono::Local;
use clap::Parser;
use cli::{Cli, Command, Common, ConfigAction, RouteAction};
use config::{Config, Place, Route, Waypoint};
use entur::Client;
use entur::trip::TripQuery;
use location::{Options, Origin, resolve_named, resolve_origin};
use render::{Style, board};
use std::io::Write;

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("feil: {e:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// Write to stdout, treating a closed pipe as a normal end rather than an error.
///
/// Without this, `ruter hjem | head -5` panics as soon as `head` exits, which is
/// the default behaviour of the `print!` macros. Every command writes through
/// here, including the list and confirmation output, so that piping any of them
/// into `head` ends quietly rather than panicking.
pub fn emit(text: &str) -> Result<()> {
    let mut out = std::io::stdout().lock();
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        other => Ok(other?),
    }
}

fn run(cli: Cli) -> Result<()> {
    // Both handled before the config is touched. `deny_unknown_fields` means a config
    // written by a newer version stops an older binary dead; upgrading is the way out of
    // that, and diagnosing it is exactly what `doctor` is for. Neither may need a
    // readable config to run.
    match &cli.command {
        Some(Command::Upgrade { check }) => return upgrade::run(*check),
        Some(Command::Doctor) => return doctor::run(),
        _ => {}
    }

    // `--watch` returns into a full-screen loop that never reaches the `--json`
    // branch, so the two together would silently print nothing at all.
    if cli.common.json && cli.common.watch {
        bail!("--json og --watch kan ikke brukes sammen. Velg \u{e9}n av dem.");
    }

    let config = Config::load()?;
    let client = Client::new(&config.client_name);
    let style = Style::detect(cli.common.colour_override());

    match cli.command {
        Some(Command::Config { action }) => cmd_config(action, config, &client),
        Some(Command::Route { action }) => cmd_route(action, config, &client, &cli.common),
        Some(Command::Near { radius, stops }) => {
            cmd_near(&client, &config, &cli.common, style, radius, stops)
        }
        Some(Command::Where) => cmd_where(&client, &config, &cli.common, style),
        Some(Command::Upgrade { .. }) | Some(Command::Doctor) => {
            unreachable!("handled before the config is loaded")
        }
        // Joined back into one string: the destination is collected word by word so
        // that an address with spaces does not have to be quoted.
        None => {
            let destination = (!cli.destination.is_empty()).then(|| cli.destination.join(" "));
            cmd_trip(&client, &config, &cli.common, style, destination)
        }
    }
}

// ---------------------------------------------------------------------------
// ruter [DESTINATION]
// ---------------------------------------------------------------------------

fn cmd_trip(
    client: &Client,
    config: &Config,
    common: &Common,
    style: Style,
    destination: Option<String>,
) -> Result<()> {
    // Checked up front, before anything touches GPS or the geocoder: a typo in a flag
    // should not cost the user a position lookup and two network round trips first.
    // Checked here rather than in the client so that it fails as a plain CLI error,
    // instead of opening the watch view only to fill it with a fetch error.
    let modes = common.modes.clone().unwrap_or_else(|| config.modes.clone());
    entur::validate_modes(&modes)?;

    let destination = destination.or_else(|| config.default_destination.clone()).context(
        "ingen destinasjon oppgitt.\n\
         Bruk `ruter <sted>`, eller sett `default_destination` i konfigurasjonen.",
    )?;

    // A saved route wins over a place of the same name: `route add` refuses to create
    // that collision, so one can only exist in a hand-edited config.
    let route = config.route(&destination).cloned();

    let opts = Options { no_gps: common.no_gps, no_ip: common.no_ip };
    let saved_start = route.as_ref().and_then(|r| r.from.as_ref());
    let origin = match (common.from.as_deref(), saved_start) {
        // An explicit --from overrides a route's saved start, so that
        // `ruter sognsvann --from jobb` runs the same route from elsewhere.
        (Some(explicit), _) => resolve_named(client, config, explicit, location::Source::Explicit)?,
        (None, Some(start)) => place_origin(start, location::Source::SavedPlace),
        (None, None) => resolve_origin(client, config, None, opts)?,
    };

    let (target, via) = match &route {
        Some(route) => (
            place_origin(&route.to, location::Source::SavedPlace),
            route.via.iter().map(|w| w.id.clone()).collect(),
        ),
        None => {
            (resolve_named(client, config, &destination, location::Source::Explicit)?, Vec::new())
        }
    };

    let query = TripQuery {
        via,
        num_patterns: common.count.unwrap_or(config.num_results),
        max_walk_minutes: config.max_walk_minutes,
        modes,
    };

    if common.watch {
        return watch::run_trip(
            &config.client_name,
            &origin,
            &target,
            &query,
            config.watch_interval_secs,
        );
    }

    let patterns = client.trip(origin.coord, target.coord, &query)?;

    if common.json {
        return emit(&format!("{}\n", serde_json::to_string_pretty(&patterns)?));
    }

    let now = Local::now().fixed_offset();
    emit(&board::trip_board(&patterns, &origin, &target.name, now, style))
}

/// Let the user pick among geocoder matches, or take the first one outright.
///
/// A single match needs no question, and `--yes` skips the prompt entirely so the
/// command stays usable from a script.
fn choose(
    matches: Vec<entur::geocode::GeoMatch>,
    query: &str,
    yes: bool,
) -> Result<entur::geocode::GeoMatch> {
    if yes || matches.len() == 1 {
        return Ok(matches.into_iter().next().expect("geocode errors when empty"));
    }

    let mut prompt = format!("Treff for \"{query}\":\n");
    for (i, m) in matches.iter().enumerate() {
        let layer = m.layer.as_deref().unwrap_or("");
        prompt.push_str(&format!("  {}. {}  {}\n", i + 1, m.label, layer));
    }
    prompt.push_str(&format!("Velg [1-{}] (Enter = 1): ", matches.len()));
    emit(&prompt)?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim();
    let index = if input.is_empty() {
        0
    } else {
        input
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=matches.len()).contains(n))
            .context("ugyldig valg")?
            - 1
    };
    Ok(matches.into_iter().nth(index).expect("index was range-checked"))
}

/// Turn a saved place into an `Origin` without going back to the network.
fn place_origin(place: &Place, source: location::Source) -> Origin {
    Origin {
        coord: entur::Coord { lat: place.lat, lon: place.lon },
        name: place.label.clone(),
        source,
    }
}

// ---------------------------------------------------------------------------
// ruter route ...
// ---------------------------------------------------------------------------

fn cmd_route(
    action: RouteAction,
    mut config: Config,
    client: &Client,
    common: &Common,
) -> Result<()> {
    match action {
        RouteAction::List => {
            let mut out = String::new();
            if config.routes.is_empty() {
                out.push_str(
                    "Ingen lagrede reiseveier enn\u{e5}. Legg til \u{e9}n med:\n  \
                     ruter route add sognsvann --to Sognsvann --via \"Ullev\u{e5}l stadion, Oslo\"\n",
                );
            }
            for (name, route) in &config.routes {
                out.push_str(&format!("{name:<12} {}\n", route.label));
                let start = match &route.from {
                    Some(place) => place.label.as_str(),
                    None => "der du er n\u{e5}",
                };
                let mut chain = vec![start.to_string()];
                chain.extend(route.via.iter().map(|w| w.label.clone()));
                chain.push(route.to.label.clone());
                out.push_str(&format!("{:<12} {}\n", "", chain.join(" \u{2192} ")));
            }
            emit(&out)?;
        }

        RouteAction::Remove { name } => {
            if config.routes.remove(&name).is_none() {
                bail!("fant ingen lagret reisevei som heter \"{name}\"");
            }
            let path = config.save()?;
            emit(&format!("Fjernet \"{name}\" fra {}\n", path.display()))?;
        }

        RouteAction::Add { name, to, via, yes } => {
            // `ruter <navn>` looks routes up before places, so a shared name would make
            // the place unreachable. Refuse rather than silently shadow it.
            if config.place(&name).is_some() {
                bail!(
                    "\"{name}\" er allerede et lagret sted. Velg et annet navn for reiseveien, \
                     ellers blir stedet utilgjengelig."
                );
            }

            let start = match common.from.as_deref() {
                Some(raw) => Some(resolve_place(client, &config, raw, yes)?),
                None => None,
            };
            let destination = resolve_place(client, &config, &to, yes)?;

            let mut waypoints = Vec::new();
            for stop in &via {
                let chosen = choose(client.geocode_stops(stop, 8)?, stop, yes)?;
                let id =
                    chosen.id.clone().expect("geocode_stops only returns matches that carry an id");
                waypoints.push(Waypoint { label: chosen.label, id });
            }

            let label = format!(
                "{} \u{2192} {}",
                start.as_ref().map(|p| p.label.as_str()).unwrap_or("der du er n\u{e5}"),
                destination.label
            );
            let route =
                Route { label: label.clone(), from: start, to: destination, via: waypoints };

            let chain =
                route.via.iter().map(|w| w.label.as_str()).collect::<Vec<_>>().join(" \u{2192} ");
            config.routes.insert(name.clone(), route);
            let path = config.save()?;

            let mut out = format!("Lagret reiseveien \"{name}\": {label}\n");
            if !chain.is_empty() {
                out.push_str(&format!("Via: {chain}\n"));
            }
            out.push_str(&format!("{}\nKj\u{f8}r den med: ruter {name}\n", path.display()));
            emit(&out)?;
        }
    }
    Ok(())
}

/// Resolve a route endpoint to a saved `Place`, reusing an existing saved place
/// when the name matches one so that `--to hjem` does not geocode "hjem".
fn resolve_place(client: &Client, config: &Config, raw: &str, yes: bool) -> Result<Place> {
    if let Some(place) = config.place(raw) {
        return Ok(place.clone());
    }
    if let Some(coord) = location::parse_coord(raw) {
        return Ok(Place { label: raw.to_string(), lat: coord.lat, lon: coord.lon });
    }
    let chosen = choose(client.geocode(raw, 8)?, raw, yes)?;
    Ok(Place { label: chosen.label, lat: chosen.coord.lat, lon: chosen.coord.lon })
}

// ---------------------------------------------------------------------------
// ruter near
// ---------------------------------------------------------------------------

fn cmd_near(
    client: &Client,
    config: &Config,
    common: &Common,
    style: Style,
    radius: u32,
    stop_count: usize,
) -> Result<()> {
    let opts = Options { no_gps: common.no_gps, no_ip: common.no_ip };
    let origin = resolve_origin(client, config, common.from.as_deref(), opts)?;
    let per_stop = common.count.unwrap_or(config.num_results);

    if common.watch {
        return watch::run_near(
            &config.client_name,
            &origin,
            radius,
            stop_count,
            per_stop,
            config.watch_interval_secs,
        );
    }

    let boards = fetch_near(client, &origin, radius, stop_count, per_stop)?;

    if common.json {
        let payload: Vec<_> = boards
            .iter()
            .map(|(stop, board)| serde_json::json!({ "stop": stop, "departures": board }))
            .collect();
        return emit(&format!("{}\n", serde_json::to_string_pretty(&payload)?));
    }

    let now = Local::now().fixed_offset();
    emit(&board::near_board(&boards, &origin, now, style))
}

/// Shared by the one-shot and watch paths.
pub fn fetch_near(
    client: &Client,
    origin: &Origin,
    radius: u32,
    stop_count: usize,
    per_stop: usize,
) -> Result<Vec<(entur::nearest::NearbyStop, Option<entur::nearest::StopPlace>)>> {
    let stops = client.nearest_stops(origin.coord, radius, stop_count)?;
    stops
        .into_iter()
        .map(|stop| {
            let board = client.departures(&stop.id, per_stop)?;
            Ok((stop, board))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// ruter where
// ---------------------------------------------------------------------------

fn cmd_where(client: &Client, config: &Config, common: &Common, style: Style) -> Result<()> {
    // Resolve first: the Core Location authorization status only becomes
    // meaningful once something has actually asked for a position.
    let opts = Options { no_gps: common.no_gps, no_ip: common.no_ip };
    let resolved = resolve_origin(client, config, common.from.as_deref(), opts);

    let mut out = String::from("\n");
    for (key, value) in location::gps_diagnostics() {
        out.push_str(&format!("  {:<28} {value}\n", format!("{key}:")));
    }

    match resolved {
        Ok(origin) => {
            out.push_str(&format!("  {:<28} {}\n", "Posisjon:", style.bold(&origin.name)));
            out.push_str(&format!(
                "  {:<28} {:.5}, {:.5}\n",
                "Koordinater:", origin.coord.lat, origin.coord.lon
            ));
            out.push_str(&format!("  {:<28} {}\n", "Kilde:", origin.source.label()));
            if origin.source.is_coarse() {
                out.push_str(&format!(
                    "\n  {}\n",
                    style.yellow(
                        "\u{26a0} Dette er et IP-oppslag og kan bomme med flere kilometer."
                    )
                ));
            }
        }
        Err(e) => out.push_str(&format!("  {:<28} {e:#}\n", "Posisjon:")),
    }
    out.push('\n');
    emit(&out)
}

// ---------------------------------------------------------------------------
// ruter config ...
// ---------------------------------------------------------------------------

fn cmd_config(action: ConfigAction, mut config: Config, client: &Client) -> Result<()> {
    match action {
        ConfigAction::Path => {
            emit(&format!("{}\n", config::config_path()?.display()))?;
        }

        ConfigAction::List => {
            let mut out = String::new();
            if config.places.is_empty() {
                out.push_str(
                    "Ingen lagrede steder enn\u{e5}. Legg til ett med:\n  \
                     ruter config add hjem \"Dronningens gate 40, Oslo\"\n",
                );
            }
            for (name, place) in &config.places {
                out.push_str(&format!(
                    "{name:<12} {}  ({:.5}, {:.5})\n",
                    place.label, place.lat, place.lon
                ));
            }
            emit(&out)?;
        }

        ConfigAction::Remove { name } => {
            if config.places.remove(&name).is_none() {
                bail!("fant ikke noe lagret sted som heter \"{name}\"");
            }
            let path = config.save()?;
            emit(&format!("Fjernet \"{name}\" fra {}\n", path.display()))?;
        }

        ConfigAction::Add { name, query, yes } => {
            let query = query.join(" ");
            if query.trim().is_empty() {
                bail!(
                    "oppgi en adresse, f.eks. `ruter config add hjem \"Dronningens gate 40, Oslo\"`"
                );
            }
            // The mirror of the check in `route add`: routes win the name lookup, so a
            // place sharing a route's name could never be reached.
            if config.route(&name).is_some() {
                bail!(
                    "\"{name}\" er allerede en lagret reisevei. Velg et annet navn for stedet, \
                     ellers blir det utilgjengelig."
                );
            }

            let chosen = &choose(client.geocode(&query, 8)?, &query, yes)?;

            config.places.insert(
                name.clone(),
                Place { label: chosen.label.clone(), lat: chosen.coord.lat, lon: chosen.coord.lon },
            );
            let path = config.save()?;
            emit(&format!(
                "Lagret \"{name}\" \u{2192} {} ({:.5}, {:.5})\n{}\n",
                chosen.label,
                chosen.coord.lat,
                chosen.coord.lon,
                path.display()
            ))?;
        }
    }
    Ok(())
}
