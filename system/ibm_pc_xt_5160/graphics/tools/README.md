# GRAPHICS capture tools

This repository's own DOS programs (not IBM's), written for the GRAPHICS cases because neither emulator hands over what a graphics print sends to the printer: MartyPC keeps its parallel port private and 86Box's text printer drops bit images. They capture INT 17h inside the guest instead, so the same bytes are seen on every emulator. Each is one assembly file, assembled by llvm-i8086's `llvm-mc` and linked as a `.COM` (loaded at 100h) with the platform's `link_com.ld` by `i8086-ld --bin`:

```bash
CARGO_TARGET_DIR_LD=DIR system/ibm_pc_xt_5160/graphics/tools/build.sh   # → graphics/build/tools/*.COM
```

`scripts/cases.py` copies the built files into its directory, so `capture_dos.sh` adds them to drive A like any case file; `scripts/capture.sh` builds them first. Their tests are `tests/tools.rs` (on the Rust emulator, below).

| File | Bytes | SHA-256 (the oracle's capture) |
|---|---:|---|
| PRNCAP.COM | 415 | `c998fe7f6d2bdaec4f3aa65507295ee64cc7f26f1ea4fc1cd28eb8c5c10461be` |
| PRNDUMP.COM | 497 | `9576e0f64ca39ef9ea8daa6a9e8029eb5107b4960895b8ee8dacf32967b247d2` |
| DRAW.COM | 886 | `dd30ec2d47bb95fe5b6e4dcba880108ab0cbdd4156d75492f0a49860cde19f42` |

## PRNCAP.COM [e]

Stays resident (INT 21h function 31h) with a 96 KiB buffer (the paragraphs after its 1E0h-byte resident code; 1E0h + 18000h bytes kept) and hooks INT 17h:

| Call | Answer |
|---|---|
| DX = 0, AH = 0 | AL stored at the next byte of the buffer (or, once 98,304 bytes are stored, counted as lost); AH = the status, AL kept |
| DX = 0, AH = 1 or 2 | AH = the status |
| AH = E3h, BX = 5043h, any DX | the private function: BX = 4350h, ES = the buffer's segment, DX:AX = the bytes stored, SI:CX = the bytes lost; AL = 1 empties the buffer first |
| anything else | the INT 17h vector found at the start |

The status is 90h (not busy, selected); with `e` anywhere in the command tail it is 30h (busy, selected, out of paper), which GRAPHICS and the BIOS see as an error (25h). It shows `PRNCAP installed`; if the private function answers already it shows `PRNCAP already installed` and ends; `PRNCAP: not enough memory` if the buffer does not fit below PSP:0002h.

## PRNDUMP.COM file

Finds the buffer with the private function (`PRNCAP is not installed` if nothing answers), creates `file` (function 3Ch; `PRNDUMP: cannot create the file`), writes the bytes in pieces of at most 32 KiB (function 40h, DS stepped 800h paragraphs a piece; `PRNDUMP: write error`), closes it, empties the buffer, and shows `<n> bytes`, then ` (<m> lost)` if any were lost, CR LF. No name: `usage: PRNDUMP file`.

## DRAW.COM mode[b]

`mode` is a digit 0-6 (else `usage: DRAW mode (0-6), then b for busy`). It sets the mode (INT 10h 00h), draws the picture below, sets the status byte 0050:0000h to 1 if `b` follows the digit, calls INT 5, sets mode 3 (which clears the screen) and shows `INT 5 done, status XXh` with the status byte in hex.

The graphics picture (modes 4, 5: W = 320; mode 6: W = 640, x* = 2x, a colour that is not 0 drawn as 1; else x* = x), written into the CGA memory at B800h (even rows at 0000h, odd at 2000h, 80 bytes a row):

| | What | Exercises |
|---|---|---|
| E1 | corners (0, 0) colour 1, (W-1, 0) 2, (0, 199) 3, (W-1, 199) 1 | the first and last band of each mode reach the edges |
| E2 | bars of colour c = 0-3: x (8+20c)* to (24+20c)* (excluded), rows 8-23 | each colour in all four rows of four bands (320 x 200) |
| E3 | row 30, x 1 to W/2 (excluded), colour 3 | every bit position of the 640 x 200 bytes; a lone row in a band |
| E4 | rows 40-159: x = (2(y-40))*, colour 1 + y mod 3 | colours changing row by row; a growing band length |
| E5 | rows 40-159: x = W/2 + (y-40), colour 2 | one pixel a row through every bit position |
| E6 | box outline x 200* to 300* (included), rows 100-150, colour 2 | |
| E7 | block x 220* to 240*, rows 120-129, colour 3 | |
| E8 | (250*, 163) colour 1 | the band 160-163 decided by its last row alone |
| E9 | `GRAPHICS` written by the BIOS (INT 10h 0Eh, BL = 3) from text row 21, column 1 | the BIOS Print Screen's character recognition (rows 168-175) |

Rows 4-7 and 176-195 (320 x 200) and the bands of columns 608-631 (640 x 200) stay empty. The text picture (modes 0-3, C = 40 or 80 columns): row 0 `DRAW text mode m`, from row 2 the characters 0-255 in order, C to a row, row 24 `last row` ending at column C-1, every attribute 07h.

## Tests (`tests/tools.rs`, on the Rust emulator)

`IBM_PC_ROMS=DIR XT_ROMS=DIR cargo test --release --test tools -- --ignored --nocapture`:

- `draw_leaves_the_documented_picture`: DRAW in modes 1, 3, 4, 5, 6 stopped at the BIOS's INT 5 entry (F000:FF54h); the 16 KiB of CGA memory equal `common::picture` (the table above in Rust; E9 from the ROM font).
- `prncap_keeps_what_a_printer_takes`: the BIOS's Print Screen of the text picture sent to the emulator's printer adapter, and the same with PRNCAP and PRNDUMP, are the same 2,052 bytes; nothing reaches the printer while PRNCAP is installed.
- `the_buffer_holds_96_kib_and_counts_the_rest`: a second PRNCAP; one graphics print (39,972 bytes) dumped; three more fill the buffer: `98304 bytes (21612 lost)`, the file the first 98,304 bytes of the three prints; then an empty dump.
- `the_messages`: PRNDUMP without PRNCAP and without a name, DRAW with mode 9 and with none.

`scripts/decode.py` turns captured bytes into printer lines (and a PBM picture of the bit images) for reading.
