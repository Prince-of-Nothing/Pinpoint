# AGENTS.md

## Repo shape

Two backend implementations of the same API, per the root `README.md`:

- **Go API** (default Docker stack): `backend/cmd/server` + `backend/internal/`.
- **Python/FastAPI API** (alternate): `backend/app`, migrations in `backend/alembic/`, tests in `backend/tests/`.
- Web clients: `web/` (Go stack, nginx) and `backend/web/` (Python-served).
- Android app: `mobile/` — Kotlin/Jetpack Compose, Clean Architecture modules (`app` composition root + `core:{domain,network,data,map}` + `feature:{auth,map,friends,sharing}`), MapLibre maps, 0 trackers. See `mobile/README.md`.
- Docs in `docs/` (arc42 + Diátaxis + ADRs); full-stack demo via `docker compose`.

Contribution rules live in root `CONTRIBUTING.md` (Go) and `docs/CONTRIBUTING.md` (docs): small PRs, Conventional Commits, docs in the same PR, security changes need a regression test *and* a fail-closed test.

## Commands

Go (run from `backend/`; must pass before pushing):

```bash
go build ./... && go vet ./...   # go test ./...   # gofmt -l .  (must print nothing)
go run ./cmd/server              # port 8000 (PORT to override)
```

Python (run from `backend/`):

```bash
pip install -r requirements-dev.txt
pytest                                   # addopts from pyproject: -q, testpaths=tests
pytest --cov=app --cov-report=term-missing   # what CI runs
ruff check . && ruff format --check .    # line-length 100, py311
bandit -q -c pyproject.toml -r app
pip-audit -r requirements.txt
alembic upgrade head && alembic check   # needs DATABASE_URL (PostgreSQL)
```

Full stack: `docker compose up --build` → http://localhost:8080 (seeds demo accounts; local-only credentials). Python API on :8001 via `docker compose --profile python up --build python-api`.

Android (from `mobile/`; needs JDK 17 + Android SDK 35 — **not installable on the original dev machine**, so compile verification happens in CI's `mobile` job):

```bash
gradle :app:assembleDebug          # debug APK; points at http://10.0.2.2:8000/api/v1/
gradle :core:domain:test :core:network:test   # pure-JVM module tests
```

CI (`.github/workflows/ci.yml`) runs all of the above plus migrations against a Postgres 16 service; `DATABASE_URL=postgresql+psycopg://postgres:postgres@localhost:5432/always_together`. The `backend-go` job sets `TEST_DATABASE_URL` so the store conformance suite (T01) runs against real Postgres.

## Gotchas

- Known pre-existing flake: `tests/test_auth.py::test_session_limit_revokes_oldest` passes alone but fails in a full local run (verified on pristine `origin/main`). Don't chase it as your regression.
- `backend/dump.rdb` is a stray committed Redis dump — leave it unless asked.
- Security is thesis-mandated: server-side enforcement only, no secrets or real user data anywhere including tests (root `CONTRIBUTING.md`).
- Password/session/token logic differs between the Go and Python implementations — mirror changes deliberately, and check which stack a doc describes before editing.
- Mobile module rules (enforced by Gradle deps): `:feature:*` sees `:core:domain` only; Retrofit/OkHttp stay in `:core:network`; MapLibre stays in `:core:map` (only `MapLibreIntegration.kt` + `OfflineMapManager.kt` import `org.maplibre.*`). New dependencies go in `mobile/gradle/libs.versions.toml`.
- Mobile privacy baseline is thesis-relevant: no Firebase/FCM/analytics/crash SDKs, ever. Any new third-party lib must be added to `mobile/THIRD_PARTY_NOTICES.md` in the same PR. OSM attribution must stay visible on the map screen (ODbL).

## Commits

Every commit is co-authored by the human team members (not bots). Add trailers on each commit:

```
Co-authored-by: Mordret <blacksnake@noreply.codeberg.org>
Co-authored-by: Andrei Cliucovschi <cliucovschiandrei@gmail.com>
Co-authored-by: Pleșu Dinu <dinu.plesu@isa.utm.md>
```

Exclude bots: `qwen.ai[bot]`, copilot, dependabot. Local git author is already Mordret.

## Docs conventions

- Diagrams are Mermaid text, never binary images; keep the table in `docs/architecture/diagrams/README.md` in sync with files that exist.
- ADRs are immutable once accepted, named `NNNN-kebab-title.md`, indexed in `docs/adr/README.md`; architecture-significant changes need an ADR in the same PR.

## Ported artifacts (not yet integrated)

These came from a separate Python-hardening port and are **not imported** by the current `backend/app` — don't assume they're wired in, and beware near-duplicate module names (`services/rate_limit.py` vs main's `services/ratelimit.py`, `token_service.py` vs `tokens.py`):

- `backend/app/services/{auth_service,authorization,password_policy,rate_limit,token_service,audit}.py`, `backend/app/repositories/`, `backend/app/middleware/`
- `docs/reference/design-patterns.md`, `docs/architecture/diagrams/` (9 C4/UML files), `docs/adr/0003-structure-backend-around-design-patterns.md`, root `AGENTS.md` (this file)
