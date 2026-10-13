# Security Architecture and Data Protection

## What must be protected

| Asset | Classification | Main threat | Required protection |
|---|---|---|---|
| Current precise location | Critical | stalking, breach, IDOR | directional consent, backend authorization, TLS, encryption at rest, short retention, audit access |
| Approximate location and stale state | High | inference, false safety | explicit precision, freshness timestamps, uncertainty labels, restricted recipients |
| Location permissions and trusted contacts | Critical | account takeover, coercion | fresh authentication, transactional changes, audit events, notifications, revocation |
| User identity and account recovery | High | takeover, impersonation | passkeys/MFA, secure recovery, rate limits, immutable internal ID |
| Profiles, usernames, statuses | Medium/High | doxxing, harassment | empty non-friend view, pseudonyms, cooldowns, per-field visibility |
| Check-ins and trip plans | High | physical safety inference | recipient restrictions, expiry, notification privacy, deletion controls |
| Caches, media, prompts | Medium/High | malware, harassment, illegal content | signed URLs, scanning, size/type limits, moderation, takedown, retention |
| Messages and report evidence | High | abuse, disclosure, false reports | E2EE direction, selected evidence escrow, short report window, restricted review |
| Friend graph and group membership | High | social inference, targeting | relationship-based responses, minimal discovery, block rules, access logs |
| Audit logs | High | cover-up, privacy breach | append-only design, restricted access, redaction, retention, integrity monitoring |
| Keys, tokens, secrets | Critical | total compromise | secrets manager, rotation, no logs, short token life, refresh rotation, least privilege |
| Backups and exports | Critical | offline breach | encryption, separate credentials, access review, restoration tests, secure deletion |

## Security boundaries

### Mobile client

The mobile app is untrusted from the server's perspective. It can request
permissions and present data, but it cannot decide whether a viewer may access
another person's location. Store tokens in platform secure storage, protect
sensitive actions with device authentication, minimize local history, and treat
screenshots, rooted devices, malware, and physical access as residual risks.

### API boundary

Every request carries an authenticated principal. The API evaluates resource
ownership, friendship, directional consent, scope, expiry, block state, and
purpose before returning data. Object identifiers are not authorization. Denials
should not reveal whether hidden resources exist.

### Realtime boundary

A WebSocket connection is not a permission grant. Authenticate the connection,
authorize each subscription, re-check revocation, limit channels to opaque
resource identifiers, and disconnect or downgrade subscriptions when consent
changes.

### Database boundary

The application database role should not be able to perform unrestricted
operational administration. Separate migration, application, worker, reporting,
and support roles. Encrypt sensitive columns or records where appropriate, but
remember that encryption does not fix an authorization flaw.

### Object-storage boundary

Clients never receive permanent bucket credentials. The API issues short-lived
signed URLs after authorization. Uploads land in quarantine, are scanned, and are
released only after policy checks. Media metadata must be stripped or controlled
so images do not leak hidden GPS data.

### Operator boundary

Moderators and support operators receive minimum necessary fields. Precise
location access is exceptional, logged, time-limited, and reviewed. Operators must
not browse private location data for curiosity or convenience.

## Control sequence for a location read

1. Authenticate the caller and validate the token/session.
2. Resolve the immutable caller and target IDs.
3. Check blocks and account state.
4. Check that the caller is the current authorized recipient.
5. Check directional consent, purpose, precision, expiry, and stale status.
6. Apply coarse/approximate transformation if required.
7. Redact fields not necessary for the requested view.
8. Record a privacy-preserving access event.
9. Return only the authorized representation.

The same pattern applies to statuses, caches, friend-map layers, media, reports,
and emergency context.

## Encryption and key management

- TLS protects client/API, API/provider, worker/database, and storage connections.
- Database and object storage encryption protect disks and service-side storage.
- A secrets manager holds database credentials, signing keys, provider keys, and
  encryption-key references.
- Key-encryption keys and data-encryption keys have separate access policies.
- Rotation and revocation procedures are documented and tested.
- Tokens, passwords, private keys, recovery codes, and raw location are never
  written to ordinary logs.

Application-level encryption may be added for especially sensitive location or
report records. It introduces key-recovery and moderation tradeoffs and must be
designed before claiming that operators cannot read data.

## Authentication and sessions

Use OIDC/OAuth 2.0 authorization code with PKCE S256 for mobile and web public
clients. Use short-lived access tokens, rotating refresh tokens, device/session
lists, logout/revocation, rate limits, and risk-based fresh authentication for
changing trusted contacts or opening emergency context.

For a web companion, use secure host-only cookies where appropriate with
`HttpOnly`, `Secure`, and an intentional `SameSite` policy. Use CSRF protection for
cookie-authenticated state changes and a strict CSP; native clients do not use
browser cookies as their primary token store.

## Testing evidence

- Authorization matrix tests for every actor/resource/action combination.
- IDOR tests using another user's identifiers.
- Revocation and expiry tests for location, check-ins, caches, sessions, and
  signed URLs.
- Token replay, refresh-rotation, logout, recovery, and brute-force tests.
- Input fuzzing for coordinates, usernames, statuses, files, prompts, and group
  criteria.
- XSS/CSRF tests for any web companion.
- Database-role, backup-restore, key-rotation, and log-redaction tests.
- Mobile permission, battery, offline, OS suspension, and stale-state tests.
- Tabletop incident response tests and documented residual risks.

See [Security Control Register](../reference/security-control-register.md) for the
control-level (SC-01 through SC-09) problem/relevance/implementation/test/limitation
breakdown, [Security Threats and Controls](./security-threats-and-controls.md)
for the threat-first view across location, identity, social/content, sponsorship,
and infrastructure, and [API Security Implementation](../reference/api-security-implementation.md)
for detailed endpoint-level security code examples, OAuth 2.0/PKCE flow diagrams,
authorization sequences, rate limiting configuration, input validation routines,
error handling patterns, and audit logging specifications.

---
*Part of [§8 Crosscutting Concepts](./crosscutting-concepts.md).*
