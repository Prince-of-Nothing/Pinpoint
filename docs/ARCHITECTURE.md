# Architecture

## Overview
Modular monolith. One Go service owns all business logic and security decisions; clients
(web SPA, future Kotlin app, weba/webk) are thin and untrusted.

```
 Browser / Mobile ──HTTPS──▶ nginx (web, static + proxy)   [compose mode]
                       └──▶ Go server :9000 (/api/v1, optional static WEB_ROOT)
                                     │
                        ┌────────────┼──────────────┐
                        ▼            ▼              ▼
                   internal/     internal/      internal/
                   handlers      security       store (in-memory now → Postgres Phase 1)
                        │            │              │
                        └──── models (User, Friendship, SharePermission, Location) ────┘
```

## Package layout (`backend/`)
| Package             | Responsibility                                                        |
|---------------------|-----------------------------------------------------------------------|
| `cmd/server`        | Entrypoint: config load, router wiring (chi), middleware stack, shutdown |
| `internal/config`   | Env-driven settings (`JWT_SECRET`, `SEED_DEMO_DATA`, `COOKIE_INSECURE`, `WEB_ROOT`, port) |
| `internal/models`   | Domain types: User, Friendship, SharePermission (precision/purpose/expiry/state), Location |
| `internal/store`    | Data access + invariants (email→ID index, cascade deletes, seed). **Swap point for Postgres.** |
| `internal/security` | bcrypt hashing, JWT issue/verify, session store, CSRF double-submit, rate limiter |
| `internal/handlers` | HTTP layer per bounded context: auth, users, friendships, shares, locations, admin, static |

## Request pipeline
1. Middleware: request log → CORS → rate limit → auth (session cookie **or** Bearer JWT)
   → CSRF check (session callers only) → must-change-password gate.
2. Handler validates input strictly (bounds on lat/lon, string lengths, enum values).
3. Handler calls store with the **authenticated principal**; store enforces ownership/consent.
4. Location reads apply precision downgrade server-side before serialization.
5. Errors map to generic client messages; details stay in logs.

## Key design decisions (ADRs in brief)
- **ADR-1 Go replaces FastAPI** — static binary deployment, no runtime deps, native concurrency;
  refactor complete, Python removed.
- **ADR-2 Directional consent records** — `SharePermission{SharerID, RecipientID, Precision,
  Purpose, ExpiresAt, State(active/paused/revoked/expired)}`; friendship is necessary but
  never sufficient.
- **ADR-3 Server-side precision fuzzing** — approximate/city_only computed at read time so a
  changed grant can't leak previously-exact cached payloads.
- **ADR-4 Sessions + CSRF double-submit over pure JWT for browsers** — HttpOnly cookies keep
  tokens out of JS reach; JWT retained for API/mobile consumers.
- **ADR-5 In-memory store for MVP** — deterministic tests & demo; interface designed so
  Postgres drops in without handler changes (Phase 1).
- **ADR-6 Two deployment shapes** — compose (nginx serves SPA + proxies `/api`) or single
  binary serving `WEB_ROOT`; same-origin default avoids CORS complexity.

## Data model sketch
```
User(id, email*, display_name, password_hash, role, must_change_password, locked, timestamps)
Friendship(id, requester_id, addressee_id, status: pending|accepted|rejected|removed)
SharePermission(id, sharer_id, recipient_id, precision, purpose, expires_at, state, timestamps)
Location(user_id PK, lat, lon, accuracy_m, label, battery_pct, updated_at)   -- current publish only
```
\* unique index; store keeps an email→id map that `UpdateUser` maintains in sync.

## Extension points
- Realtime: WebSocket hub in Phase 1 pushing `location.updated` events to consented recipients.
- Check-ins: job queue (Redis) evaluating scheduled/trip check-ins → missed-check-in notifications.
- New clients (`weba`, `webk`, Kotlin mobile): consume the same `/api/v1`; no bespoke endpoints
  per client — authorization lives in one place.

## Scaling path
Stateless API replicas → shared Postgres + Redis (sessions/rate limits) → CDN for tiles/static.
Nothing in the architecture blocks this today; the in-memory store is the only barrier.
