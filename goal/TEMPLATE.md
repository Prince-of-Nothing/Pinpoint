# NN — Short imperative title

Read `goal/CONTEXT.md` first. Claimed by `<agent>`.

## Context

Two or three sentences: what exists, what is wrong or missing, and the exact
module or screen. Link a helper or pattern to reuse. No secrets or real data.

## Task

One deliverable. State the user-visible or testable result.

## Files

- `src/...` — what changes.
- `tests/...` or `src/...` tests — what proves it.
- `CHANGELOG.md` — one line.

## Steps

1. Small step.
2. Small step.
3. Add the focused test.

## Verify

```text
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
cargo test --locked
```

Expected: the new test fails before the change and passes after; all existing
tests stay green. Add any device/web check below this block.

## Done

- [ ] One `[FLAG]` line under `[Unreleased]` in `CHANGELOG.md`.
- [ ] Branch, issue, PR with `Closes #N`, green CI, squash merge.
- [ ] Prompt deleted:
  `powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 done -Id NN -Agent <agent>`
