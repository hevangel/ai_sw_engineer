# PRINT (PC DOS 2.00) for the IBM PC XT 5160: report

Issue [hevangel/ai_sw_engineer#297](https://github.com/hevangel/ai_sw_engineer/issues/297). Run of 2026-11-09 on a Linux host.

## Steps of the title lifecycle (spec/software-lifecycle.md)

| Step | Result |
|---|---|
| 1. Original | `PRINT.COM` from PC DOS 2.00 disk 1, 4,608 bytes, SHA-256 `a437d515...` ([provenance](../original/provenance.md)); not committed |
| 2. Original on the emulators | 86Box `a1e92633` IBM PC/XT with the original XT BIOS, 256 KiB, the Generic Text Printer on LPT1: 10 transcripts with the printed lines ([oracle](../oracle/README.md)); MartyPC is not used (its parallel port is private). Our Rust emulator (5150 board, `Machine::new(256)`, `attach_printer`, the same disk): **identical to 86Box on all 10**, screen and printed lines |
| 3. Source | none known to be published |
| 4. Documents | no manual page fetched; the binary is the document used; 86Box's `src/printer/prt_text.c` read for the printed lines |
| 5. Reverse engineering | [spec](../spec/spec.md) and [I/O](../spec/io.md) from the 4,608 bytes read in full with GNU objdump (REA needs a Ghidra-class provider, not installed here) |
| 6. Rust | `src/lib.rs` (`no_std`: the transient part over a `Dos` trait; `resident`, the resident logic for the host), `src/resident.s` (the resident part, spec D1-D2), `src/main.rs` (the target's INT 21h/2Fh/27h calls) |
| 7-8. Four-way matrix | identical on all 10 cases, below |

## The matrix

What is compared ([test plan](../plans/testplan.md)): the screen rows from the case's first command line through the screen's last `A>`, and every printed line (`#lpt|`; on the Rust emulator and the host the bytes the printer took, rendered as 86Box's text printer lays out its pages).

| | 86Box IBM PC/XT (XT BIOS, PC DOS 2.00, text printer) | Rust emulator (5150 BIOS, PC DOS 2.00, 256 KiB, printer at 378h) |
|---|---|---|
| original PRINT.COM | the oracle (10 transcripts) | 10 of 10 identical (`tests/rust_emulator.rs`) |
| Rust PRINT.COM | 10 of 10 identical (`capture_dos.sh` with `ORACLE=86box PROGRAM=`, `compare.py`) | 10 of 10 identical (`tests/rust_emulator.rs`) |

The host replay (`tests/oracle_replay.rs`: `run` and the `resident` model in front of a model of DOS 2.00 and COMMAND over the cases' pcdisk images, the background printing tick by tick, spec H1-H4) is also 10 of 10, with and without `XT_ROMS`, plus the renderer's own test and 9 tests from the spec of what no case shows (a second run, `/T`, `/C` before and after a name and the queue's reordering, the full queue, the column and a file without ^Z, a name that is no device, the LPT/COM tables and the INT 17h/14h/5 answers, a read error on the printer, the version check). `the_build_has_the_layout_the_host_replay_models` checks that `build/print.com` holds the bytes of the host's program image at 100h-52Ah and A66h-BF9h, and its handlers where the host puts them.

On the Rust emulator the original and the rewrite sent the printer the same number of bytes in every case (24, 83, 25, 1,821, 22, 83, 0, 0, 24, 24), which are the files' bytes to ^Z, tabs expanded, and one form feed per file (p03: the device PRN queued as a file gives only its form feed). No emulator change was needed, and the Rust emulator agrees with 86Box on the original in every case: its timer and DOS idle calls drive the background printing to the same byte stream; only how long it takes differs, which no case looks at.

## Commands

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR XT86_ROMS=DIR BOX86=FILE MARTY_TARGET_DIR=DIR CARGO_TARGET_DIR_LD=DIR \
  system/ibm_pc_xt_5160/print/scripts/run_all.sh
```

## Size

`build/print.com`: 6,335 bytes (the original: 4,608), SHA-256 `656ac715bcf2d54983950eebfd17e7cfe33464fb9365c261cd8051d9b5d0aa5b`. The resident part is the original's size: PSP:0000h-0BF9h (INT 27h with DX = 0BFAh), with the data at the original's offsets (100h-4FBh, the queue at A66h-BF9h), the rewrite's variables at 4FCh-52Ah and its handlers at 52Bh-A5Ah (1,328 bytes of code). The transient part (freed by INT 27h) is 2,966 bytes of code from BFAh (the start, the INT 2Fh/27h helpers, `dos.s`, the Rust core) and 559 bytes of messages and tables; no `.bss`. Built by the msp430 Rust route and `llvm-i8086`.

## Deliberate differences and limits

- D1: the resident part is assembly (`src/resident.s`), written from the specification: interrupt handlers that keep every register, switch to their own stack, chain with `JMP FAR`, call the device driver with `CALL FAR` and live at fixed offsets below the transient code cannot be compiled Rust here (no inline assembly; compiled code needs DS = SS = CS and a frame). The same logic is `software::resident` in Rust, which the host replay runs.
- D2: the data are at the original's offsets; the code from 4FCh and the variables there (request header, driver pointer, saved INT 5/13h/14h/17h vectors, printer and port numbers) are the rewrite's own, at its own offsets. The INT 2Fh handler's first 13 bytes, which a later run compares to find the resident copy, come out byte for byte the same as the original's, so the original and the rewrite should use each other's resident copy (not run; no case mixes them).
- D3: the transient part is Rust with its own start at BFAh; its buffers are Rust locals; the messages, their order and the INT 27h size are the original's.
- Found in the binary, not run on the original: PRINT 2.00 has no `/D:` (p03 shows `/d:prn` as switch `d` and the name `:prn`, the device PRN queued); a switch applies to the name before it; the INT 2Fh handler takes any AL and is not chained; the INT 24h message adds the drive to the `A` of its FAT message and never puts it back; the column is not reset by LF; a file without ^Z prints the zeros padding its last record.
- The printed lines on the Rust emulator and the host are a rendering of the bytes, from 86Box's `prt_text.c` (spec H4): it assumes no INIT pulse after the first page and no 5-second pause inside the stream, true of every case.
- The Rust-emulator half runs on the 5150 board model, not an XT (the XT's Rust emulator is ai_sw_engineer#29). Not run on hardware.
- Not covered by a case (host tests only, or not at all): the cancel switches and a second run, a busy or off-line device, INT 24h during a read, COM devices, the INT 13h/17h/14h/5 hooks against another program.
