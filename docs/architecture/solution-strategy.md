# 4. Solution Strategy

## 4.1 Architectural Style

A **modular monolith**, not microservices, for the MVP: one versioned API, one
relational database, one worker process, one object-storage boundary, one realtime
gateway. This is easier to secure, test, deploy, and explain during the internship
timeline. Interfaces around identity, relationships, location, content, and
notifications are kept separable so they *can* be split out later if scale
requires it. See [Technology Stack and Infrastructure](../reference/technology-stack-and-infrastructure.md).

## 4.2 Frozen MVP Technology Stack

Recorded as a decision, not a hidden assumption — see
[ADR-0002](../adr/0002-adopt-always-together-product-direction.md):

- **Mobile client:** Flutter + native location modules (Android/iOS platform
  channels for background permissions, battery-aware sampling, geofencing,
  platform notifications).
- **Backend:** FastAPI + SQLAlchemy + Alembic, versioned HTTPS API.
- **Database:** PostgreSQL (system of record for users, relationships, permissions,
  check-ins, audit metadata).
- **Queue/cache:** Redis + Celery, strict TTLs; Redis is never authoritative for
  location permissions or consent.
- **Object storage:** S3-compatible encrypted storage, signed URLs, upload scanning.
- **Realtime transport:** authenticated WebSocket gateway, per-connection
  authorization.
- **Identity/authorization:** OIDC/OAuth 2.0 with PKCE S256, short-lived access
  tokens, rotating refresh tokens, passkeys where practical.
- **Map provider:** OSM-derived tile/routing provider behind a map adapter with
  attribution; external navigation deep links in MVP.
- **Operations:** Docker + GitHub Actions; managed container hosting with managed
  PostgreSQL and object storage (hybrid: managed for pilot, infra-as-code designed
  for future self-hosting).

## 4.3 Strategy for Top Quality Goals

- **Safety / consent correctness:** every location read passes through the same
  control sequence — see
  [Security Architecture §"Control sequence for a location read"](./security-architecture-and-data-protection.md).
  Directional consent (friendship ≠ location access) is enforced at the API layer,
  never only in the client.
- **Privacy:** minimum collection, coarse-by-default history, private-by-default
  profiles, purpose/expiry/revocation on every grant. See
  [Privacy and Data Handling](./privacy-and-data-handling.md).
- **Battery-aware availability:** adaptive sampling intervals, explicit stale/dead
  state instead of false liveness. See
  [Security Control SC-07](../reference/security-control-register.md#sc-07-battery-and-stale-state-safety).
- **Extensibility without eroding the safety core:** feature boundaries (location,
  social, creative, experimental) use separate permissions, retention, and abuse
  controls, so later features (caches, trails, groups, prompts) cannot silently gain
  location access. See [Architecture Models](./architecture-models.md).

## 4.4 Organizational Strategy

Work is sequenced per the [Roadmap and Open Decisions](../explanation/development-roadmap-and-evidence.md)
and [Development Roadmap and Evidence](../explanation/development-roadmap-and-evidence.md):
trusted location sharing and safety controls ship first; social, creative, and
experimental features layer on afterward without changing the safety foundation.

---
*Part of the [architecture documentation](./README.md) (arc42 §4).*
