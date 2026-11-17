# Pinpoint shared map foundation

A standalone MapLibre GL JS + PMTiles module and preview page. It proves that
the M1 `monaco.pmtiles` fixture renders as an offline vector map with local
friend markers, and it is the component the web app, Android WebView and
desktop WebView will consume.

This is a **foundation**, not the finished offline feature. The preview now
wires the on-device `native_router` car route to a panel and `/map/route` on
both loopback map servers. The pack download manager, OPFS storage and
service worker live in the web-app stream; the `search.js` module already
searches the M1 `places`/`places_fts` SQLite index on-device (see
[`search.md`](search.md)).

## Run the preview

From the repository root:

```powershell
node web/map/serve.mjs
# open http://127.0.0.1:8080/map/index.html
```

or pick another port:

```powershell
node web/map/serve.mjs 9000
```

PMTiles reads tiles with HTTP Range requests, so the static server **must**
support `Range`. `web/map/serve.mjs` does. `python -m http.server` does not
implement Range and will make PMTiles fail; `npx serve` and normal static hosts
generally do support it. Opening `index.html` from `file://` will not work
because ES modules, workers and Range requests need an HTTP origin.

Everything the page loads is local: MapLibre, its worker, PMTiles, the style
and the fixture. There are no runtime CDN imports and the style has no glyphs,
sprites or remote tile URLs.

## Files

| Path | Purpose |
|---|---|
| `web/map/index.html` | Preview page with controls, status and visible attribution |
| `web/map/preview.css` | Preview-only styling |
| `web/map/map.js` | ES module API (`createMap`, markers, clicks, route line, theme, destroy) |
| `web/map/search.js` | On-device SQLite FTS5 search API (`openSearch`, `search`, `close`) |
| `web/map/search.md` | Search API, hosting, CSP and WebView integration notes |
| `web/app/route-ui.js` | Route planner, summary and turn-by-turn component (`createRouteUI`) |
| `web/app/route.css` | Route component styles, scoped under `.pinpoint-route` |
| `web/map/style.json` | Minimal OpenMapTiles-schema style for the fixture |
| `web/map/fixtures/monaco.pmtiles` | 94 KB ODbL test fixture |
| `web/map/fixtures/balti-search.sqlite` | 24 KB ODbL FTS5 search fixture (Bălți/Кишинёв) |
| `web/map/fixtures/README.md` | Fixture provenance, build date and ODbL notice |
| `web/map/serve.mjs` | Dependency-free static server with Range support |
| `web/map/test/smoke.mjs` | Static + headless-browser map smoke test |
| `web/map/test/search-smoke.mjs` | HTTP + Chromium/Node offline-search smoke test |
| `web/map/test/route-e2e.mjs` | Chromium E2E for the local route panel, run by `tests/map_route_e2e.rs` |
| `web/vendor/maplibre-gl.js` | MapLibre GL JS 6.18.0 ES module |
| `web/vendor/maplibre-gl-worker.mjs` | MapLibre 6.18.0 worker (required sibling) |
| `web/vendor/maplibre-gl.css` | MapLibre 6.18.0 stylesheet |
| `web/vendor/pmtiles.js` | PMTiles 4.5.0 browser IIFE build |
| `web/vendor/sqlite3.mjs` | `@sqlite.org/sqlite-wasm` 3.54.1-build1 engine |
| `web/vendor/sqlite3.wasm` | SQLite 3.54.1 WASM binary with FTS5 |

## Install in a page

```html
<link rel="stylesheet" href="/vendor/maplibre-gl.css">

<div id="map" style="height: 480px"></div>

<script type="module">
  import { createMap } from "/map/map.js";

  const map = createMap({
    container: "#map",
    styleUrl: "/map/style.json",
    pmtilesUrl: "/map/fixtures/monaco.pmtiles",
    center: [7.4246, 43.7384], // [longitude, latitude]
    zoom: 13.2,
    onLoad: () => console.log("map ready"),
    onError: (error) => console.error("map failed", error),
  });

  map.setLocations([
    {
      user_id: 1,
      user_name: "Ana",
      latitude: 43.7384,
      longitude: 7.4246,
      precision_level: "exact",
    },
  ]);

  map.onClick(({ lat, lon }) => console.log("clicked", lat, lon));
</script>
```

The container must have a non-zero height before `createMap` runs. The module
adds the `pinpoint-map` class and its own small popup/theme stylesheet; the host
owns layout and sizing.

`map.js` loads `../vendor/pmtiles.js` itself if `globalThis.pmtiles` is not
already present. The preview also includes the script tag explicitly so the
dependency is visible in the page. If the host uses a strict CSP, allow
`script-src 'self'` (or preload the script with the page's nonce).

## API

### `createMap(options)`

Returns a map handle synchronously. Initialization (PMTiles script, style
fetch, MapLibre map creation) is asynchronous; `onLoad` fires after MapLibre's
`load` event and the first render.

| Option | Type | Notes |
|---|---|---|
| `container` | `HTMLElement \| string` | Required. Element or selector. |
| `styleUrl` | `string` | Required. Local style URL. |
| `pmtilesUrl` | `string` | Required. Local PMTiles path, absolute/root-relative/page-relative. |
| `center` | `[lon, lat]` | Default `[0, 0]`. |
| `zoom` | `number` | Default `2`. |
| `onLoad` | `(handle) => void` | Optional. |
| `onError` | `(error) => void` | Optional. Style, worker and tile errors. |

The style must contain at least one vector source whose `url` starts with
`pmtiles://`. `createMap` rewrites every such source to the `pmtilesUrl`
passed by the caller, which is how the same style works for the fixture, a
downloaded Moldova pack or a loopback WebView path. It throws if the style has
no such source.

`pmtilesUrl` can be relative (`fixtures/monaco.pmtiles`), root-relative
(`/packs/moldova.pmtiles`) or absolute. All style and library URLs are derived
from the module's own location, so the page works under any static origin or
sub-path.

### `setLocations(locations)`

Replaces the friend markers and returns the number of accepted locations.
Each location is:

```js
{
  user_id: 1,             // number or string
  user_name: "Ana",
  latitude: 43.7384,
  longitude: 7.4246,
  precision_level: "exact" // exact | approximate | city_only | anything else
}
```

Invalid or out-of-range coordinates are skipped. Colours follow the existing
Rust map: exact green, approximate amber, city-only red, unknown grey. Clicking
a circle opens a popup with the name, precision and coordinates. Markers are
in-memory GeoJSON only; they are not persisted after a reload. Safe to call
before the style has loaded — the latest call wins.

### `onClick(callback)`

Registers a general map click callback receiving `{ lat, lon }`. Returns an
unsubscribe function. Marker clicks also emit this event.

### `setTheme("light" | "dark")`

Sets `data-theme` and the `pinpoint-map--dark` class on the container and
recolours the known layer ids from `style.json` (background, land, water,
boundaries, rail and roads). A replacement style with different layer ids keeps
its own colours but still gets the CSS class.

### `destroy()`

Removes the map, canvas, listeners and popups. The PMTiles protocol stays
registered so a later `createMap` can reuse it. Calling `destroy` twice is safe.

### `drawRoute(routeOrPoints)` / `clearRoute()`

`drawRoute` accepts the route UI shape (`points: [[lat, lon], ...]`) or a plain
point list. It creates or updates one GeoJSON source with a casing and a line
layer, recolours them with the active theme and fits the map bounds. Safe before
the style finishes loading: the latest call is drawn on load. Returns `false`
when there are fewer than two valid points. `clearRoute` removes the layers and
source and is safe to call repeatedly.

### Extras

- `resize()` — call after the container changes size.
- `getMap()` — returns the underlying MapLibre map (or `null` before
  initialization) for advanced use such as `flyTo`.

## Local routing panel

The preview mounts `web/app/route-ui.js` beside the map. It defaults to
Chișinău `47.0105, 28.8638` → Bălți `47.76, 27.93` and sends only:

```text
GET /map/route?from_lat=47.0105&from_lon=28.8638&to_lat=47.76&to_lon=27.93&profile=auto
```

The desktop (`src/map_server.rs`) and Android (`src/mobile/map_server.rs`)
loopback servers answer from the same `src/local_route.rs` handler and the
on-device `native_router` graph. There is no online routing service and no
fallback. The response is `{distance_m, duration_s, points, maneuvers}`; the
page draws it with `drawRoute` and the component renders the summary and
directions. Clear removes the line; Stop only ends guidance.

The graph cache is resolved lazily from `PINPOINT_ROUTER_GRAPH`, otherwise
`<app data>/packs/moldova.graph.bin`. A missing or invalid cache returns
`503 {"error":"ROUTING_UNAVAILABLE"}`. Missing/malformed coordinates return
`400`, and outside-coverage/no-path return `404`. Only the car profile is
implemented; the page disables Walk and Bike instead of pretending they work.

```powershell
$env:PINPOINT_ROUTER_GRAPH="C:\path\to\moldova.graph.bin"
# start the desktop map server, then open its /map/index.html
```

## Offline search

`web/map/search.js` is a separate ES module for the M1 SQLite search index. It
does not require MapLibre and does not touch `map.js`:

```js
import { openSearch } from "/map/search.js";

const search = await openSearch({
  sqliteUrl: "/map/fixtures/balti-search.sqlite",
});
console.log(search.search("balti"));
search.close();
```

The full API, tokenisation rules, ranking, CSP guidance and the COOP/COEP
position are in [`search.md`](search.md). The preview page does **not** wire a
search box yet; that integration belongs to the web-app stream.

## Style contract

`style.json` renders `background`, `water`, `waterway`, `landuse`, `landcover`
(when present), `boundary`, `transportation` roads/rail and no labels. It uses
the OpenMapTiles 3.16 layer names produced by the M1 pipeline.

There are no `glyphs` or `sprite` keys, so the page never fetches a font or
sprite host. The source `attribution` and style metadata carry the OSM,
OpenMapTiles and ODbL notice. The module sets `attributionControl: false`;
**the host page must show attribution itself**. The preview does, in the
bottom-right overlay:

```html
Map data ©
<a href="https://www.openstreetmap.org/copyright">OpenStreetMap contributors</a>
· <a href="https://www.openmaptiles.org/">OpenMapTiles</a>
· <a href="https://opendatacommons.org/licenses/odbl/1-0/">ODbL 1.0</a>
```

Do not hide or remove it. See `fixtures/README.md` for the fixture's ODbL
provenance and build date.

## Pinned versions

| Package | Version | Upstream |
|---|---|---|
| `maplibre-gl` | `6.18.0` | <https://www.npmjs.com/package/maplibre-gl/v/6.18.0> |
| `pmtiles` | `4.5.0` | <https://www.npmjs.com/package/pmtiles/v/4.5.0> |
| `@sqlite.org/sqlite-wasm` | `3.54.1-build1` | <https://www.npmjs.com/package/@sqlite.org/sqlite-wasm/v/3.54.1-build1> |

Exact tarball URLs, SHA-256 hashes, build notes and licenses are in
`web/vendor/README.md`. MapLibre 6 ships as an ES module and resolves its
worker relative to `maplibre-gl.js`, so `maplibre-gl-worker.mjs` must stay in
the same directory. PMTiles is the IIFE browser build and bundles `fflate`.
SQLite WASM is vendored instead of `sql.js` because the M1 index uses FTS5 and
the stock `sql.js` browser build does not compile FTS5.

## Test

Static checks only:

```powershell
node web/map/test/smoke.mjs --static-only
```

Full render test with the Puppeteer installation available in this environment:

```powershell
$env:NODE_PATH="C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\node_modules"
$env:PINPOINT_SCREENSHOT="C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\web-foundation.png"
node web/map/test/smoke.mjs
```

The test starts `serve.mjs` on a random loopback port, checks HTTP 200 and MIME
types for the page, style, fixture, vendored JS/CSS and licenses, checks a
byte-range request (`206`) and the fixture SHA-256, then opens
`/map/index.html` in headless Chromium. It waits for `window.__pinpointMapReady`
(set from MapLibre's `load` event), asserts a non-zero canvas, the three sample
markers, the `pmtiles://` source binding and visible attribution, checks that no
request left the local origin, and saves a screenshot. If Puppeteer is absent
it prints a clear `SKIP browser` line and runs the static checks only.

Also useful:

```powershell
node --check web/map/map.js
node web/map/test/search-smoke.mjs
```

`search-smoke.mjs` serves the same tree, checks the pinned SQLite assets and
fixture, then runs the real search module (Chromium when Puppeteer is
available, Node fallback otherwise). It asserts `balti` → Bălți,
`кишин` → Кишинёв, punctuation-only → `[]`, and local-only traffic.

The route panel has its own ignored Rust + Chromium E2E (synthetic graph by
default, real Moldova cache with `PINPOINT_E2E_GRAPH`):

```powershell
$env:NODE_PATH="C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\node_modules"
cargo test --locked --test map_route_e2e -- --ignored --nocapture
```

It asserts the local `/map/route` response, summary, maneuver list, casing and
line layers, fitted bounds, Stop keeps the line, Clear removes it, and no
external/failed request, page error or console error.

## Limitations and follow-up stream

- **No offline packs yet.** This loads a local PMTiles file that is already on
  the origin. The download manager, manifest verification, SHA-256 check,
  atomic replace, progress UI, OPFS storage and service worker are the next
  web stream. The web app consumes this module through `web/app/map.js`; the
  Rust server serves it under `/map-assets/*` and the vendored libraries under
  `/vendor/*`.
- **No labels.** Glyphs and sprites are deliberately absent. A production
  style needs vendored glyphs/sprites and a label layer pass.
- **Search is offline-first in the app.** `web/app/offline-search.js` opens the
  installed pack's SQLite FTS5 index through the service worker and `nav.js`
  falls back to the existing server search when no pack is installed. The
  standalone preview page still has no search box.
- **Routing UI is not wired to a local engine yet.** The map adapter draws
  routes and the route UI component is ready, but the app still calls the
  server route endpoint until the native/WebAssembly router is connected.
- **No persistence of friend pins.** `web/app/app.js` still owns session-backed
  friend data; the adapter maps `setFriendMarkers` onto `setLocations` and keeps
  the existing auth, share and privacy behavior.
- **Markers are not clustered.** `setLocations` renders one circle per valid
  location; large friend lists may need clustering later.
- **Theme support is style-aware.** `setTheme` recolours the layer ids in this
  style; a different style needs either matching ids or its own theme handling.
- **Android/desktop WebView wiring is separate.** A loopback HTTP server with
  Range support, the JNI/wry bridge, CSP and packed file paths belong to the
  Android/desktop stream. This module already uses relative URLs so the same
  page works there.
- **Range support is mandatory.** Static hosts or WebView servers that ignore
  `Range` will not work with PMTiles. The dev server here is the reference.
- **PMTiles IIFE is loaded with a script tag.** If a host CSP forbids inline
  script injection, preload `/vendor/pmtiles.js` before importing the module
  (the preview does this).
