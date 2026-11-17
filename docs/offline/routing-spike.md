# Offline routing spike: Valhalla WASM in the browser

Date: 2026-11-17
Branch: `feat/wave2-routing-spike` (from `main` at `e9f8a2e7`)
Scope: M1 Moldova graph, Chișinău → Bălți, headless Chromium on Windows.
Status: **routing works, but not with the two packages named in the design
spec; recommendation is conditional.**

This spike answers one question: can the web/MapLibre client compute a real
turn-by-turn route from the M1 Moldova pack with no network? It does **not**
test offline tiles, search, OPFS storage, Android/iOS WebViews or physical
phones.

## Verdict

| Question | Answer |
|---|---|
| Can a browser route on the M1 graph today? | **Yes**, with `@adret-maps/valhalla-wasm@3.9.0-alpha.2`: 135.002 km, 6047.209 s, 29 maneuvers for `auto`. |
| Can `valhalla-browser@0.2.1` load the M1 tar? | **No.** It requires Valhalla's native indexed `graph.tar` and a Valhalla 3.8.3 dataset. Exact error below. |
| Can `@jansoft/mbujkanji-valhalla-wasm@0.1.2` load it? | **No.** `loadTiles` calls a missing module function. |
| Is this shippable for 2.0 as-is? | **No.** The working package is a pre-release alpha, the M1 graph is 3.9.2 while the engine is 3.9.0, and desktop Chromium memory is high. |
| Recommendation | **Go** for a time-boxed web routing prototype behind an adapter; **no-go** for making web offline parity a 2.0 release gate. Keep native Valhalla for Android/desktop and online routing as the web fallback. |

The design's fallback list is therefore exercised: native offline + web online
is the safe 2.0 behavior; the WASM path is viable enough to prototype, not yet
safe to promise.

## Artifacts

| Artifact | Value |
|---|---|
| M1 tar | `dist/offline/moldova/moldova-valhalla.tar` (not committed) |
| Tar size / SHA-256 | 120,319,488 bytes / `a9fce80fae0e6fea566ee215d916bf98a0af909253fe81b6dedc1c6383fe3fa8` |
| Tiles | 114 `.gph` files, 120,218,576 bytes |
| Graph build | Valhalla **3.9.2-b95151088** (read from the tile headers) |
| Graph size | 510,140 nodes, 1,354,988 directed edges (`docs/offline/moldova.md`) |
| Config | `dist/offline/moldova/moldova-valhalla.json` (not committed) |
| Endpoints | Chișinău `47.01, 28.86` → Bălți `47.76, 27.93` |
| Costings | `auto`, `pedestrian`, `bicycle` |

Machine: Windows 11 IoT Enterprise LTSC 26300, Intel Core i5-13420H (8C/12T),
16 GB RAM. Browser: Puppeteer Chrome **156.0.8078.47**, headless. Node 26.13.0.
No CDN: every module, WASM binary and graph byte came from `127.0.0.1`; the
runner recorded **0 external requests** in every scenario.

## Packages tested

| Package | Version | What it is | Result |
|---|---|---|---|
| `valhalla-browser` | `0.2.1` | Browser Valhalla in a worker; 9,861,835-byte WASM; runtime Valhalla 3.8.3, revision `a60c7cbfc83e073f50887cd27e0109d02e6b64e5` | **No-go on the M1 pack** |
| `@jansoft/mbujkanji-valhalla-wasm` | `0.1.2` | "Valhalla WASM for offline-first web apps"; 6,834,675-byte WASM; 77 direct dependencies | **Broken** |
| `@adret-maps/valhalla-wasm` | `3.9.0-alpha.2` | Valhalla 3.9.0 compiled to WASM; 6,039,068-byte WASM; route/trace/locate; no dependencies | **Routes the M1 graph** |

The design spec spells the Jansoft package `@jansoft/mbukanji-valhalla-wasm`.
That exact name is **not published**:

```text
$ npm view @jansoft/mbukanji-valhalla-wasm version
npm error code E404
npm error 404 Not Found - GET https://registry.npmjs.org/@jansoft%2fmbukanji-valhalla-wasm - Not found
```

The published package is **`@jansoft/mbujkanji-valhalla-wasm`** (with `j`).
The spike tested that package. `valhalla-browser` and
`@adret-maps/valhalla-wasm` are pinned in
`tools/valhalla-web/package.json`; the Jansoft package is installed only for
the negative test with `npm run install:jansoft` because it pulls 699 transitive
packages and reported 14 vulnerabilities (3 critical).

## Test 1 — `valhalla-browser` against the unmodified M1 tar

The package expects a versioned release directory with a manifest, a browser
config and an **indexed** graph archive whose first tar entry is `index.bin`.
The M1 tar is a plain `tar -C ... -cf moldova-valhalla.tar valhalla_tiles`, so
the test pointed a manifest at it and let the SDK validate it.

```text
GET /datasets/moldova-20261116-raw/manifest.json   200
GET /datasets/moldova-20261116-raw/config.json     200
GET /data/moldova-valhalla.tar                     206
```

Exact error:

```text
RoutingError [DATASET]: Archive does not begin with the native index.
    at e.onmessage (node_modules/valhalla-browser/dist/index.js:185:15)
```

This is not a tuning failure: the package's own data guide states that the
native `valhalla_build_extract` must write `index.bin`, and that "an ordinary
`tar -cf`, gzip archive, or hand-written index is not a substitute." The M1
pack was built with `tar -C "$packDir" -cf ...`, so it cannot be consumed by
`valhalla-browser` as shipped.

## Test 2 — `valhalla-browser` with an experimental indexed repack

To give the package a fair second chance, `prepare-sdk-dataset.py` repacked the
same tiles into the SDK's layout: `index.bin` first, 512-byte-aligned tile
bodies, a manifest, and a browser config with the native `/data/...` paths
removed. The script also produced the raw-tar manifest used in Test 1.

```text
indexed graph.tar: 120,310,784 bytes
index.bin: 1,824 bytes (114 × 16)
GET /datasets/moldova-20261116/manifest.json   200
GET /datasets/moldova-20261116/config.json     200
GET /datasets/moldova-20261116/graph.tar       206 (header, index, tiles)
```

Exact error:

```text
RoutingError [RUNTIME]: Valhalla initialization failed.
    at e.onmessage (node_modules/valhalla-browser/dist/index.js:185:15)
```

The `individual-tiles` transport was also tried with the same manifest and the
114 extracted `.gph` files; it failed with the same `RUNTIME` error. The SDK
does not expose the native message (`printErr` is suppressed in its worker), so
this is the complete public error. Two independent blockers explain it:

1. The pack was built by **Valhalla 3.9.2**, the SDK's WASM is **3.8.3**; the
   package guide says a graph from an arbitrary Valhalla installation is not
   automatically compatible.
2. The index was hand-written by this spike, which the package explicitly
   does not support. A correct result requires rebuilding the Moldova pack with
   the SDK's pinned native builder, not repacking the existing tar.

**No-go for `valhalla-browser` on the M1 pack.** A future attempt would rebuild
the pack from the PBF with the SDK's pinned 3.8.3 tools (including
`valhalla_build_extract`), then repeat this test.

## Test 3 — `@jansoft/mbujkanji-valhalla-wasm`

The package initialized, but loading the tar failed in both Node and headless
Chromium:

```text
ValhallaError [201]: this.module.loadTiles is not a function
    at ValhallaRouter.loadTiles (node_modules/@jansoft/mbujkanji-valhalla-wasm/dist/index.js:559:13)
```

The package's `loadTiles` calls `module.loadTiles`, but the shipped
`valhalla.js` glue exposes no such export. There is no package code path that
can load tiles, so the failure is deterministic, not a tar-format issue. The
package also declares React, Next.js, Vite, Playwright, MapLibre, canvas and
many UI packages as **runtime** dependencies (699 packages installed for a
routing library); `npm audit` reported 14 vulnerabilities, including 3 critical.
**No-go.**

## Test 4 — `@adret-maps/valhalla-wasm` on the same M1 tiles

The package takes individual tiles through a synchronous `TileSource`, not a
tar. The spike parses the M1 tar in the page (zero-copy `Uint8Array` views) and
loads either all 114 tiles or only the 24 tiles in a generous corridor around
the route. No network is used after the tar is fetched.

### Route output (exact)

All runs used `language: "en-US"` and `directions_type: "instructions"`.
Valhalla returns `length` in kilometres and `time` in seconds.

| Costing | Distance | Time | Maneuvers | Response time (desktop) |
|---|---:|---:|---:|---:|
| `auto` | 135.002 km | 6047.209 s (1 h 40 min) | 29 | 148–207 ms |
| `pedestrian` | 123.456 km | 87182.171 s (24 h 13 min) | 90 | 140–218 ms |
| `bicycle` | 135.648 km | 32598.951 s (9 h 03 min) | 138 | 150–333 ms |

The ranges are two warm runs (corridor / all tiles). Cold WASM compilation is
reported separately below.

First `auto` maneuvers:

```json
[
  {"type":3,"instruction":"Drive west.","time":1.743,"length":0.045,"travel_mode":"drive","travel_type":"car"},
  {"type":15,"instruction":"Turn left onto Bulevardul Iuri Gagarin/R2.","street_names":["Bulevardul Iuri Gagarin","R2"],"time":5.078,"length":0.059,"travel_mode":"drive","travel_type":"car"},
  {"type":14,"instruction":"Make a sharp left to stay on Bulevardul Iuri Gagarin.","street_names":["Bulevardul Iuri Gagarin"],"time":52.685,"length":0.59,"travel_mode":"drive","travel_type":"car"},
  {"type":26,"instruction":"Enter the roundabout and take the 2nd exit onto Strada Albișoara.","time":0.632,"length":0.008,"travel_mode":"drive","travel_type":"car"}
]
```

The route response also carries `trip.status: 0`,
`trip.status_message: "Found route between points"`, `units: "kilometers"`,
`language: "en-US"`, and a 9,361-character polyline6 leg shape. The readiness
check is a real geometry: the route bbox is
`[27.930316, 47.009668, 28.863722, 47.760239]`.

**Important API detail:** `directions_type: "maneuvers"` returns maneuver
geometry but **empty `instruction` strings**. Use `"instructions"` (the default)
to get text; the response still contains the full maneuver list. Without a
`language`, Valhalla also returned empty instructions in this test.

### Startup and memory

Both runs are in fresh headless Chromium profiles; "load" is
`loadValhalla()` (WASM compile and init), excluding the tar fetch.

| Metric | All 114 tiles (120.2 MB) | Corridor, 24 tiles (49.8 MB) |
|---|---:|---:|
| Tar fetch + parse | 358 ms + 1 ms | 1170 ms + 1 ms |
| WASM load/init | 95 ms | 104 ms |
| Total to first route | 1.2 s | 1.7 s |
| WASM memory after load | 32.0 MiB | 32.0 MiB |
| WASM after `auto` | 100.5 MB | 69.7 MB |
| WASM after pedestrian | 144.7 MB | 120.6 MB |
| WASM after bicycle | 211.5 MB | 144.7 MB |
| Page JS heap / UA-specific | 122.2 MB / 121.7 MB | 122.2 MB / 121.7 MB |
| Chrome process tree RSS | 829 MB | 765 MB |

The process-tree figure includes headless Chrome itself: an empty browser
measured 408 MB on this machine. The app-attributable delta is roughly
360–420 MB for the corridor case and 420–450 MB for the full pack. The
121 MB JS figure is mostly the fetched tar buffer; a production client that
extracts only corridor tiles from OPFS should not keep the full tar in memory.

Cold-cache caveat: the very first tar fetch took **56.8 s** and a later cold
run 20.3 s; warm runs took 0.36–1.7 s. This is Windows filesystem/antivirus
behaviour on the 120 MB file, not package CPU time. Report startup from warm
runs, and budget a one-time pack import separately.

At **4× CPU throttling** (a rough mid-range-phone stand-in, not a real phone):
WASM load 1,077 ms; `auto` 1,577 ms, `pedestrian` 1,363 ms, `bicycle` 1,924 ms.
The route call is synchronous, so the production client must run it in a Web
Worker.

### Why the corridor matters

`MemoryTileSource.get(path)` is synchronous. The engine cannot await a fetch
mid-search. The client must preload the tiles a route may visit. The 24-tile
corridor (27.8–28.95° E, 47.0–47.8° N, with buffer) is enough for this route
and cuts engine memory and tile bytes by more than half. A production adapter
must compute a wider corridor per costing, load it before calling the engine,
and expose a clear "tiles missing; widen the corridor" retry path. The package
README explicitly warns that a route needs every tile its search visits, not
only the endpoint tiles.

## Route API for the web/MapLibre client

This is the proposed contract for `web/map/router.js` (not implemented in this
spike). It hides the engine choice behind the same `Router` seam as the native
side and keeps Valhalla JSON out of the UI.

### Request

```js
const route = await router.route({
  from: { lat: 47.01, lon: 28.86 },
  to: { lat: 47.76, lon: 27.93 },
  costing: "auto",            // auto | pedestrian | bicycle
  language: "en",             // required for instruction text
  units: "kilometers",        // only kilometers in v1
  shapeFormat: "polyline6",   // MapLibre adapter decodes it
});
```

Exactly two locations in v1; no via points, alternates or departure time.
Reject anything else with `INVALID_REQUEST` before touching the engine.
Check both endpoints against the pack manifest's coverage first; do not call
the engine for a point outside Moldova.

### Response (normalized, not raw Valhalla)

```js
{
  distanceKm: 135.002,
  durationSeconds: 6047.209,
  bbox: [27.930316, 47.009668, 28.863722, 47.760239], // west,south,east,north
  shape: { format: "polyline6", value: "..." },
  maneuvers: [{
    type: 15,                         // Valhalla maneuver type
    instruction: "Turn left onto Bulevardul Iuri Gagarin/R2.",
    verbal: "Turn left onto ...",     // optional, for M6 voice
    streetNames: ["Bulevardul Iuri Gagarin", "R2"],
    distanceKm: 0.059,
    durationSeconds: 5.078,
    beginShapeIndex: 7,
    endShapeIndex: 13,
    travelMode: "drive",
    travelType: "car",
  }],
  engine: { package: "@adret-maps/valhalla-wasm", version: "3.9.0-alpha.2", valhallaVersion: "3.9.0" },
  coverage: "inside"
}
```

`distanceKm` and `durationSeconds` stay in Valhalla's units; formatting is a UI
concern. `bbox` is `[west, south, east, north]`, not Leaflet's
`[south, west, north, east]`; convert at the MapLibre boundary.

### Errors

| Code | Meaning / action |
|---|---|
| `OUTSIDE_COVERAGE` | Endpoint outside the pack; show "outside offline area" and offer online routing. Never silently fall back. |
| `NO_ROUTE` | Valhalla 442; show "no route" and keep endpoints. |
| `LOCATION_NOT_FOUND` | Valhalla 171; ask the user to move the pin nearer a road. |
| `TILES_MISSING` | The prefetched corridor did not cover the search; widen and retry once. |
| `UNSUPPORTED_COSTING` | Pack does not advertise the profile. |
| `ENGINE_INIT` | WASM/dataset mismatch or corrupt pack; disable offline routing and report the pack. |
| `CANCELLED` | Worker was terminated; recreate it on the next route. |

Preserve `nativeCode` when present for diagnostics, but never show raw Valhalla
errors to the user.

### Worker and tile contract

```
main thread                     routing worker
-----------                     --------------
init(pack, wasm)        --->    load WASM, open pack index
loadTiles(paths)        --->    read .gph bytes, build TileSource
route(request)          --->    synchronous Valhalla call
        <--- normalized result or typed error
cancel()                --->    terminate worker (only reliable preemption)
```

- Keep one worker per map session; reuse it for route/profile changes.
- `TileSource.get(path)` must be synchronous. Prefetch corridor paths from the
  pack before `route()`; do not fetch inside the search.
- Transfer tile bytes into the worker once; do not clone the whole pack per
  route.
- The engine call blocks the worker for 0.15–0.33 s on desktop and 1.3–1.9 s at
  4× throttle. The UI thread stays responsive; show a route spinner and a
  cancel button that terminates the worker.
- The adapter maps missing tiles to `TILES_MISSING`; do not let the engine turn
  them into a misleading `NO_ROUTE`.

### MapLibre integration

```js
import { decodePolyline6 } from "./polyline.js"; // tiny local decoder

const coordinates = decodePolyline6(route.shape.value); // [[lon, lat], ...]
map.addSource("route", {
  type: "geojson",
  data: { type: "Feature", properties: {}, geometry: { type: "LineString", coordinates } },
});
map.addLayer({
  id: "route-line",
  type: "line",
  source: "route",
  paint: { "line-color": "#0b7a5c", "line-width": 5, "line-opacity": 0.9 },
});
map.fitBounds(route.bbox, { padding: 48 });
```

- Decode polyline6 to `[longitude, latitude]`; MapLibre is lon/lat, Leaflet was
  lat/lon.
- The turn banner uses `maneuvers[i].instruction` and the current position's
  shape index; `beginShapeIndex`/`endShapeIndex` index the decoded line.
- Keep the existing OSM/ODbL attribution visible; the router adds no tiles and
  no network.
- On `OUTSIDE_COVERAGE`, show a non-modal banner with an explicit "Use online
  route" action. Do not mix online and offline results silently.

### Pack/API implications

`@adret-maps/valhalla-wasm` cannot read the M1 tar. The web pack needs one of:

1. A pack-v2 `routing/index.json` with each tile's path, offset and size, plus
   a tar reader in the worker; or
2. Extracted `routing/tiles/<path>.gph` files in OPFS at install time.

Either way, keep `moldova-valhalla.tar` for the native clients, add the web
routing index to the manifest, and verify SHA-256 per tile on import. The native
`Router` trait remains the seam; the web adapter is one implementation.

## Commands and exact outputs

Setup:

```powershell
cd tools/valhalla-web
npm ci
python prepare-sdk-dataset.py `
  --tar C:\Users\Dinusauuuur\Desktop\git\Pinpoint\dist\offline\moldova\moldova-valhalla.tar `
  --config C:\Users\Dinusauuuur\Desktop\git\Pinpoint\dist\offline\moldova\moldova-valhalla.json `
  --out .data\sdk-dataset
$env:NODE_PATH = "C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\node_modules"
```

Positive path:

```powershell
node run.mjs --only adret,adret-corridor
```

Key output:

```text
adret           ok True   totalMs 1215   auto 135.002 km / 6047.209 s / 29 maneuvers
adret-corridor  ok True   totalMs 1715   auto 135.002 km / 6047.209 s / 29 maneuvers
```

Negative paths:

```powershell
node run.mjs --only vb-raw,vb-indexed
npm run install:jansoft
node run.mjs --only jansoft
```

Key output:

```text
vb-raw      ok False  DATASET  Archive does not begin with the native index.
vb-indexed  ok False  RUNTIME  Valhalla initialization failed.
jansoft     ok False  ValhallaError 201  this.module.loadTiles is not a function
```

Full JSON (route geometry, maneuvers, startup timings, page/process memory,
every request and status) is written under `tools/valhalla-web/.data/results/`.
Those files are intentionally ignored; the committed harness regenerates them.

## Recommendation

**GO — conditional, prototype only.**

1. Use `@adret-maps/valhalla-wasm@3.9.0-alpha.2` behind a `Router` adapter for
   web offline routing. It is the only tested package that actually routes the
   M1 graph, returns instructions, and stays local.
2. Add a pack-v2 routing index / extracted `.gph` tiles, a worker-owned
   `TileSource`, corridor prefetch and the normalized API above.
3. Rebuild or revalidate the Moldova pack with Valhalla **3.9.0**, the engine's
   version. The 3.9.2 tiles worked for this route, but the package says graph
   formats can change between versions; a one-minor-version mismatch is not a
   shipping guarantee.
4. Gate the feature on real Android Chrome and iOS Safari tests (memory, route
   time, backgrounding, OPFS). Desktop headless Chromium is not evidence for
   physical mobile browsers.
5. Keep native Valhalla for Android/desktop and online routing for web as the
   2.0 default. Do not make web offline routing a release blocker until the
   mobile gate passes.

**NO-GO — do not ship either requested package as the web router.**

- `valhalla-browser@0.2.1`: requires a 3.8.3 native indexed pack; the M1 tar
  fails immediately and the experimental repack fails initialization. Rebuild
  the pack with its pinned tools before reconsidering.
- `@jansoft/mbujkanji-valhalla-wasm@0.1.2`: `loadTiles` is not implemented in
  the published artifact, and its dependency surface is unsuitable.

If the @adret prototype fails the mobile gate, use the design's explicit
fallback: **native Valhalla offline on Android/desktop, online routing on web,
and delay web offline parity.** Do not build a Rust router solely to rescue web
parity unless native/WASM options are exhausted; the spec's Rust fallback remains
for the native/WASM path, not as a first response to this result.

## Reproduction limits

- No physical Android Chrome or iOS Safari test; no WebView2/WebKitGTK test.
- No OPFS, service worker, pack download or SHA-256 import path; the tar was
  fetched into page memory.
- No route matrix, via points, alternates, traffic, time-dependent routing or
  voice.
- One route pair, one OSM snapshot. The result is a feasibility signal, not a
  routing-quality benchmark.
- The experimental `valhalla-browser` manifest/index is deliberately not a
  supported pack format. Do not copy it into `scripts/offline/`.
