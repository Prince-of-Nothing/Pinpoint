# Deployment View (C4 Deployment)

Physical/infra topology. The development topology is implemented and
documented here; the production topology reflects the frozen MVP stack in
[Technology Stack and Infrastructure](../../reference/technology-stack-and-infrastructure.md)
(hosting details remain risk **R1**). Corresponds to
[§7 Deployment View](../deployment-view.md).

```mermaid
C4Deployment
    title Always Together — Deployment (development / MVP demonstration)

    Deployment_Node(dev, "Developer machine", "Windows/macOS/Linux") {
        Container(uvicorn, "API Service", "uvicorn + FastAPI", "app/main.py; SQLite for quick local runs, PostgreSQL via Docker")
        ContainerDb(sqlite_or_pg, "Database", "SQLite / PostgreSQL", "Dev data only")
    }

    Deployment_Node(cloud, "Cloud environment (planned, provider-neutral)", "MVP stack per ADR-0002") {
        Deployment_Node(edge, "Edge", "") {
            Container(proxy, "Reverse proxy / TLS", "nginx or cloud LB", "TLS termination, HTTP→HTTPS, request size limits")
        }
        Deployment_Node(app_tier, "Application tier", "") {
            Container(api_x2, "API Service ×N", "FastAPI (uvicorn workers)", "Stateless apart from in-memory rate limiter — use Redis backend with N>1")
        }
        Deployment_Node(data_tier, "Data tier", "") {
            ContainerDb(pg, "PostgreSQL (managed)", "Encrypted at rest", "Backups encrypted; least-privilege DB role")
            ContainerDb(redis, "Redis (managed)", "", "Shared rate-limit counters (RATE_LIMIT_BACKEND=redis)")
        }
    }

    Deployment_Node(phone, "User device", "Android / iOS") {
        Container(mobile, "Mobile Client", "Flutter", "Tokens in Keychain/Keystore")
    }

    Rel(mobile, proxy, "HTTPS /api/v1/*")
    Rel(proxy, api_x2, "Forwards")
    Rel(api_x2, pg, "SQL over TLS")
    Rel(api_x2, redis, "TLS")
```

## Deployment security notes

- **TLS everywhere**; the API sets HSTS on HTTPS responses and refuses
  non-allowlisted `Host` headers.
- **Secrets are environment configuration**: production refuses to start
  without a real `SECRET_KEY` (≥ 32 chars, placeholders rejected) — see
  [`backend/.env.example`](../../backend/.env.example).
- **Docs endpoints** (`/docs`, `/redoc`, `/openapi.json`) are disabled when
  `ENVIRONMENT=production`.
- **Multiple workers**: switch `RATE_LIMIT_BACKEND=redis`, otherwise each
  worker enforces its own budget (documented limitation).
- Database schema is currently created at startup (`create_all`); wiring
  Alembic migrations before production is tracked as technical debt.

---
*Part of the [architecture documentation](../README.md) (arc42 §7).*
