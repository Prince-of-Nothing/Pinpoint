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
