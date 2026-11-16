# Local native routing spike (Wave 2)

Status: **proof complete, local route works, Android arm64 cross-build works.**
No online routing service is called anywhere in this crate.

Design context: `docs/superpowers/specs/2026-11-16-offline-maps-design.md` §10
chooses Valhalla C++ for native routing and names a Rust A* router over the same
OSM graph as the fallback. This spike builds that fallback and measures it
against the M1 Moldova extract.

Code: `tools/native-router/` (standalone crate, not part of the root workspace).
Graph documentation: `tools/native-router/README.md`.
Android notes: `tools/native-router/ANDROID.md`.

## What was built

- `osmpbfreader` PBF ingest, two passes:
  1. `iter_ways()` — keep `highway=*` car ways, parse access/oneway/maxspeed,
     collect their node ids.
  2. `iter_nodes()` — keep coordinates only for those ids.
- Directed CSR graph: `Node { osm_id, lat, lon }` + `Edge { from, to,
  distance_m, speed_kmh, class }`, offsets per node.
- A* on travel time with a haversine / max-speed heuristic.
- API: `route(&Graph, (lat, lon), (lat, lon), Profile::Car) ->
  Route { distance_m, duration_s, points, nodes }`.
- CLI: `native-router route --osm <pbf> --from lat,lon --to lat,lon
  --profile car`, JSON on stdout, timings on stderr.
- Tests: 25 unit/CLI tests, plus an opt-in real Moldova acceptance test.

No HTTP client is a dependency. `Cargo.lock` is committed. The PBF is not.

## Moldova graph (M1 extract)

Input: `moldova.osm.pbf`, 101,528,058 bytes (Geofabrik, M1 pack).
SHA-256 and pack details: `docs/offline/moldova.md`.

```text
graph: 1490268 nodes, 3117064 edges, max speed 120 km/h, loaded in 8.27 s
```

Peak working set during the build: **255.2 MiB** (measured with
`PeakWorkingSet64` while the CLI ran). Final graph arrays are about 104 MB
(1.49M × 24 B nodes + 3.12M × 20 B edges + offsets); the peak includes the
temporary coordinate/index hash maps.

The 8.27 s figure is a warm-cache, otherwise-idle run on the i5-13420H dev
machine. Repeats while another worktree compiled the Android app (12 `rustc`
processes, 100% CPU) took 41 s and 157 s for the same binary and file, so the
build is CPU-bound and the number is load-sensitive. A cold first read of the
101 MB file was slower again. **Do not parse the PBF at app launch on a phone:**
build the graph once at pack install and persist it (see ANDROID.md).

## Real route: Chișinău → Bălți

Command (PowerShell, release):

```powershell
$env:PINPOINT_MOLDOVA_PBF = "C:\Users\Dinusauuuur\Desktop\git\Pinpoint\dist\offline\moldova\moldova.osm.pbf"
cargo test --release --test moldova -- --ignored --nocapture
```

Output (load time from the run under the other worktree's CPU load; distance and
ETA are stable):

```text
graph: nodes=1490268 edges=3117064 load_s=140.94
route: distance_m=134109.5 duration_s=5836.8 nodes=2037 points=2037 query_ms=300.6
test chisinau_to_balti_is_routed_locally ... ok
```

- **Distance: 134,109.5 m (134.1 km)**
- **Duration: 5,836.8 s (97.3 min), ~82.7 km/h average**
- **Nodes: 2,037; points: 2,037**
- Query time: 232–463 ms in quiet runs, and 1.1–1.5 s while the other
  worktree's Android build saturated all 12 hardware threads. This includes two
  linear nearest-node scans over 1.49M nodes and A*.

CLI (idle, warm cache) produced the same route:

```powershell
.\target\release\native-router.exe route `
  --osm C:\Users\Dinusauuuur\Desktop\git\Pinpoint\dist\offline\moldova\moldova.osm.pbf `
  --from 47.01,28.86 --to 47.76,27.93 --profile car
```

```text
graph: 1490268 nodes, 3117064 edges, max speed 120 km/h, loaded in 8.27 s
route: 134.1 km, 97.3 min, 2037 points, 2037 nodes, solved in 232.5 ms
```

stdout is the `Route` JSON (`distance_m`, `duration_s`, `points`, `nodes`), about
10,000 lines because every graph node is a point. The first/last points snap to
47.0100856,28.8600694 and 47.7599660,27.9303167 (the test asserts both are
within 5 km of the requested coordinates).

## Android cross-compile

NDK 27.2.12479018, Rust target `aarch64-linux-android` via rustup 1.100.0.
VS Build Tools provides the host linker for build scripts. Exact command:

```powershell
$ndk = "$env:LOCALAPPDATA\Android\Sdk\ndk\27.2.12479018"
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = `
  "$ndk\toolchains\llvm\prebuilt\windows-x86_64\bin\aarch64-linux-android24-clang.cmd"

cargo build --release --target aarch64-linux-android --target-dir target-android
```

Result: **success**, exit 0.

```text
Compiling native-router v0.1.0 (...\tools\native-router)
Finished `release` profile [optimized] target(s) in 1m 00s
```

Artifact verification:

```text
target-android/aarch64-linux-android/release/native-router   1,093,040 bytes
ELF64, little endian, Type DYN, Machine AArch64
NEEDED: libdl.so, libm.so, libc.so
target-android/aarch64-linux-android/release/libnative_router.rlib   219,888 bytes
```

**This is an Android ELF binary and rlib, not an APK.** No JNI wrapper, Java
bridge, Gradle packaging or device run is part of this spike. The crate has no C
dependencies; the NDK is used only as linker/sysroot. `ANDROID.md` describes the
JNI shape and the graph-persistence work needed before an app can use it.

Environment note: the machine has a standalone Rust 1.99.1 first on `PATH`
(no Android std) and rustup 1.100.0 with the target. Using a separate
`--target-dir` avoids mixing artifacts from the two compilers. This is a local
toolchain detail, not a crate requirement.

## Rust router vs native Valhalla

Measured facts for the Rust spike:

- One-time graph build: 8.3 s idle, 255 MiB peak, 104 MB resident graph.
- Query: 0.23–0.46 s for a 134 km route on desktop.
- No turn restrictions, no maneuvers, no names/refs, car only.
- Rebuilding from PBF on a phone is not viable; a persisted/mmap'd graph is
  required.

M1's `moldova-valhalla.tar` already contains a Valhalla graph with **510,140
nodes / 1,354,988 directed edges** for car, pedestrian and bicycle (114 s Docker
build). The design's NDK/CMake path would compile Valhalla for `arm64-v8a`, load
the tar from app storage and expose routing + maneuvers through JNI. Pros:
production routing semantics (turn restrictions, maneuvers), smaller graph than
the 1.49M-node OSM graph, likely lower query latency. Cons: a C++ NDK build and
a much larger native artifact, neither of which is proven on this machine yet.

**Decision from this spike:** the Rust fallback is good enough to prove local
routing and is the smaller integration, but it is not a turn-by-turn engine. The
design's Valhalla choice stays the production target. If a route-only offline
milestone is needed first, ship the Rust path **with a prebuilt, persisted graph**
and keep the `Router` seam so Valhalla can replace it without touching the UI.

## Limits

- Car only. No pedestrian/bicycle profiles.
- No turn restrictions, turn penalties, lane guidance, traffic or closures.
- No ferries, no `route=ferry`; `highway=*` ways only.
- Linear nearest-node search; fine for Moldova, not for a continent.
- Outside coverage (>25 km from the nearest graph node) returns
  `RouteError::OutsideCoverage`; callers must show "outside offline area" and
  must not silently call an online router.
- Estimated speeds are routing defaults, not legal limits.
- Memory is the main cost: 255 MiB peak during the build and about 104 MB for
  the resident graph. A* itself needs roughly 30 MB of working arrays for 1.49M
  nodes; one run under severe host memory pressure failed to allocate those
  arrays even though the graph had loaded. A shipping client must budget this
  and must not build the graph on a memory-constrained device at launch.

## Licence

Map data © OpenStreetMap contributors, ODbL 1.0. The PBF and built graph stay
local and are not committed.
