# Security Control Register

Each control answers the same questions: what problem it addresses, why it
matters here, how it is implemented **as built**, how it is tested, what
the result was and which limitations remain. Status values: **Implemented**
(built and tested), **Partial** (MVP subset built; remainder planned) and
**Deferred** (designed, planned for October to December).

Test names refer to `backend/tests/`. The latest run with per-test results is
in [Security Test Report](../evidence/security-test-report.md).

## Summary

| ID | Control | Status | Main evidence |
|---|---|---|---|
| SC-01 | Authentication | Implemented (passwords); passkeys/MFA deferred | `test_auth.py`, `test_rate_limit.py` |
| SC-02 | Directional authorization | Implemented | `test_location_shares.py`, `test_locations.py`, IDOR matrix |
| SC-03 | Private-by-default profiles | Implemented | `test_social.py` |
| SC-04 | Input and API security | Implemented | `test_security_attacks.py`, `test_core.py` |
| SC-05 | Session and transport security | Implemented | `test_auth.py`, `test_web_client.py`, e2e run |
| SC-06 | Data protection | Partial | DB least-privilege evidence, latest-location-only storage |
| SC-07 | Battery and stale-state safety | Partial (server + web client) | `test_locations.py`, `test_checkins.py` |
| SC-08 | Abuse reporting and monitoring | Partial (audit trail + admin RBAC only) | `test_authz_audit.py`, `test_admin.py` |
| SC-09 | Incident response | Documented | [Incident Response Plan](../architecture/incident-response-plan.md) |

## Mapping to the internship Task 6 list

| Task 6 item | Where it is implemented |
|---|---|
| Secure authentication | `app/api/auth.py`, `app/services/sessions.py` (SC-01) |
| Password hashing | `app/services/passwords.py`: Argon2id + policy (SC-01) |
| RBAC / authorization | `app/api/deps.py` (`require_admin`, `deny_foreign`), share checks in `app/api/locations.py` (SC-02, SC-03) |
| Input validation | `app/schemas/*` (`StrictRequest`), `app/core/errors.py`, `app/core/middleware.py` (SC-04) |
| Secure JWT / session handling | `app/services/tokens.py`, `app/services/sessions.py`, `app/api/deps.py` (SC-05) |
| API rate limiting | `app/services/ratelimit.py` (SC-01, SC-04) |

---

## SC-01 Authentication

- **Problem:** account takeover through stolen, guessed or brute-forced credentials.
- **Relevance:** an attacker controlling an account could add trusted
  contacts, start sharing the victim's location or read others' locations.
- **Implementation:**
  - Registration requires a unique, normalised username and email. The
    password policy follows NIST SP 800-63B: minimum 10 characters, maximum
    128, a common-password deny-list, not containing the username or email,
    not trivially repetitive. There are no composition rules.
  - Passwords are hashed with **Argon2id** (RFC 9106 parameters: t=3,
    m=64 MiB, p=4) and rehashed transparently when parameters change.
    Plain passwords are never stored or logged.
  - Login accepts a username or email and always returns the same generic
    error. Unknown accounts run a dummy hash verification, so their timing
    matches real accounts.
  - **Lockout:** 5 failed attempts lock the account for 15 minutes, and the
    lock expires on its own. Failed current-password checks during a
    password change count too. Admins can unlock accounts.
  - **Rate limits:** login 10/min/IP, registration 5/hour/IP, refresh
    30/min/IP, password change 5/15 min/account. Exceeding one returns 429
    with `Retry-After`.
  - Password change requires the current password and revokes every session.
  - A CLI creates admin accounts; it reads the password from the
    environment or a prompt, never from argv.
- **Tests:** `test_auth.py` (registration, duplicates, weak passwords,
  Argon2id storage, generic errors, lockout and expiry, counter reset,
  password change), `test_rate_limit.py`, `test_security_attacks.py::test_sc01_*`
  (secrets never in logs), `test_sc04_sql_injection_in_login_is_inert`.
- **Result:** all pass (see report).
- **Limitations:**
  - No MFA or passkeys yet.
  - Registration reveals whether a username or email is taken. This is
    accepted, because users must be able to find each other by username,
    and registration is rate limited.
  - Lockout can be abused to lock a known account for 15 minutes. The
    per-IP limit slows this down, and admins can unlock.
  - There is no account recovery (password reset) flow yet.

## SC-02 Directional authorization

- **Problem:** a user sees another person's location without their current consent.
- **Relevance:** friendship is mutual, but location sharing must be one-way
  and revocable at any time.
- **Implementation:**
  - A `location_shares` row records owner, recipient, precision, purpose,
    expiry and revocation.
  - A share can only be created with an **accepted** friend who is not
    blocked in either direction. Only the owner can change precision or
    expiry, so a recipient cannot upgrade their own access. Either party
    can end the share.
  - `GET /locations/visible` re-checks, on **every request**, that the
    share is active and unexpired, that the two users are still friends,
    and that there is no block.
  - **No endpoint accepts another user's id to read a location**, which
    removes that IDOR surface by design.
  - Unfriending and blocking revoke shares in both directions.
  - Records belonging to others return the same 404 as missing records,
    and the attempt is audited (`authz.denied`).
  - Record ids are random UUIDs, not sequential numbers.
  - Admin endpoints require the `admin` role, read from the database on
    each request. Admins have no location endpoints (least privilege).
- **Tests:** `test_location_shares.py`, `test_locations.py` (one-way, no
  share = no access, revoked, expired, block with a surviving share row),
  `test_security_attacks.py::test_sc02_idor_matrix_every_foreign_id_is_denied`,
  `test_sc02_no_endpoint_accepts_a_user_id_for_location_reads`,
  `test_authz_audit.py`, `test_admin.py`.
- **Result:** all pass.
- **Limitations:** a trusted recipient can still screenshot or retell what
  they see. Location reads themselves are not audited (only share changes
  and denials are); per-read access logs are planned together with search
  corridors.

## SC-03 Private-by-default profiles

- **Problem:** public profile data helps stalking, impersonation and social engineering.
- **Relevance:** non-friends should learn nothing beyond the username needed
  to send a friend request.
- **Implementation:**
  - `GET /users/{username}` filters on the server: self and accepted
    friends see the display name and bio; everyone else, including blocked
    users in either direction, gets `{username, relationship: "none"}`.
  - Email, role and security fields are never returned to other users.
  - Blocked users are indistinguishable from non-existent ones in friend
    requests.
- **Tests:** `test_social.py::test_profile_visibility_by_relationship`,
  `test_blocked_user_sees_empty_profile`,
  `test_profile_never_exposes_email_or_internal_fields`,
  `test_block_removes_friendship_and_hides_user`.
- **Result:** all pass.
- **Limitations:**
  - Usernames can be confirmed to exist (lookups are rate limited to
    60/min per account).
  - Group-member visibility is not relevant yet, because groups are deferred.

## SC-04 Input and API security

- **Problem:** injection, mass assignment, malformed or oversized data, and information leakage in errors.
- **Relevance:** coordinates, names, notes and identifiers are all attacker-controlled.
- **Implementation:**
  - Pydantic schemas that **reject unknown fields** (`extra="forbid"`)
    and bound every field: coordinate ranges, NaN/Infinity rejected,
    battery 0–100, text lengths, no control characters, username
    `^[a-z0-9_]{3,30}$`, share duration 5 minutes to 7 days, pagination
    limits.
  - Timestamps must be within 2 minutes in the future and at most 24 hours
    in the past.
  - SQLAlchemy ORM only, so every query is parameterised; there is no raw
    SQL with user input.
  - Request bodies are capped at 64 KB, whether the size is declared or
    streamed (413).
  - One RFC 9457 Problem Details format for all errors, with a stable
    `code`, no stack traces, no exception text and no echoed input.
  - Rate limits on sensitive endpoints; bandit and pip-audit in CI.
- **Tests:** `test_security_attacks.py` (SQL injection payloads,
  mass-assignment matrix, 500 leak test, validation without echo, 405,
  malformed JSON), `test_locations.py::test_invalid_coordinates_and_timestamps_rejected`,
  `test_location_shares.py::test_input_validation`,
  `test_core.py::test_oversized_body_rejected`.
- **Result:** all pass; bandit reports no findings and pip-audit reports no
  known vulnerabilities.
- **Limitations:**
  - The rate limiter fails open if Redis is unavailable. This is a
    deliberate availability trade-off; lockout still applies.
  - Dependency and infrastructure vulnerabilities need continuous
    maintenance.

## SC-05 Session and transport security

- **Problem:** session theft, token replay, CSRF, XSS and network interception.
- **Relevance:** a stolen session can reveal or change safety permissions.
- **Implementation:**
  - **Access tokens:** 15-minute JWTs, with the algorithm pinned
    server-side (`none` and other keys rejected) and issuer, audience,
    `type=access` and required claims all checked. Each token carries a
    session id and is valid only while that server-side session is. So
    logout, password change, admin deactivation and idle (30 min) or
    absolute (12 h) timeouts take effect immediately.
  - **Refresh tokens:** opaque 256-bit random values, stored only as
    SHA-256 hashes and rotated on every use. **Reuse of an old refresh
    token revokes the whole session** (theft detection). There is a limit
    on sessions per user; users can list and revoke their sessions.
  - **Web client:** the refresh token lives in an `HttpOnly; Secure;
    SameSite=Strict` cookie scoped to `/api/v1/auth`. Refreshing needs a
    **CSRF token bound to the session** (stored hashed, compared in
    constant time). The access token is kept only in memory. Bearer tokens
    are never accepted from cookies or query strings.
  - **Headers:** strict CSP (`script-src 'self'`, no inline or eval; the
    Leaflet library is served locally), `nosniff`, `X-Frame-Options: DENY`,
    `frame-ancestors 'none'`, `Referrer-Policy: no-referrer`,
    `Cache-Control: no-store` on the API, and HSTS in production. No CORS
    unless origins are explicitly configured.
  - **XSS:** the client renders all user text with `textContent`, and a
    test forbids HTML sinks in the client code.
  - **Transport:** optional Caddy HTTPS proxy; Uvicorn trusts forwarded
    headers only from the proxy's address.
- **Tests:** `test_auth.py` (tampered, `alg=none`, wrong key, expired,
  refresh-as-access, rotation, reuse revocation, CSRF on cookie refresh,
  logout, logout-all, idle/absolute timeout, session limit),
  `test_web_client.py`, `test_security_attacks.py::test_sc05_*`, and the
  [browser run](../evidence/web-client-e2e.md).
- **Result:** all pass; the browser run showed no CSP violations and the
  XSS payload rendered as text.
- **Limitations:**
  - A compromised device or malicious browser extension is out of scope.
  - JWTs are signed with HS256 and a shared secret (asymmetric keys and
    key rotation are planned).
  - The demo HTTPS setup uses Caddy's internal CA.

## SC-06 Data protection

- **Problem:** a database or backup compromise exposes location data.
- **Relevance:** precise current location is highly sensitive.
- **Implementation:**
  - Only the **latest** location per user is stored (no history), and
    users can erase it.
  - Approximate shares never return the exact point: coordinates snap to a
    fixed grid of about 1.1 km.
  - The API uses a **least-privilege PostgreSQL role** (row DML only).
    Migrations run as the owner, and the audit table is append-only for
    the API role.
  - Secrets come from the environment. Production refuses to start with
    the default or a short secret, debug mode, or non-secure cookies.
  - Refresh and CSRF tokens are stored only as hashes.
- **Tests:** `test_core.py::test_production_refuses_insecure_settings`,
  `test_locations.py::test_approximate_share_hides_exact_position`,
  `test_approximation_is_stable_for_nearby_points`,
  [database least-privilege evidence](../evidence/database-least-privilege.md).
- **Result:** pass; the runtime role cannot alter the schema or audit rows.
- **Limitations:**
  - Encryption at rest and encrypted backups depend on the hosting
    provider and are not configured locally.
  - There is no separate key-management service yet.
  - Grid snapping can still leak movement across cell boundaries over time.

## SC-07 Battery and stale-state safety

- **Problem:** tracking drains the battery, or a stale point is mistaken for a live one.
- **Relevance:** the app's safety value depends on honest freshness.
- **Implementation:**
  - The server labels every location `live` (≤ 5 min), `stale`
    (≤ 60 min), `dead` or `unknown`, based on the device timestamp.
  - A skewed clock can never make data look fresher, and out-of-order
    updates are ignored.
  - Battery is reported as last-known with its time; a dead device does
    not claim a current charging state; a low-battery flag is set at ≤ 15%.
  - The web client offers high, balanced and saver presets, switches to
    saver automatically below 20% when not charging, and draws stale or
    dead markers dashed and faded.
  - Check-ins derive `pending → due_soon → overdue → missed` from the clock.
- **Tests:** `test_locations.py::test_freshness_transitions`,
  `test_low_battery_flag`, `test_out_of_order_updates_do_not_overwrite_newer_data`,
  `test_checkins.py::test_state_transitions_follow_the_clock`.
- **Result:** all pass.
- **Limitations:**
  - Background location on mobile operating systems and native battery
    sampling need the planned mobile app.
  - There are no push notifications yet; reminders appear in the client.

## SC-08 Abuse reporting and monitoring

- **Problem:** stalking, harassment and operational compromise go unnoticed.
- **Implementation (MVP subset):**
  - A structured `audit_events` table records logins, failures, lockouts,
    refresh reuse, CSRF rejections, rate-limit hits (once per window),
    authorization denials (IDOR probes), relationship and share changes,
    and admin actions. It never stores secrets or coordinates.
  - Admins read the audit log through a bounded, validated API. Blocking
    is available to every user.
- **Tests:** `test_authz_audit.py`, `test_admin.py`, `test_rate_limit.py::test_rate_limit_exceeded_is_audited_once_per_window`,
  `test_auth.py::test_login_is_audited`.
- **Limitations:** report categories, moderation workflow, alerting and
  appeals are deferred (October to December).

## SC-09 Incident response

- **Problem:** slow or confused response to account takeover, location breach or outage.
- **Implementation:** the [Incident Response Plan](../architecture/incident-response-plan.md)
  (severity levels, owners, containment and notification). The system
  supports containment directly:
  - revoke a user's sessions (deactivate or logout-all),
  - unlock or deactivate accounts,
  - users can stop sharing and erase their location immediately,
  - the audit trail supports investigation.
- **Test:** tabletop exercise planned; see the development plan.
- **Limitations:** response quality depends on staffing and provider access.
