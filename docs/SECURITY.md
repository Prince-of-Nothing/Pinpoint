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
- Passwords hashed with **bcrypt** (cost ≥ 10); never logged, never returned.
- Session cookie (HttpOnly, SameSite=Lax, Secure by default) **and** JWT bearer support.
- Logout invalidates the server-side session (no zombie tokens).
- Admin-provisioned accounts get a `must_change_password` flag; all non-change endpoints
  return 403 until the password is rotated.
- Locked users cannot authenticate; lock/unlock is admin-only.

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
- **CSRF**: double-submit token — session users must echo `X-CSRF-Token`; JWT callers exempt.
- Security headers set by nginx/Go: `X-Content-Type-Options`, `X-Frame-Options: DENY`,
  restrictive `Referrer-Policy`, CSP on the web client.
- Cookies: `HttpOnly`, `SameSite=Lax`, `Secure` (opt-out only for local http dev via
  `COOKIE_INSECURE=true`).

### Abuse resistance
- Per-IP + per-user **rate limiting** on auth and write endpoints (login throttling included).
- Minimal error bodies: internal errors surface as generic messages; details go to server logs only.
- Input validation at the boundary (types, ranges, coordinate bounds, string lengths).

### Data protection
- Location data retained only as "current publish" per user; no historical trails in MVP.
- Secrets via environment variables (`.env.example` documents all of them); nothing committed.
- Demo seeds contain fictional data only.

## Known limitations / honest gaps (thesis scope)
- In-memory store ⇒ sessions/rate counters reset on restart; Postgres/Redis planned (Phase 1).
- No OIDC/PKCE yet (planned Phase 4); current JWTs are short-lived as mitigation.
- No end-to-end encryption; server can see plaintext locations (accepted MVP trade-off,
  documented in the thesis threat model).
- Audit log exists as structured stdout logs, not a queryable table yet.

## Hardening checklist before any public deployment
- [ ] Rotate all default/seed passwords; disable demo seeding (`SEED_DEMO_DATA=false`)
- [ ] Set strong `JWT_SECRET`; HTTPS-only cookies; real domain for CORS allowlist
- [ ] Move store to PostgreSQL; enable persistent audit logging
- [ ] Add WAF/CDN, backups, and monitoring/alerting
- [ ] Re-run security test suite + manual pen-test of new infra layer
