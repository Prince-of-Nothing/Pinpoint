# 01 — Native navigation: distance to the next turn

Read `goal/CONTEXT.md` first. Claimed by `<agent>`.

## Context

`src/ui/route.rs` follows a route with `RoutePlan.next_step`, which advances
within 40 m of `RouteStep.location`. `RoutePlan::navigation_status` currently
returns only `Next: <instruction> (<destination/stop>)`. `tick_navigation` has
the GPS fix every second and already updates the map and next stop.

## Task

Show the remaining distance to the next maneuver in the banner, for example
`Next: Turn right (destination, 250 m)`. Add `nav_distance_m: Option<f64>` to
`RoutePlan`, set it in `tick_navigation` from the fix to
`steps[next_step].location` using the existing `haversine_m`, and append it with
`format_distance`. Clear it when not following, when the step has no location,
or when a new/alternative route is selected.

## Files

- `src/ui/route.rs` — field, defaults, resets, tick, banner, tests.

## Steps

1. Add the field and reset it wherever `next_step` resets.
2. In `tick_navigation`, compute the distance to the next located step.
3. Extend `navigation_status` without changing the existing “Heading to …”
   fallback.
4. Add a test for a located next step and a test for a missing location.

## Verify

```text
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
cargo test --locked
```

Expected: the new banner tests pass; all existing tests stay green.

## Done

- `[FEATURE] Native navigation: show the distance to the next maneuver` in
  `CHANGELOG.md`.
- Branch, issue, PR with `Closes #N`, green CI, squash merge.
- `powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 done -Id 01 -Agent <agent>`
