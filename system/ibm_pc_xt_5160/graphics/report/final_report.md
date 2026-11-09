# GRAPHICS (PC DOS 2.00) for the IBM PC XT 5160: report

Issue [hevangel/ai_sw_engineer#299](https://github.com/hevangel/ai_sw_engineer/issues/299). Run of 2026-11-09 on a Linux host.

## Steps of the title lifecycle (spec/software-lifecycle.md)

| Step | Result |
|---|---|
| 1. Original | `GRAPHICS.COM` from PC DOS 2.00 disk 1, 789 bytes, SHA-256 `949803e1...` ([provenance](../original/provenance.md)); not committed |
| 2. Original on the emulators | MartyPC `05c0d088` IBM 5160 with the original XT BIOS, 256 KiB: 11 transcripts with the printed bytes' hash ([oracle](../oracle/README.md)), captured in the guest by this repository's tools ([tools](../tools/README.md)); our Rust emulator (5150 board, `Machine::new(256)`, two drives, the same disks): **identical to MartyPC on all 11** |
| 3. Source | none known to be published |
| 4. Documents | no manual page fetched; the binary and the XT BIOS's Print Screen are the documents used |
| 5. Reverse engineering | [spec](../spec/spec.md) and [I/O](../spec/io.md) from the 789 bytes read in full with GNU objdump (REA needs a Ghidra-class provider, not installed here) |
| 6. Rust | `src/lib.rs` (`no_std`: `install`, the transient part, over a `Dos` trait; `int5`, the handler's logic, for the host), `src/resident.s` (the resident handler, spec D1-D2), `src/main.rs` (the target's memory access and INT 27h) |
| 7-8. Four-way matrix | identical on all 11 cases, below |

## The capture tools

PRNCAP.COM (415 bytes: resident INT 17h capture, 96 KiB buffer, a lost count, a private query function, an error-status option), PRNDUMP.COM (497 bytes: the buffer to a file in 32 KiB pieces, the count shown, the buffer emptied) and DRAW.COM (886 bytes: a documented picture in modes 0-6, INT 5, mode 3, the status byte shown), assembly built by `tools/build.sh`. Their tests (`tests/tools.rs`, 4 tests, all pass on the Rust emulator): DRAW's CGA memory in modes 1, 3, 4, 5, 6 is the documented picture; PRNCAP keeps exactly the 2,052 bytes the emulator's own printer adapter takes from the BIOS Print Screen; the buffer holds 98,304 bytes and counts 21,612 lost over three 39,972-byte prints (pieces across 64 KiB); the second install and the messages.

## The matrix

What is compared ([test plan](../plans/testplan.md)): the 25 screen rows (DRAW's status line, PRNDUMP's count, the prompts) and the printed bytes (length and SHA-256 of `OUT.PRN` read back from drive B).

| | MartyPC IBM 5160 (XT BIOS, PC DOS 2.00) | Rust emulator (5150 BIOS, PC DOS 2.00, 256 KiB) |
|---|---|---|
| original GRAPHICS.COM | the oracle (11 transcripts) | 11 of 11 identical (`tests/rust_emulator.rs`) |
| Rust GRAPHICS.COM | 11 of 11 identical (`capture.sh` with `PROGRAM=`, `compare.py`); drive B's whole-image hash also equal in all 11 | 11 of 11 identical (`tests/rust_emulator.rs`) |

| Case | Printed (all four cells) | Status |
|---|---|---|
| g01 mode 6, g08 twice | 39,972 bytes `fdf0b91f...` | 00h |
| g02 mode 4, g03 mode 5 | 26,896 bytes `f04ee971...` | 00h |
| g04 text 80, g11 text 80 without GRAPHICS | 2,052 bytes `14a8451d...` | 00h |
| g05 text 40 | 1,052 bytes `ac893df1...` | 00h |
| g06 mode 6 without GRAPHICS | 2,052 bytes `58e4d994...` | 00h |
| g07 mode 4 without GRAPHICS | 1,052 bytes `5eafc98b...` | 00h |
| g09 busy | 0 bytes | 01h |
| g10 printer error | 1 byte | FFh |

The host replay (`tests/oracle_replay.rs`: `install` and the `int5` model in front of a model of the machine, DRAW's picture, PRNCAP and the XT BIOS's Print Screen and character recognition, spec H1-H4) is also 11 of 11 with `XT_ROMS` (without it the graphics cases' bytes, which need IBM's font, are left out), plus 7 tests from the spec (the install, text modes chained, one pixel in each graphics mode, a column's colour bits, the busy status, the error bits).

No emulator change was needed, and the Rust emulator agrees with MartyPC on the original in every case: CGA memory and the BIOS's read dot, the 5150 BIOS's Print Screen and graphics character recognition (the same bytes as the XT BIOS's in g04-g07, g11), INT 27h after the move over the PSP, and the drive B writes.

## Commands

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR MARTY_TARGET_DIR=DIR CARGO_TARGET_DIR_LD=DIR system/ibm_pc_xt_5160/graphics/scripts/run_all.sh
```

## Size

`build/graphics.com`: 781 bytes (the original: 789), SHA-256 `342a22277bb6c6e37b4facf7d32169a8b54e73c636bd7a117d2ececd0f54fd40`. The resident part has the original's layout and size: 100h-335h moved to 60h-295h, INT 27h with DX = 296h; the handler and helpers are 114h-2D9h (454 bytes; the original's 114h-334h, 545 bytes), zeros to 334h. The transient part from 335h: the start and the INT 27h helper (43 bytes), `dos.s`'s helpers used (27 bytes) and the Rust core (146 bytes); no `.bss`, no text (the original's 153-byte copyright text is never shown and is not carried). Built by the msp430 Rust route and `llvm-i8086`.

## Deliberate differences and limits

- D1: the resident handler is assembly (`src/resident.s`), written from the specification; an interrupt handler running in a moved copy on the caller's stack cannot be compiled Rust here (no inline assembly, compiled code needs DS = SS = CS and the runtime, and the transient code is not kept). Its logic is `software::int5` in Rust, which the host replay runs.
- D2: the layout, the first six bytes, the data offsets, the move and the kept size are the original's; the code bytes from 114h are the rewrite's own, with every INT 10h and INT 17h call in the original's order. A program that reads GRAPHICS's code bytes would see the difference; none does.
- D3: the transient part is Rust with its own start at 335h; the copyright text is left out (it is never shown, and it is IBM's).
- Found in the binary, shown by the cases: no check for an installed copy (g08: two copies, one print); the busy test is the BIOS's status byte (g09); a printer error after the first byte (g10); INT 17h must keep AL (spec A1).
- The Rust-emulator half runs on the 5150 board model, not an XT (the XT's Rust emulator is ai_sw_engineer#29). Not run on hardware or a real Graphics Printer: the printed bytes are what INT 17h was asked to send, not what a printer would make of them.
- Not covered by a case: Shift-PrtSc from the keyboard, a real printer's timing, the rewrite and the original installed together.
- Outside this directory and not changed: `../software_catalog.md` still lists GRAPHICS.COM as "needs a printer model"; it can now say done (`graphics/`, #299).
