# Testing and Quality Strategy

## As built: how to run the checks

```bash
cd backend
pip install -r requirements-dev.txt
pytest                                   # 140 tests, in-memory SQLite, ~7 s
pytest --cov=app --cov-report=term-missing
ruff check . && ruff format --check .    # lint and formatting
bandit -c pyproject.toml -r app          # static security analysis
pip-audit -r requirements.txt            # known-vulnerable dependencies
python scripts/security_evidence.py      # regenerates docs/evidence/security-test-report.md
node ../e2e/web-client-flow.js           # browser run (needs Playwright and a running API)
```

| Layer | What is covered | Where |
|---|---|---|
| Unit/integration (API) | Every endpoint through FastAPI's test client, including authorization boundaries | `backend/tests/test_*.py` |
| Security scenarios | Injection, mass assignment, IDOR matrix, CSRF, CORS, token forgery, error leakage, secrets in logs | `backend/tests/test_security_attacks.py` |
| Migrations | Upgrade, model/migration drift check, downgrade on PostgreSQL | CI job |
| Browser end-to-end | Two-user flow, session resume, revoke, logout, XSS payload, CSP | `e2e/web-client-flow.js` |
| Static and supply chain | ruff, bandit, pip-audit | CI job |

Tests use cheap Argon2 parameters and an in-memory rate limiter for speed;
the Redis limiter has its own test when Redis is available.

## Original strategy (full product)

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
