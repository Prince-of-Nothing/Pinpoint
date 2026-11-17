# Android/desktop WebView: opt-in OpenFreeMap basemap

**Status:** Proposed, v1 · **Date:** 2026-11-17
**Scope:** the loopback map page (`web/map/index.html`) in the Android WebView and
the desktop `wry` window. Endpoints re-checked live 2026-11-17. Related:
`2026-11-17-global-maps-design.md` §6, ADR-0021, `openfreemap.md`,
`catalog-security.md` §4.

## 1. Origin and today's headers

Both servers bind `127.0.0.1:0` and serve
`http://127.0.0.1:<ephemeral>/map/index.html`, so the document origin is
`http://127.0.0.1:<port>` and changes every process start.
`src/mobile/map_server.rs::body_response` and `src/map_server.rs::asset_response`
send only `Content-Type`, `Accept-Ranges`, `Cache-Control: no-cache`,
`X-Content-Type-Options` and `Content-Length`: **there is no CSP or
`Referrer-Policy` on the map page, and no cookie is ever set.** The CSP in
`src/server/web_handlers.rs` is the web app's and must stay remote-free
(`tests/assets.rs::csp_is_same_origin_and_blocks_remote_fonts`); do not reuse it.
`web/map/serve.mjs` (preview) also sends none.

## 2. Exact hosts (from the live Liberty style)

One origin, not several — `/styles/liberty` resolves every subresource to
`https://tiles.openfreemap.org`:

| Use | URL |
|---|---|
| Style | `/styles/liberty` |
| Vector TileJSON → tiles | `/planet` → `/planet/<build>/{z}/{x}/{y}.pbf` (maxzoom 14) |
| Raster relief z0–6 | `/natural_earth/ne2sr/{z}/{x}/{y}.png` |
| Sprite | `/sprites/ofm_f384/ofm.json`, `.png`, `@2x.png` |
| Glyphs | `/fonts/{fontstack}/{range}.pbf` |

Attribution links (`openfreemap.org`, `openmaptiles.org`,
`openstreetmap.org/copyright`) are navigations, not subresources: no CSP entry.

## 3. CSP change

One copy in `web/map/csp.txt`, `include_str!` in both Rust servers (next to
`X_CONTENT_TYPE_OPTIONS`) and read by `serve.mjs`, which also sends
`Referrer-Policy: no-referrer`.

```text
default-src 'self'; script-src 'self' 'unsafe-inline'; worker-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https://tiles.openfreemap.org; font-src 'self'; connect-src 'self' https://tiles.openfreemap.org; base-uri 'none'; frame-ancestors 'none'; form-action 'none'; object-src 'none'
```

- `connect-src` covers style JSON, TileJSON, vector tiles, glyphs, sprite JSON
  **and the MapLibre worker's fetches** — dedicated workers inherit the document
  CSP.
- `img-src` covers the sprite PNG, Natural Earth PNG and the `data:` favicon.
- `font-src` stays `'self'`: glyphs are protobuf fetched as binary, so they are
  `connect-src`. Add the host only if a style ships `@font-face`.
- `worker-src 'self'`, **no `blob:`**: vendored MapLibre 6.18.0 calls
  `new Worker(same-origin maplibre-gl-worker.mjs?v=…, {type:'module'})`; its
  `Blob`/`data:` path only runs when `WORKER_URL` is cross-origin or the page
  origin is `null` (`file://`). Module workers need Chromium ≥ 80, so
  `child-src` is unnecessary.
- `'unsafe-inline'` for script is required by the two inline blocks (theme
  bootstrap + module); externalising them is the only way to tighten it. Style
  needs inline because MapLibre sets style attributes.
- No `upgrade-insecure-requests` — it would rewrite the loopback `http://` loads.
- Ship the union policy always: CSP cannot be loosened after load and listing a
  host causes no traffic, so the default-off toggle stays the privacy control.
  "Only while the toggle is on" would need a document reload per toggle.

## 4. Android WebView

- **Mixed content is not involved:** `http://127.0.0.1` is a potentially
  trustworthy origin and the subresources are HTTPS. `MIXED_CONTENT_NEVER_ALLOW`
  blocks HTTP under an HTTPS page, the opposite direction. Keep it.
- **Cleartext** stays required for the loopback origin
  (`usesCleartextTraffic="true"`); HTTPS subresources ignore it. Optional:
  network-security config permitting cleartext only for `127.0.0.1`.
- **CORS** does apply (loopback → OFM). Style, TileJSON, tile, sprite and glyph
  all return `Access-Control-Allow-Origin: *`; simple GETs, no preflight.
- `setAllowUniversalAccessFromFileURLs(false)` is already set and irrelevant to
  an HTTP origin — leave it off.
- `WebViewAssetLoader` is **not needed**: it exists to give `file://` pages an
  HTTPS origin and would need a Java `shouldInterceptRequest` serving PMTiles
  byte ranges. Loopback already provides modules, workers and Range. Only loss:
  a stable origin for the theme `localStorage`.
- `shouldOverrideUrlLoading` sees frame navigations only, never subresource
  fetches, so **CSP is the only egress filter for tiles, sprites and glyphs**.
- Change `setCacheMode(LOAD_NO_CACHE)` (`PinpointActivity.java:279`) to
  `LOAD_DEFAULT`: it applies to subresources and defeats OFM's `max-age=86400`,
  so every open refetches style, sprites and glyphs and nothing survives a
  mid-session drop. Loopback already sends `Cache-Control: no-cache`.

## 5. Cookies and auth

No leak. The page has no session; neither server sends `Set-Cookie` or account
data. `android/` never touches `CookieManager`, so the WebView jar is separate
from reqwest's session jar (`src/api.rs`), and cookies are host-scoped anyway.
`web/map/map.js` has no `transformRequest` and no `Authorization`/`X-CSRF-Token`;
MapLibre passes no credentials mode, so `fetch` defaults to `same-origin` and a
cross-origin tile request carries no cookies (`ACAO: *` would reject them).
`no-referrer` keeps even the loopback origin off the tile host.

## 6. Offline failure behavior

- Toggle off by default, persisted; disabled when `navigator.onLine === false`.
- Fetch the style first with `AbortController` + 10 s timer (not
  `AbortSignal.timeout`, which needs Chromium 103), then `setStyle(parsedJson)`.
  On error/timeout keep the local pack style rendered, show "Online basemap
  unavailable", leave the toggle on for an explicit retry. No automatic retry
  and no OFM polling; MapLibre does not retry failed tiles, so the only loop risk
  is the page's error UI — debounce `map.on('error')` to one message, and after
  3 network errors in 10 s grey the toggle for the session.
- An installed pack wins at boot; toggling off restores the local style and
  re-adds pins and the route via `style.load` (per `openfreemap.md`).

## 7. Test

**Rust** (both servers' tests): `/map/index.html` carries the `csp.txt` value,
with exactly one `https://` origin and no `*` or `blob:`; no `set-cookie`.
**Playwright** (`web/map/test/online-csp.mjs`, served by `serve.mjs` with the same
header):
1. toggle off → zero requests leave the loopback origin (existing smoke check);
2. route `https://tiles.openfreemap.org/**` to local fixtures (raster style +
   1×1 PNG, TileJSON, tile, sprite, glyph) → toggle on → map renders, every
   request is that one host, no CSP console error;
3. route abort → toggle on → local style still rendered within 10 s, ≤ 2 style
   requests in 5 s (no retry loop);
4. `fetch('https://example.com')` is blocked with a CSP violation — proves the
   allowlist, not just the default.

**Manual WebView** (`adb logcat -s PinpointMap`; `onConsoleMessage` is wired):
off → no `tiles.openfreemap.org` line; on → only style, `/planet`, tiles, sprite
and glyphs; airplane mode after load → no repeated errors, local pack renders;
`chrome://inspect` shows one `Refused to connect` for a non-allowlisted fetch
and none on the normal path.
