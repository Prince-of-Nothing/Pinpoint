# Local native routing (Wave 4: maneuvers + graph cache)

Status: **proof complete — local route with turn-by-turn maneuvers, persisted
graph cache, measured on the M1 Moldova extract.** No online routing service is
called anywhere in this crate.

Design context: `docs/superpowers/specs/2026-11-16-offline-maps-design.md` §10
chooses Valhalla C++ for native routing and names a Rust A* router over the same
OSM graph as the fallback. Wave 2 built that fallback and measured it. Wave 4
adds the two things it was missing for a usable phone client: road names and
maneuvers, and a persisted graph so launch does not re-parse the PBF.

Code: `tools/native-router/` (standalone crate, not part of the root workspace).
Graph and JSON documentation: `tools/native-router/README.md`. Android notes:
`tools/native-router/ANDROID.md`.

## What was built

Wave 2 base, still present:

- `osmpbfreader` PBF ingest, two passes: ways first (keep car ways and collect
  node ids), then nodes filtered to those ids.
- Directed CSR graph `Node { osm_id, lat, lon }` + `Edge { from, to,
  distance_m, speed_kmh, class }` with per-node offsets.
- A* on travel time with a haversine / max-speed heuristic.
- `route(&Graph, (lat, lon), (lat, lon), Profile::Car) -> Route`.

Wave 4 additions:

- **Names.** The way parser captures `name`, falls back to `ref`, trims and
  ignores empty values. Names are interned into one table per graph; each edge
  stores a 4-byte table index (`NO_NAME` when absent). Moldova has 10,807 unique
  names/refs.
- **Roundabout flag.** `junction=roundabout|circular` is stored as bit 0 of
  `Edge::flags`, for maneuver generation.
- **Maneuvers.** `Route` gains `maneuvers: Vec<Maneuver>` with the exact field
  shape used by the route UI contract: `instruction`, `kind`, `distance_m`,
  `duration_s`, `point: [lat, lon]`, `point_index`. `points` and `nodes` are
  unchanged. Generation uses the turn angle between consecutive segments with a
  15° dead band, the outgoing edge name/ref, tag-based roundabout detection and
  a depart/arrive pair. Distance and duration on each maneuver run to the next
  maneuver; the final `arrive` is zero.
- **Persisted graph.** `Graph::save` / `Graph::load` implement format version 1:
  56-byte header (magic `PNPGRPH1`, version, counts, payload length, CRC-32),
  then interned names, fixed-width nodes and edges. Save is atomic (temp file +
  fsync + rename). Load validates the header, CRC and every structural
  invariant, streams arrays into their final `Vec`s and allocates no per-edge
  strings. Truncated and tampered files are rejected.
- **CLI.** `build --osm <pbf> --out <graph.bin>` and `route --graph
  <graph.bin> --from ... --to ...`; `--osm` remains as a one-shot fallback. The
  CLI reports build/load/route timings and peak working set.
- **Tests.** 51 tests without a PBF plus an opt-in Moldova test; the latter now
  asserts maneuvers, names and a cached-graph route within 1 m / 1 s.

No HTTP client is a dependency. `Cargo.lock` is committed. The PBF and
`graph.bin` are not.

## Graph representation

```text
nodes:   Vec<Node>   { osm_id: i64, lat: f64, lon: f64 }   24 B
offsets: Vec<u32>    len = nodes.len() + 1
edges:   Vec<Edge>   { from: u32, to: u32, distance_m: f32,
                       speed_kmh: f32, class: u8, flags: u8,
                       name: u32 }                          24 B
names:   Vec<Box<str>>
```

A `String` per edge would be millions of heap allocations. Instead the name
table owns one string per unique name/ref and the edge carries a `u32` id. The
per-edge name cost is 4 bytes, 12.5 MB for Moldova's 3.12M directed edges; the
table itself is a few hundred kilobytes. `Edge::name == NO_NAME` means the way
carried neither `name` nor `ref`.

`graph.bin` stores these arrays in the same order, with `offsets` and
`max_speed_kmh` rebuilt on load. The full header/payload layout and the
versioning rule are in `tools/native-router/README.md` and `src/graph.rs`.

## Moldova graph and measurements

Input: `moldova.osm.pbf`, 101,528,058 bytes (Geofabrik, M1 pack). SHA-256 and
pack details: `docs/offline/moldova.md`.

```text
graph: 1490268 nodes, 3117064 edges, 10807 names, max speed 120 km/h
```

Measured on the i5-13420H dev machine (Windows, MSVC, release). The host load
varies; build time in particular is CPU-bound and was measured anywhere from
8.3 s on an idle machine to 46 s while another worktree compiled an Android app.

| Operation | Measurement |
| --- | --- |
| Build graph from PBF | 6.65–20.47 s across runs on this machine (load-dependent); 46.26 s under heavy CPU load |
| Save `graph.bin` | 0.55–1.23 s |
| `graph.bin` size | **110,858,815 bytes (105.7 MiB)** |
| Load `graph.bin` | **0.265 s** (final CLI), **0.270 s** (final Moldova test), 0.282–0.604 s in earlier runs — target well under 2 s |
| Route Chișinău→Bălți from cache | 125.0–364.7 ms |
| Route from a fresh PBF parse | 137.1–355.4 ms after the parse |
| Peak working set, build + save | **283.0 MiB** |
| Peak working set, load + route | **149.1 MiB** |
| Peak working set, one-shot PBF route | 282.6 MiB |

The cache file is larger than the PBF (105.7 MiB vs 96.8 MiB) because PBF is
compressed protobuf and the cache is a fixed-width array built for fast decode.
The trade is deliberate: launch drops from a multi-second PBF parse and ~283 MiB
peak to a sub-second load and ~149 MiB peak. The resident graph arrays are about
106 MB; A* adds the working arrays on top.

## Real route: Chișinău → Bălți

Commands (PowerShell, release):

```powershell
# Build once.
.\target\release\native-router.exe build `
  --osm C:\Users\Dinusauuuur\Desktop\git\Pinpoint\dist\offline\moldova\moldova.osm.pbf `
  --out C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\moldova.graph.bin

# Serve from the cache.
.\target\release\native-router.exe route `
  --graph C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\moldova.graph.bin `
  --from 47.01,28.86 --to 47.76,27.93 --profile car
```

Measured output:

```text
graph: 1490268 nodes, 3117064 edges, 10807 names, max speed 120 km/h, built from PBF in 6.65 s
saved ...\moldova.graph.bin (110858815 bytes, 105.7 MiB) in 0.55 s
peak working set: 282.8 MiB

graph: 1490268 nodes, 3117064 edges, 10807 names, max speed 120 km/h, loaded from ...\moldova.graph.bin (105.7 MiB) in 0.265 s
route: 134.1 km, 97.3 min, 2037 points, 2037 nodes, 53 maneuvers, solved in 125.0 ms
peak working set: 149.1 MiB
```

Route result:

- **Distance: 134,109.5 m (134.1 km)**
- **Duration: 5,836.8 s (97.3 min), ~82.7 km/h average**
- **Nodes/points: 2,037 each**
- **Maneuvers: 53**, of which 48 carry a road name/ref
- First point 47.0100856,28.8600694; last 47.7599660,27.9303167

Sample instructions from the actual run (diacritics preserved):

```text
[      depart]   115.8 m      8 s  Head northwest on Bulevardul Iuri Gagarin
[slight_left]    14.8 m      1 s  Keep left onto Bulevardul Iuri Gagarin
[slight_right]  403.9 m     29 s  Keep right onto Bulevardul Iuri Gagarin
[  roundabout]  144.6 m     10 s  At the roundabout, take the exit onto Strada Albișoara
[       right]  865.1 m     62 s  Turn right onto Bulevardul Renașterii Naționale
[  roundabout] 2033.7 m     92 s  At the roundabout, take the 4th exit onto Strada Florilor
[      straight] 20848.8 m   844 s  Continue onto Tiraspol-Bălți
[  sharp_left]  430.6 m     20 s  Turn sharp left onto Tiraspol-Bălți - Centura Chișinăului
[        left]  173.2 m     10 s  Turn left onto Strada Mihai Viteazul
[       right]  101.0 m     18 s  Turn right
[      arrive]    0.0 m      0 s  Arrive at your destination
```

The maneuver kinds observed on this route are `depart`, `arrive`, `straight`,
`slight_left`, `slight_right`, `left`, `right`, `sharp_left` and `roundabout`.
A `uturn` band exists but does not occur on this route.

The full JSON on stdout is the route UI contract: `distance_m`, `duration_s`,
`points: [[lat, lon], ...]`, `nodes: [osm_id, ...]` (kept for compatibility) and
`maneuvers: [{instruction, kind, distance_m, duration_s, point: [lat, lon],
point_index}]`. The real run emits roughly ten thousand lines because every
graph node is a point.

Exit ordinals are deliberately conservative: a graph without turn restrictions
cannot always count roundabout arms reliably. The counter deduplicates
bidirectional arms and falls back to `take the exit onto <name>` above eight
arms. That is why the Albișoara and Renașterii roundabouts above show a name-only
exit while the simpler Strada Florilor one shows `4th exit`.

## Mobile serving model

**Recommendation: build `graph.bin` in the offline pack pipeline and ship it
inside the Moldova pack.** The download manager already owns pack lifecycle and
SHA-256 verification; `graph.bin` becomes another verified pack asset. On the
phone the app calls `Graph::load` once and routes from memory.

Why not generate the cache on first launch:

- The desktop build takes 8–46 s and peaks at 283 MiB. A mid-range phone is
  slower and has a tighter memory budget; doing it at first launch risks an ANR
  or OOM.
- It would require shipping both the 101.5 MB PBF and the 105.7 MB cache
  (~207 MB) if the PBF is also kept for regeneration. Shipping only `graph.bin`
  is smaller than shipping both.
- A prebuilt cache is deterministic and testable: the pack's route is the route
  the user gets.

Rebuild `graph.bin` whenever the PBF changes. Bump `GRAPH_FORMAT_VERSION` when
the format changes so a client can reject an incompatible cache and fetch/rebuild
a new one instead of misreading it.

## Android integration shape

No JNI wrapper, Gradle packaging or APK change was done in this wave; this
section describes the intended shape, not delivered code. See `ANDROID.md` for
the cross-build prerequisites.

1. Ship `graph.bin` inside `moldova.pack` (or as a sibling pack asset) and verify
   it with the same manifest SHA-256 as the PBF. Copy/open it from app-private
   storage.
2. A thin JNI wrapper (or UniFFI) exposes:
   - `nativeLoadGraph(path) -> handle`: calls `Graph::load`, boxes the graph.
   - `nativeRoute(handle, fromLat, fromLon, toLat, toLon) -> JSON string`: calls
     `route`, serializes the same `Route` JSON documented above.
   - `nativeFreeGraph(handle)`.
   Load once at startup or on first navigation request, off the main thread;
   never per route. Wrap the native calls in `catch_unwind`.
3. Parse the JSON in Kotlin/Java with the existing model, or return a small
   object with `distance_m`, `duration_s`, `points` and `maneuvers`.

The Wave 2 Android arm64 cross-build (NDK 27.2.12479018, `aarch64-linux-android`,
1,093,040-byte ELF binary) predates the graph cache and maneuver code. The crate
is still pure Rust and `Graph::save`/`load` only use `std::fs`, so no new C
dependency was introduced, but **the cross-build was not re-run in this wave**.
Treat the old artifact size as historical and re-measure before packaging.

## Rust router vs native Valhalla

Measured facts for the Rust path now:

- One-time graph build: 8.3–46 s desktop (load-dependent), 283 MiB peak.
- Persisted `graph.bin`: 105.7 MiB, 0.28–0.60 s load, 149 MiB peak with A*.
- Query: 0.14–0.36 s for a 134 km route on desktop.
- Maneuvers: geometry + names/refs, roundabout tags; no turn restrictions, lane
  guidance or exit signage.
- Car only.

M1's `moldova-valhalla.tar` already contains a Valhalla graph with **510,140
nodes / 1,354,988 directed edges** for car, pedestrian and bicycle (114 s Docker
build). The design's NDK/CMake path would compile Valhalla for `arm64-v8a`, load
the tar from app storage and expose routing + maneuvers through JNI. Pros:
production routing semantics (turn restrictions, maneuvers), smaller graph than
the 1.49M-node OSM graph, likely lower query latency. Cons: a C++ NDK build and
a much larger native artifact, neither proven on this machine yet.

**Decision unchanged:** Valhalla stays the production target for full
turn-by-turn semantics. The Rust path is now viable as an interim offline
route+maneuver engine because the graph cache removes the launch-time parse and
the API already speaks the route UI contract. Keep the `Router` seam so Valhalla
can replace it without touching the UI.

## What was not verified

- **No on-device run.** All numbers are desktop Windows/MSVC release numbers.
  Android load time, peak memory and battery behaviour were not measured.
- **No Android cross-build in Wave 4.** The Wave 2 arm64 compile predates the
  graph cache/maneuvers; re-run it before claiming an Android artifact.
- **No JNI/UniFFI wrapper, Gradle wiring or APK.** Nothing in this wave ships a
  `.so` into an app.
- **No turn restrictions.** The maneuver list can tell a driver to turn where
  OSM forbids it; it is a proof, not a legal guidance product.
- **No lane guidance, exit signs, traffic, closures or ferries.**
- **No spatial index.** Snapping is a linear scan over 1.49M nodes.
- **Memory-mapping was not implemented.** `graph.bin` is decoded into `Vec`s;
  peak load+route working set is 149.1 MiB. mmap is the next optimization if
  that budget is too high on a phone.

## Limits

- Car only. No pedestrian/bicycle profiles.
- Roundabout exit ordinals are a heuristic; complex junctions fall back to a
  name-only exit instruction.
- Turn instructions are angle-driven with a 15° dead band; they do not model
  forks with equal-angle branches or turn restrictions.
- Estimated speeds are routing defaults, not legal limits.
- The PBF build is a pack-pipeline/desktop operation, not a phone operation.
- Outside coverage (>25 km from the nearest graph node) returns
  `RouteError::OutsideCoverage`; callers must show "outside offline area" and
  must not silently call an online router.

## Licence

Map data © OpenStreetMap contributors, ODbL 1.0. The PBF and built graph stay
local and are not committed.
