# Always Together

Privacy-first location sharing for trusted friends. Being friends never
reveals anyone's location: each person explicitly chooses **who** can see
them, **how precisely** (exact or about 1 km) and **for how long**, and can
stop at any time. Recipients always see how fresh a location is, so an old
point is never mistaken for a live one.

This repository contains the September MVP built during the Development of
Secure Applications internship: a FastAPI backend, a PostgreSQL schema, a web
client with an OpenStreetMap map, and the security controls, tests and
documentation behind them.

## Features (MVP)

- Accounts with private-by-default profiles; session and password management
- Friend requests, removal and blocking
- **Directional location sharing**: exact or approximate, optional purpose
  and expiry, revocable by either side
- Live / stale / dead freshness, last-known battery, navigation links
- Safety check-ins with a grace period and a visible "missed" state
- Admin account management and a security audit log
- Web client: map, friends, sharing, check-ins, account

## Security highlights

| Control | Implementation |
|---|---|
| Authentication | Argon2id, NIST-style password policy, lockout, generic errors, rate limits |
| Sessions | 15-minute JWTs bound to server-side sessions; rotating refresh tokens with reuse detection; HttpOnly SameSite=Strict cookie + CSRF token for the web |
| Authorization | Consent re-checked on every read; no endpoint takes another user's id for locations; audited IDOR denials; admin RBAC with no location access |
| Input and errors | Strict schemas (unknown fields rejected), body size limit, Problem Details errors without leaks |
| Browser | Strict CSP (`script-src 'self'`), text-only rendering, security headers |
| Data | Latest location only, approximate snapping, least-privilege DB role, append-only audit table |

Details, tests and limitations: [Security Control Register](docs/reference/security-control-register.md)
and the generated [Security Test Report](docs/evidence/security-test-report.md).

## Quick start (Docker)

```bash
docker compose up --build                 # API + PostgreSQL + Redis
open http://localhost:8000/               # web client (API under /api/v1)

# optional: demo users and an administrator
docker compose exec api python scripts/seed_demo.py
docker compose exec -e ADMIN_PASSWORD='choose a long passphrase' api \
    python -m app.cli create-admin --username ops --email ops@example.com

# optional HTTPS front-end on https://localhost:8443 (Caddy internal CA)
docker compose --profile https up --build
```

## Development

```bash
cd backend
python -m venv .venv && . .venv/bin/activate
pip install -r requirements-dev.txt
pytest                                    # 140 tests, in-memory SQLite
ruff check . && bandit -c pyproject.toml -r app && pip-audit -r requirements.txt
python scripts/security_evidence.py       # regenerate the evidence report
```

Full setup, including running against a local PostgreSQL:
[Local Development Setup](docs/tutorials/local-development-setup.md).

## Repository layout

```
backend/
  app/            FastAPI application (api/, core/, db/, models/, schemas/, services/)
  alembic/        database migrations
  tests/          pytest suite, including security attack scenarios
  web/            web client (HTML/JS, vendored Leaflet)
  scripts/        start-up, demo seed, evidence and OpenAPI export scripts
docker/           PostgreSQL role setup, Caddy HTTPS config
e2e/              Playwright browser flow
docs/             architecture (arc42), ADRs, reference, how-to, evidence
```

## Documentation

- [Documentation home](docs/README.md)
- [Implementation Plan and Status](docs/how-to/implementation-plan.md) and
  [Development Plan: October to December](docs/how-to/development-plan-october-december.md)
- [API Contract (as built)](docs/reference/api-contract.md)
- [Demonstration guide](docs/how-to/deploy-for-demonstration.md) and
  [demo script](docs/how-to/mvp-demo-script.md)
