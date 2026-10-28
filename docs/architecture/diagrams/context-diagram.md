# System Context (C4 Level 1)

The system context of **Always Together** — a consent-based location-sharing
safety app. Corresponds to [§3 Context and Scope](../context-and-scope.md).

```mermaid
C4Context
    title Always Together — System Context

    Person(user, "Resident user", "Shares location with trusted friends, completes check-ins, views friends on the map")
    Person(operator, "Operator / Moderator", "Handles reports and incidents via RBAC-restricted access")

    System(app, "Always Together", "Consent-based location sharing: mobile client + API backend with directional authorization")

    System_Ext(push, "Push Notification Service", "FCM / APNs — friend requests, check-in reminders")
    System_Ext(maps, "Map Provider", "OpenStreetMap tiles and geocoding")
    System_Ext(oidc, "Identity Provider", "OIDC/PKCE federation (planned; password login implemented for MVP)")

    Rel(user, app, "Registers, grants/revokes consent, shares location", "HTTPS")
    Rel(operator, app, "Reviews reports, actions moderation", "HTTPS")
    Rel(app, push, "Sends notifications")
    Rel(app, maps, "Requests tiles / geocoding")
    Rel(app, oidc, "Authenticates via OIDC/PKCE (planned)", "HTTPS")
```

## Key properties

- **No public/anonymous consumers**: everything inside the system requires an
  authenticated principal (private by default, SC-03).
- The system never *pushes* precise location anywhere except to a viewer
  holding a valid, unexpired consent record (SC-02).
- External systems receive only what they need: map tiles requests carry no
  friendship data; navigation deep links carry a single coordinate pair.

---
*Next level: [Container View](./container-diagram.md).*
