# Web offline: per-region packs

**Status:** Proposed · 2026-11-17 · Web counterpart to ADR-0021 and
`docs/superpowers/specs/2026-11-17-global-maps-design.md`. Related:
`pack-updates.md`, `global-maps-test-plan.md`.

## Current single-pack flow

`web/app/offline-demo.js` is a two-entry demo around one pack at a time:

- `web/app/offline-catalog.js` hardcodes the catalog in JS: the 94 KB Monaco
  fixture (`app/fixtures/offline-pack/manifest.json`) and a manual Moldova entry
  through the same-origin `/offline/packs/moldova/` proxy. Nothing is fetched
  from `/map/catalog`.
- `web/app/offline.js` downloads the M1 manifest, then each file, hashing with
  WebCrypto. Bytes go to `offline-packs/.staging-<id>-<nonce>/files/`, then move
  to `offline-packs/<id>/` with `pack.json` written and transferred last, so an
  interrupted install never lists as installed.
- `web/sw.js` serves `GET /offline-packs/<id>/<file>` from OPFS with single-range
  `206`/`416`, `Accept-Ranges`, `no-store`. It precaches the demo shell plus
  MapLibre/PMTiles and runtime-caches other same-origin GETs.
- Versioning is only `pack.json.version`; "Update" re-runs the download.
  `deletePack(id)` removes `<id>/` and any `.staging-*`/`.old-*` siblings.
- Quota is `navigator.storage.estimate()` minus a 500 MB reserve, checked only
  for `manual` packs. One `state.busy` flag is the whole job lock.

**Already N-region capable:** OPFS layout, SW route and id/file validation,
`listPacks`/`openPack`/`deletePack`, and the map switch (destroy `createMap`,
recreate with the pack's `pmtilesUrl`). **Single-pack:** the catalog literal, the
selected-manifest UI, and one manifest URL per id.

## What must change for N regions

1. **Catalog source.** Fetch `GET /map/catalog` (`cache: "no-store"`), fall back
   to the bundled entries when offline or on schema error. Ignore the server's
   `installed` flags: they describe the native `packs/` dir, not browser OPFS.
   Installed state is `listPacks()`.
2. **Per-region manifest.** `downloadPack` wants `bytes` equal to the response
   body and performs no decompression, while catalog schema 1 has
   `transfer_name`/`installed_name`/`compression`. Add a server route that emits
   an M1 manifest per region from the catalog, with the basemap file only and
   `identity` encoding. Do not extend the browser installer for gzip in phase 2.
3. **Layout.** Keep `offline-packs/<region>/files/`; one directory per region,
   independent delete/update. No path changes.
4. **Metadata and versioning.** Keep `pack.json`; treat
   `catalog.regions[].version !== pack.version` as "update available" and reuse
   the replace flow. Known gap: `downloadPack` deletes the old directory before
   moving the new one in, so a crash mid-commit loses the pack — acceptable only
   because packs are re-downloadable.
5. **Quota.** One job at a time across regions (the current `busy` flag plus a
   module-level queue). Reserve `500 MB + 2 × region` so staging fits during an
   update. Call `navigator.storage.persist()` on first install and show
   `usage/quota` in the picker.

## Storage limits and eviction

OPFS, Cache Storage and IndexedDB share **one** per-origin budget. Chromium
allows up to ~60% of disk per origin; Firefox is the smaller of ~10% of disk or
a 10 GiB group limit; Safari 17+ allows ~60% of disk for a browser and ~15% for
an embedded WebView. Quotas derive from total disk to resist fingerprinting, so
`quota` is optimistic.

OPFS is the right store: it streams and supports `file.slice(start, end)` for
Range. Cache Storage has no range semantics and would duplicate 139 MB;
IndexedDB copies blobs into transactions and cannot stream the archive.

iOS Safari specifics:

- Main-thread `FileSystemFileHandle.createWritable()` only shipped in **Safari
  26**. On iOS 18.x and older the current installer fails even though
  `getDirectory()` exists; detect it and say "offline packs need iOS 26 or
  newer" (or move writes to a worker sync access handle later).
- ITP deletes script-writable storage — including the service worker, its Cache
  Storage and OPFS — after 7 days of Safari use with no interaction, unless the
  site is a Home Screen web app. `persist()` is heuristic, not a guarantee.
  Treat packs as cache, never as the only copy of user data.

**Estimating 139 MB/region:** the Moldova transfer is 93,172,399 B basemap +
45,338,586 B gzipped graph = 138,510,985 B (~139 MB); installed it grows to
204 MB because the graph decompresses. The web needs the basemap only
(~93 MB), so use `sum(files[].installed_bytes) × 1.1` per region, add a 2× peak
for updates, and refuse an install when that exceeds free space.

## Catalog JSON and the range proxy

Yes — one catalog can serve the web installer, with two additions: a per-region
M1 manifest endpoint, and a download path. Reuse
`GET /offline/packs/<region>/<file>`: it already streams 139 MB, forwards
`Range`, returns `206`/`416`, exposes CORS headers, and whitelists ids/files
before any outbound request (no SSRF). Keep `/packs/<region>/<name>` for the
native map source. Never fetch `github.com` release assets from the page: they
send no CORS headers, and `Cross-Origin-Embedder-Policy: require-corp` plus
`connect-src 'self'` in the CSP would block them anyway. With the proxy, CSP
needs no change.

**Service-worker bug to fix before any large download:** `serveShell` caches
every same-origin GET without a `Range` header. A pack download through
`/offline/packs/<region>/...` is such a GET, so the SW would store a second
139 MB copy in Cache Storage (same quota, evictable, unusable for ranges).
Exclude `/offline/packs/` and `/map/catalog` from the runtime cache. PMTiles
reads themselves send `Range` and are already skipped.

## Phased plan

1. **Now:** keep the single-pack demo and Monaco fixture. Fix the SW cache
   exclusion.
2. **Phase 2:** region picker reading `/map/catalog`; per-region M1 manifest
   from the server; downloads through the existing Rust proxy into
   `offline-packs/<region>/`; install/update/delete; quota meter and
   `persist()`; switch the map by recreating `createMap` with the selected
   region's `pmtilesUrl`; catalog errors fall back to Monaco.
3. **Later:** download queue/background resume, per-region search index, WASM
   routing, recovery after eviction, object-storage host swap.

## Tests

Puppeteer (already used by `web/app/test/offline-smoke.mjs`) with request
interception: mock `/map/catalog` with two tiny regions and
`/offline/packs/<r>/<file>` honouring `Range` with `206`, `Content-Range` and
`416`. Assert both install into `offline-packs/<r>/pack.json`, the SW returns
`206` from OPFS for each, deleting one leaves the other, and a stubbed
`navigator.storage.estimate()` below the region size produces the quota error.
Reset with a fresh `userDataDir`; in teardown remove `offline-packs`
recursively, delete `caches.keys()`, and unregister the worker.
