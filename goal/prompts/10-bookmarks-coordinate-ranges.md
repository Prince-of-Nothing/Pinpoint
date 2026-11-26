# 10 — Saved places: validate latitude and longitude ranges

Read `goal/CONTEXT.md` first. Claimed by `<agent>`.

## Context

`src/ui/bookmarks.rs` lets the user add a saved place. The code parses
`bookmark_lat` / `bookmark_lon` as `f64` but never checks the WGS84 ranges, so
`200, 200` is stored. `crate::security::validation::validate_location` already
provides the server-side bounds and reasons.

## Task

Add a pure helper in `src/ui/bookmarks.rs` that trims the inputs, parses both
numbers, runs `validate_location(lat, lon, None)`, and returns the parsed pair or
a short error. Use it for the “Save” action and show the error via `set_error`.
Add tests for a valid pair, out-of-range latitude, out-of-range longitude and
non-numeric input. Do not change the on-disk `Bookmark` format.

## Files

- `src/ui/bookmarks.rs` only.

## Steps

1. Add `parse_bookmark_location(lat: &str, lon: &str) -> Result<(f64, f64), String>`.
2. Use it before `store.add`; keep the default name behaviour.
3. Add the focused tests in the module.

## Verify

```text
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
cargo test --locked
```

Expected: the invalid pairs no longer save; the new tests pass; existing
bookmark tests stay green.

## Done

- `[FIX] Native saved places: validate coordinate ranges` in `CHANGELOG.md`.
- Branch, issue, PR with `Closes #N`, green CI, squash merge.
- `powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 done -Id 10 -Agent <agent>`
