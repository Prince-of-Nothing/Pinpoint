# Always Together Backend (Go)

HTTP API for the Always Together product. The backend was refactored from the
original Python/FastAPI prototype to a single Go service.

## Stack

- **Go 1.19+** with [chi](https://github.com/go-chi/chi) routing
- **JWT** access tokens (`golang-jwt/jwt`) + server-side sessions
- **bcrypt** password hashing (`golang.org/x/crypto/bcrypt`)
- Thread-safe in-memory store (`internal/store`) behind a repository-style
  interface, ready to be swapped for PostgreSQL without touching handlers

## Layout

```
cmd/server/          Entry point (graceful shutdown, PORT env var)
internal/config/     Environment-driven settings (.env.example keys)
internal/models/     Domain entities (User, Session, Friendship, LocationShare, ...)
internal/security/   Password hashing/validation, JWT, session & CSRF tokens
internal/store/      In-memory persistence layer
internal/handlers/   HTTP layer: router, middleware, auth/users/friendships/locations
```

## Running

```bash
go run ./cmd/server            # listens on :8000 (override with PORT)
curl localhost:8000/health
```

Configuration is read from environment variables; see `.env.example` for all
keys and defaults.

## Testing

```bash
go vet ./...
go test ./...
```

## Security features

- Login rate limiting (fixed window per IP) and account lockout after N failed
  attempts with automatic unlock after `LOCKOUT_DURATION_MINUTES`
- Session cookies: HttpOnly, Secure, SameSite=Lax
- CSRF double-submit protection: login returns a CSRF token (JSON body +
  `X-CSRF-Token` header); cookie-authenticated state-changing requests must
  echo it back in the `X-CSRF-Token` header
- All sessions are revoked on password change; logout revokes server-side state
- Directional location sharing permissions (`to_friend`, `from_friend`,
  `mutual`) with per-share precision levels

## API surface

All endpoints live under `/api/v1`:

| Method | Path | Notes |
|---|---|---|
| POST | `/auth/login` | Sets session cookie, returns JWT + CSRF token |
| POST | `/auth/logout` | Revokes session(s), clears cookie |
| GET  | `/auth/me` | Current user profile |
| POST | `/auth/password/change` | Requires current password; revokes all sessions |
| POST | `/auth/password/force-change` | For accounts flagged `must_change_password` |
| POST | `/users/` | Admin-only user provisioning |
| GET  | `/users/`, `/users/{id}` | List / fetch users |
| PATCH | `/users/{id}` | Update profile |
| POST | `/friendships/` | Send friend request |
| GET  | `/friendships/` | List friendships |
| POST | `/friendships/{id}/respond` | Accept/reject/block |
| POST/PATCH/GET | `/locations/shares[/{id}]` | Manage directional shares |
| POST | `/locations/me` | Update own location |
| GET  | `/locations/friends` | Friend locations visible to caller |
