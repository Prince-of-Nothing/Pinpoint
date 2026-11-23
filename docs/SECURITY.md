# Security Policy

## ⚠️ Reporting a vulnerability
**Do not open a public issue.** Email the maintainer privately (see contact in this repo's
profile / thesis supervisor channel) with:
- description & impact,
- reproduction steps or proof-of-concept (against the **demo seed accounts only**),
- affected version/commit.

We aim to acknowledge within **72 hours** and ship a fix or mitigation within **30 days**
for critical issues. Details stay embargoed until a fix is released. This is an academic
project without a paid bug-bounty; credit in the thesis acknowledgments is offered instead.

## Security model summary

### Authentication
- **Identity model:** email+password hashed with **Argon2id**
  (PHC string, OWASP-aligned m=19456 KiB / t=2 / p=1; memory-hard); never
  logged, never returned. Legacy PBKDF2 hashes are still verifiable and are
  transparently re-hashed to Argon2id on the next successful login.
- **Anonymous model (ADR-0009):** device-bound account with no email/PII; a
  one-time 160-bit recovery code is shown once (SHA-256 stored, rotated on
  every recovery). Anonymous and identity accounts share the same session
  machinery and can be linked later.
- **Sessions (ADR-0012 + `docs/security/session-management.md`):** opaque
  256-bit server-side session in `pinpoint_sessions`, delivered as an
  `HttpOnly; SameSite=Lax` cookie; no JWT and no refresh token; CSRF
  double-submit (`X-CSRF-Token`); sliding idle timeout; immediate revocation on
  logout, logout-all, per-session revoke and password change.
- Session device management: `GET /auth/sessions`, `DELETE /auth/sessions/:id`,
  `POST /auth/logout-all` (the full session id is never returned — only an
  8-hex prefix).
- Logout invalidates the server-side session (no zombie tokens).
- Admin-provisioned accounts get a `must_change_password` flag; all non-change endpoints
  return 403 until the password is rotated.
- Locked users cannot authenticate; lock/unlock is admin-only.
- MFA (TOTP, replay-safe) sets `mfa_pending` on the session until verified.
  TOTP = RFC 6238 HMAC-SHA256, 30 s step, ±1 drift, one-time `otpauth_uri`,
  consumed-step replay protection (`docs/security/auth-mechanisms.md` §2).
- OAuth2/OIDC + PKCE primitives are implemented and tested
  (`src/security/oauth.rs`); login wiring, passkeys and magic links are
  pending. Password reset is a documented gap.

### Authorization (the core of the design)
- **Every** resource access is checked server-side at request time.
- Location reads require: active friendship **and** an active, unexpired, unpaused
  `SharePermission` from sharer → requester, with the requester as recipient.
- Precision downgrade applied server-side before serialization:
  - `exact` → coordinates as published
  - `approximate` → jittered (~few km radius)
  - `city_only` → city centroid only
- Revocation/pause takes effect on the very next request (no cached grants).
- IDOR posture: object IDs are validated against the authenticated principal; failures
  return generic 403/404 with no existence leakage.

### Transport & browser
- TLS mandatory in production (terminate at reverse proxy; HSTS).
- **CSRF**: double-submit token — every unsafe request must echo
  `X-CSRF-Token` (there is no JWT/bearer exemption).
- Security headers set by the Rust server and/or reverse proxy:
  `Strict-Transport-Security`, `X-Content-Type-Options: nosniff`,
  `X-Frame-Options: DENY`, `Referrer-Policy: no-referrer`,
  `Permissions-Policy` (geolocation for this origin only), `Cross-Origin-Opener-Policy: same-origin`,
  `Cross-Origin-Resource-Policy: same-origin`, `X-Permitted-Cross-Domain-Policies: none`,
  and a same-origin CSP on the web client.
- Cookies: `HttpOnly`, `SameSite=Lax`, `Path=/`; HTTPS sessions use the
  `__Host-pinpoint_session` name with `Secure`, so a sibling subdomain cannot
  overwrite them. Loopback HTTP keeps the plain `session_id` name for local dev.
- **Local-only assets:** no Google Fonts/CDNs. The UI uses `system-ui`, serves
  `/app.js`, `/app.css` and `/favicon.svg` itself, and the CSP pins
  `default-src`, `script-src`, `style-src`, `font-src`, `connect-src` and
  `img-src` to `'self'` (plus `data:` images). Browsers only see same-origin
  requests; map tiles go through `/map/tiles`. `tests/assets.rs` enforces this.

### Abuse resistance
- Per-account login lockout (five failed attempts, 15 minutes) and per-client
  throttles on login (30/5 min), registration (10/h), anonymous/recovery
  (20/10 min) and diagnostic reports (10/h), keyed by a short-lived HMAC of the
  client address (never raw IP).
  The header trust model assumes a proxy that overwrites `X-Forwarded-For`; other
  write endpoints rely on authorization plus a 120/min per-account cap
  (`src/server/state/rate_limit.rs`). A general per-IP write limiter and
  proxy-level DDoS controls remain the gap (threat model T-D1).
- The public map proxy is capped per client (tiles 1200/min, search/reverse
  60/min, route 120/min) so one script cannot exhaust the upstream OSM
  providers.
- Minimal error bodies: internal errors surface as generic messages; details go to server logs only.
- Input validation at the boundary (types, ranges, coordinate bounds, string lengths).

### Data protection
- The latest publish is replaced per user; the owner-only trail keeps points for
  7 days on the server, returns the last 24 hours, and is deleted with the account.
- Secrets via environment variables (`.env.example` documents all of them); nothing committed.
- Demo seeds contain fictional data only.

## Known limitations / honest gaps (thesis scope)
- Session/auth core is **implemented** (opaque sessions, CSRF, Argon2id,
  logout/logout-all/session revoke, anonymous create/recover). Remaining:
  anonymous device challenge-response and rate limiting on anonymous
  creation/recovery (`docs/security/session-management.md` §10).
- No OIDC/PKCE **login wiring** yet (the `security/oauth.rs` primitives exist);
  passkeys and magic links are pending.
- E2E location encryption and OS-keychain key storage are implemented
  (`src/security/e2e.rs`, `src/security/keychain.rs`); bookmark encryption
  wiring is the remaining piece.
- Users created through the API are kept in the in-memory store on the demo
  path; the Postgres schema and helpers exist but the write-through repository
  is the next persistence task.
- Audit log exists as structured stdout logs, not a queryable table yet.

## Hardening checklist before any public deployment
- [ ] No demo accounts are seeded; register real accounts and promote an admin deliberately
- [ ] HTTPS-only cookies; real domain for the CORS allowlist
- [ ] Set `PINPOINT_SESSION_IDLE_MINUTES` (30) in the deployment profile; the absolute lifetime is enforced (`PINPOINT_SESSION_MAX_DAYS`, 30 days)
- [x] Per-client keyed-HMAC throttles on login, registration, `/auth/anonymous` and recovery
- [ ] Move the in-memory store to PostgreSQL; enable persistent audit logging
- [ ] Add WAF/CDN, backups, and monitoring/alerting
- [ ] Re-run the security test suite + manual pen-test of new infra
