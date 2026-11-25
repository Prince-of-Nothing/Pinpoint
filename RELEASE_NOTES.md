# Pinpoint 3.0.0 - Release Notes

**Released:** 2026-11-25 - **Version:** `3.0.0` (from `2.7.1`) -
**Status:** Unified desktop, mobile and web release.

**Artifacts:** `pinpoint-android-3.0.0.apk` (signed universal:
`arm64-v8a` + `armeabi-v7a`, minSdk 26, targetSdk 35),
`pinpoint-desktop-linux-x64-3.0.0.zip` (10,057,368 B: binary + `pinpoint.desktop` + icon),
`pinpoint-desktop-macos-3.0.0.zip` (6,131,546 B: unsigned `Pinpoint.app`),
`pinpoint-desktop-windows-x64-3.0.0.zip` (5,722,217 B, release build) and
`pinpoint-server-windows-x64-3.0.0.zip` (5,179,101 B, release build) plus the
source archives for the `v3.0.0` tag. The server zip **is** the web bundle:
the web client is embedded, so run `pinpoint-server.exe` and open
`http://127.0.0.1:8080/`; there is no separate CDN or wasm download. iOS
reports `CFBundleShortVersionString` 3.0.0 / build 30000. The APK keeps
`allowBackup=0`, `usesCleartextTraffic=0` and the network security config.

## What is in 3.0

This is one version number for all three clients, built from the same Rust
crate:

- **Map:** desktop right-column shell, top-left search, compass/pitch,
  fullscreen, right-click/long-press actions, distance measure, friend-name
  labels and freshness on web and native.
- **Privacy:** explicit per-friend sharing, 24-hour owner-only trail, private
  notes, ECIES messages, offline packs and opt-in online tiles.
- **Auth:** password, anonymous/recovery, MFA, Google and GitHub OAuth
  (authorization code + PKCE, verified email, no auto-linking).
- **Server:** Axum services, PostgreSQL or in-memory store, no-store API
  responses, `Retry-After` throttles, split-service headers and encrypted
  backups.
- **Web:** the same versioned server serves the app and the API; the service
  worker cache is `pinpoint-offline-v10`, so installed clients refresh their
  shell once.

There is **no data migration**: API v1, cookies, sessions, PostgreSQL tables,
encrypted notes and ECIES message envelopes are unchanged from 2.7.x.

## Verification

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo build --locked`, `cargo test --locked`: pass, **381 lib tests** plus
  integration suites.
- `scripts/smoke.ps1`: **29 passed, 0 failed**.
- `/health` and the desktop About screen report `3.0.0` from the crate.
- Windows desktop `--help` reports `Pinpoint 3.0.0`; the server zip's
  `/health` reports `3.0.0` and `/` serves the web client (`text/html`).
- Linux desktop `--help` reports `3.0.0` in the release workflow.
- macOS desktop `--help` reports `3.0.0` in the release workflow.
- CI `rust`, CodeQL, `web-tests`, wasm, macOS/Windows clients and ZAP pass.
- APK `aapt`/`apksigner`: `com.pinpoint.desktop` **3.0.0 (30000)**, signed
  universal (`arm64-v8a` + `armeabi-v7a`), minSdk 26 / target 35,
  `allowBackup=0`, `usesCleartextTraffic=0`, network security config present.
