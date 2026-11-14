# Pinpoint 1.3.5 — Release Notes

**Released:** 2026-11-14 · **Version:** `1.3.5` (from `1.3.4`) ·
**Status:** Active GPS acquisition and a publish deep link, verified against a local backend.

**Artifacts:** `pinpoint-android-1.3.5.apk` (arm64-v8a) and
`pinpoint-android-1.3.5-armeabi-v7a.apk` (32-bit ARM). GitHub also generates the
source `zip`/`tar.gz` archives for the `v1.3.5` tag.

Flags: `[FEATURE]` new capability · `[FIX]` bug fix · `[SECURITY]` hardening ·
`[DOCS]` documentation · `[INFRA]` build/deploy.

---

## 0. 1.3.5 features

- **Active GPS:** when no provider has a recent last-known fix, **Use GPS**
  requests location updates for up to 8 seconds (GPS → fused → network) and
  removes them afterwards. The fix still has to be recent and reasonably
  accurate before it is published.
- **`pinpoint://publish`:** publishes the current location on launch. It waits
  up to 12 seconds for the persisted session to restore, so a cold-start deep
  link does not race the login.
- **Verified publish path:** on the API 37 emulator with a local backend, the
  app logged `applied launch deep link`, then `location published`, and the
  server recorded `POST /api/v1/locations/me`.
- **Debug UI automation:** a debug-only marker (`files/allow_screenshots`)
  allows screenshots for testing; release builds ignore it and keep
  FLAG_SECURE.

## 1. 1.3.4 features

- **One-shot GPS:** **Use GPS** requests the runtime location permission through
  the live activity, then reads the last known fix from every LocationManager
  provider. Approximate-only grants are accepted; coordinates are validated
  before publishing.
- **Share in/out:** Android accepts shared `geo:`/map links and plain
  coordinates (`ACTION_SEND`, `text/plain`) and opens the map there on launch.
  Repeat shares while Pinpoint is already running need a cold start because
  the Rust-only `NativeActivity` cannot observe Android's `onNewIntent`.
  Place sheets share a `geo:` link and the kiosk QR card shares its
  `pinpoint://kiosk?token=...` link through the system share sheet. Desktop
  copies the link instead.
- **PIN app lock:** Profile → App lock stores an Argon2id hash only and locks on
  launch and when returning to the foreground, with an exponential lockout
  after wrong PINs.
- **JNI fix:** android-activity exposes the Application context, not the
  Activity, so FLAG_SECURE, deep links, permission dialogs and sharing were
  silently failing in 1.3.3. The live `NativeActivity` is now registered and
  every JNI helper uses it.
- **Pinch-to-zoom:** multi-touch pinch steps the map zoom proportionally,
  clamped to zoom 2–18.
- **Verification:** FLAG_SECURE confirmed by a black app-window `screencap`;
  `ACTION_SEND` confirmed in logcat (`action=android.intent.action.SEND`,
  `text_present=true`); location permission and provider enumeration confirmed
  on the API 37 emulator.

## 1. 1.3.3 patch

- **Deep links:** `pinpoint://` and `geo:` are registered and routed on launch,
  including kiosk tokens and map-centre hand-off.
- **Kiosk QR:** the Kiosk screen renders a scannable
  `pinpoint://kiosk?token=...` QR.
- **FLAG_SECURE:** screenshots and screen recording of location data are blocked.
- **No-email fallback:** report and log copies use Android's system
  `ClipboardManager`, so the full report survives when no mail app exists.
- **Fallback safety:** a release with no configured backend disables session
  restore and sign-in instead of sending credentials or a stored session to the
  shared loopback port.
- **Diagnostics:** stronger redaction for Authorization/Bearer headers, JSON
  values and suffixed tokens; log clearing keeps the open writer usable.
- **Android 15:** the mobile tab bar reserves a bottom inset for the edge-to-edge
  gesture bar.
- **Crash prompt:** reports are marked seen after Email or Close, so the prompt
  does not repeat.

## 1. 1.3.2 patch

- **Android release startup:** a rejected default backend URL now falls back to
  the loopback offline client instead of panicking. The app opens with an
  **Offline** badge; `adb reverse tcp:8080 tcp:8080` lets a USB phone reach the
  laptop server at the fallback URL.
- **Crash reports:** Rust panics and `eframe::run_native` errors write a redacted
  report under the app data directory. The next launch shows an editable dialog
  with Email/Copy/Discard/Close. Nothing is sent automatically; Email opens the
  user's mail app with a `mailto:` link to `plesudinu+waitlist@proton.me`.
- **Logs and troubleshooting:** a rolling 1 MiB log, a device/page-size summary,
  log copy/clear, and a manual **Report a problem** flow in About/Profile.
- **Update check privacy:** opening About no longer contacts GitHub; the check
  runs only when the user presses **Check for updates**.
- **16 KB alignment:** the arm64 linker alignment from 1.3.1 remains in place.

## 1. 1.3.1 patch

- **Android 16 KB pages:** the arm64 native library is now linked with
  `-Wl,-z,max-page-size=16384`, so its `PT_LOAD` segments are 16 KB aligned.
  Android 15+ phones configured with 16 KB pages refuse to load 4 KB-aligned
  libraries (`program alignment (4096) cannot be smaller than system page size
  (16384)`), which closed the app before the UI started.

## 2. 1.3.0 native parity

- **Native map proxy**: search/reverse/routing go through the Pinpoint backend
  with map-centre bias, stops, avoid classes, alternatives and capability
  gating.
- **Route planner**: place-based A/B, up to six stops, swap, car/walk/bike,
  alternatives, steps, route share, GPX/KML/GeoJSON export and follow mode.
- **Place sheet**: map tap → reverse geocode, selected marker, Directions,
  Set as origin, Save, Copy, distance and external map hand-off.
- **Sharing**: per-friend sheet with existing-share lookup, enable/pause,
  precision, duration, incoming status and revoke.
- **Account**: MFA TOTP login, anonymous sign-in + recovery code, keychain
  session resume, active sessions/revoke and sign out everywhere.
- **Admin**: overview, users/roles/active/delete, audit log and diagnostics
  triage; correct `admin`/`administrator` gating.
- **Mobile UI**: ArtCraft-inspired Tools tile sheet, light/dark OSM map,
  category chips, bottom tabs, route sheet above the tab bar, About/updates.
- **Update checker**: user-initiated GitHub `releases.atom` check with semver
  comparison, release notes and release-page hand-off.
- **CI/security**: sqlx is Postgres-only, the lockfile is refreshed (mio
  1.2.5), wasm stubs exclude axum/sqlx/tokio, and `cargo audit` reports zero
  vulnerabilities (unmaintained-only warnings remain).
- **Repository**: archived MSVC/SDKs, installers, Go toolchain and build output
  are untracked; 7,810 → 796 tracked files.

## 1. 1.2.1 patch

- **ZAP baseline hardening:** every web response now sets
  `Permissions-Policy`, `Cross-Origin-Embedder-Policy`,
  `Cross-Origin-Opener-Policy` and `Cross-Origin-Resource-Policy`.
  The re-scan is **0 high / 0 medium / 0 low**; the remaining notices are
  informational (intentional `no-store` on HTML, modern-web-app hint). Raw
  evidence: `docs/evidence/zap-baseline.html|json|md`; summary:
  `docs/evidence/zap-baseline-1.2.1.md`.
- **Caching:** static JS/CSS use `no-cache` (revalidate); HTML keeps
  `no-store`.
- **Test robustness:** OS-keychain tests skip cleanly when Windows returns
  `ERROR_NO_SUCH_LOGON_SESSION` (headless/sessionless runs).

---

## 1. What is functional now

| Area | State | What works |
|---|---|---|
| **Auth & sessions** | ✅ | Email/password (Argon2id), opaque hashed sessions, CSRF, logout/logout-all, session list/revoke, MFA, password change, lockout |
| **Anonymous accounts** | ✅ | Create device-bound account, one-time recovery code, recovery rotation |
| **Usernames & privacy** | ✅ | Stable public handle (`calm-otter-42`), exact-match lookup, friend requests by handle, pending requests show handles only, names/emails revealed to accepted friends, numeric IDs not walkable, free-text search admin-only |
| **Web navigation** | ✅ | Leaflet map (local), place search + reverse, place sheet, A→B + multi-stop routes, alternatives, steps, navigate preview with live next-stop, GPX/KML/GeoJSON, route history, saved-place filters, map appearance, locate-me |
| **Route interop** | ✅ | Google Maps + Organic Maps links, Magic Earth `magicearth://` links, KML/GeoJSON/GPX import files |
| **Per-friend sharing** | ✅ | On/off, exact/approximate/city, 1h–7d/forever, immediate stop, real expiry |
| **Admin console** | ✅ | Role-gated modular tab: overview, all accounts (incl. anonymous/never signed in), role/active changes, safe purge revoking sessions |
| **Diagnostics** | ✅ | `pinpoint-diag` on **8005**, PII-free reports (category + message + public handle only), admin triage, no IP/UA/coordinates stored |
| **Admin API** | ✅ | `pinpoint-admin` on **8006**, same fail-closed role gates; report triage + append-only audit |
| **Audit trail** | ✅ | Role/active changes, account deletions and report triage in `pinpoint_admin_audit`; ids/handles only, no email or coordinates |
| **Alice/Bob containers** | ✅ | `pinpoint-demo-client` logs in through the API and publishes real locations on an interval; compose profile `demo` |
| **Desktop client** | ✅ | Map, OSM search/routing/POIs, bookmarks, check-ins, emergency, offline packs; shows the user's public handle and accepts handles or emails for friend requests |
| **Storage** | ✅ | Schema/migrations 1–15 applied against live Postgres 16; new users and locations persisted across a server restart; split ports share sessions/reports/audit |
| **OSM self-host** | ⚠️ Config | `PINPOINT_*_URL` overrides wired; public providers are demo fallbacks |
| **Mobile / WASM** | ⚠️ Scaffold | Signed Android APKs (arm64 + armv7); iOS/WASM remain scaffolds |
| **Privacy** | ✅ | No trackers/IPs, local assets, explicit consent, self-host-first OSM, user-initiated diagnostics |
| **Verification** | ✅ | fmt/clippy/build/test green; browser checks for identity, report → triage → audit; 8005/8006 ports; demo-client login/publish |

### Known limitations (honest)

- No live traffic, weather, lane guidance, road signs or voice guidance.
- Walk/bike routing requires self-hosted OSRM profiles; avoid options require
  a self-hosted profile with exclusions.
- Navigation is a preview with live next-stop; no off-route auto-reroute.
- Postgres persistence was verified against a live Postgres 16 instance,
  including migrations 1–15, restart survival and shared split-port sessions.
- ZAP scan, Android APK (NDK/JDK 21) and iOS (macOS/Xcode) remain external.
- Compose config is validated and the existing stack runs; the new 1.2.0
  images were not rebuilt in this environment.

---

## 2. Change log (flagged)

### `[FEATURE]`
- Generated public usernames + public/friend/private profile split.
- Friend requests by exact handle (legacy exact email fallback for older
  clients); pending requests no longer leak legal names.
- Step-up admin actions: `pinpoint-diag` 8005, `pinpoint-admin` 8006,
  `/api/v1/diagnostics/report`, admin diagnostics triage, `/api/v1/admin/audit`.
- Migration 15: `pinpoint_diagnostic_reports`, append-only
  `pinpoint_admin_audit`, `users.created_at`, report/audit counters.
- Real Diagnostics and Audit admin modules replacing the placeholders.
- Help & diagnostics report form in the web client (category + message only).
- Headless `pinpoint-demo-client` for Alice/Bob containers; compose profile
  `demo`.
- Explicit `PINPOINT_ALLOW_PLAINTEXT_HOSTS` exact-host opt-in for internal
  container DNS (HTTPS remains the default).
- GPX/KML/GeoJSON route export and Google/Organic/Magic Earth hand-off.

### `[FIX]`
- Location publish and emergency alerts now use the real UTC clock instead of
  the seed timestamp; check-ins and friend requests do the same.
- Duplicate friend requests are rejected instead of stacking rows.
- Accepted-friend vs pending vs unrelated profile responses are now distinct.
- Admin deletion records an audit row and removes the account without putting
  its email in the audit trail.

### `[SECURITY]`
- ZAP baseline re-scan: 0 high / 0 medium / 0 low; COOP/COEP/CORP and
  Permissions-Policy headers set on every web response.
- Argon2id; hashed sessions; strict CSP; local assets; admin role gates.
- No raw IP/UA storage; diagnostics explicitly exclude location and device
  identifiers; audit is ids/handles only; OSM calls never carry user/friend
  data.
- Numeric user IDs are not walkable; non-admin user search is exact-handle
  only.

### `[DOCS]`
- Dual-auth model updated with the public-identity matrix and lookup rules.
- Admin console guide now documents the real modules, 8005/8006 split and the
  append-only audit guarantees.
- Client-platforms guide documents the headless demo clients and plaintext
  opt-in.

### `[INFRA]`
- New binaries: `pinpoint-diag`, `pinpoint-admin`, `pinpoint-demo-client`.
- Compose: diag/admin services + gateway routes + `demo` profile for Alice/Bob.
- Per-service grants: diag gets report INSERT/SELECT only; admin reuses the
  users role with sessions/report/audit grants.

---

## 3. Run

Combined demo (web UI + all APIs):

```powershell
$env:BIND_ADDR = "127.0.0.1:8080"
.\pinpoint-server.exe
# http://127.0.0.1:8080/ · alice@example.com / DemoPass123!
# admin: admin@example.com / ChangeMe123!
```

Split ports:

```powershell
$env:BIND_ADDR = "127.0.0.1:8005"; .\pinpoint-diag.exe
$env:BIND_ADDR = "127.0.0.1:8006"; .\pinpoint-admin.exe
```

Alice/Bob as real clients:

```powershell
$env:PINPOINT_BACKEND = "http://127.0.0.1:8080"
$env:PINPOINT_EMAIL = "alice@example.com"
$env:PINPOINT_PASSWORD = "DemoPass123!"
$env:PINPOINT_LAT = "47.0105"
$env:PINPOINT_LON = "28.8638"
.\pinpoint-demo-client.exe
```

Docker compose:

```powershell
docker compose up --build -d
docker compose --profile demo up -d alice bob
# gateway :8000 · diagnostics :8005 · admin API :8006
```

Desktop: `pinpoint-desktop.exe --backend http://127.0.0.1:8080`.

Self-hosted OSM:

```powershell
$env:PINPOINT_TILE_URL = "https://tiles.example.com/{z}/{x}/{y}.png"
$env:PINPOINT_NOMINATIM_URL = "https://nominatim.example.com"
$env:PINPOINT_OSRM_URL = "https://osrm.example.com"
$env:PINPOINT_OVERPASS_URL = "https://overpass.example.com/api/interpreter"
```

## 4. Verification

- fmt ✅ · clippy `-D warnings` ✅ · build ✅ · release build ✅
- **174 unit + 7 asset + 10 contract + 14 router tests** ✅
- Browser: username display, request by handle, pending/accept, duplicate and
  unknown-handle states; report → triage → audit in the admin console.
- Ports: 8005 ingestion 201, invalid category 400, admin route isolated 404;
  8006 health 200, unauthenticated admin 401.
- Demo client: logged in as `@bright-badger-2`, published through
  `/locations/me`, and a friend read back the fresh `updated_at`.
- Postgres 16: migrations 13–15 applied; a new user (`@coral-badger-5`) and
  published location survived a server restart; one login on 8080 authorized
  the admin API on 8006 with the shared session; a report ingested on 8005
  appeared in the 8006 list, was triaged, and wrote an audit row.
- ZAP 2.17.0 baseline: **0 high / 0 medium / 0 low**, 4 informational; raw
  reports archived under `docs/evidence/`.
- Compose: `docker compose config --quiet` ✅, 11 services including
  diag/admin/alice/bob.

## 5. Next

1. Authenticated ZAP active scan (seeded session + context).
2. Android APK (NDK + JDK 21) and iOS build on macOS.
3. Rebuild the 1.2.x compose images and run the full gateway/demo profile.
4. Final thesis report / demo script.
