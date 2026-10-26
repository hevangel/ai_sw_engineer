# llvm-i8086

| | |
|---|---|
| Chip | Intel 8086 / 8088 (same instruction set; 16-bit vs 8-bit external bus). Introduction years (1978 and 1979 as commonly cited) are to be sourced from Intel material before they are stated as fact |
| Word / address width | 16-bit data, 20-bit physical addresses formed from 16-bit segment and offset |
| Strategy | a new LLVM target (`target/lib/Target/I8086`) overlaid on `external/llvm-project`, real-mode 8086 instructions only |
| Rust target | `rust/i8086-none.json` |
| Status | **assembler, disassembler, object writer and integer code generation done** (milestones 2 and 3): `llc` compiles i8/i16 code, calls and frames, verified on the 8088 core; no runtime library, linker or Rust target yet |

## Strategy

No usable 16-bit real-mode backend exists. LLVM's x86 `-m16` mode emits 386+ opcodes and 32-bit operand overrides, which an 8088 cannot execute (AGENTS rule, [spec/llvm-backends.md](../spec/llvm-backends.md)), so this is a new target rather than a configuration of X86. It follows the pattern of `llvm-i8008` and `llvm-i4004`: pseudo-instructions selected over a small allocatable register set, a documented ABI before code, and a runtime library for operations the chip lacks.

The first model is the tiny/small model: one 64 KiB segment (CS = DS = ES = SS), 16-bit near pointers, so the 8086's segment registers are set once at entry and not allocated. Far pointers, multiple segments and the large model are out of scope until a title needs more than 64 KiB.

In scope (done): the machine-code layer: `llvm-mc -triple=i8086` assembles and disassembles the whole 8086 integer instruction set in Intel syntax, relaxes branches, and writes ELF objects with the relocations of `spec/abi.md`; `llvm-objdump` and `llvm-readobj` read them. Verified against a hand-written opcode table and the physical-chip instruction corpus (`tests/`).

In scope (done): `llc -mtriple=i8086` selects integer code at 8 and 16 bits (32/64-bit by the generic legalizer with add/adc chains), native 16-bit multiply and divide, `rep movs`/`stos`, calls and stack frames (`spec/codegen.md`, `spec/abi.md`).

In scope (next): the runtime library (32/64-bit divide and shifts, 64-bit multiply, `memmove`), a linker and image packager, and the `rustc` target through `-Zbuild-std=core`. Out of scope for now: 80186+ instructions, x87, interrupts handlers written in Rust, segmented memory models, DOS `.EXE` relocation format.

## Layout

| Path | Contents |
|---|---|
| [spec/isa.md](spec/isa.md) | instruction set, encodings (8086 subset, 186+ excluded) |
| [spec/abi.md](spec/abi.md) | data layout string, register classes, calling convention, frames, relocations |
| [spec/codegen.md](spec/codegen.md) | how `llc` selects instructions: registers, addressing modes, control flow, arithmetic |
| [spec/asm_syntax.md](spec/asm_syntax.md) | assembler syntax and directives |
| [plans/](plans/implementation_plan.md) | implementation plan, [test plan](plans/testplan.md) |
| `target/` | LLVM `lib/Target/I8086` sources or patches applied onto `external/llvm-project` |
| `rust/` | Rust target JSON, build-std notes, linker wrapper |
| `asm/` | assembler/linker (LLVM MC or standalone) |
| `tests/` | `codegen/` lit+FileCheck, `mc/` encode tests, `smoke/` simulator programs |
| `scripts/` | `build_llvm.sh`, `build_rustc.sh`, `build_image.sh`, `run_tests.sh` |
| [report/](report/final_report.md) | final report |
