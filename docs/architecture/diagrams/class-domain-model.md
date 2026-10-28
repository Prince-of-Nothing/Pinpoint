# UML Class Diagram — Domain Model

Persistent domain classes as implemented in
[`backend/app/models/`](../../backend/app/models). Maps to the entities in
[Domain Data Model](../../reference/domain-data-model.md) that the MVP
implements; security-relevant columns are annotated.

```mermaid
classDiagram
    class User {
        +int id
        +str email
        +str email_normalized
        +str password_hash "bcrypt"
        +str full_name
        +UserRole role
        +bool is_active
        +bool is_locked
        +datetime locked_at
        +int failed_login_attempts
        +bool must_change_password
        +int token_version "bump revokes all JWTs"
        +datetime created_at
        +datetime last_login_at
    }

    class Session {
        +int id
        +str token_hash "SHA-256 of cookie value"
        +int user_id
        +str ip_address
        +str user_agent
        +datetime expires_at
        +datetime last_activity_at "idle timeout"
        +datetime revoked_at
        +is_valid() bool
    }

    class Friendship {
        +int id
        +int requester_id
        +int receiver_id
        +FriendshipStatus status
        +datetime created_at
        +datetime responded_at
        +other_party(int user_id) int
    }

    class LocationShare {
        +int id
        +int friendship_id
        +int owner_id
        +LocationShareDirection direction
        +bool is_active
        +str precision_level "exact|approximate|city_only"
        +str purpose
        +datetime expires_at "max 24h"
        +datetime revoked_at
        +datetime last_location_update_at
        +is_currently_valid() bool
    }

    class LocationUpdate {
        +int id "unique per owner: latest only"
        +int owner_id
        +float latitude
        +float longitude
        +float accuracy
        +int battery_level
        +datetime recorded_at
    }

    class LoginAttempt {
        +int id
        +str email_hash "keyed hash, no raw PII"
        +str ip_address
        +bool success
        +datetime attempted_at
    }

    class AuditEvent {
        +int id
        +str event_type
        +int actor_id
        +str target_type
        +str target_id
        +json context "PII-free whitelist"
        +datetime created_at
    }

    class UserRole {
        <<enumeration>>
        RESIDENT
        ADMINISTRATOR
    }

    class FriendshipStatus {
        <<enumeration>>
        PENDING
        ACCEPTED
        REJECTED
        BLOCKED
    }

    class LocationShareDirection {
        <<enumeration>>
        TO_FRIEND "owner shares with friend"
        FROM_FRIEND "informational"
        MUTUAL "requires both parties' own records"
    }

    User "1" --> "0..*" Session : has
    User "1" --> "0..*" Friendship : requester / receiver
    User "1" --> "0..*" LocationShare : owns
    User "1" --> "0..1" LocationUpdate : latest point
    Friendship "1" --> "0..*" LocationShare : grounds consent
    User --> UserRole
    Friendship --> FriendshipStatus
    LocationShare --> LocationShareDirection
```

## Integrity constraints (enforced in the schema)

- `friendships`: `UNIQUE(requester_id, receiver_id)` and
  `CHECK(requester_id <> receiver_id)` — no duplicates, no self-friendship.
  Order-insensitive duplicate prevention (A→B vs B→A) is enforced in the
  service layer.
- `location_updates`: one row per owner (upsert) — **no location history is
  stored**; trails are a deferred, separately consented feature.
- `sessions.token_hash` and `login_attempts.email_hash`: plaintext secrets
  never reach the database.

## Service-layer classes

The non-persistent classes (facades, policies, repositories) are shown in
[Class Diagram — Security Components](./class-security-components.md).

---
*See [Design Patterns and Principles](../../reference/design-patterns.md)
for the rules these classes follow (layering, transactions).*
