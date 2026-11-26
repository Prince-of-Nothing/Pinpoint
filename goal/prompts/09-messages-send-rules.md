# 09 — Messages: test the send-draft rules

Read `goal/CONTEXT.md` first. Claimed by `<agent>`.

## Context

`src/ui/messages.rs` has a composer that trims the draft before sending and
accepts only non-empty text up to 2000 characters. The rule is inline in the
draw code; only draft restoration and focus are tested. A whitespace-only or
over-limit message should never spawn `SendMessage`.

## Task

Extract a pure `message_to_send(draft: &str) -> Option<String>` in
`src/ui/messages.rs`, use it in the composer, and add tests for:
- empty and whitespace-only drafts → `None`;
- a normal draft is trimmed and returned;
- exactly 2000 characters → `Some`;
- 2001 characters → `None`.

Keep the existing `char_limit(2000)` UI hint and the clear-on-send behaviour.

## Files

- `src/ui/messages.rs` only.

## Steps

1. Add the helper near `restore_message_draft`.
2. Replace the inline trim/length branch with it.
3. Add the four focused tests in the existing module.

## Verify

```text
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
cargo test --locked
```

Expected: the new tests pass; existing message tests stay green.

## Done

- `[FIX] Native messages: test the send-draft rules` in `CHANGELOG.md`.
- Branch, issue, PR with `Closes #N`, green CI, squash merge.
- `powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 done -Id 09 -Agent <agent>`
