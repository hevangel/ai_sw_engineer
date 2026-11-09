# GRAPHICS test plan

The oracle is the original `GRAPHICS.COM` of PC DOS 2.00 run by PC DOS 2.00 on MartyPC's IBM 5160 (the 1982-11-08 XT BIOS, 256 KiB, CGA), captured by `scripts/capture.sh` (which runs `../scripts/capture_dos.sh` with `SAVE_B=1`) → `oracle/transcripts/`. Neither emulator hands over what a graphics print sends to the printer (MartyPC keeps its parallel port private; 86Box's text printer drops bit images), so every case captures INT 17h inside the guest with this repository's tools ([tools/README.md](../tools/README.md)): PRNCAP.COM stays resident and keeps the bytes, DRAW.COM draws a fixed picture in a video mode and calls INT 5, PRNDUMP.COM writes the bytes to `OUT.PRN` on drive B. Drive A is PC DOS 2.00 disk 1 plus the three tools (`scripts/cases.py` copies them from `build/tools`); drive B is a blank 360 KiB disk labelled `PRINTOUT`. Every level builds the disks the same way (the tests with the pcdisk library, `tests/common/mod.rs`; MartyPC with the pcdisk tool). The output of `cases.py` is split on `\n` only.

## What is compared

- The screen: all 25 rows, right-trimmed (`compared` in `tests/common/mod.rs`, `scripts/compare.py`). Each case ends with DRAW setting mode 3, which clears the screen, so what is compared is DRAW's status line (`INT 5 done, status XXh`), PRNDUMP's command line and its count, and the prompts: nothing of the boot (whose time differs between emulators) is left.
- The printed bytes: `OUT.PRN` read from drive B after the run (MartyPC: the saved image and `pcdisk extract`, written by `capture.sh` as `# printed: <n> bytes sha256 <hex>`; the Rust emulator: the image read back from its diskette controller and the pcdisk library; the host: the bytes the model printed), length and SHA-256.

| Case | Command lines | What it checks (spec section) |
|---|---|---|
| g01 | `prncap`, `graphics`, `draw 6`, `prndump b:out.prn` | 640 x 200 turned: bands of 8 columns from the right, the last row with a dot, 4 dot columns a row, empty bands, edge columns 639 and 0 and rows 0 and 199 |
| g02 | the same with `draw 4` | 320 x 200: bands of 4 rows, the last column with a dot, all four colours (3 dot columns, 2 pins a pixel), the band's last row alone (E8), empty bands |
| g03 | `draw 5` | mode 5 is printed as mode 4 |
| g04 | `draw 3` | a text mode goes to the old vector: the BIOS prints 25 rows of 80 characters (0 as a space) |
| g05 | `draw 1` | 40 columns |
| g06 | `prncap`, `draw 6`, `prndump` (no GRAPHICS) | the BIOS's own Print Screen in 640 x 200: its character recognition (`GRAPHICS` found, other cells spaces) |
| g07 | the same with `draw 4` | in 320 x 200, 40 columns |
| g08 | `graphics` twice | a second copy in front of the first; one print |
| g09 | `draw 6b` (status byte 1 first) | a running print screen: nothing printed, the status byte left at 1 |
| g10 | `prncap e` (status 30h) | a printer error: one byte, status FFh |
| g11 | `prncap`, `draw 3`, `prndump` (no GRAPHICS) | the BIOS's text print without GRAPHICS: the same bytes as g04 |

`tests/oracle_replay.rs` also checks, from the spec, what no case shows on its own: the install (the NOPs, the vector, the move, 296h kept), text modes 0-3 chained, one pixel in 320 x 200 and in 640 x 200 (the exact stream), the colour bits of a column, the busy status, each error bit of 25h and the bits that are not errors.

`tests/tools.rs` checks the tools (tools/README.md): DRAW's CGA memory is the documented picture (modes 1, 3, 4, 5, 6, stopped at the BIOS's INT 5), PRNCAP keeps exactly what the emulator's printer takes from the BIOS Print Screen, the 96 KiB buffer and the lost count across 64 KiB, the second PRNCAP, an empty dump, the messages.

| Level | Runs | Expected | Command |
|---|---|---|---|
| Host | `install` and the `int5` model in front of a model of the machine (spec H1-H4), DRAW's picture from `common::picture` | the oracle's screens and printed bytes (without `XT_ROMS`: the graphics cases' bytes and counts are not compared, the font being IBM's) | `cargo test` |
| Original × Rust emulator | the original under PC DOS 2.00 on `system/ibm_pc/emulator` (5150 board, `Machine::new(256)`, no printer adapter) | the oracle | `cargo test --release --test rust_emulator -- --ignored` |
| Rewrite × Rust emulator | `build/graphics.com` as GRAPHICS.COM there; also its layout (`the_build_has_the_layout_of_the_spec`) | the same | same |
| Rewrite × MartyPC | `build/graphics.com` as GRAPHICS.COM on the same disks | the same | `scripts/capture.sh` with `PROGRAM=`, then `compare.py` |
| Original × MartyPC | the oracle itself | — | `scripts/capture.sh` |
| Tools | `tests/tools.rs` on the Rust emulator | the documented picture and the emulator's printer | `cargo test --release --test tools -- --ignored` |

Not covered by a case: Shift-PrtSc from the keyboard (every case calls INT 5 from DRAW), a real printer's busy wait, a graphics mode set by a program that does not update 0040:0049h, the rewrite and the original installed together (each is a plain INT 5 handler with the same chaining, so it would work, I).
