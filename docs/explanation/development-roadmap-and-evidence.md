# Development Roadmap and Evidence

## September foundation

### Week 1: definition and domain analysis

- Confirm problem, users, stakeholders, scope, and MVP non-goals.
- Review location privacy, battery, consent, and safety risks.
- Confirm mentor feedback and record open decisions.

### Week 2: architecture and security design

- Select provisional mobile, backend, database, map, notification, and
  hosting options.
- Define identity, friendship, directional consent, location, check-in, and
  audit data models.
- Complete threat model, risk ranking, data classification, and security
  control register.

### Week 3: core MVP flow

- Build or prototype account/profile, friendship, permission,
  current-location, map, and navigation flows.
- Add input validation, authorization, secure errors, rate limits, and
  basic audit events.
- Demonstrate exact/approximate recipient behavior and private non-friend
  profiles.

### Week 4: safety and evidence

- Add check-ins, reminders, stale/dead state, battery indicators, session
  controls, and deletion/expiry behavior.
- Test security controls and main user journeys.
- Run an incident-response tabletop exercise.
- Prepare demonstration, unresolved-risk list, and mentor review.

## October to December continuation

The detailed plan is maintained separately in
[october-december-implementation-plan.md](../../october-december-implementation-plan.md).

The continuation prioritizes a verifiable safety core before feature breadth.
The schedule assumes one small student project team and should be adjusted with
the internship mentor after October's initial repository and environment review.
Each month ends with a reviewable milestone; work does not advance to broader
location or social features if the preceding consent/security gate is unmet.

### October — Make the core flow runnable

**Focus:** close the gap between documented architecture and an executable,
testable backend slice, while researching the two required deployment modes.

- Confirm the approved MVP boundary with the mentor. Resolve the conflicting
  cache scope; keep caches and all other deferred features out of the critical
  path until the decision is recorded.
- Research user/organization-operated self-hosting versus a service operated
  by the project team. Compare deployment, identity, tenant/data boundaries,
  ownership, updates, backups, support, cost, and security responsibilities.
  Prefer a shared codebase with separate deployment profiles unless evidence
  supports separate implementations.
- Inventory the source tree, identify missing API modules and configuration,
  and document the baseline revision, setup steps, and known failures.
- Complete the backend application wiring and database/migration setup needed
  to start in a clean development environment.
- Implement the narrow vertical slice: two accounts, friendship, explicit
  share request/acceptance, location update, authorized recipient read, and
  revoke/expiry.
- Define and enforce the permission record fields: owner, recipient, purpose,
  precision/scope, expiry, and revocation state. Friendship removal/blocking
  must invalidate access.
- Add focused automated tests for allow/deny cases, object-ID substitution,
  precision filtering, revocation, expiry, and malformed coordinates.
- Keep secrets out of source and replace unsafe development defaults with
  validated local configuration.

**Exit criteria / evidence**

- A clean setup can start the API and apply migrations, or remaining blockers
  are recorded precisely with an owner and resolution plan.
- A repeatable automated test demonstrates the allowed share path and denial
  for wrong recipient, non-friend, blocked, revoked, expired, and IDOR requests.
- Capture environment, revision, commands, test results, and mentor review.
- Record a mentor-reviewed deployment-mode decision and unresolved
  requirements for each mode.

### November — Implement the base product in both deployment modes

**Focus:** connect the core API to a minimal mobile experience, make the base
safety product deployable in both modes, and verify freshness, check-ins, and
platform constraints.

- Implement a thin mobile client for sign-in, contacts/sharing state, explicit
  consent, location permission, recipient location display, and stop-sharing.
- Package/configure the same core application for self-hosted
  user/organization operation and project-team-hosted operation, subject to the
  October feasibility decision. Document deployment, secrets/configuration,
  data boundaries, updates/backups, and responsibilities for each mode.
- Display last update, accuracy, precision, and stale/dead states; do not show
  stale values as live.
- Implement the MVP check-in path (manual first; scheduled/trip behavior only
  if time and mentor-approved scope permit), with reminder, grace, completion,
  cancellation, and missed states.
- Exercise low-battery, offline, permission-denied, and OS-paused update
  behavior on available supported Android hardware; document limitations
  rather than assuming equivalent iPhone behavior.
- Add API security checks for authentication/session lifecycle, rate limits,
  safe errors, audit redaction, and authorization across HTTP and realtime
  surfaces. Realtime delivery remains optional until ordinary API enforcement
  is correct.
- Run dependency and secret scans; remediate relevant findings and record
  residual risks. Review accessibility for status labels, contrast, and
  non-color stale-state communication.

**Exit criteria / evidence**

- Demonstration of location consent, authorized display, freshness, check-in
  state, and immediate stop/revocation.
- Demonstration of both deployment modes or documented blockers, risks, and
  mentor-approved scope changes.
- Test report linked to REQ-01–REQ-08, including negative authorization tests.
- Device/OS, permission, connectivity, and battery observations recorded; no
  unsupported cross-platform or safety guarantees.
- Security scan output, accessibility findings, screenshots/video, and mentor
  feedback captured.

### December — Stabilize, assess optional features, and hand over

**Focus:** close high-priority defects, evaluate the result, and produce a
defensible demonstration and continuation decision.

- Fix critical/high issues from November tests and repeat affected regression
  suites.
- Verify retention, account deletion/revocation behavior, data export/deletion
  decisions, logging redaction, database least privilege, and backup/recovery
  only to the level actually implemented in the test environment.
- Conduct a tabletop incident exercise covering account takeover, unauthorized
  location exposure, provider/outage or delayed updates, and evidence handling.
- Reconcile the requirements traceability matrix: mark a requirement
  implemented or verified only when its corresponding test/demo evidence is
  retained.
- Prepare a final demo script, architecture and threat summary, unresolved-risk
  register, mentor review, and October–December activity record.
- Update the internship report and presentation with actual results, platform
  details, limitations, and personal contribution.
- Research effort, dependencies, privacy and threat implications for optional
  messaging, voice/video calling, group chats, and the unspecified
  social-media-style concept. Do not invent the concept's behavior; define and
  validate it before estimating implementation.
- Make a mentor-reviewed go/no-go decision for optional social features. They
  are not guaranteed December deliverables; implement only a small approved
  feature if the base product and both deployment modes meet their gates and
  time remains. Trails, search corridors, caches, friend mosaics, prompts,
  challenges, and advanced moderation remain deferred unless separately
  approved with consent, threat, retention, and acceptance plans.

**Exit criteria / evidence**

- Critical/high test failures are fixed or explicitly accepted with rationale,
  owner, and mitigation; regression results are recorded.
- Requirements/evidence matrix and unresolved-risk register are current.
- Final working-slice demonstration and incident tabletop are documented.
- Final report, slides, mentor feedback, and handover/next-phase decisions are
  ready for review.

### Cross-month gates and scope control

1. No mobile polish or deferred-feature implementation takes priority over
   correct server-side consent, revocation, and negative authorization tests.
2. Do not treat self-hosted and team-hosted delivery as unrelated products
   without recording why shared code and deployment profiles are inadequate.
3. Do not claim production readiness, legal compliance, cross-platform
   background reliability, battery performance, or security validation without
   corresponding evidence.
4. Any feature change must update the requirement, API/data model, threat and
   privacy analysis, tests, and traceability evidence together.
5. If a dependency or device is unavailable, record the blocker and replace the
   claim of verification with a scoped limitation; do not fabricate results.

## Evidence checklist

- Scope and stakeholder approval.
- Architecture and technology decision records.
- Threat model and risk register.
- Security control register with tests and limitations.
- API/data model and authorization examples.
- Working MVP demonstration.
- Unit, integration, security, accessibility, and battery test evidence.
- Incident tabletop record.
- Mentor feedback and change log.
- Deferred-work plan with dependencies and risks.

---
*See [DAS Internship Alignment](./das-internship-alignment.md) for how this
roadmap satisfies internship requirements.*
