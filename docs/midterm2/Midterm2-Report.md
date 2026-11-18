# Pinpoint - Midterm 2 Report

**Project:** Pinpoint - privacy-first location sharing for trusted friends  
**Version:** 2.6.1  
**Date:** 18 November 2026  
**Author:** [Your name]  
**Live system:** https://pinpoint-uj4y.onrender.com  
**Source:** https://github.com/Prince-of-Nothing/Pinpoint  
**Release:** https://github.com/Prince-of-Nothing/Pinpoint/releases/tag/v2.6.1

---

## Abstract

Pinpoint is a privacy-first location-sharing application for trusted friends.
Its defining rule is that **friendship is not consent**: a friendship grants no
location access until the owner creates an explicit, directional share with a
chosen precision and (optionally) an expiry. The server enforces every access
decision; the client is never trusted.

The project is implemented as one Rust codebase with three client surfaces:
a web client, a desktop client (eframe/egui), and an Android build of the same
client. A Rust/Axum API persists users, friendships, shares, the latest
location, a private 24-hour trail, private place notes, check-ins, emergencies
and diagnostics in PostgreSQL. Offline map packs for Moldova and Romania are
published from `pinpoint-maps`; the web client can install the Moldova pack
into OPFS and search it without a network, while the native map server routes
on the installed graph.

At Midterm 2 the delivered system is **v2.6.1**, deployed and verified live:
registration and sessions, friendships, consent-based sharing, pause/revoke,
exact/approximate/city-only precision, the private Today trail, private photo
notes, offline map/search, online search and routing, account deletion,
diagnostics, and a signed Android APK. No demo accounts and no seeded pins are
shipped; every account is registered through the public flow.

The report covers the problem, the delivered scope, architecture, consent and
security model, data model, verification evidence, the demo, limitations and
the next steps. It is paired with a demo script and a slide deck in this
directory.

---

## 1. Problem and goals

Location sharing is useful for safety, but most products treat friendship as
permanent access. Three failures motivate Pinpoint:

1. **Friendship equals access.** Once two people are friends, one can watch the
   other indefinitely unless the other remembers to disable something.
2. **Stale data looks live.** A last-known dot is often shown without a clear
   freshness state, which is dangerous in an emergency.
3. **Control is incomplete.** Pausing, revoking, narrowing precision and
   understanding who can see you are buried or missing.

Goals:

- **Consent for every share.** A share is one-directional, explicit, and
  revocable; the owner chooses exact, approximate or city-only precision and an
  optional expiry.
- **Server-side enforcement.** Every read and write is authorized again on the
  server; a modified client cannot widen its access.
- **Honest state.** Expired, paused and revoked shares fail closed; check-ins
  and emergency alerts are owner-scoped.
- **Privacy by default.** No trackers, no third-party scripts, no IP
  geolocation, no real user data in tests, and map attribution is always
  visible.
- **Offline-first maps.** A country pack can be installed on-device, searched
  without a connection, and used for local routing on the native client.

## 2. Delivered scope (v2.6.1)

| Area | Delivered | Status |
|---|---|---|
| Identity | Email registration, login, optional TOTP, opaque sessions, sign-out, sign-out everywhere, password change, account deletion | Live |
| Friends | Request by exact handle, accept/reject, remove; handles are public, emails are not searchable | Live |
| Sharing consent | Explicit share sheet with named friend, precision, duration, a confirmation summary, and an active/paused/expired/revoked state | Live |
| Location | Publish manual or GPS coordinates; friend reads only through an active, unexpired, owner-created share | Live |
| Daily trail | A private 24-hour trail of the owner's published points, listed and drawn on the map; owner-only, 7-day server retention | Live |
| Private notes | Label + message + optional JPEG/PNG photo attached to a place; downscaled in the browser, bounded to 675 KB, owner-only | Live (web); text-only native |
| Map | OpenFreeMap online basemap, OSM search, OSRM driving directions, saved/recent places and routes | Live |
| Offline | Moldova pack (139 MB transfer, 204 MB installed), OPFS install, service-worker range serving, offline search; Romania pack in the catalog | Live web; native map server |
| Account | Active sessions with revoke, display name, diagnostics report, delete account | Live |
| Admin | Role-gated overview, users, diagnostics triage and append-only audit | Implemented; demo needs DB promotion |
| Clients | Web, desktop, Android APK (`arm64-v8a` + `armeabi-v7a`), native map server | v2.6.1 |

The native client's Today tab shows the trail and text notes. Photos remain
web-only because the native build has no image picker/decoder; the note list
labels a photo attachment and points to the web client.

## 3. Architecture

```mermaid
flowchart TB
  subgraph Clients["Clients"]
    Web["Web app<br/>vanilla ES modules + MapLibre/PMTiles"]
    Native["Desktop / Android<br/>Rust + eframe/egui"]
    MapWin["Native map server<br/>/map/* + packs"]
  end

  API["Rust API<br/>Axum /api/v1<br/>session + CSRF + rate limits"]
  PG[("PostgreSQL<br/>users, sessions, friendships,<br/>shares, locations, history, notes,<br/>check-ins, emergencies, diagnostics")]
  Tiles["OpenFreeMap<br/>online basemap"]
  OSM["OSM services<br/>Nominatim / OSRM"]
  Packs["pinpoint-maps<br/>catalog + country packs"]

  Web -->|"HTTPS, HttpOnly cookie + X-CSRF-Token"| API
  Native -->|"same wire contract"| API
  Web -->|"tiles"| Tiles
  Web -->|"search / route"| OSM
  MapWin -->|"catalog + range requests"| Packs
  API --> PG
```

The API is a single Rust service (the binaries can also run split by service:
auth, users, locations, map, diagnostics, admin). PostgreSQL is authoritative;
the in-memory store is used by unit tests and local demo mode. Every request
goes through the same middleware order: security headers, session validation,
CSRF check on unsafe methods, input validation, authorization, then the
handler.

### Request flow for a friend location read

```mermaid
sequenceDiagram
  participant A as Alice (owner)
  participant API as Pinpoint API
  participant DB as PostgreSQL
  participant B as Bob (friend)

  A->>API: POST /api/v1/locations/shares<br/>{friendship, exact, 1 hour}
  API->>DB: INSERT share (owner = Alice)
  API-->>A: 201 + active share
  B->>API: GET /api/v1/locations/friends
  API->>DB: accepted friendship + active shares + latest locations
  API-->>B: 200 only if the share is Alice's, active,<br/>unexpired and to_friend/mutual
  Note over API,B: A paused/revoked share, an expired share,<br/>or a missing friendship returns no location.
```

## 4. Consent and security model

**Directional.** A share belongs to one owner and one friendship. Alice sharing
with Bob never implies Bob shares with Alice; `from_friend` rows are hidden from
"Who can see me".

**Precision.** `exact`, `approximate` (0.01 degree) and `city_only` (0.1
degree). The precision is applied **on read** by the server, and an unknown
precision is rejected rather than falling through to exact.

**Expiry and pause.** `expires_at` is checked on every read; a paused share is
inactive. The UI preserves the original expiry when pausing and never rewrites
precision on a pause.

**Ownership.** Only the owner can pause, resume, edit or delete a share. A
missing or foreign share returns the same not-found envelope.

**Private locations.** Check-ins and emergency alerts require authentication
and are owner-scoped; another user cannot list or delete them.

**Sessions.** Argon2id password hashes, opaque 256-bit server-side sessions,
`HttpOnly` + `SameSite=Lax` + `Secure` cookies in production, a CSRF token
bound to the session and echoed in `X-CSRF-Token` on every unsafe request.
Sessions are revocable; "sign out everywhere" and password changes invalidate
them. Login and registration are rate-limited and lock out repeated failures.

**Data minimization.** The server stores the latest location plus a bounded
history (24 hours visible; 7 days retained) and private notes. It does not
store continuous background tracking. Friend coordinates are never sent to
Nominatim/OSRM/Overpass; those calls receive only the query or the route
endpoints.

**Transport.** HTTPS off loopback; plain HTTP is only accepted for loopback
development (ADR-0006). The web client is same-origin, with a strict CSP,
`frame-ancestors 'none'`, no third-party scripts and no trackers.

## 5. Data and persistence

Migration 16 adds `pinpoint_location_history` and `pinpoint_place_notes`.
Earlier migrations add users, sessions, friendships, shares, locations,
check-ins, emergencies, counters, the rollback snapshot, diagnostics and the
admin audit.

| Table | Purpose | Retention / access |
|---|---|---|
| `pinpoint_users` | Account, role, password hash, MFA fields | Until account deletion |
| `pinpoint_sessions` | Opaque session id, CSRF token, activity | Idle/absolute expiry; revoked on logout |
| `pinpoint_friendships` | Requester/receiver/status | Until removed or account deletion |
| `pinpoint_shares` | Owner, friendship, direction, precision, expiry, active | Until revoked; expired rows fail closed |
| `pinpoint_locations` | Latest published location per user | Replaced on publish; deleted with account |
| `pinpoint_location_history` | Owner-only trail points | 7 days; 24 hours returned |
| `pinpoint_place_notes` | Private text/photo notes | Until the owner deletes them or the account |
| `pinpoint_checkins` / `pinpoint_emergencies` | Owner-scoped safety state | Owner-only; deleted with account |
| `pinpoint_diagnostic_reports` | User-initiated PII-free reports | Admin triage |
| `pinpoint_admin_audit` | Append-only admin actions | Read-only audit |

Direct writes are used for the security-sensitive rows that must survive a
multi-replica refresh: users, sessions, friendships, shares, the latest
location, the trail and notes. A whole-store snapshot no longer overwrites
these tables, so a stale replica cannot resurrect a revoked share or lose a
new publish.

## 6. Verification and evidence

The project is verified at four levels.

### 6.1 Build and test gate

```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
cargo test --locked
```

Latest result: **pass**. The library has **328 unit tests** and 24 test
binaries. Relevant suites:

| Suite | Result | What it pins |
|---|---|---|
| `tests/location_consent.rs` | 10 passed | friendship is not consent; exact precision; pause/revoke owner-only; expiry; invalid values; owner-scoped check-ins/emergencies |
| `tests/today.rs` | 2 passed | owner-only trail; owner-only notes; validation; unauthorised rejection |
| `tests/contract.rs` | 11 passed | auth envelope, CSRF double-submit, numeric IDs, new history/notes paths |
| `tests/assets.rs` | 11 passed | same-origin assets, CSP, service-worker cache rule, new UI wiring |
| `tests/router_probe.rs` | 17 passed | route table, health, auth gates, admin gates |
| `tests/account_delete.rs` | 1 passed | confirmation + password + cascade |
| `tests/auth_password.rs` | 1 passed | current-password requirement |

### 6.2 Live API checks

A two-account live run on the deployed service verifies:

- email registration and login;
- friend request and accept;
- publishing two exact points;
- the owner's 24-hour trail (exact coordinates), and the trail is empty for the
  other user;
- private note create/list/delete, owner-only, with another user receiving
  `400`;
- exact share visibility to the friend;
- pause hides the location, resume restores it, revoke removes it;
- unauthenticated check-ins/emergencies return `401`;
- account deletion invalidates the session and login.

Latest live run on **v2.6.1**: **14/14 functional checks** passed (two email
accounts; trail, notes, exact share, pause/resume/revoke and account deletion).
The only failed assertion was the script's expected version string (`2.6.0`),
while the server correctly reports `2.6.1`. An earlier demo-hardening run on
v2.5.2 passed **24/24** checks.

### 6.3 Browser checks

- Live web UI: **9/9** (empty-map prompt, online map, two-point trail, trail
  toggle, photo preview, note save + map marker, mobile no horizontal
  overflow).
- Offline web: **7/7** (Moldova install, offline reload, offline map, offline
  search).
- Sharing UI unit checks: **17/17**.

### 6.4 Artifact checks

- Release `v2.6.1`: `pinpoint-android-2.6.1.apk`, **13,632,525 B**,
  versionName `2.6.1`, versionCode `20601`, `arm64-v8a` + `armeabi-v7a`,
  signed with the Pinpoint release key (SHA-256
  `2b326ca140f5d34ac61672fb3a5ec2596ced91f9a2fd618d50b52fe89dea7e49`).
- Live `/health`: version `2.6.1`, PostgreSQL connected.
- CI: Rust, wasm, map QA and offline data checks pass. The macOS job has an
  unrelated flaky `offline_install` test; it is not part of the demo path.

## 7. Demo and presentation

- **Demo script:** [`Demo-Script.md`](./Demo-Script.md) - pre-flight, two
  browser profiles, exact run of show, expected values, fallbacks, phone
  install.
- **Presentation:** [`Presentation.md`](./Presentation.md) - 13-slide deck with
  speaker notes, timing and Q&A preparation.
- **Printable deck:** [`slides.html`](./slides.html) (open in a browser and
  print to PDF; A4 landscape).
- **Screenshots:** [`assets/`](./assets) - login, map, Today, notes, mobile,
  native theme, offline pack, offline route.

The 8-minute run is: sign in; online map + Chișinău -> Bălți route; consent
share with exact precision and 1 hour; friend sees exact coordinates; pause and
revoke; Today trail; private photo note; offline reload + search; native APK.
The demo uses **real accounts registered on the live system**, not seeded demo
rows.

## 8. Requirements traceability

| Requirement | Evidence |
|---|---|
| Register, login, logout, session control | `tests/contract.rs`, `tests/auth_password.rs`, live UI |
| Friends by handle; accept/reject/remove | `tests/router_probe.rs`, live UI, `tests/location_consent.rs` |
| Friendship alone grants no location | `tests/location_consent.rs::friend_locations_require_an_active_share` |
| Explicit, directional share with precision and expiry | `src/server/locations_handlers.rs`, `tests/location_consent.rs`, live UI |
| Pause/resume/revoke; owner-only | `tests/location_consent.rs`, live 24/24 |
| Exact coordinates are not rounded | `tests/location_consent.rs::exact_share_keeps_full_precision` |
| Private 24-hour trail | `tests/today.rs`, `docs/midterm2/assets/03-today.png` |
| Private text/photo notes | `tests/today.rs`, `assets/04-notes.png` |
| Map, search, route | live OpenFreeMap/Nominatim/OSRM, `assets/02-map.png`, `assets/08-route.png` |
| Offline pack, map and search | `assets/07-offline.png`, offline 7/7 |
| Account deletion and session revocation | `tests/account_delete.rs`, live erasure |
| Diagnostics and admin gates | `tests/router_probe.rs`, `src/server/admin_handlers.rs` |
| Security headers, CSP, same-origin assets | `tests/assets.rs`, `src/server/web_handlers.rs` |

## 9. Limitations and risks

- **Admin console needs DB access.** There is no seeded admin and no HTTP
  bootstrap. An existing account must be promoted with
  `pinpoint-admin --promote-email <email>` and `DATABASE_URL`; until then the
  Admin tab is hidden.
- **MFA enrolment is not in the web UI.** The login form accepts TOTP, but
  enrolment is desktop/API only. There is no email password reset.
- **Offline web directions are unavailable.** The web client always calls
  `/map/route`; the offline pack supports map + search. The native map server
  routes on the installed graph.
- **Native photos are not implemented.** The web client supports photo notes;
  the native client sends text notes and labels attached photos.
- **No continuous background tracking.** The trail records explicit publishes,
  not a 24/7 GPS trace.
- **Anonymous accounts are not demo-safe on the multi-replica deployment.**
  Use email accounts.
- **macOS CI flake.** `offline_install::tests::installs_two_regions_into_separate_directories`
  is timing-sensitive on the macOS runner; Linux/Windows and the release job
  pass.
- **Scale.** The service is a modular monolith with a per-request convergence
  refresh; it is sized for a demonstration and small trusted groups, not a
  global consumer workload.

## 10. Next steps

1. Promote and verify the admin console with the deployment database; add a
   documented bootstrap path that does not require direct SQL.
2. Web MFA enrolment and account recovery by email.
3. Friend-visible notes with per-note consent, or explicit per-note sharing.
4. Offline web routing via a WASM router or a native bridge.
5. Mobile photo picker + image decoding.
6. Replace the per-request full refresh with targeted queries and a shared
   session/rate-limit cache; load-test the multi-replica path.
7. Fix the macOS installer test race and make all CI jobs deterministic.
8. Add a background-location design with explicit consent, retention and a
   visible indicator before claiming a "daily tracer" beyond explicit
   publishes.

---

## Appendix A - Primary endpoints

```
POST /api/v1/auth/register        POST /api/v1/auth/login
POST /api/v1/auth/logout          GET  /api/v1/auth/me
GET  /api/v1/auth/sessions        POST /api/v1/auth/logout-all
POST /api/v1/friendships/         GET  /api/v1/friendships/
POST /api/v1/friendships/:id/respond
POST /api/v1/locations/me         GET  /api/v1/locations/me
GET  /api/v1/locations/friends
GET  /api/v1/locations/history
GET/POST /api/v1/locations/notes  DELETE /api/v1/locations/notes/:id
GET/POST /api/v1/locations/shares PATCH/DELETE /api/v1/locations/shares/:id
GET/POST /api/v1/locations/check-ins
GET/POST /api/v1/locations/emergency
POST /api/v1/diagnostics/report
GET  /api/v1/admin/overview|users|diagnostics|audit
```

## Appendix B - Reproduce the gate

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
cargo test --locked
```

Live health:

```bash
curl https://pinpoint-uj4y.onrender.com/health
# {"database":"connected","mode":"postgres","status":"healthy","version":"2.6.1"}
```

---

*Prepared for Midterm 2. All screenshots are from the deployed v2.6.0/v2.6.1
system; the native screenshot is the v2.6.0 theme build.*
