# DISKCOMP (PC DOS 2.00) for the IBM PC XT 5160: report

Issue [hevangel/ai_sw_engineer#291](https://github.com/hevangel/ai_sw_engineer/issues/291). Run of 2026-11-08 on a Linux host.

## Steps of the title lifecycle (spec/software-lifecycle.md)

| Step | Result |
|---|---|
| 1. Original | `DISKCOMP.COM` from PC DOS 2.00 disk 1, 2,074 bytes, SHA-256 `5881d93e...` ([provenance](../original/provenance.md)); not committed |
| 2. Original on the emulators | MartyPC `05c0d088` IBM 5160 (256 KiB, XT BIOS 1982-11-08): 8 transcripts ([oracle](../oracle/README.md)); our Rust emulator (5150 board, `Machine::new(256)`): **identical to MartyPC on all 8**, and both disks unchanged |
| 3. Source | none published |
| 4. Documents | the binary is the document used; no manual page fetched |
| 5. Reverse engineering | [spec](../spec/spec.md) and [I/O](../spec/io.md) from the binary read in full with GNU objdump, side by side with DISKCOPY 2.0 (REA needs a Ghidra-class provider, not installed here) |
| 6. Rust | `src/lib.rs` (`no_std` core over a `Pc` trait: the original's routines, state and INT 13h calls), `src/main.rs` (INT 21h, `.COM`), `src/start.s` (the stack below the buffers, spec D1), `src/bios.s` (INT 13h, INT 11h, `REPZ CMPSB`) |
| 7-8. Four-way matrix | identical on all 8 cases, below |

## The matrix

Compared ([test plan](../plans/testplan.md)): the screen rows from the command line to the next `A>`, or, for the two cases whose output scrolls the command away (k04, k05), the oracle's rows through its final `A>` against as many of the run's.

| | MartyPC IBM 5160 (XT BIOS, PC DOS 2.00, 256 KiB) | Rust emulator (5150 BIOS 04/24/81, PC DOS 2.00, `Machine::new(256)`) |
|---|---|---|
| original DISKCOMP.COM | the oracle (8 transcripts) | 8 of 8 identical (`tests/rust_emulator.rs`) |
| Rust DISKCOMP.COM | 8 of 8 identical (`capture_dos.sh` with `PROGRAM=`, `compare.py`) | 8 of 8 identical, and the same as the original there on all 8 |

The host replay (`tests/oracle_replay.rs`: the core against a model of the oracle machine's INT 13h, DOS and memory) is 8 of 8 screens identical (with `XT_ROMS`), plus five tests from the spec (bad parameters, two passes with swapped roles and differences in both, `/1`, `/8` with `Compare more`, a one-sided image with MartyPC's head-1 status) and the placement check below. `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are clean.

## Where the rewrite lands on the disks

A rewrite run has the rewrite as DISKCOMP.COM on drive A (and, by `b:copy_a`, on B), so in k04 and k05, which compare drive A with a disk without it, the tracks holding DISKCOMP.COM could have differed between the runs, and in k02 and k03 the added files could have moved. They do not: `pcdisk add` gives the 3,306-byte rewrite the original's clusters 62-64 plus cluster 155, which is in cylinder 17 head 1, a track that already differs from a blank disk in the original's run (BASICA.COM ends there), and EXTRA.TXT and BIG.BIN still start in that track. On the disks of both MartyPC runs the tracks that differ between A and B are the same in all 8 cases (k04: cylinders 0-17, 36 tracks; k05: all 80), so the reported tracks, including those scrolled off the screen, are the same. This holds for any rewrite of 2,049 to 5,120 bytes; `the_rewrite_on_the_disks_changes_no_reported_track` checks the size and the core's whole output on the rewrite's disks against the original's on every `cargo test` after `build.sh`. No change to the comparison rule was needed.

## Commands

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR MARTY_TARGET_DIR=DIR system/ibm_pc_xt_5160/diskcomp/scripts/run_all.sh
```

## Size

`build/diskcomp.com`: 3,306 bytes (the original: 2,074), SHA-256 `5673d97d7dcbfc2a4cd53120851e5eafbac0e4dce2acbaefb46d9866b93a7e11`: code 2,792 bytes, read-only data 514 (the messages); `.bss` 2 bytes and a 1 KiB stack. Built by the msp430 Rust route and `llvm-i8086`.

## Limits and deliberate differences

- D1: `src/start.s` in place of `crt0_com.s` puts the stack after `.bss`, and the buffers start after it by the original's rounding rule (BUF 0720h, BUF2 0960h for a PSP at 0600h; the original's 06A0h, 08E0h): CAP is 48 tracks on 256 KiB as in the original, so the passes and swaps are the same. With much less memory the rewrite may hold 2 tracks fewer. Other tracks are split at 64 KiB boundaries (more INT 13h calls, the same data); an unreadable track compares memory at other addresses. Not exercised by any case.
- D2-D4: INT 13h/INT 11h/`REPZ CMPSB` in `src/bios.s`, patched messages built in a buffer, INT 20h exits as returns: nothing visible.
- Emulator findings: none; the original on the Rust emulator agrees with MartyPC on every case. The known difference on head 1 of a one-sided image (MartyPC status 20h, the Rust emulator 04h; [DISKCOPY's spec](../../diskcopy/spec/spec.md)) is not reached by any case.
- The Rust-emulator half runs on the 5150 board model, not an XT (the XT's Rust emulator is ai_sw_engineer#29). Not run on hardware.
- Not covered by a case: one-sided and 8-sector disks, `/1` and `/8` on an emulator, `Incompatible diskette or drive types`, unreadable tracks, not-ready drives, one-drive machines, less memory, DOS 1.x.
