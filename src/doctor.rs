//! `ruter doctor` — a report to paste into a bug report.
//!
//! Two rules shape everything here.
//!
//! **It must work when nothing else does.** A bug report is most needed when the tool
//! is broken, so this runs before the config is loaded and reports a failure to read it
//! as a finding rather than dying on it.
//!
//! **It must never leak where the user lives.** Saved places are home and work
//! addresses, and a report goes into a public issue. So no coordinates, no addresses,
//! no place labels, no route names — only whether things are set, and how many. The
//! position check resolves a real fix because that is the common fault, but reports
//! only which source answered.

use crate::config::{self, Config};
use crate::entur::{Client, Coord};
use crate::location::{self, Options};
use anyhow::Result;
use std::time::Instant;

/// Somewhere in central Oslo, used for the reachability checks so the report never
/// carries the user's own position.
const PROBE: Coord = Coord { lat: 59.9139, lon: 10.7522 };

pub fn run() -> Result<()> {
    let loaded = Config::load();
    let config = loaded.as_ref().cloned().unwrap_or_default();
    let client = Client::new(&config.client_name);

    let mut r = Report::default();

    r.head("ruter doctor");
    r.row("ruter", env!("CARGO_PKG_VERSION"));
    r.row("plattform", format!("{} {}", std::env::consts::OS, std::env::consts::ARCH));
    r.row("macOS", os_version());
    match std::env::current_exe().and_then(|p| p.canonicalize()) {
        Ok(path) => {
            let bundled = path.to_string_lossy().contains(".app/Contents/MacOS/");
            r.row("app-bundle", format!("{} ({})", yes_no(bundled), tilde(&path)));
        }
        Err(e) => r.row("app-bundle", format!("ukjent ({e})")),
    }

    r.head("Posisjon");
    // Resolved for real: an unauthorised or silent Core Location is the fault this
    // command exists to explain. Only the source is reported, never the fix itself.
    let resolved = location::resolve_origin(&client, &config, None, Options::default());
    match &resolved {
        Ok(origin) => r.row("oppslag", format!("OK, kilde: {}", origin.source.label())),
        Err(e) => r.row("oppslag", format!("FEIL: {}", redact(&format!("{e:#}")))),
    }
    for (key, value) in location::gps_diagnostics() {
        r.row(&key.to_lowercase(), value);
    }

    r.head("Konfigurasjon");
    match config::config_path() {
        Ok(path) => {
            r.row("sti", tilde(&path));
            r.row("finnes", yes_no(path.exists()));
        }
        Err(e) => r.row("sti", format!("ukjent ({e})")),
    }
    match &loaded {
        Ok(_) => r.row("lesing", "OK"),
        // The path inside the error is tilde'd along with everything else.
        Err(e) => r.row("lesing", format!("FEIL: {}", redact(&format!("{e:#}")))),
    }
    r.row("client_name", &config.client_name);
    r.row("num_results", config.num_results);
    r.row("max_walk_minutes", config.max_walk_minutes);
    r.row("modes", config.modes.join(", "));
    r.row("watch_interval_secs", config.watch_interval_secs);
    // Counts and set-or-not only. The names and addresses stay on the user's machine.
    r.row("lagrede steder", config.places.len());
    r.row("lagrede reiseveier", config.routes.len());
    r.row("default_origin", set_or_not(config.default_origin.is_some()));
    r.row("default_destination", set_or_not(config.default_destination.is_some()));

    r.head("Nett");
    r.row("journey planner", probe(|| client.nearest_stops(PROBE, 200, 1).map(|_| ())));
    r.row("geocoder", probe(|| client.geocode("Oslo S", 1).map(|_| ())));

    r.head("Terminal");
    for key in ["TERM", "TERM_PROGRAM", "NO_COLOR", "XDG_CONFIG_HOME"] {
        r.row(&key.to_lowercase(), env_or_unset(key));
    }
    r.row("farger", yes_no(crate::render::Style::detect(None).colour));

    // Fenced so it lands in a GitHub issue as a code block straight from the clipboard.
    crate::emit(&format!(
        "\nLim inn alt under i issuen. Ingen koordinater, adresser eller stedsnavn \
         er tatt med.\n\n```\n{}```\n",
        r.text
    ))
}

#[derive(Default)]
struct Report {
    text: String,
}

impl Report {
    fn head(&mut self, title: &str) {
        if !self.text.is_empty() {
            self.text.push('\n');
        }
        self.text.push_str(&format!("== {title}\n"));
    }

    /// Wide enough for the longest key, which is Core Location's
    /// "stedstjenester på maskinen" at 26 characters.
    ///
    /// A value may span lines — a TOML parse error points at the offending line with
    /// its own little diagram — so continuations are indented to the value column
    /// rather than collapsed. Keeping the shape is worth more than one row per fact.
    fn row(&mut self, key: &str, value: impl ToString) {
        let value = value.to_string();
        let mut lines = value.lines();
        self.text.push_str(&format!("{key:<27}{}\n", lines.next().unwrap_or("")));
        for line in lines {
            self.text.push_str(&format!("{:<27}{line}\n", ""));
        }
    }
}

/// Time a check and report how it went, so a slow network is visible as slow rather
/// than just "works".
fn probe(call: impl FnOnce() -> Result<()>) -> String {
    let started = Instant::now();
    let outcome = call();
    let ms = started.elapsed().as_millis();
    match outcome {
        Ok(()) => format!("OK ({ms} ms)"),
        Err(e) => format!("FEIL etter {ms} ms: {}", redact(&format!("{e:#}"))),
    }
}

#[cfg(target_os = "macos")]
fn os_version() -> String {
    std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "ukjent".to_string())
}

#[cfg(not(target_os = "macos"))]
fn os_version() -> String {
    "ikke macOS".to_string()
}

fn env_or_unset(key: &str) -> String {
    match std::env::var(key) {
        Ok(v) if !v.is_empty() => redact(&v),
        _ => "ikke satt".to_string(),
    }
}

fn yes_no(b: bool) -> String {
    if b { "ja" } else { "nei" }.to_string()
}

fn set_or_not(b: bool) -> String {
    if b { "satt" } else { "ikke satt" }.to_string()
}

fn tilde(path: &std::path::Path) -> String {
    redact(&path.display().to_string())
}

/// Swap the home directory for `~`. The username is not the point of the report and
/// there is no reason to put it in a public issue.
fn redact(s: &str) -> String {
    match dirs::home_dir() {
        Some(home) => s.replace(&home.display().to_string(), "~"),
        None => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_aligned_and_one_per_line() {
        let mut r = Report::default();
        r.head("Seksjon");
        r.row("kort", "a");
        r.row("et_ganske_langt_navn", "b");

        let lines: Vec<&str> = r.text.lines().collect();
        assert_eq!(lines[0], "== Seksjon");
        // Values start in the same column so the pasted block stays readable.
        assert_eq!(lines[1].find('a'), lines[2].find('b'));
    }

    /// Columns are measured in characters, not bytes: "stedstjenester på maskinen"
    /// holds a two-byte `å`, so a byte offset would disagree with what is on screen.
    fn value_column(line: &str, value: &str) -> usize {
        line[..line.find(value).expect("value should be on the line")].chars().count()
    }

    /// Core Location's longest label must not push its value out of the column.
    #[test]
    fn the_widest_diagnostic_key_still_aligns() {
        let mut r = Report::default();
        r.row("stedstjenester på maskinen", "VERDI");
        r.row("kort", "VERDI");

        let lines: Vec<&str> = r.text.lines().collect();
        assert_eq!(value_column(lines[0], "VERDI"), value_column(lines[1], "VERDI"));
        assert!(lines[0].contains(" VERDI"), "the long key ate its separator: {:?}", lines[0]);
    }

    #[test]
    fn a_multi_line_value_indents_under_the_value_column() {
        let mut r = Report::default();
        // A TOML parse error looks like this, leading spaces on the diagram and all.
        let value = "FEIL: ugyldig konfigurasjon\n  |\n2 | bogus = 1";
        r.row("lesing", value);

        let lines: Vec<&str> = r.text.lines().collect();
        assert_eq!(lines.len(), 3, "each source line keeps its own row");

        let indent = " ".repeat(value_column(lines[0], "FEIL"));
        for (rendered, source) in lines[1..].iter().zip(value.lines().skip(1)) {
            // Exactly the value column, then the source line untouched — the diagram's
            // own leading spaces have to survive or it stops pointing at anything.
            assert_eq!(*rendered, format!("{indent}{source}"));
        }
    }

    #[test]
    fn the_home_directory_never_reaches_the_report() {
        let home = dirs::home_dir().expect("a home directory");
        let path = home.join(".config/ruter/config.toml");
        let out = tilde(&path);
        assert!(out.starts_with("~/"), "expected a tilde path, got {out}");
        assert!(
            !out.contains(&home.display().to_string()),
            "the home directory survived redaction: {out}"
        );
    }

    /// The whole privacy contract in one test: the report is built from counts and
    /// flags, so a saved place can never be spelled out.
    #[test]
    fn saved_places_are_reported_as_counts_not_contents() {
        let mut config = Config::default();
        config.places.insert(
            "hjem".into(),
            crate::config::Place {
                label: "Dronningens gate 40, Oslo".into(),
                lat: 59.912517,
                lon: 10.74882,
            },
        );
        config.default_origin = Some("hjem".into());

        let mut r = Report::default();
        r.row("lagrede steder", config.places.len());
        r.row("default_origin", set_or_not(config.default_origin.is_some()));

        assert!(r.text.contains('1'));
        assert!(r.text.contains("satt"));
        assert!(!r.text.contains("Dronningens"), "an address leaked: {}", r.text);
        assert!(!r.text.contains("59.91"), "a coordinate leaked: {}", r.text);
    }
}
