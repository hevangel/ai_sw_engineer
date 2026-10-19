# llvm-i4004

| | |
|---|---|
| Chip | Intel 4004 — announced November 15, 1971 in *Electronic News* (see ai_hw_engineer `design/README.md` for sources) |
| Word / address width | 4-bit data, 8-bit instructions (1–2 bytes), 12-bit ROM address |
| Strategy | new experimental LLVM target on `llvm/llvm-project` (no existing backend) |
| Rust target | `rust/i4004-none.json` — data-layout `e-m:e-p:16:8-p1:16:8-p2:16:8-i8:8-i16:8-n8-a:8` |
| Status | **milestone 3b in progress — CodeGen backbone built**. `llc -mtriple=i4004` lowers IR to real MCS-4 asm our `llvm-mc` accepts: `ret`, i8 `add`/`sub` (ACC-centric nibble sequences with carry), i8 args (P1–P3), pair copies, unconditional `br` (`JUN`), i16 return split, and ELF `e_machine 0x4004` objects. **22/22** lit (16 MC + 6 CodeGen) and **20/20** execution checks on `external/chips` (`scripts/exec_fn.py` runs `llc`-compiled add/sub on the real `mcs4::Board` — this caught a real subtract borrow-chain bug). Still open before the firmware compiles: loads/stores to 4002 RAM (`SRC`+`RDM`/`WRM`), `icmp`/conditional branches (`SETCC`/`BR_CC`), i16/i32 expansion, `libi4004rt` mul/div/shift, deep calls, a linker/image packer. Milestone 3a (MC + disassembler, differential cross-check on chips) remains complete. See [report](report/final_report.md). |

## Strategy

There is no LLVM backend for the 4004, and it is a hostile target for a C-like IR: 4-bit accumulator, sixteen 4-bit index registers, a 3-level return-address stack, no data memory addressing except `SRC` + `RDM`/`WRM` on 4002 characters, and ROM-only code. The plan, to be fixed in `spec/abi.md` before any TableGen:

- legal types `i4` (native) and `i8` (register pairs); everything wider is expanded;
- data pointers name 4002 characters (bank, chip, register, character); an address space split between RAM data, RAM status characters and ROM constants (`FIM`/`FIN`);
- calls beyond depth 3 use a software return stack in 4002 RAM, or are inlined (the Busicom core is written to keep the call graph shallow);
- objects in ELF with a private `e_machine`, linked by `ld.lld` or by a small image packer in `asm/` that lays out 256-byte 4001 pages (jumps and `FIN` are page-relative).

The first consumer is `system/busicom_141pf/firmware`; its 1280-byte budget is the yardstick.

## Layout

| Path | Contents |
|---|---|
| [spec/isa.md](spec/isa.md) | 46 instructions, encodings, cycles (MCS-4 manual) |
| [spec/abi.md](spec/abi.md) | data layout, register classes, calling convention |
| [spec/asm_syntax.md](spec/asm_syntax.md) | assembler syntax |
| [plans/](plans/implementation_plan.md) | implementation and [test plan](plans/testplan.md) |
| `target/lib/Target/I4004/` | the backend sources (TableGen + C++): MC layer, disassembler, and the CodeGen backbone (TargetMachine/Subtarget/RegisterInfo/InstrInfo/FrameLowering/ISelLowering/ISelDAGToDAG/AsmPrinter/MCInstLower + `I4004ExpandPseudos`), applied as an overlay onto the pinned LLVM submodule |
| `target/llvm-include/` | the `ELFRelocs/I4004.def` installed into the LLVM tree by the overlay |
| `scripts/apply_overlay.py` | installs the target sources + idempotent Triple/ELF infra patches (incl. the i4004 arch + 16-bit pointer width) onto a scratch LLVM tree |
| `scripts/build_llvm.sh` | builds `llvm-mc` + `llc` + MC tools with `LLVM_EXPERIMENTAL_TARGETS_TO_BUILD=I4004` into a Docker volume |
| `scripts/run_tests.sh` | runs the `tests/mc/` + `tests/codegen/` lit suites, the MC disassembler cross-check, and the `exec_fn.py` chips execution test |
| `scripts/exec_fn.py` | execution test: compiles functions with `llc`, runs the compiled ROM on `external/chips` via `tools/chips_xcheck/run_fn`, compares to the host |
| `scripts/isel_bcd_probe.sh` | milestone acceptance harness: compiles the BCD digit-add (pinpoints the remaining load/store + compare selection layers) |
| `tests/mc/` | MC encode / error / reloc / ELF tests (one CHECK per opcode) |
| `tests/codegen/` | `llc` selection-pattern FileCheck tests (ret/add/sub/branch/elf/mc-roundtrip) |
| `rust/`, `asm/` | Rust target JSON; assembler/linker (milestone 5) |
| [report/](report/final_report.md) | final report |

## Build & test

LLVM source is the `external/llvm-project` submodule pinned to `llvmorg-23.1.3`
(the LLVM version of the repo's pinned nightly rustc). Build with Docker (the
host has no cmake/ninja); the LLVM build tree lives in a named volume:

```sh
# build llvm-mc + MC tools into llvm-i4004/install/bin
docker run --rm --user root -v B:\ai_sw_engineer_mc:/workspace \
  -v ai-sw-llvm-build:/build -w /workspace ai-sw-engineer:latest \
  bash -lc 'chown -R mos:mos /build; su mos -c "llvm-i4004/scripts/build_llvm.sh"'

# run the MC tests
docker run --rm -v B:\ai_sw_engineer_mc:/workspace -w /workspace \
  ai-sw-engineer:latest bash -lc 'llvm-i4004/scripts/run_tests.sh'
```

Simulators: ai_hw_engineer `design/intel_4004` (RTL + ISS) and `external/chips` (`src/mcs4/cpu4004.rs`).
