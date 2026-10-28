# Local Development Setup

This tutorial gets the Always Together MVP running on your machine, first
with Docker (fastest), then natively for development and testing.

## Prerequisites

- Docker with Docker Compose v2
- Python 3.11+ (for native development and tests)
- Git
- Optional: Node.js 18+ and Playwright for the browser end-to-end test

## Step 1: Clone the repository

```bash
git clone https://github.com/Bangreedy-2/LocationSharing.git
cd LocationSharing
```

## Step 2: Start the stack with Docker

```bash
docker compose up --build
```

This starts three services:

- **api**: runs the database migrations as the schema owner, then serves
  the API and the web client on <http://localhost:8000> as the restricted
  `app_runtime` database role
- **db**: PostgreSQL 16 (not exposed outside Docker)
- **redis**: Redis 7 for rate limiting (not exposed)

Check it:

```bash
curl http://localhost:8000/api/v1/health
# {"status":"ok","version":"0.2.0"}
```

Open <http://localhost:8000/> for the web client and
<http://localhost:8000/docs> for interactive API docs (development only).

## Step 3: Add demo data (optional)

```bash
docker compose exec api python scripts/seed_demo.py
```

This creates demo accounts with friendships, shares of different precision
and expiry, locations of different freshness and a check-in. The script
prints the accounts and the shared password. See the
[demonstration guide](../how-to/deploy-for-demonstration.md).

Create an administrator (the password is read from the environment or a
prompt, never from the command line arguments):

```bash
docker compose exec -e ADMIN_PASSWORD='a long admin passphrase' api \
    python -m app.cli create-admin --username ops --email ops@example.com
```

## Step 4: Native development (without Docker for the API)

```bash
cd backend
python -m venv .venv
. .venv/bin/activate
pip install -r requirements-dev.txt
cp .env.example .env        # then edit DATABASE_URL / SECRET_KEY
```

Run against a local PostgreSQL and Redis (or keep using the Docker ones by
exposing their ports):

```bash
alembic upgrade head
uvicorn app.main:app --reload
```

For a quick throwaway run without PostgreSQL or Redis:

```bash
DATABASE_URL=sqlite:///./dev.db RATE_LIMIT_BACKEND=memory COOKIE_SECURE=false \
    sh -c 'alembic upgrade head && uvicorn app.main:app --reload'
```

## Step 5: Run the checks

```bash
cd backend
pytest                                   # 140 tests
ruff check . && ruff format --check .
bandit -c pyproject.toml -r app
pip-audit -r requirements.txt
python scripts/security_evidence.py      # regenerates docs/evidence/security-test-report.md
```

Browser end-to-end test (API must be running on port 8000):

```bash
npm install playwright && npx playwright install chromium
node e2e/web-client-flow.js
```

Note: each run registers two new accounts and the registration rate limit
is 5 per hour per IP. Flush Redis (`docker compose exec redis redis-cli
flushall`) between repeated runs.

## Troubleshooting

| Symptom | Cause and fix |
|---|---|
| Login works but reloading the page logs you out | The browser rejected the `Secure` cookie on a non-localhost HTTP host. Use `localhost`, the HTTPS profile, or set `COOKIE_SECURE=false` for local testing only |
| `429 Too Many Requests` | A rate limit was hit; wait for `Retry-After` or flush Redis in development |
| API refuses to start with "Insecure production configuration" | `ENVIRONMENT=production` requires a random `SECRET_KEY` (32+ characters), `DEBUG=false` and `COOKIE_SECURE=true` |
| Map is grey | The browser cannot reach `tile.openstreetmap.org` (offline or blocked network) | 

## Next steps

- [API Contract (as built)](../reference/api-contract.md)
- [Testing and Quality Strategy](../how-to/testing-and-quality-strategy.md)
- [Security Control Register](../reference/security-control-register.md)
