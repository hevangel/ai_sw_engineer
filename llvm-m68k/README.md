# llvm-m68k

| | |
|---|---|
| Chip | Motorola MC68000 (1979) and the 68k family line — Macintosh, Amiga, Atari ST, early Unix workstations |
| Word / address width | 16/32-bit implementation, 32-bit registers, **big-endian**, 24-bit (16 MiB) original address space |
| Strategy | **reuse upstream**: the LLVM M68k backend is compiled into this repo's pinned rustc (1.99.1 and nightly-2026-10-15 both carry `m68k-unknown-linux-gnu` + `m68k-unknown-none-elf` and working M68k CodeGen — verified by compiling `core`); qualification adds the project-owned pieces the tier-3 target lacks (a bare-metal linker setup and an executor) |
| Rust target | `m68k-unknown-none-elf` (tier 3, shipped in the pinned rustc; bare-metal link needs `m68k-linux-gnu-ld` or a custom target JSON — see the audit's caveat) |
| Status | **2026-10-27 (issue ai_engineer#64): NOT qualified — two blocking defects found by the gates.** The compiler works for straight-line code (8/8 hand-verifiable ALU/shift/multiply slots byte-identical to the oracle on the independent r68k executor), but (1) **LLVM M68k miscompiles unsigned compare-select at every optimization level ≥ 1** — a flag-clobbering `move.l` is scheduled between the compare `sub.l` and the `bhi` (MOVE clears C/V on m68k), wrong on real hardware; and (2) **r68k 0.2.2 misexecutes the architecturally-correct −O0 sequence** (isolated to the compare/select/MOVEM path). Minimal reproducers, disassembly and the full evidence chain: [report/qualification.md](report/qualification.md). Issue #64 stays open (Blocked on upstream + executor fixes). |

## Layout

| Path | Contents |
|---|---|
| [spec/audit_upstream.md](spec/audit_upstream.md) | upstream survey: LLVM M68k, the Rust targets, binutils, executors; licenses; the linker caveat |
| [spec/compiler_profile.md](spec/compiler_profile.md) | MC68000 compiler contract: variant pinning, ABI, data layout, runtime, packaging |
| [rust/smoke/](rust/smoke/) | memory-driven `no_std` smoke program (nothing constant-foldable) |
| [scripts/build.sh](scripts/build.sh) | pinned build: `cargo` (m68k-unknown-none-elf, `-Z build-std=core`, `target-cpu=M68000`) → ELF + flat image + symbols + hash |
| [scripts/expected.py](scripts/expected.py) | the oracle: smoke semantics re-implemented in Python (never derived from the compiled image or an emulator) |
| [tools/r68k_run/](tools/r68k_run/) | executor: runs the image on r68k 0.2.2 (MIT; Musashi-ported, property-tested against Musashi) until the sentinel, prints the results window |
| [scripts/exec_r68k.sh](scripts/exec_r68k.sh) | executes + compares the window vs the oracle |
| [scripts/disasm_check.py](scripts/disasm_check.py) | independent opcode verification: GNU objdump decodes all of `.text` (M68000 mode), LLVM objdump cross-checks byte counts, 68010+ mnemonics banned |
| [report/qualification.md](report/qualification.md) | dated record: versions, commands, hashes, results, limits |

## Reproduce (inside the ai-sw-engineer container, repo at /workspace)

```sh
apt-get update -qq && apt-get install -y -qq binutils-m68k-linux-gnu   # until the image rebuild lands (see Dockerfile)
llvm-m68k/scripts/build.sh
llvm-m68k/scripts/exec_r68k.sh
llvm-m68k/scripts/disasm_check.py
```

## Executor note

The ai_hw_engineer MC68000 RTL is a work-in-progress skeleton (SingleStepTests
harness ready, CPU increments landing), so this qualification executes on
r68k — the Musashi-ported, MIT-licensed Rust core — and records the RTL as
the future primary executor in the report's limits.
