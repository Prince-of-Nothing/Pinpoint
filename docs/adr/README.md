# Architecture Decision Records (ADR)

This directory contains **Architecture Decision Records** — a numbered, append-only log of significant architectural decisions and their rationale.

ADRs preserve the *why* behind decisions even after code changes or team transitions.

## When to Write an ADR

Create an ADR when a decision is:
- **Hard or costly to reverse** — e.g., database technology selection
- **Cross-cutting** — affects multiple components or system architecture
- **Trade-off heavy** — meaningful pros/cons were weighed
- **Otherwise ephemeral** — would otherwise exist only in chat or memory

Examples: backend framework, real-time transport (WebSockets vs. MQTT), authentication approach, location data retention policy, mobile framework, cloud provider.

## Process

1. Copy [`template.md`](./template.md) to `NNNN-short-title.md` (next sequential number, kebab-case title)
2. Fill it in — status starts as `Proposed`
3. Discuss/review; once agreed, set status to `Accepted` (or `Rejected`)
4. Add a row to the index in [`architecture/architecture-decisions-link.md`](../architecture/architecture-decisions-link.md)
5. **Never edit accepted ADRs.** If circumstances change, write a *new* ADR that supersedes it, marking the old one `Superseded by ADR-NNNN`

## Current Decisions

| # | Title | Status |
|---|-------|--------|
| 0001 | [Record Architecture Decisions](./record-architecture-decisions.md) | Accepted |
| 0002 | [Adopt Always Together Product Direction](./adopt-always-together-product-direction.md) | Accepted |

See [`architecture/architecture-decisions-link.md`](../architecture/architecture-decisions-link.md) for the complete list.

## About This Layer

ADRs preserve *decision history* — complementing architecture documentation (current state) and explanation (broader context).

**Looking for:**
- Current architecture? → [docs/architecture/](../architecture)
- Broader context? → [docs/explanation/](../explanation)
- Technical specifications? → [docs/reference/](../reference)
