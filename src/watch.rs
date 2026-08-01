//! `--watch`: a live, self-refreshing board.
//!
//! Two clocks run at different rates on purpose. The screen redraws several
//! times a second so the "om N min" countdowns tick smoothly, while the network
//! poll happens on `interval` (default 30 s). Refreshing the network as often as
//! the screen would both look broken and get us rate-limited.

use crate::entur::Client;
use crate::entur::nearest::{NearbyStop, StopPlace};
use crate::entur::trip::{TripPattern, TripQuery};
use crate::location::Origin;
use crate::render::{
    LEG_PREFIX, RULE_WIDTH, Rgb, STOP_COLUMN, delay_marker, duration_human, hhmm, mode_label, pad,
    place_name, relative,
};
use anyhow::Result;
use chrono::{DateTime, FixedOffset, Local};
use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style as TuiStyle};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

/// What the poller thread is told to do.
enum Cmd {
    RefreshNow,
    Quit,
}

/// Whatever the current board is showing.
///
/// The `Stale` variant is the point of the whole state machine: a transient
/// network blip must never blank the board. We keep showing the last good data
/// with a banner saying how old it is.
enum State<T> {
    Loading,
    Ready { data: T, at: DateTime<FixedOffset> },
    Stale { data: T, at: DateTime<FixedOffset>, err: String },
    Failed { err: String },
}

impl<T> State<T> {
    fn apply(&mut self, incoming: Result<T>, now: DateTime<FixedOffset>) {
        match incoming {
            Ok(data) => *self = State::Ready { data, at: now },
            Err(e) => {
                let err = format!("{e:#}");
                // Downgrade Ready -> Stale, keeping the data we already have.
                match std::mem::replace(self, State::Loading) {
                    State::Ready { data, at } | State::Stale { data, at, .. } => {
                        *self = State::Stale { data, at, err }
                    }
                    _ => *self = State::Failed { err },
                }
            }
        }
    }

    fn data(&self) -> Option<&T> {
        match self {
            State::Ready { data, .. } | State::Stale { data, .. } => Some(data),
            _ => None,
        }
    }
}

pub fn run_trip(
    client_name: &str,
    origin: &Origin,
    target: &Origin,
    query: &TripQuery,
    interval_secs: u64,
) -> Result<()> {
    let (from, to) = (origin.coord, target.coord);
    let query = query.clone();
    let client_name = client_name.to_string();
    let title = format!("{}  \u{2192}  {}", origin.name, target.name);
    let source = origin.source;
    let (origin_name, dest_name) = (origin.name.clone(), target.name.clone());

    run_loop(
        title,
        source,
        interval_secs,
        move || {
            let client = Client::new(&client_name);
            client.trip(from, to, &query)
        },
        move |d: &Vec<TripPattern>, now| trip_lines(d, now, &origin_name, &dest_name),
    )
}

pub fn run_near(
    client_name: &str,
    origin: &Origin,
    radius: u32,
    stop_count: usize,
    per_stop: usize,
    interval_secs: u64,
) -> Result<()> {
    let origin_owned = origin.clone();
    let client_name = client_name.to_string();
    let title = format!("Avganger n\u{e6}r {}", origin.name);
    let source = origin.source;

    run_loop(
        title,
        source,
        interval_secs,
        move || {
            let client = Client::new(&client_name);
            crate::fetch_near(&client, &origin_owned, radius, stop_count, per_stop)
        },
        |d: &Vec<(NearbyStop, Option<StopPlace>)>, now| near_lines(d, now),
    )
}

fn run_loop<T, F, R>(
    title: String,
    source: crate::location::Source,
    interval_secs: u64,
    fetch: F,
    render: R,
) -> Result<()>
where
    T: Send + 'static,
    F: Fn() -> Result<T> + Send + 'static,
    R: Fn(&T, DateTime<FixedOffset>) -> Vec<Line<'static>>,
{
    let interval = Duration::from_secs(interval_secs.max(10));
    let (cmd_tx, cmd_rx) = channel::<Cmd>();
    let (data_tx, data_rx) = channel::<Result<T>>();

    let handle = std::thread::spawn(move || poller(fetch, cmd_rx, data_tx, interval));

    // ratatui::run installs a panic hook and restores the terminal on the way
    // out, so a panic here cannot leave the shell in raw mode.
    let result = ratatui::run(|terminal| -> Result<()> {
        let mut state: State<T> = State::Loading;
        let mut last_fetch = Instant::now();

        loop {
            let now = Local::now().fixed_offset();
            terminal.draw(|frame| {
                draw(frame, &title, source, &state, &render, now, interval, last_fetch)
            })?;

            if event::poll(Duration::from_millis(250))?
                && let Event::Key(key) = event::read()?
                && key.is_press()
            {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Char('r') => {
                        let _ = cmd_tx.send(Cmd::RefreshNow);
                    }
                    _ => {}
                }
            }

            while let Ok(msg) = data_rx.try_recv() {
                state.apply(msg, Local::now().fixed_offset());
                last_fetch = Instant::now();
            }
        }
        Ok(())
    });

    let _ = cmd_tx.send(Cmd::Quit);
    let _ = handle.join();
    result
}

/// `recv_timeout` gives us scheduled polling and instant manual refresh from a
/// single primitive, with no condvar and no busy-waiting.
fn poller<T, F>(fetch: F, cmd_rx: Receiver<Cmd>, data_tx: Sender<Result<T>>, interval: Duration)
where
    F: Fn() -> Result<T>,
{
    loop {
        if data_tx.send(fetch()).is_err() {
            return; // UI has gone away
        }
        match cmd_rx.recv_timeout(interval) {
            Err(RecvTimeoutError::Timeout) | Ok(Cmd::RefreshNow) => continue,
            Ok(Cmd::Quit) | Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw<T, R>(
    frame: &mut Frame,
    title: &str,
    source: crate::location::Source,
    state: &State<T>,
    render: &R,
    now: DateTime<FixedOffset>,
    interval: Duration,
    last_fetch: Instant,
) where
    R: Fn(&T, DateTime<FixedOffset>) -> Vec<Line<'static>>,
{
    let [header, body, footer] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)])
            .areas(frame.area());

    draw_header(frame, header, title, source, state, now, interval, last_fetch);

    let lines = match state {
        State::Loading => vec![Line::from(Span::styled(
            "  Henter avganger \u{2026}",
            TuiStyle::default().add_modifier(Modifier::DIM),
        ))],
        State::Failed { err } => vec![
            Line::from(Span::styled(format!("  {err}"), TuiStyle::default().fg(Color::Red))),
            Line::from(""),
            Line::from(Span::styled(
                "  Trykk r for \u{e5} pr\u{f8}ve igjen.",
                TuiStyle::default().add_modifier(Modifier::DIM),
            )),
        ],
        _ => state.data().map(|d| render(d, now)).unwrap_or_default(),
    };
    frame.render_widget(Paragraph::new(lines), body);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  q avslutt   r oppdater n\u{e5}",
            TuiStyle::default().add_modifier(Modifier::DIM),
        ))),
        footer,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_header<T>(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    source: crate::location::Source,
    state: &State<T>,
    now: DateTime<FixedOffset>,
    interval: Duration,
    last_fetch: Instant,
) {
    let mut spans = vec![
        Span::styled(title.to_string(), TuiStyle::default().add_modifier(Modifier::BOLD)),
        Span::raw("   "),
        Span::styled(hhmm(now), TuiStyle::default().add_modifier(Modifier::DIM)),
    ];

    match state {
        State::Stale { at, err, .. } => {
            let age = (now - *at).num_seconds().max(0);
            spans.push(Span::raw("   "));
            spans.push(Span::styled(
                format!("\u{26a0} data {age} s gammel \u{b7} {}", first_line(err)),
                TuiStyle::default().fg(Color::Yellow),
            ));
        }
        State::Ready { .. } => {
            let remaining = interval.saturating_sub(last_fetch.elapsed()).as_secs();
            spans.push(Span::raw("   "));
            spans.push(Span::styled(
                format!("oppdaterer om {remaining} s"),
                TuiStyle::default().add_modifier(Modifier::DIM),
            ));
        }
        _ => {}
    }

    if source.is_coarse() {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(
            format!("\u{26a0} posisjon: {}", source.label()),
            TuiStyle::default().fg(Color::Yellow),
        ));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans)).block(Block::default().borders(Borders::BOTTOM)),
        area,
    );
}

// ---------------------------------------------------------------------------
// Row builders
// ---------------------------------------------------------------------------

fn badge_style(colour: Option<Rgb>, declared_fg: Option<Rgb>) -> TuiStyle {
    match colour {
        Some(bg) => {
            let fg = bg.readable_text(declared_fg);
            TuiStyle::default()
                .bg(Color::Rgb(bg.r, bg.g, bg.b))
                .fg(Color::Rgb(fg.r, fg.g, fg.b))
                .add_modifier(Modifier::BOLD)
        }
        None => TuiStyle::default().add_modifier(Modifier::REVERSED),
    }
}

/// Error chains can be several lines; the header only has room for the first.
fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or(s)
}

fn dim(text: impl Into<String>) -> Span<'static> {
    Span::styled(text.into(), TuiStyle::default().add_modifier(Modifier::DIM))
}

fn realtime_span(realtime: bool) -> Span<'static> {
    if realtime {
        Span::styled("\u{25cf}", TuiStyle::default().fg(Color::Green))
    } else {
        dim("\u{25cb}")
    }
}

fn delay_span(minutes: i64) -> Span<'static> {
    let text = delay_marker(minutes, crate::render::Style::plain());
    let colour = match minutes {
        0 => return Span::raw("   "),
        d if d >= 5 => Color::Red,
        d if d > 0 => Color::Yellow,
        _ => Color::Cyan,
    };
    Span::styled(format!("{text:<3}"), TuiStyle::default().fg(colour))
}

fn trip_lines(
    patterns: &[TripPattern],
    now: DateTime<FixedOffset>,
    origin: &str,
    destination: &str,
) -> Vec<Line<'static>> {
    if patterns.is_empty() {
        return vec![Line::from(dim("  Fant ingen reiser."))];
    }
    let mut lines = Vec::new();
    for (i, p) in patterns.iter().enumerate() {
        if i > 0 {
            lines.push(rule_line());
        }
        lines.push(Line::from(vec![
            Span::raw("  "),
            realtime_span(p.has_realtime()),
            Span::raw(" "),
            Span::styled(
                pad(&relative(p.expected_start_time, now), 12),
                TuiStyle::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "{} \u{2192} {}   ",
                hhmm(p.expected_start_time),
                hhmm(p.expected_end_time)
            )),
            dim(duration_human(p.duration)),
        ]));

        for leg in &p.legs {
            let to = place_name(leg.to_place.name.as_deref(), origin, destination).to_string();
            if !leg.is_transit() {
                let text =
                    format!("\u{21b3} {} {}", mode_label(&leg.mode), duration_human(leg.duration));
                lines.push(Line::from(vec![
                    Span::raw("      "),
                    dim(pad(&text, LEG_PREFIX)),
                    dim(format!(" \u{2192} {to}")),
                ]));
                continue;
            }

            let line = leg.line.as_ref();
            let badge = line
                .and_then(|l| l.public_code.clone())
                .unwrap_or_else(|| mode_label(&leg.mode).to_string());
            let presentation = line.and_then(|l| l.presentation.as_ref());
            let colour = presentation.and_then(|p| p.colour.as_deref()).and_then(Rgb::from_hex);
            let text_colour =
                presentation.and_then(|p| p.text_colour.as_deref()).and_then(Rgb::from_hex);
            let from = place_name(leg.from_place.name.as_deref(), origin, destination);

            lines.push(Line::from(vec![
                Span::raw("      "),
                Span::styled(format!(" {badge:^3} "), badge_style(colour, text_colour)),
                Span::raw(format!(" {} ", pad(mode_label(&leg.mode), 6))),
                dim(pad(from, STOP_COLUMN)),
                Span::raw(format!(" {} ", hhmm(leg.expected_start_time))),
                realtime_span(leg.realtime),
                Span::raw(" "),
                delay_span(leg.delay_minutes()),
                dim(format!(" \u{2192} {to}")),
            ]));
        }
    }
    lines.push(rule_line());
    lines
}

/// Matches the rules the one-shot board draws, so the two views read the same.
fn rule_line() -> Line<'static> {
    dim("  ".to_string() + &"\u{2500}".repeat(RULE_WIDTH)).into()
}

fn near_lines(
    stops: &[(NearbyStop, Option<StopPlace>)],
    now: DateTime<FixedOffset>,
) -> Vec<Line<'static>> {
    if stops.is_empty() {
        return vec![Line::from(dim("  Fant ingen holdeplasser i n\u{e6}rheten."))];
    }
    let mut lines = Vec::new();
    for (i, (stop, board)) in stops.iter().enumerate() {
        if i > 0 {
            lines.push(rule_line());
        }
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(pad(&stop.name, 28), TuiStyle::default().add_modifier(Modifier::BOLD)),
            dim(format!("{} m \u{b7} {}", stop.distance_m, stop.modes.join(", "))),
        ]));

        let calls = board.as_ref().map(|b| b.estimated_calls.as_slice()).unwrap_or(&[]);
        if calls.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("      "),
                dim("ingen avganger de neste to timene"),
            ]));
            continue;
        }

        for call in calls {
            let line = call.line();
            let badge = line
                .and_then(|l| l.public_code.clone())
                .or_else(|| line.and_then(|l| l.transport_mode.clone()))
                .unwrap_or_default();
            let presentation = line.and_then(|l| l.presentation.as_ref());
            let colour = presentation.and_then(|p| p.colour.as_deref()).and_then(Rgb::from_hex);
            let text_colour =
                presentation.and_then(|p| p.text_colour.as_deref()).and_then(Rgb::from_hex);

            if call.cancellation {
                lines.push(Line::from(vec![
                    Span::raw("      "),
                    Span::styled(format!(" {badge:^3} "), badge_style(colour, text_colour)),
                    Span::raw(format!(" {} ", pad(call.destination(), 26))),
                    Span::styled("AVLYST", TuiStyle::default().fg(Color::Red)),
                ]));
                continue;
            }

            lines.push(Line::from(vec![
                Span::raw("      "),
                Span::styled(format!(" {badge:^3} "), badge_style(colour, text_colour)),
                Span::raw(format!(" {} ", pad(call.destination(), 26))),
                Span::raw(format!("{} ", hhmm(call.expected_departure_time))),
                realtime_span(call.realtime),
                Span::raw(" "),
                delay_span(call.delay_minutes()),
                dim(relative(call.expected_departure_time, now)),
            ]));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entur::trip::TripResponse;

    fn now() -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2026-07-31T13:50:00+02:00").unwrap()
    }

    #[test]
    fn network_failure_downgrades_ready_to_stale_keeping_data() {
        let mut state: State<u32> = State::Loading;
        state.apply(Ok(42), now());
        assert!(matches!(state, State::Ready { .. }));

        state.apply(Err(anyhow::anyhow!("nettverket er nede")), now());
        // The whole point: we still have the data.
        assert!(matches!(state, State::Stale { .. }));
        assert_eq!(state.data(), Some(&42));
    }

    #[test]
    fn failure_before_any_data_is_terminal() {
        let mut state: State<u32> = State::Loading;
        state.apply(Err(anyhow::anyhow!("boom")), now());
        assert!(matches!(state, State::Failed { .. }));
        assert_eq!(state.data(), None);
    }

    #[test]
    fn recovering_from_stale_returns_to_ready() {
        let mut state: State<u32> = State::Loading;
        state.apply(Ok(1), now());
        state.apply(Err(anyhow::anyhow!("blip")), now());
        state.apply(Ok(2), now());
        assert!(matches!(state, State::Ready { .. }));
        assert_eq!(state.data(), Some(&2));
    }

    #[test]
    fn builds_rows_for_a_recorded_trip() {
        let resp: TripResponse =
            serde_json::from_str(include_str!("../tests/fixtures/trip.json")).unwrap();
        let lines = trip_lines(&resp.trip.trip_patterns, now(), "Storgata", "Hjemme");
        assert!(!lines.is_empty());
    }

    /// The watch view cannot be eyeballed the way the one-shot board can, so the column
    /// layout is pinned here instead: walking and transit legs must put `→` in the same
    /// place, which is the whole point of padding them to `LEG_PREFIX`.
    #[test]
    fn itinerary_legs_share_one_destination_column() {
        let resp: TripResponse =
            serde_json::from_str(include_str!("../tests/fixtures/trip.json")).unwrap();
        let lines = trip_lines(&resp.trip.trip_patterns, now(), "Storgata", "Hjemme");

        let columns: Vec<usize> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .filter(|text| text.starts_with("      ") && text.contains('\u{2192}'))
            .map(|text| text.chars().take_while(|c| *c != '\u{2192}').count())
            .collect();

        assert!(!columns.is_empty(), "expected itinerary legs in the fixture");
        assert!(
            columns.windows(2).all(|w| w[0] == w[1]),
            "destination arrows are not in one column: {columns:?}"
        );
    }

    #[test]
    fn watch_rows_substitute_endpoint_placeholders() {
        let resp: TripResponse =
            serde_json::from_str(include_str!("../tests/fixtures/trip.json")).unwrap();
        let lines = trip_lines(&resp.trip.trip_patterns, now(), "Storgata", "Hjemme");
        let text: String =
            lines.iter().flat_map(|l| l.spans.iter()).map(|s| s.content.as_ref()).collect();
        assert!(!text.contains("Destination"), "raw Entur placeholder leaked:\n{text}");
        assert!(!text.contains("Origin"), "raw Entur placeholder leaked:\n{text}");
        assert!(text.contains("Hjemme"));
    }

    #[test]
    fn empty_results_still_render_a_row() {
        assert_eq!(trip_lines(&[], now(), "A", "B").len(), 1);
        assert_eq!(near_lines(&[], now()).len(), 1);
    }
}
