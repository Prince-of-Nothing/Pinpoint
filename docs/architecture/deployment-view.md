# 7. Deployment View

## 7.1 Infrastructure

Use managed encrypted storage, backups with tested recovery, secret management,
TLS, rate limiting, monitoring, and alerting. Separate production location data
from development and analytics environments.

## 7.2 Environments

| Environment | Hosting | Notes |
|-------------|---------|-------|
| Development / internship MVP | Controlled development or test environment | Production claims should not be made until authentication, TLS, encryption, backups, monitoring, incident response, and recovery have been tested |
| Production (future) | Hybrid/managed: containerized application services with managed PostgreSQL, managed object storage | Infra-as-code designed so a later move to self-hosting stays feasible; see [Technology Stack and Infrastructure](../reference/technology-stack-and-infrastructure.md) |

## 7.3 Operational Playbooks

Operational playbooks should cover location-service outage, notification delay,
account takeover, data breach, abusive group, malicious upload, moderation
escalation, and experimental bot resource exhaustion. See
[Incident Response Plan](./incident-response-plan.md).

## 7.4 Rollout

Roll out background-location changes gradually and monitor battery, crash,
permission, and stale-update metrics without collecting unnecessary precise
history.

## 7.5 CI/CD

Docker + GitHub Actions per the frozen stack (see
[§4.2](./solution-strategy.md#42-frozen-mvp-technology-stack)). CI should
include tests, dependency vulnerability scanning, secret scanning, schema-change
checks, and mobile permission behavior checks (see
[Technology Stack and Infrastructure §"Security and operations"](../reference/technology-stack-and-infrastructure.md)).

---
*Part of the [architecture documentation](./README.md) (arc42 §7).*
