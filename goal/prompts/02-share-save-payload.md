# 02 — Share sheet: test the SaveShare payload

Read `goal/CONTEXT.md` first. Claimed by `<agent>`.

## Context

`src/ui/share_sheet.rs` keeps the per-friend share form and its Save/Stop
actions. `draw_share_sheet` builds `PendingAction::SaveShare { friendship_id,
share_id, enabled, precision, duration_hours, label }` inline for both actions;
the model is tested but the payload fields are not.

## Task

Extract a pure `ShareSaveRequest` plus
`share_save_request(sheet: &ShareSheet, enabled: bool) -> ShareSaveRequest`
in `share_sheet.rs`. Use it for Save (`sheet.enabled`) and Stop (`false`) so the
action always carries the same fields. Add tests in the module for both cases,
including an empty label and `share_id: None` on a new share.

## Files

- `src/ui/share_sheet.rs` — helper and tests. Do not edit `src/ui.rs`.

## Steps

1. Add the small struct with the six fields.
2. Replace the two inline `PendingAction::SaveShare` blocks with the helper.
3. Test Save and Stop field-by-field; include a new-share Stop guard if it is
   already in the app path.
4. Keep the existing notice/close behaviour unchanged.

## Verify

```text
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
cargo test --locked
```

Expected: new payload tests pass; all existing tests stay green.

## Done

- `[FIX] Native share: test the SaveShare payload fields` in `CHANGELOG.md`.
- Branch, issue, PR with `Closes #N`, green CI, squash merge.
- `powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 done -Id 02 -Agent <agent>`
