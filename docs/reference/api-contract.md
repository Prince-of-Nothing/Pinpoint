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
- **Authentication (ADR-0012):** an opaque session cookie — `__Host-pinpoint_session`
  over HTTPS, `session_id` on loopback — with `HttpOnly; SameSite=Lax`
  (`Secure` on HTTPS) plus an `X-CSRF-Token` header on every unsafe method
  (`POST`/`PUT`/`PATCH`/`DELETE`). There is no `Authorization: Bearer` path
  and no refresh endpoint.
- Errors use a stable envelope:
  ```json
  { "error": { "code": "VALIDATION_FAILED", "message": "email is required" } }
  ```
- Timestamps are ISO 8601 UTC. IDs are integers.
- Text limits below count Unicode **characters**, not UTF-8 bytes. Display text
  (names, labels, note messages, emails, app versions) rejects C0/C1 and Unicode
  bidi control characters; passwords reject every control character.
- Unauthenticated requests to protected endpoints return `401`
  `UNAUTHENTICATED`; a missing/wrong CSRF token returns `403` `CSRF_FAILED`.

### Response headers and throttling

- Every response carries `Strict-Transport-Security`
  (`max-age=31536000; includeSubDomains`), `X-Content-Type-Options: nosniff`,
  `X-Frame-Options: DENY`, `Referrer-Policy: no-referrer`,
  `Permissions-Policy: geolocation=(self), camera=(), microphone=(), payment=(),
  usb=()`, `Cross-Origin-Opener-Policy: same-origin`,
  `Cross-Origin-Resource-Policy: same-origin` and
  `X-Permitted-Cross-Domain-Policies: none`. The web shell adds a strict CSP
  through `web_handlers::secure`.
- `/api/*` responses, including errors, carry `Cache-Control: no-store`, so a
  browser or proxy cache cannot replay one account's data. The geocoding
  answers `/map/search`, `/map/reverse` and `/map/route` are `no-store` for the
  same reason (they contain the query or coordinates). Map tiles carry
  `Cache-Control: public, max-age=3600` plus the ODbL attribution headers;
  static assets keep their own cache headers.
- Auth endpoints, diagnostic reports, the public map proxy and authenticated
  writes are throttled per client address or account
  (`src/server/state/rate_limit.rs`). A throttled request returns `429
  RATE_LIMITED` with `Retry-After: <seconds>`; the client should wait for that
  window instead of retrying immediately.

## Auth — `/api/v1/auth/*`

| Method | Path | Auth | Notes |
|---|---|---|---|
| POST | `/auth/register` | — | `{email, password, full_name}` → `201` session + user. Email is trimmed, lowercased and must be ≤254 characters with exactly one `@` and a dotted domain; full name 1–100 characters; password 8–128 characters |
| POST | `/auth/login` | — | `{email, password, totp?}` → `200` session + user; `MFA_REQUIRED` if enrolled without a valid code |
| POST | `/auth/anonymous` | - | `{device_name?, public_key?}` → `201` session + user + **one-time** `recovery_code`; `device_name` follows the display-name rules (1–100 characters), `public_key` is ≤512 characters; `409 ALREADY_SIGNED_IN` while a session is active |
| POST | `/auth/anonymous/recover` | — | `{recovery_code}` → `200` session + rotated `recovery_code`; old code is invalid |
| GET | `/auth/providers` | — | `{providers:["github","google"]}`; only providers whose client credentials are configured |
| GET | `/auth/oauth/:provider/start` | — | `303` to the provider; sets a 10-minute `HttpOnly` `SameSite=Lax` state cookie scoped to `/api/v1/auth/oauth`; PKCE S256 |
| GET | `/auth/oauth/:provider/callback` | — | verifies the state cookie and PKCE, exchanges the code server-side, requires a verified email, then sets the session cookie and redirects to `/`; an email with an existing password account is refused (`/?oauth_error=email_exists`) and never auto-linked |
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
| POST | `/auth/mfa/disable` | cookie + CSRF | `{code}` → disable MFA; `401 INVALID_MFA_CODE` for missing/invalid/replayed codes, `400 MFA_NOT_ENROLLED` |

Session responses include `csrf_token` in the body **and** an `X-CSRF-Token`
header. The cookie value is stored server-side only as a SHA-256 hash.

## Users & friendships — `/api/v1/*`

| Method | Path | Auth | Notes |
|---|---|---|---|
| GET | `/users/` | cookie + admin | private account listing (email, role, flags) |
| POST | `/users/` | cookie + admin + CSRF | `{email, full_name, role, password}` — the temporary password is required and the account must rotate it |
| GET | `/users/:id` | cookie | self or admin get the full record; an accepted friend gets a friend card; a pending request gets the public card; otherwise `404` |
| PATCH | `/users/:id` | cookie + CSRF | `full_name` (self); `role`, `is_active`, `must_change_password` (admin only, never on self) |
| DELETE | `/users/:id` | cookie + admin + CSRF | remove an account and its rows |
| DELETE | `/users/me` | cookie + CSRF | `{confirm:"DELETE", current_password}` — erase own account and data |
| GET | `/users/lookup?username=` | cookie | exact, case-insensitive handle lookup; public card only |
| GET | `/users/search?q=` | cookie + admin | substring search by email/name/handle |
| PATCH | `/admin/users/:id` | cookie + admin + CSRF | admin console alias: `{role?, is_active?, must_change_password?}` |
| POST | `/admin/users/:id/password` | cookie + admin + CSRF | `{password, reset_mfa?}` — temporary password; revokes sessions |
| DELETE | `/admin/users/:id` | cookie + admin + CSRF | purge an account (admin console) |
| GET | `/friendships/` | cookie | list friendships involving the caller |
| POST | `/friendships/` | cookie + CSRF | `{receiver_username}` → pending request (`receiver_email` is a legacy exact-match fallback) |
| POST | `/friendships/:id/respond` | cookie + CSRF | `{action:"accept"\|"reject"}` (receiver only); only a pending request can be answered (`400 FRIENDSHIP_NOT_PENDING`), and a rejected request needs a fresh one |
| DELETE | `/friendships/:id` | cookie + CSRF | remove a friendship and its shares |

## Locations & shares — `/api/v1/locations/*`

| Method | Path | Auth | Notes |
|---|---|---|---|
| GET | `/locations/shares` | cookie | outgoing shares created by the caller |
| POST | `/locations/shares` | cookie + CSRF | `{friendship_id, direction?, precision_level?, duration_hours? \| expires_at?, label?}` — 24 h default in the UI, `duration_hours` capped at 168; label ≤120 chars |
| PATCH | `/locations/shares/:id` | cookie + CSRF | owner only; any of `is_active`, `precision_level`, `direction`, `label`, `duration_hours`/`expires_at` |
| DELETE | `/locations/shares/:id` | cookie + CSRF | revoke (owner only) |
| POST | `/locations/me` | cookie + CSRF | `{latitude, longitude, accuracy?}` — publish latest location; latitude −90..90, longitude −180..180, `accuracy` 0–10,000,000 m |
| GET | `/locations/me` | cookie | own latest location |
| GET | `/locations/friends` | cookie | friend locations visible under an active share, precision-downgraded |
| GET | `/locations/history` | cookie | owner-only trail, 24-hour window (the database retains 7 days and prunes hourly) |
| GET/POST | `/locations/notes` | cookie (+CSRF) | list / create own private notes: `{latitude, longitude, label?, message, photo?}`; message 1–2000 chars; photo is a JPEG/PNG data URL under 675 KB; stored AES-256-GCM-encrypted when `DATA_ENCRYPTION_KEY` is set |
| DELETE | `/locations/notes/:id` | cookie + CSRF | delete own note |
| GET/POST | `/locations/check-ins` | cookie (+CSRF) | list / create own check-ins: `{latitude, longitude, note?, kind}`; `kind` is `manual`, `scheduled` or `trip`; note ≤2000 chars |
| DELETE | `/locations/check-ins/:id` | cookie + CSRF | delete own check-in |
| GET/POST | `/locations/emergency` | cookie (+CSRF) | list / trigger own emergency share: `{latitude, longitude}` |
| PATCH | `/locations/emergency/:id` | cookie + CSRF | resolve / re-open own alert: `{resolved}` |

Authorization is server-side on every request: friendship alone grants
nothing; an active, unexpired, unpaused `SharePermission` with the caller as
recipient is required, and precision (`exact` / `approximate` / `city_only`)
is applied before serialization.

## Messages - `/api/v1/messages/*`

| Method | Path | Auth | Notes |
|---|---|---|---|
| PUT | `/messages/key` | cookie + CSRF | publish/rotate this device's public X25519 + Ed25519 bundle |
| GET | `/messages/key/:user_id` | cookie | own or accepted friend's public bundle; `404` otherwise |
| POST | `/messages` | cookie + CSRF | `{recipient_id, message}` - store one signed opaque envelope for an accepted friend |
| GET | `/messages/:friend_id` | cookie | thread (`before_id`, `limit`) + the peer's public bundle |
| GET | `/messages/conversations` | cookie | accepted friends with unread counts and last activity; no plaintext |
| GET | `/messages/unread` | cookie | total unread count for the badge |
| POST | `/messages/read` | cookie + CSRF | `{friend_id}` - mark that friend's messages read |

The server stores ciphertext only: X25519 + HKDF-SHA256 + AES-256-GCM with an
Ed25519 signature over the envelope. This is an ECIES construction, **not** the
Signal Protocol; limits and the threat model are in
[ADR-0023](../adr/0023-e2e-messages-ecies.md).

## Map proxy - `/map/*`

| Method | Path | Notes |
|---|---|---|
| GET | `/map/tiles/:z/:x/:y` | ODbL raster proxy. Source order: `PINPOINT_TILE_URL` (self-hosted, ADR-0011), then `tile.openstreetmap.de` (FOSSGIS, demo fallback). Never `tile.openstreetmap.org`. Cached 1 h, returns `X-Tile-Source` and `X-Data-Attribution`. |
| GET | `/map/search?q=&lat=&lon=&limit=` | `q` is 3–120 characters; `lat`/`lon` are an optional pair used for distance bias; `limit` is 1–10 (default 8). |
| GET | `/map/reverse?lat=&lon=` | reverse geocode; coordinates must be valid. |
| GET | `/map/route?from_lat=&from_lon=&to_lat=&to_lon=&mode=&via=&avoid=&alternatives=` | OSRM route; `mode` is a configured profile. |
| GET | `/map/capabilities` | configured routing profiles and whether self-hosted search autocomplete is on. |
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

{ "token_type": "session", "expires_in": 43200,
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
- OAuth/OIDC login wiring, passkeys, magic links and email self-service
  password reset — pending (`docs/security/dual-auth-model.md`). An
  administrator can issue a temporary password today; the user must rotate it
  at the next sign-in.
- WebSocket `/ws` — planned, the demo polls.
