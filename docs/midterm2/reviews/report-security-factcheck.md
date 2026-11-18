# Security and privacy fact-check - Midterm 2 report

**Checked:** 2026-11-18, branch `docs/midterm2` (HEAD `5285bc6b`).
**Report version checked:** the current working-tree
`docs/midterm2/Midterm2-Report.md` (326 lines; `git diff --stat` shows 25
insertions / 14 deletions against HEAD). The file changed while this review was
in progress, so this review checks the current version, not the HEAD version.
Earlier wording that claimed `Secure` cookies or rate-limited registration has
already been corrected or caveated in the current text.
**Method:** static review only. No existing file was edited, no code changed,
no test was re-run, and no live endpoint was queried.
**Verdict: mostly accurate, not fully clean.** The consent/authorization core is
accurate and is backed by real tests. Two claims are still **wrong** and nine are
**overstated**; most of the overstatements are wording. The report already
discloses the main gaps (no `Secure`, no HSTS, no per-IP limit, no API security
headers) in section 9. Fix the absolute-expiry row and the OSM sentence before
submission.

## Claims that now hold (credit where due)

These were wrong or overstated in the earlier draft and are now consistent with
the code:

- The report says the session cookie is `HttpOnly; SameSite=Lax` but **not**
  `Secure`, has no `__Host-` prefix and no HSTS (report l.165-167, l.327-330).
  That matches `src/server/auth_handlers.rs:28-35`; there is no `Secure` flag in
  the app and no HSTS in the Rust server.
- The report says there is **no per-IP server-side rate limit**, only a login
  lock after five failures (report l.167, l.331-332). That matches
  `src/server/auth_handlers.rs:95-136` and the absence of any rate-limit layer
  in `src/server/mod.rs:148-248`; `src/security/rate_limit.rs` is used by the
  desktop client, not the API.
- The report documents the CSP relaxations: inline styles for MapLibre and
  `wasm-unsafe-eval` for SQLite (report l.178-179). That matches
  `src/server/web_handlers.rs:74-79`.
- The report says JSON API responses do not carry the HTML security-header set
  (report l.119, l.308, l.327-330). That is accurate: `secure()` is applied by
  the web/status handlers, not to JSON API responses.

---

## 1. Authentication and sessions

| Claim (report) | Evidence | Verdict |
|---|---|---|
| Argon2id password hashes (report l.161) | `src/security/password.rs:24-35` uses `Argon2::default()`; the crate default is Argon2id v19, m=19456 KiB, t=2, p=1 (`argon2` 0.6.0 `src/params.rs:48-67`; `Cargo.lock:252-253`). `password.rs:107-114` asserts the `$argon2id$` prefix and round-trip. | **PASS** |
| Opaque 256-bit server-side sessions (report l.161) | `src/server/state/sessions.rs:24-35`: 32 random bytes (256 bits), stored as a SHA-256 hash, not the cookie value. `sessions.rs:253-256` pins the hash-not-raw-id property. | **PASS** |
| `HttpOnly` + `SameSite=Lax` cookies (report l.162) | `src/server/auth_handlers.rs:28-31`: `session_id=...; Path=/; HttpOnly; SameSite=Lax`. Clearing uses the same flags plus `Max-Age=0` (`auth_handlers.rs:33-35`). | **PASS** |
| No `Secure`, no `__Host-`, no HSTS (report l.165-167, l.327-330) | Both cookie builders omit `Secure` (`auth_handlers.rs:28-35`); no `Strict-Transport-Security` is emitted by the Rust server; the HTTPS compose profile adds HSTS at nginx (`docker/nginx-gateway-https.conf:17`), but the live `pinpoint-server` does not terminate TLS itself. | **PASS** (accurate disclosure) |
| Session table: "Idle/absolute expiry" (report l.191) | `src/server/state/sessions.rs:40-47` is a sliding idle TTL (default 720 min); `sessions.rs:61-63` tests only `last_activity`. `pinpoint_sessions` stores `created_at` but it is never checked (`src/server/migrations.rs:112`). No absolute lifetime exists. | **WRONG** |
| CSRF token bound to the session, required in `X-CSRF-Token` on unsafe methods (report l.162-163) | `src/server/mod.rs:84-112` loads the session, rejects non-GET/HEAD/OPTIONS without an exact token, and injects the session user. The web client sends the header on non-GET (`web/app/app.js:39`); `tests/router_probe.rs:214-242` and `tests/contract.rs:88-112` pin it. | **PASS** |
| Login locks an account after five failures (report l.164, l.331) | `src/server/auth_handlers.rs:95-136`: five failed attempts set `locked_until` to now + 15 minutes; a locked account gets HTTP 429. | **PASS** |
| No per-IP server-side rate limit (report l.167, l.331-332) | No rate-limit middleware or per-IP limiter exists in `src/server/mod.rs:148-248` or `src/bin/auth.rs:25-33`; registration has no limiter (`auth_handlers.rs:174-268`). The only inbound throttles are the login lockout and a 2-permit routing semaphore (`src/server/mod.rs:1053-1063`). | **PASS** (accurate disclosure) |
| "Sessions are revocable; sign out everywhere and password changes invalidate them" (report l.163-164) | Single-process path is correct: logout/logout-all delete local and DB rows (`auth_handlers.rs:271-286,564-580`), password change revokes all sessions (`auth_handlers.rs:479-488`), and `delete_user_sessions` removes both copies (`sessions.rs:199-210`). But `get_session` returns a locally cached session without re-reading Postgres (`sessions.rs:107-116`), and `converge` refreshes users/friendships/shares/locations, not sessions (`state/mod.rs:218-243`). On a second replica that already cached the session, a logout-all/password change on replica A does not invalidate it until its idle timeout. | **OVERSTATED** |
| Architecture diagram: "session + CSRF + rate limits" (report l.101) | No per-IP/per-user rate-limit middleware exists. The label is broader than the implementation; the body and section 9 are more precise. | **OVERSTATED** |

---

## 2. Consent, precision, expiry and ownership

| Claim (report) | Evidence | Verdict |
|---|---|---|
| Directional share; friendship is not consent; `from_friend` rows hidden from "Who can see me" (report l.143-145) | `src/server/locations_handlers.rs:19-38` requires an accepted friendship **and** an active, unexpired share owned by the location owner, and excludes `direction == "from_friend"`. `list_shares` filters the owner's view at `locations_handlers.rs:170-175`. `tests/location_consent.rs:78-190` proves an accepted friendship alone returns nothing; `:589-627` proves `from_friend` is not a grant. | **PASS** |
| Exact/approximate/city-only; 0.01 and 0.1 degree; applied on read; unknown precision rejected rather than exact (report l.147-149) | `locations_handlers.rs:40-49` (0.01 = 2 dp, 0.1 = 1 dp); create/update validate the three values (`:77-88`, `:207-215`); `GET /locations/friends` applies the share's precision on read (`:596-607`). Unknown stored values degrade to `city_only`, never `exact` (`:45-48`), and are excluded by `visible_share` (`:31`). `tests/location_consent.rs:629-677` pins exact coordinates. | **PASS** |
| Expiry checked on every read; a paused share is inactive (report l.151-153) | `visible_share` checks `is_active` and parses `expires_at` fail-closed (`locations_handlers.rs:25-37`); `share_json` reports `is_active && !is_expired` (`src/server/handlers.rs:59-71`). `tests/location_consent.rs:408-467` covers expired shares and resume attempts. | **PASS** |
| UI preserves the original expiry when pausing and never rewrites precision on a pause (report l.152-153) | `web/app/sharing.js:71-90`: pause returns only `{ is_active: false }`; resume copies `current.expires_at` and the form precision. | **PASS** |
| Only the owner can pause, resume, edit or delete a share; missing/foreign share returns the same not-found envelope (report l.155-156) | PATCH and DELETE locate by id and require `owner_id == cu.user_id`, otherwise return `SHARE_NOT_FOUND` (`locations_handlers.rs:190-195`, `:284-293`). `tests/location_consent.rs:286-358` and `:470-536` cover foreign delete/PATCH. The envelope is the generic `error_envelope`, which is HTTP **400**, not 404 (`src/server/mod.rs:1170-1177`). | **PASS** (note the 400 status) |
| Check-ins and emergency alerts require authentication and are owner-scoped; another user cannot list or delete them (report l.158-159) | `require_current_user` returns 401 (`src/server/handlers.rs:13-26`); check-in list/delete filter/guard on `user_id` (`locations_handlers.rs:700-750`); emergency list/resolve are owner-scoped (`:820-875`). `tests/location_consent.rs:679-780` covers 401 and cross-user 400. There is no emergency delete endpoint; resolve is the owner-only mutation. | **PASS** |
| Account deletion revokes the session and login (report l.247, l.306) | `purge_user` removes the user and the relationship/location rows (friendships, shares, locations, check-ins, emergencies, history, notes) (`src/server/users_handlers.rs:305-327`); `sql_delete_user` also deletes reports, audit rows, sessions and the rollback snapshot (`src/server/state/users.rs:261-321`). `tests/account_delete.rs:36-149` covers confirmation, password proof, session invalidation and login failure. Same-process claim is correct; the cross-replica cache caveat is in row 9 above. | **PASS** (single-process) |
| "The server enforces every access decision; the client is never trusted" (report l.61-62) | Handlers use `cu.user_id`, never a body-supplied user id; ownership checks are server-side (`locations_handlers.rs`, `users_handlers.rs`). The multi-replica session cache is a server-side gap, not a client-trust gap. | **PASS** |

---

## 3. Data, privacy and retention

| Claim (report) | Evidence | Verdict |
|---|---|---|
| Latest location + bounded history + private notes; no continuous background tracking (report l.169-171) | `publish_location` replaces one `pinpoint_locations` row and appends to history (`locations_handlers.rs:300-358`); history is only written on an explicit publish (`src/server/state/locations.rs:232-277`); notes are explicit creates (`locations_handlers.rs:421-493`). No background tracker exists. | **PASS** |
| "7 days; 24 hours returned" and "7-day server retention" (report l.79, l.195) | `GET /locations/history` returns a 24 h window (`locations_handlers.rs:370-373`). Old rows are deleted only when the same user publishes again (`state/locations.rs:269-276`); there is no scheduled sweep, and the in-memory path has no pruning at all (`:253-255`). A user who stops publishing keeps older rows. | **OVERSTATED** |
| Private notes are owner-only and bounded to 675 KB (report l.80) | Create/list/delete filter on `user_id` (`locations_handlers.rs:495-576`); `tests/today.rs:121-226` covers owner-only list/delete and validation. The server cap is 900,000 base64 characters (`MAX_PHOTO_CHARS`, `locations_handlers.rs:410-418`), which is about 675 KB decoded; the browser downscales to at most 1024 px before encoding (`web/app/app.js:498-513`). | **PASS** |
| "Friend coordinates are never sent to Nominatim/OSRM/Overpass; those calls receive only the query or the route endpoints" (report l.171-173) | Search sends the query plus a fixed Moldova viewbox (`src/osm.rs:323-360`); reverse sends the tapped point (`:364-370`); route sends the two endpoints (`:377-454`). The native "Navigate to friend" action sets the friend's received coordinates as the route destination (`src/ui/map.rs:360-404`) and the route sheet sends those endpoints (`src/ui/route.rs:752-770`). Those coordinates therefore reach OSRM (and the server's `/map/route` proxy) when the user explicitly routes to a friend. No friend coordinates are sent by share reads, search or background refresh. | **WRONG** |
| Direct writes for "users, sessions, friendships, shares, the latest location, the trail and notes"; a whole-store snapshot no longer overwrites these tables (report l.201-205) | Sessions, friendships, shares, locations, trail and notes are direct-write-only: `sessions.rs`, `friendships.rs:41-49`, `state/locations.rs:148-176`, `record_location` `:232-277`, `sql_insert_note` `:281-300`. **Users are not**: `persist_users` upserts every user row from the in-memory snapshot, including `password_hash`, `totp_secret`, `mfa_*`, `failed_login_attempts` and `locked_until` (`src/server/state/users.rs:88-119`). A stale replica can therefore overwrite a concurrent password/MFA/lockout change, although the per-request/per-5 s refresh narrows the window. | **OVERSTATED** |
| "Append-only admin audit" (report l.84, l.199) | The admin API has only `GET /admin/audit`; there is no update/delete route (`src/server/mod.rs:305-322`, `src/server/diagnostic_handlers.rs`). But account erasure explicitly deletes rows where the erased user was the actor (`state/users.rs:278`) or anonymises them when DELETE is denied (`:300-302`). "Append-only" is true through the API, not for erasure. | **OVERSTATED** |
| "User-initiated PII-free reports" (report l.198) | `diagnostic_handlers.rs:114-123` attaches the reporter's numeric id and public handle when signed in; the report row stores `user_id` and `username` (`src/server/state/diagnostics.rs`). No email, IP, coordinates or device id is stored, so it is pseudonymous, but not literally PII-free. | **OVERSTATED** |
| No demo accounts and no seeded pins; every account registered through the public flow (report l.289-290) | `Store::new` starts empty (`src/server/state/store.rs:65-79`); no seed path exists in `src/bin`; `demo_client` is a client, not a server-side seed (`src/bin/demo_client.rs:8`). An admin can create users through `POST /users/`, but no demo rows ship. | **PASS** |
| No real user data in tests (report l.65-66) | Tests use `.invalid` addresses and fictional names, e.g. `tests/location_consent.rs:83-85` and `tests/today.rs:82-83`. | **PASS** |

---

## 4. Browser, transport and headers

| Claim (report) | Evidence | Verdict |
|---|---|---|
| "HTML and static responses pass through the security-header middleware, session validation and CSP" (report l.117-119) | There is no security-header middleware. `session_auth` and `converge` are the only global layers (`src/server/mod.rs:148-248`); `secure()` is called per response by `web_handlers` (`src/server/web_handlers.rs:83-117`) and the status handlers (`src/server/status.rs:39,48`). The substantive claim (HTML/assets get the headers; JSON API does not) is correct, but the word "middleware" is not. | **OVERSTATED** |
| "JSON API responses currently do not carry the same header set" (report l.119, l.308, l.327-330) | API handlers return JSON without `secure()`; only web/status handlers apply CSP/XFO/nosniff. In compose, nginx adds a narrower CSP and XFO to API routes (`docker/nginx-gateway.conf:24-26`), but not the web CSP. | **PASS** (accurate disclosure) |
| Strict CSP, `frame-ancestors 'none'`, no third-party scripts and no trackers (report l.176-179) | `src/server/web_handlers.rs:74-79` sets `default-src 'self'`, `script-src 'self' 'wasm-unsafe-eval'`, `style-src 'self' 'unsafe-inline'`, `font-src 'self'`, `frame-ancestors 'none'`, `object-src 'none'`. `tests/assets.rs:178-247` pins the header, the single remote host and COEP/COOP/CORP. `tests/assets.rs:40-176` bans third-party asset hosts. "Strict" is relative (`unsafe-inline` styles), but the claimed properties hold and the report documents the relaxations. | **PASS** |
| "The web client is same-origin" (report l.176) | The CSP allows `https://tiles.openfreemap.org` in `img-src` and `connect-src` (`web_handlers.rs:74-79`), and the opt-in online basemap loads that host's style/tiles (`web/app/index.html:10`, `web/app/map.js:3,627`). App assets, API and OSM proxies are same-origin; the blanket phrase is not. | **OVERSTATED** |
| No trackers, no third-party scripts, no IP geolocation (report l.65-66) | `tests/assets.rs` bans analytics/CDN hosts and remote subresources; `web/app/app.js` has no remote fetches. No IP parsing/logging exists in `src` (no `ConnectInfo`/`remote_addr`/`X-Forwarded-For` use), and `loggable_uri` strips query strings (`src/server/mod.rs:116-119`). Note: the native client's once-a-day GitHub releases check is an automatic third-party request, disclosed in `docs/PRIVACY.md:24-29`; it is not analytics or a tracker. | **PASS** (with that note) |
| "HTTPS off loopback; plain HTTP is only accepted for loopback development" (report l.175-176) | `src/api.rs:24-76` accepts `https`, loopback `http`, and plain HTTP to hosts explicitly listed in `PINPOINT_ALLOW_PLAINTEXT_HOSTS` (internal container deployments). The Rust server does not redirect HTTP to HTTPS or emit HSTS; the HTTPS compose profile adds a 301 and HSTS at nginx (`docker/nginx-gateway-https.conf:4-17`), and the live platform proxy is outside this repo. | **OVERSTATED** |
| "HSTS is not sent" (report l.179, l.328) | No HSTS header is emitted by the Rust server. The HTTPS compose profile adds it at nginx, but that is not the live `pinpoint-server` path. The platform proxy could add it; not checked here. | **PASS** (accurate for the app) |

---

## 5. Test evidence (static counts only)

The suite counts in section 6.1 are correct as **function counts**:

| Suite | Report | Static count | Note |
|---|---|---|---|
| `tests/location_consent.rs` | 10 | 10 | matches |
| `tests/today.rs` | 2 | 2 | matches |
| `tests/contract.rs` | 11 | 11 | matches |
| `tests/assets.rs` | 11 | 11 | matches |
| `tests/router_probe.rs` | 17 | 17 | matches |
| `tests/account_delete.rs` | 1 | 1 | matches |
| `tests/auth_password.rs` | 1 | 1 | matches |

"24 test binaries" also checks out structurally: 13 `[[bin]]` targets in
`Cargo.toml`, plus the library test harness and 10 integration-test files =
24. The claim "328 unit tests" was **not** reproduced: a static count of
`#[test]`/`#[tokio::test]` attributes across all `cfg(target_os)` branches is
348, so 328 is plausible for one OS but is not proven here. No test was executed
for this review, so "10 passed" etc. are counts, not independently confirmed
results.

---

## 6. Exact corrections for the report

1. **Absolute expiry (l.191).** Replace "Idle/absolute expiry" with "Sliding
   idle expiry (12 h default, `PINPOINT_SESSION_IDLE_MINUTES`); no absolute
   lifetime". Code fix if an absolute cap is intended: enforce `created_at +
   max_lifetime` in `StoredSession::is_expired`.

2. **Friend coordinates and OSM (l.171-173).** Replace with: "Search sends only
   the query (plus a fixed Moldova viewbox); routing sends the two endpoints. A
   user-initiated 'Navigate to friend' uses the friend's received coordinates as
   the route destination, so those coordinates do reach OSRM; no friend
   coordinates are sent by share reads, search or background refresh."

3. **Users are not direct-write-only (l.201-205).** Replace "Direct writes are
   used for ... users ... A whole-store snapshot no longer overwrites these
   tables" with: "Direct writes are used for sessions, friendships, shares, the
   latest location, the trail and notes. `pinpoint_users` is still upserted from
   the in-memory snapshot in `persist_users`, including password/MFA/lockout
   fields, so it is not covered by the no-snapshot guarantee."

4. **Seven-day retention (l.79, l.195).** Replace "7 days; 24 hours returned"
   and "7-day server retention" with: "24 h returned; rows older than 7 days are
   pruned on the next publish (no scheduled sweep; the in-memory store does not
   prune)."

5. **Append-only audit (l.84, l.199).** Replace with: "Append-only through the
   admin API; account erasure deletes or anonymises rows where the erased user
   was the actor."

6. **PII-free diagnostics (l.198).** Replace with: "User-initiated pseudonymous
   reports: category, message, app version/platform, plus the reporter's numeric
   id and public handle when signed in; no email, IP, coordinates or device id."

7. **"Security-header middleware" (l.117-119).** Replace with: "HTML and static
   responses are served by the web handlers, which apply the security headers
   and CSP with `secure()`; all requests also pass through `session_auth`.
   JSON API responses currently do not carry the same header set." The rest of
   the paragraph is accurate.

8. **Diagram "rate limits" (l.101).** Change "session + CSRF + rate limits" to
   "session + CSRF + login lockout", or add the limiter. The body and section 9
   already say there is no per-IP limit.

9. **"The web client is same-origin" (l.176).** Replace with: "The app assets,
   API and OSM proxies are same-origin. The CSP allows
   `https://tiles.openfreemap.org` for the opt-in online basemap; no
   third-party scripts or trackers are loaded."

10. **Transport (l.175-176).** Replace "plain HTTP is only accepted for loopback
    development" with: "plain HTTP is accepted for loopback and for hosts
    explicitly listed in `PINPOINT_ALLOW_PLAINTEXT_HOSTS` (internal
    deployments); production relies on the TLS terminator. The Rust server does
    not redirect HTTP to HTTPS or set HSTS; the HTTPS compose profile does."

11. **Session revocation (l.163-164).** Scope the sentence to the single-process
    demo, or fix the code: "In the single-process demo, logout, logout-all and
    password changes revoke sessions immediately. Across replicas, a session
    cached by another replica can survive until its idle timeout because
    `get_session` short-circuits on the local cache and `converge` does not
    refresh sessions."

12. **Not-found status (l.156, precision only).** Say "the same
    `SHARE_NOT_FOUND` envelope (HTTP 400, not 404)" so a reader does not expect
    a 404.

13. **Precision read fallback (l.149, precision only).** Add: "On read, an
    unrecognised stored level degrades to `city_only`, never `exact`." The
    create/update path does reject unknown values, as written.

---

## 7. Additional implementation risk (not a report claim, but security-relevant)

`trigger_emergency` updates the in-memory latest location and sets every share
owned by the user to `is_active = true` (`src/server/locations_handlers.rs:775-817`),
but it does **not** call `sql_upsert_location` or `sql_upsert_share`.
`persist_locations_domain` deliberately does not write shares or locations
(`src/server/state/locations.rs:153-158`), so the next `refresh_from_db`
(per-request `converge` or the 5 s background loop) reloads the old location and
the old share flags. The emergency alert row itself does persist (it is upserted
by `persist_locations_domain`). Consequences: an emergency activation is
transient on Postgres, and an emergency cannot durably override a pause. If the
report presents emergency activation as durable, add this caveat or fix the
handler to write both rows directly.

---

## 8. Not verified here

These are evidence claims in sections 6.2-6.4 that cannot be checked from the
repository; they are not marked PASS.

- The 14/14 live API run and the earlier 24/24 demo-hardening run (l.249-253).
  No committed script output or log was found.
- The 9/9 browser, 7/7 offline and 17/17 sharing-UI checks (l.257-262).
- CI status, including the macOS `offline_install` flake (l.271-272).
- Live `/health` version `2.6.1` and PostgreSQL connectivity (l.270).
- The APK size, manifest, ABIs and signing certificate (l.266-269). These are
  covered separately by `docs/midterm2/reviews/artifact.md`, which found them
  correct.
- The 328-unit-test total and the "latest result: pass" claim (l.220). No test
  run was performed for this review.

---

*Static review of the current `docs/midterm2/Midterm2-Report.md` against the
tree. No existing file was modified; this review is the only file added.*