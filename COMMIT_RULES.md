# Commit rules

These rules apply to every person and every agent that writes to this repository.
Read them before any git write: commit, push, tag or pull request.

## Agent commitment

I, the Claude agent working in this repository, will only use local git commands,
run on the owner's device with the owner's global git config and credentials, to
commit, push or open a pull request. I will not use the GitHub API, a cloud session or
any other cloud write path for any of these, and I will not set a git identity or
add an AI trailer or footer.

## Who commits

- Commits, pushes and pull requests are made with local git on the owner's device,
  using the owner's global git config and credentials. `gh` is fine there when it is
  authenticated as the owner.
- Never set `user.name`, `user.email`, `GIT_AUTHOR_*`, `GIT_COMMITTER_*` or `--author`.
  Commit with a plain `git commit`.
- Never use the GitHub API, a cloud session or any other cloud write path to commit,
  push or open a pull request.

## No AI trailers or footers

- No `Co-authored-by:` lines, for any name, in commit messages or pull request bodies.
- No `Claude-Session:` lines, no "Generated with Claude Code" footers and no other AI
  trailer or footer.
- Enable the repo hooks once per clone: `git config core.hooksPath .githooks`.
  The `commit-msg` hook rejects AI trailers. The `pre-commit` hook rejects a git
  identity that names Claude or anthropic.com.

## Branch and pull requests

- Commit straight to `Beta`, one change per commit with its type flag
  (`[FEATURE]`, `[FIX]`, `[SECURITY]`, `[DOCS]`, `[INFRA]`, `[RELEASE]`).
- No feature branches and no pull request per change.
- At release, open one pull request from `Beta` to `main` and merge it with a merge
  commit, not a squash. Then tag the release commit on `main`.
- Never commit or push to `main` directly.
- Never force-push or rewrite `Beta` or `main` unless the owner asks for it.

## Before pushing

1. `git fetch origin`. Make sure local `Beta` matches `origin/Beta` before you commit.
   If `origin/Beta` has changed under you, stop and check with the owner instead of
   forcing anything.
2. Run the full gate: `cargo fmt --all -- --check`,
   `cargo clippy --all-targets -- -D warnings`, `cargo build --locked` and
   `cargo test --locked`.
3. Check the author and the message of the commit you are about to push:

   ```powershell
   git log -1 --format='%an <%ae> | %cn <%ce>%n%B'
   ```

   The author must be the owner, and the message must carry no trailer from the
   list above.
4. Push with `git push origin Beta`, retrying network errors only.

`AGENTS.md` and `CONTRIBUTING.md` point here. This file sets the commit identity and
trailer rules, and the Beta-only workflow.
