# Implementation Plan and Status: Always Together MVP

This page tracks the internship tasks against what is actually in the
repository. It replaces the earlier 8-week plan, which assumed OIDC/PKCE and a
Flutter client; the reasons for the changed scope are recorded in
[ADR 0003](../adr/password-auth-with-rotating-refresh-tokens.md),
[ADR 0004](../adr/web-client-for-mvp-demonstration.md) and
[ADR 0005](../adr/store-latest-location-only.md).

The next phase is planned in the
[Development Plan: October to December](./development-plan-october-december.md).

## Internship task status

| # | Task | Status | Evidence |
|---|---|---|---|
| 1 | Project idea, MVP scope, users, use cases | Completed | [Introduction and Goals](../architecture/introduction-and-goals.md), [Scope](../architecture/scope-and-mvp-boundaries.md) |
| 2 | Domain and security analysis | Completed | [Security Threats and Controls](../architecture/security-threats-and-controls.md), [Privacy](../architecture/privacy-and-data-handling.md) |
| 3 | Architecture, technologies, frontend/backend/database/API structure | Completed; as-built updates added | [Building Blocks](../architecture/building-block-view.md), [Deployment](../architecture/deployment-view.md), [Stack](../reference/technology-stack-and-infrastructure.md) |
| 4 | Prioritise security requirements and mechanisms | Completed | [Security Control Register](../reference/security-control-register.md) |
| 5 | Implement essential MVP functionality | **Completed** (MVP scope) | [API Contract](../reference/api-contract.md), [RTM status](../reference/requirements-traceability-matrix.md#implementation-status-as-built-september-mvp), web client |
| 6 | Implement basic security controls | **Completed** | Register "Mapping to the internship Task 6 list" |
| 7 | Test controls; document problem, approach, method, limitations | **Completed** | [Security Test Report](../evidence/security-test-report.md), [Evidence index](../evidence/README.md) |
| 8 | Document the work; plan October-December | **Completed** | This page, ADRs 0003-0005, [development plan](./development-plan-october-december.md) |
| 9 | Prepare and demonstrate the MVP | **Prepared** | [Demonstration guide](./deploy-for-demonstration.md), [Demo script](./mvp-demo-script.md) |

## What was built

### Task 5: MVP functionality

| Feature | REQ | Where |
|---|---|---|
| Accounts: register, login, profile, sessions, password change | REQ-01 | `backend/app/api/auth.py`, `users.py` |
| Private-by-default profiles | REQ-01 | `GET /users/{username}` |
| Friend requests, accept/decline/remove, blocking | REQ-02 | `api/friendships.py`, `api/blocks.py` |
| Directional location shares: exact/approximate, purpose, expiry, revoke | REQ-03 | `api/location_shares.py` |
| Location updates and recipient view with freshness and battery | REQ-03/04/06 | `api/locations.py`, `services/locations.py` |
| Check-ins with grace period and missed state | REQ-05 (reduced) | `api/checkins.py` |
| Map with OSM attribution, freshness markers, navigation links | REQ-07 | `backend/web/` |
| Admin account management and audit log | REQ-08 | `api/admin.py`, `app/cli.py` |

### Task 6: security controls

Secure authentication (Argon2id, lockout, generic errors, timing
equalisation), session-bound JWTs with rotating refresh tokens and reuse
detection, CSRF-protected cookie refresh, RBAC and directional authorization
with audited IDOR denials, strict input validation, Problem Details errors,
rate limiting, security headers and CSP, a request size limit, a
least-privilege database role with an append-only audit table, and a
production configuration guard. Details: [Security Control Register](../reference/security-control-register.md).

### Task 7: testing

140 automated tests (97% line coverage) plus a browser end-to-end run, with
bandit and pip-audit clean. Every control has a problem statement,
implementation, tests, result and limitations in the register, and a
generated per-control test report.

## Commit history (feature by feature)

| Commit | Feature |
|---|---|
| Set up backend tooling and pinned dependencies | Task 3 |
| Rebuild application core and data model | Task 3/5 |
| Add Docker Compose environment and CI pipeline | Task 3 |
| Implement authentication and session management | Task 5/6 |
| Add API rate limiting | Task 6 |
| Add friendships, blocking and private-by-default profiles | Task 5 |
| Add directional location sharing | Task 5 |
| Add location updates and the authorised recipient view | Task 5 |
| Add safety check-ins | Task 5 |
| Add administrator role and operator endpoints (RBAC) | Task 6 |
| Add web client with OpenStreetMap map | Task 5 |
| Audit denied access to other users' records | Task 6 |
| Harden deployment: least-privilege database role and HTTPS proxy | Task 6 |
| Add attack-oriented security test suite | Task 7 |
| Document security controls with reproducible evidence | Task 7 |
| ADRs, as-built reference and architecture updates, this plan | Task 8 |
| Demo seed data, guide and script | Task 9 |

Run `git log --oneline` for the exact hashes.

## Known gaps carried into October

| Gap | Why deferred | Planned |
|---|---|---|
| Native mobile app with background location | Device testing time ([ADR 0004](../adr/web-client-for-mvp-demonstration.md)) | October |
| OIDC/passkeys, MFA, account recovery | Integration effort ([ADR 0003](../adr/password-auth-with-rotating-refresh-tokens.md)) | October to November |
| Scheduled/trip check-ins and push reminders | Need a worker and push provider ([ADR 0005](../adr/store-latest-location-only.md)) | November |
| Realtime updates (WebSocket/SSE) | Polling is sufficient for the demo | November |
| Encryption at rest, backups, monitoring in a hosted environment | No hosted environment yet | November |
| Abuse reporting and moderation (SC-08) | Not needed until groups/content exist | December |
| Account deletion and data export | Privacy workflow design pending | December |
