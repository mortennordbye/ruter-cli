//! Shared formatting: colours, time helpers and line badges.

pub mod board;

use crate::entur::nearest::EstimatedCall;
use crate::entur::trip::{Leg, Presentation, TripPattern};
use chrono::{DateTime, FixedOffset};
use std::io::IsTerminal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// Parse Entur's `presentation.colour`, which is hex with no leading `#`.
    /// Wire data must never panic, so anything unexpected yields `None`.
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.strip_prefix('#').unwrap_or(s);
        if s.len() != 6 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        Some(Rgb {
            r: u8::from_str_radix(&s[0..2], 16).ok()?,
            g: u8::from_str_radix(&s[2..4], 16).ok()?,
            b: u8::from_str_radix(&s[4..6], 16).ok()?,
        })
    }

    /// WCAG relative luminance.
    fn luminance(self) -> f64 {
        fn channel(v: u8) -> f64 {
            let v = v as f64 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        }
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }

    fn contrast_with(self, other: Rgb) -> f64 {
        let (a, b) = (self.luminance(), other.luminance());
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// Pick a readable foreground for this background.
    ///
    /// Some operators publish a `textColour` that fails contrast against their
    /// own `colour` — light yellow lines are the usual offender — so the
    /// declared value is only honoured when it is actually legible.
    pub fn readable_text(self, declared: Option<Rgb>) -> Rgb {
        const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };
        const WHITE: Rgb = Rgb { r: 255, g: 255, b: 255 };
        if let Some(d) = declared
            && self.contrast_with(d) >= 3.0
        {
            return d;
        }
        if self.contrast_with(BLACK) >= self.contrast_with(WHITE) { BLACK } else { WHITE }
    }
}

/// The text and colours of a line badge, resolved from the wire data.
///
/// The one-shot board and the `--watch` view each render two kinds of row, and all
/// four resolved this identically. The fallbacks are the part worth having in one
/// place: `publicCode` is genuinely null for some rail services, and a colour that
/// fails to parse has to leave the badge uncoloured rather than drop the row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Badge {
    pub text: String,
    pub bg: Option<Rgb>,
    pub fg: Option<Rgb>,
}

impl Badge {
    fn new(text: String, presentation: Option<&Presentation>) -> Self {
        Badge {
            text,
            bg: presentation.and_then(|p| p.colour.as_deref()).and_then(Rgb::from_hex),
            fg: presentation.and_then(|p| p.text_colour.as_deref()).and_then(Rgb::from_hex),
        }
    }

    /// For an itinerary leg, falling back to the mode name for an unnumbered service.
    pub fn for_leg(leg: &Leg) -> Self {
        let line = leg.line.as_ref();
        let text = line
            .and_then(|l| l.public_code.clone())
            .unwrap_or_else(|| mode_label(&leg.mode).to_string());
        Self::new(text, line.and_then(|l| l.presentation.as_ref()))
    }

    /// For a departure, falling back to the transport mode and then to nothing.
    pub fn for_call(call: &EstimatedCall) -> Self {
        let line = call.line();
        let text = line
            .and_then(|l| l.public_code.clone())
            .or_else(|| line.and_then(|l| l.transport_mode.clone()))
            .unwrap_or_default();
        Self::new(text, line.and_then(|l| l.presentation.as_ref()))
    }
}

/// Whether to emit ANSI escapes at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub colour: bool,
}

impl Style {
    /// Honour `NO_COLOR` and a non-tty stdout, per the usual conventions.
    pub fn detect(force: Option<bool>) -> Self {
        if let Some(force) = force {
            return Style { colour: force };
        }
        let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
        Style { colour: !no_color && std::io::stdout().is_terminal() }
    }

    pub fn plain() -> Self {
        Style { colour: false }
    }

    /// Render a line badge. `fg` is the operator's declared `textColour`, which
    /// is honoured when legible and overridden when it is not.
    ///
    /// Line codes run from "1" to "34X", so the label is centred in a fixed
    /// three-character field to keep the columns after it aligned.
    pub fn badge(self, text: &str, bg: Option<Rgb>, fg: Option<Rgb>) -> String {
        let padded = format!(" {text:^3} ");
        match (self.colour, bg) {
            (true, Some(bg)) => {
                let fg = bg.readable_text(fg);
                format!(
                    "\x1b[48;2;{};{};{}m\x1b[38;2;{};{};{}m\x1b[1m{padded}\x1b[0m",
                    bg.r, bg.g, bg.b, fg.r, fg.g, fg.b
                )
            }
            (true, None) => format!("\x1b[7m{padded}\x1b[0m"),
            (false, _) => format!("[{text:^3}]"),
        }
    }

    pub fn paint(self, text: &str, code: &str) -> String {
        if self.colour { format!("\x1b[{code}m{text}\x1b[0m") } else { text.to_string() }
    }

    pub fn dim(self, text: &str) -> String {
        self.paint(text, "2")
    }
    pub fn bold(self, text: &str) -> String {
        self.paint(text, "1")
    }
    pub fn green(self, text: &str) -> String {
        self.paint(text, "32")
    }
    pub fn yellow(self, text: &str) -> String {
        self.paint(text, "33")
    }
    pub fn red(self, text: &str) -> String {
        self.paint(text, "31")
    }
    pub fn cyan(self, text: &str) -> String {
        self.paint(text, "36")
    }
}

/// Shared column layout for the one-shot boards and the `--watch` view, so the two
/// surfaces line up identically.
///
/// Fixed rather than read from the terminal: nothing else in this crate queries the
/// terminal size, and a rule wider than the window merely wraps.
pub const RULE_WIDTH: usize = 72;

/// Minimum width of the place-name column on an itinerary row.
///
/// A minimum rather than a maximum: `pad` never truncates, so a long name like
/// "Kjelsås stasjon spor 2" pushes the rest of its own row right instead of being
/// cut. Recognising the stop matters more than a perfectly straight right edge.
pub const POINT_STOP: usize = 22;

/// The vertical connector drawn between two points of a journey.
pub const CONNECTOR: &str = "\u{254e}"; // ╎

/// What you board at a point, absent at the start and end of the journey.
#[derive(Debug, Clone)]
pub struct Ride {
    pub badge: Badge,
    /// Already translated by `mode_label`.
    pub mode: String,
    /// Where this vehicle takes you.
    pub to: String,
    pub realtime: bool,
    pub delay_minutes: i64,
}

/// One row of an itinerary drawn as a timeline.
#[derive(Debug, Clone)]
pub enum Step {
    /// A place, the time you are there, and what you board if anything.
    Point { time: DateTime<FixedOffset>, place: String, ride: Option<Ride> },
    /// The stretch between two points under your own power.
    Walk { mode: String, seconds: i64 },
}

/// Flatten a journey into the rows both boards draw.
///
/// Built here rather than in each renderer because the one-shot board and the watch
/// view have to agree exactly; the awkward parts are the joins, and doing them twice
/// is how the two drift apart.
///
/// The shape drops a repetition the old layout carried: a walking leg always ends
/// where the next leg begins, so naming its destination *and* the next boarding stop
/// printed every intermediate stop twice. Here a walk is just its duration, and each
/// place is named once, on the row that gives its time.
pub fn timeline(pattern: &TripPattern, origin: &str, destination: &str) -> Vec<Step> {
    let mut steps = Vec::new();
    let name = |raw: Option<&str>| place_name(raw, origin, destination).to_string();

    // Where you set off. Skipped when the journey opens by boarding something, since
    // that leg's own row already names the same place at the same minute.
    if let Some(first) = pattern.legs.first()
        && !first.is_transit()
    {
        steps.push(Step::Point {
            time: pattern.expected_start_time,
            place: name(first.from_place.name.as_deref()),
            ride: None,
        });
    }

    for leg in &pattern.legs {
        if !leg.is_transit() {
            steps.push(Step::Walk {
                mode: mode_label(&leg.mode).to_string(),
                seconds: leg.duration,
            });
            continue;
        }

        // The signposted platform belongs with the stop you are standing at.
        let platform = leg
            .from_place
            .quay
            .as_ref()
            .and_then(|q| q.public_code.as_deref())
            .filter(|c| !c.is_empty())
            .map(|c| format!(" spor {c}"))
            .unwrap_or_default();

        steps.push(Step::Point {
            time: leg.expected_start_time,
            place: format!("{}{platform}", name(leg.from_place.name.as_deref())),
            ride: Some(Ride {
                badge: Badge::for_leg(leg),
                mode: mode_label(&leg.mode).to_string(),
                to: name(leg.to_place.name.as_deref()),
                realtime: leg.realtime,
                delay_minutes: leg.delay_minutes(),
            }),
        });
    }

    // Where you end up. Always drawn, because it is the only row carrying the
    // arrival time even when the last leg is a ride naming the same place.
    if let Some(last) = pattern.legs.last() {
        steps.push(Step::Point {
            time: pattern.expected_end_time,
            place: name(last.to_place.name.as_deref()),
            ride: None,
        });
    }

    steps
}

/// Pad before styling. ANSI escapes count toward `{:<n}`, so padding an already-styled
/// string silently misaligns every column as soon as colour is on.
pub fn pad(text: &str, width: usize) -> String {
    format!("{text:<width$}")
}

/// `14:07`
pub fn hhmm(t: DateTime<FixedOffset>) -> String {
    t.format("%H:%M").to_string()
}

/// A countdown relative to `now`: `nå`, `om 4 min`, `om 1 t 5 min`.
pub fn relative(target: DateTime<FixedOffset>, now: DateTime<FixedOffset>) -> String {
    let secs = (target - now).num_seconds();
    if secs < -60 {
        return format!("for {} min siden", (-secs) / 60);
    }
    if secs < 30 {
        return "nå".to_string();
    }
    let mins = (secs + 30) / 60;
    if mins < 60 {
        return format!("om {mins} min");
    }
    let (h, m) = (mins / 60, mins % 60);
    if m == 0 { format!("om {h} t") } else { format!("om {h} t {m} min") }
}

/// `27 min`, `1 t 5 min`
pub fn duration_human(seconds: i64) -> String {
    let mins = (seconds + 30) / 60;
    if mins < 60 {
        return format!("{mins} min");
    }
    let (h, m) = (mins / 60, mins % 60);
    if m == 0 { format!("{h} t") } else { format!("{h} t {m} min") }
}

/// Render a delay as `+3` / `-1`, or nothing when it rounds away.
///
/// Anything under a minute is suppressed: a bus 40 seconds behind schedule is
/// on time as far as a person waiting at the stop is concerned.
///
/// Always three columns wide, including when there is no delay to show. The callers
/// print it mid-line, so a variable width here shifts every column after it.
pub fn delay_marker(delay_minutes: i64, style: Style) -> String {
    match delay_minutes {
        0 => "   ".to_string(),
        d if d >= 5 => style.red(&format!("{:<3}", format!("+{d}"))),
        d if d > 0 => style.yellow(&format!("{:<3}", format!("+{d}"))),
        d => style.cyan(&format!("{d:<3}")),
    }
}

/// Resolve a place name from a leg.
///
/// For a coordinate-based search Entur names the endpoints with the literal
/// strings "Origin" and "Destination", which are not useful to show, so they get
/// substituted for the names the user actually asked for.
pub fn place_name<'a>(raw: Option<&'a str>, origin: &'a str, destination: &'a str) -> &'a str {
    match raw {
        Some("Origin") => origin,
        Some("Destination") => destination,
        Some(other) => other,
        None => "\u{2014}",
    }
}

/// A short human label for a transport mode.
pub fn mode_label(mode: &str) -> &str {
    match mode {
        "foot" => "gå",
        "bus" => "buss",
        "tram" => "trikk",
        "metro" => "T-bane",
        "rail" => "tog",
        "water" => "båt",
        "coach" => "ekspressbuss",
        "air" => "fly",
        "bicycle" => "sykkel",
        other => other,
    }
}

#[cfg(test)]
mod timeline_tests {
    use super::*;
    use crate::entur::trip::{TripPattern, TripResponse};

    fn patterns() -> Vec<TripPattern> {
        let resp: TripResponse =
            serde_json::from_str(include_str!("../../tests/fixtures/trip.json")).unwrap();
        resp.trip.trip_patterns
    }

    fn places(steps: &[Step]) -> Vec<&str> {
        steps
            .iter()
            .filter_map(|s| match s {
                Step::Point { place, .. } => Some(place.as_str()),
                Step::Walk { .. } => None,
            })
            .collect()
    }

    #[test]
    fn a_journey_reads_start_boarding_end() {
        // Fixture pattern 1 is walk -> metro -> walk, so it should flatten to three
        // named points with a walk between each.
        let steps = timeline(&patterns()[1], "Storgata", "Hjemme");
        assert_eq!(places(&steps), ["Storgata", "Jernbanetorget spor 2", "Hjemme"]);
        assert!(matches!(steps[1], Step::Walk { .. }));
        assert!(matches!(steps[3], Step::Walk { .. }));
    }

    /// The point of the layout: a walk ends where the next leg begins, so naming its
    /// destination would print every intermediate stop twice.
    #[test]
    fn a_walk_names_no_place() {
        for pattern in patterns() {
            for step in timeline(&pattern, "Storgata", "Hjemme") {
                if let Step::Walk { mode, seconds } = step {
                    assert_eq!(mode, "gå");
                    assert!(seconds > 0);
                }
            }
        }
    }

    #[test]
    fn the_boarding_point_carries_the_signposted_platform() {
        let steps = timeline(&patterns()[1], "Storgata", "Hjemme");
        // Recorded quay publicCode is "2" on the metro leg.
        assert!(places(&steps).contains(&"Jernbanetorget spor 2"));
    }

    #[test]
    fn the_ride_is_attached_to_the_stop_you_board_it_at() {
        let steps = timeline(&patterns()[1], "Storgata", "Hjemme");
        let Step::Point { place, ride: Some(ride), .. } = &steps[2] else {
            panic!("expected a boarding point, got {:?}", steps[2]);
        };
        assert_eq!(place, "Jernbanetorget spor 2");
        assert_eq!(ride.badge.text, "1");
        assert_eq!(ride.mode, "T-bane");
        assert_eq!(ride.to, "Gaustad");
    }

    #[test]
    fn the_endpoints_carry_the_trip_times_and_no_ride() {
        let pattern = &patterns()[1];
        let steps = timeline(pattern, "Storgata", "Hjemme");

        let (first, last) = (steps.first().unwrap(), steps.last().unwrap());
        let Step::Point { time, ride: None, .. } = first else { panic!("start should be plain") };
        assert_eq!(*time, pattern.expected_start_time);
        let Step::Point { time, ride: None, .. } = last else { panic!("end should be plain") };
        assert_eq!(*time, pattern.expected_end_time);
    }

    #[test]
    fn enturs_endpoint_placeholders_never_survive() {
        for pattern in patterns() {
            let steps = timeline(&pattern, "Storgata", "Hjemme");
            for place in places(&steps) {
                assert!(place != "Origin" && place != "Destination", "leaked: {place}");
            }
        }
    }

    /// A journey that opens by boarding would otherwise print the same place twice at
    /// the same minute: once as the start row, once as the boarding row.
    #[test]
    fn boarding_immediately_produces_no_duplicate_start_row() {
        let pattern: TripPattern = serde_json::from_str(
            r#"{
              "expectedStartTime": "2026-07-31T14:00:00+02:00",
              "expectedEndTime":   "2026-07-31T14:20:00+02:00",
              "duration": 1200, "walkDistance": 0.0,
              "legs": [{
                "mode": "bus", "distance": 100.0, "duration": 1200,
                "aimedStartTime":    "2026-07-31T14:00:00+02:00",
                "expectedStartTime": "2026-07-31T14:00:00+02:00",
                "expectedEndTime":   "2026-07-31T14:20:00+02:00",
                "realtime": true,
                "fromPlace": {"name": "Storgata", "quay": null},
                "toPlace":   {"name": "Hjemme",   "quay": null},
                "line": {"publicCode": "5", "name": null, "presentation": null}
              }]
            }"#,
        )
        .unwrap();

        let steps = timeline(&pattern, "Storgata", "Hjemme");
        assert_eq!(places(&steps), ["Storgata", "Hjemme"], "expected no repeated start row");
        assert!(matches!(steps[0], Step::Point { ride: Some(_), .. }), "the ride opens the trip");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(h: u32, m: u32, s: u32) -> DateTime<FixedOffset> {
        FixedOffset::east_opt(2 * 3600).unwrap().with_ymd_and_hms(2026, 7, 31, h, m, s).unwrap()
    }

    #[test]
    fn parses_entur_colour_without_hash() {
        assert_eq!(Rgb::from_hex("EC700C"), Some(Rgb { r: 0xEC, g: 0x70, b: 0x0C }));
        assert_eq!(Rgb::from_hex("#EC700C"), Some(Rgb { r: 0xEC, g: 0x70, b: 0x0C }));
    }

    #[test]
    fn malformed_colours_never_panic() {
        assert_eq!(Rgb::from_hex(""), None);
        assert_eq!(Rgb::from_hex("ECC"), None);
        assert_eq!(Rgb::from_hex("zzzzzz"), None);
        assert_eq!(Rgb::from_hex("EC700C11"), None);
    }

    #[test]
    fn unreadable_declared_text_colour_is_overridden() {
        // Light yellow background with white text: unreadable, expect black.
        let bg = Rgb { r: 0xFF, g: 0xE0, b: 0x33 };
        let white = Rgb { r: 255, g: 255, b: 255 };
        assert_eq!(bg.readable_text(Some(white)), Rgb { r: 0, g: 0, b: 0 });
    }

    #[test]
    fn legible_declared_text_colour_is_kept() {
        // Ruter metro orange with white text: fine, keep it.
        let bg = Rgb::from_hex("EC700C").unwrap();
        let white = Rgb { r: 255, g: 255, b: 255 };
        assert_eq!(bg.readable_text(Some(white)), white);
    }

    #[test]
    fn plain_style_emits_no_escapes() {
        let s = Style::plain();
        assert_eq!(s.badge("11", Rgb::from_hex("EC700C"), None), "[11 ]");
        assert_eq!(s.red("late"), "late");
        assert!(!s.dim("x").contains('\x1b'));
    }

    #[test]
    fn badges_are_a_fixed_width_regardless_of_line_code() {
        let s = Style::plain();
        // "1", "18" and "34X" must all occupy the same number of columns, or
        // every column to the right of the badge drifts.
        let widths: Vec<usize> =
            ["1", "18", "34X"].iter().map(|c| s.badge(c, None, None).chars().count()).collect();
        assert_eq!(widths, vec![5, 5, 5]);
    }

    #[test]
    fn coloured_badge_uses_truecolor_background() {
        let s = Style { colour: true };
        let out = s.badge("11", Rgb::from_hex("EC700C"), None);
        assert!(out.contains("48;2;236;112;12"));
        assert!(out.ends_with("\x1b[0m"));
    }

    #[test]
    fn badge_honours_the_operators_declared_text_colour() {
        let s = Style { colour: true };
        // Ruter's tram blue declares white text, which is legible; keep it.
        // Getting this wrong renders blue badges with black text.
        let out = s.badge("18", Rgb::from_hex("0B91EF"), Rgb::from_hex("FFFFFF"));
        assert!(out.contains("38;2;255;255;255"), "expected white text, got: {out:?}");
    }

    #[test]
    fn relative_time_reads_naturally() {
        let now = at(14, 0, 0);
        assert_eq!(relative(at(14, 0, 10), now), "nå");
        assert_eq!(relative(at(14, 4, 0), now), "om 4 min");
        assert_eq!(relative(at(15, 5, 0), now), "om 1 t 5 min");
        assert_eq!(relative(at(15, 0, 0), now), "om 1 t");
        assert_eq!(relative(at(13, 57, 0), now), "for 3 min siden");
    }

    #[test]
    fn durations_switch_to_hours_past_sixty_minutes() {
        assert_eq!(duration_human(1611), "27 min");
        assert_eq!(duration_human(60), "1 min");
        assert_eq!(duration_human(3900), "1 t 5 min");
        assert_eq!(duration_human(3600), "1 t");
    }

    #[test]
    fn sub_minute_delays_are_suppressed() {
        let s = Style::plain();
        // Padded to a fixed three columns so it does not shift the columns after it.
        assert_eq!(delay_marker(0, s), "   ");
        assert_eq!(delay_marker(1, s), "+1 ");
        assert_eq!(delay_marker(5, s), "+5 ");
        assert_eq!(delay_marker(-2, s), "-2 ");
    }

    #[test]
    fn substitutes_enturs_endpoint_placeholders() {
        assert_eq!(place_name(Some("Origin"), "Storgata", "Hjemme"), "Storgata");
        assert_eq!(place_name(Some("Destination"), "Storgata", "Hjemme"), "Hjemme");
        // A real stop name passes through untouched.
        assert_eq!(place_name(Some("Jernbanetorget"), "Storgata", "Hjemme"), "Jernbanetorget");
        assert_eq!(place_name(None, "Storgata", "Hjemme"), "\u{2014}");
    }

    #[test]
    fn translates_transport_modes() {
        assert_eq!(mode_label("metro"), "T-bane");
        assert_eq!(mode_label("foot"), "gå");
        // Unknown modes pass through rather than being dropped.
        assert_eq!(mode_label("funicular"), "funicular");
    }
}
