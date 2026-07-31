//! The one-shot boards printed to stdout.

use super::{
    LEG_PREFIX, RULE_WIDTH, Rgb, STOP_COLUMN, Style, delay_marker, duration_human, hhmm,
    mode_label, pad, place_name, relative,
};
use crate::entur::nearest::{EstimatedCall, NearbyStop, StopPlace};
use crate::entur::trip::{Leg, TripPattern};
use crate::location::Origin;
use chrono::{DateTime, FixedOffset};

const REALTIME: &str = "\u{25cf}"; // ●
const SCHEDULED: &str = "\u{25cb}"; // ○

fn rule(s: Style, heavy: bool) -> String {
    let glyph = if heavy { "\u{2550}" } else { "\u{2500}" };
    format!("  {}\n", s.dim(&glyph.repeat(RULE_WIDTH)))
}

/// Header shown above both boards.
pub fn header(origin: &Origin, destination: &str, now: DateTime<FixedOffset>, s: Style) -> String {
    let title = format!("{}  \u{2192}  {}", origin.name, destination);
    let mut out = format!(
        "\n  {}{}\n",
        s.bold(&pad(&title, RULE_WIDTH.saturating_sub(5))),
        s.dim(&hhmm(now))
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
    out.push_str(&rule(s, true));
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

    for (i, p) in patterns.iter().enumerate() {
        if i > 0 {
            out.push_str(&rule(s, false));
        }
        let marker = if p.has_realtime() { s.green(REALTIME) } else { s.dim(SCHEDULED) };
        let transfers = p.transit_legs().count().saturating_sub(1);
        let transfer_text = match transfers {
            0 => "direkte".to_string(),
            1 => "1 bytte".to_string(),
            n => format!("{n} bytter"),
        };

        out.push_str(&format!(
            "  {marker} {} {} \u{2192} {}   {} {}\n",
            s.bold(&pad(&relative(p.expected_start_time, now), 12)),
            hhmm(p.expected_start_time),
            hhmm(p.expected_end_time),
            pad(&duration_human(p.duration), 8),
            s.dim(&transfer_text),
        ));

        for leg in &p.legs {
            out.push_str(&leg_line(leg, &origin.name, destination, s));
        }
    }
    out.push_str(&rule(s, false));

    // Situations repeat across every journey that touches the affected stop, so the same
    // notice would otherwise print three or four times. Collapse to one footnote each.
    for (text, count) in situation_footnotes(patterns) {
        let suffix = if count > 1 { format!(" ({count} reiser)") } else { String::new() };
        out.push_str(&format!("  {}\n", s.yellow(&format!("\u{26a0} {text}{suffix}"))));
    }

    out.push_str(&format!(
        "\n  {}\n\n",
        s.dim(&format!("posisjon: {} \u{00b7} sanntid fra Entur", origin.source.label()))
    ));
    out
}

/// Distinct situation summaries, in first-seen order, with the number of journeys each
/// affects. A situation attached to several legs of one journey still counts once.
fn situation_footnotes(patterns: &[TripPattern]) -> Vec<(String, usize)> {
    let mut order: Vec<String> = Vec::new();
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for p in patterns {
        let mut seen_here: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for situation in p.legs.iter().flat_map(|l| &l.situations) {
            let Some(first) = situation.summary.first() else { continue };
            let text = first.value.trim();
            if text.is_empty() || !seen_here.insert(text) {
                continue;
            }
            if !counts.contains_key(text) {
                order.push(text.to_string());
            }
            *counts.entry(text.to_string()).or_default() += 1;
        }
    }

    order
        .into_iter()
        .map(|text| {
            let n = counts[&text];
            (text, n)
        })
        .collect()
}

fn leg_line(leg: &Leg, origin: &str, destination: &str, s: Style) -> String {
    let to = place_name(leg.to_place.name.as_deref(), origin, destination);

    if !leg.is_transit() {
        let text = format!("\u{21b3} {} {}", mode_label(&leg.mode), duration_human(leg.duration));
        return format!(
            "      {} {}\n",
            s.dim(&pad(&text, LEG_PREFIX)),
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

    // The badge is a fixed five columns wide in both colour and plain mode, so the
    // remaining widths are what keep the `→ destination` column straight.
    let stop = pad(&format!("{from}{platform}"), STOP_COLUMN);
    format!(
        "      {} {} {} {} {} {} {}\n",
        s.badge(&badge_text, colour, text_colour),
        pad(mode_label(&leg.mode), 6),
        s.dim(&stop),
        hhmm(leg.expected_start_time),
        realtime,
        delay,
        s.dim(&format!("\u{2192} {to}")),
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

    for (i, (stop, board)) in stops.iter().enumerate() {
        if i > 0 {
            out.push_str(&rule(s, false));
        }
        out.push_str(&format!(
            "  {}{}\n",
            s.bold(&pad(&stop.name, 28)),
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
    out.push_str(&rule(s, false));

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
            "      {} {} {} {}\n",
            s.badge(&badge_text, colour, text_colour),
            pad(call.destination(), 26),
            s.dim(&pad(&platform, 8)),
            s.red("AVLYST"),
        );
    }

    let realtime = if call.realtime { s.green(REALTIME) } else { s.dim(SCHEDULED) };
    format!(
        "      {} {} {} {} {} {} {}\n",
        s.badge(&badge_text, colour, text_colour),
        pad(call.destination(), 26),
        s.dim(&pad(&platform, 8)),
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
