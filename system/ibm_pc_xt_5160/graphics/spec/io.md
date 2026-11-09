# GRAPHICS: I/O contract

## The transient part (`software::install`, the target's `DosCom` in `src/main.rs`)

| Access | What |
|---|---|
| CS:100h-102h | written 90h 90h 90h |
| 0000:0014h-0017h (the INT 5 vector) | read, then written 0100h, CS - 0Ah, directly (no INT 21h) |
| CS:10Ch-10Fh | written: the old INT 5 vector |
| CS:100h-335h → CS:60h-295h | read and written a word at a time, upwards |
| INT 27h | DX = 296h (`graphics_keep` in `src/resident.s`); does not come back |

`DosCom` reads and writes with the platform's `far_read16`/`far_write16` (a byte write is a word read, merge and write). The start at 335h (`resident.s`) sets SP below the top of the program's memory (PSP:0002h, at most 0FFF0h), clears `.bss` and calls `main`; nothing of it is kept.

## The resident part (`src/resident.s`; the same logic in `software::int5` for the host)

Entered by INT 5 at (PSP - 0Ah):0100h: three `NOP`s, `STI`, `JMP SHORT 114h`.

| Input or output | What |
|---|---|
| 0040:0049h | the video mode: 4, 5 (320 x 200), 6 (640 x 200); anything else goes to the vector at 10Ch with the caller's registers |
| 0050:0000h (as 0040:0100h) | read: 1 means a print is running (return at once); written 1 at the start, 0 after the print, FFh after a printer error |
| INT 10h AH = 0Dh | CX = column, DX = row, BH = the caller's; AL = the colour (0-3, or 0-1). The handler sets AH before every call |
| INT 17h AH = 0 | DX = 0, AL = the byte; AH = the status, tested with 25h; AL must be kept (spec A1) |
| registers | AX, BX, CX, DX, DS kept (pushed); SI, DI, BP, ES not touched; flags as `IRET` restores them |
| stack | the caller's: five words, then at most five words of the handler's (`CALL`s and saved DX/CX), then what INT 10h and INT 17h use |
| own data | CS:106h-113h (spec Layout); the copy is not re-entered while the status byte is 1 |

The printer stream: `0A 0D 1B 33 18`, then a line per band: (`1B 4C n1 n2` and n bytes) or nothing, then `0A 0D`; then `1B 33 24` (spec "The INT 5 handler").

## The test harness

The cases capture INT 17h inside the guest (tools/README.md): PRNCAP.COM answers INT 17h for printer 0 with AH = 90h (or 30h after `prncap e`) and keeps the bytes; PRNDUMP.COM writes them to drive B; DRAW.COM sets the mode, draws, calls INT 5 and shows 0050:0000h. Nothing in the rewrite depends on them.
