# Backlog

Known gaps we have agreed to leave for later. Work in progress belongs on a branch, not here.

## `--modes` is silently ignored by `ruter near`

**What.** `--modes` is a global flag, but only the trip path reads it. `ruter near --modes tram`
still lists buses: verified against the live API, a `--stops 1` board at Storgata returned bus 17
and bus 54 alongside the trams. An invalid value is not rejected on this path either, because the
`validate_modes` call sits in `cmd_trip`.

**Why deferred.** The `--json` + `--watch` clash and the mode validation were fixed in the same
pass; this one was found while verifying them and is a separate decision. It is not obvious whether
the flag should filter departures client-side, filter which stops are considered, or be rejected
outright on this subcommand — and the three give noticeably different boards.

**What would unblock it.** A decision on the intended meaning. Journey Planner's `estimatedCalls`
does not take a transport mode argument, so honouring the flag means either filtering the returned
calls in `fetch_near`, or passing `filterByModes` to `nearest(...)` so mode selection picks the
stops rather than the departures. Rejecting it for `near` is also defensible and is the smallest
change.

**Where.** `src/main.rs` `cmd_near` (never reads `common.modes`), `src/entur/nearest.rs`
(`nearest_stops`, `departures` — neither query carries a mode filter), `src/entur/mod.rs`
(`validate_modes`), `src/cli.rs` (`Common::modes`, declared `global = true`).
