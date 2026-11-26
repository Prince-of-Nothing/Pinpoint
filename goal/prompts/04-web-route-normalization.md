# 04 — Web route UI: test route normalization edges

Read `goal/CONTEXT.md` first. Claimed by `<agent>`.

## Context

`web/app/route-ui.js` is the self-contained turn-by-turn component. Its
`normalizeRoute` has validation rules for distance/duration, points and
maneuvers, but it is private and only exercised through the demo flow in
`web/app/test/route-ui.mjs`. A bad backend payload can therefore reach the UI
unnoticed.

## Task

Export `normalizeRoute` and add module-level checks to `route-ui.mjs`:
- negative or non-finite `distance_m` / `duration_s` throws `TypeError`;
- out-of-range and non-numeric points are dropped;
- blank maneuver instructions are dropped, valid ones are trimmed;
- a negative maneuver `distance_m` becomes `null`;
- an empty `maneuvers` array is accepted and stays empty.

Keep `route-ui.js` self-contained: no imports, fetch, absolute URLs or tiles.

## Files

- `web/app/route-ui.js` — export `normalizeRoute`.
- `web/app/test/route-ui.mjs` — static checks only.

## Steps

1. Change the function declaration to an export.
2. Import the module in the existing static block of `route-ui.mjs`.
3. Add one `check(...)` per rule with a clear message.
4. Do not change the browser flow or the demo payload.

## Verify

```text
node web/app/test/route-ui.mjs --static-only
```

Expected: new `PASS` lines and exit 0; the browser flow is not required for this
change. Also run the repo host gate in `goal/CONTEXT.md`.

## Done

- `[INFRA] Web tests: pin route normalization edge cases` in `CHANGELOG.md`.
- Branch, issue, PR with `Closes #N`, green CI, squash merge.
- `powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 done -Id 04 -Agent <agent>`
