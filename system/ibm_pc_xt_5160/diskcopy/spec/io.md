# DISKCOPY: I/O contract

| Channel | Call | Use |
|---|---|---|
| DOS version | INT 21h AH=30h | AL below 2: `Incorrect DOS version`, INT 20h |
| Drive check | AX at entry | AL or AH = FFh (COMMAND found an invalid drive in parameter 1 or 2): `Invalid drive specification`, INT 20h |
| FCBs, tail | PSP:5Ch, 5Dh; 6Ch, 6Dh; 80h, 81h | the drives of parameters 1 and 2 (0: the default); a name in either is `Invalid parameter`; `/1` in the tail |
| Drives | INT 11h (bits 7-6), INT 21h AH=19h | the number of diskette drives (one: A to A); the default drive for a parameter without one |
| Memory | PSP:0002h, CS | the original: the track buffer from ((CS + 9Ah) AND FFE0h) + 20h to PSP:0002h; the rewrite: by the same rule from the end of its stack (spec D1), to PSP:0002h |
| Diskette | INT 13h AH=00h | reset after every failed call (DL the drive) |
| | INT 13h AH=02h, 03h, 04h | read, write, verify AL sectors from CL of track CH, head DH, drive DL at ES:0000 (one track, or two calls for a track split at a 64 KiB boundary); one sector (9 of track START) for the drive checks |
| | INT 13h AH=05h | format a track of the target: ES:BX = nine (C, H, R, N) entries, AL = 9 (8) |
| Output | INT 21h AH=09h | every message (`$`-terminated) |
| Input | INT 21h AX=0C01h | the key after each `Strike any key`, `Correct, then strike any key` and `Copy another (Y/N)?` (flushes the type-ahead; echoes) |
| Exit | INT 20h | no exit code (the rewrite: from PSP:0000, spec D1) |

The target build (`src/main.rs`) is a `.COM` (`link_com.ld`, `i8086-ld --com`) with `src/start.s` in place of `crt0_com.s` (the stack after `.bss`, spec D1) and the shared `dos.s`; `src/bios.s` makes INT 13h (`diskcopy_int13`: AX, CX, DX, ES, BX from a block; 0 or 100h + AH) and INT 11h. The program is 3,526 bytes, with 2 bytes of `.bss` and a 1 KiB stack after it.

## Memory map on the oracle machine (256 KiB, PSP at 0600h)

| Paragraphs | Original | Rewrite |
|---|---|---|
| 0600h-060Fh | PSP; the stack below 0100h | PSP |
| 0610h-06A8h | the file (2,444 bytes) | the file (3,526 bytes, to 06ECh), `.bss`, the stack (to 072Ch) |
| BUF + *k* x 120h | track buffer *k*, BUF = 06A0h (0-39 used; 51 counted) | BUF = 0740h (0-39 used; 50 counted) |
| 4000h | PSP:0002h, the top of memory | the same |
