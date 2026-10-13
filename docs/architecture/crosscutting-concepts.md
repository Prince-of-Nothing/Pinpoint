# 8. Crosscutting Concepts

## 8.1 Privacy & Data Protection

Given how central this is for a location-sharing product, it has its own dedicated
document: [privacy-and-data-handling.md](./privacy-and-data-handling.md) (following
the pattern used by OwnTracks, Traccar, OsmAnd, and Mastodon, all of which publish
privacy/data-handling as a standalone doc rather than a footnote).

## 8.2 Identity, Profiles, and Visibility

Every account has an immutable internal user ID; usernames and display names are
never database relationship keys. Non-friends receive an empty profile by default.
See [Actors and Accounts](../reference/actors-and-accounts.md) and
[Profile and Visibility Model](../reference/profile-and-visibility-model.md).

## 8.3 Authentication & Authorization

OIDC/OAuth 2.0 with PKCE S256 for mobile/web public clients; short-lived access
tokens, rotating refresh tokens, device/session management, passkeys where
practical. Authorization is always relationship-based and enforced server-side,
independent of token validity. See
[Security Architecture §"Authentication and sessions"](./security-architecture-and-data-protection.md#authentication-and-sessions).

## 8.4 Real-Time Communication

Authenticated WebSocket gateway with per-connection authorization; a connection is
not a permission grant, and every subscription is re-checked against current
consent. See
[Security Architecture §"Realtime boundary"](./security-architecture-and-data-protection.md#realtime-boundary).

## 8.5 Battery and Stale-State Safety

Background tracking uses adaptive intervals and platform-approved services. Stale
or dead device state must never be displayed as live. See
[Security Control SC-07](../reference/security-control-register.md#sc-07-battery-and-stale-state-safety).

## 8.6 Threats and Safety (summary)

- **Unwanted tracking:** explicit consent, visible sharing status, short defaults,
  stop-sharing, block, revoke, and deletion.
- **Location breach:** minimum collection, encryption, limited history,
  relationship-based access, and retention controls.
- **Trajectory misuse:** separate consent for movement history and search sharing,
  pre-authorized recipients, activation logs, expiry, uncertainty display, and
  immediate revocation.
- **Public-cache abuse:** moderate public cache content and locations, limit
  harassment-prone coordinates, support reports and takedowns, and detect obvious
  location spoofing where practical.
- **Friend-map inference:** coarse regions, minimum-area thresholds, consent, and
  sensitive-place suppression so a mosaic cannot reveal a friend's home, routine,
  or visit to a protected location.
- **Account takeover:** passkeys or strong authentication, optional 2FA, session
  management, and rate limits.
- **Random-group abuse:** age-appropriate discovery, reporting, blocking,
  moderation, creator rules, and separation from trusted location circles.
- **Malicious media or caches:** permissions, expiry, file scanning, content
  review, and audit logs.
- **False safety assumptions:** freshness and accuracy indicators plus a clear
  statement that the app is not an emergency service.
- **Missing-person escalation:** a search corridor is an aid for trusted people and
  emergency services, not proof of a person's location or a replacement for
  contacting emergency authorities.
- **Untrusted uploads:** scan files, restrict executable content, isolate
  previews, and keep user-submitted media separate from private location data.

See [Security Threats and Controls](./security-threats-and-controls.md) for the
full detail across location/physical safety, identity/authorization, social/content
abuse, sponsorship/payments, and infrastructure/operations, and
[Security Control Register](../reference/security-control-register.md) (SC-01
through SC-09) for the implementation-level controls.

## 8.7 Error Handling & API Conventions

All error responses follow Problem Details (RFC 7807); denials must not reveal
whether hidden resources exist. See [API Contract](../reference/api-contract.md).

## 8.8 Accessibility & Internationalization

Screen readers, text alternatives, color contrast, captions, and reduced motion are
required non-functional goals — see [§10 Quality Requirements](./quality-requirements.md).

Each concept above that represents a deliberate choice among alternatives is
captured as an [ADR](../adr) where the choice is architecturally significant.

---
*Part of the [architecture documentation](./README.md) (arc42 §8).*
