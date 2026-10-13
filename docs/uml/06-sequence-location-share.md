# 06 — Sequence: Location publish → consent-checked read → fan-out 📋

```mermaid
sequenceDiagram
    autonumber
    participant O as Owner (Android app, T21 scheduler)
    participant API as Go backend
    participant DB as Store (Postgres T01)
    participant V as Viewer (web SPA / app)
    participant WS as WS Hub (T07)
    participant P as Push (FCM T17)

    O->>API: POST /api/v1/locations {lat, lon, accuracy, ts}<br/>Cookie + CSRF header
    API->>API: validate ranges, freshness, rate bucket
    API->>DB: insert Location(owner)
    API-->>O: 201 Created

    V->>API: GET /api/v1/users/{owner}/visible
    API->>DB: Friendship accepted? (V ↔ O)
    API->>DB: Share(O→V): unexpired, unpaused? precision?
    alt no friendship OR no active share
        API-->>V: 403 NO_CONSENT (audited attempt, T08)
    else consent ok
        API->>DB: latest Location of O
        API->>API: apply precision transform:<br/>exact | jitter≈500m | city centroid
        API->>DB: AuditLog(view_read, actor=V, target=O)
        API-->>V: 200 {coords, precision, timestamp, stale?}
    end

    %% realtime path
    O->>API: new location published
    API->>WS: event(loc-updated, owner=O)
    WS-->>V: push to subscribed viewers WITH active shares only
    opt viewer offline & opted-in
        API->>P: FCM notification "Alice shared a new location"
    end
```

Key rule visualised: **precision is applied server-side at read time**; raw coordinates never leave the system at higher precision than the Share grants. Stale flag = timestamp older than freshness threshold shown like web UI states.
