# Issue tracker taxonomy

Status: active from 2026-11-15 (issue #35). Every open issue follows this
scheme; pull requests still use the commit type flags from `AGENTS.md`.

## 1. Issue type

Every issue has exactly one native GitHub type:

| Type | Use for |
|---|---|
| **Bug** | Something behaves differently from the documented contract. |
| **Feature** | A new user-visible capability or a meaningful extension of one. |
| **Task** | Documentation, build/CI, deployment, release chores, security hardening and other maintenance. |

Set it with `gh issue edit <number> --type Bug|Feature|Task`. The type is
authoritative; labels are secondary filters.

## 2. Type ID in the title

Titles use `PREFIX-n: short imperative summary`, for example
`DOCS-1: Issue taxonomy: types, severity and priority`.

| Prefix | Native type | Use for |
|---|---|---|
| `BUG` | Bug | Defects and regressions. |
| `FEAT` | Feature | User-visible features and product epics. |
| `INFRA` | Task | Build, CI, deployment, releases, dependencies. |
| `DOCS` | Task | Documentation, contribution process, tracker policy. |
| `SEC` | Task | Security hardening that is not a vulnerability report. |
| `TASK` | Task | Other maintenance that fits no prefix above. |

Rules:

- IDs are **per prefix**, increase monotonically and are **never reused**.
- GitHub's `#N` remains the canonical reference for commits, PRs and
  `Closes #N`; the type ID is for humans and filters.
- Closed issues created before 2026-11-15 keep only their `#N`; they are not
  retroactively renumbered.
- Retrofitted open issues: `FEAT-1`–`FEAT-5`, `INFRA-1`–`INFRA-2`, `BUG-1`,
  `DOCS-1`.

Create an issue with the next free ID in one step:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/new-issue.ps1 `
  -Prefix FEAT -Type Feature -Title "Offline map packs" `
  -Severity medium -Priority normal -Label android `
  -Body "Download a bounded area for use without a connection."
```

Use `-DryRun` to print the computed title and command without creating
anything. To assign an ID manually, list the prefix and add one to the largest
number:

```bash
gh issue list --repo Prince-of-Nothing/Pinpoint --state all --limit 300 \
  --search 'in:title "FEAT-"' --json title
```

## 3. Severity: impact if unfixed

| Label | Meaning | Typical action |
|---|---|---|
| `severity:critical` | Safety, privacy or security breach, data loss, no usable public service, or a release blocker. | Stop and fix before the next release. |
| `severity:important` | A major feature is broken or only has an unsafe/painful workaround; significant user impact. | Fix in the current or next release. |
| `severity:medium` | Limited impact, or a reasonable workaround exists. | Schedule normally. |
| `severity:minor` | Cosmetic, documentation or low-impact process issue. | Backlog or opportunistic. |

## 4. Priority: when to work on it

| Label | Meaning |
|---|---|
| `priority:asap` | Drop planned work; act now. Use sparingly. |
| `priority:high` | Current milestone, or the next work item after in-flight work. |
| `priority:normal` | Scheduled; the default for triaged work. |
| `priority:low` | Backlog, opportunistic, or blocked by an external decision. |

Severity and priority are independent: severity answers “how bad is the
impact?”, priority answers “when do we act?”. An issue blocked on the owner
still gets a priority, normally `priority:low` until it is unblocked.

## 5. Labels

Each open issue should have exactly one `severity:*` and one `priority:*`
label. Domain and workflow labels are optional:

| Group | Labels |
|---|---|
| Severity | `severity:critical`, `severity:important`, `severity:medium`, `severity:minor` |
| Priority | `priority:asap`, `priority:high`, `priority:normal`, `priority:low` |
| Kind / topic | `bug`, `enhancement`, `documentation`, `accessibility`, `security`, `dependencies` |
| Domain | `android`, `backend`, `family`, `infra` |
| Workflow | `epic`, `blocked`, `needs:compliance`, `good first issue`, `help wanted`, `skip-issue-link` |
| State | `duplicate`, `invalid`, `question`, `wontfix` |

`bug`, `enhancement` and `documentation` are retained for familiar filtering
even though the native type is authoritative.

Pruned on 2026-11-15: `go` (the Go tree is archived), plus `feature`, `fix`,
`docs` and `release` because they duplicated the native type/PR title flags.

## 6. Triage checklist

For every new issue:

1. Set the native type (`Bug`, `Feature`, or `Task`).
2. Rename the title to `PREFIX-n: summary` with the next free ID.
3. Add one `severity:*` and one `priority:*` label.
4. Add domain labels and `needs:compliance` when it touches privacy,
   consent, security or location data.
5. Add `blocked` when only the owner can unblock it, and state exactly what is
   needed in the body.
6. Link the fixing PR with `Closes #N` (or `Refs #N` for partial work).

Example:

```bash
gh issue edit 34 --type Bug \
  --title "BUG-1: App update not working" \
  --add-label severity:important,priority:high
```

See [`CONTRIBUTING.md`](../CONTRIBUTING.md) for the branch/PR rules and
[`docs/adr/`](./adr/) for architecture decisions.
