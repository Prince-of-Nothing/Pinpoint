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

The detailed, month-by-month plan is in
[Development Plan: October to December](../how-to/development-plan-october-december.md);
September status is in [Implementation Plan and Status](../how-to/implementation-plan.md).

- Production authentication, passkeys/MFA, recovery, and device/session
  management.
- Mobile battery and background-location validation on supported devices.
- Strong encryption/key management, database roles, backups, and deployment
  automation.
- Notifications, scheduled jobs, offline recovery, and reliability
  improvements.
- Security testing, dependency scanning, penetration-oriented review, and
  incident exercises.
- Location trails, traversal maps, caches, friend mosaics, groups, prompts,
  challenges, and moderation only after the safety core is stable.
- iPhone and web support based on platform testing rather than assumption.

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
