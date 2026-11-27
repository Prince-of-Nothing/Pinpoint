# Pinpoint 3.0.1 - Release Notes

**Version:** `3.0.1` (from `3.0.0`) - **Status:** Patch release for the unified
desktop, mobile and web client. It has no database migration.

**Artifacts:** `pinpoint-android-3.0.1.apk` (signed universal: `arm64-v8a` +
`armeabi-v7a`, minSdk 26, targetSdk 35), `pinpoint-desktop-linux-x64-3.0.1.zip`
(binary + `pinpoint.desktop` + icon), `pinpoint-desktop-macos-3.0.1.zip`
(unsigned `Pinpoint.app`), `pinpoint-desktop-windows-x64-3.0.1.zip`,
`pinpoint-server-linux-x64-3.0.1.zip`, `pinpoint-server-macos-3.0.1.zip` (arm64)
and `pinpoint-server-windows-x64-3.0.1.zip`, plus `SHA256SUMS.txt` with the
SHA-256 of every file above. Check a download with
`sha256sum -c SHA256SUMS.txt` in the folder that holds the files.

The server zips **are** the web bundle: the web client is embedded, so run
`pinpoint-server` (or `pinpoint-server.exe`) and open `http://127.0.0.1:8080/`.
iOS reports `CFBundleShortVersionString` 3.0.1 / build 30001. The APK keeps
`allowBackup=0`, `usesCleartextTraffic=0` and the network security config.

## What changed since 3.0.0

**Web**

- Four tabs (Map, People, Today, Chats). Account moves to the avatar menu.
  Signed in, the header gives way to one command bar on the map.
- A "Sharing with you" strip lists the people whose location you can see now,
  freshest first. A location updated in the last 15 minutes is marked live.
- Place search tries the installed offline map, then the Pinpoint server, then
  data already on the device. When the server is unreachable, search still
  answers and says so. A failed search shows its reason and a Retry button.
- Account, then Server, shows which server you are on and can switch to another
  one after a confirmation. Only https is accepted, or plain http for this
  computer.
- Three optional online map layers (Streets, Satellite, Terrain) stay off until
  you turn them on. The privacy page names each provider.
- Keyboard and focus: `/` focuses search, arrow keys move through results, and
  drawers and sheets keep focus inside until they close.
- Visual identity: limestone surfaces, Codri-green ink and buttons, and
  Sunflower reserved for live locations.

**Native (Android and desktop)**

- Four bottom tabs in a floating bar. Chats shows an unread count.
- Map controls use line icons. Open sheets block input to the map behind them.
- Friends show what each one shares and how old it is. Stale pins are dimmed.
- The safety number moves behind an info button in chats.
- The dark palette matches the web client.

**Fixes and hardening**

- Latitude and longitude are range-checked before a saved place is stored.
- Escape, drawer and share handoffs return focus where it belongs.
- Random keys, salts and nonces are written straight into typed arrays, and
  fixed-size keys are checked with `try_from` instead of zero-filled buffers.

**Release process**

- The tag workflow writes `SHA256SUMS.txt` for every asset, checks it, and
  attaches it to the draft release.

The web service worker cache is now `pinpoint-offline-v11`, so installed web
clients refresh their shell once.

## Compatibility

No migration was added: `src/server/migrations.rs` and `db/` are unchanged since
3.0.0. The API v1 contract (`tests/contract.rs`) is unchanged, and cookies,
sessions, encrypted notes and ECIES message envelopes keep their formats.

## Verification

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo build --locked` and `cargo test --locked` pass: **572 tests passed,
  0 failed, 3 ignored** (the ignored ones need a running server).
- Version fields agree: `Cargo.toml` and `Cargo.lock` say `3.0.1`; Android
  derives versionName `3.0.1` and versionCode `30001` from `Cargo.toml`; iOS
  reports `3.0.1` / `30001`; the privacy page names `3.0.1`; the service worker
  cache is `pinpoint-offline-v11`.
- Release dry run: DRY_RUN_RESULT_PLACEHOLDER
