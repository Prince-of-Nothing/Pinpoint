# API Contract (as built)

Version: `api/v1` · Base path: `/api/v1` · Served by the FastAPI backend in
`backend/app`. The machine-readable schema is [`openapi.json`](./openapi.json)
(regenerate with `cd backend && python scripts/export_openapi.py`). In
development, interactive docs are available at `/docs`; they are disabled in
production.

> The longer [API Contract Reference](./api-contract-reference.md) and
> [API Security Implementation Guide](./api-security-implementation.md)
> describe the **target** OIDC/WebSocket design. Where they differ from this
> page, this page describes what is actually implemented (see
> [ADR 0003](../adr/password-auth-with-rotating-refresh-tokens.md)).

## Conventions

- JSON request and response bodies. **Unknown fields are rejected** (422).
- Authentication: `Authorization: Bearer <access_token>` on every endpoint
  except register, login, refresh and health. Tokens are never accepted in
  query strings or cookies.
- Identifiers are random UUIDs. A record that exists but belongs to someone
  else returns the **same 404** as a missing record (and is audited).
- Timestamps are ISO 8601 UTC.
- Errors use Problem Details (RFC 9457) with a stable `code`:

```json
{
  "type": "about:blank#validation_failed",
  "title": "Unprocessable Entity",
  "status": 422,
  "code": "VALIDATION_FAILED",
  "detail": "The request is invalid.",
  "instance": "/api/v1/locations/me",
  "errors": {
    "latitude": ["Input should be less than or equal to 90"],
    "user_id": ["Extra inputs are not permitted"]
  }
}
```

| Status | Meaning |
|---|---|
| 400 | Semantically invalid request (e.g. befriending yourself) |
| 401 | Missing, invalid or expired token, or ended session (`WWW-Authenticate: Bearer`) |
| 403 | Authenticated but not allowed (not friends, not the owner, not admin, CSRF failed) |
| 404 | Not found **or not yours** |
| 409 | Conflict with current state (duplicate request/share, closed check-in) |
| 413 | Body larger than 64 KB |
| 422 | Validation failed (`VALIDATION_FAILED`, `WEAK_PASSWORD`, `INVALID_TIMESTAMP`) |
| 429 | Rate limited; see `Retry-After` |
| 500 | `INTERNAL_ERROR` with a generic message only |

## Endpoints

### Authentication and sessions (`/auth`)

| Method | Path | Purpose | Rate limit |
|---|---|---|---|
| POST | `/auth/register` | Create an account `{username, email, password, display_name?}` → 201 user | 5/hour/IP |
| POST | `/auth/login` | `{login, password, token_delivery: "cookie"\|"body"}` → tokens + user | 10/min/IP |
| POST | `/auth/refresh` | Rotate refresh token. Body `{refresh_token}` (native) **or** cookie + `X-CSRF-Token` header (web) | 30/min/IP |
| POST | `/auth/logout` | Revoke the current session, clear cookies → 204 | |
| POST | `/auth/logout-all` | Revoke every session of the user → 204 | |
| GET | `/auth/sessions` | List own active sessions (`current` flag) | |
| DELETE | `/auth/sessions/{id}` | Revoke one own session → 204 | |
| POST | `/auth/password` | `{current_password, new_password}` → 204; revokes all sessions | 5/15 min/account |

Login response (`token_delivery: "body"`; in cookie mode `refresh_token` is
`null`, the refresh token is set as an `HttpOnly; SameSite=Strict` cookie on
path `/api/v1/auth` and `csrf_token` is returned):

```json
{
  "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "token_type": "bearer",
  "expires_in": 900,
  "refresh_token": "D4yDUbW1_6kOl2XVuMw2XnmE9hpAD_aclng0ZDtw8WE",
  "csrf_token": null,
  "user": {
    "id": "7ea801db-0273-4583-bd56-d0a50e371ecd",
    "username": "alice",
    "email": "alice@example.com",
    "display_name": "Alice",
    "bio": null,
    "role": "user",
    "created_at": "2026-09-29T04:37:36.428923Z",
    "last_login_at": "2026-09-29T04:37:36.451615Z"
  }
}
```

Access-token claims: `sub` (user id), `sid` (session id), `role`,
`type="access"`, `jti`, `iss`, `aud`, `iat`, `nbf`, `exp`.

### Users and profiles (`/users`)

| Method | Path | Purpose |
|---|---|---|
| GET | `/users/me` | Own account |
| PATCH | `/users/me` | Update `{display_name?, bio?}` |
| GET | `/users/{username}` | Viewer-filtered profile (60/min/account) |

`relationship` is `self`, `friend` or `none`. Non-friends and blocked users
receive only `{"username": "...", "relationship": "none"}` (other fields null).
Friend view:

```json
{
  "username": "alice",
  "relationship": "friend",
  "display_name": "Alice",
  "bio": null,
  "friends_since": "2026-09-29T04:37:36.503780Z"
}
```

### Friendships (`/friendships`) and blocks (`/blocks`)

| Method | Path | Purpose |
|---|---|---|
| GET | `/friendships` | `{friends, incoming, outgoing}` |
| POST | `/friendships` | `{username}` → 201 pending request (20/hour/account) |
| POST | `/friendships/{id}/accept` | Addressee only |
| POST | `/friendships/{id}/decline` | Addressee only → 204 |
| DELETE | `/friendships/{id}` | Cancel own request or end friendship; revokes shares both ways → 204 |
| GET | `/blocks` | Users I blocked |
| POST | `/blocks` | `{username}`: ends friendship, revokes shares, hides both users from each other |
| DELETE | `/blocks/{username}` | Unblock → 204 |

### Location shares (`/location-shares`)

| Method | Path | Purpose |
|---|---|---|
| GET | `/location-shares` | Active `{outgoing, incoming}` shares |
| POST | `/location-shares` | `{recipient_username, precision: "approximate"\|"exact", purpose?, expires_in_minutes? (5..10080)}`; recipient must be an accepted friend |
| GET | `/location-shares/{id}` | Owner or recipient only |
| PATCH | `/location-shares/{id}` | Owner only: `{precision?, purpose?, expires_in_minutes?, remove_expiry?}` |
| DELETE | `/location-shares/{id}` | Owner stops sharing or recipient stops receiving → 204 |

Share changes are limited to 30/hour/account.

```json
{
  "id": "28f49eb8-48a1-4c03-8862-071d9ced4f66",
  "direction": "outgoing",
  "owner": {"id": "7ea801db-…", "username": "alice", "display_name": "Alice"},
  "recipient": {"id": "f136275d-…", "username": "bob", "display_name": null},
  "precision": "approximate",
  "purpose": "Walking home",
  "expires_at": "2026-09-29T05:37:36.584725Z",
  "created_at": "2026-09-29T04:37:36.587834Z",
  "revoked_at": null,
  "active": true
}
```

### Locations (`/locations`)

| Method | Path | Purpose |
|---|---|---|
| PUT | `/locations/me` | `{latitude, longitude, accuracy_m?, battery_pct?, is_charging?, recorded_at?}` (60/min/account) |
| GET | `/locations/me` | Own latest location with freshness |
| DELETE | `/locations/me` | Erase own stored location → 204 |
| GET | `/locations/visible` | Everyone who currently shares with the caller |

There is deliberately **no** endpoint that takes another user's id. The
recipient view (approximate share, so coordinates snapped to the grid centre
and accuracy at least ~1.1 km):

```json
[
  {
    "share_id": "28f49eb8-48a1-4c03-8862-071d9ced4f66",
    "user": {"id": "7ea801db-…", "username": "alice", "display_name": "Alice"},
    "precision": "approximate",
    "share_expires_at": "2026-09-29T05:37:36.584725Z",
    "freshness": "live",
    "latitude": 44.425,
    "longitude": 26.105,
    "accuracy_m": 1113.0,
    "recorded_at": "2026-09-29T04:37:36.604136Z",
    "age_seconds": 0,
    "last_battery_pct": 64,
    "is_charging": false,
    "low_battery": false,
    "navigation": {
      "openstreetmap": "https://www.openstreetmap.org/directions?to=44.42500%2C26.10500",
      "google_maps": "https://www.google.com/maps/dir/?api=1&destination=44.42500,26.10500",
      "geo_uri": "geo:44.42500,26.10500"
    }
  }
]
```

`freshness`: `live` (≤ 5 min), `stale` (≤ 60 min), `dead` (older),
`unknown` (no location stored). `is_charging` is only reported while live.

### Check-ins (`/check-ins`)

| Method | Path | Purpose |
|---|---|---|
| GET | `/check-ins` | Own active and last-24h check-ins |
| POST | `/check-ins` | `{due_in_minutes (5..1440), grace_minutes? (0..120), note?}`; max 5 active |
| POST | `/check-ins/{id}/complete` | "I'm OK" |
| POST | `/check-ins/{id}/cancel` | Cancel |
| GET | `/check-ins/shared` | Check-ins of people who currently share their location with me |

`state`: `pending` → `due_soon` (≤ 10 min before due) → `overdue` (within
grace) → `missed`; or `completed` / `cancelled`.

### Administration (`/admin`, role `admin`)

| Method | Path | Purpose |
|---|---|---|
| GET | `/admin/users?limit&offset` | Accounts (no password data) |
| POST | `/admin/users/{id}/unlock` | Clear lockout |
| POST | `/admin/users/{id}/deactivate` | Deactivate and revoke all sessions |
| POST | `/admin/users/{id}/activate` | Reactivate |
| GET | `/admin/audit-events?event_type&limit&offset` | Audit trail |

Admins have no endpoint for locations or shares.

### Health

`GET /api/v1/health` → `{"status": "ok", "version": "0.2.0"}`.

---
*See also [Domain Data Model](./domain-data-model.md) and the
[Security Control Register](./security-control-register.md).*
