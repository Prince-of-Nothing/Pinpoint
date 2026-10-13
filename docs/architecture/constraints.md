# 2. Architecture Constraints

## 2.1 Technical Constraints

| Constraint | Background / Rationale |
|------------|-------------------------|
| Android and iPhone are the primary target platforms | Background location permission handling differs significantly per platform; Android is the most realistic first production target, with iPhone and web following validated battery/permission behavior (see [Technology Stack and Infrastructure](../reference/technology-stack-and-infrastructure.md)) |
| Frozen MVP technology stack | See [ADR-0002](../adr/0002-adopt-always-together-product-direction.md) and [Technology Stack and Infrastructure](../reference/technology-stack-and-infrastructure.md): Flutter mobile client, FastAPI + SQLAlchemy + Alembic backend, PostgreSQL, Redis + Celery, S3-compatible object storage, OIDC/OAuth 2.0 with PKCE, authenticated WebSocket gateway, OSM-derived map provider |
| Public OSM tile/Nominatim services are not a production SLA | Suitable for careful prototypes only; production needs a compliant hosted tile/routing provider behind a replaceable adapter (see [Research Notes and References](../explanation/research-notes-and-references.md)) |
| Modular monolith, not microservices, for the MVP | One versioned API, one relational database, one worker process, one object-storage boundary, one realtime gateway — easier to secure, test, deploy, and explain; interfaces are kept separable for future scaling (see [Technology Stack and Infrastructure](../reference/technology-stack-and-infrastructure.md)) |

## 2.2 Organizational Constraints

| Constraint | Background / Rationale |
|------------|-------------------------|
| Internship timeline (DAS internship, September MVP + Oct–Dec continuation) | See [DAS Internship Alignment](../explanation/das-internship-alignment.md) and [Development Roadmap and Evidence](../explanation/development-roadmap-and-evidence.md) for the week-by-week plan and evidence checklist |
| September MVP scope is deliberately narrow | Location trails, friend mosaics, caches, groups, prompts, and moderation are explicitly deferred so the safety foundation ships first (see [§1.5](./introduction-and-goals.md#15-mvp-requirements-overview)) |

## 2.3 Conventions

| Convention | Background / Rationale |
|------------|-------------------------|
| Documentation follows Diátaxis + arc42 + ADRs | See [docs/README.md](../README.md) |
| Every architecturally significant control uses a 5-question format (problem, relevance, implementation, test, limitation) | See [Security Control Register](../reference/security-control-register.md) |
| REQ → UI → API → DB → security control → test → evidence traceability | See [Requirements Traceability Matrix](../reference/requirements-traceability-matrix.md) |

## 2.4 Regulatory / Privacy Constraints

Location-sharing apps carry strong privacy/legal obligations. See
[Privacy and Data Handling](./privacy-and-data-handling.md) for the full model
(collection minimization, retention, consent, GDPR-style rights) and
[Research Notes and References](../explanation/research-notes-and-references.md)
for the specific external sources consulted (OSM tile/Nominatim policies, Android/
iOS background location policies, OAuth/PKCE RFCs, OWASP ASVS/API Top 10).

---
*Part of the [architecture documentation](./README.md) (arc42 §2).*
