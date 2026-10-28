# THESIS DOCUMENT CONTENT

> **Implementation status (September MVP).** Statements below describe the
> implemented MVP unless marked as planned. What is built, and what is
> deferred, is tracked in
> [docs/how-to/implementation-plan.md](docs/how-to/implementation-plan.md);
> the scope changes are recorded in ADRs 0003-0005 under `docs/adr/`.

## ABSTRACT

Always Together is a privacy-first mobile application designed to help people stay connected in the real world through consent-based location sharing, navigation, and safety features. This thesis presents the security architecture, implementation strategies, and validation evidence for a location-sharing system that prioritizes user safety, privacy, and battery-aware availability. The core contribution is a directional authorization model where friendship does not automatically grant location access—each sharing relationship requires explicit, revocable consent with configurable precision, purpose, and expiry. The system implements comprehensive security controls including authentication with token rotation, server-side authorization on every resource access, input validation, CSRF protection, encrypted storage and transport, rate limiting, and audit logging. The architecture follows a modular monolith design: the implemented MVP uses FastAPI for the backend, PostgreSQL for persistent storage, Redis for rate limiting and a same-origin web client with an OpenStreetMap map, while a Flutter mobile client, a background job queue and a WebSocket realtime channel are planned extensions. Key innovations include stale-state detection that prevents false safety confidence, battery-aware update frequencies that adapt to device conditions, and a multi-layer security approach that treats the mobile client as untrusted while enforcing all authorization decisions server-side. Testing evidence demonstrates protection against common vulnerabilities including IDOR attacks, session hijacking and token replay, CSRF, injection attacks, mass assignment and cross-account data leakage (140 automated tests, 97% line coverage). The project maintains extensive documentation covering architecture decisions, threat models, security controls, incident response procedures, and requirements traceability from user stories through implementation to test evidence. This work contributes practical patterns for building safety-critical mobile applications where location privacy and explicit consent are fundamental requirements rather than afterthoughts.

Keywords: location sharing, privacy, authorization, mobile security, consent management, stale-state detection, directional permissions

---

## INTRODUCTION

### Purpose and Problem Statement

In an increasingly connected world, maintaining awareness of trusted friends and family members locations has become an important safety mechanism. However, existing location-sharing solutions often fail to provide adequate privacy protections, leading to potential stalking, unauthorized surveillance, and data breaches. The fundamental problem addressed by this thesis is how to build a location-sharing application that simultaneously provides reliable safety features while maintaining strict privacy controls and preventing abuse.

The Always Together project tackles this challenge by implementing a consent-based location sharing system where every aspect of data access is explicitly controlled by the data owner. Unlike systems that treat friendship as implicit permission to access location data, Always Together enforces directional authorization: being friends with someone does not automatically grant access to their location. Instead, users must explicitly grant permission for each specific recipient, with fine-grained controls over precision (exact vs. approximate), purpose (why the access is needed), and expiry (when the permission ends).

This approach addresses several critical security concerns. First, it prevents accidental over-sharing where users might unintentionally expose their location to broader audiences than intended. Second, it enables granular control where different recipients can receive different levels of detail—a parent might see exact location while a casual friend sees only approximate area. Third, it supports time-limited sharing where permissions automatically expire, reducing the risk of forgotten active shares becoming privacy liabilities. Finally, it ensures that revocation is immediate and complete, allowing users to instantly cut off access when relationships change or safety concerns arise.

The stakes are particularly high for location-sharing applications because compromised location data can facilitate physical stalking, domestic abuse, burglary (knowing when homes are empty), and other real-world harms. Traditional security approaches focusing solely on authentication and encryption prove insufficient—systems must also enforce sophisticated authorization policies that reflect the nuanced social relationships and contextual factors governing appropriate location access.

### Project Scope and Objectives

This thesis documents the design, implementation, and validation of the Always Together Minimum Viable Product (MVP), which focuses on nine core requirements spanning account privacy, mutual friendship management, directional location sharing, battery-aware updates, check-in workflows, stale-state display, map integration, security baseline implementation, and comprehensive documentation.

The MVP scope includes: pseudonymous accounts and profiles; mutual friendship and directional sharing permissions; current location updates with exact/approximate recipient settings; battery-aware update frequency and precision; manual, scheduled, trip, and missed-check-in workflows; last-known location, accuracy, battery freshness, and stale/dead state; OpenStreetMap map display and external navigation export; backend authorization, audit events, rate limits, secure errors, and prioritized security tests.

Explicitly deferred beyond the MVP scope are movement trails and search corridors, visited-region maps, friend mosaics, caches, public prompts, sponsored prompts, messaging, calls, random groups, and advanced moderation. These features remain documented as extension phases with their threat implications analyzed, but they do not inflate the September MVP deliverables.

The primary objectives of this project are:

1. Implement robust authentication and session management: Argon2id password hashing, short-lived access tokens (15-minute lifetime) bound to server-side sessions, rotating refresh tokens with reuse detection, and device/session management. OAuth 2.0/OIDC with PKCE and passkeys are the planned next step (ADR 0003).

2. Design and enforce server-side authorization where every API request undergoes authorization checks evaluating current consent state, friendship relationships, block lists, and resource ownership.

3. Protect sensitive location data through encryption and access controls—data encrypted in transit via TLS and at rest in databases and object storage.

4. Prevent common web and mobile vulnerabilities through systematic input validation, parameterized queries, output encoding, and secure session handling.

5. Ensure honest freshness signaling so users never mistake stale location data for current information.

6. Maintain battery efficiency through adaptive sampling intervals that respond to device battery levels, connectivity status, and operating system restrictions.

7. Document security decisions and testing evidence comprehensively with clear traceability from requirements through implementation to validation results.

### Technical Approach and Architecture

The Always Together system employs a modular monolith architecture chosen for its balance of security simplicity, testability, and deployment manageability within the internship timeline. The implemented MVP stack consists of:

- Client: same-origin web client (HTML/JavaScript) with Leaflet and OpenStreetMap tiles
- Backend API: FastAPI with SQLAlchemy ORM and Alembic migrations
- Database: PostgreSQL with a least-privilege runtime role and an append-only audit table
- Cache: Redis for rate limiting
- Authentication: Argon2id passwords, session-bound JWT access tokens, rotating refresh tokens, CSRF-protected cookies for the web client
- Map Services: OpenStreetMap tiles and coordinate-only external navigation links
- Operations: Docker Compose (optional Caddy HTTPS) with GitHub Actions CI (lint, bandit, pip-audit, migrations, tests)

Planned extensions (October to December): Flutter mobile client with native background location, Celery job queue for reminders, authenticated WebSocket/SSE realtime updates, OAuth 2.0/OIDC with PKCE and passkeys, S3-compatible encrypted object storage, and PostgreSQL row-level security policies.

### Security Architecture Overview

Nine security controls (SC-01 through SC-09) form the foundation of the system protection strategy:

SC-01 Authentication: OAuth 2.0/OIDC with PKCE, passkeys, token rotation, device/session management, rate limiting

SC-02 Directional Authorization: Backend policy checks on every location read/write with share records specifying owner, recipient, precision, purpose, expiry, and revocation

SC-03 Private-by-Default Profiles: Server-side response filtering returning empty profiles for non-friends

SC-04 Input and API Security: Schema validation, parameterized queries, bounded payloads, safe error responses, rate limits, audit events

SC-05 Session and Transport Security: TLS, secure token storage, token expiry/invalidation, HttpOnly/Secure/SameSite cookies, CSRF protection, CSP headers

SC-06 Data Protection: TLS encryption, database/object storage encryption, secrets manager, minimum retention, encrypted backups

SC-07 Battery and Stale-State Safety: Sampling presets, advanced interval control, adaptive precision, low-battery behavior, last-update timestamps, stale/dead state transitions

SC-08 Abuse Reporting and Monitoring: Report categories, evidence selection, restricted moderation access, audit logs, alerts, appeals, rate limits

SC-09 Incident Response: Severity levels, defined owners, containment steps, user notification criteria, evidence handling, recovery procedures, post-incident review

### Thesis Structure

This thesis is organized into six chapters:

Chapter 1 examines security architecture and authentication system including directional authorization, session management, user roles, and tenant isolation.

Chapter 2 analyzes CSRF protection implementation and input validation strategies including token generation, server-side validation, and defense-in-depth approaches.

Chapter 3 explores safe API behavior and frontend security measures including consistent response formatting, error handling, XSS prevention, and content security policy.

Chapter 4 presents database architecture and migration strategies including schema design, tenant isolation at database level, transactions, and data integrity constraints.

Chapter 5 addresses infrastructure and deployment security including HTTPS configuration, certificate management, backup procedures, disaster recovery, and incident response planning.

Chapter 6 synthesizes conclusions drawn from the project, summarizing key findings, acknowledging limitations, and identifying opportunities for future enhancement.

The appendices provide supporting materials including sequence diagrams, user flow charts, database schema diagrams, security test matrices, API reference documentation, and requirements traceability matrix.

### Significance and Contribution

This project contributes practical patterns for building safety-critical mobile applications where privacy and explicit consent cannot be compromised. The directional authorization model, stale-state detection system, and comprehensive security control register provide reusable approaches for developers facing similar challenges in location-based services, health tracking applications, or any system handling sensitive personal data.

The extensive documentation and testing evidence demonstrate that rigorous security practices can be implemented within internship timeline constraints while maintaining feature completeness for core functionality. The Always Together system proves that privacy-enhancing design need not sacrifice usability—users can maintain meaningful location awareness of trusted contacts while retaining full control over their data.

Key contributions include: the Directional Authorization Pattern for decoupling social relationships from data access permissions; the Stale-State Detection Framework for honestly representing data freshness; the Security Control Register documentation format; Battery-Aware Location Sampling algorithms; and Comprehensive Requirements Traceability linking requirements through implementation to evidence.

