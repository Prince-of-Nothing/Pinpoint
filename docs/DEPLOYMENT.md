# Deployment Guide

## Zero-config local start (recommended)
```bash
docker compose up --build
# Web: http://localhost:8080   API direct: http://localhost:9000/api/v1
```
Demo data auto-seeds (`SEED_DEMO_DATA=true` in compose). Credentials listed in README.

## Shapes
### A. Compose (two containers)
- `backend` — Go API (:9000), multi-stage Dockerfile, non-root, healthcheck on `/health`.
- `web` — nginx serving the SPA + reverse-proxying `/api/` → backend (same origin ⇒ cookies
  work without CORS). Security headers set in `web/nginx.conf`.

### B. Single binary (simplest production-ish demo)
```bash
cd backend
WEB_ROOT=web/public SEED_DEMO_DATA=true go run ./cmd/server
```
One process serves both API and static client on :9000.

## Environment variables (all optional; see `backend/.env.example`)
| Var              | Default | Purpose                                            |
|------------------|---------|----------------------------------------------------|
| `PORT`           | `9000`  | Listen port                                        |
| `JWT_SECRET`     | dev key | **Set strong random value in any real deployment** |
| `SEED_DEMO_DATA` | `false` | Seed admin/alice/bob demo dataset                  |
| `COOKIE_INSECURE`| `false` | Allow non-Secure cookies over plain http (dev only)|
| `WEB_ROOT`       | —       | Serve static SPA from the Go binary                |

## Production checklist (before exposing beyond localhost)
- [ ] `SEED_DEMO_DATA=false`; rotate/remove all demo credentials
- [ ] Strong `JWT_SECRET` (e.g. `openssl rand -base64 48`) via secret manager
- [ ] TLS termination + HSTS; drop `COOKIE_INSECURE`
- [ ] Replace in-memory store with PostgreSQL (ROADMAP Phase 1) — restarts currently wipe sessions
- [ ] Persistent volumes/backups once DB lands
- [ ] Log shipping without coordinates/passwords (see SECURITY.md)
- [ ] Healthcheck wired into orchestrator; graceful shutdown honored (SIGTERM)
- [ ] Pin image digests; scan images (trivy) in CI

## Thesis-demo recipe (defense day)
1. `docker compose up --build` on a laptop.
2. Open http://localhost:8080, log in as Alice (`alice@example.com/DemoPass123!`).
3. Show map with Bob's pin + freshness badge; toggle share precision live.
4. Log in another browser as Bob → demonstrate missing-permission 403 behavior.
5. Admin flow: `admin@example.com/ChangeMe123!` → forced password change → provision user.
