# PRINT: I/O contract

## The transient part (`software::run`, the target's `DosCom` in `src/main.rs`)

| Channel | Call | Use |
|---|---|---|
| DOS version | INT 21h AH=30h | AX with AL and AH swapped outside 0136h-0200h: `Incorrect DOS version` CR LF (09h), INT 20h |
| Switch character | INT 21h AX=3700h | DL: what starts a switch in the tail |
| An installed copy | INT 21h AX=352Fh; 13 bytes at ES:BX against this copy's INT 2Fh handler | equal: no install, the requests go to that copy |
| The list device | 09h `Name of list device [PRN]: `, 0Ah (14 bytes), 09h CR LF; 0Fh on the FCB `PRN` (or the typed name) | FCB byte 18h bit 7: a device; bytes 19h-1Ch: its driver header, to 1A2h and the call pointer's segment (515h); failing: `List output is not assigned to a device`, INT 20h |
| Hooks | 35h/25h for 28h, 2Fh (not saved), 13h, then 35h for 17h, 14h, 5; 25h 17h and 5 (PRN, LPT1-3) or 14h (AUX, COM1-2); 34h (InDOS to 183h); 35h/25h for 1Ch | the saved vectors at 191h, 517h, 51Bh, 51Fh, 523h, 187h; the printer number at 527h, the port at 529h; 50Dh = CS (the request's transfer segment) |
| Command tail | PSP:80h (length), from 81h to CR | names and switches (spec "The transient part") |
| Names | 2901h into a 37-byte FCB; 1Ah + 11h/12h (DTA a 40-byte buffer); 0Fh on the entry found | each opened FCB (38 bytes) to the resident part |
| Requests | INT 2Fh: AH=0 DS:DX = the opened FCB; AH=1 DS:DX = the parsed FCB, or DX = FFFFh; AH=FFh status | AL: 1 queue full; AH: entries in use; ES:BX: the queue (the word before it: device errors); DX: the head |
| Messages | 09h | `Invalid parameter`, `File not found`, `Cannot open`, `PRINT queue is full`, `Resident part of PRINT installed`, the status lines, `Errors on list device ...` |
| End | INT 27h with DX = 0BFAh after an install, else INT 20h | PSP:0000h-0BFFh kept |

The target build is a `.COM` linked at 100h with `link_com.ld`: `src/resident.s` first (`.text.start`, 100h-BF9h
and the start at BFAh), then the helpers `print_int2f_call` and `print_keep` (`resident.s`), the shared runtime's
`dos.s` (INT 21h through `dos_call`, far words), the Rust code, its read-only data and `.bss`. INT 27h frees
everything from C00h: the start, the Rust code, its data and the stack.

## The resident part (`src/resident.s`; the same logic in `software::resident` for the host)

| Offset | Contents (spec Layout) |
|---|---|
| 100h | `E9 F7 0A` (`JMP 0BFAh`) |
| 103h-182h | the slice's stack (SP = 183h) |
| 183h-1AFh | the variables at the original's offsets |
| 1B0h-2FBh | the messages for the list device and the INT 24h message table (233h) |
| 2FCh-4FBh | the record buffer |
| 4FCh-52Ah | the byte written, the request header (4FDh: length 16h, command 4FFh, status 500h, transfer 50Bh:50Dh = CS:4FCh, count 50Fh = 1), the driver call pointer 513h, the old INT 13h/17h/14h/5 vectors 517h-526h, the printer number 527h, the port number 529h |
| 52Bh-A65h | `print_int1c` 52Bh, `print_int28` 587h, the slice, the read, the next file, INT 24h, `print_int2f` 787h, the device routines, `print_int13` 9D3h, `print_int17` 9E8h, `print_int14` A0Eh, `print_int5` A3Fh |
| A66h-BF9h | the queue: 10 (A66h), the device errors (A68h), the ring of 10 entries from A6Ah |
| BFAh | `print_start` (transient) |

| Hooked | What it does |
|---|---|
| INT 1Ch | counts ticks; every ninth tick, outside DOS and when not busy: EOI, a slice (until the buffer ends or two ticks pass), then the old INT 1Ch |
| INT 28h | when not busy: a slice of one byte, then the old INT 28h |
| INT 2Fh | submit, cancel, status (any AL); not chained |
| INT 24h | only during the slice's read (set and put back around it): the error to the list device, ignore |
| INT 13h | busy while the BIOS disk call runs |
| INT 17h / 14h | AH = A1h / 80h for another program's call to the same printer / port while a file is queued |
| INT 5 | ignored while a file is queued for printer 0 |

| Device call | Request | Use |
|---|---|---|
| Output status | command 0Ah, the driver's strategy then interrupt routine (`CALL FAR`, ES:BX = 4FDh) | before each byte of a file and of a message; status bit 9 (or 15) clear is ready |
| Write | command 8, count 1, the byte at 4FCh | each byte; a file's bytes up to ^Z with tabs expanded, then FF; the messages |

The bytes reach the printer through the list device's own driver (for `PRN`/`LPTn`, DOS 2.00's printer driver,
which calls INT 17h; PRINT's INT 17h hook lets these calls through since PRINT is busy while it makes them).
