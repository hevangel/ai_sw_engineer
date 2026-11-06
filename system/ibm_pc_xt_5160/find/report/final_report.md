# FIND (PC DOS 2.00) for the IBM PC XT 5160: report

Issue [hevangel/ai_sw_engineer#283](https://github.com/hevangel/ai_sw_engineer/issues/283). Run of 2026-11-06 on a Linux host.

## Steps of the title lifecycle (spec/software-lifecycle.md)

| Step | Result |
|---|---|
| 1. Original | `FIND.EXE` from PC DOS 2.00 disk 1, 5,888 bytes, SHA-256 `947b34cb...` ([provenance](../original/provenance.md)); not committed |
| 2. Original on the emulators | MartyPC `05c0d088` IBM 5160 with the original XT BIOS: 22 transcripts ([oracle](../oracle/README.md)); our Rust emulator (5150 board, 64 KiB, PC DOS 2.00 from a 180 KiB disk): **identical to MartyPC on all 22** |
| 3. Source | Microsoft's MS-DOS 2.0 `FIND.ASM` (MIT), Rev. 8; the binary is Rev. 6 (revision byte `6`, no Rev. 7 patch area, file-name errors without the `_post` messages). Microsoft's built `FIND.EXE` has the same load module as IBM's ([reference](../reference/ms-dos-2.0/README.md)) |
| 4. Documents | no IBM manual page fetched; the binary and the source are the documents used |
| 5. Reverse engineering | [spec](../spec/spec.md) and [I/O](../spec/io.md) from the code (2Ah-331h) and data of the 5,284-byte load module read in full with GNU objdump 2.42 (REA needs a Ghidra-class provider, not installed here) |
| 6. Rust | `src/lib.rs` (`no_std` core over a `Dos` trait: the original's option scan, quote rules, 4,096-byte buffer with the seek back, counters and writes), `src/main.rs` (INT 21h: 30h, 3700h, 3Dh, 3Fh, 42h, 3Eh, 40h, 09h, 4Ch) |
| 7-8. Four-way matrix | identical on all 22 cases, below |

## The matrix

What is compared: every screen row from the command line to the next `A>` prompt, empty rows included ([test plan](../plans/testplan.md)).

| | MartyPC IBM 5160 (XT BIOS, PC DOS 2.00) | Rust emulator (5150 BIOS, PC DOS 2.00) |
|---|---|---|
| original FIND.EXE | the oracle (22 transcripts) | 22 of 22 identical (`tests/rust_emulator.rs`) |
| Rust FIND.EXE | 22 of 22 identical (`capture_dos.sh` with `PROGRAM=`/`NAME=FIND.EXE`, `compare.py`) | 22 of 22 identical (`tests/rust_emulator.rs`) |

The host replay (`tests/oracle_replay.rs`: the core with a DOS model, its output drawn as the console draws it) is also 22 of 22, plus four rule tests (exit codes; option letters and the signed capitalization; a lone LF always matching; line numbers running ahead after a 4,096-byte refill). All passed on the first run.

Found while getting there: the oracle's ^Z behaviour (f02, f06, f19) is DOS's, not FIND's: FIND treats ^Z as a byte and DOS's console write stops at it. The host model does the same; the rewrite does nothing special.

## Commands

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR system/ibm_pc_xt_5160/find/scripts/run_all.sh
```

## Size

`build/find.exe`: 2,822 bytes (the original: 5,888 bytes on disk, a load module of 5,284 bytes of which 776 are code and most of the rest the 4,097-byte buffer), SHA-256 `4f1afe555f1d53b1ec25d2106fc260e70bb6233aa704995add9b2365bda9874f`, a tiny-model `.EXE` with the shared DOS runtime: `.text` 2,141 bytes, `.rodata` 169, `.bss` 4,099 (the buffer and the runtime's word): 6,409 of 65,024 bytes of the segment. Built by the msp430 Rust route (nightly-2026-10-15) and `llvm-i8086`.

## Limits and deliberate differences

- D1-D4 in the spec change nothing a case or a practical command line can observe: 128-byte string and name buffers (the original's name buffer is 65 bytes), the lone-LF match taken as a rule, the bad option letter written from a local byte, the buffer as a static array of the same 4,097 bytes.
- Kept as the original has them: the hang on a line over 4,096 bytes, line numbers running ahead by one per refill, only the low bytes of the counters cleared between files, exit codes 2/1/0.
- Not run on the hardware project's XT SV machine or on hardware.
- The Rust emulator half runs on the 5150 board model, not an XT (the XT's Rust emulator is ai_sw_engineer#29).
- Not covered on screen: files over 4,096 bytes (host test only), a read error, DOS 1.x, the exit code.
