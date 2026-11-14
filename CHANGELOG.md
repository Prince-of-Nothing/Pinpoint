# Changelog

All notable changes to this project are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning follows
Semantic Versioning. 1.0.0 is the thesis demo release; production hardening is
tracked in [`ROADMAP.md`](./ROADMAP.md).

Release flags: `[FEATURE]` new capability · `[FIX]` bug fix ·
`[SECURITY]` hardening · `[DOCS]` documentation · `[INFRA]` build/deploy.
The per-item flagged list for this release is in
[`RELEASE_NOTES.md`](./RELEASE_NOTES.md).

## [Unreleased]

## [1.3.6] - 2026-11-14
### Removed
- [FIX] Android: remove the `ACTION_SEND` text-share target. Repeat shares were silently ignored while Pinpoint was alive because Android delivers them to `onNewIntent`, which the Rust-only `NativeActivity` cannot observe. Share-out (places/kiosk), `pinpoint://` and `geo:` deep links remain. `launchMode` stays `singleTop`; `standard` caused android-activity lifecycle errors and elevated threads. See ADR-0014.

## [1.3.5] - 2026-11-14
### Added
- [FEATURE] Android: `pinpoint://publish` (also `publish-gps`/`share-location`) publishes the current location on launch, waiting for a restored session before it does.
- [FEATURE] Android: `Use GPS` now actively requests a location when no provider has a recent last-known fix (8 s window, GPS → fused → network), then removes the request.

### Fixed
- [FIX] GPS: the publish action re-checks the fix after the active request and reports success in the device log; verified on the API 37 emulator against a local backend (`POST /api/v1/locations/me`).
- [FIX] Android debug builds can opt into screenshots for UI automation via `files/allow_screenshots`; release builds ignore the marker and keep FLAG_SECURE.

## [1.3.4] - 2026-11-14
### Added
- [FEATURE] Android: one-shot GPS. **Use GPS** checks the runtime location permission, asks for it when missing, then publishes the last known GPS or network fix (approximate grants included).
- [FEATURE] Android: share intents. Map links and coordinates shared from Maps, OSM, Apple Maps or chat apps open in Pinpoint; place and kiosk links gain a **Share** action that opens the system share sheet (desktop copies the link).
- [FEATURE] Privacy: optional on-device PIN app lock (Argon2id hash only). It locks on launch and on returning from the background, with a lockout after repeated wrong PINs.
- [FEATURE] Map: pinch-to-zoom on touch screens, accumulated into whole zoom levels.
- [FEATURE] Offline: the current-area pack now caches three zoom levels (out, current, in) instead of one, so the map stays usable when zooming offline.

### Fixed
- [FIX] Android: JNI now targets the live `NativeActivity`, not the Application context, so FLAG_SECURE, launch deep links, runtime permission requests and the share sheet actually run. Shared `ACTION_SEND` text opens the linked location.
- [FIX] Share/deep links: a linked coordinate now survives the first map render (zoom is initialized), so the map actually opens at the shared place. Repeat shares while Pinpoint is already running remain a documented Rust-only limitation.
- [FIX] GPS: only fixes newer than 10 minutes and with accuracy ≤5 km are published; providers are tried gps → fused → network → passive and one failing provider no longer hides the rest.
- [FIX] Lock: an unreadable `lock.json` fails closed, PIN changes use an atomic temp-file write with fsync, and existing Unix lock files are tightened to 0600.
- [FIX] Share parser: accepts map URLs surrounded by prose/trailing punctuation and `destination=` links; shared text can only open locations/kiosks, never `pinpoint://admin`.
- [FIX] Pinch zoom no longer pans with the first finger and no longer builds a zoom backlog at the 2/18 limits.

## [1.3.3] - 2026-11-14
### Added
- [FEATURE] Android: register and route `pinpoint://` and `geo:` launch intents, including kiosk token links and map-centre hand-off.
- [FEATURE] Kiosk: render a scannable QR for `pinpoint://kiosk?token=...` links.

### Security
- [SECURITY] Android: FLAG_SECURE blocks screenshots and screen recording of location data.
- [SECURITY] A release with no configured backend disables session restore and sign-in instead of sending credentials or a persisted session to the shared loopback port.

### Fixed
- [FIX] Android: report and log copies use the system `ClipboardManager`, so the no-email fallback leaves the full report on the real clipboard.
- [FIX] Android: reserve a bottom inset so the edge-to-edge gesture bar does not cover the mobile tab bar.
- [FIX] Diagnostics: redact Authorization/Bearer headers, JSON values, suffixed tokens and quoted coordinates; mailto re-redacts before encoding.
- [FIX] Logs: Clear log truncates the open writer, so subsequent events remain visible.
- [FIX] Crash prompt is marked seen after Email or Close, so it does not repeat on every launch.

## [1.3.2] - 2026-11-14
### Added
- [FEATURE] Crash reporting: Rust panics and `eframe::run_native` startup errors write a redacted app-private report; the next launch shows an editable dialog with Email/Copy/Discard/Close, and About/Profile expose a manual Report a problem flow.
- [FEATURE] Rolling native log under the app data directory, with 1 MiB rotation, copy and clear actions.
- [FEATURE] Troubleshooting card in About with device/page-size summary, log path, pending report access and an Offline badge on mobile.

### Fixed
- [FIX] Android release: a rejected default backend URL now opens the app with the loopback offline fallback and a visible Offline badge instead of panicking before the UI.
- [FIX] Updates: opening About no longer contacts GitHub automatically; the release check runs only when the user presses Check for updates.
- [FIX] WASM: keep the web stub compiling by stubbing the native URL opener.

## [1.3.1] - 2026-11-14
### Fixed
- [FIX] Android: align the arm64 native library to 16 KB so the APK loads on Android 15+ devices configured with 16 KB pages.

## [1.3.0] - 2026-11-14
### Added
- [FEATURE] Native map proxy: search, reverse geocoding and routing now call the Pinpoint backend with map-centre bias, stops, avoid classes, alternatives and capability gating.
- [FEATURE] Native route planner: place-based A/B, stops, avoid classes, swap, car/walk/bike, alternatives, step list, route sharing and KML/GeoJSON export.
- [FEATURE] Per-friend sharing sheet: existing-share lookup, enable/pause, precision, duration presets, incoming status and revoke; accepted friends are listed with Sharing/Remove actions.
- [FEATURE] Native route following: locate/self marker, 1 Hz fix polling, next-stop advancement and exit navigation.
- [FIX] Mobile locate no longer publishes a location; the publish sheet is reachable from the drawer and the drawer now has a refresh action.
- [FEATURE] Account lifecycle: anonymous sign-in, recovery code/recovery, keychain-backed session resume, active-session list/revoke and sign out everywhere.
- [FEATURE] Profile utilities: PII-free diagnostics report and map label/icon size sliders.
- [FEATURE] Admin console parity: overview counters, user search/role/active/delete, audit log and diagnostics triage; accepts `admin` and `administrator` roles.
- [FEATURE] ArtCraft-inspired mobile Tools sheet: colored tool tiles for map, friends, sharing, saved, trips, emergency, profile, updates, admin, kiosk and password, plus publish/refresh actions.
- [FEATURE] About & updates screen: version/build identity, user-initiated GitHub release check, semver comparison, release notes and release-page hand-off.
- [INFRA] Release workflow: `v*` tags build signed arm64/armv7 APKs, verify their certificates and create a GitHub release with the source archives.
- [FEATURE] mobile: Port webapp Kiosk check-in mode & Admin console to Android app and eframe UI, with deep links `pinpoint://kiosk?token=...` and `pinpoint://admin`.
- [FEATURE] Android APK: cargo-apk 0.10 build with NativeActivity/`android_main`, eframe `android-native-activity`, API 35 manifest metadata and launcher icons; runs on the API 37 x86_64 emulator.
- [FEATURE] Mobile map-first shell: web-loyal full-bleed dark map with floating search, zoom/locate/menu controls, bottom drawer, publish/place sheets, touch-sized controls, compact login/register forms, live tiles and scrollable locations.
- [FEATURE] Mobile map app polish: clean light OSM map by default with a light/dark toggle, category chips, bottom tab bar (Map/Friends/Share/Saved/Me), and a route polyline with A/B markers.
- [FEATURE] On-device tile cache/prefetch through `/map/tiles` under the Android app data dir.

### Changed
- [INFRA] Repository cleanup: untrack the archived MSVC/Windows SDK, downloaded installers, Go toolchain and `archive/app/target`; `.gitignore` now covers them plus Android signing material.

### Fixed
- [FIX] Auth: native login accepts an optional TOTP code, so MFA-enabled accounts can sign in instead of always receiving `MFA_REQUIRED`.
- [FIX] Admin navigation is hidden for residents and shown for `admin`/`administrator` only.
- [FIX] Mobile route sheet sits above the bottom tab bar and no longer clips its actions.
- [FIX] GitHub update check forces IPv4, matching the tile proxy, so it works on Android emulators with a broken IPv6 path.
- [FIX] CI: scope sqlx to Postgres-only (removing unused SQLite/MySQL advisories), refresh the lockfile to mio 1.2.5, and keep axum/sqlx/tokio out of the wasm stub build.
- [FIX] Tiles: force IPv4 for proxy/download requests and avoid the Docker gateway on 8000 by using `10.0.2.2:8080` for Android debug builds.

## [1.2.1] - 2026-11-14
### Security
- [SECURITY] **ZAP baseline hardening**: `Permissions-Policy`,
  `Cross-Origin-Embedder-Policy`, `Cross-Origin-Opener-Policy` and
  `Cross-Origin-Resource-Policy` are set on every web response. Re-scan is
  0 high / 0 medium / 0 low; raw evidence in
  `docs/evidence/zap-baseline.*` and the summary in
  `docs/evidence/zap-baseline-1.2.1.md`.
- [SECURITY] Static JS/CSS now use `Cache-Control: no-cache` (revalidate)
  while HTML keeps `no-store`.

### Fixed
- [FIX] OS-keychain tests skip cleanly when Windows returns
  `ERROR_NO_SUCH_LOGON_SESSION` (headless/sessionless runs) instead of
  failing the suite.

## [1.2.0] - 2026-11-13
### Added
- [FEATURE] **Headless Alice/Bob demo clients**: `pinpoint-demo-client` signs in
  through the API and publishes locations on an interval, so the demo runs the
  two accounts as real client containers (`docker compose --profile demo up
  alice bob`) instead of server-side seed rows. Adds an explicit exact-host
  `PINPOINT_ALLOW_PLAINTEXT_HOSTS` opt-in for internal container DNS; HTTPS
  remains the default.
- [FEATURE] **Diagnostics and audit split ports**: `pinpoint-diag` on 8005
  ingests PII-free, user-initiated reports (category + message + public
  handle only), and `pinpoint-admin` on 8006 serves the role-gated admin API.
  Migration 15 adds `pinpoint_diagnostic_reports`, append-only
  `pinpoint_admin_audit` and `users.created_at`; the admin console now has
  real Diagnostics (triage) and Audit modules instead of placeholders.
- [FEATURE] **Generated public usernames and private profiles**: every account
  gets a stable handle such as `calm-otter-42`; friends add each other by exact
  username, pending requests show handles only, and real names/emails are
  revealed to accepted friends. Numeric user IDs are no longer walkable and
  free-text user search is admin-only.

### Fixed
- [FIX] **DB bootstrap on Windows checkouts**: `db/init/01-roles.sh` is forced
  to LF (`*.sh text eol=lf`), so the container entrypoint no longer fails with
  `set: illegal option -` on a fresh volume.
- [FIX] **Real timestamps on publish**: location updates, emergency alerts,
  check-ins and friend requests use the current UTC clock instead of the seed
  date; friends see a truthful `updated_at`.
- [FIX] Duplicate friend requests are rejected; unknown/pending profiles no
  longer leak email or legal names.

### Infrastructure
- [INFRA] **Postgres verified**: migrations 1–15 applied against live Postgres
  16; a new user and location survived a restart; 8005 ingestion and 8006
  triage shared sessions and the report/audit tables.
- [INFRA] New binaries: `pinpoint-diag`, `pinpoint-admin`,
  `pinpoint-demo-client`; compose adds diag/admin services, gateway routes and
  the `demo` profile.

## [1.1.0] - 2026-11-13
### Added
- [FEATURE] **Export and hand-off to other maps**: place/route links for
  Google Maps and Organic Maps, Magic Earth `magicearth://` links
  (destination-only; multi-stop falls back to KML), plus KML and GeoJSON
  exports alongside GPX. All are user-initiated; no third-party scripts or
  tracking.
- [FEATURE] **Saved-place filters**: category chips derived from saved places
  (All / cafe / pharmacy / …) with a category badge per row.
- [FEATURE] **Per-account route history**: the last 20 routes (origin,
  destination, stops, mode, distance/time) are stored on-device under
  `pinpoint.v1.<userId>.routes`; click to reopen, clear separately from
  places. Route result lists use a delegated click handler so a fresh search
  result cannot lose its listener.
- [FEATURE] **Route panel drag/collapse**: drag the handle to reposition on
  desktop, or collapse to a compact summary; the map stays visible.
- [FEATURE] **Live next-stop ETA**: during Navigate, the next stop advances
  when within 60 m, completed nodes mark as done, and the remaining
  distance/time is recalculated from the current fix.
- [FEATURE] **Map appearance settings**: label size (10–18 px) and icon size
  (18–40 px) sliders, persisted on the device; pins, labels, tooltips and
  route markers follow the setting.
- [FEATURE] **Route progress and next-stop ETA**: A → numbered stops → B
  progress bar, next stop highlighted, and per-leg distance/time from OSRM
  (`legs` in the route JSON); route sharing copies a summary or uses the Web
  Share API, with GPX export as the fallback.
- [FEATURE] **Route options**: multi-stop routing (add stops by clicking the
  map, numbered markers, remove), GPX 1.1 export from the route panel, and
  avoid-motorway/toll/ferry controls. Avoid needs a self-hosted OSRM profile;
  `/map/capabilities` reports `avoid` and the UI disables the controls when
  the public demo cannot honour them.
- [FEATURE] **Per-friend sharing controls**: every accepted friend gets a
  “Sharing” sheet with an on/off switch, precision (`exact` / `approximate` /
  `city_only`) and duration presets (1 h, 8 h, 24 h, 7 days, until turned
  off); creating is an upsert per friendship+direction, and “Stop sharing”
  takes effect immediately.
- [FEATURE] **Modular admin console**: role-gated Admin tab in the web client
  with a module registry (Overview, Users, Diagnostics/Audit placeholders),
  `/api/v1/admin/overview|users|users/:id`, safe purge that revokes sessions
  and cascades friendships/shares/locations/check-ins/emergencies, and
  self-demotion/self-deletion guards. Extension guide:
  `docs/admin-console.md`.
- [FEATURE] **Web place search and A→B routing**: same-origin
  `/map/search`, `/map/reverse`, `/map/route` proxies over `src/osm.rs`
  (validation, LRU cache, 2-permit route semaphore, `alternatives=true`),
  with OSRM foot/bike profiles enabled only when
  `PINPOINT_OSRM_FOOT_URL` / `PINPOINT_OSRM_BIKE_URL` are configured
  (`/map/capabilities` reports what is available).
- [FEATURE] **Map-first web UI with locally vendored Leaflet 1.9.4**
  (BSD-2, served from `/vendor/`, no CDN): full-viewport map, dark tile
  treatment, friend pins, place sheet, route panel with alternatives and
  step list, scale bar and ODbL attribution.
- [FEATURE] **Search UX**: submit-on-Enter by default (Nominatim policy),
  opt-in 300 ms autocomplete with `PINPOINT_SEARCH_AUTOCOMPLETE=1` for a
  self-hosted Nominatim, category chips, distance-sorted results,
  “What’s here?” reverse geocoding on map click.
- [FEATURE] **Saved places and recent places** on-device, scoped per account
  (`pinpoint.v1.<userId>.savedPlaces|recents`), with clear-saved and
  clear-history actions.
- [FEATURE] **Locate me** (explicit button, browser permission only) and
  route follow mode using `watchPosition` (preview, no voice).
### Fixed
- [FIX] **Share expiry used a hardcoded seed date** (`2026-11-08`), so a share
  created for 1 hour never expired in the demo. Expiry now parses RFC 3339
  and compares against the real UTC clock; `duration_hours=0` means “until
  turned off”.
- [FIX] **Admin user list could be stale on a replica** started before new
  accounts were created elsewhere: `/api/v1/admin/overview` and
  `/api/v1/admin/users` refresh from Postgres before reading and label the
  view “all accounts, including anonymous and never-signed-in users”.
- [FIX] **User management was open to any signed-in account** (critical): the
  existing list/create/update/delete user endpoints did not check for an
  admin role, so a resident could raise their own role or delete accounts.
  They now require `admin`/`administrator`; self-role/active changes are
  refused and `/users/search` requires a session.
- [FIX] **Navigation Start failed when geolocation was blocked**: “Navigate”
  now always enters a route-preview banner (distance/time + first step) even
  without GPS, shows a precise blocked/unavailable/timeout message with an
  “Enable location” retry, and keeps the route drawn. Exit stops the watch and
  removes the self marker.
- [FIX] **No “you are here” marker after locating**: the locate button only
  flew the map to the GPS fix. It now draws a pulsing “You” marker plus a
  dashed accuracy circle (also updated during route follow mode) and clears
  it on logout.
- [FIX] **Cross-account saved-place flash**: switching accounts without a
  reload could briefly show the previous account’s saved places; the drawer
  is cleared on logout and re-rendered from the current account’s keys.
- [FIX] **Search/route coordinates in server logs**: `converge` now logs the
  path only (`loggable_uri`), so queries and coordinates never reach stderr.
### Security
- [SECURITY] Map endpoints accept no session/friend data, are cached by
  query/coords only, and redact query strings from logs.

## [1.0.0] - 2026-11-13
### Added
- Client platforms track (`docs/client-platforms.md`): Windows/Linux/macOS
  desktop via `src/main.rs` (`--help`, `$PINPOINT_BACKEND`, per-platform
  defaults in `platform.rs`), Linux `.deb`/`.desktop` + macOS `.app`
  metadata (`Cargo.toml`, `packaging/`), Android `cargo-apk` scaffold
  (`android/` incl. `network_security_config.xml`), iOS `cargo-xcodebuild`
  scaffold (`ios/Info.plist`), and a Rust/WASM web entry (`web/`,
  `src/bin/pinpoint-web.rs` via Trunk). Status is scaffolding-only for
  mobile/web — GUI verification needs NDK/Xcode/wasm toolchains or CI
  matrices on those OSes.
- OSM map parity (web + Android + Rust desktop): interactive Slippy web map
  with CARTO/Wikimedia layers, Nominatim search/reverse, OSRM routing
  (driving/foot/bike + GPX), Overpass POI browse (9 categories), on-device
  bookmarks (GeoJSON export); Android MapLibre with 3 OpenFreeMap styles,
  offline packs, Nominatim/OSRM/Overpass screens, DataStore bookmarks;
  Rust desktop OSM hand-off links fixed (OSM/directions/Organic Maps + ODbL credit)
- Full SDLC documentation set (`docs/sdlc/00-16`): BRD, FR/NFR/TRS, OSM
  integration guide (ODbL, Nominatim/OSRM/Overpass policies, self-hosting),
  architecture, security/privacy, QA, deployment, risks, traceability
- UML diagrams 12–15: map search/places, routing/navigation, offline/
  bookmarks, web Slippy engine (Mermaid, with FR traceability)
- Automatic QA: `scripts/qa.sh` + `scripts/qa.ps1` (tile policy, attribution,
  CSP sync, SW SHELL, Mermaid, notices) + CI `map-qa` job
- Rust desktop client (`pinpoint-desktop`, root crate): eframe/egui UI with
  stateful login/register/logout/password-change, `reqwest` API client bound to
  the server contract (session cookie + `X-CSRF-Token` double-submit), transport
  policy requiring HTTPS off-loopback (ADR-0006), and unit + API-contract tests
  (`tests/contract.rs`) covering fail-closed login/logout and CSRF placement
- CI `rust` job: `cargo fmt --check`, `clippy -D warnings`, locked build/test,
  and a `cargo-audit` dependency vulnerability audit
- CI `release` job (`windows-latest`): builds the `--release` binary, packages it
  into `pinpoint-desktop.zip`, and uploads it as a GitHub Actions artifact
- `docs/DEPLOYMENT.md` §4 Rust Desktop Deployment: build, run flags, ADR-0006
  transport policy, verified distributable artifact path, and demo credentials
- Kotlin/Jetpack Compose Android app scaffold (`mobile/`) — planned next
- `weba` admin console and `webk` kiosk check-in clients — planned
- Project governance docs: CONTRIBUTING, CODE_OF_CONDUCT, docs/{PROJECT, ROADMAP,
  ARCHITECTURE, SECURITY, PRIVACY, TESTING, DEPLOYMENT, GLOSSARY}, LICENSE template
- OS keychain integration (`src/security/keychain.rs`): unified `Keychain`
  trait with `OsKeychain` (Windows Credential Manager / macOS Keychain /
  Linux Secret Service via the `keyring` crate) and `FileKeychain`
  fallback. `KeyManager` now persists its active key through
  `Keychain::load_or_generate`; `PinpointApp::new` opens the
  best-available keychain via `keychain::open_default`. Closes the
  "OS keychain for bookmark keys" hardening item.
- Spiral execution plan (`docs/sdlc/17-…`): model statement, turns S0–S6
  (Foundation → OSM parity → Security → Reliability → Multiplatform/mobile →
  Verification → Deployment/report), four stages per turn (goal elaboration,
  risk analysis & prototyping, interim version, next-turn planning), exit
  criteria, pros/cons + mitigations, and the critical path to completion.
- Interactive Slippy tile canvas (`src/map.rs`, `src/tilepacks.rs`,
  `src/ui/map.rs`): Web Mercator projection (`latlon_to_world_px` /
  `world_px_to_latlon`), drag-to-pan, scroll/`+`/`-` zoom (`z2..=z18`),
  on-disk PNG tile texture rendering from `~/.pinpoint/tiles/{z}/{x}/{y}.png`,
  in-app **"Download current area pack"** button with multi-source ODbL
  fallback (CARTO → Wikimedia → CARTO Voyager), and clickable ODbL
  `https://www.openstreetmap.org/copyright` attribution.
- E2E location encryption + OS keychain publish wiring (`src/app.rs`):
  derives per-user `e2e_location_key` on login/register, persists it in the
  OS keychain, encrypts published locations with AES-256-GCM before sending,
  and wipes key material on logout.
- Real GPS publish + dual mobile apps (`src/platform.rs`, `src/mobile/mod.rs`,
  `mobile/`, `android/`, `ios/`): `EnvOrFileGpsProvider` (`PINPOINT_GPS_FIX`
  or `gps_fix.json`), **"📍 Use GPS"** button on the Map screen, `geo:` and
  `pinpoint://` mobile deep-link parser, `[package.metadata.android]` for
  `cargo-apk`, and the clean Kotlin/Jetpack Compose Android app in `mobile/`
  alongside the Rust mobile client.
- Bookmarks GeoJSON export/import (`src/bookmarks.rs`, `src/ui/bookmarks.rs`):
  RFC 7946 `FeatureCollection` export/import with fail-closed coordinate
  validation and one-click clipboard copy.
- Component documentation for **all architectures** (`docs/components/`):
  01 Go backend · 02 Python/FastAPI backend · 03 Rust/Axum backend ·
  04 Rust multiplatform client · 05 Kotlin Android app · 06 archived vanilla-JS
  web client — each standalone with stack, module map, API surface, data model,
  security controls, build/run/test, deployment, and cross-component relations.
- Competitive & similar-app analysis (`docs/components/07-competitive-analysis.md`):
  Google Maps LS · Apple Find My · Life360 · Glympse · WhatsApp/Telegram Live
  Location · Strava Beacon · Organic Maps · OsmAnd — feature matrix, consent/
  privacy model comparison, architecture patterns, gaps, and prioritised P1–P3
  recommendations (temporary share links, real-time streaming, battery-aware
  sampling, E2E bookmark sync). Referenced from `docs/sdlc/00-overview.md` and
  `docs/PRIVACY.md`; ROADMAP updated with the P2/P3 items.
- Dual authentication model (ADR-0009, **Accepted (partial)** +
  `docs/security/dual-auth-model.md`): anonymous device-bound accounts
  (no PII; one-time recovery code) alongside identity-linked accounts
  (email/OAuth/passkey), with an upgrade path and Sybil/abuse controls.
  Phases 1–2 implemented; OAuth/passkey/magic-link/link endpoints pending.
- **Session management decision + spec** (`docs/security/session-management.md`,
  ADR-0012): opaque 256-bit server-side sessions (`pinpoint_sessions`) shared
  across services, `HttpOnly; SameSite=Lax` cookie + session-bound CSRF
  double-submit, sliding idle timeout, immediate revocation on
  logout/logout-all/password-change/per-session revoke, `mfa_pending` gate,
  split-service sharing and in-memory fallback. The JWT + rotating-refresh
  design of ADR-0003 is superseded for transport (ADR-0012 keeps the password
  decision).
- **Authentication mechanisms reference** (`docs/security/auth-mechanisms.md`):
  TOTP (RFC 6238, HMAC-SHA256, ±1 step, replay-safe), OAuth2/OIDC + PKCE
  primitives, passkeys/magic-link designs, anonymous recovery codes, lockout
  and rate limiting, CSRF, audit, scraping detection, request signing, and a
  per-mechanism implementation-status table (TOTP + OAuth primitives
  implemented; passkeys/magic links/password reset pending).
- Auth/session **handlers landed**: `User` gains `auth_method` /
  `public_key` / `recovery_code_hash` / `identity_subject`; migration 14 adds
  the columns and `pinpoint_sessions`. The mock tokens are gone: login/register
  create a real session (HttpOnly cookie + CSRF), logout revokes it, password
  change revokes all sessions, and `session_auth` middleware validates the
  cookie + `X-CSRF-Token` on every request.
- **Session device management**: `GET /api/v1/auth/sessions` (8-hex prefix +
  `current` flag, full id never exposed), `DELETE /api/v1/auth/sessions/:id`
  (own sessions only, CSRF-protected), `POST /api/v1/auth/logout-all`; sliding
  `last_activity`, configurable idle timeout (`PINPOINT_SESSION_IDLE_MINUTES`,
  default 12 h demo), expired sessions deleted on use.
- **Anonymous accounts** (`POST /api/v1/auth/anonymous`,
  `/auth/anonymous/recover`): device-bound account with no PII, one-time
  160-bit recovery code shown once (SHA-256 stored, rotated on every
  recovery), same session/CSRF machinery; web UI adds "Continue anonymously",
  a recovery-code panel and a "Recover with code" form.
- **Web UI** (`web/app/`, served at `/`): login/register, anonymous sign-in,
  Slippy map via the server `/map/tiles` proxy, publish location, friends,
  shares, active-sessions card with revoke/sign-out-everywhere; CSP +
  security headers; ODbL attribution. Plain HTML/CSS/JS, no third-party
  scripts, no IP geolocation.
- **Tile proxy fix**: `tile.openstreetmap.de` (FOSSGIS) is the readable demo
  default with Wikimedia/CARTO fallbacks, an accepted User-Agent, one retry for
  rate-limited bursts, and a source-aware cache. Fixed a latent cache-key bug
  (`u8` truncated x/y at z ≥ 9 and could serve the wrong tile). `/map/tiles`
  returns 200 without a watermark; production self-hosts (ADR-0011).
- **Local-only web assets**: local `/favicon.svg` (no favicon service), CSP
  tightened to `style-src 'self'` and explicit `font-src 'self'` (the UI uses
  `system-ui`, so no font request is made at all), and `tests/assets.rs`
  guards against Google Fonts/CDN URLs or absolute subresources in the shipped
  HTML/CSS/JS.
- **Desktop tile packs via the backend**: `tilepacks::download_pack` now
  prefers `<backend>/map/tiles/{z}/{x}/{y}` before `PINPOINT_TILE_URL` and the
  public fallbacks, so desktop clients do not call a CDN directly when a
  Pinpoint server is configured.
- ADR-0009 status → **Accepted (partial)**; traceability matrix (15) rebuilt
  against the current Rust/Kotlin tree with FR-23…FR-30 auth/session/E2E/
  GPS/dual-mobile rows.
- **Database design** (`docs/database/schema.md`, ADR-0010, reference DDL
  `db/schema/00-target.sql`): normalized domain-owned tables (auth/social/
  location/audit), no single JSONB source of truth, identity columns instead
  of a counters table, application-level AES-256-GCM column encryption with
  `v1:` format + blind indexes (`email_hash`), one-way hashes for
  passwords/session ids/CSRF/recovery, per-service least-privilege roles, and an
  additive-modularity rulebook. Migration path documented (backfill →
  encrypt → retire `pinpoint_state`/`pinpoint_counters`).
- **Self-hosted OSM services** (ADR-0011, `docs/maps/self-hosted-osm.md`):
  `PINPOINT_NOMINATIM_URL`, `PINPOINT_OSRM_URL`, `PINPOINT_OVERPASS_URL`,
  `PINPOINT_TILE_URL` make every OSM endpoint self-host-first; public
  endpoints are demo fallbacks only. ODbL attribution stays mandatory
  (`openstreetmap.org/copyright` + `X-Data-Attribution`). Added `.env.example`
  with the OSM + security variables.
- **OSM parity quick wins** (`docs/maps/osm-feature-parity.md`): POI result
  list with opening hours/phone/website + Save/OSM buttons, full route-steps
  display, **GPX 1.1 export** for routes (`src/osm.rs::export_route_gpx`),
  and a Save button on search results.
### Fixed
- [FIX] **CSRF lost after reload** (`web/app/app.js`, `auth_handlers::me`): a
  reloaded page kept the `HttpOnly` session cookie but had no CSRF token, so
  the first publish/revoke returned `403 CSRF_FAILED`. `GET /auth/me` now
  returns `X-CSRF-Token` and the client re-arms it from any response.
- [FIX] **Session ids stored in plaintext** (`state/sessions.rs`): a database dump
  could have been replayed as cookies. Sessions are now persisted/kept as
  SHA-256 hashes; the raw 256-bit value exists only in the cookie.
- [FIX] **Tile cache key truncation** (`src/server/mod.rs`): `(u8,u8,u8)` wrapped
  x/y at z ≥ 9 and could serve a neighbouring tile; the key is now
  `(u32,u32,u32)` and cache hits report their `X-Tile-Source`.

### Changed
- [SECURITY] **Password hashing → Argon2id** (`src/security/password.rs`): new hashes are
  Argon2id PHC strings (`$argon2id$v=19$m=19456,t=2,p=1$…`, OWASP-aligned,
  memory-hard) via the `argon2` crate 0.6. Legacy `pbkdf2$…` hashes still
  verify and are transparently re-hashed to Argon2id on the next successful
  login (`needs_rehash`). `pbkdf2` remains only for legacy verification and
  E2E key derivation. Chosen over bcrypt (72-byte limit, recent
  RUSTSEC-2026-0199 DoS advisory).
- [INFRA] Rust desktop client: bumped `eframe` 0.30 → 0.32, moving the Linux
  accessibility stack off `quick-xml 0.30.0` (RUSTSEC-2026-0194,
  RUSTSEC-2026-0195, both high) to `0.41.0`; `cargo audit` now exits 0 with
  0 vulnerabilities (5 unmaintained warnings only)

## [Unreleased-redundancy]
### Added
- Client-side redundancy: `src/security/circuit_breaker.rs` (per-backend
  circuit breaker with Closed/HalfOpen/Open states), `src/backend/health.rs`
  (active health monitoring of backend URLs), `src/backend/failover.rs`
  (FailoverBackend wrapping multiple REST backends with automatic failover,
  circuit breakers, and exponential backoff retry), `FailoverV1`
  `BackendKind` + `PINPOINT_FAILOVER_URLS` env support
- Server reliability: `/health` verifies Postgres connectivity (returns
  `degraded` when DB unreachable); new `GET /api/v1/sync-status` endpoint
  reports replication freshness (`last_persist_at`, `last_persist_age_secs`,
  `fresh`) for monitoring replica health
- Client offline resilience: `PendingMutation` queue in `PinpointApp` — all
  mutation methods queue locally when all backends are unavailable;
  `sync_pending_mutations()` auto-replays queued mutations when connectivity
  returns; `pending_mutation_count()` / `pending_mutation_descriptions()`
  exposed for UI; offline mode auto-engaged on failure
- Map tile redundancy: `src/server/mod.rs` `map_handlers::tile` now tries
  multiple ODbL-compliant sources in order (CARTO with key → Wikimedia →
  OpenFreeMap) instead of failing closed at 502 when CARTO_API_KEY is missing.
  `X-Tile-Source` header reports which source served the tile. Attribution
  endpoint lists all configured sources.
- E2E encryption: `src/security/e2e.rs` provides AES-256-GCM end-to-end
  encryption for location data with PBKDF2-derived per-user keys. Location
  payloads are encrypted client-side before being sent to the server; the
  server stores ciphertext only. `derive_location_key`, `encrypt_location`,
  `decrypt_location` exported from `src/security/mod.rs`. `pbkdf2` made public
  in `src/security/password.rs`.

## [0.3.0] - 2026-10-13
### Added
- Docker deployment: `docker-compose.yml`, multi-stage `backend/Dockerfile` (non-root,
  healthcheck), `web/Dockerfile` + `web/nginx.conf` (SPA + `/api` reverse proxy)
- Idempotent demo seeding (`SEED_DEMO_DATA`): admin (forced password change), Alice & Bob
  with mutual shares and Paris locations
- Static file serving from the Go binary when `WEB_ROOT` is set (single-container option)
- `COOKIE_INSECURE` dev flag; `.env.example` documenting all settings
- Web SPA v1: login/CSRF flow, OSM map with friend pins, freshness/stale badges, precision
  badges, navigation export, location publishing, friend requests, directional share
  management, admin provisioning, forced-password-change UX

### Changed
- Mobile client decision: Flutter → **Kotlin (Android-first)** due to no macOS/iPhone
  development hardware; Android emulator covers the full test loop

## [0.2.0]
### Added
- Full Go backend feature parity: JWT + session auth, bcrypt, CSRF double-submit,
  rate limiting, lock/unlock, must-change-password gate
- Authorization + security test suite (`go test ./...` green, incl. IDOR fail-closed cases)

### Removed
- Entire legacy Python/FastAPI backend (refactor complete; no `.py` files remain)

## [0.1.0]
### Added
- Initial Python/FastAPI prototype (superseded by 0.2.0)
- Core domain model: users, friendships, directional share permissions, current location
