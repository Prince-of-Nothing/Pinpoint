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
| `PORT`           | `8000`  | Listen port (Render injects its own)               |
| `SECRET_KEY`     | dev key | **Set strong random value in any real deployment** |
| `DB_DRIVER`      | `memory`| `memory` or `postgres`                             |
| `DB_REQUIRED`    | `false` | Fail startup if Postgres is down / default secret  |
| `DATABASE_URL`   | local   | pgx URL, add `?sslmode=require` for hosted DBs     |
| `ALLOWED_ORIGINS`| local   | Comma-separated CORS origins                       |
| `SEED_DEMO_DATA` | `false` | Seed admin/alice/bob demo dataset                  |
| `COOKIE_INSECURE`| `false` | Allow non-Secure cookies over plain http (dev only)|
| `WEB_ROOT`       | —       | Serve static SPA from the Go binary                |

## Production checklist (before exposing beyond localhost)
- [ ] `SEED_DEMO_DATA=false`; rotate/remove all demo credentials
- [ ] Strong `SECRET_KEY` (e.g. `openssl rand -base64 48`) via secret manager; `DB_REQUIRED=true`
- [ ] TLS termination + HSTS; drop `COOKIE_INSECURE`
- [ ] Replace in-memory store with PostgreSQL (ROADMAP Phase 1) — restarts currently wipe sessions
- [ ] Persistent volumes/backups once DB lands
- [ ] Log shipping without coordinates/passwords (see SECURITY.md)
- [ ] Healthcheck wired into orchestrator; graceful shutdown honored (SIGTERM)
- [ ] Pin image digests; scan images (trivy) in CI

## Free-tier hosting (Supabase + Render + Vercel)

1. **Supabase**: create a project; copy the *session pooler* connection string (port 5432, IPv4
   — Render cannot reach the IPv6-only direct host). Append `?sslmode=require`.
2. **Render**: New → Blueprint from this repo (`render.yaml`); set `DATABASE_URL` and
   `ALLOWED_ORIGINS` (the Vercel origin). `SECRET_KEY` is generated.
3. **Vercel**: import the repo with root directory `web`; edit the Render hostname in
   `web/vercel.json` rewrites. The client stays same-origin, so cookies and CSP are unchanged.

Free-tier limits: Render sleeps after ~15 min idle (cold start of tens of seconds); Supabase pauses
projects after a week of inactivity; Vercel Hobby is non-commercial. WebSockets cannot pass
through Vercel rewrites.

## Thesis-demo recipe (defense day)
1. `docker compose up --build` on a laptop.
2. Open http://localhost:8080, log in as Alice (`alice@example.com/DemoPass123!`).
3. Show map with Bob's pin + freshness badge; toggle share precision live.
4. Log in another browser as Bob → demonstrate missing-permission 403 behavior.
5. Admin flow: `admin@example.com/ChangeMe123!` → forced password change → provision user.
