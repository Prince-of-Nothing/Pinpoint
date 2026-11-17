# Pinpoint global maps v1 — worldwide online + per-country offline

**Status:** Proposed (owner gave the data repo; build in progress)
**Date:** 2026-11-17
**Scope:** Android/desktop map window: catalog-driven per-country offline
basemap + car routing. Web: worldwide online basemap. Web per-region offline is
a follow-up.
**Related:** `2026-11-16-offline-maps-design.md`, ADR-0020 (MapLibre/PMTiles),
data repo `Prince-of-Nothing/pinpoint-maps`.

## 1. Intent

Make Pinpoint usable anywhere: open the map online and see the world, or
download a country once and browse + route offline. Adding a country must not
need an app release. Map data lives in `pinpoint-maps`, not in the app repo.

## 2. Current state (facts)

- `scripts/offline/build.ps1 -Region <id>` already builds PMTiles, FTS5 search
  and Valhalla tiles from a Geofabrik extract; `regions.json` has Moldova and
  Monaco.
- `tools/native-router` builds `graph.bin` from the same PBF; Moldova's graph is
  110,858,815 B, 45,338,586 B gzipped.
- `src/offline_install.rs` is hardcoded to one Moldova data release, file names,
  sizes and hashes. `/map/config`, `/map/offline/status`, `/map/offline/download`
  and `/packs/:name` assume one pack.
- `web/map/index.html` says “Moldova map and routing graph”; one offline panel.
- `Prince-of-Nothing/pinpoint-maps` exists, public, empty (initial commit only).
- The web app has a single-pack OPFS demo, not a multi-region manager.

## 3. Goals

- A catalog of regions; the mobile map lists them with size, installed state,
  download/update/delete/show.
- Multiple installed regions under `packs/<id>/`; the map uses the selected
  region and serves its PMTiles with Range requests.
- Legacy v2.3.0 root-level Moldova files are detected and migrated without a
  re-download.
- Worldwide online basemap, **opt-in** (OpenFreeMap Liberty style), with visible
  OSM/OpenFreeMap attribution and a privacy note. Off by default.
- `pinpoint-maps` owns `catalog.json`, release assets and the build workflow;
  the app fetches the catalog at runtime and falls back to a bundled Moldova
  entry when offline.
- v1 regions: Moldova, Romania, Ukraine. Any Geofabrik region can be built
  on demand from the data repo.
- No planet bundle, no all-country build, no cross-border routing, no online
  turn-by-turn outside downloaded packs, no web per-region offline in v1.

## 4. Decisions

1. **Hosting:** GitHub Releases in `pinpoint-maps` (2 GiB per asset, free). A
   normal git folder is rejected (100 MB/file). Object storage (R2) stays a
   later URL change; the catalog hides it.
2. **Online source:** OpenFreeMap public vector tiles, opt-in. Self-hosted
   low-zoom world or paid planet CDN are deferred.
3. **Catalog:** `catalog.json` on `main` of `pinpoint-maps`, fetched by the Rust
   server and cached; the app ships a small bundled fallback.
4. **Storage:** `packs/<region>/<file>`; one active download job at a time.
5. **Large regions:** cap `maxzoom` so each asset stays under 2 GiB; splitting
   is a later catalog feature (`parts`).

## 5. Catalog contract (schema 1)

```json
{
  "schema": 1,
  "updated": "2026-11-17T12:00:00Z",
  "regions": [{
    "id": "moldova",
    "name": "Moldova",
    "bbox": [26.6, 45.4, 30.2, 48.5],
    "maxzoom": 14,
    "version": "2026-11-17",
    "files": [{
      "role": "basemap",
      "transfer_name": "moldova.pmtiles",
      "installed_name": "moldova.pmtiles",
      "url": "https://github.com/Prince-of-Nothing/pinpoint-maps/releases/download/data-moldova-20261117/moldova.pmtiles",
      "transfer_bytes": 93172399,
      "transfer_sha256": "7b2715...",
      "installed_bytes": 93172399,
      "installed_sha256": "7b2715...",
      "compression": "identity"
    }, {
      "role": "graph",
      "transfer_name": "moldova.graph.bin.gz",
      "installed_name": "moldova.graph.bin",
      "url": "https://github.com/Prince-of-Nothing/pinpoint-maps/releases/download/data-moldova-20261117/moldova.graph.bin.gz",
      "transfer_bytes": 45338586,
      "transfer_sha256": "276d0d...",
      "installed_bytes": 110858815,
      "installed_sha256": "8cdbb0...",
      "compression": "gzip"
    }]
  }]
}
```

Validation: schema 1; id `[a-z0-9-]{1,64}`; unique ids; bbox 4 finite numbers;
http(s) URLs; positive sizes; 64 lowercase hex hashes; max 64 regions and
32 files per region; catalog response capped at 1 MiB.

## 6. App behavior

- `GET /map/catalog`: server returns the cached catalog with `installed` flags
  and without asset URLs; if the remote fetch fails, the bundled fallback.
- `POST /map/offline/download` `{ "region": "<id>" }`: starts that region's
  files in order; 202 or 409. Missing region -> 404.
- `GET /map/offline/status`: existing shape plus `region`; one active job.
- `GET /map/config`: active installed region's `pmtilesUrl`, graph URL and bbox;
  `regions` array of installed packs. No pack -> Monaco fixture.
- `GET /packs/:region/:name` plus legacy `GET /packs/:name`, both Range-aware,
  canonicalized under `packs/`, same MIME and security headers as today.
- Legacy migration on first status/config: move `packs/moldova.pmtiles` and
  `packs/moldova.graph.bin` into `packs/moldova/`.
- Installer verifies transfer and installed hashes/lengths, writes
  `.part` files, atomically renames, never replaces a good file on failure.
- Online: a toggle in the map page loads the OpenFreeMap Liberty style; off by
  default; the local pack style wins when a pack is active. Re-add pins/routes
  after style load. CSP allows only `https://tiles.openfreemap.org` (and its
  style/glyph/sprite hosts as verified) when the toggle is on.

## 7. Data workflow (`pinpoint-maps`)

- `build-region.yml`, `workflow_dispatch` inputs: `region`, `app_ref`,
  `maxzoom` (optional). Steps: checkout `Prince-of-Nothing/Pinpoint` into
  `app/`, install Rust + Docker, run `scripts/offline/build.ps1`, build
  `native-router` graph, gzip, hash, create release
  `data-<region>-<YYYYMMDD>`, upload assets, regenerate `catalog.json`, commit.
- Curated schedule: Moldova, Romania, Ukraine weekly; everything else on
  demand. The workflow has `contents: write` on its own repo and no secrets.
- Each release carries `ATTRIBUTION.md` (OSM/ODbL, Geofabrik, Planetiler).
- The app repo remains the source of build logic; the data repo pins `app_ref`
  so a catalog entry states which scripts built it.

## 8. Work plan (parallel slices)

1. `src/region_catalog.rs` + multi-region `src/offline_install.rs` + tests.
2. Map-server integration (`src/map_server.rs`, `src/mobile/map_server.rs`) and
   CSP for OpenFreeMap.
3. `pinpoint-maps`: README, catalog, workflow.
4. Offline scripts: graph build/gzip + catalog generator + region entries.
5. Mobile map UI: region list, sizes, download/update/delete/show, online
   toggle.
6. Research: OpenFreeMap terms/URLs; GitHub/Geofabrik limits and sizes.
7. ADR-0021 + docs + changelog.
8. Verification: Rust gate, browser E2E, data workflow dry run, Android APK.

## 9. Risks

- GitHub Releases is not a CDN; public traffic may be throttled. The catalog
  makes moving to R2 a data change.
- `build.ps1` on Linux/pwsh and Docker in Actions is unproven; dry-run Moldova
  first.
- OpenFreeMap is a third party; the toggle is opt-in and disclosed.
- Large-country assets may exceed 2 GiB; cap maxzoom until parts land.
