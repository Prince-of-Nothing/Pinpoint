# Route and turn-by-turn UI

`route-ui.js` is a self-contained ES module for the Pinpoint offline route
screen. It renders the planner, summary, maneuver list and the visual
next-maneuver banner. It makes **no network requests**: the routing engine is
injected by the host, and the component only calls that function.

This is the UI half of the offline-routing stream described in
[`docs/superpowers/specs/2026-11-16-offline-maps-design.md`](../../docs/superpowers/specs/2026-11-16-offline-maps-design.md)
§10. The Valhalla/Rust engine, the map line and geolocation are wired by the
integration stream. Nothing here is imported by `web/app/index.html` yet, and
no shared app file was changed.

## Files

| Path | Purpose |
|---|---|
| `web/app/route-ui.js` | ES module (`createRouteUI`, `formatDistance`, `formatDuration`, `parseLatLon`) |
| `web/app/route.css` | Component styles, scoped under `.pinpoint-route` |
| `web/app/route-demo.html` | Standalone demo page with visible OSM/ODbL attribution |
| `web/app/route-demo.js` | Synthetic Chișinău → Bălți stub router and SVG preview |
| `web/app/test/route-ui.mjs` | Static + headless Chromium smoke test |

## Run the demo

From the repository root:

```powershell
node web/map/serve.mjs 8080
# open http://127.0.0.1:8080/app/route-demo.html
```

`web/map/serve.mjs` is used because it serves `.mjs`/`.js` with the right MIME
types. The demo loads only local files; its attribution links are the only
absolute URLs. It does not need PMTiles or Range support, but that server is
already the repository's static server.

The demo router is a **stub**. It returns a hand-written geometry and two
maneuvers regardless of the coordinates it receives, so the UI states can be
tested without a routing engine. It is not OSM-derived and is not a real road
route. The page says so next to the preview.

## API

```js
import { createRouteUI } from "./route-ui.js";

const ui = createRouteUI({
  container: "#route-ui", // element or selector, required
  route: async ({ from, to, profile }) => engine.route(from, to, profile),
  onRoute: ({ route, request }) => {
    // draw the line and fit the map
  },
  onCancel: ({ reason }) => {
    // reason: "stop" | "clear"
  },
});
```

### `createRouteUI({ container, route, onRoute, onCancel })`

| Option | Type | Contract |
|---|---|---|
| `container` | `HTMLElement \| string` | Required. The component appends itself here and owns only its own subtree. |
| `route` | `(request) => Promise<route>` | Injected local router. If omitted, pressing Route shows "No offline routing engine is available for this page." |
| `onRoute` | `({route, request}) => void` | Called after every displayed route, including `setRoute`. Draw the line, fit the map, save the route. Treat the objects as read-only. |
| `onCancel` | `({reason, route}) => void` | Called for user-initiated cancellation. `"stop"` ends guidance and keeps the route; `"clear"` means remove the line too. |

`request` is exactly:

```js
{
  from: { lat: 47.0105, lon: 28.8638 },
  to: { lat: 47.7615, lon: 28.1940 },
  profile: "auto" // "auto" | "pedestrian" | "bicycle"
}
```

`route` must resolve to:

```js
{
  distance_m: 128400,
  duration_s: 6600,
  points: [[47.0105, 28.8638], [/* ... */]],
  maneuvers: [{ instruction: "Turn right onto ...", distance_m: 120 }]
}
```

`points` are `[latitude, longitude]` pairs. `distance_m` on a maneuver is the
length of that leg; the banner renders it as `in 42.0 km`. Extra fields on the
top-level object and on maneuvers are preserved and passed to `onRoute`, which
leaves room for Valhalla's maneuver type, bearing and shape index without a
breaking change. Invalid points are dropped; maneuvers without a non-empty
`instruction` are dropped; a missing/invalid `distance_m` or `duration_s`
rejects the route and shows an error.

### Handle

| Member | Behaviour |
|---|---|
| `setRoute(route, request?)` | Displays a route without calling the injected router. Useful for a restored route or an external engine. Throws `TypeError` for an invalid route and leaves the current UI unchanged. With `request`, the From/To fields are filled. Fires `onRoute`. |
| `clear()` | Stops guidance, removes the route, summary, maneuvers and banner, and returns to idle. Does **not** fire `onCancel` (programmatic cleanup). The Clear button does fire it. |
| `start()` / `stop()` | Programmatic equivalents of the Start/Stop button. They do not fire `onCancel`. |
| `getRoute()` | Current normalised route, or `null`. |
| `getRequest()` | Request that produced the current route, or `null`. |
| `destroy()` | Removes the component subtree and ignores any in-flight route result. Safe to call twice. Does not fire callbacks. |

The element is exposed as `ui.element` for host layout or testing. The module
does not touch `document` outside its own subtree except for resolving the
container selector.

### UI behaviour

- From/To accept `lat, lon` (comma, semicolon or whitespace; decimal point).
  Field-specific validation messages are shown in a `role="alert"` element.
- Travel mode is a three-way `aria-pressed` group: Auto, Walk, Bike. Changing
  mode does not auto-route; press Route again.
- Route shows `Routing…` while the promise is pending, disables the action
  buttons/temporarily blocks duplicate submissions, and keeps stale results from
  a superseded request from rendering.
- The summary reads e.g. `128.4 km · 1 h 50 min · Auto`.
- Start enters visual guidance: the next-maneuver banner shows maneuver 1 of N
  and highlights the matching direction. **Next maneuver** advances the preview.
  Stop leaves guidance and keeps the route. No GPS or voice is involved.
- Clear removes the route and tells the host through `onCancel({reason:
  "clear"})`; Stop tells it through `onCancel({reason: "stop"})`.
- Directions are rendered as an accessible ordered list; coordinates remain in
  the fields after Clear so a re-route is one click away.

## Styling

Link the stylesheet once, next to the other app CSS:

```html
<link rel="stylesheet" href="/route.css" />
```

The component scopes everything under `.pinpoint-route` and derives colours
from the app's variables (`--panel`, `--panel-2`, `--border`, `--text`,
`--muted`, `--accent`, `--danger`) with dark-mode fallbacks. The shared map page
sets `data-theme="light"` on the document root; `route.css` includes matching
light values. Layout is responsive down to narrow phones, and every control has
a visible `:focus-visible` outline. It uses no external fonts, images or
`@import`.

## Hook up a local routing engine

The component deliberately knows nothing about how a route is produced. The
adapter belongs to the integration stream and must stay local.

### Native / WebView (Android, desktop)

Native Valhalla (or the Rust A* fallback behind the same `Router` trait) runs
behind the existing platform bridge. The JS side exposes a same-page function:

```js
const ui = createRouteUI({
  container: "#route-ui",
  route: async ({ from, to, profile }) => {
    const raw = await window.PinpointBridge?.route({
      from_lat: from.lat,
      from_lon: from.lon,
      to_lat: to.lat,
      to_lon: to.lon,
      mode: profile,
    });
    if (!raw) throw new Error("Outside offline area. Download the Moldova pack or use the online router.");
    return raw; // { distance_m, duration_s, points, maneuvers }
  },
  onRoute: ({ route }) => mapApi.drawRoute(route),
  onCancel: ({ reason }) => {
    if (reason === "clear") mapApi.clearRoute();
  },
});
```

The bridge must serve the local pack only. Do not substitute a public routing
service when the engine is unavailable; show the "outside offline area" error
instead, as §10 requires. `route()` rejecting with an `Error` is rendered
verbatim in the alert region.

### Web (Valhalla WASM in a worker)

Run the engine in a worker and return plain JSON:

```js
const worker = new Worker("/route/valhalla-worker.js", { type: "module" });

function routeEngine({ from, to, profile }) {
  return new Promise((resolve, reject) => {
    const id = crypto.randomUUID();
    const onMessage = (event) => {
      if (event.data.id !== id) return;
      worker.removeEventListener("message", onMessage);
      if (event.data.error) reject(new Error(event.data.error));
      else resolve(event.data.route);
    };
    worker.addEventListener("message", onMessage);
    worker.postMessage({ id, from, to, profile });
  });
}
```

The same adapter can later be replaced by a Rust/WASM A* router without touching
`route-ui.js`. Keep the worker and tiles same-origin (CSP `connect-src 'self'`);
the component itself never opens a connection.

### Drawing the route line

`onRoute` receives `[lat, lon]` pairs, so a MapLibre host can build a GeoJSON
`LineString` (longitude first) and add or update a source/layer. A Leaflet host
can pass `route.points.map(([lat, lon]) => [lat, lon])` to `L.polyline`. Remove
the layer in `onCancel` when `reason === "clear"`. Fit the map in `onRoute` when
the route is set; do not fit on every Start.

### Not in this component

- GPS following and off-route detection (the existing `nav.js` geolocation
  stream, or a new adapter).
- Voice guidance via platform TTS or `SpeechSynthesis` (design M6 stretch).
- Persisting route history, sharing, GPX/KML export (existing `nav.js`).
- The map itself and offline pack management.

These are integration steps, not dependencies of this module.

## Test

Static checks plus a headless Chromium smoke test (Puppeteer is expected to be
available through `NODE_PATH` in this environment):

```powershell
$env:NODE_PATH="C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\node_modules"
$env:PINPOINT_SCREENSHOT="C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\web-route-ui.png"
node web/app/test/route-ui.mjs
```

The test serves `web/` on a random loopback port, checks the module/stylesheet/
demo MIME types and that `route-ui.js` contains no `fetch`/XHR/WebSocket and no
absolute URLs, opens `route-demo.html`, exercises validation, fills Chișinău →
Bălți, picks Auto, routes, asserts the summary and both maneuvers, starts
guidance, advances to the second maneuver, clicks Clear, and fails on any page
error, console error, failed request or external request. It saves a screenshot
where `PINPOINT_SCREENSHOT` points.

Syntax-only fallback when Puppeteer is unavailable:

```powershell
node --check web/app/route-ui.js
node --check web/app/route-demo.js
node web/app/test/route-ui.mjs --static-only
```

Without Puppeteer the test exits non-zero unless `--static-only` is passed:
a static-only run is explicitly not called a pass for the browser behaviour.

## Attribution

Any page that displays routes over OpenStreetMap-derived data must keep the
visible attribution required by the map foundation:

```html
Map data ©
<a href="https://www.openstreetmap.org/copyright">OpenStreetMap contributors</a>
· <a href="https://opendatacommons.org/licenses/odbl/1-0/">ODbL 1.0</a>
```

`route-demo.html` includes it in the footer. Do not hide or remove it when the
component is embedded in the web app or the WebView page.

## Limitations

- The demo router is synthetic and does not derive from the actual pack.
- Turn-by-turn is visual only and advances with the Next maneuver button; there
  is no GPS, heading, off-route detection or voice yet.
- `points` are consumed by the host; this component does not draw a map.
- Maneuver distances are shown as provided by the engine. If the engine reports
  "distance to the maneuver" rather than "leg length", map it in the adapter so
  the banner text stays truthful.
- No route alternatives, via points or avoid options yet; the spec's route
  response currently has one route.
