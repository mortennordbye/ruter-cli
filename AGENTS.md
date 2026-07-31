# AGENTS.md

Guidance for coding agents (Claude Code and others) working in this repository.
See [CLAUDE.md](CLAUDE.md) for the fuller working agreement.

## Commands

| Task | Command |
| ---- | ------- |
| Build | `cargo build` |
| Test | `cargo test` |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| Format | `cargo fmt --all` (check with `--check`) |
| Install (macOS) | `./scripts/install-macos.sh` |

The verification gate before calling anything done:

```bash
cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test
```

## Layout

- `src/entur/` — client for Entur's Journey Planner v3 GraphQL API and geocoder
- `src/render/` — one-shot terminal board: colours, time formatting, line badges
- `src/watch.rs` — the `--watch` ratatui view and its background poller
- `src/location.rs` — position resolution chain, incl. macOS Core Location
- `src/config.rs` — TOML config at `~/.config/ruter/config.toml`
- `macos/Info.plist` — embedded into the binary by `build.rs` for Location Services
- `tests/fixtures/` — recorded Entur responses; the test suite never hits the network

## Things that will bite you

- **Journey Planner v3 has no `maximumWalkDistance`.** Walking is capped by *duration*
  via `maxAccessEgressDurationForMode` (ISO-8601, e.g. `PT15M`).
- **GraphQL errors arrive with HTTP 200.** Read the body; never branch on status alone.
- **Departures are ordered by `expectedDepartureTime`, not `aimed`.** A delayed service is
  interleaved by when it will actually leave. There is a test pinning this.
- **Wire types are permissive on purpose.** `line`, `publicCode` and `presentation` are all
  independently nullable. Never `unwrap()` deserialized data.
- **GPS needs the app bundle.** A bare binary has no bundle identity, so macOS never
  prompts and Core Location silently returns nothing. Changes to `src/location.rs` must be
  verified via `scripts/install-macos.sh` then `ruter where` — not `cargo run`.
- **Fixtures are frozen recordings.** Re-recording them changes the timestamps every
  assertion depends on; update the assertions in the same commit.

## Conventions

- Commits follow Conventional Commits — release-please computes versions from them, and
  this repo has no PR-title check to catch mistakes.
- User-facing strings are Norwegian; code, comments and docs are English.
- Never commit secrets or credentials.
