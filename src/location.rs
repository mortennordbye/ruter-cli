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

#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
#[cfg(target_os = "macos")]
use objc2::{AnyThread, define_class, msg_send};
#[cfg(target_os = "macos")]
use objc2_core_location::{
    CLAuthorizationStatus, CLLocation, CLLocationManager, CLLocationManagerDelegate,
};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSArray, NSDate, NSError, NSRunLoop};
#[cfg(target_os = "macos")]
use std::sync::atomic::Ordering;

/// Authorization status as observed during the last real GPS attempt.
///
/// A freshly constructed `CLLocationManager` reports `NotDetermined` until
/// something asks it for a position, so `ruter where` would otherwise claim
/// macOS had never prompted on a machine where GPS demonstrably works. -1 means
/// no attempt has been made this process.
#[cfg(target_os = "macos")]
static LAST_AUTH_STATUS: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

/// Where the delegate leaves the fix for `gps()` to collect.
///
/// A static rather than a `define_class!` ivar because `gps()` runs at most once
/// per process, so there is no per-instance state to carry. Nothing here may
/// panic: unwinding out of an Objective-C callback is undefined behaviour, which
/// is why the lock is matched on rather than unwrapped.
#[cfg(target_os = "macos")]
static FIX: std::sync::Mutex<Option<Coord>> = std::sync::Mutex::new(None);

/// `NSError.code` from `didFailWithError`, or `i64::MIN` for "nothing reported".
///
/// Core Location otherwise fails in silence: without this the only signal was the
/// wait expiring, which looks the same whether the user is indoors, offline or
/// unauthorised.
#[cfg(target_os = "macos")]
static FAIL_CODE: std::sync::atomic::AtomicI64 =
    std::sync::atomic::AtomicI64::new(i64::MIN);

/// Whether macOS put the permission dialog on screen during this attempt.
#[cfg(target_os = "macos")]
static PROMPTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A sentence describing how the last GPS attempt ended, for `ruter where`.
#[cfg(target_os = "macos")]
static LAST_ATTEMPT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

#[cfg(target_os = "macos")]
fn record_attempt(what: impl Into<String>) {
    if let Ok(mut slot) = LAST_ATTEMPT.lock() {
        *slot = Some(what.into());
    }
}

/// Core Location delivers on the run loop, so it has to be pumped for any
/// delegate callback to arrive at all.
#[cfg(target_os = "macos")]
fn pump_run_loop(seconds: f64) {
    // SAFETY: pumping the calling thread's own run loop.
    unsafe {
        let until = NSDate::dateWithTimeIntervalSinceNow(seconds);
        NSRunLoop::currentRunLoop().runUntilDate(&until);
    }
}

/// Translate the `CLError` codes a user can actually act on.
#[cfg(target_os = "macos")]
fn cl_error_text(code: i64) -> String {
    match code {
        0 => "fant ingen posisjon (kCLErrorLocationUnknown). Vanlig innend\u{f8}rs, \
              uten Wi-Fi, eller rett etter oppstart"
            .to_string(),
        1 => "tilgang avsl\u{e5}tt (kCLErrorDenied)".to_string(),
        2 => "nettverksfeil (kCLErrorNetwork)".to_string(),
        other => format!("Core Location-feil {other}"),
    }
}

// A delegate is not optional, which is what the old polling loop got wrong.
//
// `CLLocationManager` only ever *pushes* fixes, through
// `locationManager:didUpdateLocations:`. Its `location` property is not a live
// reading — it is whatever the system already had cached. Polling that property
// alone therefore succeeds exactly when something else on the machine has used
// location services recently, and times out into the IP fallback otherwise.
//
// Plain comments rather than doc comments: rustdoc does not document macro
// invocations, and `///` here is a hard error under `-D warnings`.
#[cfg(target_os = "macos")]
define_class!(
    #[unsafe(super(NSObject))]
    #[name = "RuterLocationDelegate"]
    struct LocationDelegate;

    unsafe impl NSObjectProtocol for LocationDelegate {}

    unsafe impl CLLocationManagerDelegate for LocationDelegate {
        #[unsafe(method(locationManager:didUpdateLocations:))]
        fn did_update_locations(
            &self,
            _manager: &CLLocationManager,
            locations: &NSArray<CLLocation>,
        ) {
            let Some(location) = locations.lastObject() else { return };
            // SAFETY: reading the coordinate off a location Core Location just gave us.
            let c = unsafe { location.coordinate() };
            if let Ok(mut slot) = FIX.lock() {
                *slot = Some(Coord { lat: c.latitude, lon: c.longitude });
            }
        }

        #[unsafe(method(locationManager:didFailWithError:))]
        fn did_fail_with_error(&self, _manager: &CLLocationManager, error: &NSError) {
            FAIL_CODE.store(error.code() as i64, Ordering::Relaxed);
        }

        #[unsafe(method(locationManagerDidChangeAuthorization:))]
        fn did_change_authorization(&self, manager: &CLLocationManager) {
            // SAFETY: reading a property off the manager Core Location passed us.
            let status = unsafe { manager.authorizationStatus() };
            LAST_AUTH_STATUS.store(status.0, Ordering::Relaxed);
        }
    }
);

/// `Ok(None)` means "no fix within the timeout" rather than a hard failure.
#[cfg(target_os = "macos")]
fn gps() -> Result<Option<Coord>> {
    use std::time::{Duration, Instant};

    // A cold fix goes out to Wi-Fi positioning and routinely takes longer than
    // the four seconds this allowed before.
    const FIX_TIMEOUT: Duration = Duration::from_secs(10);
    // Time to read a dialog and click Allow. This clock is deliberately separate
    // from the one above: the seconds the user spends deciding must not eat into
    // the seconds Core Location gets to answer.
    const PROMPT_TIMEOUT: Duration = Duration::from_secs(60);

    unsafe {
        let manager = CLLocationManager::new();

        // Fail fast when permission has already been refused, rather than
        // making the user wait out the timeout on every single invocation.
        if matches!(
            manager.authorizationStatus(),
            CLAuthorizationStatus::Denied | CLAuthorizationStatus::Restricted
        ) {
            record_attempt("tilgang til stedstjenester er avsl\u{e5}tt for ruter");
            bail!("{}", permission_hint())
        }

        let delegate: Retained<LocationDelegate> = msg_send![LocationDelegate::alloc(), init];
        manager.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

        // 100 m is kCLLocationAccuracyHundredMeters. The default is
        // kCLLocationAccuracyBest, which keeps refining before it reports and so
        // takes far longer to first fix than choosing a nearby stop warrants.
        manager.setDesiredAccuracy(100.0);

        // Settle authorization *before* asking for a position.
        // `requestWhenInUseAuthorization` is asynchronous, so calling
        // `startUpdatingLocation` straight after it starts nothing at all while
        // the dialog is still on screen: the user clicks Allow, and the wait then
        // expires anyway on a machine that has just granted access.
        if matches!(manager.authorizationStatus(), CLAuthorizationStatus::NotDetermined) {
            PROMPTED.store(true, Ordering::Relaxed);
            manager.requestWhenInUseAuthorization();

            let asked = Instant::now();
            while asked.elapsed() < PROMPT_TIMEOUT
                && matches!(manager.authorizationStatus(), CLAuthorizationStatus::NotDetermined)
            {
                pump_run_loop(0.1);
            }
        }
        LAST_AUTH_STATUS.store(manager.authorizationStatus().0, Ordering::Relaxed);

        match manager.authorizationStatus() {
            CLAuthorizationStatus::Denied | CLAuthorizationStatus::Restricted => {
                record_attempt("du avslo tilgangsdialogen");
                bail!("{}", permission_hint())
            }
            CLAuthorizationStatus::NotDetermined => {
                record_attempt("tilgangsdialogen ble ikke besvart");
                bail!(
                    "ingen svar p\u{e5} tilgangsdialogen. Kj\u{f8}r kommandoen p\u{e5} nytt, \
                     eller bruk `--no-gps`."
                )
            }
            _ => {}
        }

        manager.startUpdatingLocation();

        let started = Instant::now();
        let mut found = None;
        let mut from_cache = false;
        while started.elapsed() < FIX_TIMEOUT {
            pump_run_loop(0.1);

            if let Ok(slot) = FIX.lock()
                && let Some(coord) = *slot
            {
                found = Some(coord);
                break;
            }

            // The cached property still short-circuits the wait when the system
            // happens to have a recent fix already.
            if let Some(location) = manager.location() {
                let c = location.coordinate();
                found = Some(Coord { lat: c.latitude, lon: c.longitude });
                from_cache = true;
                break;
            }

            // kCLErrorDenied is terminal; kCLErrorLocationUnknown is not, and
            // Apple's guidance is to keep waiting through it.
            if FAIL_CODE.load(Ordering::Relaxed) == 1 {
                break;
            }
        }
        manager.stopUpdatingLocation();
        manager.setDelegate(None);
        LAST_AUTH_STATUS.store(manager.authorizationStatus().0, Ordering::Relaxed);

        if found.is_some() {
            record_attempt(if from_cache {
                "fikk posisjon fra systemets mellomlagrede m\u{e5}ling"
            } else {
                "fikk posisjon fra Core Location"
            });
            return Ok(found);
        }

        // Report what Core Location actually said, rather than sending an
        // already-authorised user off to System Settings. That wrong advice is
        // what made this look like a permissions problem for so long.
        let reported = FAIL_CODE.load(Ordering::Relaxed);
        if reported != i64::MIN {
            let text = cl_error_text(reported);
            record_attempt(text.clone());
            bail!("{text}. Bruk `--no-gps` for \u{e5} hoppe over GPS.");
        }

        record_attempt(format!(
            "tidsavbrudd etter {} sekunder uten svar fra Core Location",
            FIX_TIMEOUT.as_secs()
        ));
        bail!(
            "Core Location svarte ikke innen {} sekunder. Se `ruter where` for hva som \
             kan blokkere. Bruk `--no-gps` for \u{e5} hoppe over GPS.",
            FIX_TIMEOUT.as_secs()
        )
    }
}

/// Explain how to get a Location Services grant.
///
/// macOS will not prompt for a process it cannot name. A bare binary has no
/// bundle identity, and the request then gets attributed to whichever process
/// launched it — if that process declares no location usage description, the
/// prompt is suppressed silently and Core Location just never returns anything.
/// `scripts/install-macos.sh` fixes this by installing into a small .app bundle.
#[cfg(target_os = "macos")]
fn permission_hint() -> String {
    "kj\u{f8}r `scripts/install-macos.sh` for \u{e5} installere ruter som app-bundle, \
     som er det som gir den egen tilgang til stedstjenester. \
     Se s\u{e5} etter «ruter» under Systeminnstillinger > Personvern og sikkerhet > \
     Stedstjenester. Bruk `--no-gps` for \u{e5} hoppe over GPS."
        .to_string()
}

/// Human-readable Core Location state, for `ruter where`.
///
/// Call this *after* attempting to resolve a position: the authorization status
/// stays `NotDetermined` until something actually asks, so reading it first
/// reports "not determined" even on a machine where GPS works fine.
#[cfg(target_os = "macos")]
pub fn gps_diagnostics() -> Vec<(String, String)> {
    let raw = match LAST_AUTH_STATUS.load(Ordering::Relaxed) {
        -1 => unsafe { CLLocationManager::new().authorizationStatus().0 },
        observed => observed,
    };
    let status_text = match CLAuthorizationStatus(raw) {
        CLAuthorizationStatus::NotDetermined => {
            "ikke avgjort \u{2014} macOS har ikke spurt enn\u{e5}"
        }
        CLAuthorizationStatus::Restricted => "begrenset av systemet",
        CLAuthorizationStatus::Denied => "avsl\u{e5}tt",
        CLAuthorizationStatus::AuthorizedAlways => "innvilget (alltid)",
        CLAuthorizationStatus::AuthorizedWhenInUse => "innvilget (ved bruk)",
        other => return vec![("Stedstjenester".into(), format!("ukjent status {}", other.0))],
    };

    // current_exe() can hand back the symlink that was invoked rather than the
    // real path inside the bundle, so resolve it before looking for the bundle
    // layout — otherwise a correctly installed ruter reports itself as unbundled.
    let bundled = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
        .unwrap_or(false);

    // Deprecated because it can block the calling thread. That is a real problem
    // on the trip path and not one here: `ruter where` exists to answer exactly
    // this question, and services being switched off machine-wide is the one
    // blocker no per-app status can reveal.
    #[allow(deprecated)]
    let services_on = unsafe { CLLocationManager::locationServicesEnabled_class() };

    let mut rows = vec![
        (
            "Stedstjenester på maskinen".to_string(),
            if services_on {
                "på".to_string()
            } else {
                "AV \u{2014} slå på under Systeminnstillinger > Personvern og sikkerhet > \
                 Stedstjenester. Ingenting annet her hjelper før den står på"
                    .to_string()
            },
        ),
        ("Tilgang for ruter".to_string(), status_text.to_string()),
        (
            "Kjører som app-bundle".to_string(),
            if bundled {
                "ja".to_string()
            } else {
                "nei \u{2014} kjør scripts/install-macos.sh, ellers spør ikke macOS om tilgang"
                    .to_string()
            },
        ),
    ];

    if let Ok(slot) = LAST_ATTEMPT.lock()
        && let Some(outcome) = slot.as_deref()
    {
        rows.push(("Siste GPS-forsøk".to_string(), outcome.to_string()));
    }

    if PROMPTED.load(Ordering::Relaxed) {
        rows.push((
            "Merk".to_string(),
            "macOS spurte om tilgang under dette fors\u{f8}ket. Er den nå innvilget, \
             finner neste kj\u{f8}ring posisjonen uten \u{e5} spørre."
                .to_string(),
        ));
    }

    rows
}

#[cfg(not(target_os = "macos"))]
pub fn gps_diagnostics() -> Vec<(String, String)> {
    vec![("Stedstjenester".to_string(), "st\u{f8}ttes bare på macOS".to_string())]
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
        assert!(parse_coord("Dronningens gate 40, Oslo").is_none());
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
