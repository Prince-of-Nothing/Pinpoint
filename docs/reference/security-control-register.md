# Security Control Register

Each selected security control is explained using the same five questions:
what problem it addresses, why the problem matters here, how it is
implemented, how it is tested, and what limitations remain.

## SC-01 Authentication

- **Problem:** account takeover through stolen or weak credentials.
- **Relevance:** an attacker who controls an account could change trusted
  contacts or expose location.
- **Implementation target:** passkeys or OAuth 2.0/OIDC with short-lived
  access tokens, refresh-token rotation, device/session management, and
  optional MFA. Passwords, if supported, use a modern slow password hash and
  never appear in logs.
- **Implemented (MVP, password path):** bcrypt hashing
  (`services/security.py`), 15-minute JWTs with required claims and an
  algorithm allowlist (`services/token_service.py`), server-side sessions
  stored as SHA-256 hashes, per-IP rate limiting, account lockout with
  automatic unlock, timing-equalised unknown-account handling, and
  token-version revocation on password change (`services/auth_service.py`).
  OIDC/PKCE remains the target for the next increment.
- **Test:** invalid credentials, token expiry, refresh rotation, logout
  invalidation, recovery abuse, brute-force limits, and second-factor
  enforcement for sensitive actions. Automated in
  `backend/tests/test_security_flows.py` (generic 401s, lockout lifecycle,
  JWT replay after password change, token-type confusion).
- **Limitation:** account recovery and compromised devices remain difficult;
  recovery must not silently bypass location protections. Refresh-token
  rotation and MFA are not yet implemented.

## SC-02 Directional authorization

- **Problem:** a user sees another person's location without current
  consent.
- **Relevance:** friendship is mutual, but location sharing is one-way.
- **Implementation target:** backend policy checks on every location
  read/write; a share record includes owner, recipient, precision, purpose,
  expiry, and revocation.
- **Implemented (MVP):** `LocationAccessPolicy`
  (`services/authorization.py`) runs per friend on every read — accepted
  friendship, owner→viewer direction, active/unrevoked/unexpired share, no
  block. Consent records carry owner, precision, purpose and a mandatory
  ≤24 h expiry; only the owner can revoke; blocking revokes every share
  between the pair. Precision is reduced server-side (exact / ~1 km / ~11 km)
  before coordinates leave the API. See
  [Sequence — Authorized Location Read](../architecture/diagrams/sequence-authorized-location-read.md).
- **Test:** owner-only access, recipient access, non-friend denial,
  revoked-share denial, expired-share denial, IDOR attempts, and cross-account
  tests. Automated: `test_directional_location_sharing_and_idor`,
  `test_approximate_precision_is_reduced_server_side`,
  `test_block_revokes_shares_and_hides_locations`.
- **Limitation:** a trusted recipient can still photograph or misuse
  information outside the app. `mutual` consent currently requires each side
  to create their own record (no server-mediated accept flow yet).

## SC-03 Private-by-default profiles

- **Problem:** public profile data helps stalking, impersonation, and social
  engineering.
- **Relevance:** non-friends should see no image, status, updates, location,
  friend list, or activity.
- **Implementation target:** response filtering on the backend, not only
  client-side hiding; immutable internal IDs and username cooldowns.
- **Implemented (MVP):** `ProfileVisibilityPolicy`
  (`services/authorization.py`) filters every profile response on the
  backend: self → full, accepted friend → display name only, anyone else
  (pending/blocked/stranger) → a redacted shell **identical** to the response
  for a non-existent account, so user IDs cannot be probed. Profile views by
  non-friends are audit-logged.
- **Test:** profile views from self, friend, group member, non-friend, and
  blocked user contexts. Automated: `test_profile_visibility_filtering`
  (stranger response == non-existent response).
- **Limitation:** a username may still be voluntarily shared outside the
  platform. Group-member visibility is deferred with the groups feature.

## SC-04 Input and API security

- **Problem:** injection, malformed data, abusive requests, and information
  leakage.
- **Relevance:** location, profiles, check-ins, statuses, and group inputs
  are attacker-controlled.
- **Implementation target:** schema validation, parameterized queries/ORM,
  bounded payloads, safe error responses, API rate limits, and structured
  audit events.
- **Implemented (MVP):** Pydantic v2 request schemas bound every field
  (coordinates, battery, timestamps, string patterns); SQLAlchemy ORM with
  parameterised queries only; per-endpoint rate limits with `429` +
  `Retry-After` (`services/rate_limit.py`); RFC 7807 error envelopes with no
  stack traces or internal messages (`app/api/errors.py`, `app/main.py`);
  future-timestamp rejection on location updates; password policy chain
  (length, character classes, deny-list) in `services/password_policy.py`.
- **Test:** malformed coordinates, oversized content, invalid identifiers,
  injection payloads, rate-limit tests, and error-response review.
  Automated: `test_login_rate_limit_returns_429_with_retry_after`,
  `test_health_and_validation_error_shape`.
- **Limitation:** dependency and infrastructure vulnerabilities require
  continuing maintenance; the common-password deny-list is small until a
  breached-password corpus is loaded.

## SC-05 Session and transport security

- **Problem:** session theft, replay, CSRF, and network interception.
- **Relevance:** a stolen session can reveal or change safety permissions.
- **Implementation target:** TLS, secure mobile token storage, token expiry
  and invalidation, device binding where practical, and for any web
  companion: HttpOnly, Secure, SameSite cookies, CSRF protection, CSP, and
  output escaping.
- **Implemented (MVP):** session cookie is `HttpOnly` + `SameSite=Lax` +
  `Secure` (over TLS); double-submit CSRF token required for every
  cookie-authenticated mutation (`api/deps.py: csrf_protect`); server-side
  session revocation on logout and password change, 3-hour absolute and
  30-minute idle expiry checked on every request; JWTs carry a token version
  that password changes bump, killing replayed tokens; security-headers
  middleware (HSTS over TLS, CSP `default-src 'none'`, nosniff, frame DENY,
  no-referrer) plus `TrustedHost` allowlisting; interactive API docs disabled
  in production.
- **Test:** token replay after logout, expiry, device removal, CSRF
  attempts, XSS payloads, and TLS/configuration checks. Automated:
  `test_logout_invalidates_cookie_session`,
  `test_cookie_authenticated_mutation_requires_csrf`,
  `test_password_change_revokes_outstanding_jwts`,
  `test_refresh_token_rejected_as_access_token`, `test_security_headers_present`.
- **Limitation:** mobile malware or a fully compromised device is outside
  normal application controls. TLS is terminated by the (planned) reverse
  proxy — deployment risk R1.

## SC-06 Data protection

- **Problem:** database, backup, or storage compromise exposes location
  data.
- **Relevance:** precise current location is highly sensitive.
- **Implementation target:** TLS in transit, encryption at rest, separated
  key management, minimum retention, latest-location separation, access
  controls, and encrypted backups.
- **Test:** backup access, database-role tests, deletion/expiry, key-access
  audit, and restoration tests.
- **Limitation:** authorized recipients can still disclose what they see;
  encryption does not solve bad authorization.

## SC-07 Battery and stale-state safety

- **Problem:** tracking drains the battery or a stale point is mistaken for
  a live point.
- **Relevance:** the app's safety value depends on both availability and
  honest freshness.
- **Implementation target:** sampling presets, advanced interval control,
  adaptive precision, low-battery behavior, last-update timestamps,
  last-known battery, and stale/dead state transitions.
- **Test:** battery profiles, offline periods, OS-paused updates,
  low-battery states, clock differences, and stale UI behavior.
- **Limitation:** operating systems may suspend background work and GPS can
  be inaccurate indoors.

## SC-08 Abuse reporting and monitoring

- **Problem:** stalking, harassment, malicious content, coordinated false
  reports, or operational compromise.
- **Relevance:** later groups, caches, prompts, and media increase the abuse
  surface.
- **Implementation target:** report categories, evidence selection,
  restricted moderation access, audit logs, alerts, appeals, and rate
  limits. AI can triage; humans decide ambiguous or serious cases.
- **Implemented (MVP):** `AuditLogger` (Observer) writes structured events
  to the application log and to the `audit_events` table. Events reference
  users by immutable ID and store whitelisted, PII-free context only —
  login attempts persist a **keyed hash** of the email, never the address
  (`services/security.py: pseudonymize_email`). Recorded events include
  login success/failure, lockout, password change, logout, friendship
  request/accept/reject/revoke, block (with share-revocation counts), share
  create/revoke, profile views by non-friends, and location list reads.
- **Test:** report submission, evidence retention window, moderator RBAC,
  false-report scenarios, alert delivery, and deletion behavior. Audit
  coverage is asserted indirectly by the flow tests; a dedicated audit-table
  test is planned.
- **Limitation:** ephemeral data may expire before a report; reported
  evidence requires a clearly documented preservation exception. Retention
  periods for `audit_events` are not yet enforced automatically.

## SC-09 Incident response

- **Problem:** slow or confused response to account takeover, location
  breach, outage, or abuse.
- **Relevance:** safety incidents can cause immediate physical and privacy
  harm.
- **Implementation target:** incident severity levels, owners, containment
  steps, user notification criteria, evidence handling, recovery,
  post-incident review, and mentor escalation.
- **Test:** tabletop exercises and at least one simulated incident during
  the project.
- **Limitation:** response quality depends on staffing, provider access, and
  regional legal duties.

---
*Referenced from [Security Architecture and Data Protection](../architecture/security-architecture-and-data-protection.md),
[Security Threats and Controls](../architecture/security-threats-and-controls.md),
and [Incident Response Plan](../architecture/incident-response-plan.md) (SC-09).*
