## Why

<!-- The problem this solves. Link any related issue. -->

## What

<!-- What changed. -->

## Verification

<!-- Delete the rows that don't apply. -->

- [ ] `cargo test` passes locally
- [ ] `cargo clippy --all-targets -- -D warnings` is clean
- [ ] `cargo fmt --all --check` is clean
- [ ] Touched the Entur client: checked the change against a live response, not only fixtures
- [ ] Touched `src/location.rs`: verified with `ruter where` from an installed app bundle
- [ ] Docs only, no verification needed
