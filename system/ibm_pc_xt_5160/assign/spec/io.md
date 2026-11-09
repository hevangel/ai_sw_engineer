# ASSIGN: I/O contract

## The transient part (`software::run`, the target's `DosCom` in `src/main.rs`)

| Channel | Call | Use |
|---|---|---|
| DOS version | INT 21h AH=30h | AX with AL and AH swapped below 00C8h: `Incorrect DOS version` CR LF (09h), INT 20h |
| Equipment | INT 11h (`assign_int11`, `resident.s`) | the word saved at 120h; read again by the equipment routine when B is assigned elsewhere |
| BIOS data | word 0040h:0010h | written by the equipment routine (spec) on every successful run |
| Command tail | PSP:0080h (length), from 0081h to CR | the pairs; the character before each `=` and the one after it |
| Errors | INT 21h AH=09h, then AH=4Ch with AL as 09h returned it | `Invalid parameter` (no CR LF) |
| Vectors | INT 21h AH=35h for 25h, 26h, 21h | saved at 113h, 117h, 10Fh; ES:BX of 21h is where an installed copy is looked for |
| An installed copy | 8 bytes at the INT 21h vector against this copy's 122h-129h | equal: this copy's 103h-10Ah to ES:103h, the equipment routine with ES:120h, AH=4Ch with AL = the word's low byte |
| Default drive | INT 21h AH=19h, AH=0Eh | at the install: 10Bh = AL + 1; table[that] - 1 selected |
| Hooks | INT 21h AH=25h for 25h, 26h, 21h | `assign_int25`, `assign_int26`, `assign_int21` (122h) |
| Stay resident | INT 21h AH=31h, DX = 31h, AL as 25h returned it | PSP:0000h-030Fh kept |

The target build is a `.COM` linked at 100h with `link_com.ld`: `src/resident.s` first (`.text.start`), then the
shared runtime's `dos.s` (INT 21h through `dos_call`, far words), the Rust code, its read-only data and `.bss`.
Function 31h frees everything from 310h: the Rust code, its data and the stack.

## The resident part (`src/resident.s`; the same logic in `software::hook` for the host)

| Offset | Contents (spec Layout) |
|---|---|
| 100h | `E9 00 02` (`JMP 303h`) |
| 103h-10Ah | the table, 1-8 at first |
| 10Bh-121h | the logical default drive, the saved byte and its offset, the old INT 21h/25h/26h vectors, the caller's return address, AH, the equipment word |
| 122h | `assign_int21`, then `assign_kind`, `assign_letter`, `assign_lookup`, `assign_int25`, `assign_int26`, `assign_identity`; zeros to 302h |
| 303h | `assign_start`: SP, `.bss`, `main` (transient) |

| Hooked | Before DOS | After DOS |
|---|---|---|
| INT 21h AH ≤ 0Ch, 50h, 51h, or the identity table | nothing (`JMP FAR`) | — |
| 4Bh | the path's drive letter | — (`JMP FAR`) |
| 0Eh | 10Bh = DL + 1; DL = table[DL + 1] - 1 | DL = 10Bh - 1 |
| 19h | — | AL = 10Bh - 1 |
| 32h, 36h, 47h | DL = table[DL] unless 0 (DL saved) | 32h, 47h: DL put back |
| 39h-3Dh, 41h, 43h, 4Eh, 56h | DS:DX's drive letter if followed by `:` | the first byte put back |
| 0Fh-17h, 21h-24h, 27h, 28h | the FCB's drive byte (0: 10Bh) = table[it] | the byte = the logical drive |
| INT 25h, INT 26h | AL = table[AL + 1] - 1 | (`JMP FAR`) |
