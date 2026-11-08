# DISKCOMP: I/O contract

| Channel | Call | Use |
|---|---|---|
| Drive check | AX at entry | AL or AH = FFh (COMMAND found an invalid drive in parameter 1 or 2): `Invalid drive specification`, INT 20h (before the version check) |
| DOS version | INT 21h AH=30h | AL below 2: `Incorrect DOS version`, INT 20h |
| FCBs, tail | PSP:5Ch, 5Dh; 6Ch, 6Dh; 80h, 81h | the drives of parameters 1 and 2 (0: the default); a name in either is `Invalid parameter`; `/1` and `/8` in the tail |
| Drives | INT 11h (bits 7-6), INT 21h AH=19h | the number of diskette drives (one: A against A); the default drive for a parameter without one |
| Memory | PSP:0002h, CS | the original: BUF = ((CS + 81h) AND FFE0h) + 20h, two tracks; BUF2 = BUF + 240h (200h for 8 sectors) to PSP:0002h; the rewrite: by the same rule from the end of its stack (spec D1) |
| Diskette | INT 13h AH=02h | read one sector (9 of track 0, head 0) for the drive checks; read AL sectors from CL of track CH, head DH, drive DL to ES:0000 (one track, or two calls for a track split at a 64 KiB boundary), BX = 0 |
| | INT 13h AH=00h | reset after a failed track read (not after a failed check) |
| Compare | `REPZ CMPSB` | DS:SI = BUF (as an offset in the program's segment) against ES:0000 = the big buffer's cylinder, 2 x 1200h (1000h) bytes |
| Output | INT 21h AH=09h | every message (`$`-terminated) |
| Input | INT 21h AX=0C01h | the key after each `Strike any key`, `Correct, then strike any key` and `Compare more diskettes (Y/N)?` (flushes the type-ahead; echoes) |
| Exit | INT 20h | no exit code (the rewrite: from `start.s` after `main`, spec D1) |

The target build (`src/main.rs`) is a `.COM` (`link_com.ld`, `i8086-ld --com`) with `src/start.s` in place of
`crt0_com.s` (the stack after `.bss`, spec D1) and the shared `dos.s`; `src/bios.s` makes INT 13h
(`diskcomp_int13`: AX, CX, DX, ES from a block, BX = 0; 0 or 100h + AH), INT 11h and the compare
(`diskcomp_compare`: SI, ES, CX; 0 if equal, else CX after the differing byte + 1). The program is 3,306 bytes
(code 2,792, messages and constants 514), with 2 bytes of `.bss` and a 1 KiB stack after it. It must stay between
2,049 and 5,120 bytes (3 to 5 clusters) so that a rewrite run's disks differ in the original's tracks
(plans/testplan.md).

## Memory map on the oracle machine (256 KiB, PSP at 0600h)

| Paragraphs | Original | Rewrite |
|---|---|---|
| 0600h-060Fh | PSP; the stack below 0100h | PSP |
| 0610h-0691h | the file (2,074 bytes) | the file (3,306 bytes, to 06DEh), `.bss`, the stack (to 071Eh) |
| BUF, 2 tracks | 06A0h-08DFh (offset A00h) | 0720h-095Fh (offset 1200h) |
| BUF2 + *k* x 120h | 08E0h: up to 40 tracks of a pass; CAP = 48 tracks (24 cylinders two-sided) | 0960h; CAP = 48 |
| 4000h | PSP:0002h, the top of memory | the same |
