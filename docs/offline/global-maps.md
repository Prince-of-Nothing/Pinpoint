# Global maps (v1)

**Status:** v1 proposed · 2026-11-17 · Design:
[2026-11-17-global-maps-design.md](../superpowers/specs/2026-11-17-global-maps-design.md) ·
Contract: [ADR-0021](../adr/0021-global-map-catalog.md)

Pinpoint shows a worldwide basemap online and can download a country for
offline browsing and car routing. Map data is published from `pinpoint-maps`,
so adding a region is a data change, not an app release.

## What v1 does

- **Online basemap, opt-in.** The map page has a toggle for the OpenFreeMap
  Liberty style. It is off by default, and an installed pack wins when active.
- **Per-country offline packs.** The map lists catalog regions with size and
  installed state; the user can download, update and show one. Several regions
  can be installed; one download runs at a time. v1 has no delete route or
  button.
- **"This area" coverage hint.** Panning the map names the catalog region under
  the view centre with its transfer size and a Download/Show button; it hides
  while a download runs and when that region is already the active source.
- **Data-driven catalog.** Adding a country means building and publishing it in
  `pinpoint-maps`; no Rust change and no app release.
- **Clients.** Android and desktop get online and offline maps. The web app
  gets the online basemap in v1; web per-region offline is a follow-up
  ([web-offline-regions.md](./web-offline-regions.md)).

## Where the data lives

Map data lives in the public
[`Prince-of-Nothing/pinpoint-maps`](https://github.com/Prince-of-Nothing/pinpoint-maps)
repository. `catalog.json` on `main` is the source of truth. Each build is an
immutable, public release `data-<region>-<YYYYMMDD>` carrying the assets and
`ATTRIBUTION.md`. Assets are release files, not git files: GitHub rejects files
over 100 MB and caps one release asset at 2 GiB.

Catalog schema 1 lists, per region, `id`, `name`, `bbox`, `maxzoom`, `version`
and `files[]`. Each file declares a `role` (`basemap` or `graph`), transfer and
installed names, URL, sizes, SHA-256 hashes, and `compression` (`identity` or
`gzip`). The app accepts at most 64 regions, 32 files each, a 1 MiB response
cap, ids matching `[a-z0-9-]{1,64}`, https URLs on an allowlisted release host,
and 64-character lowercase hashes.

How the app gets a pack:

1. The Rust server fetches `catalog.json` at runtime, validates it and caches
   it. A malformed catalog, duplicate id/role, older `updated` timestamp or bad
   hash is rejected whole; the last good catalog or bundled fallback stays.
2. `GET /map/catalog` serves the cached catalog with `installed` flags and
   without asset URLs, so the page cannot choose a URL, size or hash.
3. `POST /map/offline/download {"region":"<id>"}` starts that region's files in
   order (202; 409 if busy; 404 if unknown). Each asset streams into
   `packs/<region>/.<name>.part`; the server checks transfer length and SHA-256,
   decompresses if needed, checks installed length and SHA-256, then atomically
   renames. A bad or partial file is deleted and never replaces a working one.
4. `GET /map/offline/status` reports the one active job and its `region`.
   `GET /map/config` returns the active region's PMTiles URL, graph URL, bbox
   and installed regions; with no pack it uses the bundled Monaco fixture.
   `/packs/:region/:name` serves installed files with Range support.

## Adding or refreshing a region

1. Add an entry to `scripts/offline/regions.json` in the app repo: Geofabrik
   URL, pinned source size and SHA-256, bbox, `maxzoom`, build images.
2. In `pinpoint-maps`, run `build-region.yml` (`workflow_dispatch`: `region`,
   `app_ref`, optional `maxzoom`). It checks out the app at `app_ref`, builds the
   PMTiles basemap and graph with `scripts/offline/build.ps1`, gzips and hashes
   the graph, creates `data-<region>-<YYYYMMDD>`, uploads the assets plus
   `ATTRIBUTION.md`, then regenerates and commits `catalog.json`.
3. Review the catalog diff; the app picks it up on its next catalog refresh.
   Never overwrite a release: a rebuild is a new date, version and entry.

The 2 GiB per-asset limit is binding. If a region does not fit, lower its
`maxzoom`; splitting into `parts` is not in v1. The graph is usually the large
asset: it grows with road length and barely shrinks with `maxzoom`. Moldova is
the reference (93 MB PMTiles + 45 MB gzipped graph: 139 MB transfer, 204 MB
installed), with about 1 GB staging and Docker for Planetiler/Valhalla. Build
minutes scale with extract size. Moldova, Romania and Ukraine are curated
weekly; others are built on demand. Measured sizes:
[region-inventory.md](./region-inventory.md).

## App storage and migration

```text
<app data>/packs/
  moldova/
    moldova.pmtiles
    moldova.graph.bin
  romania/
    ...
```

Files land at `packs/<region>/<installed_name>` from the catalog. One job runs
at a time (in-process lock plus `packs/.lock`). An update replaces files only
after verification, and a failed update leaves the old pack usable. With no
delete route in v1, removing a region means deleting its directory by hand.

Version 2.3.0 and earlier keep the Moldova files at the root of `packs/`. On the
first status or config request they are moved into `packs/moldova/` after
checking against the bundled Moldova entry. Migration is idempotent: if it
cannot complete, the old layout keeps serving and nothing is re-downloaded.

## Online mode and privacy

The toggle is off by default. When on, MapLibre loads the Liberty style and
tiles from `https://tiles.openfreemap.org`; the page CSP admits that host only
while online mode is active. An installed pack style wins over it.

OpenFreeMap then sees the client IP, user agent and requested `z/x/y` tiles;
tile coordinates reveal the approximate area being viewed. There are no
accounts, cookies or API keys, and Pinpoint sends no session token, account data
or friend location. The UI must show **OpenFreeMap © OpenMapTiles Data from
OpenStreetMap** while online. Offline packs contact no tile host. Details:
[openfreemap.md](./openfreemap.md).

Offline pack data is ODbL: keep **© OpenStreetMap contributors · OpenMapTiles ·
ODbL 1.0** visible, ship `ATTRIBUTION.md` per release, and keep releases
available while shipped clients may fetch them
([odbl-attribution.md](./odbl-attribution.md)).

## Limits in v1

- **Car routing only.** There is no pedestrian or bicycle profile and no online
  turn-by-turn; routing works only inside a downloaded pack.
- **No cross-border routing.** Start and destination must both be in the active
  region; installing two adjacent packs does not join their graphs.
- **No web per-region offline.** The browser keeps its single-pack demo.
- **No planet or all-country bundle.** Each region is chosen by the user.
- **GitHub Releases is not a CDN.** No SLA, possible throttling. Moving to
  object storage later changes URLs in `catalog.json`, not the app.

## Troubleshooting

| Symptom | Cause | What to do |
|---|---|---|
| Only Moldova appears, or a new region never does | Catalog fetch/validation failed; bundled Moldova fallback in use | Check GitHub connectivity, retry later. Installed packs still work offline. |
| Hash or size mismatch | Asset does not match the catalog; `.part` discarded, old pack kept | Retry once. If it repeats, fix the release and catalog. Never bypass verification. |
| "Not enough space" | Free space below installed size plus 10% headroom; updates keep the old pack until verified | Free space, or remove an installed region's directory, then retry. |
| Blank map, or Monaco after a region's files are removed | No active installed pack; `/map/config` falls back to the fixture | Download or show an installed region. |
| Online map is blank | Online mode is off, or OpenFreeMap is unavailable (no SLA) | Use an installed pack, or enable the toggle where online tiles are acceptable. |

## References

- Design:
  [2026-11-17-global-maps-design.md](../superpowers/specs/2026-11-17-global-maps-design.md)
- Decision: [ADR-0021](../adr/0021-global-map-catalog.md)
- Data repository:
  [`Prince-of-Nothing/pinpoint-maps`](https://github.com/Prince-of-Nothing/pinpoint-maps)
- [OpenFreeMap research](./openfreemap.md) ·
  [catalog security](./catalog-security.md) ·
  [region inventory](./region-inventory.md) ·
  [Moldova reference](./moldova.md)
