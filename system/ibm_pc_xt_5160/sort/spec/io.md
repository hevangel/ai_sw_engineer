# SORT: I/O contract

| Channel | DOS call | Use |
|---|---|---|
| DOS version | INT 21h AH=30h | below 2.00: print the version message (AH=09h) and return to DOS through PSP:0000h |
| Switch character | INT 21h AX=3700h | DL; normally `/` |
| Command tail | PSP:0081h | read to the CR; redirection is already removed by COMMAND |
| Memory | INT 21h AH=48h, BX=1000h (then the largest block) | the original's 64 KiB work segment; the rewrite uses a static buffer (spec D1) |
| Input | INT 21h AH=3Fh, handle 0 | read until 0 bytes come back, or the buffer is full (then the memory message) |
| Output | INT 21h AH=40h, handle 1 | the sorted text, one write |
| Errors | INT 21h AH=40h, handle 2 | `SORT: Insufficient memory` (27 bytes) or `SORT: Insufficient disk space` (31 bytes), each with CR LF |
| Exit | INT 21h AH=4Ch | AL = the low byte of the last write's count (AX of function 40h is kept): 27 or 31 after an error, else the output length's low byte |
