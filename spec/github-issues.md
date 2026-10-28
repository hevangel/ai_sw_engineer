# GitHub issues and the project board

Read this when you start, pause, hand off or finish work that has a GitHub issue, or when you are asked which issue you are working on. Every task here has an issue in `hevangel/ai_sw_engineer`, tracked as an item in the `ai_engineer` project owned by `hevangel`. The issue and its project item are how humans and other agents see who is working on what.

### Using the GitHub CLI for task updates

Use `gh` to update the existing task issue and its item in the `ai_engineer`
project owned by `hevangel`. Search existing issues before creating a
duplicate, and record the canonical issue URL in the task plan or handoff.

1. Read the issue, acceptance criteria, current Agent and recent activity.
   Coordinate an existing active claim before taking ownership. Inspect live
   project fields and item IDs:

   ```sh
   gh project list --owner hevangel --format json
   gh project field-list PROJECT_NUMBER --owner hevangel --format json
   gh project item-list PROJECT_NUMBER --owner hevangel --limit 1000 --format json
   ```

   Account for pagination; a missing item in a partial listing is not proof
   that it is absent. Resolve option IDs from live definitions rather than
   copying IDs from another issue or a previous chat.
2. Keep implementation issues in `hevangel/ai_hw_engineer`. If misplaced,
   capture their existing project fields and relationships, then transfer with
   `gh issue transfer ISSUE_URL hevangel/ai_hw_engineer`. Record the returned
   URL and re-read the issue and project item IDs. Transfers can remove project
   membership: restore it with `gh project item-add`, then restore captured
   fields and relationships, preserving intentional Sprint commitments.
3. Set fields with `gh project item-edit`, supplying the discovered item,
   project and field IDs. Use `--single-select-option-id` for Agent and Status,
   `--date YYYY-MM-DD` for Start date, and `--text` for Path. Path names the
   owning repository and relative implementation location, for example
   `ai_hw_engineer/design/intel_80286`, without a host drive or worktree path.
   Preserve an evidenced first Start date on resume or transfer; follow the
   lifecycle guide's historical procedure when reconciling older work.
4. Register the working Agent and use Researching for active investigation or
   Building for implementation. Add a concise ownership comment with scope,
   branch/worktree, plan and intended checks where useful. Check recent
   comments to avoid duplicate notices on every resume. Write comment text to
   a temporary file and use `gh issue comment ISSUE_URL --body-file FILE`.
5. Update evidence, blockers and PR links at meaningful checkpoints. Keep
   partial milestones distinct from the issue's full acceptance. Use
   `AI verified` only after inspecting the required scoped evidence;
   `Human verified` requires explicit human approval. Clear Agent with
   `gh project item-edit --id ITEM_ID --project-id PROJECT_ID --field-id AGENT_FIELD_ID --clear`
   when active work stops, finishes or moves to inactive review. Preserve Path,
   the first Start date and other unrelated fields.
6. Read back the saved fields and comments to verify each update. An attempted
   command does not establish success. Record an access blocker and continue
   independent work when an issue update is unavailable.

Check CLI authentication with `gh auth status`. Project edits require the
`project` scope. If missing, ask the user to run
`gh auth refresh -h github.com -s project` and complete browser authorization.
Distinguish network or sandbox access failures from invalid credentials before
asking the user to log in again.
