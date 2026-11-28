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

## Branching and pull requests (Beta workflow)

All work is committed directly to `Beta`. `main` is the release branch and is
updated only by a release PR.

1. Commit each change to `Beta` with a type flag: `[FEATURE] area: ...`,
   `[FIX] ...`, `[SECURITY] ...`, `[DOCS] ...`, `[INFRA] ...`, `[RELEASE] vX.Y.Z`.
   Do not create per-change feature branches or open a PR per change.
2. Run the full gate before each commit and keep `Beta` green.
3. Keep `Beta` current with `main`: after each release, merge `main` back into
   `Beta`.
4. Release: open one PR from `Beta` to `main` when the release is ready. Its
   title uses the `[RELEASE]` flag. Merge it with a merge commit, not a squash,
   so `Beta` and `main` keep shared history, then tag `vX.Y.Z` on `main`.

Saying "push" means push `Beta`. Never push `main` directly.

## Pull request checklist

- [ ] Commits are on `Beta`; a release PR is `Beta` to `main` with a `[RELEASE]` title
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
- [ ] CI is green; merge a release PR with a merge commit, not a squash

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
[`docs/ISSUE_TAXONOMY.md`](https://github.com/Prince-of-Nothing/Pinpoint/blob/main/docs/ISSUE_TAXONOMY.md).

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
