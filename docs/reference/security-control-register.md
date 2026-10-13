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
- **Test:** invalid credentials, token expiry, refresh rotation, logout
  invalidation, recovery abuse, brute-force limits, and second-factor
  enforcement for sensitive actions.
- **Limitation:** account recovery and compromised devices remain difficult;
  recovery must not silently bypass location protections.

## SC-02 Directional authorization

- **Problem:** a user sees another person's location without current
  consent.
- **Relevance:** friendship is mutual, but location sharing is one-way.
- **Implementation target:** backend policy checks on every location
  read/write; a share record includes owner, recipient, precision, purpose,
  expiry, and revocation.
- **Test:** owner-only access, recipient access, non-friend denial,
  revoked-share denial, expired-share denial, IDOR attempts, and cross-account
  tests.
- **Limitation:** a trusted recipient can still photograph or misuse
  information outside the app.

## SC-03 Private-by-default profiles

- **Problem:** public profile data helps stalking, impersonation, and social
  engineering.
- **Relevance:** non-friends should see no image, status, updates, location,
  friend list, or activity.
- **Implementation target:** response filtering on the backend, not only
  client-side hiding; immutable internal IDs and username cooldowns.
- **Test:** profile views from self, friend, group member, non-friend, and
  blocked user contexts.
- **Limitation:** a username may still be voluntarily shared outside the
  platform.

## SC-04 Input and API security

- **Problem:** injection, malformed data, abusive requests, and information
  leakage.
- **Relevance:** location, profiles, check-ins, statuses, and group inputs
  are attacker-controlled.
- **Implementation target:** schema validation, parameterized queries/ORM,
  bounded payloads, safe error responses, API rate limits, and structured
  audit events.
- **Test:** malformed coordinates, oversized content, invalid identifiers,
  injection payloads, rate-limit tests, and error-response review.
- **Limitation:** dependency and infrastructure vulnerabilities require
  continuing maintenance.

## SC-05 Session and transport security

- **Problem:** session theft, replay, CSRF, and network interception.
- **Relevance:** a stolen session can reveal or change safety permissions.
- **Implementation target:** TLS, secure mobile token storage, token expiry
  and invalidation, device binding where practical, and for any web
  companion: HttpOnly, Secure, SameSite cookies, CSRF protection, CSP, and
  output escaping.
- **Test:** token replay after logout, expiry, device removal, CSRF
  attempts, XSS payloads, and TLS/configuration checks.
- **Limitation:** mobile malware or a fully compromised device is outside
  normal application controls.

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
- **Test:** report submission, evidence retention window, moderator RBAC,
  false-report scenarios, alert delivery, and deletion behavior.
- **Limitation:** ephemeral data may expire before a report; reported
  evidence requires a clearly documented preservation exception.

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
