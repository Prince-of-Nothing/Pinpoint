# Issue & PR Contribution Templates

Drop-in templates for GitHub-style issue/PR forms. Copy the relevant block into a new
issue/PR (or wire these into `.github/ISSUE_TEMPLATE/` when CI lands).

---

## 🐛 Bug report template
```markdown
**Summary**
<!-- One sentence. -->

**Severity** ☐ low ☐ medium ☐ high ☐ critical (security)
> If security-related, DO NOT file publicly — see docs/SECURITY.md private reporting.

**Environment**
- Deployment: ☐ docker compose ☐ single binary (WEB_ROOT) ☐ local go run
- Commit/version:
- Client: ☐ web ☐ mobile ☐ API directly

**Steps to reproduce**
1. …

**Expected vs actual**
- Expected:
- Actual:

**Logs / evidence**
<!-- Redact anything resembling real user data. Demo accounts only. -->
```

## ✨ Feature request template
```markdown
**User story**
As a …, I want …, so that …

**MVP or extension?** ☐ MVP requirement # ___ ☐ extension phase
If extension: link the required threat/privacy analysis (docs/ gate in ROADMAP).

**Consent impact**
Does this touch who can see whose location? If yes, describe how the server enforces
directional consent for it (and what fails closed).

**Test evidence plan**
Which entries of docs/TESTING.md's matrix cover this? New tests needed:
- [ ] unit ☐ handler/authz ☐ security case ☐ e2e
```

## 🔖 Pull request template
```markdown
**What & why**
Closes #___

**Checklist** (see CONTRIBUTING.md)
- [ ] `go build ./... && go vet ./... && go test ./...` pass locally
- [ ] Behavior covered by tests (cite test function names)
- [ ] `gofmt -l .` clean
- [ ] Docs updated (README/docs table, CHANGELOG "Unreleased")
- [ ] Breaking API change? ☐ no ☐ yes → migration note below
- [ ] Security-sensitive? → answered in-line: IDOR / replay / revoked-consent-mid-session /
      expired-token behavior:

**Review focus suggestions**
<!-- Where should reviewers look hardest? -->
```

## 📅 RFC template (larger changes)
```markdown
Title: RFC: <summary>

1. Problem & context
2. Proposed approach (data model changes? endpoint changes?)
3. Alternatives considered
4. Security & privacy implications (threat model delta)
5. Migration / compatibility
6. Test plan
7. Docs to update
Decision: ☐ approved by maintainer ☐ rejected ☐ deferred (date, reason)
```

## Label conventions
- Type: `bug`, `enhancement`, `docs`, `test`, `infra`, `security⚠`
- Area: `backend`, `web`, `mobile`, `weba`, `webk`, `deployment`
- Thesis: `mvp-req-N`, `extension-phase`, `needs-threat-analysis`
- Status: `good-first-issue`, `in-progress`, `blocked`
