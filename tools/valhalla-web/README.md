# Valhalla web routing spike

A throwaway measurement harness for the M1 Moldova Valhalla graph. It answers
one question: can a browser compute a real Chișinău → Bălți route offline today?
The result and recommendation are in
[`docs/offline/routing-spike.md`](../../docs/offline/routing-spike.md).

Everything is local: the page imports the npm packages from `node_modules`, the
graph tar from the M1 build, and no request leaves `127.0.0.1`. There is no CDN
at runtime.

## Packages

| Package | Version | Role |
|---|---|---|
| `valhalla-browser` | `0.2.1` | Requested package; requires a native indexed `graph.tar` and a pinned Valhalla 3.8.3 dataset |
| `@jansoft/mbujkanji-valhalla-wasm` | `0.1.2` | Requested alternate (the task's `mbukanji` spelling is not published); broken `loadTiles` |
| `@adret-maps/valhalla-wasm` | `3.9.0-alpha.2` | Alternate that consumed the M1 tiles and produced routes |

`valhalla-browser` and `@adret-maps/valhalla-wasm` are pinned in
`package.json`/`package-lock.json`. The Jansoft package is intentionally not a
dependency: installing it pulls 699 transitive packages; `npm run install:jansoft`
installs it with `--no-save --no-package-lock` only for the negative test.

## Prerequisites

- Node.js 22+.
- Python 3.9+ (standard library only) for the dataset preparation.
- The M1 tar and config (not committed):
  - `dist/offline/moldova/moldova-valhalla.tar`
  - `dist/offline/moldova/moldova-valhalla.json`
- Puppeteer in a `NODE_PATH` (the repository's map smoke test documents the same
  pattern), or install it locally.

## Run

```powershell
cd tools/valhalla-web
npm ci

# Optional: install the broken alternate for its negative test.
npm run install:jansoft

# Build the experimental valhalla-browser dataset from the M1 tar.
# This writes ~120 MB; add --with-tiles for another ~120 MB.
python prepare-sdk-dataset.py `
  --tar C:\Users\Dinusauuuur\Desktop\git\Pinpoint\dist\offline\moldova\moldova-valhalla.tar `
  --config C:\Users\Dinusauuuur\Desktop\git\Pinpoint\dist\offline\moldova\moldova-valhalla.json `
  --out .data\sdk-dataset

$env:PINPOINT_MOLDOVA_TAR = "C:\Users\Dinusauuuur\Desktop\git\Pinpoint\dist\offline\moldova\moldova-valhalla.tar"
$env:NODE_PATH = "C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\node_modules"
node run.mjs
```

Use `--only adret,adret-corridor` for the positive path, or
`--only vb-raw,vb-indexed,jansoft` for the negative paths. Full per-scenario
results and network logs are written to `.data/results/`.

## What the scenario does

- Loads the 120,319,488-byte M1 tar over local HTTP.
- `adret`: parses the tar, loads all 114 tiles into `MemoryTileSource`, starts
  `@adret-maps/valhalla-wasm`, and routes `auto`, `pedestrian` and `bicycle`.
- `adret-corridor`: same tar, but only 24 tiles inside the Chișinău–Bălți
  corridor are handed to the engine (49,847,688 bytes).
- `jansoft`: initializes the WASM, then calls `loadTiles` on the M1 tar.
- `valhalla-browser`: points at a manifest for the unmodified tar (`vb-raw`) or
  at the experimental indexed dataset (`vb-indexed`/`vb-individual`).

The prepared dataset is an **experiment**, not a shipping format. The
`valhalla-browser` guide explicitly says a hand-written index is not a substitute
for `valhalla_build_extract`, and the M1 graph is Valhalla 3.9.2 while the
package's runtime is 3.8.3.

## Files

| Path | Purpose |
|---|---|
| `prepare-sdk-dataset.py` | Builds the indexed tar + manifest for the `valhalla-browser` experiment, and a raw-tar manifest for the exact failure |
| `serve.mjs` | Local HTTP server with byte ranges, strong ETags, COOP/COEP and no CDN |
| `run.mjs` | Puppeteer runner; records timings, network, page/process memory and full JSON results |
| `spike.html`, `spike-entry.js` | The single headless page |
| `scenarios/*.js` | One module per package test |
| `lib/tar.mjs` | Minimal uncompressed tar parser used by the `adret` scenario |
| `lib/metrics.mjs` | Memory snapshot and route summary helpers |
