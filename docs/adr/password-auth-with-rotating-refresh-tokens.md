# 0003. Password authentication with session-bound JWTs and rotating refresh tokens for the MVP

Date: 2026-09-29

## Status

Accepted. Supersedes the "Identity & auth: OIDC/OAuth 2.0 with PKCE S256" row
of the frozen MVP stack for the September MVP only; OIDC/passkeys remain the
target (see the October-December plan).

## Context

The frozen stack specified OAuth 2.0/OIDC with PKCE, passkeys and device
management. Running a compliant authorization server (or integrating a hosted
one), a mobile redirect flow and passkey enrolment was not achievable in the
remaining internship time alongside the core safety features. The internship
task list explicitly requires secure authentication, password hashing,
secure JWT/session handling and rate limiting to be implemented and tested.
The existing skeleton already used passwords and JWTs, but insecurely (the
cookie was never sent, lockout was permanent, refresh tokens were accepted as
access tokens, and JWTs stayed valid after logout).

## Decision

We will ship first-party username/email + password authentication with:

- Argon2id hashing and an NIST SP 800-63B password policy;
- 15-minute access JWTs (algorithm pinned, issuer/audience/type checked)
  that carry a server-side session id and are rejected once that session is
  revoked, idle or expired;
- opaque refresh tokens stored as hashes, rotated on every use, with reuse
  detection that revokes the session;
- two delivery modes: an HttpOnly SameSite=Strict cookie plus a
  session-bound CSRF token for the web client, and response-body tokens for
  future native clients;
- lockout, per-IP/per-account rate limits and audit events.

Alternatives considered:

- *Full OIDC + PKCE now* (rejected for time; highest integration risk).
- *Hosted identity provider* (rejected for the MVP: adds an external
  dependency and data processor before the privacy review is done).
- *Stateless JWT only* (rejected: logout and compromise response would not
  take effect until expiry).

## Consequences

- Positive: every Task 6 control is implemented and testable in the repo;
  revocation is immediate; the token design maps directly onto an OIDC
  resource server later (session id ≈ `sid`, rotation semantics identical).
- Negative: no MFA or passkeys yet; the API holds password hashes; account
  recovery is not implemented. These are recorded as limitations in SC-01.
- Migration path: add an OIDC login that creates the same server-side
  session, then retire password login once passkeys are available.
