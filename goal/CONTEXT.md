# Goal context capsule

Read this before any prompt in `goal/prompts/`. It is the shared, small
context that keeps each prompt independent and safe to run in parallel.

## Project

Pinpoint is one Rust crate, `pinpoint-desktop` (v3.0.1), serving a desktop
client, an Android app, an iOS build and a portable wasm core. The shared UI is
egui/eframe; the server is Axum with an in-memory test mode and Postgres
adapters. One codebase, platform branches only in `src/platform.rs`.

## Where things live

- `src/ui.rs` + `src/ui/<screen>.rs` — egui shell and screens.
- `src/api.rs`, `src/backend/` — HTTP client and swappable backend.
- `src/osm.rs` — Nominatim/OSRM/Overpass models and calls.
- `src/offline_install.rs`, `src/region_catalog.rs` — PMTiles region packs.
- `src/mobile/map_server.rs` — loopback MapLibre + pack routes (Android).
- `web/map/`, `web/app/` — MapLibre page and web client.
- `tests/` — contract, router, consent and asset tests.
- `scripts/smoke.ps1` (server), `scripts/android-smoke.ps1` (device launch),
  `scripts/check-release.ps1` (release metadata).

## Mandatory delivery flow

1. Start from an up-to-date `main` on a fresh branch:
   `feat/<slug>`, `fix/<slug>`, `docs/<slug>`, `infra/<slug>` or
   `release/vX.Y.Z`. Never commit or push to `main`.
2. Open or reuse a GitHub issue with a `PREFIX-n` title, a type, one
   `severity:*` and one `priority:*` label. Use `scripts/new-issue.ps1`.
3. Keep the change to one deliverable. Run the full gate before pushing:
   `cargo fmt --all -- --check`,
   `cargo clippy --all-targets -- -D warnings`,
   `cargo build --locked`, `cargo test --locked`.
4. Add one line under `[Unreleased]` in `CHANGELOG.md` with the matching flag.
5. Commit with `[FEATURE]`, `[FIX]`, `[DOCS]`, `[SECURITY]` or `[INFRA]`.
   No `Co-authored-by` trailers.
6. Push the branch, open a PR with `Closes #N` or `Refs #N`, wait for CI,
   squash-merge, delete the branch, then update local `main`.
7. The claim moved the prompt out of `goal/prompts/`; include that deletion in
   the PR. After it merges, clear the local claim with
   `scripts/goal.ps1 done -Id NN -Agent <name>`.

## Current baseline (re-measure, do not assume)

- `cargo test --locked`: 420 library tests + 5 integration tests.
- `scripts/smoke.ps1`: 29 checks, 0 failures, against a running server.
- Android arm64 debug APK is aapt/apksigner verified for v3.0.0.
- The current main is whatever `git log -1` shows; never assume a hash.

## Security and privacy

- Server-side enforcement only. No secrets, tokens, real people or real
  locations in tests; use fictional data and `@example.invalid`.
- Plain HTTP is loopback-only; do not weaken `src/api.rs` URL policy.
- Do not log recovery codes, passwords, session cookies or CSRF tokens.
- Offline packs are ODbL data; keep attribution and do not add tile scraping.

## Environment

- Windows PowerShell is the default shell for repo scripts.
- Host target dir: `%TEMP%\pinpoint-host-target`.
- Android build: `android\gradlew.bat assembleDebug -Pabi=arm64-v8a` with
  `JAVA_HOME`, `ANDROID_HOME`, `ANDROID_NDK_HOME` set; verify with
  `aapt dump badging` and `apksigner verify --print-certs`.
- Docs: `AGENTS.md`, `CONTRIBUTING.md`, `docs/TESTING.md`, `docs/adr/`.
