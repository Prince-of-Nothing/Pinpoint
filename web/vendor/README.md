# Vendored browser libraries

These files are pinned copies of the upstream npm distributions. Nothing under
`web/` loads a CDN at runtime; keep it that way.

| File | Package | Version | Upstream distribution |
|---|---|---|---|
| `maplibre-gl.js` | [`maplibre-gl`](https://www.npmjs.com/package/maplibre-gl) | `6.18.0` | `https://registry.npmjs.org/maplibre-gl/-/maplibre-gl-6.18.0.tgz` → `package/dist/maplibre-gl.mjs` |
| `maplibre-gl-worker.mjs` | `maplibre-gl` | `6.18.0` | same tarball → `package/dist/maplibre-gl-worker.mjs` |
| `maplibre-gl.css` | `maplibre-gl` | `6.18.0` | same tarball → `package/dist/maplibre-gl.css` |
| `pmtiles.js` | [`pmtiles`](https://www.npmjs.com/package/pmtiles) | `4.5.0` | `https://registry.npmjs.org/pmtiles/-/pmtiles-4.5.0.tgz` → `package/dist/pmtiles.js` |
| `sqlite3.mjs` | [`@sqlite.org/sqlite-wasm`](https://www.npmjs.com/package/@sqlite.org/sqlite-wasm) | `3.54.1-build1` | `https://registry.npmjs.org/@sqlite.org/sqlite-wasm/-/sqlite-wasm-3.54.1-build1.tgz` → `package/dist/index.mjs` |
| `sqlite3.wasm` | `@sqlite.org/sqlite-wasm` | `3.54.1-build1` | same tarball → `package/dist/sqlite3.wasm` |

MapLibre GL JS 6.x is published as an ES module. The `maplibre-gl.js` copy is the
upstream `maplibre-gl.mjs` content (the distribution has no UMD build); it is
imported with `import * as maplibregl from "../vendor/maplibre-gl.js"`. MapLibre
resolves its worker relative to that module's own URL, so the worker **must**
stay next to it and keep the exact filename `maplibre-gl-worker.mjs`.

`pmtiles.js` is the upstream browser IIFE build. It bundles `fflate` and exposes
the API as `globalThis.pmtiles`; `web/map/map.js` loads it with a local
`<script>` tag the first time a map is created.

`sqlite3.mjs` is the upstream SQLite WASM ES module (`dist/index.mjs`) and
`sqlite3.wasm` is its SQLite 3.54.1 binary. It is imported dynamically by
`web/map/search.js` and initialised with the WASM binary explicitly, so the
engine never guesses a script directory or fetches a CDN. This build includes
FTS5, which the M1 `places_fts` index requires. The stock `sql.js` package is
**not** used: its standard browser build does not compile FTS5.

The trailing `sourceMappingURL` comments were removed from the JavaScript copies
so the preview never requests absent `.map` files. No other code was changed.

## SHA-256 of the vendored files

```text
fe19cbe23b0792b0fcfbeb91908ec4332eda197f6643c55391971d74a5b75b38  maplibre-gl.js
7ad0fc05178aff1690b899f7277912ebbb1659c40bf2214c83b6b914e44d2e33  maplibre-gl-worker.mjs
a90798a1953017790e590f7e1bd35ff103a8a4263972cb47290b78adf86f8c20  maplibre-gl.css
ca16879182f180bd18648457701ec7106d2159944dca107fd17afc4110c79056  pmtiles.js
0dcdd860e6ec672623b13b8b8e41bb6c78be7a5df80ec553fc4f69160161c8af  sqlite3.mjs
b8a92a19f441a90ff2339fe988fbe317c4adf3ed844096f12aee04aa72d58257  sqlite3.wasm
```

## Licenses

- MapLibre GL JS: BSD-3-Clause, see `maplibre-gl.LICENSE.txt`.
- PMTiles: BSD-3-Clause, see `pmtiles.LICENSE.txt`.
- SQLite WASM: the wrapper is Apache-2.0 and the bundled SQLite code is public
  domain; see `sqlite3.LICENSE.txt` (upstream `LICENSE.md`, which also carries
  the Emscripten/third-party notices).

## Updating

Pick explicit versions, download the tarballs with `npm pack`, copy the same
files, strip only the source-map comment, update the table and hashes above, and
rerun `node web/map/test/smoke.mjs` and
`node web/map/test/search-smoke.mjs`. Do not use a floating version.

For SQLite WASM, copy `package/dist/index.mjs` to `sqlite3.mjs` and
`package/dist/sqlite3.wasm` to `sqlite3.wasm`; also refresh
`sqlite3.LICENSE.txt` from the upstream
`https://raw.githubusercontent.com/sqlite/sqlite-wasm/main/LICENSE.md`.
