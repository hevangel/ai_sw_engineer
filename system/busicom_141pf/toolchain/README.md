# Busicom 141-PF compiler configuration, runtime and packaging profile

Owning issue: hevangel/ai_sw_engineer#53 — the per-computer ownership of the
Rust → LLVM → i4004 → ROM pipeline for this machine. The shared backend lives
in [../../../llvm-i4004](../../../llvm-i4004/README.md); this folder owns the
pinned, reproducible configuration that puts Rust firmware on the Busicom
141-PF: exact toolchain versions, the target spec, startup/linker contract,
the ROM packaging command with its size gate, and the execution/verification
chain against independent oracles. Verification record:
[report/compiler_profile.md](report/compiler_profile.md). One-command
reproduction: [verify.sh](verify.sh).

## Pinned toolchain

| Component | Pin | Where | License |
|---|---|---|---|
| Host rustc (host tests, release CLI) | `1.99.1` | Dockerfile `RUST_STABLE` | MIT/Apache-2.0 |
| IR rustc (whole-program firmware IR) | `nightly-2026-10-15` + `rust-src`, `-Z build-std=core` | Dockerfile `RUST_NIGHTLY`; [firmware/scripts/build_target_ir.sh](../firmware/scripts/build_target_ir.sh) | MIT/Apache-2.0 |
| LLVM (llc, llvm-mc, FileCheck, lit) | `llvmorg-23.1.3` with the llvm-i4004 target overlay applied by [apply_overlay.py](../../../llvm-i4004/scripts/apply_overlay.py) | `external/llvm-project` submodule (`0d261d1c…`, shallow), built into the `ai-sw-llvm-build` Docker volume by [build_llvm.sh](../../../llvm-i4004/scripts/build_llvm.sh) | Apache-2.0 WITH LLVM-exception |
| Linker / ROM packer | [llvm-i4004/asm/i4004_pack.py](../../../llvm-i4004/asm/i4004_pack.py) (page-filling placement, branch relaxation, frame overlay, call-depth check) | repo | MIT |
| Runtime / startup | [llvm-i4004/rt](../../../llvm-i4004/rt/): `crt0.s` (reset entry, stack), `libi4004rt` (wider-integer and memory libcalls), `hal.s` (Busicom board I/O) | repo | MIT |
| Rust target spec | [llvm-i4004/rust/i4004-none.json](../../../llvm-i4004/rust/i4004-none.json) — data layout `e-m:e-p:16:8-p1:16:8-p2:16:8-i8:8-i16:8-n8-a:8` | repo | MIT |
| Execution cross-check | `external/chips` @ `1bc3d5781474ef9d7e7305af91071a723797f222` (no license: run, never copy) | submodule | — |
| Build container | `ai-sw-engineer:latest` from the repo [Dockerfile](../../../Dockerfile) | repo | MIT |

Upstream support: there is no i4004 backend in LLVM or rustc; everything above
the IR is project-owned. The IR is emitted through `avr-none` — the closest
existing target whose data layout matches (little-endian, 16-bit pointers at
8-bit alignment) — and retargeted by
[retarget_ir.py](../../../llvm-i4004/rust/retarget_ir.py); the rationale and
mapping are documented in llvm-i4004 `rust/README.md` §Mapping. This is a
pinned workaround, not upstream support.

## Machine profile

- CPU: Intel 4004; ISA subset, ABI and calling convention:
  [llvm-i4004/spec/isa.md](../../../llvm-i4004/spec/isa.md),
  [abi.md](../../../llvm-i4004/spec/abi.md). The 4004 has no later
  instructions to forbid — the ISA is closed; illegal opcodes are two-byte
  JIN aliases, which the backend never emits.
- Board: the compiled firmware is linked for the **xl board**
  ([platform/link_xl.ld](../platform/link_xl.ld)): 16 × 4001 (4,096 B ROM) +
  16 × 4002 (banked RAM), Busicom I/O identical to the stock machine
  ([memory_map.md](../platform/memory_map.md) "xl"). The original 1,280 B
  stock board is the budget yardstick; the current firmware fills the xl ROM
  exactly (4,096/4,096).
- Startup/entry: `crt0.s` reset entry; software return stack in 4002 RAM
  (the 4004's hardware stack is 3 deep); interrupt/vector integration: none —
  the machine has no interrupts.
- Executable format: flat 4001 ROM image (`firmware.bin`), page-packed by
  `i4004_pack.py` with the ROM-size gate (`--max`, from the linker script).
- Firmware media legality: the original Busicom/Intel firmware is
  copyrighted and never committed — provenance and SHA-256 only
  ([../firmware/original/provenance.md](../firmware/original/provenance.md)).

## Reproducible pipeline (all inside `ai-sw-engineer:latest`)

```sh
docker run --rm -v <repo>:/workspace -v ai-sw-llvm-build:/llvm-build \
  -w /workspace ai-sw-engineer:latest bash -lc '
    system/busicom_141pf/firmware/scripts/build_target_ir.sh &&
    llvm-i4004/scripts/build_image.sh system/busicom_141pf/firmware \
        system/busicom_141pf/firmware/build/target \
        /workspace/system/busicom_141pf/platform/link_xl.ld'
# → system/busicom_141pf/firmware/build/target/firmware.{s,o,bin,map}
```

Or run every gate at once (same container):

```sh
system/busicom_141pf/toolchain/verify.sh
```

`verify.sh` = IR → target image → size/hash → 42-example replay on the
independent `external/chips` board model (`platform/scripts/run_image.sh
secondary --board xl`) → byte comparison of the printed tape payload against
the committed original-ROM oracle
([../firmware/oracle/rom_port_trace.txt](../firmware/oracle/rom_port_trace.txt)).
The primary executor is the ai_hw_engineer RTL
(`run_image.sh primary --board xl`, needs the ai-hw-engineer Verilator image);
its last recorded result is 42/42 in the firmware final report.

## Limits and gaps

- **The native Rust system emulator cannot execute the compiled firmware**: it
  models the stock board (2 × 4002, no RAM banks), while the compiled image is
  linked for the banked xl board. Attempting it prints all-sector-0 rows
  (observed 2026-10-25). Compiled images must go through
  `platform/scripts/run_image.sh --board xl`. Adding xl-board support to the
  native emulator is open work, tracked in the compiler-profile report.
- The image sits exactly at the 4,096-byte budget; any firmware growth fails
  the packer gate by design.
- The IR detour through `avr-none` depends on the pinned nightly's data layout
  remaining AVR-compatible; a native i4004 rustc target does not exist.
- ROM-only code: no data-space execution, no writable code — enforced by the
  packer's page placement.
