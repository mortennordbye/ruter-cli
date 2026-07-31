//! Entur geocoder: free-text address -> coordinates, and the reverse.
//!
//! Responses are GeoJSON, so coordinates arrive as `[lon, lat]`.

use super::{Client, Coord, GEOCODER_BASE};
use anyhow::{Result, bail};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub struct GeoMatch {
    pub label: String,
    pub coord: Coord,
    /// e.g. "address", "venue", "stop place" — helps the user pick.
    pub layer: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FeatureCollection {
    #[serde(default)]
    features: Vec<Feature>,
}

#[derive(Debug, Deserialize)]
struct Feature {
    geometry: Geometry,
    properties: Properties,
}

#[derive(Debug, Deserialize)]
struct Geometry {
    /// GeoJSON order: [longitude, latitude].
    coordinates: [f64; 2],
}

#[derive(Debug, Deserialize)]
struct Properties {
    label: Option<String>,
    name: Option<String>,
    layer: Option<String>,
}

impl FeatureCollection {
    fn into_matches(self) -> Vec<GeoMatch> {
        self.features
            .into_iter()
            .filter_map(|f| {
                let label = f.properties.label.or(f.properties.name)?;
                Some(GeoMatch {
                    label,
                    coord: Coord { lat: f.geometry.coordinates[1], lon: f.geometry.coordinates[0] },
                    layer: f.properties.layer,
                })
            })
            .collect()
    }
}

impl Client {
    /// Resolve a free-text query such as "Ullevålsveien 15, Oslo".
    pub fn geocode(&self, query: &str, size: usize) -> Result<Vec<GeoMatch>> {
        let url =
            format!("{GEOCODER_BASE}/autocomplete?text={}&size={size}&lang=no", urlencode(query));
        let fc: FeatureCollection = self.get_json(&url)?;
        let matches = fc.into_matches();
        if matches.is_empty() {
            bail!("fant ingen treff for \"{query}\"");
        }
        Ok(matches)
    }

    /// Best-effort human-readable name for a coordinate, used to label the
    /// origin when it came from GPS rather than a named place.
    pub fn reverse_geocode(&self, coord: Coord) -> Result<Option<String>> {
        let url = format!(
            "{GEOCODER_BASE}/reverse?point.lat={}&point.lon={}&size=1&lang=no",
            coord.lat, coord.lon
        );
        let fc: FeatureCollection = self.get_json(&url)?;
        Ok(fc.into_matches().into_iter().next().map(|m| m.label))
    }
}

/// Minimal percent-encoding for query strings. Pulling in a whole URL crate for
/// one query parameter is not worth it.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            b' ' => out.push_str("%20"),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_spaces_and_norwegian_characters() {
        assert_eq!(urlencode("Oslo S"), "Oslo%20S");
        // Precomposed "å" (U+00E5) is two UTF-8 bytes, both percent-encoded.
        assert_eq!(urlencode("\u{00e5}"), "%C3%A5");
        assert_eq!(urlencode("abc-123_x.y~z"), "abc-123_x.y~z");
    }

    #[test]
    fn parses_geojson_lon_lat_order() {
        // Recorded from the live geocoder for "Ullevålsveien 15, Oslo".
        let json = r#"{
            "features": [{
                "geometry": {"type": "Point", "coordinates": [10.742684, 59.920331]},
                "properties": {"label": "Ullevålsveien 15, Oslo", "layer": "address"}
            }]
        }"#;
        let fc: FeatureCollection = serde_json::from_str(json).unwrap();
        let matches = fc.into_matches();
        assert_eq!(matches.len(), 1);
        // The whole point: latitude is the SECOND element in GeoJSON.
        assert!((matches[0].coord.lat - 59.920331).abs() < 1e-9);
        assert!((matches[0].coord.lon - 10.742684).abs() < 1e-9);
        assert_eq!(matches[0].label, "Ullevålsveien 15, Oslo");
    }

    #[test]
    fn falls_back_to_name_when_label_missing() {
        let json = r#"{"features":[{"geometry":{"coordinates":[10.0,59.0]},
            "properties":{"name":"Storgata"}}]}"#;
        let fc: FeatureCollection = serde_json::from_str(json).unwrap();
        assert_eq!(fc.into_matches()[0].label, "Storgata");
    }

    #[test]
    fn empty_collection_yields_no_matches() {
        let fc: FeatureCollection = serde_json::from_str(r#"{"features":[]}"#).unwrap();
        assert!(fc.into_matches().is_empty());
    }
}
