# Region build and catalog

`scripts/offline/build.ps1` builds one country pack from a Geofabrik
extract: PMTiles basemap, FTS5 search index, Valhalla tiles, the native
routing graph (`.graph.bin` plus `.graph.bin.gz`) and `manifest.json`.
`scripts/offline/catalog.ps1` turns published region manifests into the
schema-1 `catalog.json` the app downloads.

Both scripts run on Windows PowerShell 5.1 and on `pwsh` 7. Scripts are
ASCII-only. The build never publishes a partial pack: every step writes into
`dist/offline/.staging/<region>-<id>-work` and the published directory is
replaced only after `verify.ps1` accepts every file. A failed graph step
publishes nothing.

## Prerequisites

- PowerShell 5.1+ or `pwsh` 7.
- Rust toolchain (`cargo`) plus a C++ linker for
  `pinpoint-offline-index` and `tools/native-router`.
- Docker Engine/Desktop for Planetiler and Valhalla (skippable with
  `-SkipBasemap -BasemapPath` and `-SkipRouting`).
- `curl` (or `curl.exe`) and `tar`.
- Disk: roughly 2-3 GB of staging space for a country-sized PBF.

## Build

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/build.ps1 `
  -Region moldova
```

On Linux or CI use the same arguments with `pwsh -NoProfile -File`.

Useful switches:

| Switch | Effect |
|---|---|
| `-Maxzoom <0-14>` | Basemap zoom override; capped at 14 for the release-size budget |
| `-AllowUnpinnedSource` | Continue when the Geofabrik `-latest` SHA-256 no longer matches the pinned one; the downloaded hash is recorded |
| `-SkipGraph` | Publish a basemap/search/routing pack without `.graph.bin` and `.graph.bin.gz` |
| `-SkipBasemap -BasemapPath <file>` | Reuse a prebuilt PMTiles basemap |
| `-SkipDownload -OsmPath <file>` | Build from a local PBF |
| `-SkipRouting`, `-SkipSearch` | Skip those steps for a smoke build |
| `-DryRun` | Print the planned commands and the source-pin policy without writing |

`-DryRun` needs no real files:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/build.ps1 `
  -Region monaco -DryRun
```

It prints `maxzoom:`, `allow unpinned source:`, the list of published files
and the `native-router`/gzip commands.

## Published layout

```text
dist/offline/<region>/
  <region>.pmtiles
  <region>-search.sqlite
  <region>-valhalla.tar
  <region>.graph.bin
  <region>.graph.bin.gz
  manifest.json
  ATTRIBUTION.md
```

A `-SkipRouting` build publishes no Valhalla tar; a `-SkipGraph` build
publishes no graph files. `verify.ps1 -AllowMissingRouting -SkipGraph`
accepts those packs.

The graph step:

1. `cargo build --release --locked --manifest-path tools/native-router/Cargo.toml`
   (or uses an existing `tools/native-router/target/release/native-router`).
2. `native-router build --osm <region>.osm.pbf --out <region>.graph.bin`.
3. Gzips the graph with the portable .NET `GZipStream` into
   `<region>.graph.bin.gz`.

The manifest keeps one entry per transfer file. The graph entry carries both
sides of the install:

```json
{
  "name": "moldova.graph.bin.gz",
  "role": "graph",
  "url": "https://.../moldova.graph.bin.gz",
  "bytes": 45338586,
  "sha256": "276d0d...",
  "installed_name": "moldova.graph.bin",
  "installed_bytes": 110858815,
  "installed_sha256": "8cdbb0..."
}
```

Other roles are `basemap`, `search`, `routing` and `other`. The manifest also
records the actual input extract:

```json
"source_url": "https://download.geofabrik.de/europe/moldova-latest.osm.pbf",
"source_sha256": "249574c4...",
"source_bytes": 101528058,
"downloaded_at": "2026-11-17T12:00:00Z"
```

## Source pin policy

| `source.sha256` | `-AllowUnpinnedSource` | Behavior |
|---|---|---|
| pinned and matches | either | Build proceeds; the manifest records the hash |
| pinned and differs | off (default) | Build fails after download, showing the pinned and downloaded hashes |
| pinned and differs | on | Warning with old/new hashes; build continues and records the downloaded hash |
| empty | either | Download is allowed with a visible warning; the computed hash and byte count are printed so they can be pinned |

Pinned `regions.json` is for reproducible local and release builds: the same
Geofabrik snapshot must be used. Geofabrik's `-latest` files change whenever
upstream updates, so scheduled weekly builds pass `-AllowUnpinnedSource` and
publish the fresh extract. That flag is passed by the build workflow in the
`pinpoint-maps` data repo (the workflow change lands there, not in this
script). A local `-OsmPath` build records `local:<filename>` as `source_url`
so the build machine path is not published. The manifest's `source_sha256`,
`source_bytes`, `source_url` and `downloaded_at` are the audit trail for that
release; the catalog carries only the published file hashes.

## Limits

- GitHub Releases allows **2 GiB per asset**. Check every output before
  uploading; if the PMTiles file is too large, lower `-Maxzoom` (the build
  refuses values above 14) or split the region in a later catalog feature.
- `regions.json` values follow the mobile pack budget:
  Moldova `maxzoom` 14, Romania 13 (a 6 GB phone should not carry the z14
  PMTiles), Ukraine 13. Ukraine is not in the weekly schedule; it needs a
  larger runner and a lower zoom or basemap-only decision before publication.
- The graph is not affected by `maxzoom`. A graph that is too large for the
  phone budget must ship without routing or be split.
- Gzip bytes are stable for one runtime, but .NET Framework (PowerShell 5.1)
  and .NET 8 (`pwsh` 7) may produce different deflate output for the same
  graph. The manifest pins the bytes that were actually published; do not
  expect a release hash to reproduce across hosts.

## Catalog

Given a published region directory:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/catalog.ps1 `
  -RegionDir dist/offline/moldova `
  -Catalog catalog.json `
  -BaseUrl https://github.com/Prince-of-Nothing/pinpoint-maps/releases/download/data-moldova-20261117 `
  -Version 2026-11-17
```

`catalog.ps1` reads `manifest.json`, selects exactly one basemap and one graph,
checks the declared sizes and SHA-256 values against the files, and writes or
merges `catalog.json`:

- `url` is `BaseUrl + "/" + transfer_name`; `BaseUrl` is the release download
  prefix, not the region directory.
- Basemap uses `compression: "identity"`; graph uses `compression: "gzip"` and
  exposes the decompressed install fields.
- Regions are sorted by id and files are ordered basemap then graph, so a merge
  is deterministic and re-running a region replaces its entry.
- The catalog is created when absent. Invalid schema, ids, bbox, sizes,
  hashes, URLs or missing files abort before any write.
- `source_sha256` is an additive schema-1 region field; it is optional and
  comes from the manifest.

Expected output:

```text
catalog: 2 region(s) -> catalog.json
```

## Add a region

1. Add an entry to `scripts/offline/regions.json` with `id`, `name`, the
   Geofabrik `geofabrik` URL, a rough `bbox`, `minzoom`/`maxzoom`, the pinned
   Docker images, a `base_url` placeholder, and an empty
   `"source": { "sha256": "", "bytes": 0 }`.
2. Run the first build. It downloads the current `-latest` extract and prints:
   `source sha256: <hash> (<bytes> bytes)`.
3. Pin that hash and byte count in `regions.json` for reproducible rebuilds.
4. Publish the region files to the release named by `BaseUrl`.
5. Add the region to `catalog.json` with `catalog.ps1` and commit the catalog.

Scheduled weekly builds should pass `-AllowUnpinnedSource`; local release
builds should keep the pin strict and fail on a changed upstream extract.

## Verification

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/verify.ps1 `
  -PackDir dist/offline/moldova
```

`verify.ps1` checks every manifest entry, both graph files (transfer and
installed size/hash), the PMTiles header, the search index and the routing tar.
Add `-AllowMissingRouting` for `-SkipRouting` packs and `-SkipGraph` for
graph-less packs.

Offline pipeline tests (no Docker or network):

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/tests/run.ps1
```
