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
| 0002 | [Adopt Pinpoint Product Direction](./0002-adopt-pinpoint-product-direction.md) | Accepted |
| 0003 | [Password Authentication with Session-Bound JWTs and Rotating Refresh Tokens](./password-auth-with-rotating-refresh-tokens.md) | Accepted |
| 0004 | [Web Client for the MVP Demonstration; Flutter Deferred](./web-client-for-mvp-demonstration.md) | Accepted |
| 0005 | [Store Only the Latest Location; Derive States on Read](./store-latest-location-only.md) | Accepted |
| 0006 | [Rust Desktop Client Transport Policy and Session Contract](./0006-rust-client-transport-and-session-contract.md) | Accepted |
| 0007 | [Rust Multiplatform as the Sole Application Codebase](./0007-rust-multiplatform.md) | Accepted |
| 0008 | [Split Rust API into Independently Scalable Services Behind a Gateway](./0008-modular-rust-services.md) | Accepted |
| 0009 | [Dual Authentication Models — Anonymous Device-Bound and Identity-Linked Accounts](./0009-dual-auth-anonymous-and-identity.md) | Accepted (partial) |
| 0010 | [Normalized, Encrypted, Modular Database Schema](./0010-normalized-encrypted-database.md) | Accepted (design target) |
| 0011 | [Self-Hosted OpenStreetMap Services](./0011-self-hosted-osm-services.md) | Accepted (config + docs) |
| 0012 | [Opaque Server-Side Sessions Instead of JWT + Rotating Refresh Tokens](./0012-opaque-server-side-sessions.md) | Accepted |
| 0013 | [Use the Live Android Activity for JNI and Accept Cold-Start Share/Deep-Link Delivery](./0013-android-native-activity-jni.md) | Accepted |

See [`architecture/architecture-decisions-link.md`](../architecture/architecture-decisions-link.md) for the complete list.

## About This Layer

ADRs preserve *decision history* — complementing architecture documentation (current state) and explanation (broader context).

**Looking for:**
- Current architecture? → [docs/architecture/](../architecture)
- Broader context? → [docs/explanation/](../explanation)
- Technical specifications? → [docs/reference/](../reference)
