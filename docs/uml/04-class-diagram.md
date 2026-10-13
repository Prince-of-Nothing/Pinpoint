# 04 — Class Diagram (domain model) 📋

Based on `internal/models` + target persistence (T01/T02).

```mermaid
classDiagram
    class User {
        +string ID
        +string Email
        +string PasswordHash
        +bool MustChangePassword
        +bool IsAdmin
        +bool Locked
        +int FailedAttempts
        +time CreatedAt
        +time LastLoginAt
    }
    class Friendship {
        +string ID
        +string RequesterID
        +string AddresseeID
        +string Status «pending|accepted|rejected»
        +time UpdatedAt
    }
    class Share {
        +string ID
        +string OwnerID
        +string ViewerID
        +string Precision «exact|approximate|city_only»
        +string Purpose
        +time ExpiresAt
        +bool Paused
    }
    class Location {
        +string ID
        +string UserID
        +float Lat
        +float Lon
        +float AccuracyM
        +time Timestamp
    }
    class AuditLog {
        +string ID
        +string ActorID
        +string Action
        +string TargetUserID
        +time At
    }
    class Store {
        <<interface>>
        +CreateUser(User) error
        +GetUserByEmail(string) (User, error)
        +ListVisibleLocations(viewer, owner) []Location
        +UpsertShare(Share) error
        +RecordAudit(AuditLog) error
    }
    class LocationHandler {
        +POST /locations publish
        +GET /users/{id}/visible read w consent
    }

    User "1" -- "0..*" Friendship : participates
    User "1" -- "0..*" Share : owns/grants
    Share "*" -- "1" Viewer : directed to
    User "1" -- "0..*" Location : publishes
    Share ..> Location : gates visibility + precision
    Store <|.. MemoryStore : current impl
    Store <|.. PostgresStore : T01
    LocationHandler --> Store
    AuditLog ..> Share : consent trail (T08)
```

Invariants: a Location is visible to Viewer only if an accepted Friendship exists **and** an unexpired, unpaused Share covers it; precision applied at read time (never mutates stored coords).
