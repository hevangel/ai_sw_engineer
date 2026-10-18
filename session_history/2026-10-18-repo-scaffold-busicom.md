# 2026-10-18 — Repository scaffold and Busicom firmware M1

- Captured: 2026-10-18 (UTC); one dashboard conversation, Kiro (Claude) via Kiro CLI.
- Model usage: exact per-model token totals are not exposed to the agent; not recorded.
- Branches: `infra/repo-scaffold` ([PR #1](https://github.com/hevangel/ai_sw_engineer/pull/1)), `system/busicom_141pf` (stacked on it).

## Requests

1. Set up this repo following `../ai_hw_engineer`, for reimplementing historical software in Rust → LLVM IR → per-chip backend → run on the chip simulators. Folders: `reference/`, `system/<system>/<software>/`, `llvm-<chip>/`, `external/` submodules.
2. (mid-session) Add a Docker image for the Rust compiler and this repo's backends.
3. (mid-session) First project: the Busicom firmware in Rust.
4. (mid-session) Add the Rust 4004 emulator to `external/` and use it for the firmware.

## Work done

- Top-level README, AGENTS.md, CONTRIBUTING.md, `.gitignore`/`.gitattributes`; `docs/` (docker, bare-metal Rust, llvm-mos, new-backend guide); `reference/`, `failure_notes/` READMEs.
- `system/_template/` (system README, `platform/` with `target.env`, `link.ld`, `run_image.sh` hook; `software/` crate template with oracle replay test and build/run_host/run_sim/compare/run_all scripts) and `llvm-_template/` (ISA/ABI/asm specs, Rust target JSON, build/test hooks). Script contracts in `system/_template/README.md`.
- `scripts/new_system.sh`, `new_software.sh`, `new_llvm_target.sh`, `check_toolchain.sh`, `docker_run.sh`.
- `Dockerfile` on `mrkits/rust-mos:13f2838f9-334fc98-8f3a80f8` + Rust 1.99.1 + nightly-2026-10-15 (rust-src) + clang/lld/cmake/ninja/lit.
- Shallow submodules: llvm-mos, llvm-mos-sdk, rust-mos, mos6502, martypc, 6502_65C02_functional_tests, busicom, chips (V. Ilmer's Rust MCS-4 emulator and Busicom board).
- `system/busicom_141pf` + `platform/` docs, `llvm-i4004` scaffold, `firmware` title: digit-serial BCD core, keys/switches/lamps, printed rows, add/sub/ST/=/C/CE/SIGN; oracle = 42 manual examples with original-ROM output imported from ai_hw_engineer; ratchet test.

## Validation

- `docker build` succeeded.
- In the container, the generated template title passed fmt, clippy `-D warnings`, oracle replay, host build and host IR; target stage SKIPs until the backend exists.
- Busicom firmware `run_all.sh`: clean; manual replay 7/42 (1-1…1-4, 14-1…14-3) match the original ROM.

## Not done / next

- Firmware M2 (multiply/divide/percent/EX/memory/square root), then M3 drivers checked against `external/chips`, M4 `llvm-i4004`, M5 run on ai_hw_engineer RTL.

## Continuation (2026-10-18, same conversation)

- Merged PR #1 and PR #2 (retargeted #2 to `main`); deleted both feature branches.
- Two sub-agents in separate worktrees: llvm-i4004 ISA/ABI spec ([PR #3](https://github.com/hevangel/ai_sw_engineer/pull/3), data layout `e-m:e-p:16:8-p1:16:8-p2:16:8-i8:8-i16:8-n8-a:8`); Busicom M2 ([PR #4](https://github.com/hevangel/ai_sw_engineer/pull/4)). The M2 agent hit its 100-call limit at 40/42 and was resumed to finish at 42/42.
- Parent fixes on PR #4: `run_all.sh` scripts-dir resolution (title + template), exec bit, `*.env`/`*.ld` LF in `.gitattributes`.
- Validation: `run_all.sh` clean in the container (worktree and git-archive export); manual replay 42/42, 135 checkpoints. Model calls: 3 sub-agent runs; token totals not exposed.
