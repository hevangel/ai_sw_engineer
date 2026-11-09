# EDLIN (PC DOS 2.00) for the IBM PC XT 5160: report

Issue [hevangel/ai_sw_engineer#293](https://github.com/hevangel/ai_sw_engineer/issues/293). Run of 2026-11-08 on a Linux host.

## Steps of the title lifecycle (spec/software-lifecycle.md)

| Step | Result |
|---|---|
| 1. Original | `EDLIN.COM` from PC DOS 2.00 disk 1, 4,608 bytes, SHA-256 `d4ea0212...` ([provenance](../original/provenance.md)); not committed |
| 2. Original on the emulators | MartyPC `05c0d088` IBM 5160 (256 KiB, XT BIOS 1982-11-08): 14 transcripts ([oracle](../oracle/README.md)); our Rust emulator (5150 board, `Machine::new(256)`): **identical to MartyPC on all 14** |
| 3. Source | Microsoft's MS-DOS 2.0 EDLIN.ASM, EDLPROC.ASM, EDLMES.ASM (MIT), revision 14; the binary is revision 12 ([reference](../reference/ms-dos-2.0/README.md), with the differences) |
| 4. Documents | the source and the binary are the documents used; no manual page fetched |
| 5. Reverse engineering | [spec](../spec/spec.md) and [I/O](../spec/io.md) from the binary read in full with GNU objdump against the source (REA needs a Ghidra-class provider, not installed here) |
| 6. Rust | `src/lib.rs` (`no_std` core over a `Dos` trait: the original's routines, DOS calls and data at the original's offsets of the PSP's segment), `src/main.rs` (the target), `src/start.s` and `scripts/mkexe.py` (the load-high program, spec D1), `src/edlin.s` (INT 21h with Ctrl-C caught, INT 23h/24h, INT 20h, string instructions on the PSP's segment) |
| 7-8. Four-way matrix | identical on all 14 cases, below |

## The matrix

Compared ([test plan](../plans/testplan.md)): the screen rows from the command line to the next row that is only `A>` (each case's TYPE of the files EDLIN wrote is inside), or, for the two cases whose output scrolls the command away (e06, e08), the oracle's rows through its final `A>` against as many of the run's.

| | MartyPC IBM 5160 (XT BIOS, PC DOS 2.00, 256 KiB) | Rust emulator (5150 BIOS 04/24/81, PC DOS 2.00, `Machine::new(256)`) |
|---|---|---|
| original EDLIN.COM | the oracle (14 transcripts) | 14 of 14 identical (`tests/rust_emulator.rs`) |
| Rust EDLIN.COM | 14 of 14 identical (`capture_dos.sh` with `PROGRAM=`, `compare.py`) | 14 of 14 identical, and the same screen and the same files on drive A as the original there on all 14 |

The host replay (`tests/oracle_replay.rs`: the core against a model of the oracle machine's DOS session, with the PSP's segment as EDLIN finds it) is 14 of 14 with `XT_ROMS` (13 without: e09 needs COMMAND.COM's bytes), plus seven tests from the spec (an empty file, M without a destination, T of a file without ^Z, `/B`, ^V, Ctrl-C at the prompt, E with 200 bytes of memory). `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are clean.

## Where e09's `[` comes from

Case e09 merges OTHER.TXT (`one`, `two`, ^Z) into FRUIT.TXT with T and writes it with E; TYPE then shows `[` after the last line. The original's MERGE (revision 12, BBAh) drops the last byte the read gave (the ^Z) and moves the rest of the text down after the merged lines without storing a ^Z at the new end (revision 13 of Microsoft's source fixed both); E writes the byte at ENDTXT as the file's last byte, so FRUIT.TXT ends `elderberry` CR LF `[`. That byte is at offset 156Ch of EDLIN's segment (START + 51), which EDLIN never wrote: it is byte 212Ch of COMMAND.COM, which DOS loaded at 0534h:0100h during the boot (EDLIN's PSP is at 0600h). Both emulators boot to the same memory there, so the original shows `[` on both.

A `.COM` rewrite cannot reproduce it: the rewrite's 10 KiB of code would be loaded over that memory (the original's file and data end below START, 1539h; 5,177 bytes). The rewrite is therefore shipped as a load-high `.EXE` named EDLIN.COM (spec D1): PC DOS 2.00 runs it by its `MZ` signature and, with MINALLOC = MAXALLOC = 0, places it at the top of memory (3CB6h:0000h), leaving the PSP's segment as the boot left it; the core keeps the original's data group and text there at the original's offsets, so the merge leaves the same byte at the same place and E writes `[` too.

## Commands

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR MARTY_TARGET_DIR=DIR system/ibm_pc_xt_5160/edlin/scripts/run_all.sh
```

## Size

`build/edlin.com`: 12,156 bytes (the original: 4,608), SHA-256 `d41ee8c711437f6ddd21354206e819f725a06c2ea433c8e3d10674eedb8654dd`: a 32-byte MZ header, code 9,649 bytes, read-only data 415 (the messages), data 2, then `.bss` (10 bytes) and the 2 KiB stack as zeros. In memory 12,124 bytes at the top; the text has the original's memory (START 1539h to PSP word 6 - 1, FFEFh with 256 KiB). Built by the msp430 Rust route and `llvm-i8086`.

## Limits and deliberate differences

- D1: a load-high `.EXE` named EDLIN.COM, the original's data and text in the PSP's segment at the original's offsets. The text memory and its contents are the original's; below START the segment holds what the boot left instead of EDLIN's code, seen only by APPEND's backward scan for a line longer than all memory.
- D2-D5 (spec): INT 21h, the vectors and the string instructions in `src/edlin.s` with Ctrl-C as a return through the core; the original's scalars and stack outside the segment (CHECK_END's look at the stack byte below START is "not LF"); ONE4TH and THREE4TH for a new file as the original's file bytes; messages from the rewrite's memory. Nothing visible.
- Emulator findings: none. The original on the Rust emulator agrees with MartyPC on every case, including e09's byte of COMMAND.COM left in memory by the boot.
- The Rust-emulator half runs on the 5150 board model, not an XT (the XT's Rust emulator is ai_sw_engineer#29). Not run on hardware.
- Not covered by a case: W and A as commands, files larger than memory, a name with a directory (CHDIR, INT 24h), `Disk full`, critical errors, `/B` and ^V on an emulator, the templates' function keys, DOS 1.x.
