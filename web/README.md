# web/ — Rust/WASM clients (modular)

Browser clients share the `pinpoint_desktop` crate and its modular backend
abstraction (`crate::backend::{Backend, BackendConfig, DynBackend}`), so
backend architecture changes (endpoints/auth/protocol version) are handled in
adapters/config rather than UI code.

Build with [Trunk](https://trunk.rs/):

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk --locked

# General web client
trunk serve --config web/Trunk.toml --release        # http://localhost:8080

# Admin console (weba)
trunk serve --config web/weba/Trunk.toml --release

# Kiosk/check-in (webk)
trunk serve --config web/webk/Trunk.toml --release
```

## What's here

- `index.html` + `Trunk.toml` — general web shell / `src/bin/pinpoint-web.rs`.
- `weba/` — admin console SPA shell + Trunk config / `src/bin/pinpoint-weba.rs`.
- `webk/` — kiosk/check-in shell + Trunk config / `src/bin/pinpoint-webk.rs`.
- `src/webapp/` — shared portable webapp modules (config, view models, admin,
  kiosk adapter points).

## Modularity / backend changes

- Webapps resolve backend config through `webapp::config::WebAppConfig` and the
  `backend` module. Add a new `BackendKind`/adapter when the backend
  architecture changes; keep UI/view-model code stable.
- `webk` kiosk token/share-link endpoints are isolated behind
  `webapp::kiosk::KioskBackend` so evolving kiosk contracts don't leak into the
  general client.
- `weba` uses the shared `Backend` admin/user operations; server-side
  authorization remains authoritative (UI role checks are fail-closed hints).

## Assets are local-only

- **Fonts:** the UI uses the operating system's `system-ui` stack. There is no
  `@font-face`, no Google Fonts and no font CDN, so a page load makes zero
  font requests. `font-src 'self'` in the CSP keeps it that way if a bundled
  font is added later.
- **Scripts/styles/icons:** `/app.js`, `/app.css` and `/favicon.svg` are served
  by the Pinpoint server itself (embedded with `include_str!`), not a CDN.
- **Map tiles:** the browser only calls `/map/tiles/{z}/{x}/{y}` on the same
  origin; the server proxies or serves self-hosted tiles (ADR-0011).
- **Desktop:** offline tile packs prefer the configured backend's
  `/map/tiles` proxy before any public fallback (`tilepacks::download_pack`).
- `tests/assets.rs` fails the build if a third-party asset host or an absolute
  subresource URL is added to the shipped HTML/CSS/JS.

## Status (honest)

Desktop GUI modules (`src/ui`, `map`, `tilepacks`) are `#[cfg(not(wasm32))]`.
The wasm bins currently provide modular wiring/stubs plus HTML shells. A full
egui web UI reusing desktop screens requires the `tokio` → wasm-runtime swap
(`wasm_bindgen_futures`/async spawn), browser storage for bookmarks, and no
filesystem tilepacks — tracked in [`docs/client-platforms.md`](https://github.com/Prince-of-Nothing/Pinpoint/wiki/Client-Platforms).

Backend URL: `PINPOINT_BACKEND` / host config > per-platform default. In
production, use HTTPS (ADR-0006); plain HTTP is only valid for loopback
development contexts as enforced by backend adapters.
