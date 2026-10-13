# 3. Context and Scope

## 3.1 Business Context

### Actors

- **User:** owns a profile, devices, permissions, caches, media, and memberships.
- **Trusted friend:** receives location only after explicit consent.
- **Group member:** participates in a chat or community group without receiving
  location by default.
- **Creator:** owns group rules, membership settings, prompts, or challenges.
- **Moderator:** reviews reports, abusive content, and group violations.
- **Platform operator:** manages reliability and security without routine access to
  precise private location.

Accounts support pseudonymous display names, account recovery, device/session
management, blocking, reporting, and deletion. See
[Actors and Accounts](../reference/actors-and-accounts.md) and
[Profile and Visibility Model](../reference/profile-and-visibility-model.md) for
identity/visibility rules.

### External systems

| Communication Partner | Input | Output |
|------------------------|-------|--------|
| End user (mobile app) | Location updates, friend requests, consent changes | Friends' locations (per consent), notifications, stale-state indicators |
| OpenStreetMap-derived tile/routing provider | Map/route requests | Tiles, routes, geocoding — no private location history or personal data is sent to map services |
| Push notification service (platform-native) | Event triggers (check-ins, sharing changes, reminders) | Notifications with opaque event IDs/generic text — never precise locations or private content |
| OIDC/OAuth 2.0 identity provider | Auth code + PKCE verifier | Short-lived access tokens, rotating refresh tokens |

## 3.2 Technical Context

- Versioned HTTPS API (`api/v1`) is the technical boundary for all mobile-to-backend
  communication — see [API Contract](../reference/api-contract.md).
- Authenticated WebSocket gateway is the boundary for realtime location/check-in
  events; a connection is not itself a permission grant — every subscription is
  authorized independently (see
  [Security Architecture §"Realtime boundary"](./security-architecture-and-data-protection.md)).
- All error responses follow Problem Details (RFC 7807).

---
*Part of the [architecture documentation](./README.md) (arc42 §3, C4 Level 1: Context).
See [`diagrams/`](./diagrams) for a visual context diagram once drawn.*
