# 0020. Hybrid MapLibre offline maps (PMTiles + SQLite FTS5 + Valhalla)

Date: 2026-11-17

## Status

Proposed

Amends [ADR-0007](./0007-rust-multiplatform.md) — map subsystem only.

## Context

ADR-0007 made Rust the sole application codebase and ruled that the map
hand-off stays OSM-only, with "no bulk tile caching in-app; offline UX comes
from OS-level packs / user downloads, not a bundled engine". Pinpoint 2.0
instead commits to a first-class offline map for Moldova (design:
`docs/superpowers/specs/2026-11-16-offline-maps-design.md`).

An egui-native vector map would mean reimplementing a renderer, style engine,
label placement and gesture handling in Rust. The mature, OpenMapTiles-
compatible renderer is MapLibre GL JS, which runs in the WebView already used
by Android and can be embedded on desktop. Search and routing, by contrast,
benefit from native SQLite and the Valhalla C++ engine. The map subsystem
therefore needs a documented exception to ADR-0007's "Rust only" rule rather
than a silent divergence.

## Decision

The map subsystem is hybrid: a MapLibre GL JS page inside a WebView, backed by
region packs built by an offline data pipeline. Everything outside the map
subsystem stays Rust/egui.

- **Rendering:** MapLibre GL JS in a WebView (Android `WebView`, desktop `wry`/
  WebView2/WKWebView, and the same module in the web client). Native serves
  `web/map/` and packs from a loopback `127.0.0.1` HTTP server with range
  support; the WebView never receives session or CSRF material.
- **Basemap:** PMTiles built from the Geofabrik extract with Planetiler's
  OpenMapTiles schema, zoom 0–14 with overzoom to 18; styles, sprites and
  glyphs are vendored.
- **Search:** a per-region SQLite database with FTS5 tables (`places`,
  `places_fts`), built by the Rust CLI `pinpoint-offline-index`
  (`offline-build` feature). Names are indexed for `name`, `name:ro`,
  `name:ru`, `name:uk`, `name:bg`, `name:gag`, `name:en`, `alt_name`,
  `official_name` and `short_name`; diacritics are folded and Cyrillic queries
  work.
- **Routing:** Valhalla tiles for `auto`, `pedestrian` and `bicycle`, shipped as
  `moldova-valhalla.tar` and consumed natively through a small FFI/JNI wrapper;
  the web client uses a Valhalla WASM build where the M1 spike proves it. The
  router is behind a `Router` trait so a Rust fallback can replace it.
- **Pipeline:** `scripts/offline/` (PowerShell + Docker) drives
  Geofabrik → Planetiler → PMTiles, `pinpoint-offline-index` → search SQLite,
  `valhalla_build_tiles` → routing tar, then writes `manifest.json` with
  `version`, `built_at`, `bbox`, `minzoom`, `maxzoom` and per-file `url`,
  `bytes`, `sha256`, plus the ODbL license and attribution. The app only ever
  consumes the manifest; it never guesses URLs.

### ADR-0007 exception

For the map subsystem only: MapLibre GL JS in a WebView is permitted, and
"no bulk tile caching in-app" is replaced by verified, manifest-driven region
packs downloaded to app storage (OPFS on web, `filesDir` on Android). The
rest of ADR-0007 stands: no Kotlin/JS app rewrite, platform code stays in
`src/platform.rs`, and the surrounding UI, data and networking layers remain
Rust.

### Source and image pinning

The Geofabrik input is the moving `-latest.osm.pbf` snapshot and the build
images were referenced as `:latest` while M1 was probed. From now on:

- A published pack is built from digest-pinned images; the digests are recorded
  with the pack (M1 records
  `openmaptiles/planetiler-openmaptiles@sha256:f5d7bcd8…` and
  `ghcr.io/valhalla/valhalla@sha256:b8c515f8…` in `docs/offline/moldova.md`).
  `:latest` is a local-development convenience only and never acceptable for a
  released pack.
- The source extract is pinned by download date and SHA-256, and pack versions
  are immutable (`<region>-YYYYMMDD`); rebuilding `latest` produces a new
  version, never a silent change to an existing one.
- The manifest binds the artifacts to those bytes, so the app can verify what
  it downloaded regardless of what the host serves later.

### ODbL obligations

OpenStreetMap data is ODbL 1.0. Each pack ships `ATTRIBUTION.md` and a manifest
carrying `"© OpenStreetMap contributors"`, the license id and the license URL;
the map, About screen and pack download UI show the credit. Derived databases
(the search index and routing graph) are distributions of ODbL data: they stay
attributed, are not relicensed, and any publicly served pack keeps the same
notice. No third-party tile host is contacted, and friend coordinates are never
sent to a map provider.

## Consequences

- The map subsystem no longer follows ADR-0007 literally; this ADR is the
  recorded exception and must be Accepted before M1 ships.
- Two runtimes to test (Rust core + WebView page), plus native Valhalla and
  SQLite packaging.
- Offline behaviour is auditable: packs are versioned, hashed and verified
  before the app claims offline readiness.
- The same MapLibre module serves the web client, so online and offline maps
  share one style and one set of layers.
- Supply-chain risk is reduced by digest-pinned images and immutable pack
  versions; `latest` remains a dev-only shortcut.
- If the WebView/Valhalla WASM spikes fail, the fallbacks are a separate map
  window and a Rust router behind the `Router` trait; the pack format does not
  change.

## Alternatives considered

- **MapLibre Native on Android/iOS plus a separate web map** — better native
  performance, but two map stacks, two style pipelines and no desktop story.
- **Rewrite the client as a PWA** — discards the Rust/egui app and the
  multiplatform work of ADR-0007.
- **Raster tile packs in the existing egui map** — small scope, but no vector
  styling, poor size/quality trade-off and no search/routing parity.
