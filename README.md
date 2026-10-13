# Always Together

A privacy-first location-sharing platform: stay connected with the people you trust, on your terms.

**Always Together** enforces *directional consent*: being friends with someone does **not**
automatically grant access to your location. Every share is explicit, configurable
(exact / approximate / city-only), purpose-bound, time-limited, and instantly revocable —
with all authorization decisions enforced **server-side**.

> 📚 This repository is part of a thesis project. See [`thesis_content.md`](thesis_content.md)
> for the full academic write-up, threat model, and validation evidence.

---

## Stack

| Layer        | Technology                                             | Location        |
|--------------|--------------------------------------------------------|-----------------|
| Backend API  | Go 1.22 · chi router · JWT/session auth · bcrypt       | `backend/`      |
| Web client   | Vanilla-JS SPA (OpenStreetMap tiles, no build step)    | `web/public/`   |
| Mobile       | Kotlin / Jetpack Compose (Android-first)               | `mobile/` *(planned)* |
| Deployment   | Docker Compose (API + nginx web) or single Go binary   | `docker-compose.yml` |

The original Python/FastAPI prototype was fully refactored to Go; no Python remains in the repo.

## Quick start (zero configuration)

```bash
docker compose up --build
```

Then open **http://localhost:8080**. Demo data is seeded automatically:

| Account | Email                | Password       | Notes                     |
|---------|----------------------|----------------|---------------------------|
| Admin   | `admin@example.com`  | `ChangeMe123!` | Forced password change    |
| Alice   | `alice@example.com`  | `DemoPass123!` | Friends with Bob          |
| Bob     | `bob@example.com`    | `DemoPass456!` | Mutual shares + Paris locs|

### Run without Docker

```bash
cd backend
go build ./... && go test ./...
WEB_ROOT=web/public SEED_DEMO_DATA=true COOKIE_INSECURE=true go run ./cmd/server
# API on :9000, serving the bundled web client too
```

## Core features (MVP)

- Pseudonymous accounts & profiles
- Mutual friendship requests (add / accept / reject)
- **Directional sharing permissions** with precision, purpose, and expiry
- Current-location publishing (device geolocation or manual coordinates)
- Freshness & stale-state indicators (never a false sense of safety)
- OpenStreetMap display + external navigation export
- Rate limiting, CSRF protection, secure errors, audit-friendly logging
- Admin provisioning (user creation, lock/unlock)

Deferred by design (documented as extension phases): movement trails, visited-region maps,
messaging/calls, random groups, advanced moderation. See `docs/ROADMAP.md`.

## Repository layout

```
backend/            Go API (cmd/, internal/{config,models,store,handlers,security})
  Dockerfile        Multi-stage static build, non-root, healthcheck
  web/public/       Copy of the SPA served when WEB_ROOT is set
web/                Web client source + nginx Dockerfile (proxies /api → backend)
docs/               Project description, roadmap, architecture, security policy,
                    contribution guide, code of conduct, release plan, glossary…
thesis_content.md   Full thesis document
docker-compose.yml  One-command deployment
```

## Documentation index

| Document                              | What it covers                                        |
|---------------------------------------|-------------------------------------------------------|
| [PROJECT.md](docs/PROJECT.md)         | Vision, problem statement, scope, objectives          |
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | System design, data model, request flow             |
| [SECURITY.md](docs/SECURITY.md)       | Security model + **vulnerability reporting**          |
| [PRIVACY.md](docs/PRIVACY.md)         | Consent model & data handling promises                |
| [ROADMAP.md](docs/ROADMAP.md)         | Future plans, phases, backlog                         |
| [CONTRIBUTING.md](CONTRIBUTING.md)    | How to contribute, review rules, dev workflow         |
| [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) | Community standards                                |
| [TESTING.md](docs/TESTING.md)         | Test strategy & evidence requirements                 |
| [DEPLOYMENT.md](docs/DEPLOYMENT.md)   | Docker, env vars, production checklist                |
| [CHANGELOG.md](CHANGELOG.md)          | Release history (Keep a Changelog)                    |
| [GLOSSARY.md](docs/GLOSSARY.md)       | Domain terms (directional consent, stale-state…)      |
| [TASKS.md](docs/TASKS.md)             | 40-task board: 5 members × 8 weeks, dependencies & completion protocol |
| [PLANNED_TASKS.md](docs/PLANNED_TASKS.md) | Backlog beyond the board (grooming list)            |
| [uml/](docs/uml/README.md)            | UML/C4 diagrams: context, containers, sequences, ER, states, deployment |

## License

Pending — this is currently an academic/thesis work. Until a license file is added,
all rights are reserved to the author.
