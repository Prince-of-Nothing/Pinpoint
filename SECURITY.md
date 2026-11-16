# Security Policy

Pinpoint is a privacy-first location-sharing app. Security is thesis-mandated
and enforced server-side only.

## Reporting a vulnerability

Please email the maintainers (see `CONTRIBUTING.md`) with a description, steps
to reproduce, and impact. Do **not** open a public issue for security bugs.
We aim to acknowledge within 72 hours.

## Supported versions

Only the latest commit on `main` is supported. Historical stacks removed from
the working tree are not maintained.

## Security controls

- Transport: plain HTTP only for loopback (ADR-0006); production requires HTTPS.
- Auth: PBKDF2-HMAC-SHA256 (salted), TOTP MFA with replay protection.
- Consent: directional shares (to_friend/from_friend/mutual) with precision
  and expiry; enforced server-side; friend coords never sent to OSM services.
- DB: per-service least-privilege roles, FKs, typed migrations v1–v12.
- No trackers, no analytics, no advertising SDKs.

## Known limitations (design-only)

- E2E encryption and OS keychain integration.
- ZAP active scan pending.
- Tiles require a free `CARTO_API_KEY` for live 200s.

See `docs/security/` for the full threat model (STRIDE/DREAD/PASTA) and the
incident-response plan.