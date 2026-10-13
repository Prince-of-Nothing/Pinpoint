# 6. Runtime View

This section describes the dynamic behavior of the system during execution, showing how components interact to fulfill key user scenarios. Sequence diagrams illustrate the message flows between actors and system components.

## 6.1 Location Sharing (Core MVP Scenario)

See [Location Sharing Lifecycle](./location-sharing-lifecycle.md) for the full flow:
select recipient/duration → recipient accepts/declines → active sharing displayed
with freshness/accuracy/expiry → either side can stop/revoke/block/report → expiry
triggers deletion or coarse retention.

### Sequence Diagram: Location Sharing Initiation

```mermaid
sequenceDiagram
    participant U as User (Sharer)
    participant M as Mobile App
    participant API as REST API
    participant DB as Database
    participant WS as WebSocket
    participant R as Recipient

    U->>M: Select contact + duration
    M->>API: POST /api/v1/sharing/sessions<br/>(recipient_id, duration, precision)
    API->>DB: Validate relationship & consent
    DB-->>API: Valid consent record
    API->>DB: Create sharing_session
    API-->>M: 201 Created {session_id, expires_at}
    M->>U: Show "Waiting for acceptance"
    
    Note over WS,R: Push notification sent
    WS->>R: Notification: "User wants to share location"
    R->>M: Open app, view request
    R->>M: Accept sharing request
    M->>API: POST /api/v1/sharing/sessions/{id}/accept
    API->>DB: Update session status = active
    API->>WS: Broadcast session activated
    WS->>U: Real-time update: "Sharing active"
    WS->>R: Real-time update: "Location available"
    
    loop Every 30 seconds (or on movement)
        M->>API: PATCH /api/v1/locations/current<br/>(lat, lng, accuracy, timestamp)
        API->>DB: Store encrypted location
        API->>WS: Broadcast location update to recipients
        WS->>R: Render updated position on map
    end
    
    U->>M: Stop sharing (or expiry reached)
    M->>API: DELETE /api/v1/sharing/sessions/{id}
    API->>DB: Mark session revoked, purge precise locations
    API-->>M: 204 No Content
    M->>U: Show "Sharing stopped"
```

*Figure 1: Sequence diagram showing location sharing initiation, real-time updates, and termination.*

**Key Security Checkpoints:**
1. **Consent validation** before session creation (prevents unauthorized sharing)
2. **Explicit acceptance** required before any location transmitted to recipient
3. **Encrypted storage** of all location data at rest
4. **Real-time revocation** immediately stops data flow
5. **Automatic expiry** triggers data deletion per privacy-by-design

## 6.2 Control Sequence for a Location Read

The canonical runtime scenario for authorization-sensitive reads (applies equally
to statuses, caches, friend-map layers, media, reports, and emergency context):

1. Authenticate the caller and validate the token/session.
2. Resolve the immutable caller and target IDs.
3. Check blocks and account state.
4. Check that the caller is the current authorized recipient.
5. Check directional consent, purpose, precision, expiry, and stale status.
6. Apply coarse/approximate transformation if required.
7. Redact fields not necessary for the requested view.
8. Record a privacy-preserving access event.
9. Return only the authorized representation.

### Sequence Diagram: Authorized Location Read

```mermaid
sequenceDiagram
    participant R as Recipient
    participant M as Mobile App
    participant API as REST API
    participant Auth as Auth Service
    participant DB as Database
    participant AL as Audit Log

    R->>M: Open app, view friend map
    M->>API: GET /api/v1/locations/{user_id}
    API->>Auth: Validate JWT token
    Auth-->>API: Token valid, user_id=recipient_123
    API->>DB: Load requester & target accounts
    DB-->>API: Account states (active, not blocked)
    API->>DB: Check active sharing_session
    DB-->>API: Session found, consent=granted,<br/>precision=fine, expires=2024-10-01T20:00:00Z
    API->>DB: Fetch latest location (encrypted)
    DB-->>API: Encrypted location blob
    API->>API: Decrypt location (server-side)
    API->>API: Apply precision filter (fine → no reduction)
    API->>AL: Log access event {who, when, whom,<br/>purpose=friend_map, precision=fine}
    API-->>M: 200 OK {lat, lng, accuracy,<br/>freshness, expires_at}
    M->>R: Render avatar on map at location
```

*Figure 2: Authorization flow for reading another user's location with all security checks.*

See [Security Architecture and Data Protection](./security-architecture-and-data-protection.md)
for the boundary-by-boundary detail (mobile, API, realtime, database, object
storage, operator).

## 6.3 Missing-Person / Search-Corridor Activation (Deferred Feature)

See [Movement Trails and Search Corridors §"Missing-person workflow"](./movement-trails-and-search-corridors.md#missing-person-workflow)
for the full sequence: safety plan creation → pre-authorization → missed check-in
or trusted-person concern → search view activation → last verified point + trail +
corridor + freshness/uncertainty displayed → automatic expiry and access logging.

### Sequence Diagram: Emergency Search Activation

```mermaid
sequenceDiagram
    participant TP as Trusted Person
    participant M as Mobile App
    participant API as REST API
    participant DB as Database
    participant ES as Emergency Service<br/>(deferred)
    
    Note over TP,DB: Pre-condition: Safety plan exists<br/>with TP designated, corridor pre-authorized
    
    TP->>M: Report concern (missed check-in)
    M->>API: POST /api/v1/emergency/search-request<br/>{target_user_id, reason}
    API->>DB: Verify safety plan exists
    DB-->>API: Plan found, TP is authorized
    API->>DB: Check trigger conditions met<br/>(missed_check-in OR manual_concern)
    DB-->>API: Conditions satisfied
    API->>DB: Activate search_corridor
    API->>DB: Retrieve last_known_location + trail
    API->>ES: Notify emergency contact (if configured)
    API-->>M: 201 Created {corridor_id, search_view_token}
    M->>TP: Display search interface:<br/>- Last known point<br/>- Movement trail (coarse)<br/>- Search corridor (500m buffer)<br/>- Uncertainty visualization
```

*Figure 3: Emergency search corridor activation when safety plan triggers are met.*

**Privacy Safeguards:**
- Requires pre-authorized safety plan (cannot be activated ad-hoc)
- Trigger conditions must be objectively verified (missed check-in or explicit concern)
- Search view shows **coarse** locations only (privacy-preserving even in emergencies)
- All access logged with immutable audit trail
- Automatic expiry after configurable period (e.g., 24 hours)

## 6.4 Check-in Lifecycle

Manual, scheduled, and trip check-ins send reminders before any configured
escalation; states are pending, reminder_sent, grace_period, completed, cancelled,
missed. See [API Contract §"Check-Ins"](../reference/api-contract.md) for the
endpoint-level detail.

### Sequence Diagram: Scheduled Check-in Flow

```mermaid
sequenceDiagram
    participant U as User
    participant S as Scheduler<br/>(CloudWatch Events)
    participant API as REST API
    participant N as Notification<br/>(FCM/APNs)
    participant DB as Database
    participant TP as Trusted Person
    
    Note over U,DB: Pre-condition: Scheduled<br/>check-in created (daily at 20:00)
    
    S->>API: Trigger check-in job (cron: 0 20 * * *)
    API->>DB: Find due check-ins
    DB-->>API: List of check_in_ids
    loop For each due check-in
        API->>DB: Set status = reminder_sent
        API->>N: Send push: "Time to check in"
        N->>U: Display notification
    end
    
    alt User checks in within grace period
        U->>API: POST /api/v1/check-ins/{id}/complete
        API->>DB: Set status = completed
        API-->>U: 200 OK "Check-in complete"
    else User misses grace period
        S->>API: Trigger escalation (after 30 min)
        API->>DB: Set status = missed
        API->>DB: Load safety_plan.trusted_persons
        API->>N: Notify trusted persons
        N->>TP: Alert: "User missed check-in"
        opt Escalate to emergency services
            API->>N: Contact emergency services<br/>(if pre-authorized)
        end
    end
```

*Figure 4: Scheduled check-in lifecycle with reminder and escalation paths.*

---

## Diagram Conventions

All sequence diagrams follow these conventions:
- **Participants**: Actors (users, external systems) and system components
- **Solid arrows** (`->>`): Synchronous requests/calls
- **Dashed arrows** (`-->>`): Responses/returns
- **Curved arrows** (`-)`: Asynchronous messages (notifications, events)
- **Rectangles**: Processing steps or state changes
- **Notes**: Preconditions, postconditions, or important context
- **Alt/Opt loops**: Conditional branches and optional flows

For static architecture views, see [Building Block View](./building-block-view.md) and [Deployment View](./deployment-view.md).

---
*Part of the [architecture documentation](./README.md) (arc42 §6).*
