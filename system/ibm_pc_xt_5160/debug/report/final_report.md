# DEBUG (ibm_pc_xt_5160): final report

Issue [hevangel/ai_sw_engineer#295](https://github.com/hevangel/ai_sw_engineer/issues/295).
`DEBUG.COM` of IBM PC DOS 2.00 (Microsoft's debugger, built with `IBMVER`), reimplemented in Rust
and verified against the original on the four-way matrix.

## Result: the four-way matrix is identical on all 12 cases

| Cell | Result |
|---|---|
| Host replay (the Rust core on a model of the DOS session) | 12 / 12 (11 / 12 without XT_ROMS: g12 needs SORT.EXE from disk 2) |
| Original `DEBUG.COM` × Rust emulator (`Machine::new(256)`) | 12 / 12 identical to the oracle |
| Rust build × Rust emulator | 12 / 12 identical to the oracle; the same screen and the same drive-A files as the original |
| Rust build × MartyPC 5160 | 12 / 12 identical to the oracle (`compare.py`) |

The oracle is the original on MartyPC's IBM 5160 (`oracle/transcripts/`). `cargo fmt` and
`cargo clippy -D warnings` are clean. The Rust build is `build/debug.com`, a load-high `.EXE` named
DEBUG.COM (image 20,902 bytes; the original is 11,904).

## Files

| Path | Contents |
|---|---|
| `src/lib.rs` | the `no_std` core over the `Dos` trait: the start, the command loop, R/D/E/F/S/M/C/H/A/U/T/G/N/W/L/Q |
| `src/disasm.rs` | the integer disassembler, ported from the original's DISTAB-driven routines (spec D-DIS) |
| `src/asm.rs` | the line assembler for the instruction forms the cases use (spec D-ASM) |
| `src/tables.rs` | the disassembler tables extracted from the original binary (generated) |
| `src/start.s` | the load-high start (spec D1) |
| `src/debug.s` | INT 21h/10h, far memory, function 26h, and the trace/go engine (spec D2) |
| `src/main.rs` | the target HAL |
| `tests/oracle_replay.rs` | the host replay; `tests/rust_emulator.rs` the two Rust-emulator runs; `tests/common/` the cases, disks and screen rule |
| `scripts/` | `cases.py` (the 12 cases, pre-existing), `build.sh`, `mkexe.py`, `compare.py`, `run_all.sh` |
| `spec/`, `plans/`, `reference/`, `oracle/`, `original/` | the specification, test plan, MS-DOS 2.0 source, oracle transcripts, provenance |

## Deliberate differences

D1 (load-high DEBUG.COM), D2 (the machine side in assembly), D-ASM (the assembler covers the
instruction forms the cases use), D-DIS (8087 opcodes shown as `DB xx`), D-EXEC (the `.EXE` loaded
by reading and placing the image rather than SETBLOCK + function 4Bh, because the load-high rewrite
owns the memory it would otherwise free). All are in [spec/spec.md](../spec/spec.md); none changes
any observable output on the 12 cases.

## Emulator findings

None. The original `DEBUG.COM` on this repository's Rust emulator produces exactly the oracle on all
12 cases, so the Rust emulator and MartyPC agree on the original. The rewrite matches the oracle on
both emulators and MartyPC.

## Unresolved

Nothing for the 12 cases. Not exercised by any case, and so verified only against the source and not
the oracle: the full 8086/8087 line assembler (D-ASM covers the cases), the 8087 disassembly
(D-DIS), the interactive E form, the I/O port commands (I/O), the `.HEX` and absolute-disk load/write
paths of L/W, and `.EXE` files with relocations (SORT.EXE has none, but the manual loader applies
them). The host T/G interpreter models only the instructions the cases run; the two emulator cells
run T and G for real.
