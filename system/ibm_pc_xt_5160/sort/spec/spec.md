# SORT (PC DOS 2.00): behaviour specification

Derived from the original `SORT.EXE` (SHA-256 `755a3c1c...`, [../original/provenance.md](../original/provenance.md)) by reading its 768-byte load module disassembled with GNU objdump 2.42 (`objdump -D -b binary -m i8086` on the bytes after the 512-byte header). REA (spec/software-lifecycle.md) was not used: it needs a Ghidra/IDA/Hopper provider, none is installed, and the program is small enough to read in full. Microsoft's MS-DOS 2.0 `SORT.ASM` (MIT; [../reference/ms-dos-2.0/](../reference/ms-dos-2.0/README.md)) is a later version of the same program, with a collating table and three fixes dated 3-18-83, after this binary; it names the parts (MAXREC = 256 spaces, the buffer at offset 100h) but does not define the behaviour. Offsets are in the load module (CS:0). Observations are marked (O), derivations from them (D); every rule below is exercised by a case in `../scripts/cases.py` and checked against the oracle.

## Entry and checks

- (O) 0h-18h: function 30h; if the version is below 2.00, prints `SORT: Incorrect DOS version` with function 09h and returns to DOS with `retf` to PSP:0000h (INT 20h).
- (O) 37h-3Dh: function 37h AL=0 gives the switch character, kept at 202h.

## Command line (42h-9Fh)

- (O) Two states over PSP:0081h up to the CR. First state: `+` starts a column; the switch character moves to the second state; anything else is skipped. Second state: `+` starts a column, `R` or `r` sets reverse, anything else is skipped; the parse never returns to the first state.
- (O) A column is the decimal digits after `+`, accumulated as `column = column * 10 + digit` with a 16-bit `mul` whose high word is dropped (D: modulo 65536). The first non-digit is examined in the second state, whatever the state was.
- (D) So `R` counts only after a switch character or after a column; `+n` works without a switch character; `/+0` and `/+1` are the same; a column of 70000 is column 4464.
- (O) 8Dh: key offset = column + 2, minus 1 unless that gives 2. Reverse patches the byte at 183h from 73h (`jae`) to 72h (`jb`).

## Input (A0h-101h)

- (O) Allocates 64 KiB (function 48h, BX=1000h; else the largest block), fills offsets 0-FFh with spaces, reads handle 0 to offset 102h in a loop until a read returns 0. Filling the buffer is the `Insufficient memory` error.
- (O) EFh: if the byte 3 before the end is CR the input ends there (a final CR LF ^Z is dropped); else if the last byte is ^Z it is dropped; else nothing is dropped. No bounds check (D: under three bytes of input it looks at offsets FFh-101h).

## Records (10Bh-131h)

- (O) Scans for CR; each line becomes a record: a 16-bit length (counting the length word) written over the previous line's CR LF (at 100h for the first), then the text. The byte after a CR is skipped without being checked (D: assumed LF).
- (D) Input that does not end with CR LF ^Z or ^Z but with CR LF gets a final empty record (case c10); input without a final newline does not (c11).

## Sort (132h-1C7h)

- (O) Selection sort: for each position, the best record among the rest is found and rotated into place (a backward byte move, a forward copy, a word move closing the gap).
- (O) A record's key is its bytes from the key offset to its end; a record not longer than the key offset has the key "256 bytes from offset 0+key offset" (the spaces, D: compares as blanks).
- (O) Keys are compared with `repz cmpsb` over the shorter length: unsigned bytes, case-sensitive (c01: `123` < `Apple` < `Banana` < `apple`). If one key is a prefix of the other, the shorter is smaller (`cmp ax,dx`).
- (O) The best record is replaced only by a strictly smaller one (`jae` keeps it): equal keys keep their input order (c07). With `/R` (`jb`) it is replaced by any not-smaller one: the last of equal keys wins, so equal keys come out in reverse input order (c08).

## Output (1CAh-1FFh)

- (O) Each record's length word after the first becomes CR LF; one write to handle 1 of everything from 102h to the last terminator, which is excluded (D: no CR LF after the last line). A failed or short write is the `Insufficient disk space` error.
- (O) Exit with function 4Ch, AL left from the last write (the count's low byte).

## The rewrite

`src/lib.rs` follows these steps on the same buffer layout. Deliberate differences:

| ID | Difference | Why | Effect |
|---|---|---|---|
| D1 | The buffer is a 32,767-byte static array in the program's own segment, not a 64 KiB segment from function 48h | Rust objects are limited to `isize::MAX` (32,767) with 16-bit pointers, and the tiny model has one segment | `Insufficient memory` comes at about 32 KiB of input instead of about 64 KiB; nothing else changes |
| D2 | The version message is printed with function 02h, byte by byte, not 09h | no `$`-terminated string needed | the same characters on the console |
| D3 | The reverse switch is a flag, not a patched opcode | no self-modifying code | the same comparisons |
| D4 | The two bytes at 100h-101h are zero; the original's are whatever the allocated memory held | only read when the input is under three bytes | no case observes them (c13 is empty input: those bytes are neither CR nor ^Z in either) |
