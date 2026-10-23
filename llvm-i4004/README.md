# llvm-i4004

| | |
|---|---|
| Chip | Intel 4004 — announced November 15, 1971 in *Electronic News* (see ai_hw_engineer `design/README.md` for sources) |
| Word / address width | 4-bit data, 8-bit instructions (1–2 bytes), 12-bit ROM address |
| Strategy | new experimental LLVM target on `llvm/llvm-project` (no existing backend) |
| Rust target | `rust/i4004-none.json` — data-layout `e-m:e-p:16:8-p1:16:8-p2:16:8-i8:8-i16:8-n8-a:8` |
| Status | **2026-10-22 (issue #18): the Busicom firmware compiles to a flat 4,095 B image that runs 42/42 on the ai_hw_engineer RTL and on external/chips** (no banking; see [report](report/final_report.md)). Earlier status, kept for history: **complete (2026-10-21 wrap-up)** — full custom LLVM target for the 4004 on `llvm/llvm-project`: MC layer (encoder/disassembler/relocations), CodeGen lowering **43/43** Busicom firmware functions, lit **28/28**, and the `libi4004rt` runtime at **1,351/1,351** chips execution tests. Calling convention uses a software return stack (8-bit SSP) with cross-bank tokens + ROM-bank latch to beat the 3-level hardware stack; `asm/i4004_pack.py` banks the whole firmware (BANKS=48, 100,352 B) with far-thunks, per-page `jin p0` stubs and compact JCN/ISZ page-crossing relaxation. Data layout follows the AVR model: i8 in a register pair (PR8), i16/pointers in a double-pair (DR16), i16 arithmetic expanded to i8; AND/OR/XOR + shifts + mul/div/rem (i8/i16/i32) + memcpy/memset via libi4004rt libcalls. **Known limit (signed off)**: the compiled firmware is ~48 KB, ~11× the 4004's 4,096-byte ROM ceiling; the banked 42-example target run is blocked by a `external/chips` 4002 SRC-latch deviation (CM-RAM/DCL not honoured) plus the RTL board's 16-ROM cap. See [report](report/final_report.md) STATUS 2026-10-21 for what works and what does not. **2026-10-22 — fit plan (issue #18) under way:** two silent miscompiles fixed — signed `icmp` was lowered as unsigned, and every frame slot aliased at `__frame_<fn>+0` (see `failure_notes/2026-10-22-*`); `scripts/exec_cmp.py` runs every compare predicate on chips (5,540/5,540); constant `and/or/xor`, constant shifts and disjoint-nibble `or` are now native nibble code instead of bit-serial libcalls (`scripts/exec_nibble.py`, 5,520/5,520); `run_tests.sh` ends with the whole-firmware size gate, which fails by design until the firmware fits (56,525 B `.text`, 13.8× the 4,096 B ceiling; history in `system/busicom_141pf/firmware/report/size_history.md`). |

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
