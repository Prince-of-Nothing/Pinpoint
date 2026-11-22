# Pinpoint

**A privacy-first location-sharing platform for trusted friends.**

Friendship does not grant access to anyone's location: each person explicitly
chooses who can see them, at what precision, and for how long. Every share can
be revoked, and recipients can see whether a location is fresh or stale.

**Current release: [1.2.1](https://github.com/Prince-of-Nothing/Pinpoint/blob/main/RELEASE_NOTES.md)** — thesis demo, not
production-hardened. See the functional-state matrix and per-item
`[FEATURE]` / `[FIX]` flags in the release notes.

---

## What works today

- **Accounts & sessions** — email/password (Argon2id) and anonymous
  device-bound accounts, opaque `HttpOnly` session cookies + CSRF, session
  list/revoke, logout-all, lockout, TOTP MFA.
- **E2E messages** — 1:1 text between accepted friends; X25519 + Ed25519 +
  AES-256-GCM with a ciphertext-only server, safety number and read receipts on
  web and native. ECIES, not the Signal Protocol; see
  [`docs/messaging.md`](docs/messaging.md).
- **Web UI at `/`** — map-first MapLibre GL JS + PMTiles UI (locally
  vendored), place search,
  A→B routing with alternatives/steps (drive; walk/bike when self-hosted),
  saved/recent places per account, locate-me, plus the auth, publish, friends,
  shares and sessions surfaces in a drawer.
- **Identity & privacy** — every account has a generated public handle
  (`calm-otter-42`); friends add by exact username, pending requests show
  handles only, and real names/emails are revealed to accepted friends.
- **Admin console** — role-gated modules: overview, all accounts (including
  anonymous and never-signed-in), role/active changes, safe purge, PII-free
  diagnostics with triage, and an append-only audit trail. The compose stack
  runs auth, users, locations, messages and map services behind a gateway;
  diagnostics and the admin API use dedicated ports 8005 and 8006.
- **Desktop client** — map, Nominatim search, OSRM routing (drive/walk/bike),
  Overpass POIs, bookmarks, check-ins, emergency, offline tile packs.
- **Android client** - same Rust core packaged with Gradle + cargo-apk; responsive
  map-first shell, live OSM tiles via the backend, kiosk check-in, admin and
  tool screens, share-in via a small Java `onNewIntent` bridge, optional
  biometric unlock and a user-started foreground location service. Local crash
  reports, a rolling log and About → Troubleshooting make failures reportable
  without a server.
  Builds and runs on the API 37 x86_64 emulator; tag-driven signed arm64/armv7
  APKs come from the release workflow, and a physical AArch64 device run is
  still pending.
- **Consent model** — directional, precision-scoped (`exact` / `approximate` /
  `city_only`), purpose-bound, expiring, revocable; enforced server-side.
- **Privacy** — no trackers, no analytics, no raw IP storage, no IP/WebRTC
  geolocation; local-only web assets (no Google Fonts/CDNs); ODbL attribution
  everywhere. The only automatic non-Pinpoint request is a once-a-day,
  opt-out GitHub release check (ADR-0015).
- **Postgres persistence** - migrations 1-17, verified against live Postgres 16:
  a registered user and published location survived a server restart, and the
  split diagnostics/admin ports shared sessions, reports and audit rows.
- **OSM self-hosting** — `PINPOINT_TILE_URL`, `PINPOINT_NOMINATIM_URL`,
  `PINPOINT_OSRM_URL`, `PINPOINT_OVERPASS_URL`; public providers are demo
  fallbacks only.

**Still pending:** OAuth/passkeys/magic links, iOS build, and physical AArch64
device validation. See the [roadmap](https://github.com/Prince-of-Nothing/Pinpoint/wiki/Roadmap) and
[`RELEASE_NOTES.md`](https://github.com/Prince-of-Nothing/Pinpoint/blob/main/RELEASE_NOTES.md).

---

## Quick start

### Web (single binary)

```bash
cargo run --bin pinpoint-server
http://127.0.0.1:8000/  (in-memory store; register an account)
# → http://127.0.0.1:8000/status  (service + integration status, ADR-0016)
```

If port 8000 is taken (Docker), use `BIND_ADDR=127.0.0.1:8080`.

### Desktop

```bash
cargo run -- --backend http://127.0.0.1:8000
```

The shared MapLibre page (`web/map/`) can open in a separate native WebView
window from **Map → MapLibre map window → Open map window**. It is opt-in:

```bash
cargo run --features map-webview -- --backend http://127.0.0.1:8000
# or run the window directly (starts a loopback server on an ephemeral port)
cargo run --features map-webview -- --map-window
```

The window loads `http://127.0.0.1:<port>/map/index.html` from a Rust axum
server with HTTP Range support, so PMTiles works. WebView2 is required on
Windows, WebKitGTK on Linux and WKWebView on macOS. `wry`/`tao` need their own
event loop on the main thread, which eframe already owns, so the window runs in
a child process of the same binary. If the platform WebView runtime is missing,
the launcher opens the same loopback URL in the system browser and keeps the
server alive while Pinpoint runs. The egui map remains the fallback; auth,
friends and privacy behavior are unchanged. `web/` is resolved next to the
executable, from the current directory, or through `PINPOINT_WEB_ROOT`, so a
packaged build must ship the `web/` tree.

### Android (Rust APK)

```powershell
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:ANDROID_NDK_HOME = "$env:ANDROID_HOME\ndk\27.2.12479018"
$env:JAVA_HOME = "C:\Program Files\Eclipse Adoptium\jdk-21.0.13+9"
cd android
.\gradlew.bat :app:assembleDebug -Pabi=x86_64
adb install -r -d -t app\build\outputs\apk\debug\app-debug.apk
adb shell am start -n com.pinpoint.desktop/.PinpointActivity
```

The Android emulator reaches the host at `http://10.0.2.2:8080`; run
`BIND_ADDR=0.0.0.0:8080 cargo run --bin pinpoint-server`. Use Android Studio
for the emulator/logcat; Gradle packages the Rust `.so`, Java bridge and APK. See
[`android/README.md`](https://github.com/Prince-of-Nothing/Pinpoint/blob/main/android/README.md) for prerequisites and caveats.

Pushing a `vX.Y.Z` tag that matches `Cargo.toml` runs
[`.github/workflows/release.yml`](https://github.com/Prince-of-Nothing/Pinpoint/blob/main/.github/workflows/release.yml): it builds
one signed universal APK (`arm64-v8a` + `armeabi-v7a`), verifies the release
certificate and creates a draft GitHub release. Publish the draft so the in-app
About → Updates checker can see the notes and the direct APK link. Manual
workflow runs are dry runs. See [`android/README.md`](https://github.com/Prince-of-Nothing/Pinpoint/blob/main/android/README.md) for
the key secrets and caveats.

### Docker

```bash
docker compose up --build -d
# gateway → http://127.0.0.1:8000/  · diagnostics :8005 · admin API :8006 · messages :8007
# Alice/Bob as real headless clients:
docker compose --profile demo up -d alice bob
```

`pinpoint-demo-client` signs in through `/api/v1/auth/login` and publishes
locations through `/api/v1/locations/me`; it is not a server-side seed. The
internal gateway name is allowed to use plaintext HTTP only through the
explicit `PINPOINT_ALLOW_PLAINTEXT_HOSTS=gateway` opt-in (exact hostnames,
no wildcard); HTTPS remains the default everywhere else.

#### Docker disk maintenance

Rust builds accumulate BuildKit cache quickly (`docker system df`). Run an
Administrator PowerShell from the repository root to prune cache beyond 5 GB
and compact Docker Desktop's WSL disk:

```powershell
./scripts/docker-maintenance.ps1
```

The script keeps images, containers and volumes. Windows does not shrink
`docker_data.vhdx` automatically, so pruning alone does not return the space
to the host until the disk is compacted.

### Release artifact

Unzip `dist/pinpoint-1.2.1-windows-x64.zip`, run `pinpoint-server.exe`, open
`http://127.0.0.1:8080/`.

---

## Architecture

One Rust crate (`pinpoint-desktop`): portable core in `src/lib.rs`, desktop
entry in `src/main.rs`, Axum API in `src/server/` (auth/users/locations/map
routers behind a gateway), OSM clients in `src/osm.rs`, platform code in
`src/platform.rs`. Desktop, Android, iOS and a WASM stub share the core.

- [`docs/ARCHITECTURE.md`](https://github.com/Prince-of-Nothing/Pinpoint/blob/main/docs/ARCHITECTURE.md) - current module map
- [`docs/messaging.md`](https://github.com/Prince-of-Nothing/Pinpoint/blob/main/docs/messaging.md) - E2E message protocol, limits and tests
- [`docs/adr/`](https://github.com/Prince-of-Nothing/Pinpoint/blob/main/docs/adr/README.md) - decisions (ADR-0007…0012)
- [`docs/SECURITY.md`](https://github.com/Prince-of-Nothing/Pinpoint/blob/main/docs/SECURITY.md) — security model and gaps
- [`docs/PRIVACY.md`](https://github.com/Prince-of-Nothing/Pinpoint/blob/main/docs/PRIVACY.md) — GDPR posture
- [`docs/reference/api-contract.md`](https://github.com/Prince-of-Nothing/Pinpoint/blob/main/docs/reference/api-contract.md) — live API

---

## Quality gates

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked && cargo test --locked
```

CI adds `cargo-audit`, map QA, and the flagged release artifact job.

---

## License

MIT © 2026 Pinpoint. Map data © OpenStreetMap contributors, ODbL.

## Contributing

Small PRs, conventional commits, docs in the same PR. Security changes need a
regression test and a fail-closed test. Commits are authored by the repository
owner only — no co-author trailers (see `CONTRIBUTING.md`).
