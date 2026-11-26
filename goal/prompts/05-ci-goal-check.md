# 05 — CI: enforce the goal prompt queue

Read `goal/CONTEXT.md` first. Claimed by `<agent>`.

## Context

`goal/README.md` defines the prompt queue and `scripts/goal.ps1 check` validates
it. The check is currently manual, so a malformed or misplaced prompt can land
on `main`. The `rust` job in `.github/workflows/ci.yml` already runs on Ubuntu
with `pwsh` available.

## Task

Add a “Goal queue” step to the `rust` job after the test step:

```yaml
- name: Goal queue
  shell: pwsh
  run: ./scripts/goal.ps1 check
```

The step must fail the job when `check` exits non-zero. Do not change the
prompt files or the validator itself.

## Files

- `.github/workflows/ci.yml` — one step in the `rust` job.

## Steps

1. Read the `rust` job to match indentation and step style.
2. Add the step; keep `shell: pwsh` explicit.
3. Open a PR and confirm the `rust` job runs it on both the PR and a push.

## Verify

```text
pwsh -File scripts/goal.ps1 check
```

Expected: `goal queue OK` and exit 0 locally; the CI `rust` job logs the same.
Then run the host gate from `goal/CONTEXT.md`.

## Done

- `[INFRA] CI: validate the goal prompt queue` in `CHANGELOG.md`.
- Branch, issue, PR with `Closes #N`, green CI, squash merge.
- `powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 done -Id 05 -Agent <agent>`
