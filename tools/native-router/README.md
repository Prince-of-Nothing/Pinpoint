# native-router — local offline routing spike

A standalone Rust crate that answers **car routes entirely on the device** from
an OpenStreetMap PBF extract. It has no HTTP client, no API keys and no online
routing fallback: the PBF is the only input.

This is the Rust A* fallback named in
`docs/superpowers/specs/2026-11-16-offline-maps-design.md` §10 ("If that build
is not feasible, the fallback is a Rust A* router over the same OSM graph").
It is a **spike/proof**, not a shipping navigation engine.

## What works

- Streaming PBF parse with [`osmpbfreader`](https://crates.io/crates/osmpbfreader).
- Directed car graph built from `highway=*` ways with `access`/`motorcar`/
  `motor_vehicle`/`vehicle` filtering, `oneway` (including `-1` and
  roundabouts), `maxspeed` (km/h and mph) and road-class speed defaults.
- A* search on travel time with a haversine/max-speed heuristic.
- `Route { distance_m, duration_s, points, nodes }`, plus a JSON CLI.
- Synthetic unit tests and an opt-in real Moldova acceptance test.

## Graph format

Kept deliberately small and documented in `src/graph.rs`:

```text
nodes:   Vec<Node>   { osm_id: i64, lat: f64, lon: f64 }
offsets: Vec<u32>    len = nodes.len() + 1
edges:   Vec<Edge>   { from, to, distance_m: f32, speed_kmh: f32, class }
```

`edges[offsets[i]..offsets[i + 1]]` are the directed edges leaving node `i`
(compressed sparse row). The PBF builder emits one edge per allowed travel
direction, so two-way ways appear twice. Distances are haversine metres and
`duration = distance / speed`; there are no turn costs.

Memory for Moldova (measured on the M1 extract, see
`docs/offline/native-routing.md`): **1,490,268 nodes / 3,117,064 directed
edges**, about **255 MiB peak working set** during the build and **8.3 s** to
load on a warm OS cache with the machine otherwise idle. The build reads the PBF
twice (ways, then nodes filtered to way node ids) so it never holds every
Moldova node in memory. Nearest-node snapping is a linear scan; the
Chișinău–Bălți route answers in about 0.2–0.5 s on desktop when the machine is
not saturated.

## Usage

```powershell
cd tools/native-router
cargo build --release

.\target\release\native-router.exe route `
  --osm C:\Users\Dinusauuuur\Desktop\git\Pinpoint\dist\offline\moldova\moldova.osm.pbf `
  --from 47.01,28.86 `
  --to 47.76,27.93 `
  --profile car
```

stdout is JSON:

```json
{
  "distance_m": 0.0,
  "duration_s": 0.0,
  "points": [[47.01, 28.86]],
  "nodes": [0]
}
```

stderr carries timings and graph statistics, for example:

```text
graph: N nodes, M edges, max speed 130 km/h, loaded in X.XX s
route: XXX.X km, XX.X min, N points, N nodes, solved in X.X ms
```

`--profile` accepts `car` (and `auto` as an alias). Other profiles are not
implemented yet.

## Tests

```powershell
cargo test
```

Covers CSR construction, distance/duration, oneway, faster-not-shorter route
selection, snapping, disconnected graphs, outside-coverage reporting, tag
parsing and CLI coordinate parsing. No network and no PBF needed.

Real Moldova route (PBF not committed):

```powershell
$env:PINPOINT_MOLDOVA_PBF = "C:\Users\Dinusauuuur\Desktop\git\Pinpoint\dist\offline\moldova\moldova.osm.pbf"
cargo test --release --test moldova -- --ignored --nocapture
```

## Limits (spike scope)

- Car profile only; no pedestrian/bicycle.
- No turn restrictions, turn penalties, lane guidance, traffic, closures or
  live data. Maneuvers are not produced; this route is geometry + ETA.
- No ferries; only `highway=*` ways are used.
- No spatial index. Snapping is O(nodes) per query (fine for Moldova, not for
  a continent).
- No binary graph cache. Re-parsing a 101 MB PBF on every app start is not
  acceptable on a phone; see `ANDROID.md` for the next step.
- Coordinates and durations are estimates; this must not be presented as a
  legal speed-limit or live-traffic product.

## Data licence

Map data © OpenStreetMap contributors, ODbL 1.0. The PBF stays local and is not
committed to this repository.
