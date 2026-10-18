# llvm-i4004

| | |
|---|---|
| Chip | Intel 4004 — announced November 15, 1971 in *Electronic News* (see ai_hw_engineer `design/README.md` for sources) |
| Word / address width | 4-bit data, 8-bit instructions (1–2 bytes), 12-bit ROM address |
| Strategy | new experimental LLVM target on `llvm/llvm-project` (no existing backend) |
| Rust target | `rust/i4004-none.json` — data-layout `e-m:e-p:16:8-p1:16:8-p2:16:8-i8:8-i16:8-n8-a:8` |
| Status | **milestone 1 complete** — ISA + ABI + asm-syntax specs written, Rust target JSON finalized; no TableGen/C++ yet (specs before code) |

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
| `target/`, `rust/`, `asm/`, `tests/`, `scripts/` | see `llvm-_template/README.md` |
| [report/](report/final_report.md) | final report |

Simulators: ai_hw_engineer `design/intel_4004` (RTL + ISS) and `external/chips` (`src/mcs4/cpu4004.rs`).
