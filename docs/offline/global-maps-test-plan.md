# Global maps v1 — test plan

Pairs with `docs/superpowers/specs/2026-11-17-global-maps-design.md` and
ADR-0021. **P0 = must pass before merge. P1 = follow-up.** All fixtures are
synthetic or committed; no real user data or live account.

Tests this plan expects to exist: `src/region_catalog.rs` unit tests,
`tests/region_catalog.rs`, `tests/global_maps_e2e.rs`,
`web/map/test/regions.mjs`, `scripts/offline/catalog.ps1` +
`scripts/offline/tests/catalog.ps1`, `scripts/v2/check-global-maps.ps1`.

## P0 — Rust

| # | Case | Assertion | Command |
|---|------|-----------|---------|
| R1 | Catalog parse | Valid schema-1 JSON round-trips; `installed` flags added, `url` fields absent from `/map/catalog` | `cargo test --locked --test region_catalog` |
| R2 | Catalog validation | Reject: schema ≠ 1, id not `[a-z0-9-]{1,64}`, duplicate id, bbox not 4 finite numbers, non-http(s) URL, 0 size, non-hex/uppercase hash, >64 regions, >32 files, >1 MiB body. Each has one negative test | same |
| R3 | Two-region install | Moldova + Romania fixtures install into `packs/<id>/`; both files per region; a second job returns 409; unknown region 404; one job at a time | `cargo test --locked --test global_maps_e2e` |
| R4 | Hash/length failure | Wrong transfer hash, wrong installed hash and short body leave an existing good file untouched; `.part` cleaned; phase `failed` | `cargo test --locked --lib offline_install` |
| R5 | Legacy migration | Root `packs/moldova.pmtiles`/`.graph.bin` move to `packs/moldova/`; idempotent; no re-download; interrupted move keeps the source | `cargo test --locked --test global_maps_e2e legacy` |
| R6 | Range serving | `/packs/<region>/<file>` 200/206/416/HEAD, correct `Content-Range` and MIME; `/packs/<name>` legacy path still works; name not in that region's `installed_name` list, `..`, encoded traversal, other extensions → 404 | `cargo test --locked --lib map_server` + R3 file |
| R7 | Remote fallback | Fetch timeout/500/invalid JSON serves the bundled Moldova entry; download still starts from bundled URLs | `cargo test --locked --lib region_catalog` |
| R8 | Config | `/map/config` returns active region `pmtilesUrl`, `graphUrl`, bbox and `regions[]` with installed ids; no pack → Monaco fixture | `cargo test --locked --test global_maps_e2e` |
| R9 | URL policy | Catalog/asset URLs must be https on the release host; reject loopback, private/link-local, IP literal, userinfo and custom port; reject off-host or https→http redirects and >3 hops | `cargo test --locked --test region_catalog url` |
| R10 | Page boundary | Download accepts only `{"region":"<id>"}`; unknown/misspelled fields → 400; 2 GiB per-file cap; free-space preflight; symlinked region dir/target refused; `.part` never left | `cargo test --locked --test global_maps_e2e boundary` |

## P0 — data repo (`pinpoint-maps`)

| # | Case | Command |
|---|------|---------|
| D1 | Workflow parses and is dispatchable | `gh workflow view build-region.yml -R Prince-of-Nothing/pinpoint-maps --yaml` |
| D2 | Dry run | `gh workflow run build-region.yml -R Prince-of-Nothing/pinpoint-maps -f region=monaco -f app_ref=<sha> -f maxzoom=10 -f dry_run=true` then `gh run watch -R Prince-of-Nothing/pinpoint-maps` |
| D3 | Catalog generator/merge | `pwsh -NoProfile -File scripts/offline/tests/catalog.ps1` — two fixture regions; rebuild one keeps the other; rejects duplicate id, missing asset, bad hash, asset ≥ 2 GiB |
| D4 | Release assets | `gh release view data-moldova-20261117 -R Prince-of-Nothing/pinpoint-maps --json assets --jq '.assets[] \| select(.size > 2147483648) \| .name'` prints nothing; download + `Get-FileHash` matches `catalog.json` |

**Gap to resolve in P0:** the design lists only `region`, `app_ref`, `maxzoom`.
Add a `dry_run` boolean (default true for manual runs); otherwise D2 can only
be a real Monaco publish. Also define which repo owns the generator: it lives
in the app repo (`scripts/offline/catalog.ps1`) so D3 runs in the app gate.

## P0 — web/map

| # | Case | Assertion | Command |
|---|------|-----------|---------|
| W1 | Region list | Renders every catalog region with size and installed state; no hardcoded "Moldova map" | `node web/map/test/regions.mjs --static-only` |
| W2 | Download POST | Exactly `{"region":"<id>"}`, `Content-Type: application/json`; 202 starts, 409 shows "already running", 404 shows unknown region | `cargo test --locked --test global_maps_e2e -- --ignored --nocapture` |
| W3 | Progress | Bar advances within `expected_download_bytes`; verifying → installed; button hidden while running | same |
| W4 | Online toggle | Off by default; no external request while off; turning it on loads only OpenFreeMap hosts; local pack wins when installed | same + W1 |
| W5 | Style switch | Toggle off→on→off keeps pins and route; `pinpoint-locations-circles` source/layer and `pinpoint-route` re-added after each style load | same |
| W6 | CSP | Response header on `/map/index.html` has no `*`; only `https://tiles.openfreemap.org` plus verified style/glyph/sprite hosts; local pack path remains allowed | `cargo test --locked --lib map_server csp` |

**Design gap:** CSP is per-response, not per-toggle. Either send one CSP that
admits OpenFreeMap hosts (safe only because the page makes no request until
toggled), or serve a second page/variant. Test the chosen header, and pin "no
request while off" separately (W4).

## P0 — Android / desktop

- Loopback `/map/catalog`, `/map/config` and `/packs/<region>/<file>` range
  206 work in the app WebView (same assertions as R6/R8, exercised through
  `tests/global_maps_e2e.rs` on desktop and the device checklist).
- Storage: desktop `~/.pinpoint/packs/<region>/`; Android
  `/data/data/com.pinpoint.desktop/files/packs/<region>/`.
  `adb shell run-as com.pinpoint.desktop ls -l files/packs/moldova` shows
  `moldova.pmtiles` and `moldova.graph.bin`, no `.part`.
- Legacy: seed root-level files, launch, confirm they moved and were not
  re-downloaded.
- APK: `cd android; .\gradlew.bat :app:assembleDebug -Pabi=arm64-v8a`.

## P0 — live/lab (device, airplane mode)

1. `adb shell pm clear com.pinpoint.desktop`; install Moldova; route
   Chișinău → Bălți.
2. Install Romania; both dirs present; Moldova route still works.
3. `adb shell cmd connectivity airplane-mode enable`; force-stop and relaunch;
   both maps render, both routes work, no tile host request.
4. Delete Moldova in the UI; only `packs/romania/` remains; Romania routes.
5. Disable airplane mode. Repeat 1–4 on desktop.

## P1

Ukraine real build at capped `maxzoom` (< 2 GiB); update-in-place replaces one
region without touching others; interrupted download retry; catalog cache TTL
and stale-cache behavior; OpenFreeMap outage shows a clear error while local
packs keep working; large-catalog render time; iOS; web per-region offline.

## Gate integration

- **`scripts/v2/check.ps1` (every PR, no network):**
  - add `web/map/test/regions.mjs` to the explicit `$webTests` list (the
    auto-glob only covers `web/app/test`);
  - add `"catalog"` to `$tests` in `scripts/offline/tests/run.ps1`;
  - new Rust unit/integration tests are already picked up by
    `cargo test --locked`; no other gate change.
- **New `scripts/v2/check-global-maps.ps1` (opt-in, not CI):**
  `-E2E` runs `cargo test --locked --test global_maps_e2e -- --ignored --nocapture`;
  `-Data` runs D1/D4 read-only checks; `-Dispatch` runs D2; `-Android` runs the
  Gradle build; default with no switch prints the lab checklist and skips.
  This keeps network, GitHub and emulator dependencies out of the merge gate.

## P0 command summary

```powershell
cargo test --locked --test region_catalog
cargo test --locked --test global_maps_e2e
cargo test --locked --lib offline_install
cargo test --locked --lib region_catalog
cargo test --locked --lib map_servernode web/map/test/regions.mjs --static-only
cargo test --locked --test global_maps_e2e -- --ignored --nocapture
pwsh -NoProfile -File scripts/offline/tests/catalog.ps1
pwsh -NoProfile -File scripts/v2/check.ps1
cd android; .\gradlew.bat :app:assembleDebug -Pabi=arm64-v8a
```
