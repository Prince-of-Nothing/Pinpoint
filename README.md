# Always Together

A privacy-first location-sharing platform for trusted friends. Friendship does
not grant access to anyone's location: each person explicitly chooses who can
see them, at what precision, and for how long. Every share can be revoked, and
recipients can see whether a location is fresh or stale.

This repository contains both backend implementations:

- **Go API** in `backend/cmd` and `backend/internal` (the default Docker stack).
- **Python/FastAPI API** in `backend/app` (available as an alternate Docker
  profile and for local development).

Both implementations are accompanied by the web client, tests, database
migrations, security controls, and project documentation.

## Features

- Private-by-default accounts, sessions, and password management
- Friend requests, removal, and blocking
- Directional location sharing with precision, purpose, expiry, and revocation
- Live, stale, and expired location states
- Safety check-ins, admin account management, and audit logging
- Browser client with an OpenStreetMap map

## Quick start: Go API and web client

```bash
docker compose up --build
```

Open <http://localhost:8080>. The local demo stack seeds demo accounts; these
credentials and its default secrets are for local development only.

## Run the Python/FastAPI API

The Python service is available separately on <http://localhost:8001>. It uses
PostgreSQL and Redis from the optional `python` Compose profile:

```bash
docker compose --profile python up --build python-api
```

For local development without Docker:

```powershell
cd backend
python -m venv .venv
.venv\Scripts\Activate.ps1
pip install -r requirements-dev.txt
pytest
```

See [Local Development Setup](docs/tutorials/local-development-setup.md) for
the Python database setup and additional details.

## Run the Go API locally

```bash
cd backend
go test ./...
go run ./cmd/server
```

The Go API listens on port 8000 by default. Set `PORT` to change it.

## Repository layout

```text
backend/app/          Python/FastAPI implementation
backend/alembic/      Python database migrations
backend/tests/        Python test suite
backend/cmd/          Go API entry point
backend/internal/     Go configuration, handlers, models, security, and store
backend/web/          Python-served web client
web/                  Go deployment web client and nginx reverse proxy
docker/               PostgreSQL role setup and Caddy HTTPS configuration
docs/                 Architecture, reference, deployment, and project guides
```

## Documentation

- [Documentation home](docs/README.md)
- [Project overview](docs/PROJECT.md) and [architecture](docs/ARCHITECTURE.md)
- [Deployment](docs/DEPLOYMENT.md) and [testing](docs/TESTING.md)
- [Security policy](docs/SECURITY.md) and [privacy](docs/PRIVACY.md)
- [Python API contract](docs/reference/api-contract.md)
- [Python local development guide](docs/tutorials/local-development-setup.md)
- [Go backend guide](backend/README.md)
- [Roadmap](docs/ROADMAP.md)
