# Pinpoint 2.7.1 - Release Notes

**Released:** 2026-11-25 - **Version:** `2.7.1` (from `2.7.0`) -
**Status:** Native map evidence and screenshot fixes after 2.7.0.

**Artifacts:** `pinpoint-android-2.7.1.apk` (`arm64-v8a`, minSdk 26,
targetSdk 35) and the source archives for the `v2.7.1` tag. The debug APK keeps
`allowBackup=0`, `usesCleartextTraffic=0` and the network security config.

## What is new

- **`--screenshot` is real:** `pinpoint-desktop --screenshot PATH` now captures
  the actual egui window via `ViewportCommand::Screenshot`, waits for the first
  frames to settle and writes a PNG. A backend without readback support exits
  cleanly after a bounded wait instead of hanging CI.
- **Screenshot mode seeds a map:** a demo user, two friend pins (Sam at exact,
  Mira at approximate) and the owner’s position, so the artifact shows the
  native Map screen rather than the login form.
- **Screenshot mode loads real tiles:** with `--backend` running it uses the
  public `/map/tiles` proxy; `PINPOINT_SCREENSHOT_ROUTE=1` seeds a deterministic
  A→B route for the route capture.
- **Readable friend labels:** native labels draw a dark rounded pill; the
  previous white text disappeared on light map tiles.
- **Desktop route panel:** the map screen now actually draws `draw_route_sheet`
  when a route is opened (it previously drew only the line), and the sheet uses
  the window height instead of a fixed 520 px.
- **Refreshed evidence:** `docs/assets/presentation/app-map.png`,
  `docs/midterm2/assets/06-native.png` and `08-route.png` now come from the
  2.7.1 code with real OSM tiles.

## Verification

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo build --locked`, `cargo test --locked`: pass, **381 lib tests** plus
  integration suites.
- `scripts/smoke.ps1`: **29 passed, 0 failed**.
- Native map screenshot: 1,259,142-byte 1500x1000 PNG with tiles, labelled
  pins and the scale bar.
- Native route screenshot: `2.8 km · 8 min`, summary and steps.
- CI `rust`, CodeQL, `web-tests`, wasm, macOS/Windows clients and ZAP pass.
- APK `aapt`: `com.pinpoint.desktop` **2.7.1 (20701)**, `arm64-v8a`,
  minSdk 26 / target 35, `allowBackup=0`, `usesCleartextTraffic=0`, network
  security config present.
