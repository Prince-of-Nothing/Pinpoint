# 11. Risks and Technical Debt

Track known risks and accumulated technical debt so they are visible and not silently
lost as the team/contributors change over time.

| ID | Risk / Debt | Impact | Likelihood | Mitigation / Plan | Status |
|----|-------------|--------|------------|--------------------|--------|
| R1 | Production hosting/infrastructure details beyond the frozen MVP stack are not yet decided (only "managed/hybrid, provider-neutral" direction is set) | Blocks production deployment | Medium (post-MVP) | Decide via ADR before first production release; see [Technology Stack and Infrastructure §"Initial deployment direction"](../reference/technology-stack-and-infrastructure.md#initial-deployment-direction) | Open |
| R2 | Documentation drift: `TBD` placeholders in `architecture/` left unfilled, or docs not updated alongside code | Misleads contributors, hides real architecture | Medium (grows over time without discipline) | Periodic doc audit per [`docs/CONTRIBUTING.md`](../CONTRIBUTING.md#keeping-docs-from-rotting-audit-cadence); update docs in the same PR as the code change | Open |
| R3 | Precise location-history retention period is undecided | Affects privacy posture and storage cost | Medium | Decide via ADR; see [Roadmap and Open Decisions](../explanation/development-roadmap-and-evidence.md) | Open |
| R4 | Age/verification rules for random-group discovery are undecided | Child-safety and moderation risk if shipped without rules | High (if random groups ship) | Legal/policy review before random groups leave the deferred backlog; see [Security Threats and Controls §"Social and content abuse"](./security-threats-and-controls.md#social-and-content-abuse) | Open |
| R5 | Search-corridor / missing-person feature check-in, recipient, and retention rules are undecided | Safety-critical feature could be shipped with unclear boundaries | High (if shipped before decided) | Must be resolved via ADR before [Movement Trails and Search Corridors](./movement-trails-and-search-corridors.md) exits deferred status | Open |
| R6 | Public-cache exact-location exposure rules (exact point vs. area vs. hint) are undecided | Harassment/stalking risk if caches ship without a decision | High (if shipped before decided) | Must be resolved via ADR before [Cache and Permission Model](./cache-and-permission-model.md) public caches ship | Open |
| R7 | Friend-mosaic minimum-region-size and sensitive-place-suppression thresholds are undecided | Location-inference privacy risk | Medium | Must be resolved via ADR before [Friend Mosaic Maps](./friend-mosaic-maps.md) ships | Open |
| R8 | Rate limiting falls back to a per-process in-memory limiter when Redis is unavailable; with multiple API workers each worker enforces its own budget | Weaker brute-force protection in scaled deployments | Medium | Set `RATE_LIMIT_BACKEND=redis` for any multi-worker deployment (fallback is logged at boot); covered in [Deployment View](./diagrams/deployment-diagram.md) | Open |
| R9 | Database schema is created with `create_all` at startup; no Alembic migrations wired | Schema drift and unsafe production DDL | Medium | Wire Alembic before first production deployment | Open |
| R10 | `passlib` pins `bcrypt` to 4.0.x (incompatible with ≥ 4.1) | Dependency friction; pin must be revisited | Low | Track passlib release; see `backend/requirements.txt` | Open |
| R11 | Common-password deny-list embedded in code is small (top ~24) | Weak passwords outside the list pass policy | Medium | Load a breached-password corpus (e.g. top-100k) before release; policy chain makes this a drop-in rule change | Open |
| R12 | Account signup, recovery, and MFA are not implemented (users are provisioned directly in the DB for the MVP demo) | Blocks real user onboarding; recovery is a known takeover vector | High (post-MVP) | Implement via the OIDC/PKCE path per [SC-01](../reference/security-control-register.md#sc-01-authentication) before any external users | Open |

Update this table as risks are identified, mitigated, or resolved. For risks that
lead to a concrete architectural response, record the response as an [ADR](../adr).

---
*Part of the [architecture documentation](./README.md) (arc42 §11).*
