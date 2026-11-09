# ASSIGN (PC DOS 2.00): behaviour specification

Derived from the original `ASSIGN.COM` (896 bytes, SHA-256 `080db454...`, [../original/provenance.md](../original/provenance.md)) by reading it in full disassembled with GNU objdump 2.42 (`objdump -D -b binary -m i8086 -M intel --adjust-vma=0x100 ASSIGN.COM`). REA (spec/software-lifecycle.md) was not used: it needs a Ghidra/IDA/Hopper provider, none is installed here, and the program is about 600 bytes of code, read in full. IBM published no source; the file has no copyright or version text, only its two messages. Offsets are the `.COM`'s (CS:100h is the first byte). Observations are marked (O), derivations from them (D), inferences not run (I).

ASSIGN makes DOS calls for one drive go to another. It parses `X=Y` pairs into an eight-byte table, installs an INT 21h hook (and INT 25h/26h hooks) and stays resident in its first 784 bytes; run again, it finds that copy through the INT 21h vector and replaces its table.

## Layout (O)

| Offset | Size | Use |
|---|---|---|
| 100h | 3 | `JMP 303h` (E9 00 02); the byte at 102h (02h) is entry 0 of the table |
| 102h | 9 | the table: the byte at 102h + *d* is the drive that drive *d* (1 = A) goes to; 103h-10Ah start as 1-8 |
| 10Bh | 1 | the logical default drive, 1 = A (what the user selected; function 19h answers it) |
| 10Ch | 1 | the drive byte or DL as the caller passed it (restored after the call) |
| 10Dh | 2 | where that byte is (the path's first byte, the FCB's drive byte) |
| 10Fh, 113h, 117h | 4 each | the INT 21h, INT 25h and INT 26h vectors found at the start (offset, segment) |
| 11Bh | 4 | the caller's return address (IP, CS), taken off the stack |
| 11Fh | 1 | AH of the call |
| 120h | 2 | the BIOS equipment word (INT 11h) at the start |
| 122h-2B5h | | the INT 21h hook |
| 2B6h, 2BEh | | the table lookups: AL = table[AL + 1] - 1 (a 0-based drive), AL = table[AL] (1-based) |
| 2D0h, 2D8h | | the INT 25h and INT 26h hooks |
| 2E0h-302h | | the identity test: ZF = 1 if 103h-10Ah hold 1-8 |
| 303h-422h | | the transient part (start, parse, install) |
| 423h | | `Invalid parameter$`; 317h `Incorrect DOS version` CR LF `$` |
| 435h-47Fh | | `d1.COM091.C` and zeros: leftovers of the build, never used |

The resident part is PSP:0000h-030Fh: function 31h keeps 31h paragraphs.

- (O) The lookup (2BEh) uses the byte as a signed index: `CBW`, then CS:[102h + AX]. (D) A drive number above 8 reads the variables at 10Bh onwards (up to 7Fh: the hook's own code bytes); a number of 80h-FFh reads PSP:82h-101h (the resident copy's command tail and the `JMP`).

## Start (303h)

- (O) Function 30h; with AL and AH swapped (AH = major), AX below 00C8h (DOS 1, AL = 0) prints `Incorrect DOS version` CR LF with function 09h and ends with INT 20h. (D) Every DOS 2.x or later passes.
- (O) 32Fh: INT 11h; the equipment word goes to 120h.

## The parse (334h-360h)

- (O) From PSP:80h with `LODSB`: a length byte of 0 goes straight to the install with the table as it is (1-8). Otherwise the bytes from 81h are read until CR; each `=` takes:
  - the character just before the `=` (`[SI-2]` after the `LODSB` that read it), validated (3E8h): letters a-z made capital, minus 40h; unless the result is 1-8 (`A`-`H`), `Invalid parameter` (function 09h, no CR LF) and function 4Ch with AL as function 09h left it;
  - the next byte: CR is `Invalid parameter`; otherwise validated the same way and stored at 102h + the first.
  - The scan goes on after that second byte.
- (D) So only the characters next to each `=` matter: `a`, `ab=c` (B = C) and `x y a=b` are accepted, `a=`, `a:=b`, `a= b`, `a=q`, `i=a` are `Invalid parameter`; a later pair for the same drive wins; nothing is installed or changed on an error; `assign` alone (or with no `=`) installs or puts back the identity table.

## Finding an installed copy (361h-3ABh)

- (O) Function 35h for INT 25h, 26h and 21h into 113h, 117h, 10Fh. The 8 bytes at the INT 21h vector (ES:BX) are compared with this copy's 122h-129h (`REPE CMPSB`).
- (O) Equal: the copy at ES is ASSIGN's. 103h-10Ah are copied to ES:103h (the new table; ES:10Bh, the logical drive, is kept), then the equipment routine (3FFh) with ES:120h (the equipment word that copy saved), then function 4Ch with AL = the low byte of the word the routine stored.
- (D) A copy whose hook is no longer the first on INT 21h (another program hooked it after) is not found, and a second copy is installed in front of it (I: not run).

## Installing (3ACh-3E7h)

- (O) The equipment routine with this copy's 120h. Function 19h; AL + 1 goes to 10Bh (the logical default drive); the physical one, table[that] - 1, is selected with function 0Eh (the hook is not installed yet). INT 25h → 2D0h, INT 26h → 2D8h, INT 21h → 122h (function 25h each). Function 31h with DX = 31h, AL as function 25h left it.
- (D) While the table is the identity the hook passes every call through, 19h included. So after `assign a=b` (which selected the physical B) and a later `assign` that puts the identity back, the prompt shows `B>`: the logical drive at 10Bh is no longer answered (the host replay shows this; I: not run on the original, which can only be run then from a drive B that holds ASSIGN.COM).

## The equipment routine (3FFh)

- (O) If table entry 2 (104h, drive B) is 2, the word in AX is stored at 0040:0010h (the saved one: the BIOS's own count of drives). Otherwise INT 11h; if its bits 6-7 are 0 (one diskette drive) bit 6 is set (two); that is stored. (D) With B assigned elsewhere the BIOS claims a second drive, so DOS does not take B as A's other name on a one-drive machine (I: not run; every machine here has two drives, and the routine then writes the word it read).

## The INT 21h hook (122h)

Before the call, in this order (O):

1. AH up to 0Ch, AH = 50h or 51h, or the identity table (2E0h): straight to the old handler (`JMP FAR CS:[10Fh]`).
2. AH = 4Bh (EXEC): the path at DS:DX gets its drive letter translated (below), then straight to the old handler (the program started calls INT 21h itself, so nothing is restored).
3. Otherwise the caller's IP and CS are popped into 11Bh (its flags stay on the stack), AX and BX pushed, and by AH:
   - 0Eh (select): 10Bh = DL + 1; DL = table[DL + 1] - 1.
   - 32h, 36h, 47h (a drive in DL, 0 = default): 10Ch = DL; DL = table[DL] unless 0.
   - 39h-3Dh, 41h, 43h, 4Eh, 56h (a path at DS:DX): 10Ch = its first byte, 10Dh = DX; if the second byte is `:`, the letter is translated: (byte OR 20h) - 60h, looked up, + 40h (so the result is a capital letter for drives 1-26).
   - 0Fh-17h, 21h-24h, 27h, 28h (an FCB at DS:DX; FFh first: an extended FCB, its drive byte 7 further): a drive byte of 0 is taken as 10Bh (the logical default); 10Ch = that, 10Dh = its address; the byte becomes table[that].
   - any other function: unchanged.
4. AX and BX popped, 11Fh = AH, `CALL FAR CS:[10Fh]`: the old handler returns with `IRET`, taking the caller's flags.

After the call (22Fh), with the flags DOS returned pushed, then BX, AX, and AH = 11Fh (O):

- 32h, 47h: DL = 10Ch (the caller's drive). 0Eh: DL = 10Bh - 1. 19h: AL = 10Bh - 1 (the logical default drive).
- the path and FCB functions above: the byte at DS:[10Dh] = 10Ch (the path's letter as given; the FCB's drive byte as the logical drive: an FCB that had 0 now holds the logical default drive's number).
- 25h, 26h, 36h, the rest: nothing.
- Return: AX, BX and the flags popped, `JMP FAR CS:[11Bh]`.

(D) Consequences the cases show: COMMAND's prompt keeps the letter the user selected (function 19h is answered from 10Bh); a program COMMAND looks for on the default drive is looked for on the drive that one is assigned to (`Bad command or file name` while A is assigned elsewhere: ASSIGN itself can then only be run from the drive A goes to); function 29h (parse a name) is not translated, so COMMAND rejects a drive letter beyond the drives DOS has (`Invalid drive specification` for `c:` after `assign c=b`) before any translated call is made; `DIR` shows the logical letter but the other drive's files, label, current directory and free space. (D) Calls not translated: the second path of 56h (ES:DI), 1Ch (allocation information for DL), 44h, 4Bh's second phase, handle calls (the drive was fixed at open); none of the cases makes them through ASSIGN.

## The INT 25h/26h hooks (2D0h, 2D8h)

- (O) AL (0 = A) = table[AL + 1] - 1, then `JMP FAR` to the saved vector.

## The rewrite

`src/lib.rs` has the transient part (`run`: version, parse, find, install, the equipment routine) over a `Dos` trait, and `hook`, the hook's behaviour above as Rust over the resident copy's bytes, which the host replay (`tests/oracle_replay.rs`) puts in front of its DOS model. `src/resident.s` is the resident part itself, in assembly, at the original's offsets; `src/main.rs` is the target's `Dos`. Deliberate differences:

| ID | Difference | Why | Effect |
|---|---|---|---|
| D1 | The resident hooks (INT 21h, 25h, 26h) are assembly (`src/resident.s`), written from this specification, not Rust | the hook runs on the caller's stack with the caller's DS, must keep every register and flag, takes the return address off the stack, chains with `JMP FAR`/`CALL FAR` and must stay in the first 784 bytes at fixed offsets; compiled Rust needs DS = SS = CS, a stack frame and the runtime, and cannot be placed below the transient code (no inline assembly) | the same behaviour; `hook` in `lib.rs` is the same logic in Rust for the host |
| D2 | The resident data is at the original's offsets (100h-121h, the hook at 122h, `JMP` disp 0200h so 101h-102h are 00h 02h, 31h paragraphs kept), but the code from 122h on is the rewrite's own (shorter; zeros up to 303h) | the code was not copied | the lookup of a drive number of 20h-7Fh (a path letter of 80h or above) reads code bytes, which differ; every other lookup reads the same bytes. The 8-byte signature at 122h differs from the original's in its fifth byte (the `JBE` displacement: 1Bh, the original 2Dh), so the rewrite and the original do not recognise each other's resident copy (each installs another); neither case mixes them |
| D3 | The transient part (from 303h) is Rust with its own start (`resident.s`), the stack at the top of the program's memory as `crt0_com.s` sets it, and messages printed with function 09h from the Rust strings | Rust | none visible; exit codes as the original's (AL of 09h, of 25h, the equipment word's low byte) |

## Assumption ledger

The rewrite's code makes no assumption; these are the host replay's model of the session (`tests/oracle_replay.rs`), not of ASSIGN:

| ID | Assumption |
|---|---|
| H1 | COMMAND looks for an external command on the default drive with function 11h (an FCB with drive 0), then EXECs `X:NAME.COM` with X the default drive's letter from function 19h; `Bad command or file name` when the search fails. (The oracle shows only the outcome: any call that names the default drive, by FCB drive 0, by path without a drive or by 19h's letter, gives the same through the hook) |
| H2 | TYPE and DIR check the name's drive with function 29h (`Invalid drive specification`), TYPE opens with 3Dh and reads with 3Fh, DIR makes the label search (11h, extended FCB, attribute 08h), 47h, 11h/12h and 36h for the drive as given, in DOS 2.00's layout (a leading empty row, the size in 9 columns, no date for a directory date of 0, the count in 9 and the free bytes in 10 columns) |
| H3 | The oracle machine's equipment word reports two diskette drives (its value is not visible; ASSIGN only tests bits 6-7) |
