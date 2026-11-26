# `/goal` — agent prompt queue

This folder is the single entry point for delegated work. Every agent prompt is
a small Markdown file in `goal/prompts/`; there are no ad-hoc prompts outside
this folder. A prompt is consumed by exactly one agent, then deleted after its
PR merges.

## Layout

```text
goal/
  README.md        this protocol
  CONTEXT.md       shared project capsule every prompt points to
  TEMPLATE.md      copy this for a new prompt
  prompts/         pending, numbered prompts (01..NN)
  claimed/         local claim locks, gitignored, never committed
```

The initial queue has ten prompts, one per agent. Each prompt is one
deliverable, one branch, one issue and one PR. Prompts name the files they may
touch so the ten agents do not collide.

## Swarm protocol (10 agents)

Each agent runs independently:

1. `powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 list`
2. Pick the lowest-numbered pending prompt that fits its role, then:
   `powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 claim -Id 03 -Agent agent-c`
   This **moves** the prompt into `goal/claimed/agent-c/`, so no other agent can
   claim it. The command prints the new path.
3. Read `goal/CONTEXT.md`, then the claimed prompt. Follow it exactly.
4. Do the work through the branch → issue → PR → gate → squash-merge flow in
   `CONTEXT.md`.
5. After the PR merges:
   `powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 done -Id 03 -Agent agent-c`
   This deletes the claimed prompt. Git history is the archive.

If an agent stops early, restore its prompt:
`powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 release -Id 03 -Agent agent-c`.

Do not edit a claimed prompt. Do not copy a prompt out of `goal/` and hand it to
an agent by chat; the file is the contract.

## Validate the queue

`powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 check`

The check fails if a prompt is outside `prompts/`, has a bad filename, is
missing one of the required sections, or exceeds the size cap. It also reports
claimed prompts so a forgotten claim is visible. Run it after adding prompts and
in CI if a job is available.

## Prompt quality rules

A prompt is good when a fresh agent can finish it without asking questions:

- **One deliverable.** No “also refactor X” lists.
- **Context first.** Link `goal/CONTEXT.md`; add only the local facts the agent
  cannot infer. Name the exact files and the existing helpers/patterns to reuse.
- **Bounded.** Target ≤ 60 lines; the validator enforces 120 as a hard cap.
- **Verifiable.** Give the exact test or command that proves it, and the
  expected result. “Looks right” is not acceptance.
- **Safe.** No secrets, real users, real coordinates or private data in
  prompts, tests or fixtures.
- **Independent.** Avoid files another pending prompt owns. If two prompts must
  share a file, make one depend on the other and say so.
- **Deletable.** End with the exact `goal.ps1 done` command.

Use `TEMPLATE.md` and keep the section headings; `scripts/goal.ps1 check` reads
them.

## Adding work

1. Copy `TEMPLATE.md` to `goal/prompts/NN-kebab-title.md`, using the next free
   number.
2. Fill every section; keep it small enough for one PR.
3. Run `scripts/goal.ps1 check`.
4. Commit the new prompt on a `docs/goal-*` branch with an issue/PR, or add it
   to the current prompt PR if it is part of the same queue change.

The queue is intentionally finite. When it is empty, add a new batch of
prompts; do not resurrect deleted files.
