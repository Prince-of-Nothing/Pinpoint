# 0021. Global map catalog and manifest-driven offline regions

Date: 2026-11-17

## Status

Proposed

Extends [ADR-0020](./0020-hybrid-maplibre-offline-maps.md) — the map
subsystem keeps MapLibre + PMTiles + the native router; this ADR decides where
map data lives and how a region is added.

## Context

ADR-0020 delivered exactly one offline region. `src/offline_install.rs` is
hardcoded to a single Moldova data release: one set of file names, transfer and
installed sizes, and SHA-256 hashes. `/map/config`,
`/map/offline/status`, `/map/offline/download` and `/packs/:name` all assume
that one pack, and the map page advertises "Moldova map and routing graph".

That is enough for the thesis demo, not for users outside Moldova. Pinpoint
needs two things at once: a worldwide basemap while online, and a country that
can be downloaded once and then browsed and routed offline. Adding a country
today means editing hashes in Rust and shipping an app release, which couples
data freshness to the release train and makes every new region a code change.

`scripts/offline/build.ps1 -Region <id>` already builds PMTiles, the FTS5
search index and the routing graph from any Geofabrik extract, so the build
side is not the blocker — discovery and hosting are. `regions.json` only lists
Moldova and Monaco, and the public `Prince-of-Nothing/pinpoint-maps` repository
exists but is empty apart from its initial commit.

The constraints that shape the answer:

- Release assets are limited to 2 GiB, so a planet or even a large-country pack
  at high zoom does not fit in one file.
- A normal git folder is worse: GitHub rejects files over 100 MB.
- App storage on phones is measured in gigabytes shared with everything else,
  so users must choose regions, not receive all of them.
- Offline data is ODbL-licensed and changes weekly; whatever we choose must be
  rebuildable without touching the app repository.
- The online basemap is the first third-party tile host in the map path, so it
  has to be opt-in and disclosed.

## Decision

Map data becomes a versioned, remotely described catalog. The app is a
catalog-driven installer rather than the owner of a single pack's coordinates.

- **Runtime catalog.** `pinpoint-maps` owns `catalog.json` (schema 1) on
  `main`. The Rust server fetches it at runtime, validates it, caches it, and
  serves `GET /map/catalog` with per-region `installed` flags and without asset
  URLs. If the fetch fails, the app falls back to a small bundled Moldova
  entry, so a fresh install with no network still has a working map story.
  Validation: schema 1, id `[a-z0-9-]{1,64}`, unique ids, a bbox of four finite
  numbers, http(s) URLs, positive sizes, 64-character lowercase hex hashes, at
  most 64 regions and 32 files per region, and a 1 MiB response cap.
- **Manifest-driven multi-region installer.** Each region lists its files with
  a `role` (`basemap`, `graph`), a transfer name, an installed name, URL,
  transfer/installed byte counts and SHA-256 hashes, and a `compression`
  (`identity` or `gzip`). The installer streams each file to a `.part`, verifies
  the transfer hash, decompresses if needed, verifies the installed hash and
  length, then atomically renames into place. A failed or partial download never
  replaces a good installed file. One download job runs at a time;
  `POST /map/offline/download { "region": "<id>" }` starts it (202 or 409, 404
  for an unknown region) and `GET /map/offline/status` reports it.
- **Hosting.** Assets live as release assets in `pinpoint-maps`, one immutable
  release per region and build date (`data-<region>-<YYYYMMDD>`), each carrying
  `ATTRIBUTION.md` (OSM/ODbL, Geofabrik, Planetiler). GitHub Releases is the v1
  host: free, public, and already where the current Moldova assets live. The
  `build-region.yml` workflow takes `region`, `app_ref` and an optional
  `maxzoom`, checks out the app repo at that ref, builds with the app's own
  scripts, uploads the assets, then regenerates and commits `catalog.json`. The
  data repo pins `app_ref`, so a catalog entry records which scripts built it.
- **2 GiB cap.** Every region's assets must fit GitHub's 2 GiB per-asset limit.
  `maxzoom` is capped per region to make that true; splitting a region into
  `parts` is a later catalog feature, not v1. A country that cannot fit at an
  acceptable zoom simply ships at a lower `maxzoom`.
- **OpenFreeMap online basemap, opt-in.** The map page gets an online toggle
  that loads the OpenFreeMap Liberty style, off by default. An installed local
  pack wins over the online style. Pins and routes are re-applied after the
  style loads. CSP only admits `https://tiles.openfreemap.org` and the verified
  style/glyph/sprite hosts, and only while the toggle is on. The page shows
  OpenStreetMap/OpenFreeMap attribution and a privacy note next to the toggle.
- **Regions and storage.** v1 lists Moldova, Romania and Ukraine; any Geofabrik
  region can be published on demand. Installed packs live under
  `packs/<region>/<file>`, served Range-aware from
  `GET /packs/:region/:name`. `GET /map/config` returns the active region's
  PMTiles URL, graph URL and bbox plus the installed regions; with no pack it
  falls back to the Monaco fixture.
- **Legacy migration.** On the first status or config request, the v2.3.0
  root-level `packs/moldova.pmtiles` and `packs/moldova.graph.bin` are moved
  into `packs/moldova/`. The existing install stays usable and is not
  re-downloaded.

## Consequences

- Adding a region is a data-repo action — build, upload, regenerate
  `catalog.json` — with no app release and no Rust change. Data freshness is
  decoupled from the release train.
- The app gains a small, validated parsing layer (`src/region_catalog.rs`) and
  a general installer, replacing per-file constants. The cost is that a bad or
  malicious catalog is now an attack surface; schema limits, hash pinning and
  the bundled fallback are what keep it bounded.
- Storage layout is per region, so several countries can coexist and the user
  can delete one without touching the others. One active job at a time keeps
  progress reporting and disk accounting simple, at the cost of slower
  multi-region installs.
- Legacy migration mutates app storage on first launch after upgrade. It must
  be idempotent and must fail safe: never delete the source until the
  destination is in place, otherwise an upgrade forces a re-download.
- GitHub Releases is not a CDN. Public traffic can be throttled or rate
  limited, and there is no SLA. The catalog is the mitigation: moving to object
  storage later changes URLs in `catalog.json`, not the app.
- Large countries may not fit 2 GiB at high zoom, so their `maxzoom` drops and
  detail suffers until per-region `parts` exist.
- The data repo becomes a second release pipeline to maintain: workflow
  permissions (`contents: write` on its own repo, no secrets), pinned
  `app_ref`, and weekly curation for Moldova, Romania and Ukraine.
- Online mode is a privacy change: enabling it sends tile requests to
  OpenFreeMap, which sees the client IP and the viewed map extent. It is off by
  default, disclosed in the UI, and no session token, account data or friend
  location is attached to those requests. Offline packs still contact no tile
  host.
- No cross-border routing and no online turn-by-turn outside downloaded packs
  in v1; those remain explicit non-goals rather than silent gaps.

## Alternatives considered

- **Ship a planet bundle with the app** — a complete world in one artifact, but
  it is hundreds of gigabytes, exceeds the 2 GiB asset cap by orders of
  magnitude, and no phone can store it. It also makes every user pay for data
  they never open.
- **Store packs as folders/files in a git repository** — simple URLs and free
  hosting, but GitHub rejects files over 100 MB, so even Moldova's 110 MB graph
  fails, and every clone would carry binary history forever.
- **Move to object storage (R2/S3) now** — the right long-term CDN, but it adds
  credentials, cost and CI wiring before there is traffic to justify it. The
  catalog makes that switch a data-only change, so deferring costs nothing.
- **Prebuild every country up front** — maximal coverage, but large build
  minutes, large storage, unknown demand, and a maintenance burden across
  hundreds of Geofabrik extracts. Building on demand from a curated list keeps
  the same capability with a fraction of the work.
- **Keep hardcoded packs and add regions by app release** — the status quo;
  rejected because it is exactly the coupling this ADR removes.
