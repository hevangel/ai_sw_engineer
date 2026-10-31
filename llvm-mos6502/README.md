# llvm-mos6502

| | |
|---|---|
| Chip | MOS 6502 (1975, MOS Technology / Chuck Peddle) |
| Word / address width | 8-bit word, 16-bit address space (64 KiB addressable) |
| Strategy | Existing LLVM backend: `llvm-mos` + `rust-mos` fork |
| Rust target | `rust/mos6502-none.json` (also built-in `mos-unknown-none` / platform configs) |
| Status | Operational (LLVM-MOS Clang 20.0, rust-mos 1.87-dev, compiler-builtins mos-0.1.148) |

## Strategy

Per `AGENTS.md` ("Prefer existing backends: llvm-mos for the 6502 family"), this repository uses the mature upstream `llvm-mos` and `rust-mos` ecosystem pinned in the Docker toolchain image (`mrkits/rust-mos`).

Chips sharing this backend:
- MOS 6502 (Apple II, Commodore PET, BBC Micro)
- MOS 6507 (Atari 2600 VCS)
- MOS 6510 (Commodore 64)
- Ricoh 2A03 (NES / Famicom, minus decimal mode)

## Layout

| Path | Contents |
|---|---|
| [spec/isa.md](spec/isa.md) | Instruction set, encodings, cycle counts (MOS Technology 1975) |
| [spec/abi.md](spec/abi.md) | Data layout string, register classes, calling convention, zero-page imaginary registers |
| [spec/asm_syntax.md](spec/asm_syntax.md) | Assembler syntax and directives |
| [plans/](plans/implementation_plan.md) | Implementation plan, [test plan](plans/testplan.md) |
| `rust/` | Rust target JSON (`mos6502-none.json`), build-std notes, linker wrapper |
| `tests/` | Lit codegen tests, simulator smoke programs (`mos-sim`) |
| [report/](report/final_report.md) | Final report |
