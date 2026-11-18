# Pinpoint — Midterm 2 handout

**Privacy-first location sharing for trusted friends.** Version **2.6.1** · 18 November 2026
Live: https://pinpoint-uj4y.onrender.com · Source: https://github.com/Prince-of-Nothing/Pinpoint
Release / APK: https://github.com/Prince-of-Nothing/Pinpoint/releases/tag/v2.6.1

## Problem and rule

Location sharing is useful for safety, but common products fail in three ways:
friendship is treated as permanent access; stale last-known dots look live; and
pausing, revoking or narrowing precision is buried or missing.

**The rule: friendship is not consent.** A friendship grants no location access
until the owner creates an explicit, one-directional share with a chosen
precision and an optional expiry. The server decides every read; the client is
never trusted.

## Delivered scope (v2.6.1)

| Area | Delivered |
|---|---|
| Identity | Email registration/login, optional TOTP, opaque sessions, sign-out everywhere, password change, account deletion |
| Friends | Request by exact handle, accept/reject/remove; emails are not searchable by other residents |
| Consent | Share sheet with named friend, precision, duration, confirmation summary; active/paused/expired/revoked state |
| Location | Manual or GPS publish; a friend reads only through an active, unexpired, owner-created share |
| Private trail | Owner-only 24-hour trail, listed and drawn on the map; pruned to 7 days on the next publish |
| Private notes | Label + message + optional JPEG/PNG photo, downscaled in-browser, server-enforced at about 675 KB decoded, owner-only (web; text-only native) |
| Map | OpenFreeMap basemap, OSM search, OSRM driving directions, saved/recent places and routes |
| Offline | Moldova native pack: 138.5 MB transfer / 204.0 MB installed; web OPFS pack: 220.3 MB; OPFS install, service-worker range serving, offline search. Romania is not in either client catalog |
| Account | Active sessions with revoke, display name, diagnostics report, delete account |
| Admin | Role-gated overview, users, diagnostics triage, append-only audit (needs DB promotion) |
| Clients | Web, desktop, Android APK (`arm64-v8a` + `armeabi-v7a`), native map server |

## Architecture in text

Three client surfaces: the web client (vanilla ES modules +
MapLibre/PMTiles), the desktop/Android client (Rust + eframe/egui, same wire
contract), and a native Rust map server serving `/map/*` from installed packs.
All clients call a **Rust/Axum API** at `/api/v1`; the same crate also builds
split service binaries (auth, users, locations, map, diagnostics, admin).

**PostgreSQL is authoritative:** users, sessions, friendships, shares, latest
location, trail, private notes, check-ins, emergencies, diagnostics, admin
audit. An in-memory store serves unit tests and local demo mode. HTML and static
responses are sent by handlers that apply the security headers, session
validation and CSP; JSON API responses do not carry the same header set.
Handlers then validate input, check authorization and write the response.

External services are limited to map data: OpenFreeMap tiles, Nominatim search
and OSRM routing. Country packs come from `pinpoint-maps`. The friend list,
account data and trail are never sent to those services; a search sends only the
query, and a route sends only the endpoints being routed. Transport is HTTPS off
loopback; plain HTTP is accepted only for loopback development or an explicitly
allowlisted internal host (ADR-0006).

<div style="page-break-after: always;"></div>

## Consent and security invariants

- **Directional.** A share belongs to one owner and one friendship. Alice
  sharing with Bob never implies Bob shares with Alice.
- **Precision on read.** `exact`, `approximate` (0.01°) and `city_only` (0.1°)
  are applied by the server; an unknown precision is rejected, never treated as
  exact.
- **Expiry and pause fail closed.** `expires_at` is checked on every read; a
  paused share is inactive; pausing preserves the original expiry. Exception:
  triggering an emergency alert reactivates the caller's existing shares,
  including paused ones, until changed again; deleted shares stay gone. This is
  a known gap.
- **Owner-only control.** Only the owner can pause, resume, edit or delete a
  share. A missing or foreign share returns the same not-found envelope.
- **Owner-scoped safety state.** Check-ins and emergency alerts require
  authentication; another user cannot list or delete them.
- **Sessions.** Argon2id password hashes, opaque 256-bit server-side sessions, `HttpOnly` + `SameSite=Lax` cookies, a session-bound CSRF token echoed in `X-CSRF-Token`. Sign-out everywhere and password changes revoke sessions in the database; another replica may accept a cached copy until its idle timeout. Login locks an account after five failures; there is no per-IP server-side rate limit. The current build does not set `Secure`, has no `__Host-` prefix and no HSTS.
- **Data minimization.** Latest location plus a bounded history (24 h visible,
  7 d retained) and private notes. No continuous background tracking by
  default; opt-in Android interval sharing shows a persistent notification.
- **Web hardening.** Same-origin app assets, API and OSM proxies; strict CSP,
  `frame-ancestors 'none'`, no third-party scripts, no trackers, no IP
  geolocation. The opt-in basemap loads OpenFreeMap tiles. OSM attribution is
  always visible (ODbL).

## Verification

| Check | Result |
|---|---|
| Build/test gate (`fmt`, `clippy -D warnings`, `build --locked`, `test --locked`) | Pass |
| Library unit tests | **328** tests, 24 test binaries |
| Live functional run on v2.6.1 (two email accounts) | **14/14**, manual, transcript not archived |
| Live web UI checks | **9/9**, manual, transcript not archived |
| Offline web checks (install, offline map, local search; no reload) | **7/7**, manual, transcript not archived |
| Sharing UI unit checks | 17/17 |

The live run covers registration/login, friend request/accept, two exact
publishes, owner-only trail, owner-only note create/list/delete, exact share
visibility, pause/resume/revoke, unauthenticated check-in/emergency rejection
and account deletion. It was a manual run; no transcript is archived. The only
failed assertion was the script's expected version string (`2.6.0`) while the
server correctly reports `2.6.1`.

Contract and consent suites pin the wire format and the rule: `tests/contract.rs`
(11), `tests/location_consent.rs` (10), `tests/router_probe.rs` (17),
`tests/assets.rs` (11). Live health returns
`{"database":"connected","mode":"postgres","status":"healthy","version":"2.6.1"}`.

## Live system and release

- App: https://pinpoint-uj4y.onrender.com — health at `/health`
- Release: `v2.6.1`, APK `pinpoint-android-2.6.1.apk`, 13,632,525 B,
  versionName `2.6.1`, versionCode `20601`, `arm64-v8a` + `armeabi-v7a`, signed
  with the Pinpoint release certificate (SHA-256 `2b326ca140f5d34ac61672fb3a5ec2596ced91f9a2fd618d50b52fe89dea7e49`)
- No seeded accounts, demo pins or shared passwords; demo accounts are
  registered through the public flow.

## Known limits

- Admin has no seeded user and no HTTP bootstrap; an existing account must be
  promoted with `pinpoint-admin --promote-email` and `DATABASE_URL`.
- MFA is accepted at login but enrolled only on desktop/API; no email password
  reset.
- Offline web supports map and search, not directions; the native map server
  routes on the installed graph.
- Native photos are not implemented; the native note list labels an attached
  photo and points to the web client.
- The trail records explicit publishes, not a 24/7 GPS trace; Android interval
  sharing is opt-in and shows a notification.
- Anonymous accounts are not demo-safe on the multi-replica deployment; use email accounts.
- Transport/cookie hardening is incomplete: the live cookie is not `Secure` and has no `__Host-` prefix; HSTS and API JSON security headers are missing. Fix before real accounts.
- Auth abuse limits are per account, not per IP; register and anonymous signup have no server-side IP limit.
- CI: the v2.6.1 tag run fails only the dependency audit; build, test, wasm,
  macOS and Windows pass. Nine unmaintained/unsound transitive crates with no
  fixed upgrade are recorded as accepted risk in `audit.toml`; no known
  vulnerability is open.
- The API is a modular monolith with a per-request convergence refresh, sized
  for a demonstration and small trusted groups, not a global workload.

## Demo flow (5 lines)

1. Sign in as the owner; show the online map and the Chișinău → Bălți route
   (136.4 km, 2 h 04).
2. As the friend, open Sharing and create an **exact**, **1 hour** share; read
   the friend/precision/expiry summary aloud and save.
3. As the owner, reload and show the friend's exact `47.0245, 28.8322`; then in
   the friend window press **Pause** (gone), **Resume** (back), **Revoke** (gone
   for good).
4. As the owner, open **Today** for the two-point private trail and **Private
   notes** for the photo note; state that both are owner-only.
5. Go offline, reload, show the installed Moldova map and an offline search for
   Bălți; close on the signed 2.6.1 APK and the live `/health` version.
