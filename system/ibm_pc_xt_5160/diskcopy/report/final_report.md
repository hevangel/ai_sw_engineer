# DISKCOPY (PC DOS 2.00) for the IBM PC XT 5160: report

Issue [hevangel/ai_sw_engineer#289](https://github.com/hevangel/ai_sw_engineer/issues/289). Run of 2026-11-07 on a Linux host.

## Steps of the title lifecycle (spec/software-lifecycle.md)

| Step | Result |
|---|---|
| 1. Original | `DISKCOPY.COM` from PC DOS 2.00 disk 1, 2,444 bytes, SHA-256 `2189a014...` ([provenance](../original/provenance.md)); not committed |
| 2. Original on the emulators | MartyPC `05c0d088` IBM 5160 (256 KiB, XT BIOS 1982-11-08): 7 transcripts with drive B's hash ([oracle](../oracle/README.md)); our Rust emulator (5150 board, `Machine::new(256)`): **identical to MartyPC on all 7**, screen and drive B |
| 3. Source | none published; Microsoft's `DISKCOPY.ASM` is a different program (provenance) |
| 4. Documents | the binary is the document used; no manual page fetched |
| 5. Reverse engineering | [spec](../spec/spec.md) and [I/O](../spec/io.md) from the binary read in full with GNU objdump (REA needs a Ghidra-class provider, not installed here) |
| 6. Rust | `src/lib.rs` (`no_std` core over a `Pc` trait: the original's routines, state bytes and INT 13h calls), `src/main.rs` (INT 21h, `.COM`), `src/start.s` (the stack below the track buffer, spec D1), `src/bios.s` (INT 13h, INT 11h) |
| 7-8. Four-way matrix | identical on all 7 cases, screen and drive B, below |

## The matrix

Compared ([test plan](../plans/testplan.md)): the screen (the rows from the command line to the next `A>`) and drive B's SHA-256 after the run (a rewrite run's copy of its own drive A, which holds the rewrite as DISKCOPY.COM, counted as the copy of the original's drive A; byte for byte otherwise).

| | MartyPC IBM 5160 (XT BIOS, PC DOS 2.00, 256 KiB) | Rust emulator (5150 BIOS 04/24/81, PC DOS 2.00, `Machine::new(256)`) |
|---|---|---|
| original DISKCOPY.COM | the oracle (7 transcripts, 6 drive B hashes; d06 has no drive B) | 7 of 7 identical (`tests/rust_emulator.rs`) |
| Rust DISKCOPY.COM | 7 of 7 identical (`capture_dos.sh` with `PROGRAM=` and `SAVE_B=1`, `compare.py`) | 7 of 7 identical, and the same as the original there on all 7 |

The host replay (`tests/oracle_replay.rs`: the core against a model of the oracle machine's INT 13h and DOS) is 7 of 7 screens and 6 of 6 drive B hashes identical (with `XT_ROMS`; screens only without it), plus three tests from the spec (bad parameters and `/2`, two passes with one drive, `/1`).

The drive-A rule is still needed: the rewrite's raw drive B differs from the oracle's (d01 `b3dce880...` against `55f412a1...`) because it is a copy of a drive A that holds the rewrite; d08 (B to B) is identical as it is (`fce4aa49...`).

## History: head 1 of a one-sided image

The first oracle copied disk 1 itself, a one-sided 180 KiB image, in a double-sided drive. A read of its head 1 ends on MartyPC with ST1 = ST2 = 0 (fluxfox `SeekError`; BIOS status 20h), on the Rust emulator with ST1 = 04h (No Data; status 04h). DISKCOPY takes 02h/04h for a one-sided disk and anything else for unreadable tracks, so on MartyPC it reported 40 unreadable head-1 tracks and wrote the track buffers' earlier memory to them, and on the Rust emulator it copied one side: the five A-to-B cases differed between the emulators for the original and the rewrite alike. A real one-sided diskette gives 02h, so MartyPC was the outlier. Neither emulator was changed: the cases now copy a double-sided 360 KiB copy of disk 1 (`a:system360`, `pcdisk syscopy`), the oracle was re-captured, and the measures that existed only for the unreadable tracks (the rewrite moving itself so that its buffers were the original's; the host model of memory before the program) were removed ([spec](../spec/spec.md), "Head 1 of a one-sided image").

## Commands

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR MARTY_TARGET_DIR=DIR system/ibm_pc_xt_5160/diskcopy/scripts/run_all.sh
```

## Size

`build/diskcopy.com`: 3,526 bytes (the original: 2,444), SHA-256 `204d249a4f4ef93648aa36601e2bac15cec791dd2c25a77de032e020b2365b77`: code 2,783 bytes, read-only data 743 (the messages); `.bss` 2 bytes and a 1 KiB stack. Built by the msp430 Rust route and `llvm-i8086`.

## Limits and deliberate differences

- D1: `src/start.s` in place of `crt0_com.s` puts the stack after `.bss`, and the track buffer starts after it by the original's rounding rule (0740h for a PSP at 0600h, the original's 06A0h): 50 tracks fit on 256 KiB against the original's 51, the same passes and swaps while 40 fit. With less memory the rewrite may need more passes than the original. An unreadable source track is written from the buffer's earlier bytes, which are other addresses than the original's. Not exercised by any case.
- D2-D4: INT 13h/INT 11h in `src/bios.s`, patched messages built in a buffer, the format table in the rewrite's segment: nothing visible.
- The Rust-emulator half runs on the 5150 board model, not an XT (the XT's Rust emulator is ai_sw_engineer#29). Not run on hardware.
- Not covered by a case: one-sided and 8-sector sources, unreadable source tracks, formatting a target, write/verify/format errors, write protection, not-ready drives, `Incompatible drive types`, one-drive machines, less memory, DOS 1.x.
