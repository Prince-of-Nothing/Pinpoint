# Pinpoint — Threat Model (DSA PBL, STRIDE + DREAD + PASTA)

**Status:** v2.0 for Midterm 2 (consolidated from `docs/security-architecture.md` §2 + `docs/SECURITY.md`).
**Method:** Problem-based learning secure development. Definitions below follow the course slides
(Threat Modeling, Vasile Drumea, 2022). Evidence column links each defence to code/tests.
**Scope:** Rust multiplatform client + Rust modular services (`auth/users/locations/map` behind nginx
gateway) + Postgres (`pinpoint_state`), OSM integrations (Nominatim/OSRM/Overpass). No trackers.

## 1. Definitions (course terminology)

- **Asset** — something of value to protect. Pinpoint assets: current publishes (lat/lon + accuracy
  + timestamp per user), account credentials, session/CSRF tokens, share consent records,
  friendship relations, check-ins/emergency alerts, audit logs, OSM attribution/compliance budget.
- **Risk** — asset + threat × vulnerability × impact (unwanted outcome if threat succeeds).
  Risk = Likelihood × Impact (guideline formula 5.1).
- **Threat** — potential attacker action/condition (e.g. "attacker replays a publish request").
- **Vulnerability** — weakness that lets a threat succeed (e.g. missing ownership check → IDOR).
- **Threat modelling** — structured analysis answering: what are we building, what can go wrong,
  what are we doing about it, when can we stop. We use **STRIDE** (identify per flow),
  **DREAD** (rank), **PASTA** (business risk view), **attack trees** (decompose goals).

## 2. Trust boundaries & data flows

```
[User device: Rust client] --TLS--> [Gateway: nginx] --/
    | local only                      | strip/fwd X-Real-IP   |
    | bookmarks, settings             v                        v
    | (never synced)          [auth 8001] [users 8002] [locations 8003] [map 8004]
    |                                \_______ shared Store/Postgres _______/
    | OSM: user-typed queries only (never friend coords)
    +--> Nominatim (search/reverse) / OSRM (route) / Overpass (POI)
```

Boundaries: **B1** device↔gateway (TLS/mTLS future), **B2** gateway↔services (internal net),
**B3** services↔Postgres (least-privilege creds), **B4** client↔OSM (privacy envelope),
**B5** on-device storage (bookmarks at rest).

## 3. PASTA (risk-centred snapshot)

| Stage | Pinpoint answer |
|---|---|
| 1. Define objectives | Privacy-first location sharing; no stalking use; GDPR data minimisation. |
| 2. Define technical scope | 4 Rust services + gateway + Postgres; OSM read-only. |
| 3. Decompose | DFD above; 12 use cases (auth, share, publish, search, route, POI, bookmark, check-in, emergency, admin). |
| 4. Analyse threats | STRIDE table §4 (24 threats). |
| 5. Detect vulns | Code review, `cargo test` contract/IDOR tests, `cargo-audit`, SAST (`clippy -D warnings`). |
| 6. Impact analysis | Location leak = physical safety risk → highest DREAD. |
| 7. Risk treatment | Prioritised defences §6; residual risks §7 with owners/dates. |

## 4. STRIDE threat register (prioritised by DREAD)

DREAD 1–3 each (Damage, Reproducibility, Exploitability, Affected users, Discoverability); priority
= mean. High ≥2.6, Med ≥2.0. `R = L × I` checked against DREAD.

| ID | STRIDE | Threat (attacker goal → step) | Asset / component | Vuln if defence missing | DREAD | Countermeasure (implemented) | Evidence |
|---|---|---|---|---|---|---|---|
| T-S1 | Spoofing | Credential stuffing on `/auth/login` | Accounts / auth svc | No throttling | 2.8 H | Generic errors; login lockout (5 failures/15 min); per-client HMAC throttle 30/5 min; Argon2id | `security/validation.rs`, `server/state/rate_limit.rs`, `tests/contract.rs::rejected_login_*` |
| T-S2 | Spoofing | Session steal (XSS/mitm) → impersonate | Session / gateway | Plain HTTP, JS-readable cookie | 2.6 H | HttpOnly+SameSite=Lax+Secure cookies; HTTPS-only off-loopback (ADR-0006 `validate_backend_url`); rotation; logout confirmed | `api.rs`, `security-architecture §3–4` |
| T-T1 | Tampering | Forge publish/share via CSRF | Locations/shares | No CSRF token | 2.7 H | Double-submit `X-CSRF-Token` on every unsafe method; GETs never send CSRF | `api.rs::request`, `unsafe_requests_send_csrf_*` |
| T-T2 | Tampering | IDOR: `GET /users/{id}`, `PATCH shares/{id}` for other user | Locations / users svc | Missing ownership check | 3.0 H | Server-side consent check (friendship + active unexpired share + recipient); generic 403/404; validation at boundary | `server/handlers.rs`, `contract.rs` |
| T-T3 | Tampering | Coordinate out-of-range / NaN publish | Locations | No range check | 2.2 M | `validate_location` (lat ±90, lon ±180, finite, accuracy ≥0) client + server | `validation.rs`, `handlers::publish_location` |
| T-R1 | Repudiation | User denies share/emergency action; no audit | Accountability | No logging | 2.0 M | `AuditLogger` (auth/friends/publish + share/emergency); never logs secrets/coords; append-only file sink | `security/audit.rs`, `app.rs` |
| T-I1 | Info disclosure | Stalking via forgotten/over-broad grants | Sharer location | Blanket visibility | 2.9 H | Directional shares + precision (exact/approx/city_only) applied server-side; pause/resume/delete; expiry (`duration_hours`/`expires_at`); latest publish plus an owner-only 7-day trail pruned hourly | `models.rs::LocationShare`, `handlers::list_friend_locations` |
| T-I2 | Info disclosure | Friend coords leak to Nominatim/OSRM/Overpass | Sharer location / OSM | Auto-sending coords for search/route | 2.4 M | OSM privacy envelope: only user-typed queries + user-chosen endpoints; POI centre = map centre; never friend coords | `osm.rs` header + `security-architecture §11` |
| T-I3 | Info disclosure | DB dump exposes history | Locations / Postgres | History retained | 2.2 M | Latest publish plus an owner-only 7-day trail (24 h returned), erased with the account; bookmarks on-device JSON with owner-only permissions on Unix (AES-256-GCM primitives available, not wired) | `src/server/state/locations.rs`, `bookmarks.rs`, `encryption.rs` |
| T-I4 | Info disclosure | Error bodies leak internals | Server | Verbose errors | 1.8 L | `{error:{code,message}}` envelope; generic messages; details in server logs only | `api.rs::format_api_error`, `mod.rs::error_envelope` |
| T-D1 | DoS / DDoS | Flood login/publish/search → service down | Availability / gateway | No rate limits | 2.3 M | Per-client HMAC auth throttles (login 30/5 min, register 10/h, anonymous/recovery 20/10 min) and 120/min per-account writes; healthchecks; modular scaling (`--scale`); general per-IP write limiter and proxy DDoS controls remain | `server/state/rate_limit.rs`, `server/mod.rs`, `docker-compose.yml` |
| T-D2 | DoS | Nominatim abuse → OSMF ban | OSM budget / map | Unthrottled scraping | 1.9 L | 1 req/s client throttle + per-client server caps on `/map/search`, `/map/reverse`, `/map/route` and `/map/tiles`; user-initiated only; no bulk | `osm.rs`, `server/state/rate_limit.rs`, QA grep for `tile.openstreetmap.org` |
| T-E1 | Elevation of privilege | Non-admin calls admin `POST /users/`, `DELETE /users/{id}` | Admin / users svc | Missing role check | 2.7 H | `administrator` role gate; `must_change_password` forces rotation (403 otherwise); audit | `handlers::create_user/delete_user`, `SECURITY.md` |
| T-E2 | Elevation of privilege | Expand share precision without consent | Sharer location | Client-side rounding only | 2.5 H | Precision downgrade **server-side** before serialization; PATCH validates enum values | `handlers::update_share`, `list_friend_locations` |
| T-L1 | LINDDUN | Linkability via persistent IDs across services | Privacy | Stable pseudonymous IDs logged | 1.7 L | Pseudonymous accounts, no analytics/trackers (0 SDKs), audit excludes coords/IDs where possible | `Cargo.toml` (no trackers), `PRIVACY.md` |

Full attack-tree decomposition for the top goal (locate a victim) is in §5. LINDDUN privacy threats
are covered by T-L1 + T-I1–T-I3.

## 5. Attack tree (goal: learn victim's current location)

```
GOAL: learn Alice's exact location [T-I1, DREAD 2.9]
├─ OR 1. Be a legitimate friend with active unexpired share
│     └─ AND 1a. Get accepted friendship  ── blocked by explicit accept/reject + audit [T-R1]
│     └─ AND 1b. Sharer sets exact precision ── blocked by expiry/pause/delete defaults [T-I1]
├─ OR 2. Account takeover (T-S1 → T-S2)
│     ├─ Credential stuffing  ── Argon2id + rate limit + lockout
│     └─ Session hijack via HTTP/XSS ── Secure/HttpOnly + ADR-0006 + CSP
├─ OR 3. IDOR / broken access control (T-T2)
│     └─ GET /locations/friends/{alice_id} ── server consent check → generic 403/404
├─ OR 4. Side channel via OSM (T-I2)
│     └─ Force routing/search with victim coords ── envelope: only user-typed data sent
└─ OR 5. DB / device theft (T-I3)
      ├─ Postgres dump ── latest publish + owner-only 7-day trail; erased with account
      └─ Stolen phone ── owner-only bookmark JSON on Unix; AES-GCM primitives not wired
```

Mitigation coverage: every leaf has ≥1 implemented control; leaves 2–5 are high-confidence from
tests; leaf 1 is a **user-consent risk**, mitigated by UX (expiry, precision, one-tap revoke).

## 6. Prioritised defence mechanisms (guideline order)

| Rank | Defence | Covers | Status | Test evidence |
|---|---|---|---|---|
| 1 | Server-side authorisation (consent check + IDOR fail-closed) | T-T2/T-E2/T-I1 | ✅ Implemented | contract/IDOR tests |
| 2 | Secure auth/session + CSRF double-submit + HTTPS-off-loopback | T-S1/T-S2/T-T1 | ✅ Implemented | `api.rs` 4 tests + validation |
| 3 | Input validation + output encoding (boundary) | T-T3/T-I4 | ✅ Implemented | `validation.rs` 11 tests |
| 4 | Rate limiting (server auth + write throttles + client OSM limits) + generic errors | T-D1/T-S1 | ✅ Implemented | `server/state/rate_limit.rs` tests |
| 5 | Password hashing (Argon2id) + `must_change_password` + lockout | T-S1/T-E1 | ✅ Argon2id + lockout | shipped |
| 6 | Encryption in transit (TLS); at rest, owner-only bookmark JSON (AES-GCM primitives available) | T-I3/T-S2 | ✅ Partial | `encryption.rs` 5 tests |
| 7 | Secret management (env-only, no repo secrets) | T-I3 | ✅ Implemented | `.env.example`, gitignore |
| 8 | Logging/monitoring (audit + healthchecks) | T-R1 | ✅ Implemented | `audit.rs` 5 tests |
| 9 | Backups/recovery (Postgres volume, `pinpoint_state` snapshot) | T-D1 | ✅ Implemented | `docker-compose.yml` volume |
| 10 | MFA (TOTP), E2E messages, OS keychain | T-S1/T-I3 | ✅ MFA and message E2E; OS keychain pending | `mfa.rs`, `message_crypto.rs` |

## 7. Residual risks & honest gaps (thesis scope)

- No demo accounts are seeded; accounts are created by registration and use Argon2id hashes.
- `pinpoint_state` JSONB snapshot → needs normalised tables + migrations + least-privilege DB user.
- Location data is not end-to-end encrypted: the server sees plaintext locations (accepted, documented). TOTP MFA and message E2E are implemented.
- Request signing exists client-side only; server verification + OS-keychain for bookmark keys pending.
- HTTPS/TLS termination at gateway/nginx is deployment-config, not code-enforced.

## 8. Traceability

Requirement (SDLC-10) → Threat (§4) → Defence (§6) → Code → Test evidence in §6 table.
References: OWASP ASVS/Top 10, Threat Modeling Manifesto, OSMF policies, ADR-0006/0007/0008.

## Rejected patterns (GDPR / ethics)

- **Covert IP / WebRTC geolocation.** Pinpoint never derives location from IP addresses, WebRTC candidates, call metadata or any similar side channel. Location comes only from explicit GPS permission, manual entry, or a disclosed opt-in network-location action.
- **Raw IP logging.** The app does not store IP addresses for tracking. Rate limiting uses a per-process HMAC-SHA-256 of the client address; raw IPs are never stored and the counters expire with the window (pseudonymous, security-only, never geolocation). A general per-IP write limiter and proxy-level DDoS controls remain the gap.
- **Online status via IP.** Login state is the session ID, not the IP; binding sessions to IPs causes false logouts on NAT/CGNAT/mobile networks and adds little security.
