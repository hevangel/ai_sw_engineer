# FIND (PC DOS 2.00): behaviour specification

Derived from the original `FIND.EXE` (SHA-256 `947b34cb...`, [../original/provenance.md](../original/provenance.md)) by reading its code (2Ah-331h of the 5,284-byte load module) and data (0h-29h, 1400h-14A3h) disassembled with GNU objdump 2.42 (`objdump -D -b binary -m i8086 -M intel` on the bytes after the 512-byte header; the header gives CS:IP 0000:002Ah). REA (spec/software-lifecycle.md) was not used: it needs a Ghidra/IDA/Hopper provider, none is installed, and the program is small enough to read in full. Microsoft's MS-DOS 2.0 `FIND.ASM` (MIT; [../reference/ms-dos-2.0/](../reference/ms-dos-2.0/README.md)) is Rev. 8 of the same program; the binary is Rev. 6 (its revision byte at 15h is `6`), and apart from the patch area, the Kanji option (off) and the file-name error messages the code is the source's instruction for instruction. Offsets are in the load module (CS:0). Observations are marked (O), derivations from them (D); every rule below that a case in `../scripts/cases.py` exercises is checked against the oracle, and the others by the host tests in `../tests/oracle_replay.rs`.

## Memory (O)

| Offset | Contents |
|---|---|
| 0h-15h | `MAUlloa/Microsoft/V12` and the revision `6` |
| 16h | `: ` (the count's separator) |
| 18h-20h | `[` and 8 bytes for the digits of a number |
| 21h, 22h, 23h | the `V`, `C`, `N` flags (FFh when set); 24h-25h unused |
| 26h, 28h | the matched-line counter and the line counter (words) |
| 2Ah-331h | code |
| 332h, 334h | the string's length (word) and the string (128 bytes) |
| 3B4h, 3B6h | the file name's length (word) and the name (65 bytes, NUL-terminated) |
| 3F7h | the file buffer: 4,096 bytes and a guard byte |
| 1400h-14A3h | messages (below) |
| SS = 14Bh paragraphs, SP = 100h | the stack |

Messages: 1400h `Incorrect DOS version$` (function 09h); 1416h CR LF; 1418h `FIND: Invalid number of parameters` CR LF (length byte 24h at 143Ch); 143Dh `FIND: Syntax error` CR LF (14h); 1452h `FIND: File not found ` (15h); 1468h `FIND: Read error in ` (14h); 147Dh `FIND: Invalid Parameter ` (18h); 1496h CR LF `---------- ` (0Dh). Errors go to handle 2, everything else to handle 1, each piece in its own function 40h call.

## Entry (2Ah-41h)

- (O) Function 30h; `cmp al,2; jge`: a signed comparison. Below 2: `Incorrect DOS version` with function 09h (no `FIND:`, no CR LF), then `retf` to PSP:0000h (INT 20h).

## Command line (42h-116h)

- (O) Blanks (spaces only; `kill_bl`, 27Ah) are skipped. An empty tail is `Invalid number of parameters`, exit code 2.
- (O) Function 37h AL=0 gives the switch character. Then, character by character: a blank skips the blanks after it, and a CR there is `Invalid number of parameters`; a CR is that error too; the switch character takes the next character as an option: a blank is ignored, a CR is the error, `V`, `C` or `N` sets its flag after `make_caps`, anything else prints `FIND: Invalid Parameter ` + the letter (capitalized) + CR LF and the scan goes on (case f16); any other character ends the options.
- (O) `make_caps` (327h) is `cmp al,'a'; jb; cmp al,'z'; jg; and al,0DFh`: the second comparison is signed, so bytes 80h-FFh are capitalized too (D: `/` E1h reports C1h).
- (D) So each option is one letter after its own switch character (`/c/n` or `/c /n`; `/cn` makes `n` the start of the string, a syntax error).
- (O) The character after the options must be `"` (else `Syntax error`, exit 2: f10, f11). The string runs to the next `"` that is not doubled; `""` inside it is one `"` (f12); a CR before the closing quote is `Syntax error`.
- (O) An empty string ends the program at once with exit code 1, before any file is opened or anything printed (f13).
- (O) After the string: blanks; a CR means standard input (f14, f15, f21); otherwise file names follow, each ending at a blank or the CR (no other separator; a tab is part of a name).

## Files (117h-171h, 25Ah-273h)

- (O) For each name: the low bytes only of the two counters are cleared (`mov byte ptr es:[26h],0`, 28h; D: past 255 lines the high byte carries over to the next file); the name is copied and NUL-terminated; function 3Dh AL=0 opens it.
- (O) Open error: `FIND: File not found ` + the name + CR LF to handle 2, then the next name (f09).
- (O) Opened: CR LF `---------- ` + the name to handle 1, then CR LF unless `/C` (the count follows on the same line: f03).
- (O) At the end of a file: with `/C`, `: ` (for a file, not for handle 0), the count in decimal and CR LF (f03, f05, f18, f21); then, unless the handle is 0, function 3Eh closes it and the next name is taken; the end of handle 0 is the end of the program.
- (O) A read or seek error: on handle 0 the program ends (exit 0); on a file it is closed and `FIND: Read error in ` + the name + CR LF goes to handle 2, then the next name.
- (O) The exit code is 0 after the last name or the end of standard input, whatever was found.

## Reading (171h-1C0h, 22Fh-245h)

- (O) Function 3Fh reads up to 4,096 bytes to 3F7h. A read of fewer than 4,096 bytes (`cmp ax,0FFFh; jg`) that does not end with LF gets CR LF appended (the guard byte makes room): a last line without a newline is still a line and is printed with CR LF (f17).
- (O) Lines are scanned for LF (`repnz scasb`). When no full line is left, the remaining bytes (if any) are taken back with function 42h AL=1, CX:DX = minus their count, and the buffer is refilled; a read of 0 bytes is the end.
- (D) A line longer than the buffer (4,096 bytes without an LF) is read again forever: the original hangs (its source says "bigger lines will bomb the program").
- (O) The line counter is incremented at the top of the line loop (1C1h), before it is known that a full line is there, so the partial line at the end of each full buffer is counted, and counted again when it is read again (D: with `/N`, line numbers after the first 4,096 bytes run ahead by one per refill; host test `line_numbers_run_ahead_after_a_refill`).

## Matching (1D3h-206h)

- (O) A line is its bytes up to and including the LF. The search covers its length minus 2 (`dec cx` twice: the CR LF, D: or the last byte and the LF of a line ending in a bare LF). A line of length 2 (an empty line) is never a match (`jcxz`, f06 line 7).
- (O) The string is searched for byte by byte: case-sensitive, any byte value (f07: `Apple` is not `apple`; f22: a blank).
- (D) A line of length 1 (a lone LF) gives CX = FFFFh: `repnz scasb` runs over the whole 64 KiB segment from the line on and, at the latest, reaches the string's own buffer at 334h, where `repz cmpsb` compares the string with itself: a lone LF always matches (host test `a_lone_lf_matches_and_an_empty_line_does_not`).
- (O) A line qualifies when it matches, or with `/V` when it does not (f02, f05, f06).
- (O) A qualifying line is counted with `/C` (16-bit, wrapping) and not printed; otherwise `[n]` is printed first with `/N` (n the line counter in decimal, one write), then the line with its line end (one write).

## What DOS adds

- (O, the console) A function 40h write to the console stops at ^Z. FIND treats ^Z as an ordinary byte: in FRUIT.TXT (ending CR LF ^Z) the ^Z is a tenth line, which gets CR LF appended; `/V` prints it and the console shows nothing of it (f02), `/N /V` shows `[10]` with no line end after it (f06); in CTRLZ.TXT the line `^Zafter an` matches `an` and is printed, and the console shows none of it (f19).
- (O) COMMAND removes `<file` from the command tail and gives FIND the file as handle 0 (f14, f21); with `type file | find` handle 0 is the pipe's temporary file (f15; drive A must be writable).

## The rewrite

`src/lib.rs` follows these steps with the same buffer size and the same writes. Deliberate differences:

| ID | Difference | Why | Effect |
|---|---|---|---|
| D1 | The string and the file name each have a 128-byte buffer; the original's name buffer is 65 bytes, followed by the file buffer | the command tail is at most 127 bytes, so neither can overflow; no write past a Rust array | a name of more than 64 characters: the original's tail of it is overwritten by the first read, which shows only in `Read error in` for such a name (DOS 2.00 cannot open one; not observable in practice) |
| D2 | A lone-LF line is a match by rule; the original reaches that result by scanning its segment | the rewrite has no such segment layout | the same output (host test) |
| D3 | The unknown option letter is written from a local byte; the original stores it at 334h (the string buffer, overwritten by the string later) | no shared buffer | the same output |
| D4 | The buffer is a static array in the program's own segment (`main.rs`), not at 3F7h; the program is a tiny-model `.EXE` (CS = DS = SS = the PSP's segment) | the shared DOS runtime of `system/ibm_pc/platform` | none: the same size, 4,096 bytes plus the guard; exit through INT 20h for DOS 1 is the same as the original's `retf` to PSP:0000h |
