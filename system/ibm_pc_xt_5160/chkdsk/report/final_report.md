# CHKDSK (PC DOS 2.00) for the IBM PC XT 5160: report

Issue [hevangel/ai_sw_engineer#287](https://github.com/hevangel/ai_sw_engineer/issues/287). Run of 2026-11-07 on a Linux host.

## Steps of the title lifecycle (spec/software-lifecycle.md)

| Step | Result |
|---|---|
| 1. Original | `CHKDSK.COM` from PC DOS 2.00 disk 1, 6,400 bytes, SHA-256 `89f0a7bb...` ([provenance](../original/provenance.md)); not committed |
| 2. Original on the emulators | MartyPC `05c0d088` IBM 5160 (256 KiB, XT BIOS 1982-11-08): 15 transcripts ([oracle](../oracle/README.md)); our Rust emulator (5150 board, `Machine::new(256)`, SW2 for 256 KiB): **identical to MartyPC on all 15** |
| 3. Source | Microsoft's MS-DOS 2.0 CHKDSK.ASM, CHKPROC.ASM, CHKMES.ASM (MIT), "Version 2.30" ([reference](../reference/ms-dos-2.0/README.md)) |
| 4. Documents | the source and the binary are the documents used; no manual page fetched |
| 5. Reverse engineering | [spec](../spec/spec.md) and [I/O](../spec/io.md) from the binary read in full with GNU objdump against the source (REA needs a Ghidra-class provider, not installed here); the IBM/source differences are listed in the spec |
| 6. Rust | `src/lib.rs` (`no_std` core over a `Dos` trait: the original's routines, state and arithmetic), `src/main.rs` (INT 21h, DPB far access, `.COM`), `src/disk.s` (INT 25h/26h) |
| 7-8. Four-way matrix | identical on all 15 cases, below |

## The matrix

What is compared ([test plan](../plans/testplan.md)): the rows from the case's command line to the next `A>`, or, where the command line has scrolled off (c08, whose `/F` run is followed by `chkdsk b:/v` on the fixed disk), the screen from the top through the final `A>` against as many last rows of the run. c15 (`b:`, `a:chkdsk` at `B>`, `a:`) is compared from after `A>b:` to the `A>` at its end.

| | MartyPC IBM 5160 (XT BIOS, PC DOS 2.00, 256 KiB) | Rust emulator (5150 BIOS 04/24/81, PC DOS 2.00, `Machine::new(256)`) |
|---|---|---|
| original CHKDSK.COM | the oracle (15 transcripts) | 15 of 15 identical (`tests/rust_emulator.rs`) |
| Rust CHKDSK.COM | 15 of 15 identical (`capture_dos.sh` with `PROGRAM=`, `compare.py`) | 15 of 15 identical (`tests/rust_emulator.rs`) |

The host replay (`tests/oracle_replay.rs`: the core against a DOS 2.00 model over the case images, with the oracle machine's top of memory 4000h and segment 0600h, COMMAND's drive changes and prompts) is also 15 of 15, with and without `XT_ROMS`, plus a test from the spec (a bad switch; `/F` writing the fixes of INVALID.IMG to both FATs and the entries, after which a second run reports nothing).

### History of the two earlier differences

- The Rust emulator first gave the memory lines as `65536 bytes total memory` / `40960 bytes free`: CHKDSK prints PSP:0002h x 16 and (PSP:0002h - CS) x 16, DOS takes the top of memory from the BIOS, and the 5150 BIOS 04/24/81 (E3F8h-E422h) adds SW2 (port C bits 3-0) x 32 KiB to the 64 KiB planar, which the emulator read as 0. The emulator now presents SW2 for its RAM size (`Board.sw2`, 6 for 256 KiB; spec E-A12 of the emulator), and the cells agree.
- The first c15 ran `chkdsk` on drive A, which holds CHKDSK.COM itself, so the larger rewrite (25 clusters instead of 13, spec D5) changed the counts. c15 now checks drive B as the default drive with the program run from A, and the rewrite's size no longer reaches any case.

## Commands

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR system/ibm_pc_xt_5160/chkdsk/scripts/run_all.sh
```

## Size

`build/chkdsk.com`: 12,534 bytes (the original: 6,400), SHA-256 `39b749f2e762996ffd18772fcd2afcd2b223aa393bd0310fba0c362ac083f556`: code 10,600 bytes, read-only data 1,934 (the messages); `.bss` 12,360 bytes (the 12,288-byte FAT/map/sector buffer, the Ctrl-Break state, the DPB pointer); 24,894 of 65,024 bytes of the segment. Built by the msp430 Rust route and `llvm-i8086`.

## Limits and deliberate differences

- D5: the build is 12,534 bytes, the original 6,400; a disk holding it in place of CHKDSK.COM has 12 clusters less free on PC DOS 2.00 disk 1. No case checks that disk.
- D1: one 12,288-byte buffer for the FAT, the cluster map and a sector (any FAT12 disk with 512-byte sectors fits); no STACKLIM check in the recursion. D2-D3, D6-D7: the DTA set before each search, INT 25h/26h in `src/disk.s`, out-of-line INT 21h wrappers, HAVFIX not kept: nothing visible.
- D4: Ctrl-Break restores the user's directory and drive as the original does; INT 24h is not hooked, so an Abort on a critical error leaves the target drive selected (the original selects the user's drive first). Not exercised.
- The Rust-emulator half runs on the 5150 board model, not an XT (the XT's Rust emulator is ai_sw_engineer#29). Not run on hardware.
- Not covered by a case: `.`/`..` damage and recovery, directory conversion, `Probable non-DOS disk`, disk errors, bad sectors, too few root entries for the recovered chains, `File not found`, a switch character other than `/`, Ctrl-Break and critical errors.
