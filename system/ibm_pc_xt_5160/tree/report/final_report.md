# TREE (PC DOS 2.00) for the IBM PC XT 5160: report

Issue [hevangel/ai_sw_engineer#285](https://github.com/hevangel/ai_sw_engineer/issues/285). Run of 2026-11-06 on a Linux host.

## Steps of the title lifecycle (spec/software-lifecycle.md)

| Step | Result |
|---|---|
| 1. Original | `TREE.COM` from PC DOS 2.00 disk 1, 1,513 bytes, SHA-256 `132d9995...` ([provenance](../original/provenance.md)); not committed |
| 2. Original on the emulators | MartyPC `05c0d088` IBM 5160 with the original XT BIOS: 9 transcripts ([oracle](../oracle/README.md)); our Rust emulator (5150 board, 64 KiB, two drives, PC DOS 2.00 from a 180 KiB disk, the case disk of 360 KiB in drive B): **identical to MartyPC on all 9** |
| 3. Source | none known to be published |
| 4. Documents | no manual page fetched; the binary is the document used |
| 5. Reverse engineering | [spec](../spec/spec.md) and [I/O](../spec/io.md) from the 1,513 bytes read in full with GNU objdump (REA needs a Ghidra-class provider, not installed here) |
| 6. Rust | `src/lib.rs` (`no_std` core over a `Dos` trait: the original's flags, path buffer, two extended FCBs and walk), `src/main.rs` (INT 21h, `.COM`) |
| 7-8. Four-way matrix | identical on all 9 cases, below |

## The matrix

What is compared ([test plan](../plans/testplan.md)): the rows from the command line to the next `A>`, or, where the output scrolled the command line off the screen (t02-t05, t08), the screen from the top through the final `A>` against as many last rows of the run.

| | MartyPC IBM 5160 (XT BIOS, PC DOS 2.00) | Rust emulator (5150 BIOS, PC DOS 2.00) |
|---|---|---|
| original TREE.COM | the oracle (9 transcripts) | 9 of 9 identical (`tests/rust_emulator.rs`) |
| Rust TREE.COM | 9 of 9 identical (`capture_dos.sh` with `PROGRAM=`, `compare.py`) | 9 of 9 identical (`tests/rust_emulator.rs`) |

The host replay (`tests/oracle_replay.rs`: the core against a DOS model that serves searches from the cases' pcdisk images in directory order, the search position kept in the FCB) is also 9 of 9, with and without `XT_ROMS`, plus a test of the parts the scrolled cases do not show (header with and without a label, `/F` names with and without an extension, indented continuation names).

No emulator change was needed: the Rust emulator's SW1 = 6Fh already reports two drives to the 5150 BIOS, and its controller takes the 360 KiB disk in drive B.

## Commands

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR system/ibm_pc_xt_5160/tree/scripts/run_all.sh
```

## Size

`build/tree.com`: 3,429 bytes (the original: 1,513), SHA-256 `11dc59effe285c45409687b86c0afca7597800e5cfa2b35a68a0c0df8586525d`: code 3,178 bytes, read-only data 251; `.bss` 16,452 bytes (the 16,384-byte path and list buffer, the Ctrl-Break state); 19,881 of 65,024 bytes of the segment. Built by the msp430 Rust route and `llvm-i8086`.

## Limits and deliberate differences

- D1: the path and name list share a 16,384-byte buffer (the original: up to PSP:0006h), so a list chunk holds about 1,480 names instead of about 5,800. Over that the original's overflow path runs, which never clears its overflow flag and so, by the disassembly, prints `None` without end; the rewrite keeps that logic. Not reached by a case.
- D2-D4 (version message by function 02h, DTA set before each search, Ctrl-Break handler in Rust) change nothing on the screen.
- Found in the binary, not run: a 12-character directory name (`ABCDEFGH.EXT`) that is not the last in the path is not crossed by the 12-byte backward separator scan; the path's depth is limited only by DOS refusing CHDIR (`Invalid path`).
- The Rust-emulator half runs on the 5150 board model, not an XT (the XT's Rust emulator is ai_sw_engineer#29). Not run on hardware.
- Not covered by a case: `Invalid path`, the list overflow, a switch character other than `/`, DOS other than 2.x, Ctrl-Break; the header row on MartyPC (scrolled off in the cases that print it).
