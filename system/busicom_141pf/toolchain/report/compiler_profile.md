# Compiler profile verification record — Busicom 141-PF

Owning issue: hevangel/ai_sw_engineer#53. Recorded 2026-10-25 (America/Vancouver).
Profile and pinned toolchain: [../README.md](../README.md). All steps were run
fresh in this session on the host `ai-sw-engineer:latest` container
(image ID `15c2601a16ff`); nothing below is quoted from an earlier report.

## Pipeline gates (run 2026-10-25)

1. **Whole-program IR** — `firmware/scripts/build_target_ir.sh` in the
   container: pinned `nightly-2026-10-15` (`-Z build-std=core`, fat LTO,
   `opt-level=z`, `panic=abort`, `avr-none`), retargeted by
   `llvm-i4004/rust/retarget_ir.py`
   (`triple+datalayout->i4004, dropped 262 code addrspace(1); i32 uses=2,
   i64 uses=21 (reported, not refused)`) → `build/firmware_i4004.ll`
   (1,081 lines).
2. **Target image** — `llvm-i4004/scripts/build_image.sh <title> <out>
   …/platform/link_xl.ld` with the volume-cached LLVM 23.1.3 tools:
   `llc` firmware object + `llvm-mc` runtime objects → `i4004_pack.py`
   (15 objects, max 4096) → `firmware.bin`, **4,096 / 4,096 bytes**.
   - SHA-256 `4c0a9535d15735ab653066d84662dc7b3ffe84c142d9966e889755dfba678061`
   - The same hash was produced by two independent runs in this session
     (first build and `toolchain/verify.sh`), i.e. the pipeline is
     bit-reproducible for this firmware.
3. **Execution on the independent `external/chips` MCS-4 board model**
   (`platform/scripts/run_image.sh secondary --board xl`, chips pin
   `1bc3d5781474ef9d7e7305af91071a723797f222`): all 42 manual examples ran;
   the printed-tape payload (600 ROW/LAMPS lines) is **byte-identical** to the
   committed original-ROM oracle trace
   `firmware/oracle/rom_port_trace.txt` (whose payload the native emulator
   release CLI also reproduces line-for-line).
4. **`toolchain/verify.sh` end-to-end** — the same five gates re-run as one
   command inside the container: `PASS: compiled firmware tape identical to
   the original-ROM oracle`.
5. **Primary RTL executor** — `run_image.sh primary --board xl` (Verilator,
   `ai-hw-engineer:codex-java` image, spin 1481): all 42 manual examples ran;
   the printed-tape payload is **byte-identical** to the original-ROM oracle,
   and the RTL trace file is byte-identical to the chips trace from gate 3
   (SHA-256 `8513330f7338599d4df584c90c51c2e5dce876bd5681b375a9ebb7fb391e1748`).

## Acceptance mapping (issue #53)

| Acceptance box | Evidence |
|---|---|
| Audit upstream support; pin versions/licenses/acquisition | [../README.md](../README.md) table: no upstream i4004 support; every component pinned with license and acquisition (Dockerfile, submodule pin, volume build) |
| Exact machine/CPU spec, ISA, ABI, data layout | llvm-i4004 `spec/isa.md` + `spec/abi.md` (i4/i8 types, 16-bit pointers, software return stack); machine/board contract in `platform/memory_map.md` + `link_xl.ld`; no floating point (the 4004 has none; integer policy = the firmware's BCD arithmetic in `asm/`) |
| Maintained target specification | `llvm-i4004/rust/i4004-none.json`, applied via `apply_overlay.py` onto the pinned LLVM; exercised by every build above |
| Startup/entry/stack/sections/linker/relocations/runtime | `llvm-i4004/rt/` (`crt0.s` entry, software return stack in 4002 RAM), `hal.s` board I/O, `i4004_pack.py` page placement + branch relaxation; no interrupts on this machine |
| Build/link/package commands with size checks and legal media | The two container commands in the README + `toolchain/verify.sh`; packer ROM gate (4,096/4,096); original firmware never committed (provenance + SHA-256 only) |
| Smoke programs + independent disassembly | `llvm-i4004/tests/mc` + `tests/codegen` lit suites and the MC disassembler cross-check are the backend's standing evidence (recorded 28/28, 43/43 in its final report; not re-run in this session — see Limits); per-machine, the compiled firmware itself was decoded by an independent executor (chips) and matched the original ROM byte for byte |
| Execute on a qualified independent emulator; record commands | Gate 3 above on `external/chips` (the oracle cross-checked against the project RTL); commands in the README/verify.sh; expected output = the committed ROM trace |
| Regression tests + report with versions/commands/hashes | This report; `toolchain/verify.sh` re-runs gates 1–3 + comparison on demand |

## Limits and explicit gaps

- **Native Rust system emulator cannot run the compiled image.** It models the
  stock board (2 × 4002, no RAM banks); the compiled firmware is linked for
  the banked xl board (16 × 4002). Attempting it on 2026-10-25 printed only
  all-sector-0 rows (`ROW|000000000000000|◇|#`) — the printer hammer chain is
  filled from RAM the stock model does not provide. Not a compiler defect:
  the same image replays identically on chips/RTL. Open work: teach the
  native emulator the xl board, or link a stock-board firmware variant.
- **The image fills the xl ROM exactly** (4,096/4,096): zero headroom; any
  firmware growth fails the packer gate by design.
- **The IR path depends on `avr-none`** staying data-layout-compatible in the
  pinned nightly; there is no native i4004 rustc target. `retarget_ir.py`
  reports (does not refuse) i32/i64 uses — currently 2/21 — which the runtime
  expands.
- **Backend lit/MC suites were not re-run this session** (they exercise the
  chip-level backend, not the per-machine profile; last recorded 28/28 MC,
  43/43 codegen, 5,540/5,540 + 5,520/5,520 chips execution checks in
  llvm-i4004's final report). Re-run them with
  `llvm-i4004/scripts/run_tests.sh` when touching the backend.
- Protected mode, hardware tasks and the 286-style MMU do not exist on the
  4004 and are out of scope here.
