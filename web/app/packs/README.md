# Offline pack manifests (same-origin)

The Pinpoint server publishes the offline map pack manifest from this folder so
the web app and the Android/desktop WebView can download packs from the **same
origin** as the UI. GitHub release assets do not send CORS headers, so a page
cannot `fetch` them directly (and a WebView page on a loopback origin cannot
either). The server proxies them instead of loosening CSP.

## Files

| File | What it is |
|---|---|
| `moldova.json` | Committed manifest for the Moldova pack, byte-identical to the data release `manifest.json` except every `files[].url` is rewritten to the same-origin proxy path. |

The upstream release is
`https://github.com/Prince-of-Nothing/Pinpoint/releases/download/data-moldova-20261116`
and holds `manifest.json`, `moldova.pmtiles`, `moldova-search.sqlite` and
`moldova-valhalla.tar`.

`src/server/offline_handlers.rs` serves `moldova.json` verbatim and streams the
three artifacts. Keep the `bytes` and `sha256` values in sync with the release
and with `docs/offline/moldova.md`: the download manager verifies them, so a
stale hash turns into a failed install, not silent corruption.

## Proxy path contract

| Method | Path | Response |
|---|---|---|
| `GET` | `/offline/packs/moldova/manifest.json` | `200 application/json` — the committed manifest above |
| `GET`/`HEAD` | `/offline/packs/moldova/moldova.pmtiles` | upstream bytes, `Content-Type: application/vnd.pmtiles` |
| `GET`/`HEAD` | `/offline/packs/moldova/moldova-search.sqlite` | upstream bytes, `Content-Type: application/vnd.sqlite3` |
| `GET`/`HEAD` | `/offline/packs/moldova/moldova-valhalla.tar` | upstream bytes, `Content-Type: application/x-tar` |
| `OPTIONS` | any of the three assets | `204` CORS preflight |

Contract details:

- `Range` is forwarded to the release host, so `206`, `Content-Range`,
  `Content-Length` and `Accept-Ranges: bytes` come back exactly as upstream
  sends them. PMTiles readers and resumable downloads depend on this.
- Bodies are streamed, never buffered: `moldova.pmtiles` plus
  `moldova-valhalla.tar` is roughly 213 MB.
- Every response carries `Access-Control-Allow-Origin: *` and
  `Access-Control-Expose-Headers: Content-Range, Content-Length, Accept-Ranges`
  so a local WebView page can read the pack with `fetch`.
- Only the pack id `moldova` and those three file names resolve. Everything
  else is `404` **before** any outbound request — the proxy never takes a URL
  from the client, so it cannot be used as an SSRF hop.
- `PINPOINT_PACK_BASE_URL` overrides the upstream base (used by tests against
  a local fixture server); when unset the GitHub release URL above is used.
- Responses are cacheable for five minutes (`Cache-Control: public,
  max-age=300`). The path is stable across pack versions, so the download
  manager must always re-read the manifest and verify SHA-256 rather than
  trusting a cached asset.

## Licensing and attribution

Pack data is © OpenStreetMap contributors, licensed under the
[Open Database License (ODbL) 1.0](https://opendatacommons.org/licenses/odbl/1-0/).
The manifest carries `attribution`, `license` and `license_url`; the published
pack ships `ATTRIBUTION.md`; the map UI shows the ODbL credit. Derived
databases (the PMTiles basemap, the search index and the Valhalla graph) stay
under ODbL. Do not publish a pack without the attribution and license fields.

This proxy only relays byte-for-byte artifacts that the Pinpoint release
already publishes. It adds no caching layer, no rewrite of the payload and no
data collection: the release host sees the server's IP, not the user's.
