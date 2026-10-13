# Testing and Quality Strategy

- Unit-test consent, expiry, access checks, retention, and navigation-link
  generation.
- Integration-test background updates, offline recovery, notifications,
  media handling, and moderation.
- Test Android and iPhone permission differences on real devices.
- Run accessibility checks on map alternatives, status labels, forms, and
  media.
- Security-test authentication, authorization, IDOR, abuse reporting, file
  uploads, rate limits, and audit logs.
- Load-test location updates separately from social and creative-media
  workloads.
- Use scenario tests for stopping sharing, account deletion, cache
  revocation, blocking, and stale location.
- Test movement trails with missing points, poor accuracy, transport
  changes, offline periods, and contradictory directions.
- Verify that only pre-authorized recipients can activate or view a search
  corridor, every access is logged, expiry works, and the UI never presents
  an estimate as a guaranteed location.
- Test visited-region generation, uncertain samples, region deletion,
  cache-range intersection, riddle gating, public-cache reporting, and
  protection of visitor identity and trail data.
- Test friend-layer opt-out, viewer-local hiding, friendship removal,
  consent revocation, identical versus different friend graphs, and
  suppression of home/sensitive regions.
- Test challenge invitations, participant withdrawal, group-chat membership
  changes, deadline locking, scoring fairness, private submissions, and
  results visibility.

See [Requirements Traceability Matrix](../reference/requirements-traceability-matrix.md)
for how these tests map to specific REQs and evidence, and
[Security Control Register](../reference/security-control-register.md) for
security-specific test expectations per control.

---
*Part of the [how-to guides](./README.md).*
