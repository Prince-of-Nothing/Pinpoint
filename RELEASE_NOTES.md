# Pinpoint 2.7.0 - Release Notes

**Released:** 2026-11-25 - **Version:** `2.7.0` (from `2.6.1`) -
**Status:** Map tools, GitHub OAuth and hardened responses.

**Artifacts:** `pinpoint-android-2.7.0.apk` (`arm64-v8a`, minSdk 26, targetSdk 35)
and the source archives for the `v2.7.0` tag. The debug APK keeps
`allowBackup=0`, `usesCleartextTraffic=0` and the network security config.

## What is new

### Map

- **Desktop web shell:** the section cards are a persistent 380 px right
  column; search moves to the top-left and results open under it. The map and
  its chrome shrink beside the column instead of hiding beneath it.
- **Map controls:** compass/pitch and fullscreen on the web map; mobile zoom
  and fullscreen controls stay above the search and tab bars.
- **Right-click / long-press actions** on web and native: directions from/to
  here, measure from here, add note here, copy coordinates.
- **Distance measure:** numbered points, live total, undo and clear on both
  clients. Pure haversine math in `web/app/measure.js` and
  `models::path_length_m`.
- **Friend labels:** web pins now show the friend's name above the circle
  using DOM markers, so labels survive the glyph-less offline style.
- **Freshness:** friend locations say `updated just now`, `5m ago`, `2d ago`
  on web and native.

### Auth and security

- **GitHub OAuth 2.0** sign-in alongside Google: authorization code + PKCE,
  single-use state, verified email, no automatic email linking, same opaque
  session. Client secrets are stored apart from endpoint URLs.
- `Cache-Control: no-store` on every `/api/*` response and on map search,
  reverse and route answers; tiles keep their public one-hour cache.
- Every 429 carries `Retry-After`; split service binaries apply the same
  baseline headers as the combined router.
- CodeQL (Rust), weekly ZAP baseline and Dependabot remain green; the split
  client secret refactor removed the long-standing code-scanning false
  positive on the guarded OAuth userinfo URL.

### Docs and evidence

- Single evidence index: `docs/evidence/README.md`.
- Refreshed Midterm 2 screenshots for the desktop/mobile map, Today and
  private notes.
- API contract now documents response headers, cache rules and throttling.

## Verification

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo build --locked`, `cargo test --locked`: pass, **381 lib tests** plus
  the router/contract/OAuth integration suites.
- `scripts/smoke.ps1`: **29 passed, 0 failed**.
- Web checks: sharing 20, admin 7, navigation, messages-poll,
  password-length and OAuth.
- CI: rust, CodeQL, web-tests, wasm, macOS/Windows clients and the ZAP
  baseline. Vercel preview builds are rate-limited on the free plan.
- APK rebuilt from the release commit; `aapt` reports
  `com.pinpoint.desktop` 2.7.0 (20700), `arm64-v8a`, minSdk 26 / target 35,
  `allowBackup=0`, `usesCleartextTraffic=0` and the network security config.
