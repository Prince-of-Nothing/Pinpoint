# Always Together — Web Client

Browser client for the Always Together API (Go stack; the Python stack has a
separate Leaflet-based client under `backend/web/`). Privacy-first: **zero
third-party JavaScript, zero trackers**, modular ES modules with **no build
step**, offline app shell, OSM-licensed tiles from a policy-compliant source
(see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)).

## Architecture

Plain ES2022 modules loaded natively by the browser — no npm, no bundler, no
supply chain. The dependency tree is: nothing.

```
public/
├── index.html            # static shell (all views, dialogs)
├── app.css               # dark theme, all styling
├── manifest.webmanifest  # PWA install metadata (no remote icons)
├── sw.js                 # service worker: offline shell (never caches data)
└── js/
    ├── config.js         # tile source + policy notes, freshness thresholds
    ├── api.js            # ONLY module talking to /api/v1 (CSRF, errors)
    ├── session.js        # login/logout/session-restore (cookie bootstrap)
    ├── ui.js             # DOM helpers, toast, offline banner
    ├── map.js            # tile rendering + pins + ODbL attribution
    └── main.js           # tab wiring, password dialog, boot
```

Module boundaries (by convention, enforced in review):

- `api.js` owns all `fetch` — features never build URLs or headers.
- `map.js` owns all tile/attribution logic — the compliance-sensitive part.
- Everything that renders server data goes through `esc()` (no HTML sinks
  with untrusted data; CI/Python tests assert no `innerHTML` in the
  Leaflet client, and the same discipline applies here).

## Running

Served by nginx in the full stack:

```bash
docker compose up --build   # web on http://localhost:8080
```

Local dev without Docker: serve `public/` with any static server and proxy
`/api/` to a running Go backend (`cd backend && go run ./cmd/server`).

## Features

- **Login + session restore**: HttpOnly session cookie survives reloads;
  `POST /auth/session` re-validates it on boot and rotates CSRF.
- **Map**: friend pins over raster tiles, freshness coloring (ok/warn/stale),
  precision badges, navigate via `geo:` URI or openstreetmap.org.
- **Friends**: invite by email, accept/reject requests.
- **Sharing**: create/pause/resume per-friend shares with precision levels.
- **Admin**: user table + create-user (administrator role only).
- **Password change** incl. forced-change flow for temporary passwords.
- **Offline shell**: service worker caches the app UI only — live locations
  require connectivity by design; API responses are *never* cached (location
  data must not sit in Cache Storage on shared devices).

## Map tiles & licensing

The tile source is one line in `js/config.js` (`tiles.url`). The default is
the Wikimedia Foundation endpoint (app-friendly, keyless, OSM data). Never
point it at `tile.openstreetmap.org`: OSMF's Tile Usage Policy prohibits
serving apps from their infrastructure. Production deployments should
self-host tiles (Martin / TileServer GL with Geofabrik extracts). The ODbL
attribution line under the map is a legal requirement — keep it. Full
obligations: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Security headers

`nginx.conf` sets a CSP that structurally enforces the privacy stance:
`default-src 'self'` with the tile host as the only external allow-listed
origin (img-src). If you change `tiles.url`, update the CSP `img-src` to
match — the browser will silently blank the map otherwise.

## Roadmap (space for improvement)

- WebSocket live updates (board T07) replacing periodic refresh.
- Map pan/zoom (current view is a static centroid render — zero-JS-weight
  trade-off; a self-hosted MapLibre GL build could replace `map.js` later).
- Web app PWA icons (currently data-URI emoji).
- Shared API-client package with `weba`/`webk` once those land (T03/T10).
