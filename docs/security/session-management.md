# Session Management — Decision & Specification

Companion to [ADR-0012](../adr/0012-opaque-server-side-sessions.md) (the
accepted session design), [ADR-0003](../adr/password-auth-with-rotating-refresh-tokens.md)
(password authentication; its JWT/refresh transport is superseded) and
[ADR-0009](../adr/0009-dual-auth-anonymous-and-identity.md) (dual auth).
See also [`auth-mechanisms.md`](auth-mechanisms.md) for TOTP, OAuth, passkeys,
recovery and abuse controls.
Status: **implemented** (see §10).

## 1. Decision in one paragraph

Pinpoint uses **server-side sessions** identified by a 256-bit opaque
`session_id`, delivered to the browser as an `HttpOnly; SameSite=Lax` cookie
and stored in `pinpoint_sessions` so the auth / users / locations services and
every replica share it. No JWT is issued and no credential is readable by
JavaScript. State-changing requests require a **session-bound CSRF token**
(`X-CSRF-Token` double-submit). The session has a **sliding idle timeout**
(default 12 h in the demo, 30 min recommended for production, configurable
with `PINPOINT_SESSION_IDLE_MINUTES`). Logout, `logout-all`, password change
and session revocation **delete the row immediately**, so the next request
fails closed. This is the concrete implementation of ADR-0012, extended to
cover the anonymous auth model (ADR-0009).

## 2. Why server-side sessions (not stateless JWT only)

| Requirement | Server-side session | Stateless JWT |
|---|---|---|
| Immediate logout / password-change revocation | ✅ delete the row | ❌ valid until expiry (needs denylist) |
| Split services (auth/users/locations) share auth | ✅ one Postgres row | ✅ (shared secret) |
| Audit + session list / device management | ✅ queryable | ❌ |
| MFA-pending state | ✅ column | ⚠️ claim, hard to revoke |
| Cost | one indexed lookup per request | no lookup |

ADR-0003 already rejected stateless-only JWT for exactly the revocation
reason. ADR-0012 goes one step further: once the session row must be checked
for revocation anyway, a signed access token adds verification cost without a
new security property, so the opaque cookie is the only credential. The
per-request lookup is one indexed query (memory-cached in a single process,
shared through Postgres across replicas).

## 3. Data model

### 3.1 `pinpoint_sessions` (migration 14, landed)

| Column | Type | Notes |
|---|---|---|
| `session_id` | TEXT PK | SHA-256 of the 256-bit cookie value; the raw value is never stored |
| `user_id` | BIGINT | FK to `pinpoint_users` |
| `csrf_token` | TEXT | 256-bit random; compared constant-time |
| `auth_method` | TEXT | `email` \| `anonymous` \| `oauth` \| `passkey` |
| `created_at` | TIMESTAMPTZ | absolute lifetime floor |
| `last_activity` | TIMESTAMPTZ | idle-timeout clock |
| `mfa_pending` | BOOLEAN | true until TOTP verified |

Index: `idx_sessions_user (user_id)` for "revoke all sessions" and the
session list.

### 3.2 Refresh tokens — superseded, not needed

ADR-0012 removed the refresh-token table. The session cookie is revocable on
the server at any time and is re-issued on every login/recovery, so there is
no rotation chain to protect. Migration 15 (`pinpoint_refresh_tokens`) is
**not** part of the schema; the design is kept here only as a historical
alternative in case a native client later needs offline-tolerant sessions.

## 4. Tokens & delivery

| Token | Lifetime | Storage | Sent as |
|---|---|---|---|
| Session id | sliding idle 12 h demo / 30 min prod, absolute 30 d | `HttpOnly; SameSite=Lax` cookie (`Secure` over TLS) | `session_id` cookie |
| CSRF token | session lifetime | client memory (returned in `X-CSRF-Token` and the login body) | `X-CSRF-Token` header on unsafe methods |

There is no access JWT, no bearer token, and no refresh token. The CSRF token
is compared against the session row, not signed.

## 5. Lifecycle

```
login / register / anonymous / recover
        │ verify credential (Argon2id / recovery code / device key)
        ▼
create pinpoint_sessions row  ──► Set-Cookie(session) + X-CSRF-Token
        │
        ▼
each request:
  1. parse session cookie
  2. load session row; reject if missing/expired (idle or absolute)
  3. reject unsafe method if X-CSRF-Token ≠ session.csrf_token
  4. reject if mfa_pending and path ≠ /auth/mfa/verify
  5. touch last_activity (memory every 5 min, DB on the same cadence)
  6. handler runs with user_id from the session
        │
        ├─ idle timeout  → delete row on next use (401)
        ├─ logout        → DELETE current session
        ├─ logout-all    → DELETE all sessions for the user
        └─ password change → DELETE all sessions for the user
```

Timeouts: **idle 12 h demo / 30 min prod** (configurable), **absolute 30 days**
(created_at floor). Every authenticated request slides `last_activity`. Expired
rows are pruned when a new session is created; the absolute ceiling is set by
`PINPOINT_SESSION_MAX_DAYS`.

## 6. Endpoints

| Method | Path | Notes |
|---|---|---|
| POST | `/api/v1/auth/login` | email+password → session |
| POST | `/api/v1/auth/register` | create user → session |
| POST | `/api/v1/auth/anonymous` | create a device-bound account → session + one-time recovery code |
| POST | `/api/v1/auth/anonymous/recover` | recovery code → session + rotated code |
| POST | `/api/v1/auth/logout` | revoke current session |
| POST | `/api/v1/auth/logout-all` | revoke every session for the user |
| GET | `/api/v1/auth/sessions` | list active sessions (8-hex prefix, `current` flag) |
| DELETE | `/api/v1/auth/sessions/:prefix` | revoke one session by prefix (own sessions only) |
| POST | `/api/v1/auth/password/change` | re-hash + revoke all sessions |
| POST | `/api/v1/auth/mfa/enroll` \| `/mfa/verify` | TOTP enrollment and step verification |
| DELETE | `/api/v1/auth/sessions/:id` | revoke one session |
| GET | `/api/v1/auth/me` | current user from the session |
| POST | `/api/v1/auth/link` | attach an identity to an anonymous session |
| POST | `/api/v1/auth/mfa/verify` | clear `mfa_pending` (replay-safe TOTP) |
| POST | `/api/v1/auth/mfa/disable` | valid unused TOTP code → disable MFA |

## 7. Split-service behavior

The stack runs auth / users / locations / map as separate services behind the
gateway (`docker-compose.yml`). Sessions must therefore be **shared**:

- **Auth service** creates/deletes sessions.
- **Every service** loads the session row by cookie id (indexed lookup)
  before honoring the request.
- **In-memory mode** (no `DATABASE_URL`, `pinpoint-server` single binary):
  sessions live in an `Arc<Mutex<HashMap<…>>>` on `AppState`; split services
  without Postgres are **not supported** for auth (documented limitation).

The in-process map already acts as the validated-session cache; Postgres is
only consulted on a miss (other replica, restart).

## 8. Security properties

- **Fail closed:** missing/expired session → 401; missing/wrong CSRF on
  unsafe methods → 403; `mfa_pending` → refused by handlers except the verify
  path.
- **Exact CSRF compare** against the session row; the token is never trusted
  from a cookie (double-submit header only).
- **No secrets in logs:** session ids, CSRF tokens and recovery codes are
  never logged (audit logs record user/action/outcome only).
- **Revocation is immediate:** the row is the authority, so logout,
  `logout-all`, password change and per-session revoke take effect on the next
  request — no 15-minute JWT window.
- **Cookie flags:** `HttpOnly; SameSite=Lax` (`Secure` over HTTPS); no
  `Domain` attribute (host-only); `Max-Age=0` on logout.
- **Anonymous sessions:** same machinery plus a one-time recovery code whose
  hash is stored; the code rotates on every successful recovery, so a stolen
  code is single-use.

## 9. Client behavior

- **Web:** the session is an `HttpOnly` cookie the script cannot read; the
  CSRF token comes from the login/anonymous response body or the
  `X-CSRF-Token` header and is attached to POST/PUT/PATCH/DELETE. On 401 the
  UI returns to the sign-in panel (no refresh loop — the cookie is the only
  credential).
- **Rust desktop:** `src/api.rs` captures `Set-Cookie` + `X-CSRF-Token` and
  attaches them on every request; a 401 clears local state.
- **Mobile (when rebuilt):** persist the cookie jar in the
  Keystore/Keychain-backed store; never keep the CSRF token in
  `localStorage`-equivalent plain files.
- **Anonymous mode:** show the recovery code **once** at creation/rotation;
  the server stores only its SHA-256 (the code is 160-bit random, so a
  dictionary attack is infeasible). Device challenge-response is the remaining
  follow-up.

## 10. Implementation status (honest)

| Piece | Status |
|---|---|
| `User` schema fields (`auth_method`, `public_key`, `recovery_code_hash`, `identity_subject`) | ✅ landed (`src/models.rs`) |
| Migration 14 (`pinpoint_sessions` + user auth columns) | ✅ landed (`src/server/migrations.rs`) |
| `load_users` / `persist_users` / `sql_insert_user` include the new columns | ✅ landed (`src/server/state/users.rs`) |
| Session store + `AppState` session methods (create/get/list/revoke/touch/expiry) | ✅ landed (`src/server/state/sessions.rs`) |
| Session/CSRF middleware | ✅ landed (`session_auth` in `src/server/mod.rs`) |
| Real login/register/logout handlers (mock tokens removed) | ✅ landed (`src/server/auth_handlers.rs`) |
| Session list / revoke one / logout-all | ✅ landed + `DELETE /auth/sessions/:prefix` |
| Anonymous `/auth/anonymous` + `/anonymous/recover` (one-time code, rotation) | ✅ landed |
| Web UI: sign-in, anonymous, recovery-code panel, sessions card | ✅ landed (`web/app/`) |
| Refresh-token table (migration 15) | ❌ superseded by ADR-0012 — not needed |
| Tests (session lifecycle, CSRF, revocation, anonymous, router probes) | ✅ 158 unit + 10 contract + 7 router |
| Anonymous device challenge-response (`/anonymous/verify`) | ⬜ follow-up (`public_key` column ready) |
| Rate limiting on `/auth/anonymous` and recovery | implemented (per-client keyed-HMAC throttle) |

## 11. Follow-ups

1. Anonymous device challenge-response: store an Ed25519 public key and issue
   a one-time challenge (`/auth/anonymous/verify`).
2. Per-client keyed-HMAC throttles are in place for login, registration and
   anonymous/recovery, and authenticated unsafe requests are capped at 120/min
   per account; add a general per-IP write limiter and proxy-level DDoS controls.
3. Persist users created via the API in Postgres (`sql_insert_user` is
   available; the demo path is in-memory).
4. The absolute 30-day ceiling and its test are in place
   (`PINPOINT_SESSION_MAX_DAYS`); set the deployment idle timeout to 30 min.
5. Add a session screen to the Rust desktop client (the HTTP endpoints are
   already there).
6. Document the split-service limitation (no Postgres ⇒ single-process auth).
