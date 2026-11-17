# API Contract (as built, Rust)

Version: `api/v1` · Base path: `/api/v1` · Implemented by the Axum server in
`src/server/` (`pinpoint-server` for the combined demo, or the split
`pinpoint-auth` / `pinpoint-users` / `pinpoint-locations` / `pinpoint-map`
services behind the gateway).

This page describes **what is implemented today**. The contract is pinned by
`tests/contract.rs` and `tests/router_probe.rs`; route definitions live in
`src/server/mod.rs`. The Go/FastAPI-era documents
([`api-contract-reference.md`](./api-contract-reference.md),
[`api-security-implementation.md`](./api-security-implementation.md)) describe
the historical/target design and are **not** the current contract.

## Conventions

- JSON request and response bodies; unknown JSON fields are ignored.
- **Authentication (ADR-0012):** an opaque `session_id` cookie
  (`HttpOnly; SameSite=Lax`, `Secure` over HTTPS) plus an `X-CSRF-Token`
  header on every unsafe method (`POST`/`PUT`/`PATCH`/`DELETE`). There is no
  `Authorization: Bearer` path and no refresh endpoint.
- Errors use a stable envelope:
  ```json
  { "error": { "code": "VALIDATION_FAILED", "message": "email is required" } }
  ```
- Timestamps are ISO 8601 UTC. IDs are integers.
- Unauthenticated requests to protected endpoints return `401`
  `UNAUTHENTICATED`; a missing/wrong CSRF token returns `403` `CSRF_FAILED`.

## Auth — `/api/v1/auth/*`

| Method | Path | Auth | Notes |
|---|---|---|---|
| POST | `/auth/register` | — | `{email, password, full_name}` → `201` session + user |
| POST | `/auth/login` | — | `{email, password, totp?}` → `200` session + user; `MFA_REQUIRED` if enrolled without a valid code |
| POST | `/auth/anonymous` | — | `{device_name?, public_key?}` → `201` session + user + **one-time** `recovery_code` |
| POST | `/auth/anonymous/recover` | — | `{recovery_code}` → `200` session + rotated `recovery_code`; old code is invalid |
| POST | `/auth/session` | cookie | validate the cookie, return `{user}` + `X-CSRF-Token` |
| GET | `/auth/me` | cookie | current user |
| POST | `/auth/logout` | cookie + CSRF | revoke the current session, clear cookie |
| POST | `/auth/logout-all` | cookie + CSRF | revoke every session for the user |
| GET | `/auth/sessions` | cookie | `{sessions:[{id, current, auth_method, mfa_pending, created_at, last_activity}]}` — `id` is an 8-hex prefix, never the full token |
| DELETE | `/auth/sessions/:id` | cookie + CSRF | revoke one own session by prefix; clears the cookie if it was current |
| POST | `/auth/password/change` | cookie + CSRF | `{current_password, new_password, confirm_password}`; revokes all sessions |
| POST | `/auth/password/force-change` | cookie + CSRF | `{new_password, confirm_password}`; for `must_change_password` accounts, revokes all sessions |
| POST | `/auth/mfa/enroll` | cookie | generate a TOTP secret + `otpauth_uri` |
| POST | `/auth/mfa/verify` | cookie | `{code}` → enable MFA |
| GET | `/auth/mfa/status` | cookie | `{mfa_enabled}` |
| DELETE | `/auth/mfa` | cookie + CSRF | disable MFA |

Session responses include `csrf_token` in the body **and** an `X-CSRF-Token`
header. The cookie value is stored server-side only as a SHA-256 hash.

## Users & friendships — `/api/v1/*`

| Method | Path | Auth | Notes |
|---|---|---|---|
| GET | `/users/` | cookie | list users (scoped by role) |
| POST | `/users/` | cookie | create a user (admin flows) |
| GET | `/users/:id` | cookie | user by id |
| PATCH | `/users/:id` | cookie + CSRF | update own profile / admin update |
| DELETE | `/users/:id` | cookie + CSRF | delete (admin) |
| DELETE | `/users/me` | cookie + CSRF | `{confirm:"DELETE", current_password?}` — delete own account and data (cascade) |
| GET | `/users/search?q=` | cookie | search by email/name |
| GET | `/friendships/` | cookie | list friendships involving the caller |
| POST | `/friendships/` | cookie + CSRF | `{receiver_email}` → pending request |
| POST | `/friendships/:id/respond` | cookie + CSRF | `{action:"accept"\|"reject"}` (receiver only) |
| DELETE | `/friendships/:id` | cookie + CSRF | remove a friendship |

## Locations & shares — `/api/v1/locations/*`

| Method | Path | Auth | Notes |
|---|---|---|---|
| GET | `/locations/shares` | cookie | shares created by the caller |
| POST | `/locations/shares` | cookie + CSRF | `{friend_id, precision_level, purpose?, expires_at?}` |
| PATCH | `/locations/shares/:id` | cookie + CSRF | `{is_active}` — pause/resume (owner only) |
| DELETE | `/locations/shares/:id` | cookie + CSRF | revoke (owner only) |
| POST | `/locations/me` | cookie + CSRF | `{latitude, longitude, accuracy?}` — publish latest location |
| GET | `/locations/me` | cookie | own latest location |
| GET | `/locations/friends` | cookie | friend locations visible under an active share, precision-downgraded |
| GET/POST | `/locations/check-ins` | cookie (+CSRF) | list / create a check-in |
| DELETE | `/locations/check-ins/:id` | cookie + CSRF | delete own check-in |
| GET/POST | `/locations/emergency` | cookie (+CSRF) | list / trigger an emergency share |
| PATCH | `/locations/emergency/:id` | cookie + CSRF | resolve |

Authorization is server-side on every request: friendship alone grants
nothing; an active, unexpired, unpaused `SharePermission` with the caller as
recipient is required, and precision (`exact` / `approximate` / `city_only`)
is applied before serialization.

## Map proxy — `/map/*`

| Method | Path | Notes |
|---|---|---|
| GET | `/map/tiles/:z/:x/:y` | ODbL raster proxy. Order: `PINPOINT_TILE_URL` (self-hosted, ADR-0011) → `tile.openstreetmap.de` → Wikimedia → CARTO. Never `tile.openstreetmap.org`. Cached 1 h, returns `X-Tile-Source` and `X-Data-Attribution`. |
| GET | `/map/attribution` | source list + license |
| GET | `/map/health` | tile-cache health |

## Offline packs — `/offline/packs/*`

| Method | Path | Notes |
|---|---|---|
| GET | `/offline/packs/moldova/manifest.json` | committed manifest (`web/app/packs/moldova.json`); `files[].url` is already same-origin |
| GET/HEAD | `/offline/packs/moldova/:file` | whitelisted `moldova.pmtiles`, `moldova-search.sqlite`, `moldova-valhalla.tar`. Streams the GitHub release asset, forwards `Range` (206 + `Content-Range`), always sends `Accept-Ranges: bytes`, `Access-Control-Allow-Origin: *` and `Access-Control-Expose-Headers`. Any other pack id or file name is `404` before an outbound request. `PINPOINT_PACK_BASE_URL` overrides the upstream base (tests/local fixtures). |

## Operations

| Method | Path | Notes |
|---|---|---|
| GET | `/health` | `{status, version, mode, database}` |
| GET | `/api/v1/sync-status` | replica convergence state (`mode`, `database_connected`) |
| GET | `/` | web client (`web/app/`) with CSP + security headers |
| GET | `/app.js`, `/app.css` | web assets (same-origin only) |
| GET | `/favicon.svg`, `/favicon.ico` | locally generated icon (no icon CDN) |

## Example: sign in and publish

```http
POST /api/v1/auth/login
Content-Type: application/json

{ "email": "alice@example.com", "password": "DemoPass123!" }
```

```http
HTTP/1.1 200 OK
Set-Cookie: session_id=…; Path=/; HttpOnly; SameSite=Lax
X-CSRF-Token: 3f2c…

{ "token_type": "session", "expires_in": 900,
  "csrf_token": "3f2c…", "user": { "id": 1, "email": "alice@example.com", … } }
```

```http
POST /api/v1/locations/me
Cookie: session_id=…
X-CSRF-Token: 3f2c…
Content-Type: application/json

{ "latitude": 47.0105, "longitude": 28.8638, "accuracy": 10 }
```

## Not in the contract

- `/auth/refresh` and bearer JWTs — removed by ADR-0012.
- OAuth/OIDC login wiring, passkeys, magic links and password reset — pending
  (`docs/security/dual-auth-model.md`).
- WebSocket `/ws` — planned, the demo polls.
