# Development Plan: October to December

This plan continues from the September MVP (see
[Implementation Plan and Status](./implementation-plan.md)). It extends the
product only after the safety core is production-ready, in line with the
[roadmap](../explanation/development-roadmap-and-evidence.md). Each month
ends with a demonstrable increment and updated evidence.

## Guiding priorities

1. **Real devices first.** A location-safety app is only useful with reliable
   background updates on phones.
2. **Close the security gaps recorded in the register** before adding
   features that widen the attack surface.
3. **Keep evidence reproducible.** Every new control gets a register entry,
   tests and a line in the generated test report.

## October: native client and identity

| Work item | Details | Security improvement | Done when |
|---|---|---|---|
| Flutter app (Android first) | Login (`token_delivery: "body"`), friends, sharing, map (`flutter_map` + OSM attribution), check-ins | Tokens in Android Keystore / iOS Keychain (`flutter_secure_storage`); no tokens in logs | App reaches parity with the web client on a real Android device |
| Background location | Android foreground service with a visible notification; battery presets and automatic saver; iOS significant-change updates as a second step | Least-privilege permissions ("while in use" until the user enables background sharing); permission rationale screens | 24-hour battery test log on two devices |
| Account recovery | Email verification and password reset with single-use, short-lived, hashed tokens | Reset revokes all sessions; rate limited; generic responses | Tests for token reuse, expiry and enumeration |
| Device/session UI | Show device names and last activity; remote sign-out | Already supported by the API; add device naming | Sessions screen in the app |

## November: safety features and infrastructure

| Work item | Details | Security improvement | Done when |
|---|---|---|---|
| Worker and notifications | Celery + Redis worker; FCM/APNs push without sensitive content in previews | Notification payloads carry ids only; the app fetches details after authentication | Push delivered for share started/stopped and check-in reminders |
| Full check-ins (REQ-05) | Scheduled and trip check-ins; reminder before due; escalation to share recipients after grace | Escalation only to users with an active share; every escalation audited | Lifecycle tests including missed → escalated |
| Realtime updates | Server-Sent Events or WebSocket channel for `/locations/visible` changes | Authorization re-checked for every event; connection closed on revocation or logout | Revocation stops updates within 1 second in tests |
| Stronger authentication | TOTP MFA or WebAuthn passkeys for the web; step-up auth before creating an exact share or changing the password | Reduces account-takeover impact | MFA enrolment, login and recovery tests |
| Token signing | Move from HS256 to EdDSA/RS256 with key ids and scheduled rotation; publish a JWKS | Key compromise containment; enables separate verifiers | Rotation runbook tested |
| Staging environment | Managed PostgreSQL with encryption at rest, automated encrypted backups with a restore test, secrets manager, real TLS certificate, HSTS | SC-06 moves from Partial to Implemented | Restore drill recorded |
| Monitoring and alerting | Metrics and error tracking with PII scrubbing; alerts on refresh-token reuse, lockout spikes, `authz.denied` bursts, 5xx rate | Faster detection (SC-08/SC-09) | Alert fires in a simulated attack |
| Audit log shipping | Copy audit events to append-only storage outside the database | Owner-level database access can no longer silently alter history | Tamper test documented |

## December: abuse handling, privacy and validation

| Work item | Details | Security improvement | Done when |
|---|---|---|---|
| Abuse reporting (SC-08) | Report user/share with categories; moderator role; evidence retention rules | RBAC for moderators; moderators cannot see locations unless attached to a report | Moderator workflow tests |
| Privacy rights | Account deletion (with share revocation and audit pseudonymisation), data export, retention jobs | GDPR Articles 15/17/20 | Deletion and export tests; DPIA draft |
| Security testing | OWASP ASVS Level 2 checklist review; OWASP ZAP baseline scan and gitleaks secret scanning in CI; dependency update routine | Independent verification | Findings triaged and fixed or recorded |
| Incident exercise (SC-09) | Tabletop for "stolen refresh token used from another country" and "location data exposure"; one live simulation in staging | Tested response, measured time to contain | Exercise report with follow-up actions |
| Device and usability testing | Battery and background tests on Android and iPhone; accessibility of map alternatives and status labels | REQ-04/06 acceptance on real devices | Test logs and fixes |
| Next feature design | Movement trails and search corridors design and ADR only, with a separate governed store | Threat model before any history is stored | ADR accepted |

## Dependencies and risks

| Risk | Impact | Mitigation |
|---|---|---|
| iOS background location restrictions | Updates pause, stale data | Android first; honest stale/dead display already implemented |
| Push provider setup and review delays | No reminders | Keep in-app reminders; start FCM setup early in November |
| Hosting cost/credentials | No staging | Use a student credit or a small VPS with managed Postgres; everything is containerised |
| Scope creep from social features | Safety core slips | Social/content features stay behind the December design gate |
| Single developer availability | Slower delivery | Monthly increments are independent; each ends in a demo |

## Success criteria by end of December

- Android app used daily by a small test group with background sharing.
- MFA or passkeys available; recovery flow tested.
- Hosted staging with TLS, encrypted storage, tested backups and alerting.
- Full check-in lifecycle with push reminders and escalation.
- Register shows SC-01..SC-09 as Implemented or explicitly accepted with a
  documented limitation; ASVS L2 review completed.
