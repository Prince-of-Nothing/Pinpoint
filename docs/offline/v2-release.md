# Pinpoint 2.0 release checklist

This is the integration gate for the offline-map release. It is deliberately
short: every item is a command or a device check, not a description.

## 1. Branches merged into `feat/v2-integration`

- [ ] `feat/wave2-webapp-map` — MapLibre app map, pack download/install, search
      and route wiring in `web/app`.
- [ ] `feat/wave2-android-webview` — Android WebView shell, loopback map server,
      runtime location/notification permissions, APK.
- [ ] `feat/wave2-desktop-webview` — desktop map window, loopback server,
      geolocation permission handling.
- [ ] `feat/wave3-offline-packs` — manifest download, SHA-256, OPFS, service
      worker Range serving, delete/update.
- [ ] `feat/wave3-pack-proxy` — same-origin `/offline/packs/moldova/*` proxy
      with Range and CORS.
- [ ] `feat/wave4-native-nav` — native maneuvers, road names, compact graph
      cache, Android arm64 build.

Resolve `src/server/mod.rs` and `CHANGELOG.md` conflicts by keeping both
routers and both entries. Do not drop the pack proxy's `offline_router()` or
the app adapter's routes.

## 2. Local gate

```powershell
pwsh -NoProfile -File scripts/v2/check.ps1
```

Strict mode: all of these must exist and pass.

- Rust fmt/clippy/build/test + `offline-build` feature tests.
- `scripts/offline/tests/run.ps1` (six pipeline tests).
- `tools/native-router` fmt/clippy/test.
- `web/map/test/smoke.mjs`.
- `web/map/test/search-smoke.mjs`.
- `web/app/test/search-ui.mjs`.
- `web/app/test/offline-smoke.mjs`.
- `web/app/test/route-ui.mjs`.
- Every other standalone `web/app/test/*.mjs`.

## 3. Pack and proxy

```powershell
# Local server
cargo run --bin pinpoint-server
curl -sS http://127.0.0.1:8000/offline/packs/moldova/manifest.json
curl -sS -D - -o NUL -r 0-7 -H "Origin: http://127.0.0.1:8000" `
  http://127.0.0.1:8000/offline/packs/moldova/moldova.pmtiles
```

- [ ] Manifest `200`, every `files[].url` is `/offline/packs/moldova/<name>`.
- [ ] PMTiles range `206`, body starts `PMTiles`, `Content-Range` present.
- [ ] `Access-Control-Allow-Origin: *` and
      `Access-Control-Expose-Headers: Content-Range, Content-Length, Accept-Ranges`.
- [ ] Unknown pack/file and `../` traversal return `404`.
- [ ] The 220 MB tar streams; server RSS does not grow by 220 MB.

## 4. Web app offline flow

In a clean browser profile, with DevTools open:

1. Install the Moldova pack from `/offline/packs/moldova/manifest.json`.
2. Watch progress and SHA-256 verification; reload.
3. Open the map from the local pack; pan/zoom without a network request.
4. Search `Bălți` and `Кишинёв`; select a result.
5. Route Chișinău → Bălți; confirm distance/time and maneuvers.
6. Delete the pack; confirm OPFS entries and the map source are gone.
7. Network tab: no external requests after the pack is installed.

- [ ] No console/page errors.
- [ ] © OpenStreetMap contributors / ODbL visible.

## 5. Android

```powershell
cd android
.\gradlew assembleDebug
```

Device checks:

- [ ] First location use shows the Android permission dialog.
- [ ] Grant → GPS fix appears; deny → visible error, no crash.
- [ ] Background sharing asks for notifications on Android 13+.
- [ ] Map loads from the loopback server; pack download, search and route work.
- [ ] Airplane mode: installed pack map/search/route still work.
- [ ] APK is arm64, WebView 100+, minSdk 26.
- [ ] No demo credentials, pins or seeded accounts.

## 6. Desktop

- [ ] `cargo build --locked --features map-webview`.
- [ ] Map window opens; WebView2/WebKitGTK fallback works.
- [ ] Geolocation permission is requested or clearly reported.
- [ ] Pack download/install, search and route work; no external requests when
      offline.

## 7. Release

- [ ] Bump `Cargo.toml` and `Cargo.lock` to `2.0.0`.
- [ ] Move `[Unreleased]` to `## [2.0.0] - YYYY-MM-DD`; add every feature,
      fix and infra entry.
- [ ] Replace `RELEASE_NOTES.md` with the 2.0.0 notes (pack, offline map,
      search, routing, Android/desktop, known limitations).
- [ ] Update `docs/offline/moldova.md` with the final pack version/hashes.
- [ ] Open the PR against `main` with `Closes`/`Refs`; wait for green CI.
- [ ] Squash merge; tag the merge commit `v2.0.0`.
- [ ] Let the Release workflow build/sign the APKs; publish the draft.
- [ ] Confirm the latest release is 2.0.0 and the data release stays
      `data-moldova-20261116` (not latest).
- [ ] Deploy Render; `/health` reports `2.0.0`.
- [ ] Re-check the proxy and a real pack download from the deployed origin.

## 8. Accounts

- [ ] Register the real user accounts (no seeded/demo accounts).
- [ ] Promote one real account to administrator via the admin API or a direct
      Postgres `UPDATE pinpoint_users SET role = 'admin' WHERE email = ...`.
- [ ] Verify a non-friend cannot read a location; a friend without an active
      share cannot read it; an active share can.
- [ ] Delete a test account and verify login stays `401` after a refresh cycle.
