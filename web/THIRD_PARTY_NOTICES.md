# Third-Party Notices & Licensing — Always Together Web Client

Companion to [mobile/THIRD_PARTY_NOTICES.md](../mobile/THIRD_PARTY_NOTICES.md)
(same OSM obligations; SDK tables differ per platform).

## Privacy stance (0 trackers)

The client ships **zero third-party JavaScript** — no npm dependencies, no
bundler, no CDN scripts, no fonts, no analytics. The browser only ever
contacts:

1. **Our own origin** (API via same-origin `/api/` proxy; session cookies
   are first-party, HttpOnly, SameSite=Lax).
2. **The configured map tile host** (default: Wikimedia Foundation —
   see below). Tiles are `<img>` requests; no identifiers are attached.

The nginx CSP (`default-src 'self'`, tile host as the sole img-src
exception) makes this structural: a future patch that tried to load a
tracker would be blocked by the browser, not just by review.

## Map data: OpenStreetMap (ODbL)

Identical obligations to the mobile app:

- **Data**: © OpenStreetMap contributors, [ODbL v1.0](https://opendatacommons.org/licenses/odbl/1-0/).
- **Attribution**: rendered permanently under the map (`#map-attribution`,
  populated from `config.js` `tiles.attribution`). Removing it violates the
  license; the string must stay visible on every map view.
- **Share-alike**: applies if we redistribute the database (e.g. bundle
  tiles). End-user downloads from a provider are governed by that
  provider's terms.

## Tile source: Wikimedia Foundation (default)

- **Endpoint**: `https://maps.wikimedia.org/osm-intl/{z}/{x}/{y}.png` —
  keyless, OSM-derived raster tiles.
- **Why**: unlike `tile.openstreetmap.org` (OSMF [Tile Usage
  Policy](https://osmfoundation.org/wiki/Tile_Usage_Policy) prohibits
  serving apps from their infrastructure), Wikimedia's maps service
  explicitly welcomes third-party application usage at reasonable rates.
  The default must not create a ToS violation for anyone running the demo.
- **Rate limits / ops**: Wikimedia asks for sane traffic and a descriptive
  User-Agent at scale; production deployments should self-host tiles from
  Geofabrik extracts (Martin / TileServer GL) — one line in
  `js/config.js` + the CSP `img-src` allow-list.
- **Python-served client**: `backend/web/app.js` (Leaflet) uses the same
  endpoint, same attribution; its CSP is in
  `backend/app/core/middleware.py`.

## JS libraries and assets

| Component | Version | License | Notes |
|---|---|---|---|
| (none) | — | — | The Go-stack web client has **no JS dependencies**; all code is original, covered by the repo license. |
| Leaflet (Python-served client only, `backend/web/vendor/leaflet/`) | 1.9.4 | **BSD-2-Clause** | Vendored with hash-pinned provenance in `backend/web/vendor/README.md`; attribution control kept enabled (ODbL). |
| PWA manifest / icons | — | — | Data-URI emoji icon, no binary assets. |

## Offline scope (honest limits)

The service worker caches the **app shell only** (HTML/CSS/JS). Map tiles
are *not* cached offline — bulk tile caching would violate provider terms
of service and store location data on disk. Live locations always require
connectivity; the UI shows an explicit offline banner instead of stale
positions. (Offline map packs are a mobile-only feature — see the Android
app.)

## Data protection posture

- Session: HttpOnly cookie + double-submit CSRF (token readable from the
  non-HttpOnly companion cookie by design, rotated via login/session
  bootstrap).
- No location data ever written to Cache Storage, localStorage, or
  IndexedDB — state is in-memory only and dies with the tab.
- CSP + `Referrer-Policy: no-referrer` + `X-Frame-Options: DENY` +
  `Permissions-Policy` (geolocation self-only; camera/mic/payment blocked)
  set in `web/nginx.conf` (and the Python middleware for the alternate
  stack).
