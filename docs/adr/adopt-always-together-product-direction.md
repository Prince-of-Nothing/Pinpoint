# 0002. Adopt Always Together as the product direction

Date: 2025-01-01

## Status

Accepted

## Context

The repository began as a generic "location-sharing app" scaffold with no
concrete product design. A complete, pre-existing product design — "Always
Together," a consent-based location-sharing safety and social app — was
subsequently brought into the repository (as a 34-file design vault) and
needed to be reconciled with the generic documentation scaffold rather than
lost, duplicated, or left as a separate, disconnected artifact.

Always Together's core idea is a **safety-first, consent-based** location
sharing product: location is never shared without explicit, mutual,
revocable consent, and privacy/safety are first-class architectural
concerns, not afterthoughts. This is a materially different and more
specific product than an unopinionated "location sharing app."

## Decision

We will adopt **Always Together** as the product this repository builds,
and migrate the full content of the design vault into the `docs/`
structure (arc42 architecture sections, Diátaxis reference/explanation
docs, and this ADR record), rather than maintaining the vault as a
parallel or competing source of truth.

Concretely:

- The **product rule** "location is only ever shared with mutual,
  explicit, revocable consent — never public, never silent" becomes the
  foundational constraint referenced throughout the architecture docs.
- The MVP scope, frozen technology stack, security model, data model, API
  contract, and requirements traceability matrix from the vault become the
  authoritative content of `docs/architecture/`, `docs/reference/`, and
  `docs/explanation/`.
- The design vault folder ("Always Together Master") is retired once all
  34 files' content is confirmed present in `docs/`, to avoid two
  divergent sources of truth.

## Consequences

- `docs/` becomes the single source of truth for product scope,
  architecture, and decisions — easier onboarding, easier auditing, no risk
  of the vault and the docs silently drifting apart.
- The frozen MVP technology stack (Flutter, FastAPI, PostgreSQL, Redis,
  S3-compatible storage, OIDC/PKCE, WebSocket gateway, OSM-derived maps,
  Docker/GitHub Actions) is now a recorded, reviewable decision rather than
  an implicit assumption — see
  [Technology Stack and Infrastructure](../reference/technology-stack-and-infrastructure.md)
  for the full rationale table. A dedicated ADR for this stack choice may
  follow if finer-grained decision history is later needed.
- Deferred/Full-mode features (movement trails, friend mosaics, caches,
  messaging, prompts, sponsorship) are documented now, even though they are
  explicitly out of scope for the MVP, so their safety and privacy
  implications are not lost or rediscovered later.
- Retiring the vault folder is a one-way step; it should only happen after
  a deliberate final review confirming nothing was lost in translation.

---
*See [§1 Introduction and Goals](../architecture/introduction-and-goals.md)
and [Roadmap and Open Decisions](../explanation/development-roadmap-and-evidence.md).*
