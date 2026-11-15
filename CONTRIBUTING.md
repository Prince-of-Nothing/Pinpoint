# Contributing to Pinpoint

Thanks for your interest! This is a thesis project with strict security and documentation
requirements, so contribution rules are tighter than a typical hobby repo.

## Ground rules

1. **Security is non-negotiable.** Any change touching authentication, authorization, consent,
   or location data requires: (a) a test proving the old behavior still holds, and (b) a test
   proving the new attack surface fails closed. If unsure, open an issue first.
2. **Server-side enforcement only.** Never implement a permission rule solely in a client.
   The client is untrusted by design.
3. **Small, reviewable PRs.** One concern per PR. Refactors separate from behavior changes.
4. **Docs follow code.** New endpoints/features need a doc update (README table, API notes,
   CHANGELOG entry) in the same PR.
5. **No secrets, no real user data** anywhere — including tests. Use the seeded demo identities.

## Development workflow

```bash
# Rust client + server (the only active codebase)
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
cargo test --locked

# Optional gates
cargo check --locked --target wasm32-unknown-unknown \
  --bin pinpoint-web --bin pinpoint-weba --bin pinpoint-webk
cargo audit
cargo apk build --lib --target aarch64-linux-android   # Android changes
```

Everything under `archive/` is historical and is not built.

## Branching and pull requests (mandatory)

`main` is release-ready at all times. **Never commit or push directly to `main`.**

1. Start from an up-to-date `main` and create one branch per change:
   `feat/<slug>`, `fix/<slug>`, `docs/<slug>`, `infra/<slug>` or
   `release/vX.Y.Z`.
2. Commit with a type flag: `[FEATURE] area: ...`, `[FIX] ...`, `[SECURITY] ...`,
   `[DOCS] ...`, `[INFRA] ...`, `[RELEASE] vX.Y.Z`.
3. Push the branch, open a pull request and fill in the template. The PR title
   uses the same type flag. Every PR must reference the issue it fixes with
   `Closes #N` or `Refs #N`; the `PR issue link` check enforces this.
4. Wait for CI/review, then **Squash and merge** so `main` keeps one
   type-flagged commit per change. Delete the branch.
5. Update local `main` and create the **next** branch from it before starting
   the next feature.

Multiple branches are allowed for independent work, but each must be based on
`main`, rebaseable and independently mergeable. Do not keep stacked or
long-lived branches; merge the base PR first. Releases branch from `main`, merge
through a PR, and are tagged on `main` afterward.

Saying "push" means push the current branch and open/update its PR — never push
`main`.

## Pull request checklist

- [ ] Branch is based on the latest `main` and has no unrelated changes
- [ ] PR title starts with the correct type flag
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `cargo build --locked && cargo test --locked` passes
- [ ] wasm gate and `cargo audit` pass (or the PR explains why not)
- [ ] Android APK build passes when `src/platform.rs`, `src/mobile/` or
      `Cargo.toml` Android metadata changed
- [ ] `CHANGELOG.md` `[Unreleased]` updated (Added/Changed/Fixed/Removed)
- [ ] Docs/ADR updated in the same PR
- [ ] No secrets, tokens, keystores or real user data
- [ ] No `Co-authored-by:` trailers (owner-only authorship, see `AGENTS.md`)
- [ ] CI is green; merge with **Squash and merge**, then delete the branch

## Issue tracker

Every issue has exactly one native GitHub type (**Bug**, **Feature** or
**Task**), a `PREFIX-n` title ID, one `severity:*` label and one `priority:*`
label. Triage an issue in this order:

1. Set the type.
2. Rename the title to the next ID, for example `FEAT-6: Offline map packs`.
3. Add `severity:critical|important|medium|minor`.
4. Add `priority:asap|high|normal|low`.
5. Add domain labels and `needs:compliance` when privacy, consent, security or
   location data is involved.

Create a correctly triaged issue with:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/new-issue.ps1 `
  -Prefix FEAT -Type Feature -Title "Offline map packs" `
  -Severity medium -Priority normal -Label android
```

`-DryRun` prints the next ID and command without creating anything. The full
rules, severity/priority definitions and label set are in
[`docs/ISSUE_TAXONOMY.md`](./docs/ISSUE_TAXONOMY.md).

## Code style

- Rust 2021; `rustfmt` and `clippy -D warnings` are mandatory.
- Platform/OS branches live in `src/platform.rs`; OSM network calls in
  `src/osm.rs`; no platform or network specifics in `src/ui.rs`.
- UI code goes in `src/ui.rs` or a per-screen `src/ui/<name>.rs` module.
- Prefer explicit errors (`anyhow::Result`, `thiserror`) over panics; no
  `unwrap()` on request or user-data paths.
- Keep server-side enforcement authoritative; client checks are advisory.

## Reporting bugs

Open an issue with: reproduction steps, expected vs actual, environment (Docker vs local),
and — importantly — **do not publicly disclose security vulnerabilities**; see `SECURITY.md`.

## Proposal process (RFC-lite)

For anything larger than a bugfix, open an issue titled `RFC: <summary>` describing problem,
proposed approach, alternatives, and security implications. Get a 👍 from the maintainer
before writing code. This keeps the thesis scope honest.

## License note

Until a LICENSE file is added, contributions are made under "inbound=outbound, all rights
reserved to the project author" (academic work). Ask before repurposing code elsewhere.
