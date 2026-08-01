//! Configuration file handling.
//!
//! Lives at `$XDG_CONFIG_HOME/ruter/config.toml`, falling back to
//! `~/.config/ruter/config.toml`. Deliberately *not* `dirs::config_dir()`, which
//! on macOS points at `~/Library/Application Support` — a CLI config belongs in
//! `~/.config` where the rest of the user's dotfiles live.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A saved location the user can refer to by name, e.g. `home`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Place {
    /// Human-readable name shown in output, e.g. "Hjemme".
    pub label: String,
    pub lat: f64,
    pub lon: f64,
}

/// A stop a route is required to pass through, in order.
///
/// Stored as the NSR id rather than the text the user typed: "Stubberud" matches
/// eight stops nationally and the Oslo one is not the first hit, so re-resolving
/// the name on every run would eventually route the user somewhere else entirely.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Waypoint {
    /// Human-readable name shown in output, e.g. "Smestad, Oslo".
    pub label: String,
    /// e.g. "NSR:StopPlace:58273".
    pub id: String,
}

/// A named journey with fixed endpoints and waypoints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Route {
    /// Shown by `ruter route list`, e.g. "Brekkelia → Stubberud".
    pub label: String,
    /// `None` means "start from wherever I am now".
    #[serde(default)]
    pub from: Option<Place>,
    pub to: Place,
    #[serde(default)]
    pub via: Vec<Waypoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Sent as the mandatory `ET-Client-Name` header on every Entur request.
    #[serde(default = "default_client_name")]
    pub client_name: String,

    /// Used when `ruter` is invoked with no destination.
    #[serde(default)]
    pub default_destination: Option<String>,

    /// Last-resort origin when every other position source fails.
    #[serde(default)]
    pub default_origin: Option<String>,

    #[serde(default = "default_num_results")]
    pub num_results: usize,

    /// Minutes. Caps the walk to the first stop and from the last one.
    ///
    /// Journey Planner v3 has no `maximumWalkDistance`; it constrains access
    /// and egress by *duration* via `maxAccessEgressDurationForMode`.
    #[serde(default = "default_max_walk_minutes")]
    pub max_walk_minutes: u32,

    #[serde(default = "default_modes")]
    pub modes: Vec<String>,

    /// Refresh interval for `--watch`. Kept generous: this is a free open API
    /// and hammering it is how you get rate-limited.
    #[serde(default = "default_watch_interval")]
    pub watch_interval_secs: u64,

    #[serde(default)]
    pub places: BTreeMap<String, Place>,

    #[serde(default)]
    pub routes: BTreeMap<String, Route>,
}

fn default_client_name() -> String {
    "ruter-cli".to_string()
}
fn default_num_results() -> usize {
    5
}
fn default_max_walk_minutes() -> u32 {
    15
}
fn default_watch_interval() -> u64 {
    30
}
fn default_modes() -> Vec<String> {
    ["bus", "tram", "metro", "rail", "water"].iter().map(|s| s.to_string()).collect()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            client_name: default_client_name(),
            default_destination: None,
            default_origin: None,
            num_results: default_num_results(),
            max_walk_minutes: default_max_walk_minutes(),
            modes: default_modes(),
            watch_interval_secs: default_watch_interval(),
            places: BTreeMap::new(),
            routes: BTreeMap::new(),
        }
    }
}

impl Config {
    /// Load the config, returning defaults if the file does not exist yet.
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("kunne ikke lese {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("ugyldig konfigurasjon i {}", path.display()))
    }

    pub fn save(&self) -> Result<PathBuf> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("kunne ikke opprette {}", parent.display()))?;
        }
        let text =
            toml::to_string_pretty(self).context("kunne ikke serialisere konfigurasjonen")?;
        std::fs::write(&path, text)
            .with_context(|| format!("kunne ikke skrive {}", path.display()))?;
        Ok(path)
    }

    /// Case-insensitive lookup so `ruter Home` and `ruter home` behave the same.
    pub fn place(&self, name: &str) -> Option<&Place> {
        self.places.get(name).or_else(|| {
            self.places.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v)
        })
    }

    pub fn route(&self, name: &str) -> Option<&Route> {
        self.routes.get(name).or_else(|| {
            self.routes.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v)
        })
    }
}

pub fn config_path() -> Result<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        return Ok(PathBuf::from(xdg).join("ruter").join("config.toml"));
    }
    let home = dirs::home_dir().context("fant ikke hjemmekatalogen")?;
    Ok(home.join(".config").join("ruter").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip_through_toml() {
        let cfg = Config::default();
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back.num_results, cfg.num_results);
        assert_eq!(back.modes, cfg.modes);
    }

    #[test]
    fn empty_config_uses_defaults() {
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg.num_results, 5);
        assert_eq!(cfg.client_name, "ruter-cli");
        assert!(cfg.places.is_empty());
    }

    #[test]
    fn place_lookup_is_case_insensitive() {
        let mut cfg = Config::default();
        cfg.places.insert("home".into(), Place { label: "Hjemme".into(), lat: 59.9, lon: 10.7 });
        assert!(cfg.place("home").is_some());
        assert!(cfg.place("HOME").is_some());
        assert!(cfg.place("Home").is_some());
        assert!(cfg.place("work").is_none());
    }

    #[test]
    fn routes_round_trip_through_toml() {
        let mut cfg = Config::default();
        cfg.routes.insert(
            "sorkedalen".into(),
            Route {
                label: "Brekkelia \u{2192} Stubberud".into(),
                from: Some(Place {
                    label: "Brekkelia 3D, Oslo".into(),
                    lat: 59.960913,
                    lon: 10.766685,
                }),
                to: Place { label: "Stubberud, Oslo".into(), lat: 60.013148, lon: 10.616123 },
                via: vec![
                    Waypoint { label: "Smestad, Oslo".into(), id: "NSR:StopPlace:58273".into() },
                    Waypoint { label: "R\u{f8}a, Oslo".into(), id: "NSR:StopPlace:59520".into() },
                ],
            },
        );

        let back: Config = toml::from_str(&toml::to_string_pretty(&cfg).unwrap()).unwrap();
        let route = back.route("sorkedalen").unwrap();
        // Waypoint order is the whole point of the feature.
        assert_eq!(
            route.via.iter().map(|w| w.id.as_str()).collect::<Vec<_>>(),
            ["NSR:StopPlace:58273", "NSR:StopPlace:59520"]
        );
        assert_eq!(route.from.as_ref().unwrap().label, "Brekkelia 3D, Oslo");
        assert_eq!(route.to.label, "Stubberud, Oslo");
    }

    #[test]
    fn a_route_without_a_start_parses() {
        // No `from` means "start wherever I am", which must survive a reload.
        let cfg: Config = toml::from_str(
            r#"
            [routes.jobb]
            label = "Til jobb"
            to = { label = "Jobben", lat = 59.91, lon = 10.75 }
            "#,
        )
        .unwrap();
        let route = cfg.route("JOBB").unwrap();
        assert!(route.from.is_none());
        assert!(route.via.is_empty());
    }

    #[test]
    fn places_parse_from_toml() {
        let cfg: Config = toml::from_str(
            r#"
            [places.home]
            label = "Hjemme"
            lat = 59.9430
            lon = 10.7180
            "#,
        )
        .unwrap();
        let home = cfg.place("home").unwrap();
        assert_eq!(home.label, "Hjemme");
        assert!((home.lat - 59.9430).abs() < 1e-9);
    }
}
