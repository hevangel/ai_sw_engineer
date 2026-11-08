# DISKCOMP (PC DOS 2.00): behaviour specification

Derived from the original `DISKCOMP.COM` (2,074 bytes, SHA-256 `5881d93e...`, [../original/provenance.md](../original/provenance.md)), IBM's "Diskette Compare Utility Version 2.0" by David Litton and Mark Tyson (the copyright text at 103h), by reading it in full disassembled with GNU objdump 2.42 (`objdump -D -b binary -m i8086 -M intel --adjust-vma=0x100 DISKCOMP.COM`, code from 1C1h, start from 810h), side by side with DISKCOPY 2.0 by the same authors ([../../diskcopy/spec/spec.md](../../diskcopy/spec/spec.md)), disassembled the same way. No source is published. REA (spec/software-lifecycle.md) was not used: it needs a Ghidra/IDA/Hopper provider, none is installed here, and the program is about 1,400 bytes of code read in full. Offsets are the `.COM`'s (CS:100h is the first byte). Observations are marked (O), derivations from them (D), inferences not run (I).

## Shared with DISKCOPY (O)

The two programs are built the same way: a `JMP` to a start routine at the end of the file that is then free memory, a block of state bytes after the copyright text, the stack in the PSP (`MOV SP,100h`), a track buffer from ((CS + start/16) AND FFE0h) + 20h, the drive and parameter checks in the same order of instructions (FCB names, INT 11h drives, AH=19h default, the letters ORed into the messages, `REPNZ SCASB` for `/`), the same track loop (`XCHG [sides],DH` after each track, the next cylinder when DH is 0 again), the same 64 KiB split of a track into two INT 13h calls (DISKCOPY 2E7h, DISKCOMP 33Ch), the same one-side detection at head 1 of cylinder 0 (status 02h or 04h after four tries), and the same `Track NN, Side N` digits routine. DISKCOMP only reads; it has no write, verify or format path, and resets the drive only after a failed track read.

## Layout of the file (O)

| Offset | Contents |
|---|---|
| 100h | `JMP 810h`, then the copyright text (103h-1ADh) |
| 1AEh-1C0h | the state (below) |
| 1C1h-60Bh | the compare routines |
| 60Ch-80Dh | the `$`-terminated messages; bytes patched in: drive letters at 631h, 658h, 6B8h; track digits at 6C2h-6C3h, side at 6CBh; sectors at 715h, sides at 72Ah |
| 80Eh-919h | the start (memory after it is the buffers'; the buffers start at 0A00h for a PSP at 0600h) |

| State | Use |
|---|---|
| 1AEh | BUF2: the big buffer's first paragraph, BUF + 240h (200h for 8 sectors) |
| 1B0h | POS: the big buffer's cylinder to compare next (+ 240h, 200h, per compare) |
| 1B2h | BUF: the buffer's first paragraph, two tracks (one cylinder, or two one-sided cylinders) |
| 1B4h | BUF as an offset in the program's segment: (BUF - CS) x 16, DS:SI of the compare |
| 1B6h | START: the first cylinder of the read |
| 1B7h | SIDES: 1 while head 1 of the cylinder is still to do; exchanged with DH after each track (1 at the start, 0 with `/1`) |
| 1B8h | CAP: the big buffer's tracks; halved to cylinders when the first disk turns out two-sided |
| 1B9h | END: the cylinder after the read (START + count, at most 40; halved with CAP) |
| 1BAh | OP: 01h two sides; 02h sides known; 04h a difference was reported; 08h the second part of a track split at a 64 KiB boundary is next |
| 1BBh | MODE: bits 1-0 count the drive checks; 04h one drive (first = second); 08h `/1`; 10h `/8`; 20h 8 sectors (first disk, or `/8`); 40h the second disk read as 8 sectors |
| 1BCh | tries |
| 1BDh | SMALL: the cylinders BUF holds, 2 (1 once the disk is two-sided) |
| 1BEh | MARK: the cylinder after the big buffer's read |
| 1BFh | LEN: the bytes of a track, 1200h (1000h for 8 sectors) |

## Start (810h-917h)

- (O) `CLD`, SP = 100h, SIDES = 1, a 0 pushed (a `RET` would reach PSP:0000, INT 20h).
- (O) AX at entry: AH OR AL = FFh (either parameter named an invalid drive): `Invalid drive specification`, INT 20h; this comes before the version. INT 21h AH=30h, AL below 2: `Incorrect DOS version`. A file name in FCB 1 (PSP:5Dh not a space) or FCB 2 (PSP:6Dh): `Invalid parameter`. Each such message is CR LF, the text, CR LF, BEL.
- (O) First = PSP:5Ch, second = PSP:6Ch (copied to 5Dh). INT 11h bits 7-6 + 1 = the number of drives. One drive: both are A. Otherwise a 0 is the default drive (AH=19h, + 1), and a drive above the number of drives is `Invalid drive specification`. The letters go into the two `Insert` messages; first = second sets MODE 04h.
- (O) 8BAh: `REPNZ SCASB` for `/` over the tail (PSP:80h, 81h); while bytes are left after a `/`, the next byte must be `1` (MODE 08h) or `8` (MODE 10h), else `Invalid parameter`; the scan goes on from there for the next `/`. (D) A `/` that is the tail's last byte is ignored; `/1/8` sets both; a third parameter is ignored.
- (O) BUF = ((CS + 81h) AND FFE0h) + 20h (06A0h for a PSP at 0600h), its offset (BUF - CS) x 16 (0A00h); `/1`: SIDES = 0.

## The compare (1C1h-2CFh)

- (O) `Insert first diskette in drive X:`; with two drives `Insert second diskette in drive Y:`; `Strike any key when ready` and a key (AX=0C01h).
- (O) 1D1h: the first drive's check (2D2h); the first time, CAP (381h); the big read (3C6h) of the **first** drive from START into BUF2 (CAP tracks, at most to 40), MARK = its end; with one drive `Insert second...` and a key.
- (O) 202h, for each cylinder (SMALL of them) of the second drive: its check (2D2h, the first time); unless `/8`, MODE 20h and 40h must agree (`AND DH,60h` / `JP`: an odd parity, one of them set, is `Incompatible diskette or drive types`, INT 20h); the read of SMALL cylinders into BUF (3C6h); the compare (4A7h). END 40: the end. Else START = END, and again until END = MARK.
- (O) 23Fh: the big read of the **second** drive from START into BUF2, MARK; with one drive `Insert first...` and a key; 263h: for each cylinder of the first drive the read into BUF and the compare, until END 40 (the end) or MARK; then back to 1D1h (the first drive's big read; the checks and CAP are not made again).
- (D) The roles swap each pass so that one drive needs one disk swap a pass. Two sides, CAP 48: the first pass holds cylinders 0-19 of the first disk (END 40 halved at head 1 of cylinder 0), the second 20-39 of the second disk (CAP / 2 = 24, capped at 40): `diskcomp b: b:` asks for first, second, first (case k07). One side: one pass of 40 cylinders.
- (O) 28Bh: unless OP 04h, `Diskettes compare ok`. `Compare more diskettes (Y/N)?` and a key (ORed with 20h): `n` INT 20h; `y` again (START = 0; unless `/1` SIDES = 1, and SMALL + 1 if two-sided; OP = 0; MODE keeps 04h, 08h, 10h; LEN = 1200h; SP = 100h; so the checks and CAP are made again); anything else asks again.

## Drive check (2D2h)

- (O) Only the first two calls of a compare do anything (MODE bits 1-0 count to 2). The second with MODE 20h only sets MODE 40h. With `/8` the first sets MODE 20h and LEN = 1000h without reading.
- (O) Otherwise INT 13h AX=0201h, CX=0009h (cylinder 0, sector 9), DH = 0, ES:BX = BUF:0000, four tries without a reset; status 80h: `Not ready error reading drive X`, `Correct, then strike any key`, a key, and again without counting. The fourth failure: on the first drive MODE 20h and LEN = 1000h (8 sectors); on the second MODE 40h (then `Incompatible` at 209h unless the first is 8-sector too).

## Capacity (381h)

- (O) BUF2 = BUF + 240h (200h for MODE 30h). n = (PSP:0002h - BUF) / 120h (100h), rounded down to even; below 4: `Insufficient memory`, INT 20h; CAP = n - 2 (the two tracks at BUF). On the oracle machine (top 4000h, BUF 06A0h) n = 51 → 50, CAP = 48.

## Read (3C6h)

- (O) With DH = the count: tries = 0, BP = ES, CH = START, END = START + count (at most 40), DH = 0. For each track: the INT 13h AH=02h (33Ch: one call, or two for a track split at a 64 KiB boundary; the callers loop while OP 08h is set) into the slot at BP; then BP + 120h (100h for MODE 70h), `XCHG [1B7h],DH`, the next cylinder when DH is 0 again, until END.
- (O) The sides are decided on the first track that tells, while OP 02h is clear: with `/1` the first successful read sets OP 02h, DH = 0 and prints `Comparing 9 sectors per track, 1 side(s)`; otherwise the first successful read of head 1 of cylinder 0 sets OP 03h, halves CAP and END, SMALL - 1 and prints `..., 2 side(s)` (8 for MODE 20h).
- (O) A failed read: INT 13h AH=00h (reset), status 80h: the not-ready message and a key, again without counting; else again, four times. After the fourth: at head 1 of cylinder 0 with status 02h or 04h: if the sides are known, `Incompatible diskette or drive types`, INT 20h; else one-sided (BP back one slot, OP 02h, DH = 0, `..., 1 side(s)`). Otherwise `Unrecoverable read error on drive X` and `Track NN, Side N`, and the read goes on as if it had worked (the slot keeps what it held).
- (D) As in DISKCOPY, a failed first part of a split track leaves OP 08h set, so the next try is the second part. The rewrite does the same.

## Compare (4A7h)

- (O) `REPZ CMPSB` of 2 x LEN bytes, DS:SI = BUF (offset 1B4h) against ES:DI = POS:0000. A difference sets OP 04h and prints `Compare error(s) on` / `Track NN, Side N`: if CX left is at least LEN the byte was in the first track (START, side 0), and the compare goes on from the second track (SI and ES:DI moved to its start, CX = LEN); else in the second track, which is side 1 of START when two-sided, or side 0 of START + 1 when one-sided, and the compare ends. Each track is reported at most once. Then POS + 240h (200h for MODE 70h).

## Messages (O)

`Compare error(s) on ` has a trailing space; `Track NN, Side N` ends with CR LF BEL; the not-ready message prints the drive letter through the `@` of `Unrecoverable read error on drive @` (6B8h), and `Compare more diskettes (Y/N)?` has no CR LF after it (the echoed key stays on its row).

## The emulators (O)

The original on our Rust emulator (5150 board, `Machine::new(256)`) gives the oracle's screen on all 8 cases, and leaves both disks as they were. No case reads a one-sided image's head 1, where MartyPC and the Rust emulator report different statuses (20h and 04h, [../../diskcopy/spec/spec.md](../../diskcopy/spec/spec.md) "Head 1 of a one-sided image"); `oracle_replay.rs` models MartyPC's 20h and checks what DISKCOMP then does (a read error, compared as two-sided).

## The rewrite

`src/lib.rs` follows these routines with the same state, the same INT 13h calls in the same order with the same registers (including the 64 KiB split and its retry behaviour), the same compare and the same messages. Deliberate differences:

| ID | Difference | Why | Effect |
|---|---|---|---|
| D1 | `src/start.s` replaces the shared `crt0_com.s`: the stack (1 KiB) is right after `.bss` instead of at the top of the segment, and BUF is computed by the original's rule from the end of that stack: 0720h for a PSP at 0600h (offset 1200h), BUF2 0960h, instead of the original's 06A0h and 08E0h; CAP is computed as in the original (48 on 256 KiB, as the original) | the rewrite (3,306 bytes; the original 2,074) does not fit below 06A0h, and the buffers (up to 42 tracks) run across the segment's top, so the stack cannot be there (the original keeps it in the PSP) | the same passes and swaps while CAP is at least 40; with much less memory the rewrite may hold 2 tracks fewer than the original. Tracks land 800h bytes higher, so other tracks are split at 64 KiB (more INT 13h calls, the same data); an unreadable track compares memory that is at other addresses |
| D2 | INT 13h, INT 11h and `REPZ CMPSB` are made by `src/bios.s` | the shared runtime has INT 21h only, and Rust has no inline assembly here | none |
| D3 | The messages with patched bytes are built in a buffer and printed with one function 09h call each | no writable message text | none: the same characters, the same 09h calls |
| D4 | INT 20h from inside the original's routines (insufficient memory, incompatible diskettes, `n`) is a return through the core to `start.s` | Rust | none |

Assumption ledger: none (`src/lib.rs` has no `ASSUMPTION`).
