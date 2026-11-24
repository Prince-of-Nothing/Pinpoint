# Technology Stack and Infrastructure

The stack is chosen for one property above all: **one Rust codebase** can serve
the desktop app, the Android app, the iOS target and the API, while the web
client stays a thin same-origin page. That removes the duplicated validation
and crypto logic that a split stack would create, and keeps the security
boundary in code the tests can drive directly.

## Clients

| Client | Stack | Notes |
|---|---|---|
| Desktop | Rust + `eframe`/`egui` 0.32 (glow) | Windows/Linux/macOS; the same UI code is the shared `src/ui/` shell |
| Android | Rust + `eframe` native activity, a small Java bridge for platform APIs | ADR-0013/0017; the APK is assembled by `android/gradlew`, not by `cargo-apk` |
| iOS | Rust + `eframe`, built with `cargo-xcodebuild` | Target exists; unverified in CI (no Xcode) |
| Web | Vanilla ES modules, MapLibre GL JS 6.x, PMTiles 4.x, service worker | No framework, no CDN, no third-party scripts; assets are embedded in the server binary |

## Server

| Concern | Choice | Why |
|---|---|---|
| HTTP | `axum` 0.7 on `tokio` 1 | Small, type-safe extractors; the same router builds the combined and split binaries |
| Persistence | `sqlx` 0.8 + PostgreSQL 16 | Compile-checked queries, bound parameters only, migrations in `src/server/migrations.rs` |
| Sessions | Opaque server-side sessions (ADR-0012) | No bearer JWT to protect or refresh; revocation is a row lookup |
| Passwords | `argon2` 0.6 (Argon2id PHC) | Memory-hard; legacy PBKDF2 hashes rehash on login |
| MFA | TOTP (`totp-rs`) with replay protection | No SMS/email dependency |
| OAuth | Google OIDC + GitHub, authorization code + PKCE (ADR-0024) | Verified email, no auto-linking |
| E2E messages | `x25519-dalek` 2, `ed25519-dalek` 2, `hkdf` 0.12, `aes-gcm` 0.11 | ECIES, ciphertext-only server (ADR-0023) |
| Notes at rest | `aes-gcm` 0.11 + `DATA_ENCRYPTION_KEY` | Column encryption before storage (ADR-0025) |
| HTTP client | `reqwest` 0.12 with rustls | No OpenSSL on the client path; HTTPS-only policy (ADR-0006) |
| Logging | `tracing`/`tracing-subscriber` | Redacted fields; diagnostics stay local |
| Time | `chrono` 0.4 | ISO-8601 timestamps across the wire |

Split deployables (`src/bin/`): `pinpoint-server` (combined), `auth`, `users`,
`locations`, `messages`, `map`, `diag`, `admin`, plus the web/wasm stubs. They
share the `AppState` and routers, so the demo and the service-per-container
deployment run the same handler logic.

## Data

- PostgreSQL tables are prefixed `pinpoint_` (`users`, `friendships`, `shares`,
  `locations`, `location_history`, `place_notes`, `checkins`, `emergencies`,
  `sessions`, `messages`, `message_keys`, `diagnostic_reports`, `admin_audit`).
- Messages are ciphertext only; note label/message/photo are AES-256-GCM
  encrypted when the key is configured; passwords are Argon2id hashes.
- Backups: the compose stack writes `pg_dump` every five minutes;
  `scripts/backup-db.ps1` produces an encrypted, verified off-host copy and
  `scripts/restore-db.ps1` restores it (see [deployment](../DEPLOYMENT.md)).

## Maps and external services

- MapLibre GL JS renders vector tiles; PMTiles packs provide offline maps.
- The online basemap is opt-in OpenFreeMap; the default is the installed
  offline pack, so the app opens without a third-party request.
- Self-hosted OSM services are the production path (Nominatim, OSRM,
  Overpass, a tile server); the public instances are demo fallbacks only and
  are rate-limited by the proxy (ADR-0011, ADR-0021, `docs/OSM_POLICY.md`).
- Attribution (OSM/ODbL and OpenFreeMap) is always rendered.

## Infrastructure

- `docker-compose.yml`: gateway, replicated services, PostgreSQL, `db-backup`;
  `docker-compose.https.yml` adds a self-signed TLS gateway for local checks.
- CI (`.github/workflows/`): Rust gate (fmt, clippy, build, test, cargo audit),
  web Node checks, wasm compile, macOS/Windows client builds, CodeQL, weekly
  ZAP baseline, OSM compliance.
- Secrets come from the environment, never the repository: `DATABASE_URL`,
  `DATA_ENCRYPTION_KEY`, `PINPOINT_OAUTH_*`, `PINPOINT_BACKUP_PASSPHRASE`.

## Why not the alternatives

| Alternative | Reason not chosen |
|---|---|
| Flutter/Kotlin mobile + separate backend | Duplicates crypto and validation; the thesis needs one auditable security core |
| FastAPI/SQLAlchemy | The historical prototype; removed from the tree (ADR-0007) |
| JWT + refresh tokens | Adds a second credential and refresh loop without a revocation gain for a same-origin client (ADR-0012) |
| Hosted map SDK with a key | Third-party script/key in the privacy boundary; MapLibre + PMTiles keeps it same-origin |
| WebSocket realtime | Polling (5 s web inbox, 30 s native) is enough for the MVP and simpler to secure; realtime is future work |

All Rust versions are pinned in `Cargo.lock`; the web libraries are vendored
under `web/vendor/`.
