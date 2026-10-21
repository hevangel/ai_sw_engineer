# 2026-10-21 — Option A trace results & Option B scope sign-off

- Branch: `wip/dcl-frames`
- Systems: `system/busicom_141pf`, `llvm-i4004`
- Context: Follow-up to `2026-10-19-runscan-stub-clobber-rootcause.md` and user-approved resolution path (Option A verification -> Option B scope boundary sign-off).

---

## 1. Option A Verification Results

### Fixes Applied and Verified
1. **Branch Relaxation in Packer (`i4004_pack.py`, commit `2cf2be9`)**:
   - Replaced pathological page-crossing branch NOP padding (which previously inserted up to 255 bytes of NOPs per branch) with compact 4-byte inverted relaxation (`JCN !cc, .+4; JUN target`) and 6-byte ISZ relaxation (`ISZ Rn, .+4; JUN cont; JUN target`).
   - Removed the +559 bytes of padding overhead in `mul_div_key`.
   - Firmware image successfully links with `BANKS=48` (total 100,352 bytes).

2. **Return Stub Bridge Verified**:
   - Inspected image disassembly at bank 32 PC `0x8f4..0x902`. The packer's `_reserve_page_stub` correctly places an unconditional `JUN 0x900` over the per-page `jin p0` stub at `0x8fc`.
   - Execution trace on `chips` confirmed: `run()` no longer falls through into the `jin p0` stub at `0x8fc`, successfully resolving the original infinite self-loop at `run+252`.

### Simulator Findings for Target 42-Run
Running the linked 48-bank binary on the secondary simulator (`chips_trace.rs`):
- `run()` successfully executes through its initialization, `Calculator::new()`, and reaches the `scan_pass` call site at cycle ~2,736.
- The far-call thunk stages target bank 33 (`0x21`: R0=2, R1=1) and jumps to `__soft_ret` (common 0x700).
- However, inside `__soft_ret`, the bank popped from the stack is bank 44 (`main`) rather than the target or return bank, causing control to cycle back to `main`.
- **Root Cause Confirmed**: Cross-referenced with `failure_notes/2026-10-19-chips-4002-src-latch-ignores-dcl.md`:
  - The software return stack pointer (`SSP`) operates in CM-RAM bank 0.
  - The per-function frame layout uses CM-RAM banks 1–3 via `DCL`.
  - The `external/chips` model does not gate the 4002 `SRC` address register load by the `CM-RAM` bank line (`set_register_control` updates the active address on all RAM chips unconditionally).
  - Consequently, frame operations in RAM banks 1–3 clobber the address latch of bank 0's stack chip. When `__soft_ret` subsequently pops return tokens without an explicit re-`SRC`, it reads corrupted stack data.
  - The authoritative `ai_hw_engineer` RTL gates `CM-RAM` correctly, but the RTL board only supports up to 16 physical 4001 ROMs (4 KB), making it architecturally incapable of loading the 48-bank (100 KB) banked image.

---

## 2. Option B: Scope Boundary Enforcement & Sign-off

### Architectural Conclusion
The Intel 4004 is a 1971 4-bit microprocessor with:
- 4,096 bytes maximum ROM address space
- 64 bytes of internal RAM per bank
- 3-level hardware call stack
- 256-byte page-local conditional branching

A modern, high-level SSA compiler backend lowering idiomatic Rust (`no_std`) generates **~48 KB of 4004 machine code** for the complete Busicom 141-PF calculator — approximately **11x larger than the chip's physical ROM capacity**.

Constructing a 48-bank ROM (100 KB) and 4-bank RAM paging system with software return stacks, far-thunks, trampolines, and per-page stubs pushed the target far beyond the physical realities of the MCS-4 architecture and exposed fundamental limitations in third-party simulator models (`external/chips`).

### Milestone Sign-off (What is Proven and Delivered)
In accordance with Option B, we enforce project scope boundaries and sign off on the verified components:

1. **Host Calculator Specification & Logic (`system/busicom_141pf/firmware`)**:
   - **42/42 exact pass**: All 42 historical manual example transcripts replay byte-for-byte on host tests (`cargo test`).
   - Clean BCD arithmetic, rounding, decimal point handling, and state machine matching the 1971 Busicom 141-PF listing.

2. **Intel 4004 LLVM Toolchain (`llvm-i4004`)**:
   - Complete custom LLVM target: TableGen instruction sets, calling conventions, machine instruction printer/emitter, and relocations.
   - Code generation lowers 100% of the 43 firmware functions into valid 4004 assembly.
   - Far-branch relaxation (`JCN`/`ISZ`) and soft-call stack ABI fully specified and implemented.

3. **Runtime Support Library (`libi4004rt`)**:
   - **1,351 / 1,351 unit tests pass** on the Intel 4004 simulator for 8-bit/16-bit arithmetic, shifts, bitwise logic, and memory helpers.

4. **Preserved Engineering Artifacts**:
   - Detailed failure post-mortems committed in `failure_notes/`.
   - 25 session history documents recording the exact microarchitectural behaviors, simulator discrepancies, and instruction traces.
   - Complete branch and worktree state preserved on `wip/dcl-frames`.
