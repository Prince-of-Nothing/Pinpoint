# 7. Deployment View

## 7.1 Infrastructure

Use managed encrypted storage, backups with tested recovery, secret management,
TLS, rate limiting, monitoring, and alerting. Separate production location data
from development and analytics environments.

## 7.1a As built (local and demonstration)

`docker-compose.yml` at the repository root:

| Service | Image | Exposure | Notes |
|---|---|---|---|
| `api` | built from `backend/Dockerfile` (non-root user) | `127.0.0.1:8000` | Runs `alembic upgrade head` as the schema owner, then serves as the restricted `app_runtime` role; serves the web client |
| `db` | `postgres:16-alpine` | internal only | `docker/postgres/init-roles.sh` creates `app_runtime` (row DML only) |
| `redis` | `redis:7-alpine` | internal only | Rate-limit counters; no persistence |
| `caddy` (profile `https`) | `caddy:2-alpine` | `127.0.0.1:8443` | TLS with Caddy's internal CA; the API trusts forwarded headers only from Caddy's fixed IP |

Production guard: with `ENVIRONMENT=production` the API refuses to start
with a default/short `SECRET_KEY`, `DEBUG=true` or non-secure cookies,
disables `/docs`, and sends HSTS.

## 7.2 Environments

| Environment | Hosting | Notes |
|-------------|---------|-------|
| Development / internship MVP | Controlled development or test environment | Production claims should not be made until authentication, TLS, encryption, backups, monitoring, incident response, and recovery have been tested |
| Production (future) | Hybrid/managed: containerized application services with managed PostgreSQL, managed object storage | Infra-as-code designed so a later move to self-hosting stays feasible; see [Technology Stack and Infrastructure](../reference/technology-stack-and-infrastructure.md) |

## 7.3 Operational Playbooks

Operational playbooks should cover location-service outage, notification delay,
account takeover, data breach, abusive group, malicious upload, moderation
escalation, and experimental bot resource exhaustion. See
[Incident Response Plan](./incident-response-plan.md).

## 7.4 Rollout

Roll out background-location changes gradually and monitor battery, crash,
permission, and stale-update metrics without collecting unnecessary precise
history.

## 7.5 CI/CD

As built: `.github/workflows/ci.yml` runs ruff (lint + format), bandit,
pip-audit, Alembic upgrade/check/downgrade against PostgreSQL and the test
suite with coverage on every push and pull request.

Docker + GitHub Actions per the frozen stack (see
[§4.2](./solution-strategy.md#42-frozen-mvp-technology-stack)). CI should
include tests, dependency vulnerability scanning, secret scanning, schema-change
checks, and mobile permission behavior checks (see
[Technology Stack and Infrastructure §"Security and operations"](../reference/technology-stack-and-infrastructure.md)).

---
*Part of the [architecture documentation](./README.md) (arc42 §7).*
