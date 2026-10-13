# 02 — Container Diagram (C4 L2) 📋

Deployment as defined in `docker-compose.yml` + planned services (T01, T06).

```mermaid
graph LR
    B[(Browser)]
    PH[Android Phone]

    subgraph Docker["docker compose network"]
        NGINX[nginx container :8080<br/>serves web/weba/webk static,<br/>reverse-proxies /api → backend]
        GO[Go backend :9000<br/>chi router, handlers,<br/>auth, consent engine]
        PG[(PostgreSQL<br/>T01/T02 — replaces in-memory store)]
        RD[(Redis<br/>rate limits + asynq queue — T06)]
    end

    B <-->|HTTPS/WSS| NGINX
    PH <-->|HTTPS /api/v1| NGINX
    NGINX -->|proxy_pass| GO
    GO -->|pgx SQL| PG
    GO -->|limiter/job client| RD
    GO -.->|FCM HTTP v1| EXT1[Firebase]
    GO -.->|SMTP| EXT2[Mail provider]
```

**Protocols & boundaries**
| Edge | Protocol | Auth |
|------|----------|------|
| Browser↔nginx | HTTPS, WSS (T07) | session cookie + CSRF header |
| App↔nginx | HTTPS | session/JWT, keystore-held |
| nginx↔backend | internal HTTP | none (private network) |
| backend↔PG | TCP 5432 | DSN secret |
| backend↔Redis | TCP 6379 | optional AUTH |

**Single-binary mode:** `WEB_ROOT` set → Go serves clients itself (no nginx container) for simple deploys.
