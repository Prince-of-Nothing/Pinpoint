# Security Threats and Controls

## Location and physical safety

- **Stalking or coercive tracking:** mutual friendship, directional consent, visible sharing, stop/revoke controls, sensitive-action re-authentication, and no sharing with non-friends.
- **Location breach or inference:** minimum collection, coarse defaults, retention limits, sensitive-area suppression, encryption, access logs, and relationship-based authorization.
- **False safety confidence:** last-update time, accuracy, battery freshness, stale/dead state, uncertainty labels, and clear non-emergency-service boundaries.
- **Battery exhaustion:** adaptive sampling, interval controls, low-battery behavior, OS-compliant background services, and battery-impact monitoring.
- **Location spoofing:** validate coordinates and timestamps, detect impossible movement where practical, and never treat a single point as confirmed truth.

## Identity and authorization

- **Account takeover:** passkeys or strong authentication, session/device management, recovery controls, rate limits, and alerts for sensitive changes.
- **IDOR or access-control failure:** enforce authorization on the backend for every profile, status, location, cache, group, report, and media request.
- **Username impersonation:** immutable internal IDs, username cooldowns, reserved names, change history, and clear identity indicators.
- **Malicious device access:** PIN/biometric/passkey confirmation for starting shares, changing trusted contacts, changing retention, and opening emergency context.

## Social and content abuse

- **Harassment and random-group abuse:** blocking, reporting, moderation roles, owner controls, age-aware discovery, rate limits, and separation from trusted location circles.
- **Child safety risk:** transparent age bands, safe defaults, reviewed neutral topics, no adult-targeted discovery of minors, human escalation, and legal review.
- **Malicious caches or media:** content scanning, file limits, dangerous-type restrictions, reporting, takedown, expiry, and encrypted storage.
- **Prompt safety failures:** AI screening, similarity analysis, human review of ambiguous cases, appeals, age-sensitive viewing controls, and a ban on sexual exploitation of minors.
- **Moderation manipulation:** independent report signals, duplicate/coordinated-report detection, transparent outcomes, appeals, and no hidden permanent karma punishment.

## Sponsored prompts and payments

- **Sponsor fraud:** company verification, blacklist checks, written prompt briefs, human approval, clear sponsorship labels, and cancellation rights.
- **Prize disputes:** publish eligibility, scoring, minimum-validity rules, judging weights, tie handling, taxes, processor fees, platform share, and delivery dates.
- **Paid influence:** sponsorship may affect a separate reviewed calendar or capped selection weight, but money cannot bypass safety review or guarantee a community prompt.

## Infrastructure and operations

- **Provider compromise:** encrypt sensitive data, separate keys, minimize plaintext access, use least privilege, and maintain portable exports.
- **Outage or notification failure:** show stale state honestly, retry safely, provide recovery procedures, and test backups.
- **Data leakage through logs/analytics:** redact location and identity data, separate production from analytics, restrict access, and define retention.
- **Vulnerable dependencies:** automated dependency scanning, patch policy, secret scanning, security tests, and incident response.

Security controls must be tested in the API, mobile clients, background-location
workflows, moderation tools, and operational procedures. A disclaimer cannot
replace safe defaults or reasonable controls.

See [Security Control Register](../reference/security-control-register.md) for
control-level evidence and
[Security Architecture and Data Protection](./security-architecture-and-data-protection.md)
for data classification, boundaries, encryption, authorization, and testing
design.

---
*Part of [§8 Crosscutting Concepts](./crosscutting-concepts.md).*
