# OpenFreeMap as Pinpoint's opt-in online basemap

Date: 2026-11-17 · Scope: desktop/web (MapLibre GL JS) and Android WebView · Status:
**usable in production, no SLA.** Live endpoints checked today; no code changes.

## Verdict

Free, no API key, registration or cookies; commercial use allowed. The public instance
has been MapHub's production basemap since June 2024, so it is not a toy
([homepage](https://openfreemap.org/), [GitHub](https://github.com/hyperknot/openfreemap)).
No blocker; the risk is operational: one maintainer, donation-funded, no SLA, may be
discontinued at any time.

## Endpoints

One host serves everything: `tiles.openfreemap.org`.

| Item | Exact value |
|---|---|
| Liberty / Bright / Positron | `https://tiles.openfreemap.org/styles/liberty` · `/bright` · `/positron` |
| Dark / Fiord | `https://tiles.openfreemap.org/styles/dark` · `/fiord` |
| Vector source (TileJSON) | `https://tiles.openfreemap.org/planet` → `https://tiles.openfreemap.org/planet/{version}/{z}/{x}/{y}.pbf`, maxzoom 14; `/planet/latest/...` aliases the current version |
| Sprite | `https://tiles.openfreemap.org/sprites/ofm_f384/ofm` (+ `.json`, `.png`, `@2x.png`) |
| Glyphs | `https://tiles.openfreemap.org/fonts/{fontstack}/{range}.pbf` |

API key: **none** — do not add one. Liberty/Bright/Positron share the same sprite and
glyph URL. Styles are MIT-licensed forks
([openfreemap-styles](https://github.com/hyperknot/openfreemap-styles)); data is OSM (ODbL).

## Terms and attribution

Per the [ToS](https://openfreemap.org/tos/) (updated 2026-10-14) and homepage:

- No limit on map views or requests. Commercial use is allowed.
- The **integrator** must be 18+; end users of the map may be any age.
- No automated data collection except offline downloads.
- Offline downloads: **≤ 3,000 tiles or 50 MB per download**, **≤ 4 parallel requests**, **unique User-Agent identifying the app**. Larger downloads require permission (`info@openfreemap.org`).
- “As-is”, no warranty; may be discontinued at any time without notice; Cloudflare may process requests; Hungarian law.

Attribution is required; MapLibre adds it automatically from the TileJSON `attribution`
field to its expanded `AttributionControl` — keep that control visible. Required text:

> OpenFreeMap © OpenMapTiles Data from OpenStreetMap

For print, video or non-MapLibre clients, use that line with links to openfreemap.org,
openmaptiles.org and openstreetmap.org/copyright; OpenFreeMap is optional per the docs,
OpenMapTiles and OSM are not.

## CORS and CSP

Checked live 2026-11-17: `Access-Control-Allow-Origin: *` on style JSON, `/planet`
TileJSON, `.pbf` tiles, sprite JSON/PNG and glyph `.pbf`, with
`Cache-Control: max-age=86400, public`. Requests are simple GETs (no preflight, no
credentials). Android WebView follows the same CORS rules: the wildcard works from
`https://appassets.androidplatform.net` (the recommended `WebViewAssetLoader` origin);
enable JavaScript and WebGL, and inject no key.

```text
connect-src https://tiles.openfreemap.org;
img-src https://tiles.openfreemap.org data: blob:;
worker-src 'self' blob:;
child-src 'self' blob:;   # older WebViews
```

`connect-src` covers style, TileJSON, tiles, glyphs and sprite JSON; `img-src` covers
sprite PNGs and the z0–6 Natural Earth relief (`/natural_earth/ne2sr/...`). A
CDN-loaded MapLibre 6 builds its worker from a Blob URL, hence `blob:`; a self-hosted
worker needs only `'self'`
([MapLibre CSP](https://maplibre.org/maplibre-gl-js/docs/guides/v5-to-v6-migration-guide/)).

## Privacy and disclosure

The tile host sees IP, User-Agent, Referer and each requested `z/x/y` tile — tile
coordinates reveal the approximate viewed area. No accounts, cookies or tracking. Per
the [Privacy Policy](https://openfreemap.org/privacy/) (updated 2026-10-08): access logs
omit IPs; error logs may contain IP/URL and are deleted after **7 days**;
security-incident IP logging may run **30 days**; Cloudflare may process requests.
Controller: Hyperknot Software Kft., Hungary (GDPR).

> Pinpoint's online basemap is optional. When enabled, map styles and tiles load from
> OpenFreeMap (`tiles.openfreemap.org`), operated by Hyperknot Software Kft. in Hungary.
> OpenFreeMap receives your IP address, browser type and the map tiles requested; tile
> coordinates reveal the approximate area you are viewing. It uses no accounts, cookies
> or API keys, does not store IPs in access logs, and deletes error logs after 7 days
> (security-incident IP logs are capped at 30 days). Cloudflare may process requests as
> a CDN. Pinpoint sends only the map view needed to draw tiles, not account data or the
> friend list.

## Availability and fallbacks

No SLA or personal support; donations/GitHub Sponsors, possible future Pro plan. Two
round-robin servers, weekly planet updates, no cloud. Treat as best-effort; keep the
opt-in switch and an alternative path.

| Fallback | Cost / terms | Tradeoff |
|---|---|---|
| [Protomaps](https://protomaps.com/api) self-hosted PMTiles | storage + egress; BSD/ODbL | One static file, no third party at runtime; large planet, range-request hosting, ops |
| Protomaps hosted API | free non-commercial; commercial via GitHub Sponsors; key | CDN-cached, per-key CORS; key management, less-than-weekly updates |
| [MapTiler](https://www.maptiler.com/cloud/pricing/) | Free: 5k sessions + 100k requests/month, **non-commercial**, logo; Flex $30/month (25k sessions, 500k requests) | Key + account; 99.9% SLA only on Custom |
| Self-host OpenFreeMap | server + disk; MIT | Same stack/style, no external dependency; Ubuntu 24.04 + Btrfs + weekly updates |

## Runtime style switch drops custom GeoJSON

No. `map.setStyle(url, {diff: true})` compares the serialized current style with the new
one and removes layers/sources absent from it, so runtime `addSource`/`addLayer` GeoJSON
overlays disappear. Re-add them in `style.load`, or merge them via `transformStyle`
([Map#setStyle](https://maplibre.org/maplibre-gl-js/docs/API/classes/Map/#setstyle)).
`diff: true` still avoids a full repaint because the three styles share sprites and
glyphs, but it does not preserve custom layers.

## Blocker

None for commercial production. Watch: no SLA, withdrawal without notice, 18+ acceptance
term, offline caps require a unique User-Agent, and very old Android WebViews without
WebGL cannot render MapLibre.
