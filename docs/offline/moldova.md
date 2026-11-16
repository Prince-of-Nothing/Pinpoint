# Moldova offline pack (M1)

Built: 2026-11-16
Source: `https://download.geofabrik.de/europe/moldova-latest.osm.pbf`
Output: `dist/offline/moldova/` (not committed)

## Verified artifacts

| File | Bytes | SHA-256 |
|---|---:|---|
| `moldova.pmtiles` | 93,172,399 | `7b27155519e3384e1520d1a3dc6488f93cf1e46292dae22129f972bc97524f2e` |
| `moldova-search.sqlite` | 6,770,688 | `c576be58672ab39118b4ae12b07600c2632b081945dfeb2be28f4820b8d78c82` |

- Basemap: OpenMapTiles schema, zoom 0–14, bbox `[26.6, 45.4, 30.2, 48.5]`.
- Search: **36,913** FTS5 places, names indexed from `name`, `name:ro`,
  `name:ru`, `name:uk`, `name:bg`, `name:gag`, `name:en`, `alt_name`,
  `official_name` and `short_name`; diacritics folded, Cyrillic searchable.
- Manifest: version `moldova-20261116`; attribution and ODbL license present;
  no UTF-8 BOM.

## Verification

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/verify.ps1 `
  -PackDir dist/offline/moldova -AllowMissingRouting
```

Output:

```text
manifest: moldova moldova-20261116 (2 files)
pmtiles: ok
search: {"places":36913,"fts":1}
routing: skipped
attribution: ok
verified 2 files for moldova
```

## Build inputs

- Planetiler OpenMapTiles image: `sha256:f5d7bcd8be4f534fef0132bd669c15e47b34503f14e070209d7680d1dd77eaa4`
- Valhalla image: `sha256:b8c515f8b3c212047755594fefe1c5c837ca417094b6908840179f130a3aa80b`
- Search builder: `pinpoint-offline-index` (`offline-build` feature).

## Routing status

The Valhalla graph build is the remaining M1 item. It parses 510,140 nodes and
1,354,988 directed edges; the enhancement phase was interrupted by agent and
Docker restarts during the first runs. A detached build is in progress. Do not
treat routing as verified until `moldova-valhalla.tar` is listed in the manifest
and this passes without `-AllowMissingRouting`:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/verify.ps1 `
  -PackDir dist/offline/moldova
```

## Reproduce

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/build.ps1 -Region moldova
```

## Licenses

Map data © OpenStreetMap contributors, ODbL 1.0. The pack ships
`ATTRIBUTION.md`; the manifest carries the same attribution and license URL.
