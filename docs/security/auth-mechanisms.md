# Authentication Mechanisms — TOTP, OAuth, Passkeys, Recovery & Abuse Controls

Companion to [`session-management.md`](session-management.md) and
[`dual-auth-model.md`](dual-auth-model.md). Status: **partially implemented**
(see the status table in §12).

## 1. Overview

| Mechanism | Purpose | Status |
|---|---|---|
| Email + password | identity sign-in (Argon2id) | ✅ implemented |
| Anonymous device key | no-PII sign-in | 🟡 schema landed, endpoints pending |
| **TOTP (MFA)** | second factor for identity accounts | ✅ implemented |
| OAuth2/OIDC + PKCE | "Sign in with Google/GitHub" | 🟡 primitives implemented, login wiring pending |
| Passkeys (WebAuthn) | phishing-resistant passwordless | ⬜ planned |
| Magic links | passwordless email | ⬜ planned |
| Recovery code | restore an anonymous account | 🟡 design (ADR-0009) |
| Password reset | restore an identity account | ⬜ not implemented (documented gap) |
| Lockout + rate limiting | brute-force resistance | 🟡 lockout implemented; server-side rate limit pending |
| CSRF | cross-site request forgery | ✅ double-submit |
| Audit logging | accountability | ✅ structured logs |
| Scraping detection | abuse detection | ✅ primitives |
| Request signing | tamper-evident requests | ✅ primitives (client-side) |
| Session/device management | list/revoke sessions | ⬜ planned (session store pending) |

## 2. TOTP (MFA) — implemented

**Module:** `src/security/mfa.rs`. Pure Rust, no external auth service.

- **Algorithm:** RFC 6238 TOTP with **HMAC-SHA256**, 30-second step,
  dynamic truncation, 6 digits.
- **Secret:** 160-bit random, base64; stored **server-side only** in
  `pinpoint_users.totp_secret` (never returned after enrolment except the
  one-time `otpauth_uri`).
- **Drift:** verification accepts the previous, current and next step
  (±30 s).
- **Comparison:** constant-time (XOR-fold), fail-closed on malformed secrets.
- **Replay protection:** `verify_totp` returns the matched step; the caller
  stores `mfa_last_step` and rejects any step ≤ the last consumed one.
- **Enrolment flow:**
  1. `POST /api/v1/auth/mfa/enroll` → `{secret, otpauth_uri}` (MFA still off).
  2. User scans the URI in an authenticator app.
  3. `POST /api/v1/auth/mfa/verify {code}` → `mfa_enabled = true`, step consumed.
  4. `GET /api/v1/auth/mfa/status` → `{mfa_enabled}`.
  5. `POST /api/v1/auth/mfa/disable {code}` → clears secret + flag only after a
     valid unused TOTP code (a stolen session alone cannot remove MFA).
- **Login gate:** when `mfa_enabled`, `POST /auth/login` requires a valid
  `totp` in the body; a missing/invalid/replayed code returns
  `MFA_REQUIRED` / `INVALID_MFA_CODE`.
- **Session integration (pending):** sessions carry `mfa_pending`; the
  middleware rejects everything except `/auth/mfa/verify` while pending.
  Once the session store lands, the `totp`-in-login-body check moves to the
  session gate.
- **Recovery:** TOTP has no recovery codes yet. If the authenticator is
  lost, the user must disable MFA with a valid session (or an admin
  unlocks/resets). **Documented gap.**

## 3. OAuth2 / OIDC + PKCE — primitives implemented, wiring pending

**Module:** `src/security/oauth.rs`.

- **Providers:** Google (`openid email profile`) and GitHub
  (`read:user user:email`); client IDs come from env, never the binary.
- **PKCE:** mandatory S256 — `generate_verifier()` (43+ chars, URL-safe) and
  `challenge_s256()` (RFC 7636 vector-tested).
- **State:** opaque, CSRF-bound (`generate_state()`), validated on callback.
- **Redirect allowlist:** exact-match validation (`validate_redirect_uri`),
  no open redirects.
- **Authorization URL:** includes `response_type=code`, `client_id`,
  `redirect_uri`, `scope`, `state`, `code_challenge`,
  `code_challenge_method=S256`.
- **What's missing:** the token exchange, the callback handler, user
  linking/creation, and the session issuance. When wired, an OAuth login
  creates the **same `pinpoint_sessions` row** as password login, with
  `auth_method = oauth` and `identity_subject = provider:sub`.
- **Privacy:** the IdP learns that the user uses Pinpoint; that is the
  explicit trade-off of Model B (identity-linked). Model A (anonymous)
  avoids it.

## 4. Passkeys / WebAuthn — planned

- **Design:** register a platform authenticator (iCloud Keychain / Google
  Password Manager) for the identity account; authenticate with a signed
  challenge. Phishing-resistant, no shared secret.
- **Session:** same server-side session; `auth_method = passkey`.
- **Recovery:** platform sync (iCloud Keychain / Google Password Manager) or
  a fallback password/magic link.
- **Status:** design only; no WebAuthn crate in `Cargo.toml` yet.

## 5. Magic links — planned

- `POST /auth/magic-link {email}` → signed, single-use, expiring link
  (10 min) → `POST /auth/magic-link/verify {token}` → session.
- Rate-limit per email/IP; never reveal whether the email exists.
- Status: design only.

## 6. Recovery codes (anonymous accounts)

- At anonymous sign-up, the server generates a random recovery code, shows
  it **once**, and stores only its PBKDF2 hash (`recovery_code_hash`).
- `POST /auth/anonymous/recover {recovery_code}` → new session + new device
  key registration.
- Without the code (and without the device key), the account is
  unrecoverable **by design** — the UI must state this explicitly.
- Status: schema landed (`recovery_code_hash`); endpoint pending.

## 7. Password reset — not implemented

- Current gap: an identity user who forgets the password cannot recover the
  account (no reset endpoint). Workaround: admin unlock/reset.
- Design when added: email token (single-use, 15 min), rate-limited, always
  return the same response (no account enumeration), revoke all sessions on
  reset.

## 8. Lockout & rate limiting

- **Lockout (implemented):** 5 failed logins → `locked_until = now + 15 min`;
  `ACCOUNT_LOCKED` (429) while locked; reset on success. Admin unlock is
  available via the admin role.
- **Client-side rate limiter (implemented):** `src/security/rate_limit.rs`
  (`api_login`, `api_general`, `nominatim` buckets) used by the UI/app.
- **Server-side rate limiting (pending):** per-IP + per-account limits belong
  at the gateway/service layer. The archived Go backend had this; the Rust
  backend must add it before public traffic.
- **Anonymous abuse:** `/auth/anonymous` must be rate-limited per IP/device,
  optionally with device attestation (App Attest / Play Integrity).

## 9. CSRF

- **Double-submit:** the session issues a random CSRF token (stored in
  `pinpoint_sessions.csrf_token`); unsafe requests must echo it in
  `X-CSRF-Token`.
- **Comparison:** constant-time; missing/mismatched → 403.
- **JWT/bearer clients:** exempt (no cookie auto-send), per ADR-0003.

## 10. Audit logging & abuse detection

- **Audit (`src/security/audit.rs`):** structured events for login success/
  failure, logout, share create/update/delete, publish, check-in, emergency;
  records actor/action/outcome, **never** coordinates or secrets.
- **Scraping detection (`src/security/scrape.rs`):** sliding-window request
  detector + UA allowlist; used to flag enumeration/abuse.
- **Request signing (`src/security/signing.rs`):** HMAC-SHA256 signatures
  with timestamp + nonce; client-side primitive; server-side verification is
  a follow-up.
- **Log hygiene:** no tokens, passwords, coordinates, or recovery codes in
  logs; security events go to structured stdout (queryable table pending).

## 11. Session / device management — planned

Once the session store lands:
- `GET /auth/sessions` → list (device, auth_method, created, last_activity).
- `DELETE /auth/sessions/:id` → revoke one.
- `POST /auth/logout-all` → revoke all.
- Display in Profile → Security.

## 12. Implementation status (honest)

| Piece | Status | Evidence |
|---|---|---|
| TOTP generation/verification | ✅ | `mfa::tests` (RFC vectors, drift, replay, leading-zero) |
| TOTP enrolment/verify/status/disable endpoints | ✅ | `src/server/auth_handlers.rs`; e2e MFA checks |
| TOTP session gate (`mfa_pending`) | ⬜ | pending session store |
| OAuth PKCE/state/redirect primitives | ✅ | `oauth::tests` |
| OAuth callback/token exchange/linking | ⬜ | pending |
| Passkeys | ⬜ | planned |
| Magic links | ⬜ | planned |
| Anonymous recovery code | 🟡 | schema landed; endpoint pending |
| Password reset | ⬜ | documented gap |
| Lockout | ✅ | `auth_handlers::login` |
| Client-side rate limiter | ✅ | `rate_limit` tests |
| Server-side rate limiting | ⬜ | pending |
| CSRF double-submit | ✅ | `tests/contract.rs` |
| Audit logging | ✅ | `src/security/audit.rs` |
| Scraping detection | ✅ | `src/security/scrape.rs` |
| Request signing (client) | ✅ | `src/security/signing.rs` |
| Session/device management | ⬜ | pending session store |

## 13. Trade-offs & decisions

- **TOTP over SMS:** no phone number, no SIM-swap risk, works offline.
- **TOTP over passkeys for now:** pure Rust, no platform SDK, testable.
  Passkeys are the target (ADR-0003) once the WebAuthn crate + platform
  flows are affordable.
- **OAuth primitives deferred wiring:** avoids shipping a half-tested
  callback; the primitives are tested and ready.
- **No password reset yet:** accepted MVP gap; anonymous accounts use a
  recovery code, identity accounts use admin reset until the email flow lands.
- **No recovery codes for TOTP yet:** accepted gap; document it in the UI so
  users enable MFA only after saving backup access.

## 14. Password hashing — Argon2id

- **Algorithm:** Argon2id (RFC 9106), the OWASP first choice; memory-hard
  (resists GPU/ASIC cracking) and no 72-byte input limit (unlike bcrypt).
- **Format:** PHC string `$argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>`
  (`argon2` crate 0.6).
- **Parameters:** `Argon2::default()` (m=19456 KiB, t=2, p=1) — OWASP-aligned.
- **Migration:** legacy `pbkdf2$…` hashes still verify; `needs_rehash()` and
  the login handler transparently re-hash them to Argon2id on the next
  successful login.
- **Why not bcrypt:** bcrypt truncates at 72 bytes and has a recent DoS
  advisory (`RUSTSEC-2026-0199`, patched in 0.19.2); Argon2id is the modern
  recommendation. PBKDF2 remains only for legacy verification and E2E key
  derivation (`src/security/e2e.rs`).
