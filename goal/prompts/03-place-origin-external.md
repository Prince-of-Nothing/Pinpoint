# 03 — Place sheet: cover Set as origin and external maps

Read `goal/CONTEXT.md` first. Claimed by `<agent>`.

## Context

`src/ui/map.rs` exposes `PlaceSheetOutcome` ids for Directions, Save, Copy and
Share, and `src/ui.rs` has an interaction test for them. “Set as origin” and
the OSM / Google / Organic / Magic Earth buttons are still only covered by the
URL helper test; their click wiring is untested.

## Task

Expose `set_origin_id` and four external-link ids on `PlaceSheetOutcome`, then
extend `real_place_sheet_actions_return_their_actions` (or add a sibling test)
to click all five and assert `PlaceSheetAction::SetOrigin` and the exact
`OpenUrl` strings from `place_external_urls`.

## Files

- `src/ui/map.rs` — outcome fields and button ids.
- `src/ui.rs` — interaction test only.

## Steps

1. Capture ids in the `draw_place_sheet_view` action row.
2. Add the fields to the outcome and initialize them to `None`.
3. Add the clicks through the modal gate, using the existing press/release
   helpers.
4. Assert the four URLs exactly; do not only check the prefix.

## Verify

```text
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
cargo test --locked
```

Expected: the new clicks fail before the wiring and pass after; all existing
tests stay green.

## Done

- `[INFRA] Native tests: click the place sheet origin and external map actions`
  in `CHANGELOG.md`.
- Branch, issue, PR with `Closes #N`, green CI, squash merge.
- `powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 done -Id 03 -Agent <agent>`
