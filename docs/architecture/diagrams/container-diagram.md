# Container View (C4 Level 2)

Deployable/runnable units of Always Together. Corresponds to
[§5 Building Block View](../building-block-view.md). The API service and its
data stores are implemented; the realtime gateway, workers, and mobile client
are the next increments.

```mermaid
C4Container
    title Always Together — Container View

    Person(user, "Resident user")
    Person(operator, "Operator / Moderator")

    System_Boundary(app, "Always Together")
        Container(mobile, "Mobile Client", "Flutter", "Profiles, map, consent controls, check-ins; stores tokens in Keychain/Keystore")
        Container(api, "API Service", "FastAPI (Python)", "Authentication, directional authorization, rate limits, RFC 7807 errors — SEE component view")
        Container(realtime, "Realtime Gateway", "WebSocket (planned)", "Location/session push to clients (deferred with MVP)")
        Container(workers, "Workers", "Scheduler (planned)", "Check-in reminders, expiry sweeps, notification fan-out")
        ContainerDb(db, "PostgreSQL", "Relational store", "Users, sessions (hashed tokens), friendships, consent records, latest locations, audit events")
        ContainerDb(cache, "Redis", "Cache / shared limiter", "Distributed rate limiting (MVP falls back to per-process memory)")
        ContainerDb(storage, "Object Storage", "S3-compatible", "Media/profile images (deferred with content services)")
    End System_Boundary

    System_Ext(push, "Push Notification Service", "FCM / APNs")
    System_Ext(maps, "Map Provider", "OpenStreetMap")

    Rel(user, mobile, "Uses")
    Rel(operator, api, "Moderation via restricted RBAC", "HTTPS")
    Rel(mobile, api, "REST /api/v1/*", "HTTPS + JWT or session cookie + CSRF")
    Rel(mobile, realtime, "Subscribes", "WSS (planned)")
    Rel(api, realtime, "Publishes events (planned)")
    Rel(workers, api, "Triggers scheduled jobs (planned)")
    Rel(api, db, "Reads/writes via repositories", "SQL over TLS")
    Rel(api, cache, "Rate-limit windows", "TLS")
    Rel(api, storage, "Media URLs (planned)")
    Rel(workers, push, "Delivers notifications (planned)")
    Rel(mobile, maps, "Loads tiles")
```

## Security-relevant container properties

| Container | Property |
|---|---|
| API Service | Only entry point for location data; every read runs complete mediation ([Sequence: Authorized Location Read](./sequence-authorized-location-read.md)) |
| PostgreSQL | Session tokens stored **hashed**; login attempts store keyed email hashes; only the latest location per user is retained |
| Redis | Shared rate-limit counters — required when the API runs with >1 worker (per-process limiter otherwise) |
| Mobile Client | Tokens in platform secure storage; cookies are HttpOnly+SameSite for any web companion |

---
*Next level inside the API service: [Component View — Backend API](./component-backend-api.md).*
