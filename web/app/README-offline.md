# Offline packs on the web

This is the browser half of the M2 offline-map stream from
`docs/superpowers/specs/2026-11-16-offline-maps-design.md` §11: manifest
download, SHA-256 verification, OPFS storage, a service worker with HTTP Range,
and delete/update management. It is additive; the shared app files
(`web/app/index.html`, `app.js`, `app.css`, `map.js`, `nav.js`,
`web/map/map.js` and `web/vendor/*`) are unchanged.

The standalone demo is `web/app/offline-demo.html`. The reusable module is
`web/app/offline.js`, the default catalog is `web/app/offline-catalog.js`, and
the worker is `web/sw.js`.

## Try the demo

From the repository root:

```powershell
node web/map/serve.mjs
# open http://127.0.0.1:8080/app/offline-demo.html
```

The demo needs an HTTP origin that supports Range for the source fixture.
`127.0.0.1` is a secure context, so OPFS and service workers are available.
Opening the file with `file://` will not work.

The default catalog has two entries:

- **Monaco**, a 94 KB fixture downloaded automatically by the smoke test.
- **Moldova**, the ~220 MB production pack. It is an **optional manual
  download** in the demo and is served through the same-origin proxy paths
  below. Nothing starts on page load; the user must select Moldova and press
  its download button.

The Monaco fixture is a 94 KB OpenMapTiles-schema PMTiles archive plus its
`LICENSE.txt`. It is committed only for tests and demos; the Moldova production
pack is built by `scripts/offline/` and is not committed.

## Default catalog and same-origin proxy

Browser code must never fetch the GitHub release directly. GitHub release
assets do not send CORS headers, so a browser `fetch()` cannot read them from
the app or a WebView page. The demo and the later app adapter use the
Pinpoint server's proxy instead:

```text
/offline/packs/moldova/manifest.json
/offline/packs/moldova/moldova.pmtiles
/offline/packs/moldova/moldova-search.sqlite
/offline/packs/moldova/moldova-valhalla.tar
```

These are declared in `web/app/offline-catalog.js` (`MOLDOVA_PROXY_ROOT`,
`MOLDOVA_PROXY_FILES`, `DEFAULT_PACK_CATALOG`). The proxy is being added in a
separate server change. It must also rewrite the manifest's file `url` values to
these same-origin paths; `downloadPack()` resolves file URLs relative to the
manifest, and a manifest that still points at `github.com` will fail at the
browser CORS boundary. If a pack is ever hosted on a different origin, that host
must return explicit CORS headers and be added to the page CSP `connect-src`.

Local installed-pack URLs are different from the proxy URLs:

```text
/offline/packs/moldova/...          # upstream same-origin proxy (HTTP)
/offline-packs/moldova/...          # local OPFS served by the service worker
```

The demo's catalog lets a person view the Moldova manifest and explicitly start
the manual ~220 MB download when the proxy is available. The automated smoke
test only downloads Monaco, so CI never transfers the production pack.

## Files

| Path | Purpose |
|---|---|
| `web/app/offline.js` | Manifest verification, OPFS storage, download/update/delete/open API |
| `web/app/offline-catalog.js` | Default Monaco fixture + same-origin Moldova proxy catalog |
| `web/sw.js` | App-shell cache and `/offline-packs/<id>/<file>` Range server |
| `web/app/offline-demo.html` | Standalone manager + MapLibre map demo |
| `web/app/offline.css` | Demo-only styling |
| `web/app/fixtures/offline-pack/manifest.json` | Monaco fixture manifest (real bytes and SHA-256) |
| `web/app/fixtures/offline-pack/manifest-broken.json` | Deliberately corrupt fixture used by the smoke test |
| `web/app/fixtures/offline-pack/LICENSE.txt` | ODbL notice shipped inside the pack |
| `web/app/test/offline-smoke.mjs` | Static + headless Chromium smoke test |
| `web/map/fixtures/monaco.pmtiles` | Existing 94 KB ODbL PMTiles fixture, reused as pack content |

## API

```js
import {
  listPacks,
  downloadPack,
  deletePack,
  openPack,
  verifyManifest,
  OfflinePackError,
} from "./offline.js";
```

All functions are browser-only. OPFS, WebCrypto and service workers require a
secure context (`https:` or a loopback origin such as `127.0.0.1`).

### `verifyManifest(manifest)`

Pure, synchronous validation of the M1 manifest. Returns a normalised copy with
a computed `totalBytes`; throws `OfflinePackError` with code `MANIFEST_INVALID`
or `INVALID_URL`.

It rejects:

- ids with path separators, a leading dot, or anything outside
  `[A-Za-z0-9][A-Za-z0-9._-]{0,63}`;
- file names outside `[A-Za-z0-9][A-Za-z0-9._-]{0,127}`, including `..`,
  slashes and backslashes;
- duplicate names, zero/negative byte counts, bad SHA-256 lengths, empty file
  lists, and more than 64 files;
- missing attribution, license or a non-HTTP(S) `license_url`.

### `downloadPack(manifestUrl, { onProgress, signal, minFreeBytes })`

1. Fetches the manifest with `credentials: "same-origin"` and `cache: "no-store"`.
2. Verifies the manifest and checks the estimated free space.
3. Downloads each file, streaming chunks into a progress counter.
4. Checks the exact byte count and SHA-256 with `crypto.subtle.digest`.
5. Writes verified bytes into `.staging-<id>-<nonce>/files/`.
6. Writes `pack.json` metadata into staging, then moves the staged files into
   `<id>/`. `pack.json` is transferred last.
7. Returns the same shape as `openPack(id)`, plus `updated: true` when an old
   version of the same id was replaced.

`onProgress` receives:

```js
{
  id: "monaco",
  version: "2026.11.16",
  phase: "manifest" | "download" | "verify" | "commit" | "done",
  file: "monaco.pmtiles" | null,
  fileIndex: 0,
  filesTotal: 2,
  receivedBytes: 47123,
  totalBytes: 94794,
  percent: 49.7,
  bytesPerSecond: 123456
}
```

Calls are throttled to roughly 10 per second; phase changes and completion are
always emitted. Exceptions thrown by the callback are ignored so UI bugs cannot
corrupt an operation.

`signal` is an `AbortSignal`. Cancellation removes the staging directory and
throws `OfflinePackError` with code `CANCELLED`.

`minFreeBytes` refuses to start when `navigator.storage.estimate()` reports
less free space than `totalBytes + minFreeBytes`. The demo passes `0` so the
94 KB fixture installs on small CI disks. The production integration should
pass `500 * 1024 * 1024` per the design's storage guard.

### `listPacks()`

Reads `offline-packs/*/pack.json`, ignores staging/backup directories and
incomplete packs, verifies the stored metadata, and returns stable-ordered
records:

```js
{
  id, name, version, installedAt, source,
  totalBytes, attribution, license, license_url,
  files: [{ name, bytes, sha256, url }],
  pmtiles: "monaco.pmtiles",
  pmtilesUrl: "/offline-packs/monaco/monaco.pmtiles",
  built_at, bbox, minzoom, maxzoom
}
```

### `openPack(id)`

Returns the same record as `listPacks()` for one id, or `null` when the pack is
not installed. The record also has `file(name)`, which returns the stored
`File` for advanced callers. `pmtilesUrl` is the URL to pass to
`createMap({ pmtilesUrl })`.

### `deletePack(id)`

Removes `<id>/` and any abandoned `.staging-<id>-*`, `.old-<id>-*` or
`.trash-<id>-*` entries. Returns `true` when an installed directory existed.

### Error codes

| Code | Meaning |
|---|---|
| `MANIFEST_INVALID` | Shape, id, file name, byte count or hash field is invalid |
| `INVALID_URL` | Manifest/file URL is malformed, non-HTTP(S) or contains credentials |
| `NETWORK` | Manifest/file request failed or returned a non-2xx status |
| `SIZE_MISMATCH` | Downloaded bytes differ from the manifest |
| `HASH_MISMATCH` | SHA-256 differs from the manifest |
| `QUOTA` | Estimated or actual browser storage is insufficient |
| `UNSUPPORTED` | OPFS, WebCrypto or another required API is unavailable |
| `CANCELLED` | The caller aborted the download |
| `BUSY` | The same manifest or pack id is already downloading |
| `STORAGE` | Any other OPFS read/write failure |
| `NOT_FOUND` | Reserved for callers that require a specific pack |

A failed download or verification never replaces an installed pack and never
writes `pack.json`, so `listPacks()` cannot report a partial pack as installed.

## Manifest contract

```json
{
  "id": "monaco",
  "version": "2026.11.16",
  "name": "Monaco",
  "built_at": "2026-11-16T21:11:47Z",
  "bbox": [7.4, 43.72, 7.44, 43.75],
  "minzoom": 0,
  "maxzoom": 12,
  "files": [
    {
      "name": "monaco.pmtiles",
      "url": "../../../map/fixtures/monaco.pmtiles",
      "bytes": 94389,
      "sha256": "a600dbfd9ddb25b82661f01d228a39d2b386dd13e401d2081b3ce7fd9c6de82f"
    },
    {
      "name": "LICENSE.txt",
      "url": "LICENSE.txt",
      "bytes": 405,
      "sha256": "bf40327577f4ae509a121e90610b0d6dcad71d64a5e9a4ceac9d6d190933becf"
    }
  ],
  "attribution": "© OpenStreetMap contributors",
  "license": "ODbL-1.0",
  "license_url": "https://opendatacommons.org/licenses/odbl/1-0/"
}
```

- `id` is the storage directory and URL segment. It must be a simple name.
- `version` is free-form display metadata, at most 96 printable characters.
- File `url` values are resolved relative to the manifest URL, so a manifest
  can describe sibling files or an absolute pack host. Absolute URLs may be
  cross-origin; that host must send CORS headers because the page fetches the
  files. The manifest itself is normally same-origin from the Pinpoint server.
- `name` is the filename inside the pack. Only simple names are accepted; there
  is no directory structure inside a pack.
- `bytes` and `sha256` are mandatory and exact. The hash is lowercase hex.
- `attribution`, `license` and `license_url` are mandatory and are stored with
  the pack. ODbL attribution must stay visible wherever the pack is shown.
- Extra fields (`name`, `built_at`, `bbox`, `minzoom`, `maxzoom`, vector/index
  metadata) are preserved in OPFS metadata and returned by `listPacks()`.

The manifest is data, never code. The demo writes all values with
`textContent`; it does not inject manifest HTML.

## OPFS layout and atomicity

```text
offline-packs/
  monaco/
    pack.json                 # installed metadata, written last
    files/
      monaco.pmtiles
      LICENSE.txt
  .staging-monaco-<nonce>/    # only while downloading; removed on success/failure
```

`pack.json` contains schema `1`, the manifest fields, `installedAt`, `source`
(the manifest URL) and the verified file list. An update stages the new pack
completely, verifies it, removes the old `<id>/`, then transfers the staged
files. `pack.json` is the last file transferred, so an interruption can leave
an unlisted orphan directory but never an installed-looking partial pack.
`listPacks()` ignores dot-prefixed and metadata-less directories;
`deletePack(id)` removes both the current pack and its staging leftovers.

If a browser does not implement `FileSystemFileHandle.move()`, the module falls
back to reading and writing each file. Chromium (including the headless test
browser) has file-level `move()` but no directory-level `move()`, so the
implementation deliberately moves files rather than renaming directories.

## Service worker

`web/sw.js` has two jobs.

### App shell cache

`install` precaches the demo page, `offline.js`, the manifest fixtures, the
shared map module/style, the Monaco fixture and the vendored MapLibre/PMTiles
assets. `activate` removes older `pinpoint-offline-*` caches and claims existing
clients. Non-pack same-origin requests are cache-first with a network fallback;
successful first-time GETs are added to a runtime cache. Requests made with
`cache: "no-store"` (the manager's manifest refresh) go network-first and then
refresh the cached copy, falling back to the cache when offline. Cross-origin
requests are not intercepted. There is no CDN, remote font, sprite or tile host
in the precache list.

Bump `VERSION` in `web/sw.js` when the precache list or shell assets change.

### Pack Range server

Requests under `/offline-packs/<id>/<file>` are read from
`offline-packs/<id>/files/<file>` and never touch the network. The worker
supports:

- `GET` and `HEAD`;
- full responses (`200`) with `Accept-Ranges: bytes`;
- single byte ranges (`206`) with `Content-Range: bytes start-end/size` and
  `Content-Length`;
- suffix ranges (`bytes=-N`) and open-ended ranges (`bytes=N-`);
- `416` with `Content-Range: bytes */size` for malformed or unsatisfiable
  ranges.

`Content-Type` is selected from the extension: `.pmtiles` is
`application/vnd.pmtiles`, SQLite is `application/vnd.sqlite3`, JSON is
`application/json`, and unknown files are `application/octet-stream`. Pack
responses are `Cache-Control: no-store` so a delete/update is visible
immediately.

PMTiles requires Range. This worker is why MapLibre can read
`pmtiles:///offline-packs/monaco/monaco.pmtiles` even though the bytes are in
OPFS rather than behind an HTTP server.

The demo registers `../sw.js`, whose default scope is `/`. The pack URL prefix
is root-absolute (`/offline-packs/`). If the app is later served under a
sub-path, either keep the service worker at the site root or change
`OFFLINE_PACK_URL_PREFIX` and the worker's `PACK_ROUTES` together.

### Demo diagnostics

The demo exposes `window.__offlineDemo.swStats()` which posts
`{ type: "PINPOINT_SW_STATS" }` to the controller. The worker replies with
counters (`packRequests`, `pack206`, `pack200`, `pack404`, `pack416`,
`shellCacheHits`, `networkFetches`, `offlineFallbacks`) for tests and debugging.
`PINPOINT_SKIP_WAITING` is also handled for a future update prompt.

## Integration hook for the later web app

The module was designed so the app integration can be a small adapter, without
editing this feature's demo:

```js
// Later, in the web-app adapter:
import { createMap } from "/map/map.js";
import { listPacks, downloadPack, deletePack, openPack } from "/app/offline.js";

await navigator.serviceWorker.register("/sw.js"); // once, at app start

const packs = await listPacks();                   // render the manager list
await downloadPack("/offline/packs/moldova/manifest.json", {
  minFreeBytes: 500 * 1024 * 1024,
  onProgress: updateProgress,
});

const pack = await openPack("moldova");
const handle = createMap({
  container: "#map",
  styleUrl: "/map/style.json",
  pmtilesUrl: pack.pmtilesUrl,                     // /offline-packs/moldova/moldova.pmtiles
  center: [28.8638, 47.0105],
  zoom: 8,
});
```

Integration rules:

- Use the catalog's same-origin proxy paths
  (`/offline/packs/moldova/manifest.json` and the files it lists). Never put a
  `github.com` release URL in page code; GitHub release assets do not send CORS
  headers, so the browser cannot read them. The proxy must rewrite manifest file
  URLs to same-origin paths.
- Register the worker once and wait for `navigator.serviceWorker.controller`
  before opening a pack, or the `/offline-packs/` URL will not be intercepted.
- Keep the ODbL attribution visible on every map and in the pack list.
- Pass the production storage reserve (`500 MB`) and warn on metered networks.
- If a future pack host is cross-origin, add it to the page CSP `connect-src`
  and ensure it returns CORS headers. Do not add it to `script-src`.
- The service worker precaches MapLibre from `/vendor/`; a strict CSP should
  allow `script-src 'self'`, `worker-src 'self'` and `connect-src 'self'` plus
  the pack host.
- Do not replace `web/app/map.js` or `app.js` in this stream; the adapter can
  call `openPack()` and pass `pmtilesUrl` to the shared map module when the app
  map integration lands.

## Test

Static checks only:

```powershell
node web/app/test/offline-smoke.mjs --static-only
```

Full headless Chromium test (Puppeteer from the environment's shared
`node_modules`):

```powershell
$env:NODE_PATH="C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\node_modules"
$env:PINPOINT_OFFLINE_SCREENSHOT="C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\web-offline-packs.png"
node web/app/test/offline-smoke.mjs
```

The test starts the Range-capable server from `web/map/serve.mjs`, checks the
fixture byte counts and SHA-256, opens `offline-demo.html` in headless
Chromium, and asserts:

1. the default catalog exposes the same-origin Moldova proxy paths and contains
   no GitHub release URL in executable code;
2. the service worker registers, activates and controls the page;
3. the demo shows the manifest size, version, file count and ODbL attribution;
4. `verifyManifest` rejects traversal, bad ids and bad hashes;
5. a corrupt manifest fails with `HASH_MISMATCH`, installs nothing and leaves
   no staging directory;
6. the real fixture downloads with download/verify/done progress and writes
   `pack.json`, `monaco.pmtiles` and `LICENSE.txt` to OPFS with the expected
   hashes and sizes;
7. MapLibre renders a non-zero full-size canvas from
   `/offline-packs/monaco/monaco.pmtiles`;
8. a direct Range request through the worker returns `206`, the correct
   `Content-Range`/`Content-Length`, `application/vnd.pmtiles` and the PMTiles
   magic bytes;
9. the worker reports at least one `206` pack response;
10. update replaces the installed pack, reopens the map and leaves no staging or
    backup directory;
11. delete removes the map, `pack.json`, the OPFS directory and the list row;
12. there are zero external requests, failed requests, page errors and console
    errors.

If Puppeteer is not installed, the script prints a clear `SKIP browser` line
and runs the static checks only. The full test writes a screenshot to
`$env:PINPOINT_OFFLINE_SCREENSHOT` (default: the system temp directory).

The smoke test never downloads Moldova. Its manifest is fetched only when a
person selects the optional catalog entry; the 220 MB transfer is manual and
requires the server proxy to be deployed.

## Security, privacy and ODbL

- No analytics, telemetry, external CDN or third-party tile/font host.
- Production packs are fetched only through same-origin
  `/offline/packs/...` proxy paths. GitHub release assets do not send CORS
  headers, so they are never wired into browser code; the proxy forwards the
  bytes and rewrites manifest file URLs.
- The only runtime network requests are the manifest and files the caller
  explicitly downloads. Credentials are `same-origin`, so pack-host requests
  do not send Pinpoint cookies cross-origin.
- HTTPS is expected in production; loopback HTTP is allowed for development
  because browsers treat `127.0.0.1` as a secure context.
- Every file is verified against a published byte count and SHA-256 before it
  is eligible for installation.
- File and pack names are strict simple names; traversal, absolute paths and
  backslashes are rejected before any OPFS call.
- The manifest is treated as data. The demo uses `textContent`; no HTML,
  `eval`, `Function` or dynamic script URL comes from a manifest.
- The pack's ODbL attribution and license metadata are stored in `pack.json`
  and shown in the manager and on the map. Keep
  “© OpenStreetMap contributors / ODbL” visible.

## Limitations and follow-up

- **No resume yet.** Cancellation or a network failure removes staging and a
  retry starts from the beginning. Adding HTTP Range resume is a later M2
  refinement; the service worker already supports ranges.
- **Memory use.** Files are buffered in memory to compute SHA-256 with
  WebCrypto. This is fine for the 94 KB fixture and typical vector basemaps, but
  very large packs (multi-hundred-MB search/routing bundles) should move to a
  streaming hash and chunked staging writes.
- **OPFS directory rename.** Chromium exposes file `move()` but not directory
  `move()`. The module moves files individually and writes metadata last. A
  hard crash during the final transfer can leave an unlisted orphan directory;
  `deletePack` cleans it.
- **Service worker scope.** The demo assumes the site is served at `/` and pack
  URLs under `/offline-packs/`. A sub-path deployment needs a matching worker
  scope/prefix change.
- **Precache is all-or-nothing.** If one shell asset is missing, worker install
  fails. That is deliberate so the app never claims offline readiness with an
  incomplete shell.
- **No glyphs, sprites, search or routing.** The map style remains the
  label-free foundation style. Those are M5/M6 streams.
- **Safari/Firefox.** OPFS and service-worker Range handling are checked in
  headless Chromium here. Safari's OPFS and service-worker APIs have different
  maturity and need a separate device test; `FileSystemFileHandle.move()` is
  feature-detected and falls back to copying.
- **Proxy dependency.** The default Moldova catalog entry points at
  `/offline/packs/moldova/manifest.json`. Until the parallel server proxy is
  deployed, selecting Moldova shows a clear manifest error in the demo; nothing
  falls back to GitHub. The proxy must rewrite the manifest's file URLs to the
  same-origin paths.
- **220 MB is manual and optional.** The demo never auto-downloads Moldova, and
  CI only downloads Monaco. The production app should keep the same opt-in
  behavior and show the size before starting.
- **Production storage guard.** The demo passes `minFreeBytes: 0` for the
  Monaco fixture and `500 * 1024 * 1024` for manual Moldova; the app integration
  should keep the 500 MB reserve for production packs.
