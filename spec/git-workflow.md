# Commits, branches, worktrees and PRs

Read this before committing, creating a branch or worktree, or opening a PR.

## Commit Messages

```
<area>: <short description>
```

Areas: `system/<system>/<software>`, `system/<system>/platform`, `system/<system>/emulator`, `llvm/<chip>`, `external`, `docs/<tool>`, `reference`, `scripts`, `infra`.

## Branches and PRs

- `main` is protected; never push to it directly.
- Branches: `system/<name>`, `llvm/<chip>`, `docs/<topic>`, `fix/<desc>`, `infra/<desc>`.
- Worktrees: every `git worktree add` goes under `worktrees/` in the repo root (e.g. `git worktree add worktrees/<name> <branch>`). Never create one as a sibling of the repo. `worktrees/` is gitignored.
- PR description: What / Why / Verification / Agent.
