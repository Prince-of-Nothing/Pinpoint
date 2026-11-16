# Offline place search

`web/map/search.js` searches the M1 `places` / `places_fts` SQLite index on the
device. It is a small ES module with no imports from `web/app/` and no runtime
CDN: the database, the SQLite WASM engine and the WASM binary all come from the
page's own origin. Query text never leaves the device.

The module is intended for the web app and the Android/desktop WebView pages.
It does not depend on `map.js`, so search can be used without a rendered map and
the map can be used without search.

## Quick start

```html
<script type="module">
  import { openSearch } from "/map/search.js";

  const search = await openSearch({
    sqliteUrl: "/packs/moldova-search.sqlite",
  });

  try {
    const places = search.search("balti", 5);
    for (const place of places) {
      console.log(place.name, place.lat, place.lon);
    }
  } finally {
    search.close();
  }
</script>
```

The database is opened and deserialised during `openSearch`; `search()` is
synchronous afterwards.

## API

### `openSearch({ sqliteUrl, wasmUrl? })`

Returns a `Promise` that resolves to a search handle after the engine and index
are ready.

| Option | Type | Required | Notes |
|---|---|---|---|
| `sqliteUrl` | `string \| URL` | yes | M1 `places`/`places_fts` SQLite file. Relative paths resolve against the page (`document.baseURI`). |
| `wasmUrl` | `string \| URL` | no | SQLite WASM binary. Defaults to `web/vendor/sqlite3.wasm` relative to this module. Override for a different deployment path. |

Both URLs must have the **same origin as the page**. `openSearch` rejects a
cross-origin pack or WASM file with an error instead of fetching it. The engine
is initialised once per page; a later call reuses it.

The SQLite file is fetched as bytes and passed to `sqlite3_deserialize` in WASM
memory. It does not need to be copied into OPFS, and the opened connection is
put in `PRAGMA query_only = ON` mode. The file must be the M1 schema; an
arbitrary SQLite database is rejected with a clear error.

Failure cases:

- missing `sqliteUrl`;
- a URL that is not same-origin;
- an HTTP error or an empty body;
- a file without `places` and `places_fts`;
- a WASM/engine load failure.

### `search(query, limit = 10)`

Returns an array of at most `limit` places, ordered by FTS5 relevance and
`places.importance`:

```js
[
  {
    name: "Bălți",        // display name from places.name
    kind: "city",         // places.kind
    lat: 47.76,
    lon: 27.93,
    address: null,        // places.address or null
    importance: 0.9,
  },
]
```

`limit` is clamped to `1..50`; a non-finite value falls back to `10`.

Results are plain data. Render `name`, `kind` and `address` with `textContent`
(or an equivalent safe binding), not `innerHTML`; a pack is untrusted input even
when it is served from the same origin.

Query handling:

- The query is normalised with Unicode NFKC and split into runs of Unicode
  letters and numbers. Punctuation, whitespace, symbols and emoji are
  separators, so `Bălți, Moldova` becomes two tokens.
- Romanian `ă â î ș ț` and Russian `ё` are folded the same way the M1 builder
  folds `places.normalized`, so `balti` finds `Bălți` and `кишин` finds
  `Кишинёв`.
- Every token becomes an FTS5 **prefix** term (`"balti"*`). Multiple tokens use
  FTS5's implicit AND.
- Ranking is `rank * (1 + 0.75 * importance)`, then `importance DESC`, then
  name. This promotes important places without replacing text relevance
  entirely.
- An empty, whitespace-only, punctuation-only or emoji-only query returns `[]`
  and never reaches FTS5; FTS5 would treat those as malformed MATCH syntax.
- Query strings are tokens, not SQL. Quotes and FTS5 operators are not
  interpreted as syntax.

`search` throws after `close()`. It does not throw for an ordinary empty or
punctuation-only query.

### `close()`

Releases the WASM database and its deserialised buffer. It is idempotent.
A later `search` throws.

## Hosting requirements

The page must be served over HTTP(S); `file://` cannot satisfy same-origin
module/WASM loading.

Ship these two files next to each other under `web/vendor/` (or pass a
same-origin `wasmUrl`):

| File | Version | License |
|---|---|---|
| `sqlite3.mjs` | `@sqlite.org/sqlite-wasm` `3.54.1-build1` | Apache-2.0 wrapper, SQLite public domain |
| `sqlite3.wasm` | same release | SQLite public domain |

`web/map/serve.mjs` serves `.wasm` as `application/wasm` and `.sqlite` as
`application/vnd.sqlite3`. A production static host should do the same; a
wrong WASM MIME type usually falls back to non-streaming instantiation rather
than failing, but the correct type is recommended.

The SQLite wrapper is vendored because the M1 index uses **FTS5**. The stock
`sql.js` browser distribution is smaller but does not compile FTS5, so it
cannot query `places_fts`.

### CSP and COOP/COEP

This module does **not** use OPFS, SharedArrayBuffer or a worker. It fetches the
database and WASM as ordinary same-origin responses and deserialises the
database into the WASM heap. Consequently:

- **No COOP/COEP headers are required.** The page does not need
  `Cross-Origin-Opener-Policy` or `Cross-Origin-Embedder-Policy`, and does not
  need `crossOriginIsolated === true`.
- A strict CSP needs `script-src 'self' 'wasm-unsafe-eval'` (or
  `'unsafe-eval'` on browsers that do not yet know `'wasm-unsafe-eval'`) so
  `WebAssembly.compile` is allowed. `connect-src 'self'` must allow the
  same-origin SQLite fetch; `worker-src` is not needed for the search path.
- Serving the module and WASM from a CDN or another origin is intentionally
  rejected. Host them on the page origin.

If a later stream adds OPFS-backed pack storage, that feature would be separate
and would introduce cross-origin-isolation requirements. It is not used here.

## Android and desktop WebView

Point the WebView at a loopback HTTP server (not `file://`) that serves `web/`
and the pack from the same origin:

```js
const search = await openSearch({
  sqliteUrl: "http://127.0.0.1:8765/packs/moldova-search.sqlite",
});
```

The page itself must also be loaded from `http://127.0.0.1:8765/...`; mixing a
loopback page with a pack on another origin is rejected by design. The WebView
server must support ordinary GET responses for the SQLite and WASM files
(Range support is only needed for PMTiles, not search).

## Data attribution

The index is derived from OpenStreetMap data. **© OpenStreetMap contributors,
ODbL 1.0.** Keep the credit visible wherever search results or the map are
shown, and keep the ODbL notice with any redistributed pack. See
[`fixtures/README.md`](fixtures/README.md).

## Test

```powershell
node web/map/test/search-smoke.mjs
```

The test serves `web/` on a random loopback port, checks the vendored assets
and fixture, then runs the real search module. It uses headless Chromium when
Puppeteer is available; otherwise it runs the same assertions in a Node
fallback. It asserts that `balti` finds Bălți, `кишин` finds Кишинёв,
punctuation-only queries return `[]`, and no request leaves the local origin.

If Puppeteer lives in a separate `node_modules`:

```powershell
$env:NODE_PATH="C:\path\to\node_modules"
node web/map/test/search-smoke.mjs
```
