# Evidence

Reproducible evidence for the implemented MVP and its security controls
(internship Tasks 5-7). Each file says how to regenerate it.

| Evidence | What it shows | How to reproduce |
|---|---|---|
| [security-test-report.md](./security-test-report.md) | Every automated test grouped by security control, with coverage, lint, bandit and pip-audit results for a specific commit | `cd backend && python scripts/security_evidence.py` |
| [database-least-privilege.md](./database-least-privilege.md) | The API's PostgreSQL role cannot alter the schema or tamper with the audit trail; client IPs survive the HTTPS proxy | Commands in the file, against `docker compose --profile https up` |
| [web-client-e2e.md](./web-client-e2e.md) | The real web client in headless Chromium: sharing, revocation, session resume, logout, XSS payload inert, no CSP violations | `node e2e/web-client-flow.js` |
| CI (`.github/workflows/ci.yml`) | Lint, bandit, pip-audit, migrations up/check/down on PostgreSQL, tests with coverage on every push | GitHub Actions |

See the [Security Control Register](../reference/security-control-register.md)
for how each control maps to its problem, implementation, tests and
limitations.
