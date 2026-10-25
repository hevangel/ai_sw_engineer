# Boot ROM implementation plan

1. Collect the descriptions (MO5, HxC) and the MO5 emulator's disk
   protocol (done). 2. Spec and ledger BOOT-1..4 (done). 3. Rust core and
   `__leaf_` asm drivers; host disk model (done). 4. Fit 256 bytes (done,
   241 B). 5. Boot the other titles on the RTL (done). 6. Report (done).

## Design decisions

- The control flow (seek, sector loop, checksum, retry) is Rust; the
  controller and stack-card port sequences are `__leaf_` asm routines that
  keep the caller's registers (llvm-i8008 `abi.md` §3.3).
- The checksum is computed while the bytes pass through the stack card: the
  ROM shadows page 0 for reads, so the boot cannot read back what it stored.
