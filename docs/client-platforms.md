# Pinpoint — Client Platforms (one Rust crate, every OS)

Every client is the same crate (`pinpoint-desktop`, `src/`) — the only
per-OS code lives in `src/platform.rs`. The table below is the honest
status as of this commit; **nothing in this repo has been GUI-tested on
Linux/macOS/Android/iOS/Web from a Windows host** — the desktop CI job
(`windows-latest`) plus manual first-run checks on each OS are the
verification path.

| Client | Entry | Build | Status |
|---|---|---|---|
| Windows desktop | `src/main.rs` (`pinpoint-desktop`) | `cargo build --locked` | Builds in CI; interactive Slippy canvas + offline packs + OS keychain; run: `cargo run -- --backend http://localhost:8000` |
| Linux desktop | same binary | `Dockerfile.client` (X11) + `packaging/linux/` (.deb/.desktop) | Image builds; GUI needs X11 host |
| macOS desktop | same binary | `cargo build --locked` + `packaging/macos/` (.app via `cargo-bundle`) | Builds in CI (`client-macos`); GUI needs macOS |
| Android (Rust) | same `src/` via `cargo-apk` | `cargo apk build --lib --target x86_64-linux-android` (NDK 27 + SDK 35; see `android/README.md`) | APK builds, installs and runs on the API 37 x86_64 emulator with live tiles, responsive map-first UI and NativeActivity; AArch64 device/CI and release signing pending |
| Android (Kotlin) | `mobile/` (`org.pinpoint.mobile.MainActivity`) | `cd mobile && ./gradlew :app:assembleDebug` | Jetpack Compose + MapLibre + Keystore `SessionStore` + Clean Architecture (`:core:*`, `:feature:*`); build needs JDK 17 + Android SDK |
| iOS (Rust) | same `src/` via `cargo-xcodebuild` | `cargo xcodebuild --locked` (`ios/Info.plist`) | Bootstrap + deep-link + GPS provider in `src/mobile`; build needs macOS/Xcode |
| Web (Rust/WASM) | `src/bin/pinpoint-web.rs` + `web/` | `trunk serve --config web/Trunk.toml --release` | Static shell + wasm hook (`cargo check --target wasm32-unknown-unknown` in CI) |
| Web Admin `weba` | `src/bin/pinpoint-weba.rs` + `web/weba/` | `trunk serve --config web/weba/Trunk.toml --release` | Modular admin console shell; server-side authz authoritative |
| Web Kiosk `webk` | `src/bin/pinpoint-webk.rs` + `web/webk/` | `trunk serve --config web/webk/Trunk.toml --release` | Modular kiosk/check-in shell; token/share-link adapter points |
| Headless demo clients | `src/bin/demo_client.rs` (`pinpoint-demo-client`) | `Dockerfile.service` with `BIN=pinpoint-demo-client` | Alice/Bob containers log in via the API and publish locations; compose profile `demo` |

## Headless demo clients

`pinpoint-demo-client` is the honest replacement for server-side seed rows. It
uses the same public API as every other client:

1. `POST /api/v1/auth/login` with `PINPOINT_EMAIL` / `PINPOINT_PASSWORD`.
2. `POST /api/v1/locations/me` every `PINPOINT_INTERVAL_SECS` (default 30).
3. Re-authenticate when a publish fails (expired or admin-revoked session).

```bash
docker compose --profile demo up -d alice bob
docker logs -f pinpoint-alice
```

Only the public handle and demo coordinates are printed — never the password
or email. Plain HTTP to the internal `gateway` hostname requires the explicit
`PINPOINT_ALLOW_PLAINTEXT_HOSTS=gateway` opt-in; HTTPS remains the default.

## Mobile strategy (dual apps)

- Two distinct mobile clients are developed in parallel:
  - **Kotlin/Android** (`mobile/`, Jetpack Compose) — Android-first.
  - **Rust** (`android/` + `ios/`, `cargo-apk`/`cargo-xcodebuild`) —
    Android + iOS from the same crate.
- Both consume the same backend contract (`src/api.rs`, REST v1). Build
  the Rust mobile targets on a host with the NDK/Xcode; the Kotlin app
  builds with JDK 17 + Android SDK.

## Tooling

- Efficient AI-assisted development: see `docs/antigravity-guide.md`.

## Shared behavior (all platforms)

- Map-first landing with offline demo pins; sign-in optional; sign-in
  screens gate Friends/Sharing/Admin/Password.
- Backend access is modular (`src/backend/`): app/UI/webapp/mobile code depends
  on `Backend`/`DynBackend` + `BackendConfig`/`BackendKind`, with the current
  REST v1 contract implemented by `src/api.rs`. Changed backend architectures
  should be handled by adding/selecting an adapter, not rewriting clients.
- Backend URL: `--backend` > `$PINPOINT_BACKEND` > `platform::default_backend_url()`
  (`http://localhost:8000` desktop, `http://10.0.2.2:8000` Android emulator).
  Plain HTTP off-loopback is refused client-side (`api::validate_backend_url`, ADR-0006).
- OSM services: `$PINPOINT_NOMINATIM_URL`, `$PINPOINT_OSRM_URL`,
  `$PINPOINT_OVERPASS_URL`, `$PINPOINT_TILE_URL` point at self-hosted
  infrastructure in production (ADR-0011); public defaults are demo-only.
- Transport (REST v1 adapter): session cookie + `X-CSRF-Token` double-submit (`src/api.rs`).
- Privacy: friend coordinates are never sent to Nominatim/OSRM/Overpass
  (`src/osm.rs`); OSM hand-off links only; ODbL attribution always rendered.
- Storage: bookmarks/tilepacks under `~/.pinpoint` on desktop;
  `platform::app_data_dir()` is `None` on Android/iOS/Web → in-memory only.
  OS keychain for keys/tokens is wired via `src/security/keychain.rs`
  (`OsKeychain` on desktop; `FileKeychain` fallback).
- Web variants: `webapp::WebAppKind::{Web,Admin,Kiosk}` share `src/webapp/*`;
  `webk` kiosk/share-link flows are isolated behind `webapp::kiosk::KioskBackend`.

## Per-platform build/verify

### Desktop (Windows/Linux/macOS)

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked && cargo test --locked
cargo run -- --backend http://localhost:8000
./target/debug/pinpoint-desktop --help
```

### Linux

- Native: deps in `Dockerfile.client` (xcb, libssl, fontconfig).
- Container: `docker compose --profile client run --rm client` (X11 host only).
- `.deb`: `cargo deb --locked` (`[package.metadata.deb]` + `packaging/linux/`).

### macOS

- `xcode-select --install` then `cargo build --locked`.
- `.app`: `cargo bundle --release --format app` (`[package.metadata.bundle]`).
- iOS: `cargo xcodebuild --locked` (`ios/Info.plist`).

### Android (Rust — `cargo-apk`)

```bash
rustup target add aarch64-linux-android
cargo install cargo-apk --locked
cargo apk build --locked
adb install target/aarch64-linux-android/debug/pinpoint-debug.apk
```

Cleartext is allowlisted only for `10.0.2.2`/`localhost` (`android/network_security_config.xml`).

### Android (Kotlin — Jetpack Compose + MapLibre)

```bash
cd mobile
./gradlew :app:assembleDebug
adb install app/build/outputs/apk/debug/app-debug.apk
```

Clean Architecture modules (`:core:domain`, `:core:network`, `:core:data`, `:core:map`, `:feature:auth`, `:feature:map`, `:feature:friends`, `:feature:sharing`) talking to the same `/api/v1/*` contract.

### iOS

```bash
rustup target add aarch64-apple-ios
cargo install cargo-xcodebuild --locked
cargo xcodebuild --locked
```

### Web (Rust/WASM)

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
trunk serve --release   # http://localhost:8080
```

The static shell in `web/index.html` drives first paint today; the full
egui UI on wasm requires replacing the desktop `tokio` runtime with a
wasm-friendly spawn (`wasm_bindgen_futures`), moving bookmarks to
`localStorage`, and stubbing tilepacks — tracked as the next web step.
`src/bin/pinpoint-web.rs`'s WASM entry is the seam for that work.

## What can't be tested here

- No Linux X11 display, no macOS/Xcode, no Android NDK/SDK, no wasm
  toolchain in this session. CI covers Windows build + clippy + fmt +
  tests; the other matrices need `macos-latest` / `ubuntu-latest` /
  Android-SDK jobs, or a manual first-run on each OS above.
