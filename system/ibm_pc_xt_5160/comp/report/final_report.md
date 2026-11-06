# COMP (PC DOS 2.00) for the IBM PC XT 5160: report

Issue [hevangel/ai_sw_engineer#284](https://github.com/hevangel/ai_sw_engineer/issues/284). Run of 2026-11-06 on a Linux host.

## Steps of the title lifecycle (spec/software-lifecycle.md)

| Step | Result |
|---|---|
| 1. Original | `COMP.COM` from PC DOS 2.00 disk 1, SHA-256 `4b30a36d...` ([provenance](../original/provenance.md)); not committed |
| 2. Original on the emulators | MartyPC `05c0d088` IBM 5160 with the original XT BIOS: 10 transcripts (`oracle/transcripts`); our Rust emulator (5150 board, 64 KiB, PC DOS 2.00 from a 180 KiB disk): **identical to MartyPC on all 10** |
| 3. Source | none published (IBM's own program) |
| 4. Documents | no manual page fetched; the binary is the document used |
| 5. Reverse engineering | [spec](../spec/spec.md) and [I/O](../spec/io.md) from the 2,523-byte image read in full with GNU objdump (REA needs a Ghidra-class provider, not installed here) |
| 6. Rust | `src/lib.rs` (`no_std` core over a `Dos` trait, one method per DOS call the original makes), `src/main.rs` (INT 21h, `.COM`) |
| 7-8. Four-way matrix | identical on all 10 cases, below |

## The matrix

What is compared: the rows from the one after the command line to the next `A>` prompt, empty rows included, or the tail of the screen where the command line has scrolled off (p03) ([test plan](../plans/testplan.md)).

| | MartyPC IBM 5160 (XT BIOS, PC DOS 2.00) | Rust emulator (5150 BIOS, PC DOS 2.00) |
|---|---|---|
| original COMP.COM | the oracle (10 transcripts) | 10 of 10 identical (`tests/rust_emulator.rs`) |
| Rust COMP.COM | 10 of 10 identical (`capture_dos.sh` with `PROGRAM=`, `compare.py`) | 10 of 10 identical (`tests/rust_emulator.rs`) |

The host replay (`tests/oracle_replay.rs`: the core with a model of DOS 2.00's FCB calls, its output drawn as the console draws it) is also 10 of 10, plus a test of the name parse model. The rewrite agreed with the oracle on the first run at every level; no difference had to be fixed.

## Commands

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR system/ibm_pc_xt_5160/comp/scripts/run_all.sh
```

## Size

`build/comp.com`: 4,630 bytes (the original: 2,523), SHA-256 `7532dc42881809835765ef18c90aeab1125af08397a44464a7c6df04eff6cb86`, with the shared DOS runtime; 9,294 bytes in memory with `.bss` (the state, including the 4 KiB piece buffer). Built by the msp430 Rust route and `llvm-i8086`.

## Limits and deliberate differences

- D1: the large buffer is the free memory of the program's own 64 KiB segment, from the end of `.bss` to 2 KiB below the stack: 53,248 bytes (D000h) with 256 KiB, where the original takes up to 61,440 (F000h) in memory past its 6,232 bytes (the image and its 4 KiB piece buffer). Files over 52 KiB fill it a different number of times, which can change which file's last byte is checked for the ^Z; the `Insufficient memory` threshold differs too. D2-D5 in the spec change nothing observable.
- Not run on the hardware project's XT SV machine or on hardware.
- The Rust emulator half runs on the 5150 board model, not an XT (the XT's Rust emulator is ai_sw_engineer#29).
- Not covered by a case: directories and `- Invalid path`, an invalid drive, `Y` to Compare more, files over 52 KiB, empty files, DOS other than 2.00, `Insufficient memory` (plans/testplan.md).
