# AI Coding Agent Guidelines

This document is the entry point for AI coding agents in this repository. It is deliberately short. The detailed rules live in [`spec/`](spec/README.md), one topic per file: **read the files that match your task before you start, and only those.**

## Role

You are the sole author of all code, documentation and configuration here. Humans specify, review and approve; they do not edit files.

## Mission

Build both the Rust/LLVM toolchain for each chip family and a runnable Rust emulator for each complete system, alongside historical software reimplementations. An LLVM backend or a host-only rewrite of a title does not replace the system emulator. The pipeline for each title is: oracle → Rust (`no_std`) → LLVM IR → chip backend → target run, plus a host system emulator in `system/<system>/emulator/`. Details: [spec/mission.md](spec/mission.md).

## Load rules on demand

Open the file when the task matches. Do not read the whole `spec/` folder up front.

| Task | Read |
|---|---|
| Start a title, system or backend; what "done" means | [spec/mission.md](spec/mission.md) |
| Create or find files; run a `new_*.sh` scaffold; write a script | [spec/repository.md](spec/repository.md) |
| Write or change Rust | [spec/rust.md](spec/rust.md) |
| Work under `llvm-<chip>/` | [spec/llvm-backends.md](spec/llvm-backends.md) |
| Capture oracle data, run a simulator, work on `system/<system>/emulator/` | [spec/simulators.md](spec/simulators.md) |
| Handle an original ROM, disk image, listing or manual | [spec/original-software.md](spec/original-software.md) |
| Finish a task; touch a design doc, README or `ASSUMPTION` | [spec/documentation.md](spec/documentation.md) |
| Open a PR or decide a task is done | [spec/verification.md](spec/verification.md) |
| A bug escaped the title's own tests; plan verification | [spec/escaped-defects.md](spec/escaped-defects.md) |
| Commit, branch, add a worktree, write a PR description | [spec/git-workflow.md](spec/git-workflow.md) |
| Start, pause, hand off or finish work that has a GitHub issue; claim an issue; update the `ai_engineer` project | [spec/github-issues.md](spec/github-issues.md) |
| Reverse engineer an original binary (REA); work through a `[Software]` issue step by step; the four-way original/rewrite x emulator matrix | [spec/software-lifecycle.md](spec/software-lifecycle.md) |

## Rules that always apply

These are the short forms; the linked file has the full rule.

- Track every task on its GitHub issue and its `ai_engineer` project item with `gh`: claim it (Agent, Status, Path, Start date) when work starts, clear Agent when work stops ([github-issues](spec/github-issues.md)).
- Never push to `main`; work on a branch, with worktrees under `worktrees/` ([git-workflow](spec/git-workflow.md)).
- Never commit a copyrighted ROM or disk image; record provenance and SHA-256 instead ([original-software](spec/original-software.md)).
- Never edit `external/` in place; it holds pinned third-party submodules ([repository](spec/repository.md)).
- Expected behavior comes from the original binary or manufacturer documentation, never from your own Rust or from the emulator under test ([escaped-defects](spec/escaped-defects.md), [simulators](spec/simulators.md)).
- Fill `spec/` and `plans/` of a title or backend before writing its Rust or TableGen ([repository](spec/repository.md)).
- A change that alters what a design doc says updates that doc in the same change ([documentation](spec/documentation.md)).
- A task is not done until the checklist for the area you touched passes ([verification](spec/verification.md)).

## Maintaining these rules

Edit the topic file, not this one. When you add, rename or remove a file in `spec/`, update the table above and [spec/README.md](spec/README.md) in the same change. Keep this file short enough to be read in full on every task. Do not use `@`-style imports for the `spec/` files: they would load them eagerly and defeat the purpose.
