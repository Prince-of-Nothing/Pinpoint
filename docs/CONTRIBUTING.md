# Contributing to Documentation

This guide covers how to write and maintain docs in [`docs/`](./README.md). It
complements (doesn't replace) any project-wide `CONTRIBUTING.md` for code.

## Where does my change go?

Use this checklist (from [Diátaxis](https://diataxis.fr/)):

| If you're writing... | It goes in... | Because... |
|---|---|---|
| A guided lesson for newcomers | [`tutorials/`](./tutorials) | It's learning-oriented |
| Steps to accomplish one specific task | [`how-to/`](./how-to) | It's task-oriented |
| A factual description of an API, config, or schema | [`reference/`](./reference) | It's information-oriented |
| Design discussion, background, trade-offs on a broad topic | [`explanation/`](./explanation) | It's understanding-oriented |
| A structural description of the system itself | [`architecture/`](./architecture) | arc42 section |
| A single, significant, discrete decision | [`adr/`](./adr) | Immutable decision record |

If content doesn't fit cleanly, prefer splitting it rather than mixing modes in one
page (e.g. don't bury a how-to inside a tutorial).

## Style

- Write in plain, direct language; prefer short sentences and active voice.
- One page = one purpose. Don't mix tutorial/how-to/reference/explanation content.
- Use relative Markdown links between docs so they work both on GitHub/Codeberg and
  in any generated docs site.
- Keep code samples runnable/copy-pasteable; verify they actually work before
  committing.
- Prefer diagrams as text (Mermaid) over binary images so diffs are reviewable.
- Use consistent terminology — check the [glossary]../architecture/glossary.md)
  before introducing a new term for an existing concept.

## Docs-as-code workflow

- Documentation changes are reviewed the same way as code changes (pull/merge
  request + review), not edited out-of-band in a wiki or shared doc.
- When a code change alters behavior, update the relevant docs in the **same**
  PR/MR — don't defer it to "a docs pass later."
- When you make an architecturally significant decision, add an
  [ADR](./adr/README.md) as part of that change, not retroactively.
- Recommended automated checks to add to CI as the project grows (not yet wired
  up in this repo):
  - Markdown lint (formatting/style consistency)
  - Link checker (catch broken internal/external links)
  - Optional: doc build step if a static-site generator is later adopted

## Keeping docs from rotting: audit cadence

Documentation, like code, decays if untended. Schedule a periodic documentation
audit (e.g. every release or every quarter, whichever is more frequent) to:

1. Remove or update docs that no longer match the code (check `reference/` and
   `architecture/` first — they drift fastest).
2. Confirm every `TBD` placeholder in [`architecture/`](./architecture) has either
   been filled in or is tracked as an open item in
   [`architecture/11-risks-and-technical-debt.md`](./architecture/risks-and-technical-debt.md).
   For example, doc rot from unfilled placeholders is tracked as risk **R2** there.
3. Verify the ADR index in
   [`architecture/09-architecture-decisions.md`]../architecture/architecture-decisions-link.md)
   matches the files actually in [`adr/`](./adr).
4. Prune or archive guides that describe removed features.

Record the audit itself (date, what changed) as a short note in the relevant PR —
no separate audit log is needed for a project this size.
