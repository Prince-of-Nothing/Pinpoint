# UML Diagrams — GoLocate

This folder holds all modelled views of the system. Diagrams are written as **Mermaid** inside Markdown so they render natively on GitHub (no external tooling required) and live-review in PRs like code.

## Index

| File | Viewpoint | Contents |
|------|-----------|----------|
| [01-system-context.md](01-system-context.md) | C4 Level 1 — Context | Users, external systems (OSM, FCM, SMTP), the four clients |
| [02-container-diagram.md](02-container-diagram.md) | C4 Level 2 — Containers | Compose services, ports, protocols, deployment modes |
| [03-component-diagram.md](03-component-diagram.md) | C4 Level 3 — Components | Backend internal packages & responsibilities |
| [04-class-diagram.md](04-class-diagram.md) | Static structure | Domain entities + store/handler relationships |
| [05-sequence-auth.md](05-sequence-auth.md) | Behaviour | Login → session cookie + CSRF flow |
| [06-sequence-location-share.md](06-sequence-location-share.md) | Behaviour | Publish location → consent check → fan-out to viewers |
| [07-sequence-friendship.md](07-sequence-friendship.md) | Behaviour | Friend request / accept / reject lifecycle |
| [08-state-diagrams.md](08-state-diagrams.md) | Lifecycle states | Share state machine, account state machine |
| [09-er-diagram.md](09-er-diagram.md) | Data model | Postgres schema (target, per T01/T02) with cardinalities |
| [10-deployment-diagram.md](10-deployment-diagram.md) | Deployment | Dev compose vs production topology |
| [11-use-case-diagram.md](11-use-case-diagram.md) | Requirements | Actors ↔ use cases mapped to tasks Txx |

## Conventions
- **Naming:** files numbered by viewpoint order; keep one diagram family per file.
- **Status markers:** 📋 draft · 🚧 under review · ✅ baseline (agreed). Baseline changes need a PR approved by at least two members.
- **Traceability:** every element that maps to a task or requirement carries its ID (`Txx`, `FR-x`) inside the diagram label or notes.
- **Tooling:** Mermaid ≥ v10 (GitHub's bundled renderer). For high-res exports use `mmdc` from `@mermaid-js/mermaid-cli`; add a CI job later if needed (tracked in PLANNED_TASKS backlog idea).

## Maintenance rule
When a task in [TASKS.md](../TASKS.md) is stamped ✅ Done and it changes architecture/data model, the same PR must update the affected diagram(s) here — otherwise the PR fails review (see CONTRIBUTING.md §Documentation sync).
