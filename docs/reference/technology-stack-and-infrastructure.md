# Technology Stack and Infrastructure

## Product clients

- Cross-platform mobile client for Android and iPhone where practical.
- Native location modules for background permissions, battery-aware sampling,
  geofencing, and platform notifications.
- Optional web companion for account management, prompt galleries, group
  administration, and map viewing; web is not assumed to replace mobile
  background location.

## Backend

- Versioned HTTPS API for identity, friendships, directional consent,
  profiles, check-ins, current location, maps, caches, groups, prompts,
  challenges, reports, and notifications.
- Realtime channel for authorized location and check-in updates, with
  authorization applied before every event.
- Background jobs for reminders, stale-state transitions, expiry, moderation
  queues, prompt scheduling, and notifications.
- Provider-neutral service boundaries so hosted infrastructure can later move
  to self-hosted or hybrid deployment.

## Data and storage

- Relational database for users, relationships, permissions, check-ins,
  prompts, groups, and audit metadata.
- Encrypted object storage for caches and media with size, type, malware,
  retention, and deletion controls.
- Latest-location storage separated from historical or traversal data.
- Cache, prompt, and user-owned achievement records have explicit retention
  and deletion rules.
- Backups are encrypted, access-controlled, tested, and exportable.

## Maps and notifications

- OpenStreetMap data with attribution and a compliant tile provider.
- Licensed routing/geocoding provider or external navigation deep links.
- Push notifications for check-ins, sharing changes, reminders, and group
  activity without leaking sensitive content in notification previews.

## Security and operations

- Passkeys or strong authentication, device/session management, and fresh
  authentication for sensitive actions.
- TLS in transit, encryption at rest, separated key management,
  least-privilege service accounts, rate limits, and audit events.
- Observability for availability, battery impact, stale updates, notification
  delivery, authorization failures, abuse reports, and data access.
- CI checks for tests, dependency vulnerabilities, secret exposure, schema
  changes, and mobile permission behavior.
- Disaster recovery, incident response, data export, account deletion, and
  provider-outage playbooks.

## Initial deployment direction

Use hosted or hybrid services for affordable testing, while keeping
encryption, backups, data portability, and infrastructure-as-code sufficient
for future self-hosting. Android is the most realistic first production
target; iPhone and web support should follow validated battery and
permission behavior.

## Frozen technology stack (MVP)

This is the frozen, recorded stack decision for the MVP — see also
[§4.2 Solution Strategy](../architecture/solution-strategy.md#42-frozen-mvp-technology-stack)
and [§2 Constraints](../architecture/constraints.md).

| Layer | Choice | Rationale |
|---|---|---|
| Mobile client | Flutter + native platform channels (Android/iOS) for background permissions, battery-aware sampling, geofencing, notifications | Shared UI code and consistent map/UI behavior across Android/iPhone |
| Backend | FastAPI + SQLAlchemy + Alembic, versioned HTTPS API | Typed APIs, rapid development, async endpoints, background-job integration with Celery, readable security surface |
| Database | PostgreSQL with SQLAlchemy ORM | Transactions, relationships, system of record for users, relationships, permissions, check-ins, audit metadata |
| Queue/cache | Redis with strict TTLs + Celery | Durable job queue (reminders, stale-state transitions, expiry, moderation tasks, prompt scheduling, notifications); Redis is never authoritative for permissions/consent |
| Object storage | S3-compatible encrypted storage, signed URLs, scanning | Metadata in PostgreSQL; short-lived signed URLs; uploads scanned before release; content limits and retention/deletion per policy |
| Realtime transport | Authenticated WebSocket gateway, per-connection authorization | Recipients subscribe only to resources already allowed by current consent |
| Identity & auth | OIDC/OAuth 2.0 with PKCE S256 | Short-lived access tokens, rotating refresh tokens, device/session management, passkeys where practical; backend authorization independent of token validity |
| Maps | OSM-derived tile/routing provider behind a map adapter, with attribution | External navigation deep links in MVP; no private location history sent to map services |
| Operations | Docker + GitHub Actions CI/CD; managed container hosting with managed PostgreSQL and object storage | Hybrid: managed services for pilot, infra-as-code designed for future self-hosting; Celery worker runs alongside the API |
| Security controls | SC-01 through SC-09 (see [Security Control Register](./security-control-register.md)) implemented at the API boundary | TLS in transit, encryption at rest, separated key management, least-privilege service accounts, rate limits, audit events |

The choice between Flutter/React Native, FastAPI/Django, and specific hosting
providers is a recorded decision rather than a hidden assumption. All
interfaces (identity, relationships, location, content, notifications) are
designed so they can be separated later if scale requires it.

## Baseline architecture rationale

A modular monolith is used rather than microservices for the MVP: one
versioned API, one relational database, one worker process, one
object-storage boundary, and one realtime gateway. This is easier to secure,
test, deploy, and explain during the internship timeline. Interfaces around
identity, relationships, location, content, and notifications are kept
separable so the system can be split later if scale requires it.

---
*See also [ADR index](../adr) — the frozen stack and modular-monolith choice
are architecturally significant decisions and should be backed by ADRs.*
