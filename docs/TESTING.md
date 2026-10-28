# Testing Strategy

Goal: every security-relevant behavior has executable evidence (thesis traceability:
requirement → implementation → test).

## Layers
| Layer          | Tooling                         | Where                     | Status |
|----------------|---------------------------------|---------------------------|--------|
| Unit           | `go test` stdlib                | `backend/internal/*_test.go` | ✅ passing |
| Security cases | table-driven tests              | `internal/security`, `internal/handlers` | ✅ (CSRF, rate limit, authz) |
| Integration    | httptest against full router    | `internal/handlers`       | 🚧 expanding |
| E2E (web)      | Playwright smoke flows          | planned `e2e/`            | 📋 |
| Mobile         | JUnit + Espresso (Kotlin)       | planned `mobile/`         | 📋 |
| Load           | k6 on `/locations` hot path     | planned `perf/`           | 📋 |

## Required test matrix for location access (the heart of the thesis)
For each case, assert status code AND payload shape (no leakage):
1. No friendship → 403
2. Friendship accepted, no permission → 403
3. Permission active, `exact` → full coords + accuracy
4. Permission active, `approximate` → jittered coords, accuracy ≥ threshold
5. Permission active, `city_only` → centroid only, no raw coords anywhere in body
6. Permission paused → 403
7. Permission revoked → 403, immediately after revoke (no cache)
8. Permission expired (past `expires_at`) → 403
9. Recipient ≠ authenticated user (IDOR: change ID in URL) → 403/404 generic
10. Sharer themselves → always own data
11. Locked user session → rejected; must-change-password user → 403 except change endpoint
12. Missing/wrong `X-CSRF-Token` on session-auth write → 403
13. Rate limit exceeded on login → 429, later attempts recover
14. Stale-state: old `updated_at` → freshness field reflects stale/dead, never "fresh"

## Conventions
- Table-driven tests, helper `newTestServer(t)` builds router with seeded isolated store.
- Time-dependent tests inject a clock (no sleeps).
- Test fixtures use fictional data only (`*_test.go` seeds mirror demo accounts).
- Race detector in CI: `go test -race ./...`.

## Running
```bash
cd backend
go test ./...          # unit + security + handler suites
go test -race ./...    # before merging anything touching store/session maps
```

## Evidence for the thesis
Each requirement in `thesis_content.md`'s MVP list cites the test function(s) proving it.
When adding features, extend this mapping (update the table above + CHANGELOG).
