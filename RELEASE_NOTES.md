# Pinpoint 3.0.0 - Release Notes

**Released:** 2026-11-25 - **Version:** `3.0.0` (from `2.7.1`) -
**Status:** Unified desktop, mobile and web release.

**Artifacts:** `pinpoint-android-3.0.0.apk` (signed universal:
`arm64-v8a` + `armeabi-v7a`, minSdk 26, targetSdk 35) and the source archives
for the `v3.0.0` tag. The web client is served by `pinpoint-server` from the
same crate; the Windows/Linux/macOS desktop binaries are built by the `client`
CI jobs. iOS reports `CFBundleShortVersionString` 3.0.0 / build 30000. The APK
keeps `allowBackup=0`, `usesCleartextTraffic=0` and the network security
config.

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
- CI `rust`, CodeQL, `web-tests`, wasm, macOS/Windows clients and ZAP pass.
- APK `aapt`/`apksigner`: `com.pinpoint.desktop` **3.0.0 (30000)**, signed
  universal (`arm64-v8a` + `armeabi-v7a`), minSdk 26 / target 35,
  `allowBackup=0`, `usesCleartextTraffic=0`, network security config present.
