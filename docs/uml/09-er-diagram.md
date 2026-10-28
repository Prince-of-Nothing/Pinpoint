# 09 — ER Diagram (target Postgres schema, T01/T02) 📋

```mermaid
erDiagram
    USERS ||--o{ FRIENDSHIPS : "participates"
    USERS ||--o{ SHARES : "grants"
    USERS ||--o{ LOCATIONS : "publishes"
    USERS ||--o{ SESSIONS : "has (T11)"
    USERS ||--o{ AUDIT_LOG : "actor/target"
    USERS ||--o{ DEVICE_TOKENS : "registers (T17)"
    USERS ||--o{ SHARE_LINKS : "issues (T10 webk)"
    FRIENDSHIPS ||--o{ SHARES : "gates"

    USERS {
        uuid id PK
        varchar email UK "lowercased, unique"
        varchar password_hash "bcrypt"
        bool must_change_password
        bool is_admin "→ role enum in T13"
        varchar role "user|moderator|admin (T13)"
        bool locked
        int failed_attempts
        timestamptz locked_until
        timestamptz created_at
        timestamptz last_login_at
        timestamptz updated_at
    }
    FRIENDSHIPS {
        uuid id PK
        uuid requester_id FK
        uuid addressee_id FK
        varchar status "pending|accepted|rejected"
        timestamptz updated_at
    }
    SHARES {
        uuid id PK
        uuid owner_id FK
        uuid viewer_id FK
        varchar precision "exact|approximate|city_only"
        text purpose
        timestamptz expires_at
        bool paused
        jsonb schedule "windows (T16)"
        jsonb geofence "circle (T16)"
    }
    LOCATIONS {
        uuid id PK
        uuid user_id FK
        numeric lat "check -90..90"
        numeric lon "check -180..180"
        numeric accuracy_m
        timestamptz ts "client timestamp"
        timestamptz received_at
    }
    SESSIONS {
        uuid id PK "session jti"
        uuid user_id FK
        timestamptz expires_at
        inet ip
        varchar user_agent
        bool revoked
    }
    AUDIT_LOG {
        bigserial id PK
        uuid actor_id FK
        uuid target_user_id FK
        varchar action "view_read|share_created|admin_unlock|..."
        jsonb detail
        timestamptz at
    }
    DEVICE_TOKENS {
        uuid id PK
        uuid user_id FK
        varchar token UK
        varchar platform "android"
        timestamptz last_seen
    }
    SHARE_LINKS {
        uuid id PK
        uuid owner_id FK
        varchar token UK "signed, short-expiry"
        varchar precision
        timestamptz expires_at
        int max_uses
        int uses
    }

    index_notes: "LOCATIONS(user_id, ts DESC) BRIN; AUDIT_LOG(at); SHARES(owner_id, viewer_id) UNIQUE"
```

Migration files (`migrations/` via goose, T02) must match this exactly; any column change = new migration + diagram update in the same PR.
