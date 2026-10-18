# llvm-i4004 implementation plan

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

2. **MC layer.** Next concrete steps:
   - `target/lib/Target/I4004/I4004RegisterInfo.td`: `ACC`, `CY`, `R0`–`R15`,
     `P0`–`P7` register pairs; classes `GR4`, `PR8`; reserve P0, P7; mark
     ACC/CY fixed. Verify against `abi.md` §2.
   - `I4004InstrInfo.td` / `I4004InstrFormats.td`: one record per `isa.md` row,
     with the exact `OPR OPA` encoding bits and operand classes from
     `asm_syntax.md`. Two-word instructions carry their second word as a fixup.
   - `I4004AsmParser` + `I4004InstPrinter` + `I4004MCCodeEmitter` + fixups for
     `R_I4004_ADDR12/PAGE8/IMM8/FIN8` (`abi.md` §5).
   - **Exit:** `tests/mc/` assembles every one of the 46 opcodes and
     `llvm-mc -show-encoding` matches the `isa.md` byte column; page-local
     fixups reject a cross-page target.

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
