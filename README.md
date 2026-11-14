# Pinpoint

**A privacy-first location-sharing platform for trusted friends.**

Friendship does not grant access to anyone's location: each person explicitly
chooses who can see them, at what precision, and for how long. Every share can
be revoked, and recipients can see whether a location is fresh or stale.

**Current release: [1.2.1](./RELEASE_NOTES.md)** — thesis demo, not
production-hardened. See the functional-state matrix and per-item
`[FEATURE]` / `[FIX]` flags in the release notes.

---

## What works today

- **Accounts & sessions** — email/password (Argon2id) and anonymous
  device-bound accounts, opaque `HttpOnly` session cookies + CSRF, session
  list/revoke, logout-all, lockout, TOTP MFA.
- **Web UI at `/`** — map-first Leaflet UI (locally vendored), place search,
  A→B routing with alternatives/steps (drive; walk/bike when self-hosted),
  saved/recent places per account, locate-me, plus the auth, publish, friends,
  shares and sessions surfaces in a drawer.
- **Identity & privacy** — every account has a generated public handle
  (`calm-otter-42`); friends add by exact username, pending requests show
  handles only, and real names/emails are revealed to accepted friends.
- **Admin console** — role-gated modules: overview, all accounts (including
  anonymous and never-signed-in), role/active changes, safe purge, PII-free
  diagnostics with triage, and an append-only audit trail. Dedicated services
  run on 8005 (diagnostics) and 8006 (admin API).
- **Desktop client** — map, Nominatim search, OSRM routing (drive/walk/bike),
  Overpass POIs, bookmarks, check-ins, emergency, offline tile packs.
- **Android client** — same Rust core packaged with cargo-apk 0.10; responsive
  map-first shell, live OSM tiles via the backend, kiosk check-in and admin
  screens, drawer navigation. Builds and runs on the API 37 x86_64 emulator;
  AArch64 device/CI and release signing are follow-ups.
- **Consent model** — directional, precision-scoped (`exact` / `approximate` /
  `city_only`), purpose-bound, expiring, revocable; enforced server-side.
- **Privacy** — no trackers, no analytics, no raw IP storage, no IP/WebRTC
  geolocation; local-only web assets (no Google Fonts/CDNs); ODbL attribution
  everywhere.
- **Postgres persistence** — migrations 1–15, verified against live Postgres 16:
  a registered user and published location survived a server restart, and the
  split diagnostics/admin ports shared sessions, reports and audit rows.
- **OSM self-hosting** — `PINPOINT_TILE_URL`, `PINPOINT_NOMINATIM_URL`,
  `PINPOINT_OSRM_URL`, `PINPOINT_OVERPASS_URL`; public providers are demo
  fallbacks only.

**Still pending:** OAuth/passkeys/magic links, iOS build, Android release
signing/CI, and physical AArch64 device validation. See
[`ROADMAP.md`](./ROADMAP.md) and [`RELEASE_NOTES.md`](./RELEASE_NOTES.md).

---

## Quick start

### Web (single binary)

```bash
cargo run --bin pinpoint-server
# → http://127.0.0.1:8000/  (in-memory demo store, seeded Alice/Bob)
# demo login: alice@example.com / DemoPass123!
```

If port 8000 is taken (Docker), use `BIND_ADDR=127.0.0.1:8080`.

### Desktop

```bash
cargo run -- --backend http://127.0.0.1:8000
```

### Android (Rust APK)

```powershell
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:ANDROID_NDK_HOME = "$env:ANDROID_HOME\ndk\27.2.12479018"
$env:JAVA_HOME = "C:\Program Files\Eclipse Adoptium\jdk-25.0.5.8-hotspot"
cargo apk build --lib --target x86_64-linux-android
adb install -r -t target/debug/apk/pinpoint.apk
```

The Android emulator reaches the host at `http://10.0.2.2:8080`; run
`BIND_ADDR=0.0.0.0:8080 cargo run --bin pinpoint-server`. Use Android Studio
for the emulator/logcat; the APK itself is built by cargo-apk, not Gradle. See
[`android/README.md`](./android/README.md) for prerequisites and caveats.

### Docker

```bash
docker compose up --build -d
# gateway → http://127.0.0.1:8000/  · diagnostics :8005 · admin API :8006
# Alice/Bob as real headless clients:
docker compose --profile demo up -d alice bob
```

`pinpoint-demo-client` signs in through `/api/v1/auth/login` and publishes
locations through `/api/v1/locations/me`; it is not a server-side seed. The
internal gateway name is allowed to use plaintext HTTP only through the
explicit `PINPOINT_ALLOW_PLAINTEXT_HOSTS=gateway` opt-in (exact hostnames,
no wildcard); HTTPS remains the default everywhere else.

### Release artifact

Unzip `dist/pinpoint-1.2.1-windows-x64.zip`, run `pinpoint-server.exe`, open
`http://127.0.0.1:8080/`.

---

## Architecture

One Rust crate (`pinpoint-desktop`): portable core in `src/lib.rs`, desktop
entry in `src/main.rs`, Axum API in `src/server/` (auth/users/locations/map
routers behind a gateway), OSM clients in `src/osm.rs`, platform code in
`src/platform.rs`. Desktop, Android, iOS and a WASM stub share the core.

- [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) — current module map
- [`docs/adr/`](./docs/adr/) — decisions (ADR-0007…0012)
- [`docs/SECURITY.md`](./docs/SECURITY.md) — security model and gaps
- [`docs/PRIVACY.md`](./docs/PRIVACY.md) — GDPR posture
- [`docs/reference/api-contract.md`](./docs/reference/api-contract.md) — live API

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
