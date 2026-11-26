# 07 — Server smoke: cover the offline catalog endpoints

Read `goal/CONTEXT.md` first. Claimed by `<agent>`.

## Context

`scripts/smoke.ps1` exercises the auth, friendship, sharing, notes, check-in
and search paths against a running server. The offline map catalog/config
routes (`/map/catalog`, `/map/config`) are covered by Rust tests but not by the
end-to-end smoke, so a routing or serialization regression there can ship.

## Task

Add two checks to `scripts/smoke.ps1` after the existing search checks:
- `GET /map/catalog` returns an object with a non-empty `regions` array and
  each entry has `id` / `name`;
- `GET /map/config` returns `regions` and the `packInstalled` / `graphInstalled`
  booleans, without any absolute asset URL or secret.

Keep the script’s `Check` style and exit-code contract.

## Files

- `scripts/smoke.ps1` only.

## Steps

1. Read the `Check` helper and the end of the search section.
2. Add the requests with a short timeout and no session cookie unless the route
   actually needs one.
3. Assert the shape, not a specific region count.
4. Run it against the in-memory server on a free port.

## Verify

```text
powershell -ExecutionPolicy Bypass -File scripts/smoke.ps1 -Base http://127.0.0.1:8097
```

Expected: the new PASS lines and the existing 29 checks all green (31 total, or
the current count + the new checks). Then run the host gate from
`goal/CONTEXT.md`.

## Done

- `[INFRA] Smoke: check the offline map catalog and config` in `CHANGELOG.md`.
- Branch, issue, PR with `Closes #N`, green CI, squash merge.
- `powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 done -Id 07 -Agent <agent>`
