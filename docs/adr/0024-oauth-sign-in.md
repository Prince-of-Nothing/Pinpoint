# 0024. Optional OAuth 2.0 (Google) Sign-In

Date: 2026-11-25

## Status

Accepted.

## Context

The DSA technical requirements name OAuth 2.0 for API authentication and
authorization. Pinpoint's primary authentication is email/password (Argon2id)
with optional TOTP and opaque server-side sessions (ADR-0012); the API is not
exposed to third-party clients, so OAuth as an *API authorization* mechanism
does not apply. What does apply is federated sign-in: letting a person create
or enter an account through an identity provider instead of a password.

`src/security/oauth.rs` already contained PKCE helpers and provider metadata
from the internship prototype, but no route, configuration or UI used them.

## Decision

Add **optional Google OIDC sign-in** with the authorization-code flow and PKCE
(S256):

- `GET /auth/oauth/google/start` generates an opaque state and a PKCE verifier,
  stores the pending pair for ten minutes, sets a short-lived `HttpOnly`
  `SameSite=Lax` state cookie scoped to `/api/v1/auth/oauth`, and redirects to
  Google.
- `GET /auth/oauth/google/callback` requires the state cookie to equal the
  `state` query value, consumes the state exactly once, exchanges the code
  server-side (client secret and verifier never leave the server), and reads
  the userinfo endpoint.
- The provider identity is stored as `identity_subject = "google:<sub>"` and
  `auth_method = "oauth"`.
- A **verified email** is required. If the email already belongs to a password
  account, the callback refuses with `email_exists` instead of linking
  automatically: an email change at the provider must never become an account
  takeover. Explicit linking while signed in is future work.
- Success issues the same opaque session cookie as password sign-in, so CSRF,
  expiry, revocation and auditing behave identically.
- The provider is enabled only when `PINPOINT_OAUTH_GOOGLE_CLIENT_ID` and
  `PINPOINT_OAUTH_GOOGLE_CLIENT_SECRET` are set; otherwise the buttons are
  hidden and the routes answer `provider_unknown`.
- `PINPOINT_PUBLIC_URL` builds the registered redirect URI:
  `{public}/api/v1/auth/oauth/google/callback`.

GitHub uses the same shape but needs a second userinfo call for the primary
verified email; it is a follow-up rather than part of this decision.

## Consequences

- The course requirement "use OAuth 2.0" is met with a flow that can be
  demonstrated live once a Google OAuth client exists.
- No secrets ship in the repository; the client ID and secret come from the
  environment. Tests use a local mock provider and inject the client into
  `AppState`.
- Pending states are in memory, so a multi-replica deployment without sticky
  sessions fails closed (the callback returns `state_mismatch`). A shared store
  is required before scaling the flow.
- A provider compromise cannot reach an existing password account; the reverse
  (a password user later linking a provider) needs an authenticated linking
  flow, deliberately not built yet.
- Native clients do not use the browser flow; OAuth sign-in is web-only for
  now.

## Alternatives considered

- **Auto-link by verified email.** Simpler for users, but turns any provider
  email change or provider-side account takeover into a Pinpoint takeover.
  Rejected.
- **JWT access tokens for the API.** Would satisfy a literal reading of
  "OAuth for API authorization" but adds a second credential and refresh loop
  without a security gain for a same-origin first-party client (ADR-0012).
- **A self-hosted identity provider.** More operational surface than the MVP
  needs.

## References

- RFC 6749 (OAuth 2.0), RFC 7636 (PKCE), OpenID Connect Core.
- ADR-0012 (opaque sessions), ADR-0006 (transport policy).
- `src/security/oauth.rs`, `src/server/oauth_handlers.rs`,
  `tests/oauth.rs`.
