# Reference

*Information-oriented*: precise, factual specifications consulted during implementation. No opinions or tutorials — only accurate technical facts.

## Core Documents

| Document | Contents |
|---|---|
| [Actors and Accounts](./actors-and-accounts.md) | Roles, identity/visibility fields, account model |
| [Profile and Visibility Model](./profile-and-visibility-model.md) | Default-empty non-friend profile rule, relationship-based views |
| [Domain Data Model](./domain-data-model.md) | Core entities (User, LocationShare, MovementTrail, Cache, Challenge, etc.) |
| [API Contract](./api-contract.md) | Versioned `api/v1` HTTPS + WebSocket endpoints summary |
| [API Contract Reference](./api-contract-reference.md) | Complete API specification with request/response examples |
| [API Security Implementation](./api-security-implementation.md) | OAuth 2.0/PKCE flow, authorization sequences, rate limiting, input validation, error handling, audit logging |
| [Technology Stack and Infrastructure](./technology-stack-and-infrastructure.md) | Frozen MVP stack (Flutter/FastAPI/PostgreSQL/Redis/etc.) and rationale |
| [Security Control Register](./security-control-register.md) | SC-01 through SC-09: problem/relevance/implementation/test/limitation |
| [MVP Requirements and Acceptance Criteria](./mvp-requirements-and-acceptance-criteria.md) | REQ-01 through REQ-09 with acceptance criteria and explicit non-goals |
| [Requirements Traceability Matrix](./requirements-traceability-matrix.md) | REQ → UI → API → DB → security control → test → evidence |
| [UI Navigation and Wireframes](./ui-navigation-and-wireframes.md) | Primary navigation and key screens |

## About This Layer

Reference material mirrors the system structure and stays strictly descriptive. See the [Diátaxis reference guide](https://diataxis.fr/reference/) for writing principles.

**Looking for:**
- Architecture rationale? → [docs/architecture/](../architecture)
- Deferred-feature discussion? → [docs/explanation/](../explanation)
- How-to guides? → [docs/how-to/](../how-to)
- Tutorials? → [docs/tutorials/](../tutorials)
