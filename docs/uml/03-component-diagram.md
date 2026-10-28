# 03 — Backend Component Diagram (C4 L3) 📋

Mirrors actual Go package layout under `backend/`.

```mermaid
graph TD
    CMD[cmd/server/main.go<br/>wiring, graceful shutdown]

    subgraph Internal
        CFG[internal/config<br/>env parsing — SEED_DEMO_DATA, DB_DRIVER...]
        RT[router.go<br/>chi mux, middleware chain]
        SEC[internal/security<br/>csrf, ratelimit, password rules]
        HND[internal/handlers]
        AUTH[handlers/auth.go<br/>login/logout/me, force-change]
        USR[handlers/users.go]
        FRD[handlers/friendships.go]
        LOC[handlers/locations.go<br/>publish + consent-filtered read]
        ADM[handlers/admin.go<br/>invite, list, unlock — extend in T13]
        ST[handlers/static.go<br/>WEB_ROOT serving]
        MOD[internal/models<br/>User, Friendship, Share, Location]
        STR[internal/store<br/>repository interfaces + memory impl<br/>→ Postgres impl T01]
        WS[internal/ws hub<br/>planned T07]
        JOB[internal/jobs<br/>asynq workers planned T06]
    end

    CMD --> CFG & RT
    RT --> SEC --> HND
    HND --> AUTH & USR & FRD & LOC & ADM
    HND --> ST
    AUTH & USR & FRD & LOC & ADM --> MOD
    AUTH & USR & FRD & LOC & ADM --> STR
    LOC -.fan-out.-> WS
    JOB -.expiry sweep.-> STR
```

**Middleware order (current):** RequestID (T15) → RateLimit → Recoverer → CORS → CSRF (state-changing) → Auth(jwt/session) → Role(admin).
