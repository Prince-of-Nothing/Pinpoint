# Architecture Documentation

This directory contains the **arc42-structured** architecture documentation for **Always Together**. Each file corresponds to a standard arc42 section. Diagrams follow the C4 model and are stored in [`diagrams/`](./diagrams).

## arc42 Sections

| Section | File |
|---------|------|
| 1. Introduction and Goals | [introduction-and-goals.md](./introduction-and-goals.md) |
| — Scope and MVP Boundaries | [scope-and-mvp-boundaries.md](./scope-and-mvp-boundaries.md) |
| 2. Constraints | [constraints.md](./constraints.md) |
| 3. Context and Scope | [context-and-scope.md](./context-and-scope.md) |
| 4. Solution Strategy | [solution-strategy.md](./solution-strategy.md) |
| — Architecture Models | [architecture-models.md](./architecture-models.md) |
| 5. Building Block View | [building-block-view.md](./building-block-view.md) |
| 6. Runtime View | [runtime-view.md](./runtime-view.md) |
| 7. Deployment View | [deployment-view.md](./deployment-view.md) |
| 8. Crosscutting Concepts | [crosscutting-concepts.md](./crosscutting-concepts.md) |
| 9. Architecture Decisions | [architecture-decisions-link.md](./architecture-decisions-link.md) (index into [ADRs](../adr)) |
| 10. Quality Requirements | [quality-requirements.md](./quality-requirements.md) |
| 11. Risks and Technical Debt | [risks-and-technical-debt.md](./risks-and-technical-debt.md) |
| 12. Glossary | [glossary.md](./glossary.md) |

## Deep-dive Documents

These documents provide detailed coverage of specific topics and are referenced from the main arc42 sections:

| Document | Topic |
|---|---|
| [Privacy & Data Handling](./privacy-and-data-handling.md) | Standalone privacy reference — referenced from §8 |
| [Security Architecture and Data Protection](./security-architecture-and-data-protection.md) | Data classification, security boundaries, encryption, authorization control sequence |
| [Security Threats and Controls](./security-threats-and-controls.md) | Threat-first view across location, identity, social/content, sponsorship, infrastructure |
| [Incident Response Plan](./incident-response-plan.md) | Severity levels, response stages, evidence handling |
| [Location Sharing Lifecycle](./location-sharing-lifecycle.md) | Core MVP consent/sharing/expiry flow |
| [Movement Trails and Search Corridors](./movement-trails-and-search-corridors.md) | Deferred: opt-in trails, missing-person search workflow |
| [Friend Mosaic Maps](./friend-mosaic-maps.md) | Deferred: collaborative visited-region maps |
| [Cache and Permission Model](./cache-and-permission-model.md) | Location-bound content and location-locked/public caches |
| [Messaging and Group Lifecycle](./messaging-and-group-lifecycle.md) | Deferred: direct messaging, groups, in-chat challenges |

## Related Documentation

- Full documentation structure: [docs/README.md](../README.md)
- Product scope, requirements, data model, API contract: [docs/reference/](../reference)
- Deferred-feature rationale: [docs/explanation/](../explanation)
- Architectural decisions: [docs/adr/](../adr)
