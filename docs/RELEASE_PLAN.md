# Release Plan & Process

## Version policy
- SemVer; currently pre-1.0 (MVP thesis work). `0.x.y`: x = phase, y = increment.
- Tag every demo/defense milestone: `v0.3.0-thesis-demo`, etc.

## Release process (each tagged version)
1. Branch `release/vX.Y.Z` from `main`.
2. Freeze scope; verify the docs/TESTING.md matrix fully passes (`go test -race ./...`).
3. Update `CHANGELOG.md`: move Unreleased → versioned section with date.
4. Bump versions in `docker-compose.yml` image tags and any client manifests.
5. Build & smoke-test the compose stack from a clean machine (`docker compose up --build`).
6. Tag, push, open release notes (copy CHANGELOG section).
7. Merge back to `main`; delete branch.

## Planned releases
| Version | Theme                                   | Contents                                                   | Target        |
|---------|-----------------------------------------|------------------------------------------------------------|---------------|
| v0.3.0  | Docker-ready MVP ✅                     | Go backend, web SPA, compose deploy, seeded demo           | shipped 2026-10 |
| v0.4.0  | Persistence & realtime                  | PostgreSQL store, Redis rate limits/queue, WebSockets, audit table | next       |
| v0.5.0  | Mobile alpha                            | Kotlin app: auth, map, publish, share mgmt (Android emulator) | after v0.4.0 |
| v0.6.0  | Companion clients                       | `weba` admin console, `webk` kiosk check-in                |               |
| v0.7.0  | Check-ins                               | Scheduled/trip check-ins, missed-check-in alerts, notifications |          |
| v1.0.0  | Thesis submission build                 | Frozen feature set matching thesis document + full evidence | defense date  |
| post-1.0| Extensions                              | Trails/geofences/emergency mode — each gated by threat RFC | public beta   |

## Definition of done (per release)
- All checklist items above · zero known critical/high findings · honest "Known limitations"
  section updated in docs/SECURITY.md · demo recipe re-tested end-to-end.
