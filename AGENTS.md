# AGENTS.md — Pinpoint (Rust multiplatform)

## Repo shape (active — Rust only)

- **Rust multiplatform client** (root crate `pinpoint-desktop` v1.2.1):
  `src/lib.rs` (portable core) + `src/main.rs` (desktop), eframe/egui UI +
  `reqwest`. One codebase for desktop / Android (`cargo-apk`) / iOS
  (`cargo-xcodebuild`) / wasm stub — see `docs/adr/0007-rust-multiplatform.md`.
  Modules: `api` (session cookie + `X-CSRF-Token` double-submit, plain HTTP
  only for loopback, ADR-0006), `app`/`ui` (state/screens), `map` (pins +
  OSM hand-off + ODbL credit), `osm` (Nominatim/OSRM/Overpass in Rust),
  `platform` (all `cfg(target_os)`), `bookmarks` (on-device JSON).
- `tests/contract.rs` pins the wire contract. CI `rust` job runs
  fmt/clippy/build/test/audit.
- Docs in `docs/` (arc42 + Diátaxis + ADRs; SDLC set `docs/sdlc/00–17`
  incl. the spiral execution plan; UML `docs/uml/01–15`).
  Contribution rules in `CONTRIBUTING.md`.

## Archive (`archive/` — NOT built)

Everything else was archived in the Rust-multiplatform pivot:
`backend/` (Go + Python), `web/` (vanilla JS), `mobile/` (Kotlin),
`app/` (gidireads Rust static site), `e2e/`, `docker/`, `scripts/`,
`shots/`, plus compose/render/binaries and junk files. See
`archive/README.md`. Do not build, lint or import from `archive/`; consult
it only as historical reference. OSM maps policy lives in
`docs/sdlc/07-openstreetmap-integration.md`; never use
`tile.openstreetmap.org` (OSMF policy).

## Commands (Rust — must pass before pushing)

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked && cargo test --locked
cargo run -- --backend http://localhost:8000   # desktop dev (loopback only)
```

Android/iOS need their Rust targets + NDK/Xcode (see `README.md`
multiplatform section); they share `src/` via `cfg(target_os)`.

## Gotchas

- Security is thesis-mandated: server-side enforcement only, no secrets or
  real user data anywhere including tests (`CONTRIBUTING.md`).
- Password/session/token wire contract lives in `src/api.rs` + `tests/` —
  change it only with a contract test update and an ADR if semantic.
- Platform code goes in `src/platform.rs`; OSM network calls in `src/osm.rs`;
  nothing platform- or network-specific inside `src/ui.rs`.

## Commits

Every commit is authored by the repository owner only. No `Co-authored-by:`
trailers — not for teammates, not for bots (`qwen.ai[bot]`, copilot,
dependabot).

Prefix every subject with a type flag so history is scannable:

- `[FEATURE] area: what was added` — new capability
- `[FIX] area: what was broken` — bug/regression fix
- `[DOCS] area: what was documented` — docs only
- `[RELEASE] vX.Y.Z` — version bump, release notes, packaging

Keep commits small and single-purpose. Do not mix a release commit with
feature work, and do not leave a completed feature uncommitted. The release
commit is the only one tagged `vX.Y.Z`.


## Module map (AI-friendly)

Pinpoint is one Rust crate (`pinpoint-desktop`) serving desktop/Android/iOS/Web.
All platform code lives in `src/platform.rs`; all OS branches are there.

- `src/lib.rs` declares modules; `src/main.rs` is the desktop entry.
- `src/api.rs` — HTTP client (session cookie + CSRF, URL policy ADR-0006).
- `src/app.rs` — `PinpointApp` state, depends on `DynBackend` (not a concrete client).
- `src/backend/` — `DynBackend` trait + REST adapter (swappable backend).
- `src/ui.rs` + `src/ui/` — egui shell + 10 per-screen modules.
- `src/osm.rs` — Nominatim/OSRM/Overpass; friend coords never sent.
- `src/map.rs`, `src/tilepacks.rs`, `src/bookmarks.rs` — map pins, offline tiles, bookmarks.
- `src/app_lock.rs` — optional on-device Argon2id PIN lock (launch/foreground lock).
- `src/models.rs` — serde domain models + pure math.
- `src/i18n.rs` — 79-language registry + `t()` + auto-detect.
- `src/security/` — one mechanism per file (validation, rate_limit, encryption, audit, session, signing, mfa, password).
- `src/patterns/` — Repository (InMemory + Postgres), Strategy, Command, State, Observer, Factory, Decorator, Proxy.
- `src/server/` — Axum services: `mod.rs` (routers, /health, convergence middleware), `state/` (mod, store facade, users, friendships, locations, sessions, diagnostics), `*_handlers.rs`, `migrations.rs` (v1–15).
- `src/bin/` — one binary per deployable (`pinpoint-server`, `auth`, `users`, `locations`, `map`, `diag` 8005, `admin` 8006).
- `src/webapp/` — portable web clients (admin, kiosk, view-model).
- `src/mobile/` — platform adapters (android, ios).

Additive seams (never rewrite): new table → migration + domain module; new screen → `src/ui/<name>.rs`; new service → router + binary + gateway; new language → `Lang` variant + block; new platform → `platform.rs` only.
## Changelog & auto-commit on "push"

- Every completed feature gets a commit (per the Commits section) and a
  one-line entry under `[Unreleased]` in `CHANGELOG.md` (Added/Changed/
  Fixed/Removed). The `scripts/commit.ps1 -Message "..."` helper does
  both: runs the full gate, prepends a stub, commits.
- When the user says "push" (or "commit and push"): run the full gate,
  update `CHANGELOG.md`, commit, then `git push` (only after the gate is
  green). Never push a red tree.
- Never commit without running `cargo fmt/clippy/build/test` first.

## Docs conventions

- Diagrams are Mermaid text, never binary images; keep the table in
  `docs/uml/README.md` in sync with files that exist.
- ADRs are immutable once accepted, named `NNNN-kebab-title.md`, indexed in
  `docs/adr/README.md`; architecture-significant changes need an ADR in the same PR.
