//! The one-shot boards printed to stdout.

use super::{Rgb, Style, delay_marker, duration_human, hhmm, mode_label, place_name, relative};
use crate::entur::nearest::{EstimatedCall, NearbyStop, StopPlace};
use crate::entur::trip::{Leg, TripPattern};
use crate::location::Origin;
use chrono::{DateTime, FixedOffset};

const REALTIME: &str = "\u{25cf}"; // ●
const SCHEDULED: &str = "\u{25cb}"; // ○

/// Header shown above both boards.
pub fn header(origin: &Origin, destination: &str, now: DateTime<FixedOffset>, s: Style) -> String {
    let mut out = format!(
        "\n  {}  \u{2192}  {}{}\n",
        s.bold(&origin.name),
        s.bold(destination),
        s.dim(&format!("   {}", hhmm(now)))
    );
    if origin.source.is_coarse() {
        out.push_str(&format!(
            "  {}\n",
            s.yellow(&format!(
                "\u{26a0} posisjon fra {} \u{2014} kan bomme med flere kilometer",
                origin.source.label()
            ))
        ));
    }
    out
}

pub fn trip_board(
    patterns: &[TripPattern],
    origin: &Origin,
    destination: &str,
    now: DateTime<FixedOffset>,
    s: Style,
) -> String {
    let mut out = header(origin, destination, now, s);

    if patterns.is_empty() {
        out.push_str(&format!("\n  {}\n\n", s.dim("Fant ingen reiser.")));
        return out;
    }

    for p in patterns {
        out.push('\n');
        let marker = if p.has_realtime() { s.green(REALTIME) } else { s.dim(SCHEDULED) };
        let transfers = p.transit_legs().count().saturating_sub(1);
        let transfer_text = match transfers {
            0 => "direkte".to_string(),
            1 => "1 bytte".to_string(),
            n => format!("{n} bytter"),
        };

        out.push_str(&format!(
            "  {marker} {:<12} {} \u{2192} {}   {:<8} {}\n",
            s.bold(&relative(p.expected_start_time, now)),
            hhmm(p.expected_start_time),
            hhmm(p.expected_end_time),
            duration_human(p.duration),
            s.dim(&transfer_text),
        ));

        for leg in &p.legs {
            out.push_str(&leg_line(leg, &origin.name, destination, s));
        }

        for situation in p.legs.iter().flat_map(|l| &l.situations) {
            if let Some(first) = situation.summary.first() {
                out.push_str(&format!(
                    "      {}\n",
                    s.yellow(&format!("\u{26a0} {}", first.value))
                ));
            }
        }
    }

    out.push_str(&format!(
        "\n  {}\n\n",
        s.dim(&format!("posisjon: {} \u{00b7} sanntid fra Entur", origin.source.label()))
    ));
    out
}

fn leg_line(leg: &Leg, origin: &str, destination: &str, s: Style) -> String {
    let to = place_name(leg.to_place.name.as_deref(), origin, destination);

    if !leg.is_transit() {
        return format!(
            "      {} {}\n",
            s.dim(&format!(
                "\u{21b3} {} {:<5}",
                mode_label(&leg.mode),
                duration_human(leg.duration)
            )),
            s.dim(&format!("\u{2192} {to}"))
        );
    }

    let line = leg.line.as_ref();
    let badge_text = line
        .and_then(|l| l.public_code.clone())
        // publicCode is genuinely null for some rail services; fall back to the
        // mode rather than dropping the leg from the itinerary.
        .unwrap_or_else(|| mode_label(&leg.mode).to_string());
    let presentation = line.and_then(|l| l.presentation.as_ref());
    let colour = presentation.and_then(|p| p.colour.as_deref()).and_then(Rgb::from_hex);
    let text_colour = presentation.and_then(|p| p.text_colour.as_deref()).and_then(Rgb::from_hex);

    let platform = leg
        .from_place
        .quay
        .as_ref()
        .and_then(|q| q.public_code.as_deref())
        .filter(|c| !c.is_empty())
        .map(|c| format!(" spor {c}"))
        .unwrap_or_default();

    let from = place_name(leg.from_place.name.as_deref(), origin, destination);
    let realtime = if leg.realtime { s.green(REALTIME) } else { s.dim(SCHEDULED) };
    let delay = delay_marker(leg.delay_minutes(), s);

    format!(
        "      {} {:<7} {}{} {} {} {}\n",
        s.badge(&badge_text, colour, text_colour),
        mode_label(&leg.mode),
        s.dim(from),
        s.dim(&platform),
        hhmm(leg.expected_start_time),
        realtime,
        format_args!("{delay} {}", s.dim(&format!("\u{2192} {to}"))),
    )
}

pub fn near_board(
    stops: &[(NearbyStop, Option<StopPlace>)],
    origin: &Origin,
    now: DateTime<FixedOffset>,
    s: Style,
) -> String {
    let mut out = header(origin, "avganger i n\u{e6}rheten", now, s);

    if stops.is_empty() {
        out.push_str(&format!("\n  {}\n\n", s.dim("Fant ingen holdeplasser i n\u{e6}rheten.")));
        return out;
    }

    for (stop, board) in stops {
        out.push('\n');
        out.push_str(&format!(
            "  {}  {}\n",
            s.bold(&stop.name),
            s.dim(&format!("{} m \u{00b7} {}", stop.distance_m, stop.modes.join(", ")))
        ));

        let calls = board.as_ref().map(|b| b.estimated_calls.as_slice()).unwrap_or(&[]);
        if calls.is_empty() {
            out.push_str(&format!("      {}\n", s.dim("ingen avganger de neste to timene")));
            continue;
        }
        for call in calls {
            out.push_str(&departure_line(call, now, s));
        }
    }

    out.push_str(&format!(
        "\n  {}\n\n",
        s.dim(&format!("posisjon: {} \u{00b7} sanntid fra Entur", origin.source.label()))
    ));
    out
}

fn departure_line(call: &EstimatedCall, now: DateTime<FixedOffset>, s: Style) -> String {
    let line = call.line();
    let badge_text = line
        .and_then(|l| l.public_code.clone())
        .unwrap_or_else(|| line.and_then(|l| l.transport_mode.clone()).unwrap_or_default());
    let presentation = line.and_then(|l| l.presentation.as_ref());
    let colour = presentation.and_then(|p| p.colour.as_deref()).and_then(Rgb::from_hex);
    let text_colour = presentation.and_then(|p| p.text_colour.as_deref()).and_then(Rgb::from_hex);

    let platform = call
        .quay
        .as_ref()
        .and_then(|q| q.public_code.as_deref())
        .filter(|c| !c.is_empty())
        .map(|c| format!("spor {c}"))
        .unwrap_or_default();

    if call.cancellation {
        return format!(
            "      {} {:<24} {:<8} {}\n",
            s.badge(&badge_text, colour, text_colour),
            call.destination(),
            s.dim(&platform),
            s.red("AVLYST"),
        );
    }

    let realtime = if call.realtime { s.green(REALTIME) } else { s.dim(SCHEDULED) };
    format!(
        "      {} {:<24} {:<8} {} {} {:<5} {}\n",
        s.badge(&badge_text, colour, text_colour),
        call.destination(),
        s.dim(&platform),
        hhmm(call.expected_departure_time),
        realtime,
        delay_marker(call.delay_minutes(), s),
        s.dim(&relative(call.expected_departure_time, now)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entur::Coord;
    use crate::entur::nearest::StopPlaceResponse;
    use crate::entur::trip::TripResponse;
    use crate::location::Source;

    fn origin(source: Source) -> Origin {
        Origin { coord: Coord { lat: 59.9139, lon: 10.7522 }, name: "Storgata".to_string(), source }
    }

    fn now() -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2026-07-31T13:50:00+02:00").unwrap()
    }

    fn patterns() -> Vec<TripPattern> {
        let resp: TripResponse =
            serde_json::from_str(include_str!("../../tests/fixtures/trip.json")).unwrap();
        resp.trip.trip_patterns
    }

    #[test]
    fn renders_a_plain_trip_board() {
        let out = trip_board(&patterns(), &origin(Source::Gps), "Hjemme", now(), Style::plain());
        assert!(out.contains("Storgata"));
        assert!(out.contains("Hjemme"));
        // A line badge and a walking leg should both be present.
        assert!(out.contains('['), "expected an ASCII line badge:\n{out}");
        assert!(out.contains("gå"));
        // No escape sequences in plain mode.
        assert!(!out.contains('\x1b'), "plain output must not contain ANSI escapes");
    }

    #[test]
    fn warns_when_position_came_from_ip() {
        let out = trip_board(&patterns(), &origin(Source::Ip), "Hjemme", now(), Style::plain());
        assert!(out.contains("kan bomme med flere kilometer"));
    }

    #[test]
    fn does_not_warn_for_gps() {
        let out = trip_board(&patterns(), &origin(Source::Gps), "Hjemme", now(), Style::plain());
        assert!(!out.contains("kan bomme"));
    }

    #[test]
    fn empty_results_render_a_message_not_a_crash() {
        let out = trip_board(&[], &origin(Source::Gps), "Hjemme", now(), Style::plain());
        assert!(out.contains("Fant ingen reiser"));
    }

    #[test]
    fn renders_a_departure_board() {
        let resp: StopPlaceResponse =
            serde_json::from_str(include_str!("../../tests/fixtures/departures.json")).unwrap();
        let stop = resp.stop_place.unwrap();
        let nearby = NearbyStop {
            id: stop.id.clone(),
            name: "Jernbanetorget".into(),
            distance_m: 255,
            modes: vec!["tram".into(), "bus".into()],
        };
        let out = near_board(&[(nearby, Some(stop))], &origin(Source::Gps), now(), Style::plain());
        assert!(out.contains("Jernbanetorget"));
        assert!(out.contains("255 m"));
        assert!(!out.contains('\x1b'));
    }

    #[test]
    fn stop_with_no_departures_says_so() {
        let nearby = NearbyStop {
            id: "NSR:StopPlace:1".into(),
            name: "Tomt".into(),
            distance_m: 10,
            modes: vec!["bus".into()],
        };
        let out = near_board(&[(nearby, None)], &origin(Source::Gps), now(), Style::plain());
        assert!(out.contains("ingen avganger"));
    }
}
