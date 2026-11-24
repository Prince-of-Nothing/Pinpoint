# Security Control Register

Each control answers the same questions: what problem it addresses, why it
matters here, how it is implemented **as built**, how it is tested, what the
result was and which limitations remain. Status values: **Implemented**
(built and tested), **Partial** (MVP subset built; remainder planned) and
**Deferred** (designed, planned for October to December).

Test names refer to the Rust suites under `tests/` and the unit tests under
`src/`. The current gate and scan evidence are in
[TESTING](../TESTING.md), [the DAST report](../evidence/zap-baseline.html) and
the [security architecture](../security-architecture.md) §15.

## Summary

| ID | Control | Status | Main evidence |
|---|---|---|---|
| SC-01 | Authentication | Implemented (password, TOTP, anonymous, Google OAuth); GitHub OAuth implemented, unmerged; passkeys deferred | `tests/router_probe.rs`, `tests/auth_password.rs`, `tests/oauth.rs` |
| SC-02 | Directional authorization | Implemented | `tests/location_consent.rs`, `tests/router_probe.rs` |
| SC-03 | Private-by-default data | Implemented | `tests/today.rs`, `tests/account_delete.rs`, `src/server/locations_handlers.rs` |
| SC-04 | Input and API security | Implemented | `src/security/validation.rs`, `tests/location_consent.rs`, `tests/router_probe.rs` |
| SC-05 | Session and transport security | Implemented | `src/server/state/sessions.rs`, `tests/router_probe.rs`, ADR-0012 |
| SC-06 | Data protection at rest | Partial (notes and messages encrypted; location rows plaintext) | `tests/note_encryption.rs`, ADR-0023, ADR-0025 |
| SC-07 | Stale-state and availability | Partial (latest-only + timestamp; no battery sampling, no push) | `src/models.rs`, `src/server/locations_handlers.rs` |
| SC-08 | Monitoring and abuse reporting | Partial (rate limits, audit, diagnostics; no alerting pipeline) | `src/security/audit.rs`, `src/server/admin_handlers.rs`, `docs/evidence/` |
| SC-09 | Incident response | Documented | [Security architecture §13](../security-architecture.md) |

## Mapping to the internship Task 6 list

| Task 6 item | Where it is implemented |
|---|---|
| Secure authentication | `src/server/auth_handlers.rs`, `src/security/password.rs`, `src/security/oauth.rs` (SC-01) |
| Password hashing | `src/security/password.rs`: Argon2id PHC (`m=19456, t=2, p=1`), legacy PBKDF2 rehash (SC-01) |
| RBAC / authorization | `src/server/mod.rs` role checks, `src/server/locations_handlers.rs` consent checks (SC-02, SC-03) |
| Input validation | `src/security/validation.rs`, `src/server/*_handlers.rs` (SC-04) |
| Secure session handling | `src/server/state/sessions.rs`, opaque `__Host-` cookie + CSRF (SC-05), ADR-0012 |
| API rate limiting | `src/server/state/rate_limit.rs` (SC-01, SC-04, SC-07) |

---

## SC-01 Authentication

- **Problem:** account takeover through stolen, guessed or brute-forced
  credentials, or through a provider identity that lands on someone else's
  email.
- **Relevance:** an attacker with an account could add contacts, receive
  shared locations, or read the victim's notes and trail.
- **Implementation:**
  - Email/password registration with server-side validation; passwords are
    hashed with **Argon2id** (`$argon2id$v=19$m=19456,t=2,p=1$...`, PHC), and
    legacy PBKDF2 hashes are verified then rehashed on the next login.
  - Optional **TOTP MFA**; disabling it requires a current code and replay is
    blocked by the stored last step.
  - **Anonymous device accounts** with a one-time recovery code stored as a
    SHA-256 hash; recovery rotates the code and the old one stops working.
  - **Lockout:** 5 failed logins lock the account for 15 minutes; IP throttles
    are login 30/5 min, register 10/h, anonymous/recovery 20/10 min.
  - **OAuth 2.0 / OIDC:** Google sign-in (authorization code + PKCE, verified
    email, no email auto-linking). GitHub is implemented against the same
    flow but its PR is not merged yet.
- **Testing:** `tests/router_probe.rs` (register, login, MFA, anonymous,
  recovery, sessions), `tests/auth_password.rs` (password change), unit tests
  in `src/security/password.rs`, `tests/oauth.rs` (Google flow, state replay,
  unverified email, email collision), `scripts/smoke.ps1` (29 live checks).
- **Limitations:** no passkeys/magic links; GitHub OAuth pending merge; the
  lockout and throttles are per replica, not shared.

## SC-02 Directional authorization

- **Problem:** IDOR and over-sharing: reading a location the owner did not
  share, or sharing in the wrong direction.
- **Relevance:** location is the most sensitive asset; a single missed check
  exposes a person's whereabouts.
- **Implementation:** every read resolves the caller's accepted friendship and
  an **active, unexpired, unpaused** `LocationShare` whose recipient is the
  caller; `direction` (`to_friend`/`from_friend`/`mutual`) is stored per
  friendship; precision (`exact`/`approximate`/`city_only`) is applied before
  serialization. Notes, trail, bookmarks and check-ins are filtered by owner.
- **Testing:** `tests/location_consent.rs` (consent, pause, expiry, precision,
  IDOR), `tests/router_probe.rs` (403/404 behaviour), `scripts/smoke.ps1`.
- **Limitations:** a compromised client still depends on the server; friends
  can screenshot what they are allowed to see.

## SC-03 Private-by-default data

- **Problem:** accidental exposure of profiles, notes, photos, trail or
  bookmarks to non-friends or other accounts.
- **Relevance:** the product promise is that nothing is shared without
  explicit consent.
- **Implementation:** profile responses are scoped (self, accepted friend,
  pending request, otherwise 404); notes/photos, history, check-ins and
  bookmarks are owner-only; share lists are owner-only; account deletion
  cascades through every table.
- **Testing:** `tests/today.rs`, `tests/account_delete.rs`,
  `tests/router_probe.rs`, `scripts/smoke.ps1`.
- **Limitations:** photos are server-stored (encrypted when the key is set);
  the database role can read non-encrypted columns.

## SC-04 Input and API security

- **Problem:** injection, oversized input, control-character spoofing and
  error responses leaking internals.
- **Relevance:** every endpoint accepts JSON from untrusted clients.
- **Implementation:** `security::validation` counts characters (not bytes),
  rejects C0/C1 and bidi controls where display text is rendered, validates
  email/password/names/coordinates/accuracy and search length; sqlx uses bound
  parameters only; errors return a stable `{error:{code,message}}` envelope
  without stack traces; per-account writes are capped at 120/min.
- **Testing:** unit tests in `src/security/validation.rs`,
  `tests/location_consent.rs`, `tests/router_probe.rs`, `scripts/smoke.ps1`
  (bidi email/name, multibyte lengths, labels, notes, check-ins, search).
- **Limitations:** validation is per field; a new field must be added to the
  tests; no schema validator on responses.

## SC-05 Session and transport security

- **Problem:** session hijacking, CSRF, fixation and cleartext transmission.
- **Relevance:** the session cookie is the only bearer of identity.
- **Implementation:** opaque server-side session (`__Host-pinpoint_session`
  over HTTPS, `session_id` on loopback), `HttpOnly; SameSite=Lax; Secure`
  where applicable, CSRF double-submit on every unsafe method, 12 h sliding
  idle and 30-day absolute expiry, PostgreSQL-authoritative lookup on each
  authenticated API request, revocation on logout, logout-all, password
  change, admin reset and deactivation; the client refuses plaintext HTTP to
  non-loopback hosts (ADR-0006). OAuth adds a 10-minute state cookie and PKCE.
- **Testing:** `tests/router_probe.rs` (401/403/CSRF), `tests/contract.rs`
  (cookie + CSRF), `tests/oauth.rs` (state cookie), unit tests in
  `src/server/state/sessions.rs`.
- **Limitations:** the idle window is generous for a demo; rate limits are per
  replica.

## SC-06 Data protection at rest

- **Problem:** a database dump, a backup copy or a stolen disk exposes private
  data.
- **Relevance:** the server stores notes, photos, trail and current location.
- **Implementation:** passwords are Argon2id; E2E message bodies are
  ciphertext (X25519 + HKDF-SHA256 + AES-256-GCM, Ed25519 signature); note
  label/message/photo are AES-256-GCM encrypted with `DATA_ENCRYPTION_KEY`
  before storage (`enc:v1:`), with legacy plaintext still readable;
  `scripts/backup-db.ps1` produces an encrypted off-host `pg_dump` and verifies
  the roundtrip.
- **Testing:** `tests/note_encryption.rs`, `tests/contract.rs` (message
  vectors), `scripts/backup-db.ps1 -SelfTest`.
- **Limitations:** location rows, friendships and profile fields are plaintext;
  the compose `db-backup` volume remains plaintext unless the wrapper replaces
  it; no key-rotation tool yet (see ADR-0025).

## SC-07 Stale-state and availability

- **Problem:** presenting an old location as current, and staying usable when
  the network or a replica fails.
- **Relevance:** a stale "live" pin is a safety risk; an unavailable map is a
  usability failure.
- **Implementation:** only the latest location is stored (plus the owner-only
  24 h trail), with `updated_at`; precision and accuracy are returned; the
  client works offline with PMTiles packs; the gateway and services can run
  replicated; rate limits protect the OSM upstreams.
- **Testing:** `tests/location_consent.rs` (latest-only), `tests/today.rs`,
  `tests/map_offline_e2e.rs`, `map-qa.yml`.
- **Limitations:** no battery/charging signal and no live/stale badge on every
  surface; no push notifications; a single Postgres primary (5-minute dumps).

## SC-08 Monitoring and abuse reporting

- **Problem:** detecting abuse and reconstructing what happened.
- **Relevance:** rate limits and admin actions need an audit trail.
- **Implementation:** per-client HMAC-keyed rate limiter (raw IPs never
  stored), authenticated write cap, admin audit rows for user
  creation/reset/deactivation, diagnostics reports (public, 10/h), client-side
  audit log for auth events, ZAP DAST baseline and `cargo audit` in CI.
- **Testing:** `tests/router_probe.rs` (admin audit), `src/security/audit.rs`
  unit tests, `docs/evidence/zap-baseline.*`.
- **Limitations:** no external alerting/SIEM; logs stay local; the audit
  coverage is app-level, not every read.

## SC-09 Incident response

- **Problem:** responding to a suspected compromise or data exposure.
- **Relevance:** the course and GDPR require a plan, not only controls.
- **Implementation:** documented in
  [Security architecture §13](../security-architecture.md): detection
  sources, containment (revoke sessions, rotate keys, pause shares), evidence
  preservation, notification and post-incident actions.
- **Testing:** reviewed as part of the security documentation; no drill yet.
- **Limitations:** no on-call process; secrets (OAuth client secret,
  `DATA_ENCRYPTION_KEY`, backup passphrase) need an operator runbook.
