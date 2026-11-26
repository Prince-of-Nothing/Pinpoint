# 08 — Offline packs: add a safe uninstall route

Read `goal/CONTEXT.md` first. Claimed by `<agent>`.

## Context

`src/offline_install.rs` installs PMTiles region packs under
`packs/<region>/` and serves them through the loopback map server. Once
installed, a region cannot be removed; `docs/offline/global-maps.md` lists this
as a v1 gap. A delete route must never accept a path, only a validated region
id, and must not remove a pack while its download is running.

## Task

Add `DELETE /map/offline/:region` that:
- accepts only an id that passes `valid_region_id` and exists in the catalog;
- refuses with `409` while that region’s download job is active (or while any
  job is active if the installer is single-job);
- removes only `packs/<region>/` below the configured packs directory and
  leaves other regions and the legacy root files alone;
- returns `204` on success and `404` for an unknown region.

Mount it on the loopback map server next to the download route. Add a test that
creates a fake region directory, deletes it, and asserts another region and a
`../` path are untouched.

## Files

- `src/offline_install.rs` — handler, validation, removal helper, test.
- `src/mobile/map_server.rs` — route registration and test.
- `src/server/mod.rs` — only if the main map router must expose it too.

Do not add a UI button in this prompt; that is a follow-up.

## Steps

1. Add the delete handler and a validated removal helper in
   `offline_install.rs`.
2. Refuse if the installer is busy; map unknown ids to `404`.
3. Mount the route and add the filesystem-safety test.
4. Do not touch a running download or a sibling region.

## Verify

```text
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
cargo test --locked
```

Expected: the new route test passes; existing offline tests stay green.

## Done

- `[FEATURE] Offline packs: remove an installed region` in `CHANGELOG.md`.
- Branch, issue, PR with `Closes #N`, green CI, squash merge.
- `powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 done -Id 08 -Agent <agent>`
