# AI Coding Agent Guidelines

This document defines how AI coding agents work in this repository.

## Role

You are the sole author of all code, documentation and configuration here. Humans specify, review and approve; they do not edit files.

## Mission

Build both the Rust/LLVM toolchain for each chip family and a runnable Rust
emulator for each complete system, alongside historical software reimplementations.
An LLVM backend or a host-only rewrite of a title does not replace the system
emulator. Reimplement historical software in Rust and run it on the original chip:

1. **Oracle** — run the original binary in a chip simulator and capture its behavior.
2. **Rust** — reimplement it as a `no_std` crate; prove it on the host against the oracle.
3. **LLVM IR** — `rustc --emit=llvm-ir` (or `cargo rustc -- --emit=llvm-ir`).
4. **Backend** — lower IR to the target ISA with `llvm-<chip>/` (llc → asm → assembler → linker).
5. **Target run** — run the image on the chip simulator and compare against the same oracle.
6. **System emulator** — maintain `system/<system>/emulator/`, a host Rust
   library and executable that run original and compiled machine-code images
   through the CPU, memory and peripheral models. Include keyboard/input,
   observable output, deterministic replay, specifications, tests and run commands.
   Reuse pinned chip cores where appropriate; keep system wiring and behavior in
   this repository and record dependencies and unsupported hardware explicitly.

## Repository Structure

```
ai_sw_engineer/
├── reference/                 — Shared reference docs (+ README with source URL and SHA-256 per file)
├── docs/                      — Tool docs for the agent (one subfolder per tool)
├── system/
│   ├── README.md              — System index (system, year, CPU, software titles)
│   ├── _template/             — platform/ and software/ templates
│   └── <system>/              — e.g. busicom_141pf, micral_n, apple_ii, apple_iie,
│       ├── README.md            ibm_pc, atari_2600, c64, nes
│       ├── platform/          — memory map, I/O, boot, linker script, runtime (crt0, HAL)
│       ├── emulator/          — host Rust whole-system library, executable, spec and tests
│       └── <software>/        — one folder per title (see below)
├── llvm-_template/            — Backend template
├── llvm-<chip>/               — i4004, i8008, mos6502, i8086 ...
├── external/                  — Git submodules (third-party; never edit in place)
├── scripts/                   — Project utilities
├── failure_notes/             — Escaped-defect post-mortems
└── session_history/           — Session logs
```

### Software folder (`system/<system>/<software>/`)

```
<software>/
├── README.md        — history, year, original authors, sources, status, links
├── Cargo.toml       — the crate (lib = portable logic, bin = target entry)
├── spec/            — behavior spec derived from reference material (spec.md, io.md)
├── original/        — provenance.md + SHA-256 of the original binary (binary itself gitignored
│                      unless it is freely redistributable)
├── oracle/          — captured oracle data: transcripts, traces, expected outputs (committed)
├── src/
│   ├── lib.rs       — no_std core logic, no target assumptions
│   ├── hal.rs       — trait for the machine's I/O (keyboard, display, printer, ...)
│   └── main.rs      — #![no_std] #![no_main] target entry using system/<system>/platform
├── tests/           — host tests: replay oracle transcripts through lib + a mock HAL
├── plans/           — implementation_plan.md, testplan.md
├── scripts/         — build.sh, run_host.sh, run_sim.sh, compare.sh, run_all.sh
├── bin/             — committed, signed-off target images (small ROMs only) + .sha256
├── build/           — generated: .ll, .s, .o, .elf, .map, raw images (gitignored)
└── report/          — final_report.md: size vs ROM budget, pass/fail vs oracle, limits
```

### Backend folder (`llvm-<chip>/`)

```
llvm-<chip>/
├── README.md        — chip, year, ISA summary, backend strategy, status
├── spec/            — isa.md, abi.md (calling convention, data layout), asm_syntax.md,
│                      codegen.md (design doc: lowering, frames, calls, linker)
├── plans/           — implementation_plan.md, testplan.md
├── target/          — LLVM Target sources (lib/Target/<Chip>/*.td, *.cpp) applied onto
│                      external/llvm-project, or an overlay/patch set for llvm-mos
├── rust/            — <chip>-none.json Rust target spec, build-std notes, linker wrapper
├── asm/             — assembler/linker (LLVM MC integrated, or a standalone Rust tool)
├── tests/           — lit/FileCheck codegen tests, MC encode tests, simulator smoke programs
├── scripts/         — build_llvm.sh, build_rustc.sh, run_tests.sh
└── report/          — final_report.md
```

Chips sharing an ISA share one backend: 6502/6507/6510/2A03 all use `llvm-mos6502` (llvm-mos), 8086/8088 use `llvm-i8086`.

## Creating New Work

- Platform: `./scripts/new_system.sh <system> <chip>`
- Software: `./scripts/new_software.sh <system> <software>`
- Backend: `./scripts/new_llvm_target.sh <chip>`

Fill `spec/` and `plans/` before writing Rust or TableGen.

## Conventions

### Rust
- Edition 2021, `#![no_std]` for every crate that reaches a target; `alloc` only when the platform provides a heap.
- Portable logic in `lib.rs` behind a HAL trait; the host test and the target entry both implement the HAL.
- No floating point, no `u64`/`u128` on 4/8-bit targets unless the original used multi-precision math; prefer the widths the original used (BCD nibbles, 8-bit registers).
- Size matters: build target images with `opt-level = "z"`, `panic = "abort"`, `lto = true`, `codegen-units = 1`; report the size against the ROM budget.
- `cargo fmt` and `cargo clippy -- -D warnings` clean on the host build.

### LLVM backends
- Prefer existing backends: llvm-mos for the 6502 family. Only write a new backend where none exists (4004, 8008, real-mode 8086 — LLVM's x86 `-m16` emits 386+ opcodes and is not acceptable for an 8088).
- Each backend records its data layout and calling convention in `spec/abi.md` before code is written; the Rust target JSON must match it exactly.
- Every instruction selection pattern gets a lit/FileCheck test; every MC encoding gets an encode test checked against the manufacturer opcode table.
- Once a backend compiles and links a real program, `spec/codegen.md` (design doc) and `spec/abi.md` describe the backend as it is, not as it was planned. A change to lowering or pseudo expansion, the calling convention or saved registers, frames, call strategy, the linker/packer, or a runtime routine's contract updates them in the same change. A superseded design is marked frozen or historical with a date, not deleted. If the two docs disagree with the code, the docs are the defect.

### Simulators
- Deliver a Rust system emulator for every implemented system in `system/<system>/emulator/`, in addition to the per-chip LLVM/Rust backend and target software. Host emulator crates may use `std`; the target `no_std` rule does not apply to them.
- Run both original and generated images, expose real machine I/O, and replay deterministic inputs against captured original-software output. Document hardware variants, timing approximations and gaps in `emulator/spec/spec.md` and `emulator/plans/testplan.md` before implementation, and keep them current.
- An emulator sharing a chip core with a reference adapter is not an independent oracle. Cross-check against RTL or another independent implementation; never derive expected output from the emulator under test.
- Primary oracle: the RTL and ISS in `../ai_hw_engineer` (`$AI_HW_ENGINEER`). Use its `system/<system>/` builds for whole-machine runs.
- Secondary oracle: an independent third-party emulator in `external/` (MAME, AppleWin, Stella, MartyPC, ...). When the two disagree, the disagreement is a finding, not noise.

### Original software and copyright
- Never commit a copyrighted ROM/disk image. Record source URL, SHA-256 and license status in `original/provenance.md`; fetch scripts download into `original/` (gitignored).
- Oracle transcripts (inputs and observable outputs) are committed.

### Documentation
- Markdown everywhere. Maintain `system/README.md` (system index) and `llvm-*/README.md`.
- Every software README states the original's year, authors, sources and the current status. Historical dates are sourced; uncertainty is stated, not guessed.
- **Design docs stay current.** Status docs (README status, `report/`, `size_history.md`, plan tables) record results. Design docs record how the thing works, and they go stale silently unless someone keeps them current. The design docs are:
  - **Backend:** `spec/codegen.md`, `spec/abi.md`.
  - **Title:** `spec/spec.md` (behavior and the assumption ledger), `spec/io.md` (I/O contract and drivers), `plans/testplan.md`, and the README layout table and run commands.
  - **Platform:** `memory_map.md`, `io_map.md`.

  A change that alters what one of them says updates it in the same change: a new data layout or RAM map, a driver moved to assembly, a new test level, a new script, a calling-convention change.
- Every `ASSUMPTION` in code cites a ledger ID (`ASSUMPTION (spec M3-K2): …`), and that ID exists in `spec/spec.md` with the same meaning. A new assumption gets a new ID; an ID is never reused for something else.
- A superseded design is marked frozen or historical with a date, not deleted. If a design doc disagrees with the code, the doc is the defect: fix it when you find it, even outside your task.
- Before finishing a task, check the design docs for the area you changed. A status update alone (README line, report) does not count as updating the docs.

### Scripts
- Bash, `set -euo pipefail`, clear status lines, nonzero on failure. Shell scripts are LF (`.gitattributes`).

## Verification Before PR

- [ ] Host: `cargo test` passes, oracle transcripts replay exactly
- [ ] System emulator changes: host tests, format and clippy pass; original-image replay matches committed oracle output (or the gap is documented), with commands and dependency/core provenance recorded
- [ ] `cargo fmt --check`, `cargo clippy -D warnings`
- [ ] Target image builds; size reported against the ROM/RAM budget
- [ ] Target image run on a simulator and compared against the oracle (or the gap is documented)
- [ ] Backend changes: lit + MC tests pass
- [ ] Backend changes: `spec/codegen.md` and `spec/abi.md` match the code (design changes documented in the same PR)
- [ ] Size scoreboard: a title whose target image is not yet signed off adds a row to `report/size_history.md` (date, commit, `.text`, image, largest function, call/libcall counts, host and target status) for every landing that touches it or its backend, measured the same way as the previous rows
- [ ] README / report updated
- [ ] Design docs for every touched area match the code (title `spec/`, `testplan.md`, README layout; platform `memory_map.md`/`io_map.md`); every `ASSUMPTION` ID in the diff exists in `spec/spec.md`

## Learning From Escaped Defects

A bug found by anything other than the title's own tests (the original program's behavior, a second emulator, a reviewer) is an escaped defect: write a post-mortem in `failure_notes/`.

- **Oracle from the original**: expected behavior comes from running the original binary or from the manufacturer's documentation, never from re-reading your own Rust.
- **Two-stage check**: host pass + target pass are both required; a host pass with a target fail points at the backend first.
- **Assumption ledger**: behavior the documentation does not state gets an `ASSUMPTION:` comment in code and an entry in `spec/spec.md`.
- **Whole-program size gate from the first function that lowers**: link-and-measure the whole title against the ROM budget (per-function `.text` sum and largest function) in the backend's test runner, long before linker or relaxation polish. Component tests being green says nothing about whether the whole fits.

## Commit Messages

```
<area>: <short description>
```

Areas: `system/<system>/<software>`, `system/<system>/platform`, `llvm/<chip>`, `external`, `docs/<tool>`, `reference`, `scripts`, `infra`.

## Branches and PRs

- `main` is protected; never push to it directly.
- Branches: `system/<name>`, `llvm/<chip>`, `docs/<topic>`, `fix/<desc>`, `infra/<desc>`.
- Worktrees: every `git worktree add` goes under `worktrees/` in the repo root (e.g. `git worktree add worktrees/<name> <branch>`). Never create one as a sibling of the repo. `worktrees/` is gitignored.
- PR description: What / Why / Verification / Agent.
