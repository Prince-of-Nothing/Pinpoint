# Architecture

> Current architecture (Rust). The arc42 documentation in
> [`docs/architecture/`](https://github.com/Prince-of-Nothing/Pinpoint/wiki/Architecture-Index) is the detailed source; the
> historical Go/FastAPI/Kotlin trees were removed from the working tree and
> are **not** built. Use git history or ADR-0007 if you need them.

## Overview

One Rust crate (`pinpoint-desktop`) serves every target. The API is Axum with
five independently scalable routers (auth, users, locations, messages, map)
behind a gateway in compose deployments, or one combined `pinpoint-server`
binary for the demo. All authorization and precision downgrading is server-side.

```
Desktop (eframe) ─┐
Web (web/app)    ─┼── HTTPS / loopback ──▶ Axum routers ──▶ AppState
Android / iOS    ─┘        (cookie + CSRF)      │              │
                                                ▼              ▼
                                          session_auth    in-memory store
                                          (ADR-0012)      or Postgres (ADR-0010)
```

## Module layout (`src/`)

| Path | Responsibility |
|---|---|
| `main.rs` | Desktop entry (eframe/egui) |
| `lib.rs` | Portable core module declarations |
| `api.rs` | HTTP client: session cookie + `X-CSRF-Token`, transport policy (ADR-0006) |
| `app.rs`, `ui.rs`, `ui/` | Desktop state/screens (per-screen modules, including `messages.rs`) |
| `models.rs` | Serde domain models + pure math (precision, freshness, message envelopes) |
| `server/mod.rs` | Routers, `session_auth` middleware, tile proxy, health |
| `server/auth_handlers.rs` | Register/login/logout/sessions/anonymous/MFA/password |
| `server/messages_handlers.rs` | E2E message key registry, send/thread/inbox/read endpoints |
| `server/state/` | Store facade: users, friendships, locations, sessions, messages |
| `server/migrations.rs` | Postgres migrations 1-17 (`pinpoint_messages` in 17) |
| `security/` | One mechanism per file: password (Argon2id), session, CSRF, rate limit, encryption, audit, signing, MFA, OAuth primitives, `message_crypto` |
| `patterns/` | Repository, Strategy, Command, State, Observer, Factory, Decorator, Proxy |
| `map.rs`, `tilepacks.rs`, `bookmarks.rs` | Pins, offline tiles, on-device bookmarks |
| `map_server.rs`, `map_window.rs` | Loopback Range server and the opt-in (`map-webview`) native/web browser map window; egui map remains the fallback |
| `osm.rs` | Nominatim/OSRM/Overpass (friend coordinates never sent) |
| `platform.rs` | All `cfg(target_os)` branches |
| `webapp/`, `mobile/` | Portable web clients and mobile adapters |
| `bin/` | One binary per deployable (`pinpoint-server`, `auth`, `users`, `locations`, `messages`, `map`, `web`) |

## Request pipeline

1. `converge` middleware (replica convergence tick) → `session_auth`:
   parse the `session_id` cookie, load the session row (hash lookup),
   reject expired sessions, and require exact `X-CSRF-Token` on unsafe
   methods (ADR-0012).
2. Handler validates input (coordinate bounds, lengths, enums) and uses the
   **session user** — never a body-supplied id.
3. Store enforces ownership and consent; friendship alone grants nothing.
4. Location reads apply precision downgrade (`exact` / `approximate` /
   `city_only`) before serialization.
5. Errors use `{error:{code,message}}`; details stay server-side.

## Key decisions (see `docs/adr/`)

- **ADR-0007** — Rust multiplatform is the only application codebase.
- **ADR-0008** — split API services behind a gateway (compose) with a combined
  binary for the demo.
- **ADR-0009** — dual auth: anonymous device-bound accounts + identity-linked
  accounts (email now; OAuth/passkeys pending).
- **ADR-0010** — normalized, application-encrypted Postgres schema with blind
  indexes; no single JSONB source of truth.
- **ADR-0011** — self-hosted OSM services in production; public endpoints are
  demo fallbacks with ODbL attribution.
- **ADR-0012** — opaque server-side sessions (hash-stored), not JWT + refresh;
  immediate revocation and one HttpOnly credential.

## Data model sketch

```
User(id, email, full_name, password_hash?, role, must_change_password,
     auth_method, public_key?, recovery_code_hash?, identity_subject?,
     mfa_enabled, totp_secret?, failed_login_attempts, locked_until?)
Friendship(id, requester_id, receiver_id, status, timestamps)
SharePermission(id, sharer_id, recipient_id, precision, purpose,
                is_active, expires_at?, timestamps)
Location(user_id PK, latitude, longitude, accuracy, updated_at)  -- latest only
Session(session_id_hash PK, user_id, csrf_token, auth_method,
        mfa_pending, created_at, last_activity)
```

## Extension points

- Realtime: WebSocket fan-out to consented recipients (planned; the demo polls).
- Check-ins: shared job queue evaluating scheduled/trip check-ins.
- New clients consume the same `/api/v1`; authorization stays in one place.
- New tables: migration + domain module; new screens: `src/ui/<name>.rs`.

## Scaling path

Stateless API replicas → shared Postgres (sessions, users, locations) → Redis
for rate limits/jobs → self-hosted OSM (tiles, Nominatim, OSRM, Overpass) and
a CDN for static assets. The in-memory store is the demo path only; the schema
and session sharing are already in place.
