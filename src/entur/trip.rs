//! The `trip` query: journey planning between two coordinates.

use super::{Client, Coord, transport_modes_arg};
use anyhow::Result;
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TripResponse {
    pub trip: Trip,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Trip {
    #[serde(rename = "tripPatterns", default)]
    pub trip_patterns: Vec<TripPattern>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TripPattern {
    #[serde(rename = "expectedStartTime")]
    pub expected_start_time: DateTime<FixedOffset>,
    #[serde(rename = "expectedEndTime")]
    pub expected_end_time: DateTime<FixedOffset>,
    /// Total journey time in seconds.
    pub duration: i64,
    #[serde(rename = "walkDistance")]
    pub walk_distance: f64,
    #[serde(default)]
    pub legs: Vec<Leg>,
}

impl TripPattern {
    /// The legs that actually involve a vehicle, in order.
    pub fn transit_legs(&self) -> impl Iterator<Item = &Leg> {
        self.legs.iter().filter(|l| l.is_transit())
    }

    /// Where you first board something. `None` for an all-walking trip.
    pub fn first_boarding(&self) -> Option<&Leg> {
        self.transit_legs().next()
    }

    /// True if any leg carries live data, which decides the `●` / `○` marker.
    pub fn has_realtime(&self) -> bool {
        self.legs.iter().any(|l| l.realtime)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Leg {
    /// `foot`, `bus`, `tram`, `metro`, `rail`, `water`, ...
    pub mode: String,
    pub distance: f64,
    /// Leg duration in seconds.
    pub duration: i64,
    #[serde(rename = "aimedStartTime")]
    pub aimed_start_time: DateTime<FixedOffset>,
    #[serde(rename = "expectedStartTime")]
    pub expected_start_time: DateTime<FixedOffset>,
    #[serde(rename = "expectedEndTime")]
    pub expected_end_time: DateTime<FixedOffset>,
    #[serde(default)]
    pub realtime: bool,
    #[serde(rename = "fromPlace")]
    pub from_place: Place,
    #[serde(rename = "toPlace")]
    pub to_place: Place,
    /// `None` for walking legs.
    pub line: Option<Line>,
    #[serde(default)]
    pub situations: Vec<Situation>,
}

impl Leg {
    pub fn is_transit(&self) -> bool {
        self.mode != "foot" && self.mode != "bicycle"
    }

    /// How many whole minutes late this leg departs. Negative means early.
    pub fn delay_minutes(&self) -> i64 {
        (self.expected_start_time - self.aimed_start_time).num_seconds() / 60
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Place {
    pub name: Option<String>,
    pub quay: Option<Quay>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Quay {
    /// The platform number as signposted, e.g. "2" or "L".
    #[serde(rename = "publicCode")]
    pub public_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Line {
    /// The number on the front of the vehicle, e.g. "21".
    #[serde(rename = "publicCode")]
    pub public_code: Option<String>,
    pub name: Option<String>,
    pub presentation: Option<Presentation>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Presentation {
    /// Hex RGB with no leading `#`, e.g. "EC700C".
    pub colour: Option<String>,
    #[serde(rename = "textColour")]
    pub text_colour: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Situation {
    pub summary: Vec<TranslatedString>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TranslatedString {
    pub value: String,
}

/// The tunable parts of a trip search.
///
/// Kept together because both the one-shot path and the watch poller need the
/// identical set, and threading them positionally through two layers is how they
/// end up out of step.
#[derive(Debug, Clone)]
pub struct TripQuery {
    /// NSR stop ids the journey must pass through, in order. Empty means direct.
    pub via: Vec<String>,
    pub num_patterns: usize,
    pub max_walk_minutes: u32,
    pub modes: Vec<String>,
}

impl Client {
    pub fn trip(&self, from: Coord, to: Coord, query: &TripQuery) -> Result<Vec<TripPattern>> {
        let body = build_trip_query(from, to, query);
        let response: TripResponse = self.graphql(&body)?;
        Ok(response.trip.trip_patterns)
    }
}

/// Render the ordered waypoints as a `via` argument, or nothing at all when there
/// are none — Journey Planner rejects an empty `via` list.
fn via_arg(via: &[String]) -> String {
    if via.is_empty() {
        return String::new();
    }
    let inner = via
        .iter()
        .map(|id| format!(r#"{{visit: {{stopLocationIds: ["{id}"]}}}}"#))
        .collect::<Vec<_>>()
        .join(", ");
    format!("\n    via: [{inner}]")
}

fn build_trip_query(from: Coord, to: Coord, query: &TripQuery) -> String {
    format!(
        r#"{{
  trip(
    from: {{coordinates: {{latitude: {from_lat}, longitude: {from_lon}}}}}
    to: {{coordinates: {{latitude: {to_lat}, longitude: {to_lon}}}}}{via}
    numTripPatterns: {num_patterns}
    maxAccessEgressDurationForMode: [{{streetMode: foot, duration: "PT{max_walk_minutes}M"}}]
    modes: {{accessMode: foot, egressMode: foot, transportModes: {modes}}}
  ) {{
    tripPatterns {{
      expectedStartTime
      expectedEndTime
      duration
      walkDistance
      legs {{
        mode
        distance
        duration
        aimedStartTime
        expectedStartTime
        expectedEndTime
        realtime
        fromPlace {{ name quay {{ publicCode }} }}
        toPlace {{ name quay {{ publicCode }} }}
        line {{ publicCode name presentation {{ colour textColour }} }}
        situations {{ summary {{ value }} }}
      }}
    }}
  }}
}}"#,
        from_lat = from.lat,
        from_lon = from.lon,
        to_lat = to.lat,
        to_lon = to.lon,
        via = via_arg(&query.via),
        num_patterns = query.num_patterns,
        max_walk_minutes = query.max_walk_minutes,
        modes = transport_modes_arg(&query.modes),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> TripResponse {
        let json = include_str!("../../tests/fixtures/trip.json");
        serde_json::from_str(json).expect("fixture should deserialize")
    }

    #[test]
    fn deserializes_recorded_trip_response() {
        let resp = fixture();
        assert_eq!(resp.trip.trip_patterns.len(), 2);
        // Pattern 0 is walk -> tram -> walk, pattern 1 is walk -> metro -> walk.
        assert_eq!(resp.trip.trip_patterns[0].duration, 1824);
        assert_eq!(resp.trip.trip_patterns[1].duration, 1611);
        assert!(resp.trip.trip_patterns.iter().all(|p| p.legs.len() == 3));
    }

    #[test]
    fn walking_legs_have_no_line() {
        let resp = fixture();
        let first = &resp.trip.trip_patterns[0];
        assert!(first.legs[0].line.is_none(), "walk leg should have no line");
        assert!(!first.legs[0].is_transit());
        assert!(first.legs[1].line.is_some(), "tram leg should have a line");
        assert!(first.legs[1].is_transit());
        assert!(!first.legs[2].is_transit());
    }

    #[test]
    fn extracts_line_presentation_colour() {
        let resp = fixture();
        let tram = &resp.trip.trip_patterns[0].legs[1];
        let presentation = tram.line.as_ref().unwrap().presentation.as_ref().unwrap();
        assert_eq!(presentation.colour.as_deref(), Some("0B91EF"));
        assert_eq!(presentation.text_colour.as_deref(), Some("FFFFFF"));

        let metro = &resp.trip.trip_patterns[1].legs[1];
        let presentation = metro.line.as_ref().unwrap().presentation.as_ref().unwrap();
        assert_eq!(presentation.colour.as_deref(), Some("EC700C"));
    }

    #[test]
    fn first_boarding_skips_the_walk_to_the_stop() {
        let resp = fixture();
        let boarding = resp.trip.trip_patterns[1].first_boarding().unwrap();
        assert_eq!(boarding.mode, "metro");
        assert_eq!(boarding.line.as_ref().unwrap().public_code.as_deref(), Some("1"));
        assert_eq!(boarding.from_place.name.as_deref(), Some("Jernbanetorget"));
        assert_eq!(boarding.from_place.quay.as_ref().unwrap().public_code.as_deref(), Some("2"));
    }

    #[test]
    fn counts_only_transit_legs() {
        let resp = fixture();
        // Three legs, but only one of them boards anything.
        assert_eq!(resp.trip.trip_patterns[0].transit_legs().count(), 1);
    }

    #[test]
    fn computes_delay_from_aimed_versus_expected() {
        let resp = fixture();
        // Recorded tram leg: aimed 14:09:00, expected 14:10:45 -> 1 min late.
        assert_eq!(resp.trip.trip_patterns[0].legs[1].delay_minutes(), 1);
        // Recorded metro leg: aimed 14:07:00, expected 14:20:19 -> 13 min late.
        assert_eq!(resp.trip.trip_patterns[1].legs[1].delay_minutes(), 13);
        // Walk legs have identical aimed and expected times.
        assert_eq!(resp.trip.trip_patterns[0].legs[0].delay_minutes(), 0);
    }

    #[test]
    fn detects_realtime_patterns() {
        let resp = fixture();
        assert!(resp.trip.trip_patterns[0].has_realtime());
    }

    fn query(via: &[&str]) -> TripQuery {
        TripQuery {
            via: via.iter().map(|s| s.to_string()).collect(),
            num_patterns: 3,
            max_walk_minutes: 12,
            modes: vec!["bus".to_string()],
        }
    }

    #[test]
    fn query_embeds_coordinates_and_modes() {
        let q = build_trip_query(
            Coord { lat: 59.9139, lon: 10.7522 },
            Coord { lat: 59.9430, lon: 10.7180 },
            &query(&[]),
        );
        assert!(q.contains("latitude: 59.9139"));
        assert!(q.contains("longitude: 10.718"));
        assert!(q.contains("numTripPatterns: 3"));
        assert!(q.contains(r#"duration: "PT12M""#));
        assert!(q.contains("[{transportMode:bus}]"));
    }

    #[test]
    fn no_waypoints_means_no_via_argument() {
        // An empty `via: []` is rejected by Journey Planner, so it must be absent.
        assert_eq!(via_arg(&[]), "");
        let q = build_trip_query(
            Coord { lat: 59.9, lon: 10.7 },
            Coord { lat: 59.9, lon: 10.7 },
            &query(&[]),
        );
        assert!(!q.contains("via"));
    }

    #[test]
    fn waypoints_are_listed_in_order() {
        let q = build_trip_query(
            Coord { lat: 59.9, lon: 10.7 },
            Coord { lat: 60.0, lon: 10.6 },
            &query(&["NSR:StopPlace:58273", "NSR:StopPlace:59520"]),
        );
        assert!(q.contains(r#"{visit: {stopLocationIds: ["NSR:StopPlace:58273"]}}"#));
        assert!(q.contains(r#"{visit: {stopLocationIds: ["NSR:StopPlace:59520"]}}"#));
        // Order is the routing constraint, so Smestad must precede Røa.
        assert!(q.find("58273").unwrap() < q.find("59520").unwrap());
    }
}
