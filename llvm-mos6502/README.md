# llvm-mos6502

| | |
|---|---|
| Chip | MOS 6502 (NMOS, 1975) — CPUs of the Apple II, Commodore PET/C64, Atari 2600 (6507), NES (2A03), KIM-1 |
| Strategy | **reuse upstream `llvm-mos` + `rust-mos`** (pinned); project-owned bare-metal runtime, linker script and qualification ([spec/audit_upstream.md](spec/audit_upstream.md)) |
| Rust target | `mos-unknown-none` (tier 3, shipped by the pinned `rustc +mos` 1.87.0-dev) |
| Status | **2026-10-26 (issue ai_engineer#58): qualified.** A representative `no_std` Rust program compiled with the pinned toolchain to a flat 32 KiB ROM image (SHA-256 `1b471700…`, bit-reproducible across builds) executes **identically on two independent 6502 emulators** (vendored py65 and external/mos6502 — same 2,510-instruction step count, same final registers), and its 378 `.text` bytes walk as documented NMOS opcodes in the independent py65 decoder. See [report/qualification.md](report/qualification.md). |

## Layout

| Path | Contents |
|---|---|
| [spec/audit_upstream.md](spec/audit_upstream.md) | upstream survey, pins, licenses, known image skew, variant policy |
| [spec/link.ld](spec/link.ld) | self-contained bare-metal linker script (imaginary regs at $02, RAM $0200, ROM $8000, vectors $FFFA) |
| [rt/crt0.s](rt/crt0.s) | reset runtime: S=0xFF, `cld`, zero .bss/zp_bss, copy .data/zp_data from ROM, `jsr main`, park; NMI/IRQ default trap |
| [rust/smoke/](rust/smoke/) | representative `no_std` smoke program (ALU/shift/rotate/compare/16-bit patterns into a fixed window) |
| [scripts/build.sh](scripts/build.sh) | container build: llvm-mc crt0 → `cargo +mos` (mos-unknown-none, `-Z build-std=core`) → flat image + symbols + hash |
| [scripts/expected.py](scripts/expected.py) | the independent oracle: smoke semantics re-implemented in Python (never derived from the compiled image or an emulator) |
| [scripts/exec_py65.py](scripts/exec_py65.py) | executes the image on vendored py65 and compares the window vs the oracle |
| [scripts/exec_mos6502.sh](scripts/exec_mos6502.sh) | executes the image on external/mos6502 (via [tools/mos6502_run](tools/mos6502_run/)) and compares vs the oracle |
| [scripts/disasm_check.py](scripts/disasm_check.py) | independent opcode verification with the py65 table (documented opcodes only, RESET vector, required instruction kinds) |
| [report/qualification.md](report/qualification.md) | dated record: versions, commands, hashes, results, limits |

## Reproduce (inside the ai-sw-engineer container, repo mounted at /workspace)

```sh
llvm-mos6502/scripts/build.sh          # crt0 + cargo +mos + flat image + hash
llvm-mos6502/scripts/exec_py65.py      # independent emulator #1 (ai_hw oracle)
llvm-mos6502/scripts/exec_mos6502.sh   # independent emulator #2 (mre/mos6502)
llvm-mos6502/scripts/disasm_check.py   # independent opcode verification
```

The consumer machines (KIM-1, Apple II, Atari 2600, NES, C64) add their
per-machine profiles and budgets on top of this base qualification.
