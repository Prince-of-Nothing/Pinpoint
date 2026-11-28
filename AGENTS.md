# AGENTS.md — Pinpoint (Rust multiplatform)

> **Commit rules:** before any git write (commit, push, tag or pull request), read
> [`COMMIT_RULES.md`](COMMIT_RULES.md). It sets who commits, bans AI trailers and
> footers, and keeps work on `Beta`.

## Repo shape (active — Rust only)

- **Rust multiplatform client** (root crate `pinpoint-desktop` v3.0.1):
  `src/lib.rs` (portable core) + `src/main.rs` (desktop), eframe/egui UI +
  `reqwest`. One codebase for desktop / Android (`cargo-apk` + Gradle/Java
  bridge, ADR-0017) / iOS (`cargo-xcodebuild`) / wasm stub - see
  `docs/adr/0007-rust-multiplatform.md`.
  Modules: `api` (session cookie + `X-CSRF-Token` double-submit, plain HTTP
  only for loopback, ADR-0006), `app`/`ui` (state/screens), `map` (pins +
  OSM hand-off + ODbL credit), `osm` (Nominatim/OSRM/Overpass in Rust),
  `platform` (all `cfg(target_os)`), `bookmarks` (on-device JSON).
- `tests/contract.rs` pins the wire contract. CI `rust` job runs
  fmt/clippy/build/test/audit.
- Docs: current architecture, ADRs, security, privacy, API contract and
  deployment stay in `docs/`; long-form design, SDLC, UML, component, tutorial
  and thesis docs live in the
  [Pinpoint wiki](https://github.com/Prince-of-Nothing/Pinpoint/wiki).
  Contribution rules in `CONTRIBUTING.md`.

## Repository history

Only the active Rust stack is tracked. The historical Go/Python/Kotlin/web
prototypes were removed from the working tree; consult git history or
ADR-0007 when you need them. OSM maps policy lives in the
[SDLC OSM page](https://github.com/Prince-of-Nothing/Pinpoint/wiki/SDLC-07-Openstreetmap-Integration);
never use `tile.openstreetmap.org` (OSMF policy).

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
- Android Java lives only in `android/app/src/main/java` for platform APIs with
  no Rust binding (ADR-0017). No Kotlin or AndroidX app rewrite. The APK is
  assembled by `android/gradlew`, not by cargo-apk's generated APK.

## Branching and pull requests (Beta workflow)

All work is committed directly to `Beta`. `main` is the release branch and is
updated only by a release PR.

- Commit each change to `Beta` with its type flag. Do not create per-change
  feature branches or open a PR per change. Keep commits small and
  single-purpose.
- Keep `Beta` current with `main`: after each release, merge `main` back into
  `Beta` so the two share history.
- Release: when it is ready, open one PR from `Beta` to `main`. Its title carries
  the release type flag (for example `[RELEASE] v3.0.1`), and its body lists the
  changes it brings and links them with `Refs #N`, or carries the
  `skip-issue-link` label. Merge it with a merge commit, not a squash, so `Beta`
  and `main` keep shared history. Then tag the release commit on `main` as
  `vX.Y.Z`.
- Never commit or push directly to `main`; only the release PR updates it.
- Run the full gate before every commit to `Beta`, and never push a red tree.
- Every issue carries a native type (Bug/Feature/Task), a `PREFIX-n` title ID
  and one `severity:*` plus one `priority:*` label. Use
  `scripts/new-issue.ps1` (see `docs/ISSUE_TAXONOMY.md`), and keep the tracker
  current when opening, closing or re-scoping work.
- When the user says "push", push `Beta`. Open the release PR only when the user
  asks for a release.

## Agent prompt queue (`goal/`)

- Every delegated task starts from a Markdown prompt in `goal/prompts/`; do
  not paste ad-hoc prompts outside this folder.
- Read `goal/README.md` and `goal/CONTEXT.md` first. Claim exactly one prompt:
  `scripts/goal.ps1 claim -Id NN -Agent <name>` (moves it to
  `goal/claimed/<name>/`, which is gitignored).
- The claim moves the tracked prompt out of `goal/prompts/`; the PR must include
  that deletion. After it merges, `done` only clears the gitignored local claim
  and refuses if the prompt is still pending. Release an abandoned claim with
  `release`.
- `scripts/goal.ps1 check` validates the queue (10-prompt cap, required
  sections, size, location). Keep the prompts small, independent and within
  one deliverable each.

## Commits

Every commit is authored by the repository owner only. No `Co-authored-by:`
trailers — not for teammates, not for bots (`qwen.ai[bot]`, copilot,
dependabot).

No AI trailers or footers in commit messages or PR bodies: no `Co-authored-by:`
naming Claude, no `Claude-Session:` lines, no "Generated with Claude Code". Enable the
repo hooks once per clone with `git config core.hooksPath .githooks`. The `commit-msg`
hook rejects AI trailers, and the `pre-commit` hook rejects a Claude git identity.

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
- `src/server/` — Axum services: `mod.rs` (routers, /health, convergence middleware), `state/` (mod, store facade, users, friendships, locations, sessions, diagnostics), `*_handlers.rs`, `migrations.rs` (v1–18).
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
  update `CHANGELOG.md`, commit on `Beta` and push `Beta` (only after the gate is
  green). Never push `main`; `main` changes only through the release PR from
  `Beta`. Never push a red tree.
- Never commit without running `cargo fmt/clippy/build/test` first.

## Docs conventions

- Diagrams are Mermaid text, never binary images; keep the wiki UML index in
  sync with the pages that exist.
- ADRs are immutable once accepted, named `NNNN-kebab-title.md`, indexed in
  `docs/adr/README.md`; architecture-significant changes need an ADR in the same PR.
