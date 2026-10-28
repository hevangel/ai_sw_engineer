# Repository structure and scaffolding

Read this before creating, moving or looking for files, and before running a scaffolding script.

## Repository Structure

```
ai_sw_engineer/
├── AGENTS.md                  — Agent entry point (always loaded; links into spec/)
├── spec/                      — Agent rules, one topic per file (loaded on demand; see spec/README.md)
├── reference/                 — Shared reference docs (+ README with source URL and SHA-256 per file)
├── docs/                      — Tool docs for the agent (one subfolder per tool)
├── system/
│   ├── README.md              — System index (system, year, CPU, software titles)
│   ├── _template/             — platform/, emulator/ and software/ templates
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

The repository-level `spec/` holds the rules for the agent. It is not the per-title or per-backend `spec/` folders described below, which hold behavior and design specs.

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

### System emulator folder (`system/<system>/emulator/`)

```
emulator/
├── README.md        — what is emulated, run/test commands, status
├── Cargo.toml       — host crate (`std` allowed); lib `system_emulator`, bin `<system>-emulator`
├── spec/            — spec.md: machine contract, chip cores and pins, I/O format,
│                      unsupported hardware, assumption ledger
├── plans/           — testplan.md: unit, replay, original-image, generated-image, cross-check
├── oracle/          — transcripts of the original software on the primary oracle (committed)
├── src/
│   ├── lib.rs       — Emulator: new(image), reset(), replay(input) -> observed output
│   └── main.rs      — CLI: <system>-emulator <image> <input>; nonzero exit on invalid input
├── tests/           — replay.rs: error paths, reset, oracle replay, ignored original-image test
└── report/          — verification.md: dated record of commands, results and what was not run
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

- Platform and emulator: `./scripts/new_system.sh <system> <chip>` (copies `system/_template/platform/` and `system/_template/emulator/`)
- Software: `./scripts/new_software.sh <system> <software>`
- Backend: `./scripts/new_llvm_target.sh <chip>`

Fill `spec/` and `plans/` before writing Rust or TableGen. For a new system, fill `emulator/spec/spec.md` (chip cores, machine contract, unsupported hardware) before replacing the template's placeholder emulator core.

## Scripts

- Bash, `set -euo pipefail`, clear status lines, nonzero on failure. Shell scripts are LF (`.gitattributes`).
