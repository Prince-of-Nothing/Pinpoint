# Contributing to Always Together

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
# Backend (Go 1.22+)
cd backend
go build ./...
go vet ./...
go test ./...              # must pass before pushing; keep it that way

# Full stack
docker compose up --build  # http://localhost:8080

# Lint/format
gofmt -l .                 # must print nothing
```

## Branching & commits
- `main` stays deployable; work on feature branches: `feat/…`, `fix/…`, `docs/…`, `chore/…`
- Conventional Commits: `feat(api): add share expiry`, `fix(store): keep email index in sync`
- Rebase rather than merge-commit when possible; keep history readable.

## Pull request checklist
- [ ] `go build`, `go vet`, `go test ./...` pass (CI will re-check)
- [ ] New/changed behavior covered by tests
- [ ] No new lint/gofmt findings
- [ ] Docs updated (`docs/`, README, CHANGELOG "Unreleased")
- [ ] Breaking API changes flagged in the PR title with `!` and migration note
- [ ] For security-sensitive code: threat considered — what happens on IDOR, replay,
      revoked consent mid-session, expired token? Answer in the PR description.

## Code style
- Go: standard `gofmt`; prefer explicit errors (`errors.New`/`fmt.Errorf` with `%w` wrapping);
  no global mutable state outside `internal/store`; handlers stay thin, logic in store/security.
- JS (web clients): ES modules, no build step, no dependencies unless justified in the PR.
- Naming mirrors the domain: `SharePermission`, `precision`, `expiry` — see `docs/GLOSSARY.md`.

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
