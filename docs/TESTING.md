# Testing Strategy

Goal: every security-relevant behavior has executable evidence (thesis traceability:
requirement → implementation → test).

## Layers

| Layer | Tooling | Where | CI job |
|---|---|---|---|
| Unit | `cargo test --lib` | `#[cfg(test)]` modules in `src/**` | `rust` |
| Integration | `cargo test --test <name>` | `tests/*.rs` — contract, router_probe, location_consent, today, auth_password, account_delete, assets, map_* | `rust` |
| Contract | recorded HTTP/1.1 mock (`tests/common`) | `tests/contract.rs` | `rust` |
| Web client | pure Node checks | `web/app/test/*.mjs` (sharing, admin, search, offline, map) | `web-tests` |
| Live web | headless Chromium against the served client (`/` focus and gating, drawer open/inside/Escape/map-click/1280-900-700 layout, route sheet and place sheet Escape with a stubbed reverse geocode, search result keyboard access) | `web/app/test/live-*.cjs` | `web-live` |
| Wasm | wasm32 compile check of the portable core | `src/webapp`, wasm stubs | `wasm` |
| Desktop build | release build of the native client | `src/`, `Cargo.toml` | `client-macos`, `release-windows` |
| Map data policy | OSM compliance check | tile/search routes and attribution | Map QA |
| Dependency audit | `cargo audit` with the accepted-advisory list | `Cargo.lock`, `.github/workflows/ci.yml` | `rust` |
| SAST | CodeQL (Rust) + `cargo clippy -D warnings` | `.github/workflows/codeql.yml` | CodeQL |
| Dependency updates | Dependabot (Cargo, GitHub Actions) | `.github/dependabot.yml` | Dependabot |
| End-to-end smoke | `scripts/smoke.ps1` against a running server | `scripts/` | manual |
| DAST | OWASP ZAP baseline against the running server | `.github/workflows/dast.yml`, reports in the run artifacts | DAST (weekly) |
| Backup crypto | `scripts/backup-db.ps1 -SelfTest` (encrypt/decrypt roundtrip), `restore-db.ps1 -SelfTest` (header + hash) | `scripts/` | manual |
| Release consistency | Cargo, Gradle, iOS plist, web cache marker, live `/health` + `/privacy`, and APK versionName/versionCode | `scripts/check-release.ps1` | `rust` (metadata + live server) + release workflow (APK) |

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

Integration tests drive the real Axum router with `tower::ServiceExt::oneshot`
and a throwaway `AppState`; the contract tests use the recorded mock server in
`tests/common` so the exact headers and body fields `ApiClient` sends are pinned.

## Conventions
- Unit tests live beside the code under `#[cfg(test)]`; integration tests go in `tests/<area>.rs`.
- Time-dependent behavior takes an explicit clock or compares ISO 8601 strings; no sleeps.
- Fixtures use fictional data only (`@example.invalid`), never real users or secrets.
- A wire-contract change updates `tests/contract.rs` and `tests/router_probe.rs` in the same PR.
- A web helper change gets a pure check under `web/app/test/` and a step in the `web-tests` job.
- New security rules get a focused integration test (see `tests/location_consent.rs`).

## Running

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked && cargo test --locked
cargo audit
node web/app/test/sharing-ui.mjs
node web/app/test/admin-ui.mjs
```

End-to-end smoke against a single binary and the in-memory store:

```powershell
cargo build --locked
$env:BIND_ADDR = "0.0.0.0:8080"
Remove-Item Env:DATABASE_URL -ErrorAction SilentlyContinue
.\target\debug\pinpoint-server.exe      # in-memory mode

# second terminal
powershell -ExecutionPolicy Bypass -File scripts/smoke.ps1
```

`scripts/smoke.ps1` signs up real sessions and checks the auth input rules, the
friendship rejection flow, location accuracy, share labels, notes, check-ins
and the search bound; its exit code is the number of failed checks.

`scripts/e2e.ps1` drives the Docker compose stack and predates the session-cookie
auth, so most of its checks are unauthenticated today; see the note in its header.

## Evidence for the thesis

Each requirement in the report's MVP list cites the test function(s) proving it.
When adding a feature, extend this mapping (update the table above and
`CHANGELOG.md`).
