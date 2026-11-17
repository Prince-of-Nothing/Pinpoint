# Moldova offline pack (M1)

Built: 2026-11-16. Publication layout and reproducibility inputs updated
2026-11-17; the artifact bytes and SHA-256 values below are from the original
verified build.

Source: `https://download.geofabrik.de/europe/moldova-latest.osm.pbf`
Output: `dist/offline/moldova/` (gitignored, not committed)

## Published layout

Builds are atomic: Planetiler, the search index, Valhalla and `verify.ps1` run
in `dist/offline/.staging/<region>-<id>-work`, and only after verification
passes does the published directory get replaced. The published directory
contains exactly the five shipped files; the source PBF, the generated
Valhalla config and the raw `valhalla_tiles/` directory stay in the staging
work directory and are deleted on success or failure.

```text
dist/offline/moldova/
  moldova.pmtiles
  moldova-search.sqlite
  moldova-valhalla.tar
  manifest.json
  ATTRIBUTION.md
```

A `-SkipRouting` build (the Monaco smoke path) publishes the four files it
actually built, without a routing tar, and `verify.ps1 -AllowMissingRouting`
accepts that manifest. On a failed build the previous published directory is
left untouched and no manifest is replaced.

## Verified artifacts

| File | Bytes | SHA-256 |
|---|---:|---|
| `moldova.pmtiles` | 93,172,399 | `7b27155519e3384e1520d1a3dc6488f93cf1e46292dae22129f972bc97524f2e` |
| `moldova-search.sqlite` | 6,770,688 | `c576be58672ab39118b4ae12b07600c2632b081945dfeb2be28f4820b8d78c82` |
| `moldova-valhalla.tar` | 120,319,488 | `a9fce80fae0e6fea566ee215d916bf98a0af909253fe81b6dedc1c6383fe3fa8` |

Total: **220,262,575 bytes (210 MB)**.

- Basemap: OpenMapTiles schema, zoom 0–14, bbox `[26.6, 45.4, 30.2, 48.5]`.
- Search: **36,913** FTS5 places, names indexed from `name`, `name:ro`,
  `name:ru`, `name:uk`, `name:bg`, `name:gag`, `name:en`, `alt_name`,
  `official_name` and `short_name`; diacritics folded, Cyrillic searchable.
- Routing: Valhalla graph, 510,140 nodes and 1,354,988 directed edges; car,
  pedestrian and bicycle profiles; 114 s build.
- Manifest: version `moldova-20261116`; attribution and ODbL license present;
  no UTF-8 BOM. Rebuilds derive every `files[].url` from `base_url`.

The same data release also carries the compact native routing cache produced
from the Moldova PBF. The app downloads it gzip-compressed and installs the
decompressed graph:

| File | Bytes | SHA-256 |
|---|---:|---|
| `moldova.graph.bin.gz` (transfer) | 45,338,586 | `276d0ddd3058524038f53a6562c86a12c408693a1e84bbb316c9db16badc9087` |
| `moldova.graph.bin` (installed) | 110,858,815 | `8cdbb0cb1d61c2d5857cb2677ab1343968223e4ae10892f0268c8feb0eb50e75` |

## App install (Android and desktop map window)

`src/offline_install.rs` is the native download manager shared by both
loopback map servers. It installs exactly two files into the app's `packs/`
directory:

```text
<app data>/packs/moldova.pmtiles     93,172,399 B
<app data>/packs/moldova.graph.bin  110,858,815 B
```

The transfer is 138,510,985 bytes (**139 MB**: 93,172,399 B for the PMTiles
archive plus 45,338,586 B for the gzip graph); the installed files occupy
204,031,214 bytes (**204 MB**). Both artifacts come from
`https://github.com/Prince-of-Nothing/Pinpoint/releases/download/data-moldova-20261116/`
unless `PINPOINT_OFFLINE_BASE_URL` points at a mirror or a test fixture host.
The page never downloads from GitHub; the Rust process streams each response
to `packs/.<name>.part`. The PMTiles archive is written as-is; the graph is
fed through an incremental gzip decoder into `packs/.moldova.graph.bin.part`.
Both files are hashed with SHA-256 while streaming (the graph on the
decompressed bytes, plus the compressed transfer), checked against the exact
byte length, and only then atomically renamed into place. A failed, truncated
or wrong-hash download leaves the previously installed file intact.

The map servers expose `GET /map/offline/status`,
`POST /map/offline/download`, `GET /map/config` and
`GET|HEAD /packs/<name>.pmtiles` (Range `206`/`416`). The map page shows the
**"Download Moldova map (139 MB)"** transfer size, polls progress, reports
**Routing graph installed** vs **Download routing data**, and reloads onto
`/packs/moldova.pmtiles` after an install.
The route engine stays local: `/map/route` reads the downloaded graph through
`native_router`; there is no online routing fallback or geocoder.

## Reproducibility inputs

The OSM extract is pinned by SHA-256 in `scripts/offline/regions.json`.
`download.ps1` reuses an existing file only when the hash matches, removes a
stale file before re-downloading, and rejects a download whose hash differs.
Docker images are pinned by digest, not by tag.

| Input | Value |
|---|---|
| Moldova PBF | 101,528,058 bytes, `sha256:249574c43abf7850092b15d1e396cbca0368c09f027eb99f71aeadf167905752` |
| Planetiler image | `openmaptiles/planetiler-openmaptiles@sha256:f5d7bcd8be4f534fef0132bd669c15e47b34503f14e070209d7680d1dd77eaa4` |
| Valhalla image | `ghcr.io/valhalla/valhalla@sha256:b8c515f8b3c212047755594fefe1c5c837ca417094b6908840179f130a3aa80b` |

The committed Monaco CI fixture is pinned too:
708,123 bytes, `sha256:d35d64fde04dc83062910bb858ccb4750e223694b85def924a200b4246621d36`.

## Prerequisites

- PowerShell 5.1+ (Windows) or `pwsh` (Linux/CI).
- Docker Desktop/Engine with access to `ghcr.io` and Docker Hub.
- Rust toolchain with a C++ linker (`cl.exe`/`link.exe` on Windows, `cc` on
  Linux) to build `pinpoint-offline-index`.
- `curl.exe` (Windows) or `curl`, and `tar`.
- About 1 GB of free space for the staging work directory, plus space for the
  pinned Docker images.

`build.ps1` checks Docker, cargo (or an existing `target/{debug,release}`
binary), curl and tar before the download, and skips checks for `-Skip*`
steps. When cargo is available it always runs
`cargo build --locked --features offline-build --bin pinpoint-offline-index`
and uses that fresh binary; it does not silently fall back to a stale debug
artifact when a rebuild is possible.

## Publish URL (`base_url`)

`regions.json` carries a `base_url` for each region. The checked-in value
`https://packs.pinpoint.example/moldova` is a documented placeholder: no pack
hosting is live yet. **Replace `packs.pinpoint.example` with the real pack
origin before publishing**, either by editing `base_url` or by passing
`-BaseUrl https://<host>/<region>` at build time. `New-OfflineManifest` derives
every `files[].url` from it, and `verify.ps1` rejects a manifest with an empty
file URL. Manifests generated before this change have empty URLs; rebuild the
region to regenerate them.

Publishing means uploading the five files to the host under that base URL and
serving the same manifest from the Pinpoint server, as described in the
offline-maps design §7. Do not publish the staging directory or any build
input.

## Reproduce

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/build.ps1 `
  -Region moldova
```

For a local extract, skip the download and supply it explicitly:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/build.ps1 `
  -Region monaco -SkipDownload -SkipRouting `
  -OsmPath scripts/offline/tests/fixtures/monaco.osm.pbf
```

`-SkipBasemap -BasemapPath <file.pmtiles>` reuses a prebuilt basemap (used by
the pipeline tests, which cannot run Planetiler) while still exercising the
search, manifest, verify and atomic-publish path.

## Verification

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/verify.ps1 `
  -PackDir dist/offline/moldova
```

Output from the verified build:

```text
manifest: moldova moldova-20261116 (3 files)
pmtiles: ok
search: {"places":36913,"fts":1}
routing: ok
attribution: ok
verified 3 files for moldova
```

Add `-AllowMissingRouting` for a `-SkipRouting` pack.

## Licenses

Map data © OpenStreetMap contributors, ODbL 1.0. The pack ships
`ATTRIBUTION.md`; the manifest carries the same attribution and license URL.
