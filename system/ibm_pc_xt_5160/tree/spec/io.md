# TREE: I/O contract

| Channel | DOS call | Use |
|---|---|---|
| DOS version | INT 21h AH=30h | major version not 2: the version message (AH=09h), INT 20h |
| Drive check | AX at entry | AL or AH = FFh (COMMAND found an invalid drive in parameter 1 or 2): `Invalid drive specification` to handle 2, INT 20h |
| Default drive | INT 21h AH=19h, AH=0Eh | saved at start; the FCB 1 drive (PSP:5Ch, 1 = A) is selected; the saved one is selected again at the exit |
| Switch character | INT 21h AX=3700h | DL; the first one in the tail must be followed by `F` or `f`; not `/`: the path separator becomes `/` |
| Command tail | PSP:0080h (length), PSP:0081h | searched for the switch character over the length (the CR excluded) |
| Ctrl-Break | INT 21h AX=2523h | handler: restore the directory and drive, INT 20h (spec D4 in the rewrite) |
| Current directory | INT 21h AH=47h, DL = PSP:5Ch | of the target drive, restored with CHDIR at the exit |
| DTA | INT 21h AH=1Ah | the original: 3FEh, once; the rewrite: its 44-byte record, before each search (spec D3) |
| Directories | INT 21h AH=3Bh | `\`, each path TREE builds (absolute), the parent; carry on a path: `Invalid path` |
| Searches | INT 21h AH=11h/12h, extended FCBs | attribute 08h (volume label, once), 10h (subdirectories; filtered: attribute 10h, not 02h/04h, not `.`), 00h (files, with `/F`); name `???????????`, or a directory's own name to find its place in the parent |
| Output | INT 21h AH=40h, handle 1 | the header and the directory blocks |
| Errors | INT 21h AH=40h, handle 2 | `No sub-directories exist` (29 bytes), `Path: ...` + `Invalid path` (18 bytes), `Invalid parameter` (22), `Invalid drive specification` (32); each ends with BEL |
| Exit | INT 20h | no exit code |

The target build (`src/main.rs`) is a `.COM` (`crt0_com.s`, `link_com.ld`, `i8086-ld --bin`); the stack is at the top of the program's memory (PSP:0002h), the 16,384-byte path and list buffer in `.bss` (spec D1).
