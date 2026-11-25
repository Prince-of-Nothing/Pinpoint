# Evidence Index

One page that points to the executable evidence behind every security claim.
Use it when writing the report or preparing the demo; each entry can be
reproduced with the command shown.

## Gate (every merge)

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
cargo test --locked          # 381 lib tests + integration suites
cargo audit                  # 9 accepted advisories, no open vulnerability
node web/app/test/sharing-ui.mjs
node web/app/test/admin-ui.mjs
node web/app/test/messages-poll.mjs
node web/app/test/password-length.mjs
node web/app/test/navigation-ui.mjs
node web/app/test/oauth-ui.mjs
```

CI runs the same steps in the `rust` and `web-tests` jobs
(`.github/workflows/ci.yml`), plus wasm and macOS/Windows client builds.

## Live end-to-end checks

```powershell
# single binary, in-memory store
$env:BIND_ADDR = "0.0.0.0:8080"; Remove-Item Env:DATABASE_URL -ErrorAction SilentlyContinue
.\target\debug\pinpoint-server.exe
powershell -ExecutionPolicy Bypass -File scripts/smoke.ps1
```

`scripts/smoke.ps1` signs up real sessions and checks 29 rules: auth input
validation (bidi/multibyte email, name and password lengths), friendship
rejection and re-request, accuracy bounds, share labels, notes, check-ins and
the search bound. Exit code = failures; the latest runs are reported in the
PRs and `CHANGELOG.md`.

## Static analysis (SAST) and dependencies

- CodeQL (Rust): `.github/workflows/codeql.yml`; findings appear under
  Security → Code scanning.
- `cargo clippy -D warnings` and `cargo audit` in the `rust` job.
- Dependabot: `.github/dependabot.yml` (Cargo + GitHub Actions).

## Dynamic analysis (DAST)

- Workflow: `.github/workflows/dast.yml` (weekly + on demand); it builds and
  starts the server, then runs the ZAP baseline and uploads the report.
- Latest report: [`zap-baseline.html`](./zap-baseline.html) and
  [`zap-baseline.json`](./zap-baseline.json). Current result: **0 failures,
  0 medium warnings, 4 informational** (suspicious comments, cacheability).

## Backups and restore

- `scripts/backup-db.ps1 -SelfTest` → encrypt/decrypt roundtrip identical.
- `scripts/restore-db.ps1 -SelfTest` → decrypt identical, `PGDMP` header check.
- Runbook: [deployment](../DEPLOYMENT.md#encrypted-backups-and-restore).
- Decision and residual risk: [ADR-0025](../adr/0025-note-encryption-at-rest.md).

## Threat model and controls

- Scored threat model, STRIDE, attack tree: [security architecture §2](../security-architecture.md).
- Control register (SC-01…SC-09): [security control register](../reference/security-control-register.md).
- Requirement traceability (REQ-01…REQ-09): [traceability matrix](../reference/requirements-traceability-matrix.md).
- System models (use case, sequence, activity, state, ER, component,
  deployment): [system models](../analysis/system-models.md).
- Decisions: [ADRs](../adr/README.md) (transport, sessions, OAuth, ECIES,
  note encryption, Android bridge).
- Roadmap and remaining risks: [development roadmap](../explanation/development-roadmap-and-evidence.md).

## Artifact

- Current release: [`v3.0.0`](https://github.com/Prince-of-Nothing/Pinpoint/releases/tag/v3.0.0)
  with `Pinpoint-mobile-3.0.0-debug.apk` (76,387,294 B):
  `com.pinpoint.desktop` 3.0.0 (30000), `arm64-v8a`, minSdk 26,
  targetSdk 35, `allowBackup=0`, `usesCleartextTraffic=0`, network security
  config present. Build and `aapt` verification steps are in
  [deployment](../DEPLOYMENT.md).

## How to read a claim

Each security page follows the same shape: **problem → why it matters here →
implementation → test → result/limitation**. If a control has no executable
test, the page says so; the register marks it Partial or Deferred instead of
claiming coverage.
