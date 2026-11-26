# 06 — Docs: finish the testing matrix

Read `goal/CONTEXT.md` first. Claimed by `<agent>`.

## Context

`docs/TESTING.md` lists the test layers and the live/native evidence, but the
matrix does not mention the navigation step tests, the offline region
catalog/install tests, the Android device smoke script or the goal prompt
validator. Readers cannot map those features to evidence.

## Task

Add concise rows/subsections to `docs/TESTING.md`:
- native navigation `next_step` / maneuver-location tests and the route sheet;
- offline region catalog, install and loopback pack serving;
- `scripts/android-smoke.ps1` (install, launch, rendered/ANR check);
- `scripts/goal.ps1 check` (prompt queue validation).

Link the existing commands and files; do not invent tests that are not in the
repo.

## Files

- `docs/TESTING.md` only.

## Steps

1. Read the current Layers table and Running section.
2. Add the rows in the same table shape; keep each line short.
3. Mention the exact source/script path for each row.
4. Check the Markdown table pipes and code formatting.

## Verify

```text
git diff --check
```

Expected: no whitespace errors; the table renders with the same column count and
every linked path exists.

## Done

- `[DOCS] Testing: add navigation, offline and device rows` in `CHANGELOG.md`.
- Branch, issue, PR with `Closes #N`, green CI, squash merge.
- `powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 done -Id 06 -Agent <agent>`
