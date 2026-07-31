mod cli;
mod config;
mod entur;
mod location;
mod render;
mod watch;

use anyhow::{Context, Result, bail};
use chrono::Local;
use clap::Parser;
use cli::{Cli, Command, Common, ConfigAction};
use config::{Config, Place};
use entur::Client;
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
/// the default behaviour of the `print!` macros.
fn emit(text: &str) -> Result<()> {
    let mut out = std::io::stdout().lock();
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        other => Ok(other?),
    }
}

fn run(cli: Cli) -> Result<()> {
    let config = Config::load()?;
    let client = Client::new(&config.client_name);
    let style = Style::detect(cli.common.colour_override());

    match cli.command {
        Some(Command::Config { action }) => cmd_config(action, config, &client),
        Some(Command::Near { radius, stops }) => {
            cmd_near(&client, &config, &cli.common, style, radius, stops)
        }
        Some(Command::Where) => cmd_where(&client, &config, &cli.common, style),
        None => cmd_trip(&client, &config, &cli.common, style, cli.destination),
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
    let destination = destination.or_else(|| config.default_destination.clone()).context(
        "ingen destinasjon oppgitt.\n\
         Bruk `ruter <sted>`, eller sett `default_destination` i konfigurasjonen.",
    )?;

    let opts = Options { no_gps: common.no_gps, no_ip: common.no_ip };
    let origin = resolve_origin(client, config, common.from.as_deref(), opts)?;
    let target = resolve_named(client, config, &destination, location::Source::Explicit)?;

    let count = common.count.unwrap_or(config.num_results);
    let modes = common.modes.clone().unwrap_or_else(|| config.modes.clone());

    if common.watch {
        return watch::run_trip(
            &config.client_name,
            &origin,
            &target,
            count,
            config.max_walk_minutes,
            &modes,
            config.watch_interval_secs,
        );
    }

    let patterns =
        client.trip(origin.coord, target.coord, count, config.max_walk_minutes, &modes)?;

    if common.json {
        return emit(&format!("{}\n", serde_json::to_string_pretty(&patterns)?));
    }

    let now = Local::now().fixed_offset();
    emit(&board::trip_board(&patterns, &origin, &target.name, now, style))
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
        out.push_str(&format!("  {:<22} {value}\n", format!("{key}:")));
    }

    match resolved {
        Ok(origin) => {
            out.push_str(&format!("  {:<22} {}\n", "Posisjon:", style.bold(&origin.name)));
            out.push_str(&format!(
                "  {:<22} {:.5}, {:.5}\n",
                "Koordinater:", origin.coord.lat, origin.coord.lon
            ));
            out.push_str(&format!("  {:<22} {}\n", "Kilde:", origin.source.label()));
            if origin.source.is_coarse() {
                out.push_str(&format!(
                    "\n  {}\n",
                    style.yellow(
                        "\u{26a0} Dette er et IP-oppslag og kan bomme med flere kilometer."
                    )
                ));
            }
        }
        Err(e) => out.push_str(&format!("  {:<22} {e:#}\n", "Posisjon:")),
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
            println!("{}", config::config_path()?.display());
        }

        ConfigAction::List => {
            if config.places.is_empty() {
                println!(
                    "Ingen lagrede steder enn\u{e5}. Legg til ett med:\n  \
                     ruter config add hjem \"Storgata 1, Oslo\""
                );
            }
            for (name, place) in &config.places {
                println!("{name:<12} {}  ({:.5}, {:.5})", place.label, place.lat, place.lon);
            }
        }

        ConfigAction::Remove { name } => {
            if config.places.remove(&name).is_none() {
                bail!("fant ikke noe lagret sted som heter \"{name}\"");
            }
            let path = config.save()?;
            println!("Fjernet \"{name}\" fra {}", path.display());
        }

        ConfigAction::Add { name, query, yes } => {
            let query = query.join(" ");
            if query.trim().is_empty() {
                bail!("oppgi en adresse, f.eks. `ruter config add hjem \"Storgata 1, Oslo\"`");
            }

            let matches = client.geocode(&query, 8)?;
            let chosen = if yes || matches.len() == 1 {
                &matches[0]
            } else {
                println!("Treff for \"{query}\":");
                for (i, m) in matches.iter().enumerate() {
                    let layer = m.layer.as_deref().unwrap_or("");
                    println!("  {}. {}  {}", i + 1, m.label, layer);
                }
                print!("Velg [1-{}] (Enter = 1): ", matches.len());
                std::io::stdout().flush()?;

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
                &matches[index]
            };

            config.places.insert(
                name.clone(),
                Place { label: chosen.label.clone(), lat: chosen.coord.lat, lon: chosen.coord.lon },
            );
            let path = config.save()?;
            println!(
                "Lagret \"{name}\" \u{2192} {} ({:.5}, {:.5})\n{}",
                chosen.label,
                chosen.coord.lat,
                chosen.coord.lon,
                path.display()
            );
        }
    }
    Ok(())
}
