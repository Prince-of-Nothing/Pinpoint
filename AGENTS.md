# AGENTS.md

## Repo shape

- `backend/` — FastAPI (Python 3.11+, verified on 3.12) API. Layers: `app/api` (routers + DI deps) → `app/services` (facades, policies) → `app/repositories` (queries) → `app/db` + `app/models`. Cross-cutting: `app/core` (settings), `app/middleware`, `app/schemas` (Pydantic).
- `docs/` — arc42 architecture + Diátaxis (tutorials/how-to/reference/explanation) + ADRs. Diagrams are Mermaid text under `docs/architecture/diagrams/`.
- No CI, no lockfile, no `pyproject.toml`. There is no project-wide code CONTRIBUTING; docs rules live in `docs/CONTRIBUTING.md`.

## Commands (run from `backend/`)

```bash
pip install -r requirements.txt
uvicorn app.main:app --reload          # needs .env (see below)
pytest tests/ -q                       # full suite: 23 tests, all should pass
pytest tests/test_security_flows.py::test_name -q   # single test
```

- Tests need **no external services**: `tests/conftest.py` sets env vars (SQLite DB in a temp dir, `RATE_LIMIT_BACKEND=memory`) *before* any `app` import. Never import `app.main` in a script without setting those env vars first — `Settings` and the DB engine are created at import time.
- The `app` fixture drops/recreates all tables per test and clears the settings + rate-limiter caches; don't rely on state leaking between tests.

## Backend gotchas

- `.env` is not committed: `copy .env.example .env` and set `SECRET_KEY` (≥ 32 chars; production rejects placeholders and `DEBUG=true`). `ALLOWED_ORIGINS`/`TRUSTED_HOSTS` are comma-separated strings, not JSON.
- `bcrypt==4.0.1` is **pinned on purpose**: passlib 1.7.4 breaks with bcrypt ≥ 4.1. Don't bump.
- Signup/recovery/MFA are NOT implemented (tracked as risk R12). Tests create users via the `create_user` fixture or direct DB inserts.
- Errors are RFC 7807 Problem Details with an `error_code` field — never return raw `HTTPException` detail shapes from new endpoints; follow `app/api/errors.py` + `main.py` handlers.
- Layering rules (enforced by review, see `docs/reference/design-patterns.md`): routers never touch the ORM directly; services never import FastAPI types; repositories never `commit()` (services own the transaction); `get_auth_context()` in `app/api/deps.py` is the **only** place credentials are accepted; visibility/authorization decisions belong in `app/services/authorization.py` policies, not inline in endpoints.
- Password changes bump `token_version`, intentionally killing outstanding bearer JWTs — bearer clients must re-login (asserted by tests).
- Schema is created with `create_all` at startup; Alembic isn't wired (risk R9).

## Docs rules

- Follow the Diátaxis placement table in `docs/CONTRIBUTING.md`; one page = one purpose; relative links only.
- Diagrams are Mermaid text ("diagrams as code"), never binary images. If you add one, update the table in `docs/architecture/diagrams/README.md`.
- ADRs: immutable once accepted; filename `NNNN-kebab-title.md`; add a row to `docs/adr/README.md` AND `docs/architecture/architecture-decisions-link.md`. Architecture-significant code changes need an ADR in the same PR.
- Code and its docs change in the same PR (docs-as-code). Security controls must stay mapped in `docs/reference/security-control-register.md` (status + test names) when touched.
