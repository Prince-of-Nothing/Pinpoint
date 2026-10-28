# Roadmap & Future Plans

Statuses: ✅ done · 🚧 in progress · 📋 planned · 🔬 research

## Phase 0 — Foundation (MVP) ✅
- [x] Refactor entire backend from Python/FastAPI to **Go** (chi, bcrypt, JWT/sessions)
- [x] Directional consent model (precision, purpose, expiry, pause/resume)
- [x] Friendship workflow (request/accept/reject)
- [x] Location publishing + map rendering (OSM tiles) + freshness/stale states
- [x] Security baseline: CSRF, rate limiting, forced password change, session invalidation
- [x] Docker deployment: `docker compose up --build`, zero-config demo seeding
- [x] Single-binary option: Go server serves the bundled web client (`WEB_ROOT`)
- [x] Contribution/process docs (this folder)

## Phase 1 — Hardening & persistence 📋 *(next up)*
- [ ] Replace in-memory store with **PostgreSQL** (repositories behind the existing store interface)
- [ ] Migrations tooling (`goose` or `golang-migrate`)
- [ ] Redis-backed rate limiting + job queue (scheduled check-ins)
- [ ] WebSocket channel for realtime location updates (replaces polling)
- [ ] Audit log table (who accessed whose location, when, under which consent)
- [ ] Structured logging (`slog`) + request IDs
- [ ] CI pipeline: build, vet, race-detector tests, Docker image build

## Phase 2 — Clients 🚧
- [x] Web app (vanilla-JS SPA) — login, map, friends, shares, admin
- [ ] **Kotlin / Jetpack Compose Android app** (`mobile/`)
      - Chosen over Flutter/iOS because development happens without macOS/iPhone;
        Android emulators cover the full test loop. iOS (Swift/KMP) revisited later if hardware allows.
      - API client mirroring Go endpoints, credential storage via Android Keystore
      - Battery-aware update scheduler (WorkManager)
- [ ] `weba` — dedicated **admin console** SPA (user ops, audit browsing, share oversight)
- [ ] `webk` — lightweight **kiosk/check-in** web view (quick "I'm here" without full login UX)
- [ ] Push notifications (FCM) for check-in misses & new share requests

## Phase 3 — Feature extensions (each gated by its own threat analysis) 🔬
- [ ] Scheduled & trip check-ins (missed-check-in workflows)
- [ ] Movement trails + search corridors — *high privacy risk; opt-in, expiring, encrypted*
- [ ] Visited-region maps — *aggregation leaks habits; k-anonymity required*
- [ ] Approximate-area presets ("home zone", custom geofences)
- [ ] Emergency mode (panic share to preselected contacts with expiry)
- [ ] Friend mosaics / group views — *only with unanimous per-member consent*

## Phase 4 — Trust & scale 📋
- [ ] OAuth 2.0/OIDC with PKCE, rotating refresh tokens, device/session management
- [ ] End-to-end encryption options for location payloads
- [ ] Horizontal scaling: stateless API + shared Postgres/Redis
- [ ] Moderation tooling, abuse reporting, legal/compliance review (GDPR data export/delete)
- [ ] Public hosting + mobile-store releases

## Backlog / ideas parking lot
- Battery-aware adaptive precision (auto-downgrade when battery < 15%)
- Accessibility pass (screen-reader labels, reduced motion, high-contrast map theme)
- i18n for the web clients
- Load testing harness (k6) for `/locations` hot path
- Terraform deployment for a small VPS

## Decision records worth noting
1. **Go over Python** — single static binary, easy Docker story, real concurrency, no runtime deps.
2. **Vanilla-JS web first** — zero build chain keeps the thesis reproducible; framework migration
   (React/Vue) allowed later without touching the API.
3. **Kotlin over Flutter for mobile** — see Phase 2 rationale (hardware constraints, emulator parity).
4. **nginx sidecar vs self-hosted static** — both supported; compose uses nginx, single-binary uses `WEB_ROOT`.
