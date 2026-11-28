## Summary

<!-- One paragraph: what changed and why. -->

## Type of change

- [ ] `[FEATURE]` new capability
- [ ] `[FIX]` bug fix
- [ ] `[SECURITY]` hardening
- [ ] `[DOCS]` documentation
- [ ] `[INFRA]` build/deploy
- [ ] `[RELEASE]` version bump / packaging

## Workflow (mandatory)

- [ ] Commits are on `Beta` (this PR is the release PR from `Beta` to `main`)
- [ ] PR title starts with the type flag above
- [ ] Every commit has a type flag and the gate passed before it was pushed
- [ ] After CI is green: merge with a merge commit (not a squash), then tag the
      release commit on `main`

## Checklist

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `cargo build --locked && cargo test --locked` passes
- [ ] wasm check and `cargo audit` pass (or explained below)
- [ ] Android APK build passes (if `src/platform.rs`, `src/mobile/` or Android metadata changed)
- [ ] `docker compose config --quiet` passes (if infra touched)
- [ ] `CHANGELOG.md` `[Unreleased]` updated (Added/Changed/Fixed/Removed)
- [ ] Docs/ADR updated in the same PR
- [ ] No secrets, tokens, keystores or real user data
- [ ] No `Co-authored-by:` trailers on any commit (owner-only authorship, see `AGENTS.md`)

## Related

Closes # <!-- required; use Closes #N or Refs #N -->
