# 0022. MFA disable requires a TOTP proof

- Status: Accepted
- Date: 2026-11-18
- Related: SEC-1 (#50), ADR-0012

## Context

`DELETE /auth/mfa` disabled MFA with only the session cookie and CSRF token.
A stolen live session could therefore remove the second factor, so MFA did not
protect against session theft.

## Decision

`POST /api/v1/auth/mfa/disable` accepts `{ "code": "<6 digits>" }` and verifies
it with the same `verify_totp` + `mfa_last_step` replay guard used by
`/auth/mfa/verify`. The secret and flag are cleared only after a valid, unused
code. Missing, invalid or replayed codes return `401 INVALID_MFA_CODE`; an
account with no enrolled secret returns `400 MFA_NOT_ENROLLED`.

The cookie-only `DELETE /auth/mfa` route is removed. The Android profile screen
collects the code and sends it in the same release.

## Consequences

- A session thief cannot disable MFA without the authenticator.
- Clients must collect a code before disabling; the native client does.
- Anonymous accounts have no password, so the TOTP proof is the only factor.
- The e2e script must present a code; it keeps the enrolment secret only for the
  test run and clears it on success.
