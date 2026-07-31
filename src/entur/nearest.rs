//! Nearby stop places and their upcoming departures.

use super::{Client, Coord};
use anyhow::Result;
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// nearest(...)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
struct NearestResponse {
    nearest: NearestConnection,
}

#[derive(Debug, Clone, Deserialize)]
struct NearestConnection {
    #[serde(default)]
    edges: Vec<Edge>,
}

#[derive(Debug, Clone, Deserialize)]
struct Edge {
    node: Node,
}

#[derive(Debug, Clone, Deserialize)]
struct Node {
    /// Metres from the query point.
    distance: f64,
    place: Option<PlaceNode>,
}

#[derive(Debug, Clone, Deserialize)]
struct PlaceNode {
    id: String,
    name: Option<String>,
    #[serde(rename = "transportMode", default)]
    transport_mode: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NearbyStop {
    pub id: String,
    pub name: String,
    pub distance_m: u32,
    pub modes: Vec<String>,
}

// ---------------------------------------------------------------------------
// stopPlace(...).estimatedCalls
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StopPlaceResponse {
    #[serde(rename = "stopPlace")]
    pub stop_place: Option<StopPlace>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StopPlace {
    pub id: String,
    pub name: Option<String>,
    #[serde(rename = "estimatedCalls", default)]
    pub estimated_calls: Vec<EstimatedCall>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EstimatedCall {
    #[serde(default)]
    pub realtime: bool,
    #[serde(rename = "aimedDepartureTime")]
    pub aimed_departure_time: DateTime<FixedOffset>,
    #[serde(rename = "expectedDepartureTime")]
    pub expected_departure_time: DateTime<FixedOffset>,
    #[serde(default)]
    pub cancellation: bool,
    #[serde(rename = "destinationDisplay")]
    pub destination_display: Option<DestinationDisplay>,
    pub quay: Option<super::trip::Quay>,
    #[serde(rename = "serviceJourney")]
    pub service_journey: Option<ServiceJourney>,
}

impl EstimatedCall {
    pub fn delay_minutes(&self) -> i64 {
        (self.expected_departure_time - self.aimed_departure_time).num_seconds() / 60
    }

    pub fn destination(&self) -> &str {
        self.destination_display.as_ref().and_then(|d| d.front_text.as_deref()).unwrap_or("—")
    }

    pub fn line(&self) -> Option<&DepartureLine> {
        self.service_journey.as_ref()?.line.as_ref()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DestinationDisplay {
    #[serde(rename = "frontText")]
    pub front_text: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceJourney {
    pub line: Option<DepartureLine>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DepartureLine {
    #[serde(rename = "publicCode")]
    pub public_code: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "transportMode")]
    pub transport_mode: Option<String>,
    pub presentation: Option<super::trip::Presentation>,
    /// The operator, e.g. "Ruter" or "Vy".
    pub authority: Option<Authority>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Authority {
    pub name: Option<String>,
}

impl Client {
    pub fn nearest_stops(
        &self,
        at: Coord,
        max_distance_m: u32,
        limit: usize,
    ) -> Result<Vec<NearbyStop>> {
        let query = format!(
            r#"{{
  nearest(
    latitude: {lat}
    longitude: {lon}
    maximumDistance: {max_distance_m}
    maximumResults: {limit}
    filterByPlaceTypes: [stopPlace]
  ) {{
    edges {{ node {{ distance place {{ ... on StopPlace {{ id name transportMode }} }} }} }}
  }}
}}"#,
            lat = at.lat,
            lon = at.lon,
        );
        let response: NearestResponse = self.graphql(&query)?;
        Ok(collect_stops(response))
    }

    pub fn departures(&self, stop_id: &str, count: usize) -> Result<Option<StopPlace>> {
        let query = format!(
            r#"{{
  stopPlace(id: "{stop_id}") {{
    id
    name
    estimatedCalls(numberOfDepartures: {count}, timeRange: 7200) {{
      realtime
      aimedDepartureTime
      expectedDepartureTime
      cancellation
      destinationDisplay {{ frontText }}
      quay {{ publicCode }}
      serviceJourney {{
        line {{
          publicCode
          name
          transportMode
          presentation {{ colour textColour }}
          authority {{ name }}
        }}
      }}
    }}
  }}
}}"#
        );
        let response: StopPlaceResponse = self.graphql(&query)?;
        Ok(response.stop_place)
    }
}

fn collect_stops(response: NearestResponse) -> Vec<NearbyStop> {
    response
        .nearest
        .edges
        .into_iter()
        .filter_map(|e| {
            let place = e.node.place?;
            Some(NearbyStop {
                id: place.id,
                name: place.name.unwrap_or_else(|| "Ukjent stopp".to_string()),
                distance_m: e.node.distance.round().max(0.0) as u32,
                modes: place.transport_mode,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_recorded_nearest_response() {
        let json = include_str!("../../tests/fixtures/nearest.json");
        let resp: NearestResponse = serde_json::from_str(json).unwrap();
        let stops = collect_stops(resp);
        assert_eq!(stops[0].name, "Storgata");
        assert_eq!(stops[0].id, "NSR:StopPlace:61444");
        // 43.517... metres rounds to 44.
        assert_eq!(stops[0].distance_m, 44);
        assert_eq!(stops[0].modes, vec!["bus", "tram"]);
        // Results arrive sorted by distance.
        assert!(stops[1].distance_m > stops[0].distance_m);
    }

    #[test]
    fn skips_edges_without_a_place() {
        let json = r#"{"nearest":{"edges":[{"node":{"distance":10.0,"place":null}}]}}"#;
        let resp: NearestResponse = serde_json::from_str(json).unwrap();
        assert!(collect_stops(resp).is_empty());
    }

    #[test]
    fn deserializes_recorded_departures() {
        let json = include_str!("../../tests/fixtures/departures.json");
        let resp: StopPlaceResponse = serde_json::from_str(json).unwrap();
        let stop = resp.stop_place.unwrap();
        assert_eq!(stop.name.as_deref(), Some("Jernbanetorget"));
        let first = &stop.estimated_calls[0];
        assert_eq!(first.destination(), "Storo-Grefsen st.");
        assert_eq!(first.line().unwrap().public_code.as_deref(), Some("18"));
        assert_eq!(first.line().unwrap().transport_mode.as_deref(), Some("tram"));
        assert_eq!(
            first.line().unwrap().authority.as_ref().unwrap().name.as_deref(),
            Some("Ruter")
        );
        assert_eq!(first.quay.as_ref().unwrap().public_code.as_deref(), Some("E"));
        assert!(!first.cancellation);
        assert!(first.realtime);
    }

    #[test]
    fn computes_departure_delay() {
        let json = include_str!("../../tests/fixtures/departures.json");
        let resp: StopPlaceResponse = serde_json::from_str(json).unwrap();
        let calls = resp.stop_place.unwrap().estimated_calls;
        // Recorded: aimed 13:57:00, expected 14:00:17 -> 3 min late.
        assert_eq!(calls[0].delay_minutes(), 3);
        // Recorded: aimed 13:52:00, expected 14:00:57 -> 8 min late.
        assert_eq!(calls[3].delay_minutes(), 8);
        // Recorded: aimed 14:01:00, expected 14:01:00 -> on time.
        assert_eq!(calls[4].delay_minutes(), 0);
    }

    #[test]
    fn departures_are_not_sorted_by_aimed_time() {
        // Entur orders by *expected* departure, so a heavily delayed service is
        // interleaved by when it will actually leave. Anything that assumes
        // `aimed` is monotonic will render the board in the wrong order.
        let json = include_str!("../../tests/fixtures/departures.json");
        let resp: StopPlaceResponse = serde_json::from_str(json).unwrap();
        let calls = resp.stop_place.unwrap().estimated_calls;

        let expected_sorted =
            calls.windows(2).all(|w| w[0].expected_departure_time <= w[1].expected_departure_time);
        assert!(expected_sorted, "expected departure times should be ascending");

        let aimed_sorted =
            calls.windows(2).all(|w| w[0].aimed_departure_time <= w[1].aimed_departure_time);
        assert!(
            !aimed_sorted,
            "this fixture is the interesting case: aimed times are out of order"
        );
    }

    #[test]
    fn missing_destination_display_falls_back() {
        let call: EstimatedCall = serde_json::from_str(
            r#"{"aimedDepartureTime":"2026-07-31T13:53:00+02:00",
                "expectedDepartureTime":"2026-07-31T13:53:00+02:00"}"#,
        )
        .unwrap();
        assert_eq!(call.destination(), "—");
        assert!(call.line().is_none());
    }
}
