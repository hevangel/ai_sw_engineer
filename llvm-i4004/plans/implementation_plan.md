# llvm-i4004 implementation plan

## Milestone 3b — instruction selection (COMPILES 39/43 FIRMWARE FNS, branch `llvm/i4004-codegen`)

STATUS: `llc -mtriple=i4004` compiles 39/43 Busicom firmware functions. Done:
TargetMachine/Subtarget/RegisterInfo/InstrInfo/FrameLowering/ISelLowering/
ISelDAGToDAG/AsmPrinter/MCInstLower; i8 legal in PR8, i16/pointers in DR16
(double-pair, AVR model); add/sub/compare/branch/select/call/load/store/
spill/extend/i16-const/frame-index; AND/OR/XOR + shifts + mul/div/rem +
memcpy/memset via libi4004rt libcalls. Verified: lit 25/25 + 40/40 execution on
external/chips. Remaining: the D2 callee-saved super-register spill (2 fns) and
the linker/image packer (milestone 5) for a whole-ROM image + size. See
report/final_report.md.

Goal: a **minimum** CodeGen backbone so `llc -mtriple=i4004` compiles small
functions to assembly our own `llvm-mc` accepts, with a lit/FileCheck test per
selection pattern (AGENTS.md rule). Follows `spec/abi.md` exactly: data layout
`e-m:e-p:16:8-p1:16:8-p2:16:8-i8:8-i16:8-n8-a:8`, C int = 16, `i8` the native
legal integer on register pairs, P0 = FIN/JIN scratch, P7 = software SP, args
P1–P3, P4–P6 callee-saved, 3-level hw stack then a software return stack.

Components, smallest-first (each ends with a passing lit test):

1. **TargetMachine + Subtarget + registration.** `I4004TargetMachine` with the
   abi.md data-layout string; `I4004Subtarget` owning InstrInfo/RegisterInfo/
   FrameLowering/TLI; register the TM in `LLVMInitializeI4004Target`; a
   `I4004PassConfig` adding the DAG-to-DAG pass. Exit: `llc -mtriple=i4004
   --filetype=null` accepts an empty function.
2. **RegisterInfo + FrameLowering (minimal).** `I4004RegisterInfo` (TableGen
   GPR/pair classes already exist); reserve ACC/CY/P0/P7; a trivial
   FrameLowering with no real frame yet (leaf functions only). Exit: a leaf
   returning a constant compiles.
3. **ISelLowering.** `i8` legal (on PR8 pairs), `i4` a custom value width,
   `i16`/`i32` expanded; lower `ret`, `add`/`sub` i8, constants (`LDM`/`FIM`),
   `BR`/`BRCOND`/`setcc` via JCN+CY, `load`/`store` addrspace-0 via SRC+RDM/WRM.
   The 4004 has **no AND/OR/XOR** — expand to NAND-free sequences or a libcall,
   documented. Exit: one lit test per lowered node.
4. **Calling-convention lowering** (`I4004CallingConv.td` + custom): args in
   P1–P3 then imaginary RAM regs; return i8 in P1; `JMS`/`BBL` for depth ≤ 3.
   Exit: a function calling a leaf compiles and `llvm-mc` re-accepts the asm.
5. **ISelDAGToDAG + AsmPrinter.** A DAG-to-DAG matcher (TableGen patterns where
   possible, C++ for the awkward ACC-centric ones) and an AsmPrinter emitting
   our asm syntax. Exit: the per-pattern lit tests above pass end-to-end.
6. **Milestone function:** compile ONE BCD digit-add from
   `system/busicom_141pf/firmware/src/bcd.rs` (`add_mag`'s inner nibble add, or
   a minimal extract) to a 4004 object; report byte size; compare qualitatively
   with the original ROM's add routine if identifiable (cite, never copy).
7. **No rustc custom-target integration** unless everything else is done;
   recorded as the next step.

Realistic risk: the 4004 is a hostile target (single 4-bit ACC, no GPR ALU, no
addressable memory without SRC). The honest exit for this milestone is "the
backbone selects the documented subset and compiles the largest reachable
slice of a real BCD routine," with any blocker stated precisely.

---

# llvm-i4004 implementation plan (historical milestones)

Milestones (each ends with passing tests):

1. **ISA + ABI specs written; Rust target JSON drafted. — DONE.**
   - `spec/isa.md`: all 46 instructions with encoding, words/cycles, flags;
     JCN conditions; SRC/DCL 4001/4002 addressing; 3-level stack; subtract
     carry; page-relative behavior + end-of-page quirk; sources per section.
   - `spec/abi.md`: data layout `e-m:e-p:16:8-p1:16:8-p2:16:8-i8:8-i16:8-n8-a:8`
     (n8 chosen over n4:8, justified); 3 address spaces; register classes
     ACC/CARRY/GR4/PR8 with P0/P7 reserved; imaginary registers in 4002 RAM;
     i4004-C calling convention incl. software return stack; `R_I4004_*`
     relocations and ELF `e_machine=0x4004`; `libi4004rt` helpers incl. BCD.
   - `spec/asm_syntax.md`: mnemonics, operands, directives, literals.
   - `rust/i4004-none.json`: data-layout equals abi.md; pointer/c-int = 16.
   - Open questions recorded in `abi.md` §7 (not guessed).

2. **MC layer. -- DONE.** Built on LLVM 23.1.3 as an overlay onto the
   external/llvm-project submodule (llvm-i4004/target/, applied by
   scripts/apply_overlay.py).
   - I4004RegisterInfo.td: ACC, CY, R0-R15, P0-P7 register pairs; classes
     GR4, PR8. (P0/P7 reservation per abi.md is a codegen concern deferred to
     milestone 3; the MC layer only needs the encodings.)
   - I4004InstrInfo.td / I4004InstrFormats.td: one record per isa.md row with
     the exact OPR OPA encoding bits and operand classes from asm_syntax.md.
     Two-word instructions carry their second word via a fixup.
   - I4004AsmParser + I4004InstPrinter + I4004MCCodeEmitter + I4004AsmBackend +
     I4004ELFObjectWriter: fixups fixup_addr12/page8/imm8/fin8 ->
     R_I4004_ADDR12/PAGE8/IMM8/FIN8 (abi.md sec.5) and e_machine 0x4004.
   - **Exit (met):** tests/mc/ assembles every one of the 46 opcodes and
     llvm-mc -triple=i4004 -show-encoding matches the isa.md byte column;
     10/10 MC tests pass; the ELF object carries e_machine 0x4004 and the
     R_I4004_* relocations. **Deferred:** page-local cross-page *rejection*
     (relocations exist, but the linker-side page-identity check lands with the
     linker/image-packer in milestone 5), so err_page_cross.s is not yet in.

3. **Instruction selection** for `i4`/`i8` add/sub/and/or/xor/load/store/branch
   at native width (`tests/codegen/`), ACC/CY modeled as implicit; `SRC`+`RDM`/
   `WRM` for addrspace-0 load/store; `FIN` for addrspace-1 constant loads.

4. **Legalization** of `i16`/`i32` (expand to byte ops); `libi4004rt` helpers
   (`__mulqi3`, `__udivqi3`, shifts, `memcpy`, BCD wrappers — `abi.md` §6);
   calls and the two-tier stack (hardware ≤3, software stack via P7).

5. **Linker + image packaging:** `ld.lld` i4004 hook or the `asm/` image
   packer; 256-byte page layout honoring page-local relocations; satisfy
   `scripts/build_image.sh` and fill `platform/link.ld` MEMORY/soft-stack base.

6. `core` builds via `-Zbuild-std=core`; smoke programs run on the ai_hw_engineer
   ISS/RTL and `external/chips` (`tests/smoke/`).

7. First real title: `system/busicom_141pf/firmware` compiled and run; size
   reported against 1280 / 4096 bytes.

## Immediate next actions (entering milestone 2)

- Scaffold the TableGen target skeleton under `target/` (register info first).
- Write the `tests/mc/` opcode-table fixture directly from `isa.md` (46 rows);
  it doubles as the encode oracle before any `.td` exists.
- Resolve open question #3 (i4 as a legal non-addressable value width) with a
  minimal `llc` experiment before committing the register-class design.
