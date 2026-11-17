# Online basemap integration (MapLibre page)

Date: 2026-11-17 · Scope: `web/map/map.js`, `web/map/index.html`.
Endpoints, terms and CORS evidence: [`openfreemap.md`](./openfreemap.md);
attribution: [`odbl-attribution.md`](./odbl-attribution.md).

## 1. `map.js`

`map.js` must stay origin-free (`tests/assets.rs` bans `https://` in it), so the
page passes `createMap({ onlineStyleUrl, … })`. Expose `setBasemap(mode)` and
`getBasemap()`. Keep `localStyle` (the already pmtiles-bound JSON), `basemap`,
`swapSeq`, `prefetch` (`AbortController`) and `onlineTried`.

`setBasemap("online")`: abort the previous prefetch; `fetch(onlineStyleUrl,
{ signal, mode: "cors", cache: "force-cache" })`, throw on `!ok`; bail if
`swapSeq`/`destroyed` changed; `map.setStyle(structuredClone(json), { diff: false })`.
Local mirrors it with `localStyle`. Never `setStyle(null)`.

**`diff: false` is required.** `diff: true` drops every source/layer the new
style lacks (route, pins) and races a rapid toggle. A full swap gives one
`style.load`; rebuild from pending state:

```js
map.on("style.load", () => {
  if (destroyed || !map) return;
  if (basemap === "local") applyTheme();   // Liberty is already themed
  if (locationsRequested) applyLocations();
  if (pendingRoute) applyRoute({ fit: false });
});
```

- `on`, not `once`: the second toggle re-adds too.
- Existing `getSource` guards recreate source *and* layers, same ids.
- Make `applyRoute({ fit = true } = {})`; fit only in `drawRoute`, else every
  toggle re-fits the camera.
- `installLocationHandlers` stays: layer-scoped listeners survive a swap and
  re-fire; re-registering double-fires.
- `applyTheme` returns early online: `THEME_PAINT` ids partly exist in Liberty.
- No accuracy layer in this module (`web/app/map.js` owns
  `pinpoint-self-accuracy`); if added, re-add it in this handler.

Flicker/cancel: prefetch then swap (the gap is tile latency); the container
background already covers the blank frame; the camera survives `setStyle`.
`diff: true` aborts its fetch, `setStyle(url)` does not (the detached style
cannot fire, but bytes still transfer) — the prefetched object avoids that.
Disable the toggle while swapping; `swapSeq` drops late results.

## 2. `index.html`

- `#basemap` in `.controls`, `aria-pressed`, start from
  `localStorage["pinpoint-basemap"] === "online"` (absent = offline); call
  `setBasemap` after `onLoad`.
- `LIBERTY_URL = "https://tiles.openfreemap.org/styles/liberty"` next to
  `MONACO`/`MOLDOVA`; `/map/config` can own it later.
- Keep the `.map-attribution` footer and swap its middle link
  (`openmaptiles.org` ↔ `openfreemap.org`). `attributionControl: false` renders
  the TileJSON credit nowhere; enabling it duplicates credit in local mode. Add
  the privacy line and failure text here.

## 3. CSP and CORS

The map page's headers come from `src/mobile/map_server.rs` (`body_response`,
no CSP today); `web_handlers::secure()` covers other pages and stays strict.
Send the full `web_handlers::CSP` string with these replacements, map response
only (a fragment would leave the page unrestricted):

```text
connect-src 'self' https://tiles.openfreemap.org;
img-src 'self' data: https://tiles.openfreemap.org;
font-src 'self';
worker-src 'self';
```

- `connect-src` is the one that matters: style, TileJSON, tiles, glyphs and
  sprite are CORS `fetch`es from that single origin.
- `font-src 'self'` stays: glyphs are protobuf, not `@font-face`.
- `img-src` is belt-and-braces for the z0–6 Natural Earth raster; MapLibre
  decodes raster/sprite from fetched buffers. Let the violation test decide.
- `worker-src 'self'` is enough: the vendored build loads
  `./maplibre-gl-worker.mjs` via `new Worker(new URL(...), {type:"module"})`.
  Add `blob:` only for the CDN/UMD MapLibre 6 build. Never the tile host in
  `script-src`.
- Live check: `Access-Control-Allow-Origin: *` on style, TileJSON, glyphs and
  sprite, simple GETs — `COEP: require-corp` is satisfied through CORS. No CORP
  header, so never load these with `<img>`/`no-cors`. Serve over `http(s)`.
- Keep the `/` CSP test; add a map-page assertion for this exact `connect-src`.

## 4. Offline and failure

- Off by default; `!navigator.onLine` → message, stay local, send no request.
- Fetch error/`!ok`/no `style.load` in ~10 s → one revert (`onlineTried`),
  status "Online map unavailable; using the offline map", camera kept. No timer,
  no auto-retry; the next user-initiated toggle resets `onlineTried`.
- `error` with `event.sourceId` is a tile failure: keep the basemap, quiet
  notice, never fall back mid-pan. `abort`/`cancel` stays ignored.
- Name the fallback: installed pack or Monaco fixture.

## 5. Tests

New `web/map/test/online-e2e.mjs`, Puppeteer like `smoke.mjs`, interception, no
real network:

1. Serve the style URL as `{version:8, name:"liberty-stub", sources:{},
   layers:[background], attribution, glyphs, sprite}`; 204 everything else.
   No sources → no tile traffic.
2. Assert `getStyle().name`, then `pinpoint-locations`, `…-circles`,
   `pinpoint-route`, `pinpoint-route-casing`, `pinpoint-route-line` exist and
   the route geometry survived.
3. `#basemap` pressed; footer is `openfreemap.org`.
4. Toggle back: `getSource("basemap").url` starts with `pmtiles://`, custom
   layers present, no extra style request.
5. 503 → footer back to `openmaptiles.org`, fallback status, exactly one style
   request after a 2 s settle.
6. `securitypolicyviolation` list empty.
7. Online: off-origin requests only to `tiles.openfreemap.org`. Toggle off keeps
   the existing "no external network requests" checks in `smoke.mjs`,
   `offline-e2e.mjs` and `route-e2e.mjs` untouched.
