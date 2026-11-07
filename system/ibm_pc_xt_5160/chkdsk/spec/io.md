# CHKDSK: I/O contract

| Channel | Call | Use |
|---|---|---|
| DOS version | INT 21h AH=30h | AH.AL swapped must be 1.54-2.00, else `Incorrect DOS version` (09h), INT 20h |
| Drive check | AX at entry | AL = FFh (COMMAND found an invalid drive in parameter 1): `Invalid drive specification`, INT 20h |
| Disk reset | INT 21h AH=0Dh | at the start, before each FCB open of CHAINREC, and at the end (NOWRITE) |
| Switch character | INT 21h AX=3700h | DL; `/` makes `\` the directory separator; `/F` and `/V` are looked for after it in the tail |
| Command tail, FCB 1 | PSP:80h, PSP:81h; PSP:5Ch | the switches; FCB 1's drive is the target, its name (if any) the file check's pattern |
| Default drive | INT 21h AH=19h, AH=0Eh | saved; the target is selected; the saved one is selected again at every end |
| Drive parameter block | INT 21h AH=32h (DL: 1 = A) | DS:BX kept; read: sector size (+2), cluster mask (+4), shift (+5), first FAT (+6), FAT count (+8), root entries (+9), first data sector (+0Bh), max cluster (+0Dh), FAT sectors (+0Fh), first root sector (+10h), current directory cluster (+1Ch) and text (+1Eh); written by DOTDOTHARDWAY (+1Ch = FFFFh, the text shortened). The rewrite reads and writes it with far accesses (`ibm_pc_dos::far_read16`/`far_write16`) |
| Vectors | INT 21h AX=3523h/2523h, 3524h/2524h | the original saves and hooks INT 23h and INT 24h; the rewrite hooks INT 23h only (spec D4) |
| Directories | INT 21h AH=3Bh | `\`, each subdirectory's name (relative), `..`, `.` (DOTDOTHARDWAY), the user's directory at the end |
| DTA | INT 21h AH=1Ah | the original: 19B2h for the label search, DIRBUF for the walk, FATMAP for the file check; the rewrite: its record, before each search (spec D2) |
| Searches | INT 21h AH=11h/12h | extended FCB attribute 08h (the label), extended FCB attribute 1Eh per directory (the walk: the entry number DOS keeps at FCB+17h locates the entry for writes), FCB 1 itself (the file check) |
| Open | INT 21h AH=0Fh | ORPHFCB, to find a free `FILEnnnn.CHK` name (with `/F` only) |
| Sectors | INT 25h, INT 26h (AL drive 0 = A, DS:BX buffer, CX count, DX sector) | read: the FAT (the first copy that reads), a directory sector (GETENT); write (`/F` only): directory sectors (FIXENT, `.`/`..` recovery, recovered chains) and every FAT copy at the end if the FAT changed. Both leave the flags on the stack; the rewrite calls them in `src/disk.s` (spec D3) |
| Output | INT 21h AH=09h, AH=02h | messages (`$`-terminated), digits and single characters, all to the console |
| Input | INT 21h AH=0Ah (15-character buffer), AX=0C01h | the Y/N prompts read a line (so `y` and Enter); `Abort, Retry, Ignore?` flushes and reads one key |
| Memory | PSP:0002h, CS | ` bytes total memory` = PSP:0002h x 16; ` bytes free` = (PSP:0002h - CS) x 16 |
| Exit | INT 20h | no exit code |

The target build (`src/main.rs`) is a `.COM` (`crt0_com.s`, `link_com.ld`, `i8086-ld --com`); the stack is at the top of the program's memory (PSP:0002h, at most one segment), the 12,288-byte FAT/map/sector buffer in `.bss` (spec D1). Of the segment, 24,894 bytes are used (12,534 of program, 12,360 of `.bss`).
