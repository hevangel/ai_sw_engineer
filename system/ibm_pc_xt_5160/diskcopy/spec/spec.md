# DISKCOPY (PC DOS 2.00): behaviour specification

Derived from the original `DISKCOPY.COM` (2,444 bytes, SHA-256 `2189a014...`, [../original/provenance.md](../original/provenance.md)), IBM's "Diskette Copier Version 2.0" by David Litton and Mark Tyson (the copyright text at 103h), by reading it in full disassembled with GNU objdump 2.42 (`objdump -D -b binary -m i8086 -M intel --adjust-vma=0x100 DISKCOPY.COM`). No source of this program is published. Microsoft's MS-DOS 2.0 `DISKCOPY.ASM` (Chris Peters, MIT) is a different program (it copies through DOS's INT 25h/26h and asks other questions) and was not used. REA (spec/software-lifecycle.md) was not used: it needs a Ghidra/IDA/Hopper provider, none is installed here, and the program is 1,800 bytes of code read in full. Offsets are the `.COM`'s (CS:100h is the first byte). Observations are marked (O), derivations from them (D), inferences not run (I).

## Layout of the file (O)

| Offset | Contents |
|---|---|
| 100h | `JMP 9A0h`, then the copyright text (103h-1A1h) |
| 1A2h-1ADh | the state bytes (below) |
| 1AEh-1D1h | the format table: nine (C, H, R, N) entries, R = 1..9, N = 2 |
| 1D2h-5A2h | the copy routines |
| 5B0h-6CBh | messages and the error report |
| 6CCh-99Fh | the `$`-terminated messages; some have bytes patched in (drive letters at 70Fh, 736h, 92Ah; track digits at 7E5h-7E6h, side at 7EEh; sectors at 8C1h, sides at 8D6h) |
| 9A0h-A8Bh | the start (it becomes the first track buffer's neighbour and is overwritten) |

| State | Use |
|---|---|
| 1A2h | START: the first cylinder of the pass |
| 1A3h | CAP: track buffers that fit; halved to cylinders when the disk turns out two-sided |
| 1A4h | END: the cylinder after the pass (START + CAP, at most 40; halved with CAP) |
| 1A5h | OP: bits 1-0 the BIOS function (0 read, 1 write, 2 verify, 3 format, giving AH = 2-5); 04h sides known; 08h two sides; 10h format the target; 20h the second part of a track split at a 64 KiB boundary is next; 40h try again without counting (not ready, write protected) |
| 1A6h | MODE: bits 1-0 count the drive checks; 04h one drive (source = target); 08h `/1`; 20h the source has 8 sectors per track; 40h an error was reported (`Target diskette may be unusable`) |
| 1A7h-1AAh | tries of the read, write, verify and format loops |
| 1ABh | SIDES: 1 while head 1 of the cylinder is still to do; exchanged with DH after each track (`XCHG [1ABh],DH`), so a two-sided disk alternates heads 0 and 1 and a one-sided one stays on head 0 (0 with `/1`) |
| 1ACh | BUF: the track buffer's first paragraph |

## Memory (O)

- The stack is in the PSP: `MOV SP,100h` is the first instruction after `CLD` (9A1h), and again at each new copy (226h). Nothing above the file is written by the program before the copy.
- BUF (A74h) = ((CS + 9Ah) AND FFE0h) + 20h: the paragraph after the code, rounded down to 512 bytes, plus 512 bytes. With the PSP at 0600h (the oracle machine) BUF = 06A0h.
- A track takes 120h paragraphs (9 sectors; 100h for 8). Track *k* of a pass (head 0 and head 1 of each cylinder in turn) is at BUF + *k* x 120h.
- CAP (2B0h, once, after the source's check) = (PSP:0002h - BUF) / 120h (or 100h); below 2: `Insufficient memory`, INT 20h. On the oracle machine (top 4000h) CAP = 51.
- (D) Two-sided disks: the first pass ends at min(CAP, 40) / 2 = 20 cylinders (END is halved after it was capped at 40), the second takes CAP / 2 = 25 cylinders, capped at 40: cylinders 0-19 and 20-39, 40 tracks each. One-sided: one pass of min(CAP, 40) = 40 tracks. A pass never uses more than 40 tracks, so BUF + 40 x 120h is the most memory the copy uses (spec D1).

## Start (9A0h-A89h)

- (O) INT 21h AH=30h; AL below 2: `Incorrect DOS version`, INT 20h (each message here: CR LF, the text, BEL, CR LF).
- (O) AX at entry (AL for parameter 1, AH for parameter 2): either FFh: `Invalid drive specification`, INT 20h. A file name in FCB 1 (PSP:5Dh not a space) or FCB 2 (PSP:6Dh): `Invalid parameter`, INT 20h.
- (O) Source = PSP:5Ch, target = PSP:6Ch (copied to 5Dh). INT 11h bits 7-6 + 1 = the number of drives. One drive: both are A. Otherwise a 0 (no drive given) is the default drive (AH=19h, + 1), and a drive above the number of drives is `Invalid drive specification`. The letters go into the two `Insert` messages; source = target sets MODE 04h.
- (O) A4Ch: `REPNZ SCASB` for `/` over the tail (PSP:80h length, 81h text); if CX is not 0 after it (a `/` with bytes after it), the next byte must be `1` (else `Invalid parameter`, INT 20h): MODE 08h, SIDES = 0. (D) Only the first `/` counts; a `/` that is the tail's last byte is ignored (CX is 0 after the scan); `/` followed by anything but `1` is refused.
- (D) A third parameter is ignored (case d07).

## The copy (1D2h-24Fh)

- (O) `Insert source diskette in drive X:`; with two drives `Insert target diskette in drive Y:`; `Strike any key when ready` and a key (AX=0C01h).
- (O) Each pass: the source's check (251h), CAP (first pass), the read pass (338h); with one drive `Insert target...` and a key; the target's check (251h); the write pass (3FAh). START = 40 ends: `Copy complete`, or `Target diskette may be unusable` if MODE 40h; else with one drive `Insert source...` and a key, and the next pass.
- (O) 217h: `Copy another (Y/N)?` and a key (0C01h, ORed with 20h): `n` INT 20h; `y` a new copy (START = 0, SIDES = 1 unless `/1`, OP = 0, MODE keeps 04h, 08h, 10h only, so the checks and CAP are made again); anything else asks again.

## Drive check (251h)

- (O) Only the first two calls of a copy do anything (MODE bits 1-0 count to 2). With ES = BUF, CH = START, DH = 0, CL = 9, AL = 1: on the source AH = 2 (read sector 9 of track 0), on the target AH = 3 (write that sector back to the target's track 0; skipped if the source has 8 sectors).
- (O) Four tries (reset between, 553h). Failing on the source: MODE 20h (8 sectors). On the target: OP 10h and `Formatting while copying`.

## Read pass (338h)

- (O) OP function 0; END = START + CAP, at most 40. For each track from (START, head 0): INT 13h (2E7h) into BUF + *k* x 120h; then the next buffer and `XCHG [1ABh],DH`; when DH is 0 again, the next cylinder, until END.
- (O) The sides are found on the first track that tells: the first successful read of head 1 of cylinder 0 (OP 04h not set yet) sets OP 0Ch, halves CAP and END and prints `Copying 9 sectors per track, 2 side(s)` (8 for MODE 20h). With `/1` the first successful read sets OP 04h, DH = 0 and prints `..., 1 side(s)`.
- (O) A failed read is reset (553h) and tried again, four times. After the fourth: if the sides are not known yet, it was head 1 of cylinder 0, and the status is 02h (address mark not found) or 04h (sector not found), the disk is one-sided: the buffer goes back one track, OP 04h, DH = 0, `Copying 9 sectors per track, 1 side(s)`. Otherwise `Unrecoverable read error on source` and `Track NN, side N` (MODE 40h), and the copy goes on as if the read had worked: the buffer keeps whatever it held, and the write pass writes it to the target.
- (D) A one-sided disk in a double-sided drive is therefore copied as one-sided only when head 1 reports 02h or 04h; with any other status (20h on MartyPC, below) every head-1 track of the disk is reported unreadable, the disk is copied as two-sided, and each head-1 track of the target gets the bytes its buffer held: memory as it was before the copy (the first oracle, below).

## Write pass (3FAh)

- (O) For each track of the buffer: if OP 10h and head 0, the cylinder is formatted first (4E9h); INT 13h AH=3 (2E7h); then verify (4ACh, AH=4); then the next buffer, `XCHG`, the next cylinder; at END, START = END.
- (O) Write errors: reset, four tries. Status 02h or 04h after four: the first time OP 10h, `Formatting while copying`, and (if on head 1) back to head 0 of the cylinder; the cylinder is formatted and written again. With OP 10h already set, on head 1 of cylinder 0: `Incompatible drive types`, `Target diskette unusable`, and `Copy another` (217h). Other errors: `Unrecoverable write error on target` + track line, and the next track.
- (O) Verify (4ACh): a failure is reset and the track is written again (the counter is not reset); the fourth failure prints `Unrecoverable verify error on target` + track line and goes to the next track.
- (O) Format (4E9h): ES:BX = the table (in the program's segment), C = the cylinder, H = the head in all nine entries, AX = 0509h (0508h); two-sided: head 1, then head 0. Four tries (reset between); the fourth failure prints `Unrecoverable format error on target` / `Target diskette unusable` + track line and goes to `Copy another`.

## INT 13h calls (2E7h, 553h)

- (O) 2E7h makes AX, CX, ES for the track at BP: AX = FFFh - (BP AND FFFh) is the paragraphs to the next 64 KiB boundary less one; (AX >> 5) + 1 sectors fit. A track that fits is one call (AL = 9 or 8, CL = 1, ES = BP); one that does not is two: the sectors that fit (OP 20h set), then the rest from sector *fit* + 1 with ES at the boundary (OP 20h cleared). BX is 0. The callers loop while OP 20h is set.
- (D) A failed first part leaves OP 20h set, so the retry does the second part; a failed second part retries the first. The original behaves so; the rewrite does the same. On the oracle machine the tracks that cross a boundary (buffers 8, 22, 36) are all head-0 tracks, which read.
- (O) 553h after each failure: OP 40h cleared; INT 13h AH=0 (reset, DL the drive); status 80h (timeout): `Not ready error reading` (or `writing` when OP bit 0 is set: write and format) ` drive X`, `Correct, then strike any key`, a key, OP 40h; status 03h: `Target diskette write protected` / `Correct, then strike any key`, a key, OP 40h. OP 40h makes the caller try again without counting.
- (O) The error report (685h): the message; for 6D0h (formatting) nothing more; for 85Ch (write protected) a key; else `Track NN, side N` (BEL, CR LF; the cylinder in two decimal digits) and MODE 40h.

## Head 1 of a one-sided image (O)

| | MartyPC `05c0d088` (5160) | Rust emulator (`system/ibm_pc/emulator`) |
|---|---|---|
| READ DATA (06h) of head 1 on a 180 KiB (one-head) image | fluxfox's `read_sector` refuses a head the image lacks (`SeekError`); MartyPC ends the command as an abnormal termination with ST0 = 40h and ST1 = ST2 = 0 | `fdc.rs` `stage_ready`: ST0 = 40h, ST1 = 04h (No Data) |
| BIOS status (INT 13h AH) | 20h (no ST1/ST2 bit: controller failure) | 04h (sector not found) |
| DISKCOPY (338h) | not 02h/04h: `Unrecoverable read error on source` for each of the 40 head-1 tracks, two sides, `Target diskette may be unusable` | 04h: one-sided, `Copying 9 sectors per track, 1 side(s)`, `Copy complete` |

A real one-sided diskette in a double-sided drive has no ID fields on side 1 (status 02h), which DISKCOPY also takes as one-sided: MartyPC is the outlier. Neither emulator is changed; since 2026-11-07 the A-to-B cases copy a double-sided 360 KiB copy of disk 1 (`a:system360`, `pcdisk syscopy`), which every head reads, so no case depends on this.

### Historical (2026-11-07, superseded): the first oracle with the 180 KiB source

The first oracle copied disk 1 itself. Its head-1 tracks on drive B were the track buffers' earlier contents, which identified the memory before the program (D): buffer 1 (07C00h-08DFFh) held bytes 10176-14783 of COMMAND.COM (SYSINIT loaded it at 5340h:0100h, below DISKCOPY's PSP at 0600h), buffer 13 (15400h-165FFh) the words 0100h, 0600h that DOS's EXEC leaves at 0600h:FFFAh, and the rest zeros. To give the same bytes the rewrite then moved itself above the 40 buffers so that its buffers were the original's, and the host replay modelled that memory; both are gone with the new cases.

## The rewrite

`src/lib.rs` follows these routines with the same state bytes, the same INT 13h calls in the same order with the same registers (including the 64 KiB split and its retry behaviour), the same messages and the same track buffer addresses. Deliberate differences:

| ID | Difference | Why | Effect |
|---|---|---|---|
| D1 | `src/start.s` replaces the shared `crt0_com.s`: the stack (1 KiB) is right after `.bss` instead of at the top of the segment, and the track buffer starts after the stack by the original's rule (A74h: the paragraph rounded down to 512 bytes, plus 512 bytes), at 0740h for a PSP at 0600h instead of the original's 06A0h; CAP is (PSP:0002h - BUF) / 120h as in the original (50 on 256 KiB, the original 51) | the rewrite (3,526 bytes; the original 2,444) does not fit below 06A0h, and the buffer of up to 40 tracks (180 KiB) runs across the segment's top, so the stack cannot be there (the original keeps it in the PSP) | the same passes and swaps while 40 tracks fit (two-sided: two passes of 20 cylinders; one-sided: one of 40); with less memory the rewrite fits 1-2 tracks fewer per pass than the original. The tracks land 0A00h bytes higher, which only matters for a track that does not read (its buffer's earlier bytes are written) |
| D2 | INT 13h and INT 11h are made by `src/bios.s` | the shared runtime has INT 21h only, and Rust has no inline assembly here | none |
| D3 | The messages with patched bytes are built in a buffer and printed with one function 09h call each | no writable message text | none: the same characters, the same 09h calls |
| D4 | The ES:BX of the format call is the rewrite's own copy of the table (in its segment) | the table is the core's state | none |

Assumption ledger: none (`src/lib.rs` has no `ASSUMPTION`).
