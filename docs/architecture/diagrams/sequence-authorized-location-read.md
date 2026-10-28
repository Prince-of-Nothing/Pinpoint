# Sequence — Authorized Location Read (complete mediation)

The canonical authorization scenario from
[§6.2 Control Sequence for a Location Read](../runtime-view.md), as
implemented by `GET /api/v1/locations/friends/latest` in
[`api/locations.py`](../../backend/app/api/locations.py) with
`LocationAccessPolicy` (SC-02, IDOR defence).

```mermaid
sequenceDiagram
    autonumber
    actor V as Recipient (viewer)
    participant API as Location Router
    participant AUTH as get_auth_context<br/>(DI dependency)
    participant POL as LocationAccessPolicy<br/>(Strategy)
    participant RED as PrecisionReducer<br/>(Strategy registry)
    participant DB as PostgreSQL
    participant AUD as AuditLogger

    V->>API: GET /api/v1/locations/friends/latest<br/>Authorization: Bearer <jwt>
    API->>AUTH: resolve credentials
    AUTH->>DB: JWT signature, type=access, exp, ver == user.token_version
    AUTH->>DB: session (sid): not revoked, not expired, idle window
    AUTH-->>API: AuthContext(user)

    API->>DB: list active shares owned by others, linked to viewer
    DB-->>API: [LocationShare...]

    loop each share
        API->>POL: check(viewer, owner, share)
        POL->>DB: friendship accepted? (via RelationshipService)
        POL->>POL: direction owner→viewer?<br/>share valid (active, unrevoked, unexpired)?
        POL-->>API: decision {allowed, precision, reason}
        alt not allowed
            API->>API: skip (no error detail leaked)
        else allowed
            API->>DB: latest LocationUpdate of owner
            API->>RED: get_reducer(precision)
            RED-->>API: reduced (lat, lng) — exact / ~1km / ~11km
            API->>API: stale flag if older than 30 min
        end
    end

    API->>AUD: location.friends_viewed(actor, count)
    API->>DB: commit (audit event)
    API-->>V: 200 {friends_locations: [...]}<br/>only authorized, precision-reduced entries
```

## Why each step matters

| Step | Control |
|---|---|
| 3-5 | One credential path for JWT and cookies; token versioning makes stolen JWTs die on password change |
| 8 | **Complete mediation**: the policy runs per friend — not once per request — so a revoked or expired share disappears immediately |
| 9-11 | Blocks and non-friendships deny silently; the caller cannot probe which friends had shares |
| 13 | Reduction happens **server-side** before the coordinate leaves the API; unknown precision fails closed to the coarsest reducer |
| 15 | One privacy-preserving audit event per list read (ids and counts only) |

Non-allowed reasons are recorded for forensics but never returned to the
caller (`no_active_share`, `share_expired`, `share_revoked`, `blocked`,
`direction_mismatch`).

---
*See [Privacy and Data Handling](../privacy-and-data-handling.md) and
[Security Control Register SC-02](../../reference/security-control-register.md#sc-02-directional-authorization).*
