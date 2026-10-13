# DAS Internship Alignment

## Purpose

This note maps the Always Together project to the Development of Secure
Applications (DAS) internship expectations. It is a planning and evidence
guide, **not** a replacement for the official Internship Report, PBL Report,
Log Book, weekly activity reports, attendance sheet, or ELSE templates.

## Project definition

Always Together is a privacy-first safety map. The September MVP helps a
user share their current location with selected trusted friends, complete
check-ins, see stale-device state, and navigate using an OpenStreetMap-based
map or an external navigation app.

The project is realistic because location, consent, mobile battery use,
authentication, authorization, and sensitive data create meaningful security
problems. It is extensible because later releases can add caches, traversal
maps, friend mosaics, groups, prompts, challenges, messaging, and moderated
discovery without changing the safety foundation.

## September MVP scope

### In scope

- Pseudonymous accounts and profiles.
- Mutual friendship and directional sharing permissions.
- Current location updates with exact/approximate recipient settings.
- Battery-aware update frequency and precision.
- Manual, scheduled, trip, and missed-check-in workflows.
- Last-known location, accuracy, battery freshness, and stale/dead state.
- OpenStreetMap map display and external navigation export.
- Backend authorization, audit events, rate limits, secure errors, and
  prioritized security tests.

### Deferred

Movement trails and search corridors, visited-region maps, friend mosaics,
caches, public prompts, sponsored prompts, messaging, calls, random groups,
and advanced moderation are documented as later phases. Their threat
implications remain in the design, but they are not allowed to inflate the
September MVP.

## Required security integration

The team will document and prioritize security across the mobile client,
backend API, database, infrastructure, and operational workflow:

- authentication and MFA/passkeys;
- password or credential protection where passwords exist;
- authorization and directional consent;
- input validation and secure error handling;
- API rate limiting;
- protection against IDOR, injection, XSS, CSRF, privilege escalation, and
  session abuse;
- encryption in transit and at rest;
- secure sessions and timeout/invalidation;
- RBAC for operators and moderators;
- encrypted backup and recovery;
- security logging and monitoring;
- incident response and evidence preservation.

The exact controls selected for the first implementation must be recorded
with their reason, implementation, test, and limitation (see
[Security Control Register](../reference/security-control-register.md)).

## Internship evidence to maintain

- Approved project definition and scope boundary.
- Stakeholder and actor list.
- Architecture and technology decision record.
- Data classification and privacy model.
- Systems research and technology decision record.
- Threat model and prioritized risk register.
- Security architecture and data protection model.
- Security control register with tests and limitations.
- MVP requirements and acceptance criteria.
- API/data model documentation.
- Incident response plan.
- Weekly implementation notes and mentor feedback.
- Test results, screenshots, demonstration script, and unresolved-risk list.
- Development plan for October–December.

## Expected September outcome

By the end of the internship, the team should be able to demonstrate a
small working safety flow and explain the security decisions even where
production-grade implementation remains incomplete. The documentation should
make clear what is implemented, simulated, planned, or intentionally
deferred.

---
*See [Development Roadmap and Evidence](./development-roadmap-and-evidence.md)
for the phased plan, and the full docs/ tree for the evidence artifacts
listed above.*
