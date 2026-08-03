# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Working approach

These guidelines bias toward caution over speed. For trivial tasks, use judgment.

### Think before coding

Don't assume. Don't hide confusion. Surface tradeoffs.

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them — don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

### Simplicity first

Minimum code that solves the problem. Nothing speculative.

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

### Surgical changes

Touch only what you must. Clean up only your own mess.

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it — don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: every changed line should trace directly to the user's request.

### Goal-driven execution

Define success criteria. Loop until verified.

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

### Track unfinished work in BACKLOG.md

If you leave anything unfinished, partially implemented, or explicitly defer it, add an entry to `BACKLOG.md` in the repo root before reporting the task done. Don't bury deferrals in chat — they vanish next session.

Each entry needs four things: **what** the work is, **why** it was deferred, **what would unblock it**, and **where** the relevant code lives (file paths). Read existing entries for the format.

Don't put work-in-progress on `BACKLOG.md` — WIP belongs on a branch. The backlog is for *known gaps the team has agreed to leave for later*. If you finish an item, delete it.

What counts as "unfinished":
- Tier 1 / Tier 2 splits where you only shipped Tier 1.
- Out-of-scope items you noticed but didn't fix.
- Features behind a feature flag that still need ramping or cleanup.
- Tests skipped, mocks left in, debug logging not yet stripped.
- TODO comments you wrote (write the entry instead — TODOs rot in code).

What does NOT belong:
- Forward-looking ideas the user didn't agree to defer ("we could also..."). Either do them or drop them.
- Codebase-wide debts that pre-existed your work and the user didn't ask you to track.

### No AI attribution in commits

Commits and PRs read as the human author's. No AI fingerprint, ever.

- No `Co-Authored-By` trailer naming Claude or any AI.
- No session links or IDs (e.g. a `Claude-Session:` trailer).
- No "Generated with Claude Code", 🤖 emoji, or similar tool signatures in commit messages, PR descriptions, or issue bodies.
- Describe the change, not the tool that produced it.

These guidelines are working if: fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions come before implementation rather than after mistakes.

## Development

```bash
# build:   cargo build
# test:    cargo test              (single test: cargo test <name>)
# lint:    cargo clippy --all-targets -- -D warnings
# format:  cargo fmt --all
# run:     cargo run -- <args>     e.g. cargo run -- --from jobb hjem
```

No env vars and no `.env` file. Runtime configuration lives in
`~/.config/ruter/config.toml` (respecting `$XDG_CONFIG_HOME`) and is created by
`ruter config add`. To exercise a throwaway config without touching your real one:

```bash
XDG_CONFIG_HOME=$(mktemp -d) cargo run -- config add hjem "Dronningens gate 40, Oslo" --yes
```

**On containers:** the blueprint's "develop inside containers" guidance does not apply
here and is deliberately omitted. This is a macOS-native binary whose main feature —
Core Location — cannot work in a container. The Linux build is validated by CI, not
locally.

**Installing for real work on macOS:** `./scripts/install-macos.sh`. GPS does not work
from `cargo run`; see the Safety rules below.

## Before reporting a task complete

```bash
cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test
```

Run it even when the change "looks obviously correct". Skip only for doc-only changes.

Additionally, when the change touches:
- **`src/entur/`** — check the change against a live API response, not only the recorded
  fixtures. The fixtures cannot catch a query the server would reject.
- **`src/location.rs`** — reinstall via `scripts/install-macos.sh` and confirm with
  `ruter where`. `cargo run` produces an unbundled binary that can never get a GPS fix,
  so a broken location change looks identical to a working one.
- **`src/render/` or `src/watch.rs`** — eyeball the real output. `cargo test` checks the
  strings, not whether the columns line up.

## Security baseline

**Skipped, deliberately.** The blueprint's security baseline covers projects with a
network, auth, or data surface. `ruter` is a read-only CLI: it has no server, no auth, no
sessions, no database, no user-supplied input crossing a trust boundary, and stores no
credentials. It makes outbound HTTPS calls to Entur's public API and, on fallback, to
ipinfo.io.

The two rules that do still carry over:
- **Wire data is hostile.** Everything deserialized from Entur is `Option` + `serde(default)`
  and must never be `unwrap()`ed. A malformed colour or a null `publicCode` must degrade,
  not panic.
- **Location data is personal data.** Never log, transmit, or persist coordinates beyond
  the Entur request that needs them. Saved places in the config are the user's explicit choice.

## Architecture

A single Rust binary (`ruter`) targeting Rust 1.88+, edition 2024. Blocking HTTP via
`ureq` with rustls — deliberately no async runtime, since the tool makes one or two
requests per invocation. `clap` for the CLI, `serde`/`serde_json` for the wire types,
`chrono` for time, `ratatui` for the `--watch` view, `objc2-core-location` on macOS only.

Data source is [Entur](https://developer.entur.org/) Journey Planner v3 (GraphQL) plus
its geocoder (REST/GeoJSON). Open NLOD licence, no API key, but the `ET-Client-Name`
header is mandatory.

### Data flow rules

`main.rs` wires everything and is the only place that combines config, location, client,
and renderer. Flow: resolve config → resolve origin and destination coordinates →
one GraphQL query → deserialize into wire structs → render to a `String` → write to stdout.

Rules:
- Wire structs live in `src/entur/` and stay permissive. Do not tighten them to make
  rendering convenient; Entur's optionality is real.
- Rendering is pure: it takes data plus a `now: DateTime<FixedOffset>` and returns a
  `String` (or ratatui `Line`s). It never reads the clock or the network itself — that is
  what makes it testable.
- All stdout goes through `emit()` in `main.rs`, which swallows `BrokenPipe` so
  `ruter hjem | head` does not panic.

### Safety rules for AI-assisted changes

- **Never verify a Core Location change with `cargo run`.** A bare binary has no bundle
  identity, so macOS never prompts and Core Location silently returns nothing — failure
  and success look identical. Use `scripts/install-macos.sh` then `ruter where`.
- **`CFBundleIdentifier` in `macos/Info.plist` is load-bearing.** The Location Services
  grant is keyed to it. Changing it silently revokes the user's existing permission.
- **Don't re-record `tests/fixtures/` casually.** Every timing assertion is pinned to the
  recorded timestamps. If you re-record, update the assertions in the same commit and say so.
- **Respect `watch_interval_secs` and its 10-second floor.** Entur is a free public API;
  a tight refresh loop gets the user rate-limited.
- **`--json` output is a public interface.** Changing field names breaks anyone scripting
  against it.
- **`scripts/install.sh` on `main` is live.** It is what the README's curl one-liner and
  every installed `ruter upgrade` fetch and execute. A break there is not caught by a
  release; it breaks upgrades for people who already have the tool. Test it against a real
  tag with `RUTER_BIN_DIR` and `RUTER_APP_DIR` pointed at a temp dir before merging.

### Environment variables

None. This project deliberately has no `.env` file and no `.env.example`; there is nothing
secret to configure. The only environment inputs it reads are standard ones:
`XDG_CONFIG_HOME` (config location), `NO_COLOR` (disable colour), and `TERM_PROGRAM`
(naming the terminal in error hints).

### Directory layout

```
src/
├── main.rs        CLI dispatch, exit codes, stdout handling
├── cli.rs         clap definitions
├── config.rs      TOML config + saved places
├── location.rs    position resolution chain (flag -> place -> GPS -> IP -> default)
├── watch.rs       --watch ratatui view and background poller
├── upgrade.rs     `ruter upgrade`: version check, delegates install to scripts/install.sh
├── entur/         Journey Planner v3 client, trip/nearest/geocode queries
└── render/        one-shot board: colours, badges, time formatting
macos/             Info.plist embedded by build.rs for Location Services
scripts/           install.sh (curl installer, also run by `ruter upgrade`),
                   install-macos.sh (build from source + app bundle)
tests/fixtures/    recorded API responses; the suite never hits the network
```

### Key patterns

- **`Style`** (`src/render/mod.rs`) carries a single `colour: bool` and every colouring
  helper hangs off it. `Style::plain()` in tests gives escape-free output to assert on.
- **`Rgb::readable_text`** applies a WCAG contrast check to Entur's declared `textColour`
  and overrides it when illegible. Always route badge colours through it.
- **`place_name()`** substitutes Entur's literal `"Origin"` / `"Destination"` placeholders.
  Any new leg rendering must use it or those strings leak to the user.
- **`State<T>`** in `watch.rs` degrades `Ready` → `Stale` on a fetch error so the board
  keeps showing the last good data instead of blanking. Preserve that on any change.
- **Norwegian for users, English for code.** User-facing strings, help text, and errors are
  Norwegian; identifiers, comments, and docs are English.

### Code quality

- **Reuse before adding** — check shared utilities and components before writing new ones.
- **Prefer established frameworks over reinventing** — reach for a well-maintained, widely-used library before hand-rolling HTTP, dates, argument parsing, and the like. Only build your own when no good option fits, and say why.
- **Use current, supported versions** — pick libraries that are actively maintained and pull a recent, supported release. An unmaintained library is a security and upgrade liability.
- **No dead code** — if a flag has no effect, implement or remove it.
- **No premature abstractions** — only extract a helper when it's used in 2+ places.
