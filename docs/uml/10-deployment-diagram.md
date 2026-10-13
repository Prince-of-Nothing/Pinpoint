# 10 — Deployment Diagrams 📋

## A. Development / demo (current `docker-compose.yml`)

```mermaid
graph TB
    subgraph Host["Developer machine / VM — docker compose up --build"]
        subgraph Net["compose network"]
            W[web container<br/>nginx :8080 → serves SPA,<br/>proxies /api]
            BE[backend container :9000<br/>Go binary, SEED_DEMO_DATA=true,<br/>healthcheck /health]
            PG[(postgres:16-alpine<br/>volume pgdata)] ## T01 active
            RD[(redis:7-alpine<br/>volume redisdata)] ## T06 active
        end
    end
    BR[Browser localhost:8080] --> W
    W -->|proxy /api/v1| BE
    AE[Android emulator<br/>10.0.2.2:8080] --> W
    BE --> PG
    BE --> RD
```

## B. Single-binary minimal mode

```mermaid
graph LR
    B[Browser] -->|:8080 everything| GO2[One Go container<br/>WEB_ROOT=/srv/web<br/>serves SPA + API]
    GO2 --> PG2[(Postgres)]
```

## C. Production target (post-T34)

```mermaid
graph TB
    LB[Cloudflare/LB TLS termination<br/>+ WAF] --> N1[nginx edge ×2]
    N1 --> G1[backend instance 1]
    N1 --> G2[backend instance 2]
    G1 & G2 --> PGM[(Postgres primary)]
    PGM -.streaming replica.-> PGR[(standby)]
    G1 & G2 --> RDS[(Redis HA sentinel)]
    G1 & G2 -.pub/sub cross-node WS (P-B02).-> RDS
    BK[cron: pg_dump → object storage<br/>restore drill in CI T34] --- PGM
    MON[Grafana/Prometheus P-B06] -.scrape.-> G1 & G2
```

Runbook lives in [docs/DEPLOYMENT.md](../DEPLOYMENT.md); secrets policy in SECURITY.md; task refs T05/T34/T36.
