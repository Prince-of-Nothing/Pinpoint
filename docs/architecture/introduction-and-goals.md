# 1. Introduction and Goals

## 1.1 Purpose

Always Together helps people stay connected in the real world through location sharing, navigation, messaging, group discovery, creative prompts, and optional community experiments. It is a privacy-first safety map: the September MVP helps a user share their current location with selected trusted friends, complete check-ins, see stale-device state, and navigate using an OpenStreetMap-based map or an external navigation app.

## 1.2 Product Rule

The app is allowed to become broad and personal. It should still keep trusted location sharing, consent, and safety as the anchor for every expansion.

## 1.3 Modes

- **Lite:** location, maps, navigation, caches, and safety controls.
- **Full:** Lite plus messaging, groups, media, calls, and community features.

The modes are packaging choices, not a limit on the long-term product vision.

## 1.4 Source Ideas Combined

This project incorporates earlier location-sharing, group-chat matching, and daily-prompt concepts. The imported features are intentionally marked as optional or later so the foundation can ship first without losing the ideas. See [Scope and MVP Boundaries](./scope-and-mvp-boundaries.md) for the full breakdown, and [Development Roadmap](../explanation/development-roadmap-and-evidence.md) for sequencing.

## 1.5 MVP Requirements Overview

The September MVP is realistic because location, consent, mobile battery use, authentication, authorization, and sensitive data create meaningful security problems. It is extensible because later releases can add caches, traversal maps, friend mosaics, groups, prompts, challenges, messaging, and moderated discovery without changing the safety foundation.

See [MVP Requirements and Acceptance Criteria](../reference/mvp-requirements-and-acceptance-criteria.md) for REQ-01 through REQ-09 and their acceptance criteria, and [Requirements Traceability Matrix](../reference/requirements-traceability-matrix.md) for how each requirement maps through UI → API → DB → security control → test → evidence.

### In scope (September MVP)

- Pseudonymous accounts and profiles.
- Mutual friendship and directional sharing permissions.
- Current location updates with exact/approximate recipient settings.
- Battery-aware update frequency and precision.
- Manual, scheduled, trip, and missed-check-in workflows.
- Last-known location, accuracy, battery freshness, and stale/dead state.
- OpenStreetMap map display and external navigation export.
- Backend authorization, audit events, rate limits, secure errors, and prioritized security tests.

### Deferred

Movement trails and search corridors, visited-region maps, friend mosaics, caches, public prompts, sponsored prompts, messaging, calls, random groups, and advanced moderation are documented as later phases (see [Scope and MVP Boundaries](./scope-and-mvp-boundaries.md) and [Development Roadmap](../explanation/development-roadmap-and-evidence.md)). Their threat implications remain in the design, but they are not allowed to inflate the September MVP.

### Explicit non-goals (MVP)

The MVP does not include public/random discovery, sponsored prompts, messaging, calls, trails, search corridors, traversal maps, caches, friend mosaics, or advanced media moderation. Those remain documented extension phases.

## 1.6 Quality Goals

See [Quality Requirements](./quality-requirements.md) for the full quality tree; the top-priority goals driving early architecture decisions are safety (understandable, reliable consent/revocation), privacy (minimum collection, strict access control), and battery-aware availability.

## 1.7 Stakeholders

| Role | Expectations |
|------|---------------|
| End users | Understandable, reliable, consent-based location sharing; safety without false confidence |
| Trusted friends | Only see what was explicitly, currently consented to |
| Moderators / platform operators | Manage safety and abuse without routine access to precise private location |
| Internship mentor / reviewer | See [DAS internship alignment](../explanation/das-internship-alignment.md) for internship-specific evidence and reporting expectations |

---
*Part of the [architecture documentation](./README.md) (arc42 §1).*
