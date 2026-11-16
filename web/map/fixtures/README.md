# Monaco PMTiles fixture

`monaco.pmtiles` is a small OpenMapTiles-schema vector basemap used by the map
preview and the smoke test. It is a **test fixture**, not a production pack.

**Map data © OpenStreetMap contributors, ODbL 1.0.**

- License: [Open Database License (ODbL) 1.0](https://opendatacommons.org/licenses/odbl/1-0/)
- Copyright and attribution: <https://www.openstreetmap.org/copyright>
- Data source: Geofabrik `monaco-latest.osm.pbf`
- OSM snapshot: `2026-11-15T21:22:28Z` (Geofabrik replication)
- Pack build date: `2026-11-16T21:11:47Z`
- Builder: Planetiler `0.10.3-SNAPSHOT` / OpenMapTiles `3.16.0` schema
- Coverage: Monaco bbox `[7.40, 43.72, 7.44, 43.75]`, zoom `0–12`
- Size: `94389` bytes (`94 KB`)
- SHA-256: `a600dbfd9ddb25b82661f01d228a39d2b386dd13e401d2081b3ce7fd9c6de82f`
- Source artifact: the M1 offline pipeline output
  `dist/offline/monaco/monaco.pmtiles`, copied here without modification.

The vector layers exposed by this fixture are `aeroway`, `boundary`, `landuse`,
`mountain_peak`, `place`, `poi`, `transportation`, `water` and `waterway`.
The style also declares a `landcover` layer so a fuller OpenMapTiles pack
renders it, but this Monaco fixture has no features for that layer.

This file is committed only because it is small and makes the foundation
testable without Docker, network access or a downloaded pack. The Moldova
production pack is built by `scripts/offline/` and is not committed.

ODbL requires attribution wherever this data or a derived database is shown.
Keep the “© OpenStreetMap contributors” credit visible on every map that uses
it, and keep this notice with any redistributed copy.

## Bălți/Кишинёв search fixture

`balti-search.sqlite` is the small `places`/`places_fts` fixture used by
`web/map/test/search-smoke.mjs` and the browser search module. It is a **test
fixture**, not a production pack.

**Place data © OpenStreetMap contributors, ODbL 1.0.**

- License: [Open Database License (ODbL) 1.0](https://opendatacommons.org/licenses/odbl/1-0/)
- Copyright and attribution: <https://www.openstreetmap.org/copyright>
- Data: the two sample places from the M1
  `pinpoint-offline-index --fixture` definition in
  `src/bin/offline_index.rs`:
  - `Bălți` (normalised `balti`, `city`, `47.76, 27.93`, importance `0.9`)
  - `Кишинёв` (normalised `кишинев`, `city`, `47.01, 28.86`, importance `0.9`)
- Schema: `places` plus `places_fts` FTS5 external-content table with
  `tokenize='unicode61 remove_diacritics 2'`, and the `places_ai`/`places_ad`
  triggers. This is the M1 search schema, so the browser module is tested
  against the real contract rather than a hand-written table.
- Size: `24576` bytes (`24 KB`)
- SHA-256: `118c826db8aec4207b1bb8aa909dd5d45eaa2d4172d95e9ab64ecf6afba93772`

The M1 Monaco search database is not committed; the pipeline output is
`dist/offline/<region>/<region>-search.sqlite`. This two-place fixture was
written by the real M1 builder on 2026-11-17:

```powershell
cargo run --features offline-build --bin pinpoint-offline-index -- `
  --fixture web/map/fixtures/balti-search.sqlite
```

`pinpoint-offline-index --verify` reports `{"places":2,"fts":1}` for the
committed file.

The fixture is committed only because it is small and makes offline search
testable without Docker, network access or a downloaded Moldova pack. The
Bălți/Кишинёв rows are sample data, not a claim about the production dataset.
