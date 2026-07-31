//! Working out where the user currently is.
//!
//! Sources are tried in descending order of trustworthiness and the winner is
//! reported alongside the coordinate, because the difference matters a lot: GPS
//! puts you on the right street corner, IP geolocation can put you in the wrong
//! town. Measured during development, two IP providers disagreed by 55 km on the
//! same connection — so an IP-derived origin is always labelled as such.

use crate::config::Config;
use crate::entur::{Client, Coord};
use anyhow::{Result, bail};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Explicit `--from`, either coordinates or a geocoded address.
    Explicit,
    /// A named place from the config file.
    SavedPlace,
    /// macOS Core Location.
    Gps,
    /// IP geolocation. Coarse — city level at best.
    Ip,
    /// `default_origin` from the config, used when everything else failed.
    ConfigDefault,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Explicit => "oppgitt",
            Source::SavedPlace => "lagret sted",
            Source::Gps => "GPS",
            Source::Ip => "IP (unøyaktig)",
            Source::ConfigDefault => "standard fra config",
        }
    }

    /// Whether the user should be warned that this position may be far off.
    pub fn is_coarse(self) -> bool {
        matches!(self, Source::Ip)
    }
}

#[derive(Debug, Clone)]
pub struct Origin {
    pub coord: Coord,
    pub name: String,
    pub source: Source,
}

/// Options that let the caller switch individual sources off.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    pub no_gps: bool,
    pub no_ip: bool,
}

/// Resolve the origin, trying each source in turn.
///
/// `explicit` is the raw `--from` value, which may be a saved place name,
/// a `lat,lon` pair, or a free-text address to geocode.
pub fn resolve_origin(
    client: &Client,
    config: &Config,
    explicit: Option<&str>,
    opts: Options,
) -> Result<Origin> {
    if let Some(raw) = explicit {
        return resolve_named(client, config, raw, Source::Explicit);
    }

    if !opts.no_gps {
        match gps() {
            Ok(Some(coord)) => return Ok(finish(client, coord, Source::Gps)),
            Ok(None) => {}
            // A GPS failure is expected on Linux and common on macOS when the
            // terminal lacks permission. Note it and carry on down the chain.
            Err(e) => eprintln!("obs: fant ikke posisjon via GPS ({e})"),
        }
    }

    if !opts.no_ip {
        match ip_location(client) {
            Ok(coord) => return Ok(finish(client, coord, Source::Ip)),
            Err(e) => eprintln!("obs: fant ikke posisjon via IP ({e})"),
        }
    }

    if let Some(default) = config.default_origin.as_deref() {
        return resolve_named(client, config, default, Source::ConfigDefault);
    }

    bail!(
        "fant ikke posisjonen din.\n\
         Prøv `--from \"<adresse>\"`, eller lagre et sted med \
         `ruter config add hjem \"<adresse>\"` og sett `default_origin`."
    )
}

/// Resolve a place name, a `lat,lon` pair, or an address.
pub fn resolve_named(
    client: &Client,
    config: &Config,
    raw: &str,
    source: Source,
) -> Result<Origin> {
    if let Some(place) = config.place(raw) {
        return Ok(Origin {
            coord: Coord { lat: place.lat, lon: place.lon },
            name: place.label.clone(),
            source: if source == Source::Explicit { Source::SavedPlace } else { source },
        });
    }

    if let Some(coord) = parse_coord(raw) {
        return Ok(finish(client, coord, source));
    }

    let matches = client.geocode(raw, 1)?;
    let best = matches.into_iter().next().expect("geocode returns an error when empty");
    Ok(Origin { coord: best.coord, name: best.label, source })
}

/// Attach a human-readable name to a bare coordinate. Reverse geocoding is a
/// nicety, so a failure here degrades to showing the raw numbers.
fn finish(client: &Client, coord: Coord, source: Source) -> Origin {
    let name = client
        .reverse_geocode(coord)
        .ok()
        .flatten()
        .unwrap_or_else(|| format!("{:.4}, {:.4}", coord.lat, coord.lon));
    Origin { coord, name, source }
}

/// Parse an explicit `"59.9139,10.7522"` argument.
pub fn parse_coord(raw: &str) -> Option<Coord> {
    let (lat, lon) = raw.split_once(',')?;
    let lat: f64 = lat.trim().parse().ok()?;
    let lon: f64 = lon.trim().parse().ok()?;
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return None;
    }
    Some(Coord { lat, lon })
}

// ---------------------------------------------------------------------------
// IP geolocation
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct IpInfo {
    /// "59.9127,10.7461"
    loc: Option<String>,
}

fn ip_location(client: &Client) -> Result<Coord> {
    let info: IpInfo = client.get_json("https://ipinfo.io/json")?;
    let loc = info.loc.as_deref().unwrap_or_default();
    parse_coord(loc).ok_or_else(|| anyhow::anyhow!("uventet svar fra ipinfo.io"))
}

// ---------------------------------------------------------------------------
// GPS
// ---------------------------------------------------------------------------

/// `Ok(None)` means "no fix within the timeout" rather than a hard failure.
#[cfg(target_os = "macos")]
fn gps() -> Result<Option<Coord>> {
    use objc2_core_location::{CLAuthorizationStatus, CLLocationManager};
    use objc2_foundation::{NSDate, NSRunLoop};
    use std::time::{Duration, Instant};

    const TIMEOUT: Duration = Duration::from_secs(4);

    unsafe {
        // Deliberately not calling `locationServicesEnabled()`: Apple deprecated
        // it because it can block the calling thread, and the timeout below
        // covers the "services are off" case with the same message anyway.
        let manager = CLLocationManager::new();

        // Fail fast when permission has already been refused, rather than
        // making the user wait out the timeout on every single invocation.
        match manager.authorizationStatus() {
            CLAuthorizationStatus::Denied | CLAuthorizationStatus::Restricted => {
                bail!("{}", permission_hint())
            }
            _ => {}
        }

        manager.requestWhenInUseAuthorization();
        manager.startUpdatingLocation();

        // No delegate: CLLocationManager publishes the most recent fix on its
        // `location` property, so we can pump the run loop and poll it. That
        // avoids declaring an Objective-C delegate class entirely, which is by
        // far the fiddliest part of talking to CoreLocation from Rust.
        let started = Instant::now();
        let mut found = None;
        while started.elapsed() < TIMEOUT {
            let until = NSDate::dateWithTimeIntervalSinceNow(0.1);
            NSRunLoop::currentRunLoop().runUntilDate(&until);

            if let Some(location) = manager.location() {
                let c = location.coordinate();
                found = Some(Coord { lat: c.latitude, lon: c.longitude });
                break;
            }
        }
        manager.stopUpdatingLocation();

        if found.is_none() {
            bail!("tidsavbrudd. {}", permission_hint());
        }
        Ok(found)
    }
}

/// macOS attributes a CLI tool's location request to the *terminal app* that
/// launched it, not to the binary, so the hint has to name that app to be
/// actionable. `TERM_PROGRAM` is how we find out which one it is.
#[cfg(target_os = "macos")]
fn permission_hint() -> String {
    let app = std::env::var("TERM_PROGRAM").unwrap_or_else(|_| "terminalen din".to_string());
    format!(
        "gi {app} tilgang under Systeminnstillinger > Personvern og sikkerhet > \
         Stedstjenester, og start den p\u{e5} nytt. Bruk `--no-gps` for \u{e5} hoppe over dette."
    )
}

#[cfg(not(target_os = "macos"))]
fn gps() -> Result<Option<Coord>> {
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_coordinate_pairs() {
        let c = parse_coord("59.9139,10.7522").unwrap();
        assert!((c.lat - 59.9139).abs() < 1e-9);
        assert!((c.lon - 10.7522).abs() < 1e-9);
        assert!(parse_coord(" 59.9 , 10.7 ").is_some());
        assert!(parse_coord("-33.86,151.20").is_some());
    }

    #[test]
    fn rejects_things_that_are_not_coordinates() {
        assert!(parse_coord("home").is_none());
        assert!(parse_coord("Storgata 1, Oslo").is_none());
        assert!(parse_coord("59.9139").is_none());
        assert!(parse_coord("").is_none());
    }

    #[test]
    fn rejects_out_of_range_coordinates() {
        assert!(parse_coord("91.0,10.0").is_none());
        assert!(parse_coord("59.0,181.0").is_none());
    }

    #[test]
    fn only_ip_is_flagged_as_coarse() {
        assert!(Source::Ip.is_coarse());
        assert!(!Source::Gps.is_coarse());
        assert!(!Source::SavedPlace.is_coarse());
        assert!(!Source::Explicit.is_coarse());
    }

    #[test]
    fn ipinfo_loc_field_parses() {
        let info: IpInfo =
            serde_json::from_str(r#"{"loc":"59.9127,10.7461","city":"Oslo"}"#).unwrap();
        let coord = parse_coord(info.loc.as_deref().unwrap()).unwrap();
        assert!((coord.lat - 59.9127).abs() < 1e-9);
    }
}
