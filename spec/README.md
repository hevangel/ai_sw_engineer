# Agent rules (`spec/`)

The rules for AI coding agents in this repository, one topic per file. [`AGENTS.md`](../AGENTS.md) is the short entry point that is always loaded; it links here. An agent reads only the files that match the task at hand, so the context stays small. Humans can read the same files as the project's working agreement.

These are repository-level rules. They are not the per-title or per-backend `spec/` folders (`system/<system>/<software>/spec/`, `llvm-<chip>/spec/`), which hold behavior and design specs.

| File | Read it when you are... |
|---|---|
| [mission.md](mission.md) | starting a title, system or backend; unsure what "done" means |
| [repository.md](repository.md) | creating or finding files; running `new_system.sh`, `new_software.sh` or `new_llvm_target.sh`; writing a script |
| [rust.md](rust.md) | writing or changing Rust (titles or system emulators) |
| [llvm-backends.md](llvm-backends.md) | touching anything under `llvm-<chip>/` |
| [simulators.md](simulators.md) | capturing oracle data, running an image on a simulator, or working on `system/<system>/emulator/` |
| [original-software.md](original-software.md) | fetching, storing or citing an original ROM, disk image, listing or manual |
| [documentation.md](documentation.md) | finishing a task; changing a design doc, a README or an `ASSUMPTION` |
| [verification.md](verification.md) | opening a PR or deciding a task is done |
| [escaped-defects.md](escaped-defects.md) | a bug surfaced outside the title's own tests; planning verification |
| [git-workflow.md](git-workflow.md) | committing, branching, adding a worktree, writing a PR description |
| [github-issues.md](github-issues.md) | starting, pausing, handing off or finishing work that has a GitHub issue; claiming an issue; updating the `ai_engineer` project with `gh` |

Keep each file to one topic and short enough to read in full. When a rule moves or a file is added, update this table and the index in `AGENTS.md` in the same change.
