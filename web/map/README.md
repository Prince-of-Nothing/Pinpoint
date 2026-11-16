# Pinpoint shared map foundation

A standalone MapLibre GL JS + PMTiles module and preview page. It proves that
the M1 `monaco.pmtiles` fixture renders as an offline vector map with local
friend markers, and it is the component the web app, Android WebView and
desktop WebView will consume.

This is a **foundation**, not the finished offline feature. There is no pack
download manager, OPFS storage, service worker, search or routing here yet.

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
| `web/map/map.js` | ES module API (`createMap`, markers, clicks, theme, destroy) |
| `web/map/style.json` | Minimal OpenMapTiles-schema style for the fixture |
| `web/map/fixtures/monaco.pmtiles` | 94 KB ODbL test fixture |
| `web/map/fixtures/README.md` | Fixture provenance, build date and ODbL notice |
| `web/map/serve.mjs` | Dependency-free static server with Range support |
| `web/map/test/smoke.mjs` | Static + headless-browser smoke test |
| `web/vendor/maplibre-gl.js` | MapLibre GL JS 6.18.0 ES module |
| `web/vendor/maplibre-gl-worker.mjs` | MapLibre 6.18.0 worker (required sibling) |
| `web/vendor/maplibre-gl.css` | MapLibre 6.18.0 stylesheet |
| `web/vendor/pmtiles.js` | PMTiles 4.5.0 browser IIFE build |

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

### Extras

- `resize()` — call after the container changes size.
- `getMap()` — returns the underlying MapLibre map (or `null` before
  initialization) for advanced use such as `flyTo`.

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

Exact tarball URLs, SHA-256 hashes, build notes and licenses are in
`web/vendor/README.md`. MapLibre 6 ships as an ES module and resolves its
worker relative to `maplibre-gl.js`, so `maplibre-gl-worker.mjs` must stay in
the same directory. PMTiles is the IIFE browser build and bundles `fflate`.

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
```

## Limitations and follow-up stream

- **No offline packs yet.** This loads a local PMTiles file that is already on
  the origin. The download manager, manifest verification, SHA-256 check,
  atomic replace, progress UI, OPFS storage and service worker are the next
  web stream. The web app consumes this module through `web/app/map.js`; the
  Rust server serves it under `/map-assets/*` and the vendored libraries under
  `/vendor/*`.
- **No labels.** Glyphs and sprites are deliberately absent. A production
  style needs vendored glyphs/sprites and a label layer pass.
- **No search or routing.** No SQLite WASM, Valhalla WASM or search bar. The
  existing online `/map/*` search and routing UI is unchanged by this module.
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
