# GRAPHICS (PC DOS 2.00): behaviour specification

Derived from the original `GRAPHICS.COM` (789 bytes, SHA-256 `949803e1...`, [../original/provenance.md](../original/provenance.md)) by reading it in full disassembled with GNU objdump 2.42 (`objdump -D -b binary -m i8086 -M intel --adjust-vma=0x100 GRAPHICS.COM`). REA (spec/software-lifecycle.md) was not used: it needs a Ghidra/IDA/Hopper provider, none is installed here, and the program is 636 bytes of code and data before its text, read in full. IBM published no source. The XT BIOS's Print Screen (F000:FF54h) and character read (INT 10h 08h, F629h) were read the same way from the 1982-11-08 ROM (`--adjust-vma=0xe000 xtbios.bin`), since text modes go there. Offsets are the `.COM`'s (CS:100h is the first byte). Observations are marked (O), derivations from them (D), inferences not run (I).

GRAPHICS ("The IBM Personal Computer Graphics Printer Support Version 1.00 (C)Copyright IBM Corp 1982 Licensed Material - Program Property of IBM Author - M. Hallerman", 37Ch-414h, never shown) installs an INT 5 (Print Screen, Shift-PrtSc) handler that prints the CGA's graphics modes on an IBM Graphics Printer with bit-image escapes and sends text modes on to the handler it found. It shows nothing, takes no parameters and does not look for an installed copy.

## Layout (O)

| Offset | Size | Use |
|---|---|---|
| 100h | 3 | `JMP 335h` (E9 32 02); turned into `NOP NOP NOP` by the start before it is moved |
| 103h | 1 | `STI` |
| 104h | 2 | `JMP SHORT 114h` (EB 0E) |
| 106h-109h | 4 | the four pixels of a 320 x 200 column, 109h the band's top row, 106h its fourth |
| 10Ah | 1 | the column's first printer byte |
| 10Bh | 1 | A0h in 320 x 200, 0 in 640 x 200 |
| 10Ch | 4 | the INT 5 vector found at the start (offset, segment) |
| 110h | 2 | SP inside the handler (after its five pushes), for the error exit |
| 112h | 2 | the band's length: columns (320 x 200) or rows (640 x 200) up to the last with a dot |
| 114h-263h | | the handler: dispatch, the status byte, 320 x 200 (178h) |
| 264h-2DBh | | 640 x 200 |
| 2DCh, 2E0h | | INT 17h for one byte (AH = 0, DX = 0 set by the callers), the error test |
| 2F1h | | LF, CR |
| 302h | | `ESC L n1 n2` |
| 335h-37Bh | | the transient part |
| 37Ch-414h | | the copyright text (no `$`, never shown) |

## Installation (335h) (O)

1. DS = 0. The words at CS:100h and the byte at CS:102h become 90h (the `JMP` becomes three `NOP`s, so the moved copy starts with them).
2. The INT 5 vector (0:14h) goes to CS:10Ch; then 0:14h = 0100h, 0:16h = CS - 0Ah, written directly (no INT 21h 25h).
3. The words CS:100h-335h are copied upwards to CS:60h-295h (SI from 60h, DI from 100h while DI < 335h, signed): the copy overlaps its source and lands over the PSP's FCBs and command tail. The last word holds the start's first byte (2Bh). (D) The copy runs at (CS - 0Ah):0100h with the same offsets.
4. INT 27h with DX = SI = 296h: PSP:0000h-0295h stay resident (2Ah paragraphs).

(D) Run twice, the second copy's old vector at its 10Ch is the first copy: text modes go through both to the BIOS, graphics modes are printed by the second (the last installed) and never reach the first (case g08). A run with INT 5 at another program behaves the same.

## The INT 5 handler (114h) (O)

On the caller's stack: `PUSH AX, DS, DX, CX, BX`, DX = 0, DS = 40h, AL = the video mode (0040:0049h).

- Modes 4 and 5: BL = A0h; mode 6: BL = 0; any other mode: the registers popped, `JMP FAR CS:[10Ch]` (the BIOS Print Screen, below).
- If 0040:0100h (the BIOS's Print Screen status byte 0050:0000h) is 1, it returns (`IRET`) at once; else that byte = 1. DS = CS, [110h] = SP, [10Bh] = BL.
- LF CR (2F1h), `ESC 3 24` (line spacing 24/216 inch: the eight pins), then 320 x 200 (BL = A0h) or 640 x 200, then `ESC 3 36` (1/6 inch), status byte = DH (0), the registers popped, `IRET`.
- Every byte goes out by INT 17h function 0 with DX = 0 (printer 0; 2E0h). A status with a bit of 25h (01h time-out, 04h, 20h out of paper) ends the print: SP = [110h], DH = FFh, status byte = FFh, return (case g10: one LF reaches the printer). The byte that failed was sent; nothing more is.
- (O) The handler relies on INT 17h keeping AL (the 640 x 200 byte is sent four times, the 320 x 200 first byte twice from 10Ah); the BIOS's does (AL is restored from DX at the end of PRINTER_IO). It does not touch SI, DI, BP or ES; BH is the caller's for INT 10h (ignored by the CGA read dot).

### 320 x 200 (178h)

For each band of four rows, DX = 0, 4, ..., 196 (until DL = 200):

1. From CX = 319 down, the first column with a pixel that is not 0 in rows DX-DX+3 (INT 10h 0Dh, read from the top, up to the first that is not 0). None (column 0 included): only LF CR.
2. [112h] = that column + 1; `ESC L n1 n2` with n = [112h] x 3 (302h; DX = 0 after it).
3. For each column 0 to [112h] - 1: the four pixels to 109h-106h; the first byte has 2 bits for each (C0h for row 0 down to 03h for row 3) set where the colour is at least 2 (`CMP` with 2, `JL`: signed); the second where bit 0 of the colour is set; sent first, second, first ([10Ah]).
4. LF CR.

(D) A pixel is three dot columns by two pins: colour 2 the outer two columns, colour 1 the middle one, colour 3 all six, 0 none. 960 dot columns (ESC L, dual density) for 320 pixels; 50 lines of 8 pins.

### 640 x 200 (264h)

For each band of eight columns from the right, CX = 639, 631, ..., 7 (while CX >= 0, signed):

1. From DX = 199 down, the first row where one of the columns CX to CX - 7 has a pixel that is not 0 (from the right). None (row 0 included): only LF CR.
2. [112h] = that row + 1; `ESC L` with n = [112h] x 4.
3. For each row 0 to [112h] - 1: a byte of the eight columns, CX the most significant bit (the top pin) to CX - 7 (a pixel that is not 0: CF = 1 by `CMC`, `RCL AL, 1`), sent four times.
4. LF CR.

(D) The picture is printed turned a quarter turn: the screen's right edge along the paper's top, row 0 at the left, a pixel four dot columns wide and one pin high; 80 lines.

## The BIOS Print Screen (text modes) (O, the 1982-11-08 XT ROM)

F000:FF54h: if 0050:0000h is 1 it returns; else 1. AH = 0Fh gives the columns (CL) and page (BH); LF CR; the cursor saved; for each of 25 rows and each column: the cursor set, the character read (INT 10h 08h), 0 sent as a space, INT 17h; a status with 25h ends it (status FFh, the cursor put back); LF CR after each row (the LF CR are not tested); the cursor put back, status 0. In a graphics mode INT 10h 08h compares the 8 x 8 cell (a pixel that is not 0 a dot) with the ROM font's characters 0-127 (F000:FA6Eh) and gives the first equal one, else 0 (with no INT 1Fh table): DRAW's `GRAPHICS` comes out as text and every other cell as a space (cases g06, g07).

## What the cases printed (O, the oracle)

From `../scripts/decode.py` on the bytes the oracle's capture kept (`oracle/transcripts`, `# printed:`; the line counts count every LF and FF, so the text cases' characters 0Ah and 0Ch add two):

| Case | Bytes | Lines | Bit-image lines | First lines |
|---|---:|---:|---:|---|
| g01 mode 6 | 39,972 | 82 | 77 | `0A 0D 1B 33 18 1B 4C 20 03 80 80 80 80 00 ...`: LF CR, ESC 3 24, ESC L 800 (the band 639-632 to row 199), (639, 0) the top pin; three empty bands (CR LF); last `... 01 01 01 01 0A 0D 1B 33 24` |
| g02, g03 modes 4, 5 | 26,896 | 52 | 40 | `0A 0D 1B 33 18 1B 4C C0 03 00 C0 00 ...`: ESC L 960, (0, 0) colour 1 the middle column's top pins; rows 4-7 empty; rows 8-23 ESC L 252 (the bars to x 83); row 30 ESC L 480 |
| g04, g11 text 80 | 2,052 | 29 | 0 | LF CR `DRAW text mode 3`, 25 rows of 80, LF CR each: 2 + 25 x 82 |
| g05 text 40 | 1,052 | 29 | 0 | the same at 40 columns: 2 + 25 x 42 |
| g06 mode 6, BIOS | 2,052 | 27 | 0 | 25 rows of 80 spaces but row 21 ` GRAPHICS` |
| g07 mode 4, BIOS | 1,052 | 27 | 0 | 25 rows of 40, row 21 ` GRAPHICS` |
| g08 twice | 39,972 | | | = g01 |
| g09 busy | 0 | | | status stays 01h |
| g10 printer error | 1 | | | `0A`, status FFh |

## The rewrite

- D1: the resident handler is assembly (`src/resident.s`), written from this specification: an interrupt handler that runs in a moved copy of the program with DS = CS, on the caller's stack, and chains with `JMP FAR` from a fixed offset cannot be compiled Rust here (no inline assembly; compiled code needs DS = SS = CS, a frame and the runtime, and the transient Rust code is not kept). The same logic is `software::int5` in Rust, which the host replay runs.
- D2: the layout is the original's: the `JMP` at 100h (the same three bytes, E9 32 02), `STI` and `JMP SHORT 114h` at 103h, the data at 106h-113h, the handler at 114h, the start at 335h, the same move to 60h and INT 27h with DX = 296h. The code bytes from 114h are the rewrite's own (the handler and its helpers are 114h-2D9h, zeros to 334h); every INT 10h and INT 17h call is made in the original's order with the same registers that matter (CX, DX, AH; AL to INT 17h). The moved range's last word holds the rewrite's own start byte (FCh).
- D3: the transient part is Rust (`software::install`) with its own start at 335h (SP, `.bss`, `main`), reading and writing memory directly as the original does. 781 bytes in all (the original: 789, of which 153 bytes of copyright text the rewrite does not carry, since it is never shown).
- No difference is visible in any case: screen and printed bytes are identical on both emulators.

## Assumption ledger

| ID | Assumption |
|---|---|
| A1 | INT 17h keeps AL (the BIOS's and PRNCAP's do): the rewrite sends the 640 x 200 byte four times and the 320 x 200 first byte as the original does, without reloading AL from elsewhere (`src/resident.s`). |
| H1 | Host replay: INT 5 is dispatched by its vector: to a GRAPHICS copy that `software::install` put there (its handler `software::int5`; the program's own bytes are not modelled, only what the install reads and writes) or to the BIOS model. |
| H2 | Host replay: the BIOS Print Screen and INT 10h 08h are modelled from the 1982-11-08 XT ROM (above), with the ROM's font from `$XT_ROMS/xtbios.bin` and no INT 1Fh table; the Rust emulator's 5150 BIOS gives the same bytes in every case. |
| H3 | Host replay: INT 10h 0Dh reads the CGA memory (two bits a pixel at 320 x 200, one at 640 x 200, odd rows at +2000h, 80 bytes a row); a mode set clears it (text: 0720h words) and sets 0040:0049h/004Ah. |
| H4 | Host replay: the console after DRAW's mode set: COMMAND shows CR LF `A>` before each prompt and echoes the line with CR LF; the tools' messages are tools/README.md's; under 25 rows, no scrolling. |
