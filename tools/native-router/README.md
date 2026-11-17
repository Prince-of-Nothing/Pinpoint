# native-router — local offline routing

A standalone Rust crate that answers **car routes entirely on the device** from
an OpenStreetMap PBF extract or from a compact graph cache built from it. It has
no HTTP client, no API keys and no online routing fallback.

This started as the Rust A* fallback named in
`docs/superpowers/specs/2026-11-16-offline-maps-design.md` §10. It is still a
**spike/proof**, not a shipping navigation engine, but it now produces
turn-by-turn maneuvers and can serve routes from a persisted graph instead of
re-parsing the PBF at every launch.

## What works

- Streaming PBF parse with [`osmpbfreader`](https://crates.io/crates/osmpbfreader).
- Directed car graph built from `highway=*` ways with `access`/`motorcar`/
  `motor_vehicle`/`vehicle` filtering, `oneway` (including `-1` and
  roundabouts), `maxspeed` (km/h and mph) and road-class speed defaults.
- **Road names.** `name` is captured, with `ref` as the fallback. Names are
  interned once per graph; each edge stores a 4-byte id.
- **Roundabout flag.** `junction=roundabout|circular` is stored as an edge flag.
- A* search on travel time with a haversine/max-speed heuristic.
- **Turn-by-turn maneuvers** derived from the routed geometry and edge names:
  `depart`, `straight`, `slight_left`, `slight_right`, `left`, `right`,
  `sharp_left`, `sharp_right`, `uturn`, `roundabout`, `arrive`.
- **Persisted compact graph** (`Graph::save` / `Graph::load`) with a magic
  header, format version, interned names, CRC-32 and atomic replacement.
- A JSON CLI with `build` and `route` commands.
- Synthetic unit tests, saved-graph round-trip/corruption tests and an opt-in
  real Moldova acceptance test.

## In-memory graph format

Documented in `src/graph.rs`:

```text
nodes:   Vec<Node>   { osm_id: i64, lat: f64, lon: f64 }             24 B
offsets: Vec<u32>    len = nodes.len() + 1
edges:   Vec<Edge>   { from: u32, to: u32, distance_m: f32,
                       speed_kmh: f32, class: u8, flags: u8,
                       name: u32 }                                   24 B
names:   Vec<Box<str>>  interned once; Edge::name indexes it
```

`edges[offsets[i]..offsets[i + 1]]` are the directed edges leaving node `i`
(compressed sparse row). The PBF builder emits one edge per allowed travel
direction, so two-way ways appear twice. Distances are haversine metres and
`duration = distance / speed`; there are no turn costs or turn restrictions.

**Name representation.** A Moldova-sized graph has ~3.1M directed edges, so a
`String` per edge would mean millions of heap allocations. Instead the PBF pass
interns each unique `name`/`ref` once into `names` (10,807 entries on the M1
Moldova extract) and every edge carries a `u32` index, or `NO_NAME`
(`u32::MAX`). The name table costs a few hundred kilobytes; the per-edge cost is
4 bytes (12.5 MB for Moldova). Ways that share a name share the id, including
ways split only by an administrative boundary.

**Roundabout representation.** Bit 0 of `Edge::flags` marks
`junction=roundabout|circular`. It costs no extra per-edge space because the
struct was already 24 bytes after adding the name id.

Memory for Moldova (M1 extract, measured): **1,490,268 nodes / 3,117,064
directed edges / 10,807 names**. The resident graph arrays are about 106 MB;
`graph.bin` is **110,858,815 bytes (105.7 MiB)**.

## Persisted graph (`graph.bin`)

`Graph::save(path)` writes a sibling temp file, flushes and syncs it, then
renames it over `path`; a failed save leaves the old file untouched.
`Graph::load(path)` streams the arrays straight into their final `Vec`s, so it
never allocates a string per edge.

Little-endian header (56 bytes):

```text
0   8   magic "PNPGRPH1"
8   4   format version (1)
12  4   reserved, zero
16  8   node count
24  8   edge count
32  8   interned name count
40  8   payload length in bytes
48  4   CRC-32 (IEEE) of the payload
52  4   reserved, zero
```

Payload:

```text
names:   name_count × (u32 byte length, UTF-8 bytes)
nodes:   node_count × (i64 osm_id, f64 lat, f64 lon)                 24 B
edges:   edge_count × (u32 from, u32 to, f32 distance_m, f32 speed_kmh,
                       u8 class, u8 flags, u16 reserved, u32 name)   24 B
```

Load rejects a bad magic, an unsupported version, truncation, trailing bytes, a
CRC mismatch and structural corruption (endpoint out of range, non-finite or
out-of-range coordinates, non-positive speed, unknown road class, unknown name
id, edges not grouped by source). The format version is
`GRAPH_FORMAT_VERSION = 1`; changing the payload layout requires bumping it, so
an old app can reject a new cache instead of misreading it.

Measured on the dev machine (i5-13420H, warm cache, host load varies):

| Operation | Result |
| --- | --- |
| Build graph from 101.5 MB PBF | 6.7–20.5 s across runs (8.3 s in the idle Wave 2 run; 46 s under CPU load) |
| Save `graph.bin` | 0.6–1.2 s |
| `graph.bin` size | 110,858,815 bytes (105.7 MiB) |
| Load `graph.bin` | **0.26–0.60 s** (target: well under 2 s) |
| Route Chișinău→Bălți from cache | 0.13–0.36 s, 53 maneuvers |
| Peak working set, build+save | 283.0 MiB |
| Peak working set, load+route | 149.1 MiB |
| Peak working set, PBF one-shot | 282.6 MiB |

`graph.bin` is bigger than the PBF because PBF is compressed protobuf while the
cache is a fixed-width array designed for a fast `read`/decode. It replaces a
multi-second parse and ~283 MiB peak on every launch with a sub-second load and
~149 MiB.

## Maneuvers

`Route` is unchanged for existing clients and gains one field:

```json
{
  "distance_m": 134109.49735951424,
  "duration_s": 5836.822456228866,
  "points": [[47.0100856, 28.8600694], "... 2035 more [lat, lon] pairs ..."],
  "nodes": ["... 2037 OSM node ids ..."],
  "maneuvers": [
    {
      "instruction": "Head northwest on Bulevardul Iuri Gagarin",
      "kind": "depart",
      "distance_m": 115.77375507354736,
      "duration_s": 8.33571036529541,
      "point": [47.0100856, 28.8600694],
      "point_index": 0
    },
    {
      "instruction": "At the roundabout, take the 4th exit onto Strada Florilor",
      "kind": "roundabout",
      "distance_m": 2033.7413873672485,
      "duration_s": 91.78652480077744,
      "point": [47.04292, 28.8536527],
      "point_index": 150
    },
    {
      "instruction": "Arrive at your destination",
      "kind": "arrive",
      "distance_m": 0.0,
      "duration_s": 0.0,
      "point": [47.759966, 27.9303167],
      "point_index": 2036
    }
  ]
}
```

(The arrays above are abridged for readability; the real CLI emits valid JSON
with 2,037 points and 2,037 node ids.)

Rules:

- `point` is `[lat, lon]`, `point_index` indexes `points`/`nodes`.
- The first maneuver is always `depart` at the snapped origin; the last is
  always `arrive` with zero distance and duration.
- `distance_m` / `duration_s` run from this maneuver to the next. Their sum is
  the route distance/duration.
- Turn angle is measured between consecutive path segments. Turns below
  **15°** are treated as geometry noise.
- Bands: 15–35° `slight_*` ("Keep left/right"), 35–110° `*_left/right`
  ("Turn left/right"), 110–170° `sharp_*`, above 170° `uturn`.
- A `straight` ("Continue onto …") is emitted only when the road name changes
  while the geometry stays inside the dead band. Unnamed roads do not generate
  straight noise.
- Instructions use the outgoing edge's name or ref, for example
  `Turn right onto Strada Ștefan cel Mare`; with no name the instruction is the
  bare `Turn right`.
- Roundabouts are detected from the PBF tag, not from geometry. The exit number
  counts distinct exit arms between entry and exit (bidirectional arms counted
  once). Above eight arms the ordinal is considered unreliable and the
  instruction becomes `At the roundabout, take the exit onto <name>`.
- `depart` uses an eight-point compass: `Head northwest on <name>`.

The maneuver list is deterministic for a given graph and pair of snapped nodes.
`points` and `nodes` keep their original meaning.

## Usage

```powershell
cd tools/native-router
cargo build --release

# One-time: parse the PBF and write the cache.
.\target\release\native-router.exe build `
  --osm C:\path\to\moldova.osm.pbf `
  --out C:\path\to\graph.bin

# Every launch/route: load the cache.
.\target\release\native-router.exe route `
  --graph C:\path\to\graph.bin `
  --from 47.01,28.86 `
  --to 47.76,27.93 `
  --profile car
```

`--osm` still works as a one-shot fallback that builds the graph in memory and
routes immediately; it is not the mobile path. `--profile` accepts `car` (and
`auto` as an alias).

stdout is the `Route` JSON. stderr carries timings and graph statistics:

```text
graph: 1490268 nodes, 3117064 edges, 10807 names, max speed 120 km/h, loaded from ...\graph.bin (105.7 MiB) in 0.265 s
route: 134.1 km, 97.3 min, 2037 points, 2037 nodes, 53 maneuvers, solved in 125.0 ms
peak working set: 149.1 MiB
```

## Serving model for mobile

**Recommended: build `graph.bin` in the offline pack pipeline and ship it inside
the Moldova pack.** The existing download manager already owns pack lifecycle
and SHA-256 verification; add `graph.bin` as a pack asset and verify it like any
other file. The app then only calls `Graph::load` into memory (0.3–0.6 s on
desktop, still under 2 s with margin on a phone) and never parses the PBF.

Why not generate on first launch:

- The desktop build takes 8–46 s and peaks at 283 MiB; a phone will be slower
  and has a tighter memory budget. Doing that on first launch risks an ANR/OOM.
- It would require shipping both the 101.5 MB PBF and the resulting 105.7 MB
  cache, roughly doubling pack size. Shipping only the cache is smaller.
- A prebuilt cache is deterministic: the pack is tested with the exact graph the
  user will route on.

Rebuild and bump the pack whenever the PBF changes; bump `GRAPH_FORMAT_VERSION`
when the format changes so an app can detect an incompatible cache.

## Android integration shape

No JNI wrapper, Gradle packaging or APK change is part of this work. The shape
to implement is recorded in `ANDROID.md`: load the graph once from
app-private storage into a boxed handle, route by handle, and parse the JSON (or
return a small object with the same fields). Keep routing local; there is no
HTTP fallback. The crate is still pure Rust and the library has no platform
code, but the Wave 2 Android cross-build predates this change and should be
re-run before shipping.

## Tests

```powershell
cargo test
```

51 tests run without a PBF: CSR construction, names and `NO_NAME`, oneway and
roundabout tags, route distance/duration, faster-not-shorter selection,
snapping, disconnected graphs, outside coverage, maneuver left/right/straight/
arrive, the 15° dead band, name fallback, roundabout exit counting, distance
attribution, save/load round-trip, atomic replacement, truncated/corrupt file
rejection, CLI parsing and peak-memory formatting.

Real Moldova route (PBF not committed):

```powershell
$env:PINPOINT_MOLDOVA_PBF = "C:\...\dist\offline\moldova\moldova.osm.pbf"
cargo test --release --test moldova -- --ignored --nocapture
```

The test builds the graph, asserts the Chișinău→Bălți route has more than one
maneuver and at least one named instruction, saves and reloads `graph.bin`, and
checks that the cached graph routes within 1 m / 1 s of the in-memory graph
(exact points, nodes and maneuvers are also compared).

## Limits (spike scope)

- Car profile only; no pedestrian/bicycle.
- No turn restrictions, turn penalties, lane guidance, traffic, closures or
  live data. Maneuvers are geometry + edge names, not legal lane guidance.
- Roundabout exit ordinals are a heuristic over a graph without turn
  restrictions; complex junctions fall back to a name-only exit instruction.
- No ferries; only `highway=*` ways are used.
- No spatial index. Snapping is O(nodes) per query (fine for Moldova, not for a
  continent).
- `graph.bin` is fully resident in memory; it is not memory-mapped. Load is fast
  but the ~106 MB graph still needs a memory budget.
- The PBF build itself is a desktop/pack-pipeline operation, not a phone
  operation.
- Coordinates and durations are estimates; this must not be presented as a
  legal speed-limit or live-traffic product.

## Data licence

Map data © OpenStreetMap contributors, ODbL 1.0. The PBF and built graph stay
local and are not committed.
