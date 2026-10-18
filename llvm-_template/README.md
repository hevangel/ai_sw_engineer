# llvm-[CHIP]

| | |
|---|---|
| Chip | [CHIP] — TODO manufacturer, year (sourced) |
| Word / address width | TODO |
| Strategy | TODO: new LLVM target / llvm-mos / overlay on an existing target |
| Rust target | `rust/[CHIP]-none.json` |
| Status | scaffolded |

## Strategy

TODO: why this approach, what upstream project it builds on (see `external/`), and what is out of scope.

## Layout

| Path | Contents |
|---|---|
| [spec/isa.md](spec/isa.md) | instruction set, encodings, cycle counts (manufacturer source) |
| [spec/abi.md](spec/abi.md) | data layout string, register classes, calling convention, stack, relocations |
| [spec/asm_syntax.md](spec/asm_syntax.md) | assembler syntax and directives |
| [plans/](plans/implementation_plan.md) | implementation plan, [test plan](plans/testplan.md) |
| `target/` | LLVM `lib/Target/<Chip>/` sources or patches applied onto `external/llvm-project` |
| `rust/` | Rust target JSON, build-std notes, linker wrapper |
| `asm/` | assembler/linker (LLVM MC or standalone) |
| `tests/` | `codegen/` lit+FileCheck, `mc/` encode tests, `smoke/` simulator programs |
| `scripts/` | `build_llvm.sh`, `build_rustc.sh`, `build_image.sh`, `run_tests.sh` |
| [report/](report/final_report.md) | final report |
