# llvm-i8008

| | |
|---|---|
| Chip | Intel 8008, announced April 1972 (`spec/isa.md`) |
| Word / address width | 8-bit data, 14-bit addresses (16 KiB); 7 registers, 7-level on-chip return stack, no stack pointer |
| Strategy | a new LLVM target (`target/lib/Target/I8008`) overlaid on `external/llvm-project`, Rust through `avr-none` IR, a Python linker |
| Rust target | `rust/i8008-none.json` (layout reference; the route is `rust/retarget_ir.py`) |
| Status | builds and runs four Micral N titles; lit 6/6, RTL execution tests 4650/4650 |

## Strategy

No LLVM backend for the 8008 exists, and the x86 family's is unrelated, so
this is a new target. Instruction selection works on pseudo-instructions
over the four allocatable byte registers (B, C, D, E); A and H:L are the
scratch the expansions need, because the 8008 computes only in A and
reaches memory only through H:L. Frames are static and overlaid by the
linker, which also checks the 7-level call depth (`spec/codegen.md`,
`spec/abi.md`). rustc has no 8008 target, so crates are compiled for
`avr-none` and the IR is retargeted, as for `llvm-i4004`. Out of scope:
interrupts, recursion, floating point.

## Layout

| Path | Contents |
|---|---|
| [spec/isa.md](spec/isa.md) | instruction set, encodings (MCS-8 manual) |
| [spec/abi.md](spec/abi.md) | data layout, registers, calling convention, frames, relocations, runtime |
| [spec/asm_syntax.md](spec/asm_syntax.md) | assembler syntax |
| [spec/codegen.md](spec/codegen.md) | design: route, selection, expansion, tracking, linker |
| [plans/](plans/implementation_plan.md) | implementation plan, [test plan](plans/testplan.md) |
| `target/` | `lib/Target/I8008` sources, applied onto `external/llvm-project` |
| `llvm-include/` | `ELFRelocs/I8008.def` |
| `rust/` | `i8008-none.json`, `retarget_ir.py` |
| `asm/` | `i8008_link.py`, the linker |
| `rt/` | runtime helpers: multiply, divide, shifts, memcpy/memmove/memset |
| `tests/` | `mc/` encodings and disassembly, `codegen/` lit + FileCheck, `smoke/hello` Rust program |
| `scripts/` | `build_llvm.sh`, `build_rust.sh`, `build_image.sh`, `run_tests.sh`, `exec_tests.py` |
| [report/](report/final_report.md) | final report |

## Run

```bash
WORKSPACE=$PWD llvm-i8008/scripts/build_llvm.sh   # llc, llvm-mc into llvm-i8008/install (ai-sw-engineer image)
llvm-i8008/scripts/run_tests.sh                    # lit (ai-sw-engineer image)
python3 llvm-i8008/scripts/exec_tests.py 30        # helpers and compiled IR on the RTL (ai-hw-engineer image)
```
