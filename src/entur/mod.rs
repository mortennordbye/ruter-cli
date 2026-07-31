//! Client for Entur's open APIs.
//!
//! Entur is the national journey planner for Norway; Ruter's Oslo/Akershus data
//! is served through it. The APIs are open under NLOD with no key and no
//! registration, but every request MUST carry an `ET-Client-Name` header —
//! unidentified clients are aggressively rate-limited.

pub mod geocode;
pub mod nearest;
pub mod trip;

use anyhow::{Result, anyhow, bail};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::time::Duration;

pub const JOURNEY_PLANNER_URL: &str = "https://api.entur.io/journey-planner/v3/graphql";
pub const GEOCODER_BASE: &str = "https://api.entur.io/geocoder/v1";

/// A WGS84 coordinate pair.
///
/// Note the field order: Entur's GraphQL API takes `latitude`/`longitude`
/// separately, but the geocoder returns GeoJSON `[lon, lat]`. Keeping a named
/// struct rather than a tuple is what stops that from silently swapping.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord {
    pub lat: f64,
    pub lon: f64,
}

pub struct Client {
    agent: ureq::Agent,
    client_name: String,
}

/// Standard GraphQL response envelope.
#[derive(Debug, Deserialize)]
struct GraphQlResponse<T> {
    data: Option<T>,
    #[serde(default)]
    errors: Vec<GraphQlError>,
}

#[derive(Debug, Deserialize)]
struct GraphQlError {
    message: String,
}

impl Client {
    pub fn new(client_name: &str) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(20)))
            .user_agent(concat!("ruter-cli/", env!("CARGO_PKG_VERSION")))
            // Entur reports GraphQL errors in the body with HTTP 200, but a
            // genuine 5xx also carries a useful body. ureq would turn that into
            // an Err before we ever see it, so read the body ourselves.
            .http_status_as_error(false)
            .build()
            .new_agent();
        Self { agent, client_name: client_name.to_string() }
    }

    /// POST a GraphQL query and deserialize the `data` field.
    pub fn graphql<T: DeserializeOwned>(&self, query: &str) -> Result<T> {
        let body = serde_json::json!({ "query": query });

        let mut response = self
            .agent
            .post(JOURNEY_PLANNER_URL)
            .header("ET-Client-Name", &self.client_name)
            .send_json(&body)
            .map_err(|e| anyhow!("nådde ikke Entur journey planner: {e}"))?;

        check_status(response.status().as_u16(), "Entur journey planner")?;

        let parsed: GraphQlResponse<T> =
            response.body_mut().read_json().context_msg("kunne ikke tolke svaret fra Entur")?;

        if !parsed.errors.is_empty() {
            let joined =
                parsed.errors.iter().map(|e| e.message.as_str()).collect::<Vec<_>>().join("; ");
            bail!("Entur svarte med feil: {joined}");
        }

        parsed.data.ok_or_else(|| anyhow!("Entur returnerte et tomt svar"))
    }

    /// GET a JSON document (used for the geocoder, which is REST, not GraphQL).
    pub fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let mut response = self
            .agent
            .get(url)
            .header("ET-Client-Name", &self.client_name)
            .call()
            .map_err(|e| anyhow!("nådde ikke Entur geocoder: {e}"))?;

        check_status(response.status().as_u16(), "Entur geocoder")?;

        response.body_mut().read_json().context_msg("kunne ikke tolke svaret fra Entur")
    }
}

/// Rate limiting is the failure mode most likely to be hit in the wild, so it
/// gets a hint the user can actually act on.
fn check_status(status: u16, what: &str) -> Result<()> {
    match status {
        200..=299 => Ok(()),
        429 => bail!(
            "{what} rate-limiter deg (HTTP 429). Sett en egen `client_name` i konfigurasjonen \
             og øk `watch_interval_secs`."
        ),
        code => bail!("{what} svarte HTTP {code}"),
    }
}

/// Small helper so the call sites read cleanly without importing `Context`
/// everywhere for errors that are not `anyhow::Error` already.
trait ContextMsg<T> {
    fn context_msg(self, msg: &'static str) -> Result<T>;
}

impl<T, E: std::fmt::Display> ContextMsg<T> for std::result::Result<T, E> {
    fn context_msg(self, msg: &'static str) -> Result<T> {
        self.map_err(|e| anyhow!("{msg}: {e}"))
    }
}

/// Render a mode list as a GraphQL `transportModes` argument.
pub fn transport_modes_arg(modes: &[String]) -> String {
    let inner =
        modes.iter().map(|m| format!("{{transportMode:{m}}}")).collect::<Vec<_>>().join(",");
    format!("[{inner}]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_transport_modes_argument() {
        let modes = vec!["bus".to_string(), "tram".to_string()];
        assert_eq!(transport_modes_arg(&modes), "[{transportMode:bus},{transportMode:tram}]");
    }

    #[test]
    fn empty_modes_produce_empty_list() {
        assert_eq!(transport_modes_arg(&[]), "[]");
    }
}
