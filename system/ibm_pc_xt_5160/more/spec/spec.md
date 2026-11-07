# MORE (PC DOS 2.00): behaviour specification

Derived from the original `MORE.COM` (384 bytes, SHA-256 `fa8acd69...`, [../original/provenance.md](../original/provenance.md)) by reading all of it disassembled with GNU objdump 2.42 (`objdump -D -b binary -m i8086 -M intel --adjust-vma=0x100 MORE.COM`). REA (spec/software-lifecycle.md) was not used: it needs a Ghidra/IDA/Hopper provider, none is installed, and the program is 231 bytes of code. Microsoft's MS-DOS 2.0 `MORE.ASM` (MIT; [../reference/ms-dos-2.0/](../reference/ms-dos-2.0/README.md)) is the source of the same program built with `IBMVER EQU TRUE`; the binary differs from it in one place (S1 below). Offsets are the original's addresses (CS:0100h is the first byte of the file). Observations are marked (O), derivations from them (D); every rule a case of `../scripts/cases.py` exercises names the case.

## Memory (O)

| Address | Contents |
|---|---|
| 0100h-01E6h | code |
| 01E7h | MAXROW: 18h (24) in the file, set to 19h (25) at start |
| 01E8h | MAXCOL: 50h (80) in the file, set from INT 10h AH=0Fh at start |
| 01E9h, 01EAh | CURROW, CURCOL: 1, 1 (both count from 1) |
| 01EBh-01EFh | zero |
| 01F0h | `MORETXT`: CR `-- More --` `$` |
| 01FCh | `BADVER`: `MORE: Incorrect DOS version` CR LF `$` |
| 0217h | `CRLFTXT`: CR LF `$` |
| 021Ah | `BUFFER`: 4,096 bytes (the file's last 102 bytes are its first, zero; the rest is memory past the image) |
| SP | where DOS put it (top of the segment); the program never sets it |

## Start (100h-13Ah)

- (O) Function 30h; AL and AH swapped, compared unsigned with 0200h: below DOS 2.00, `MORE: Incorrect DOS version` (function 09h) and INT 20h.
- (O) MAXROW = 25. INT 10h AH=0Fh (the BIOS video mode): AH, the number of text columns of the current mode, becomes MAXCOL (80 on the 5160's CGA in mode 3; 40 in modes 0-1).
- (O) CR LF (function 09h): the empty row after the command line (every case). MORE does not count it.
- (O) Function 45h on handle 0 (standard input as COMMAND set it up: the file of `<`, or the pipe's temporary file, m03): the new handle (AX) is kept in BP and read from; function 3Eh closes handle 0 (BX is still 0); function 45h on handle 2 (standard error, the console) takes the lowest free handle, 0. So standard input becomes the console and the key at the prompt is read from the keyboard even when the text comes from a file or a pipe. Neither call's carry is checked.

## The loop (13Bh-1E6h)

- (O) Function 3Fh on BP, 4,096 bytes into 21Ah. AX = 0: INT 20h (the end of the input). Carry is not checked: an error code in AX is taken as a count (D: the buffer's old bytes would be shown again; not observable with the cases).
- (O) Each byte in turn (`lodsb`, SI through the buffer, CX the count left):
  - 1Ah (^Z): INT 20h at once; nothing after it is shown (m06), and a ^Z that ends a file ends MORE before the read that would return 0.
  - CR: CURCOL = 1.
  - LF: CURROW + 1.
  - BS: CURCOL - 1 unless it is 1.
  - TAB: CURCOL = ((CURCOL + 7) AND F8h) + 1, the next tab stop counted from 1; never compared with MAXCOL.
  - BEL (07h): nothing (`IBMVER`: "all characters print but bell"); every other byte, control characters included, is a printing character.
  - A printing character: CURCOL + 1; if CURCOL is then above MAXCOL, CURROW + 1 and CURCOL = 1.
  - Then the byte goes to the console with function 02h (DOS expands the TAB, the console moves back on BS and beeps on BEL: m05 shows `a       b       c`, `yz`).
  - Then, if CURROW >= MAXROW: `MORETXT` (function 09h; its CR puts `-- More --` at the start of the row the cursor is on), function 0Ch with AL = 01h (flushes the type-ahead buffer, then waits for a key from standard input, now the console, and echoes it to standard output), CR LF (function 09h), CURCOL = CURROW = 1, and (S1) SI - 1, CX + 1: the same byte is taken again.
  - CX - 1; when it reaches 0, the next read.
- (O) The state is never reset between reads: a line may span two reads.

## What that shows (D, checked by the cases)

- CURCOL counts the printing characters of a row from 1, so the 80th character of a row makes CURCOL 81 > 80: the row is counted as full when its 80th character is shown, and an 80-character line with its CR LF counts as two rows (m10). The console itself moves the cursor to the next row after the 80th character, so the two counts agree for any line, and MORE's prompt for a line that fills the screen comes right after the character that filled the 24th row (m04: `-- More --` replaces the 100-character line's 80th character on the next row, which is shown again after the key: 21 dashes).
- The prompt comes when CURROW reaches 25, i.e. after 24 counted rows: after the LF of line 24 (m02, m08, m09). The LF that triggered it is taken again after the key, so every prompt is followed by an empty row (CR LF after the key, then the LF again): m09 shows `-- More --`, an empty row, `Line 25`; m08 (exactly 24 lines) shows `-- More --` and two empty rows before COMMAND's `A>`, since the prompt comes before the end of the input is seen. The next screen starts with CURROW = 2 for that LF: 23 more lines, then the prompt (m02: lines 25-47 between the two prompts; m04, m10 for wide lines).
- 23 lines (m07) give no prompt: CURROW reaches 24.
- The key is echoed after `-- More --` (function 01h): a space or Enter (its CR) leaves the row as it is, `x` shows `-- More --x` (m03's first prompt; scrolled off by the end of the case). Any key continues; there is no way to quit but ^C (function 01h checks for it).

## Differences from the MS-DOS 2.0 source (O)

| ID | Binary | Source (`MORE.ASM` on GitHub) |
|---|---|---|
| S1 | after the prompt, `dec si; inc cx` (1DCh-1DDh) before `CHARLOOP`: the byte that filled the screen is processed and shown again | goes straight to `CHARLOOP`: the next byte; no empty row after the prompt, and line 25 would be on the row after the prompt. Microsoft's own `v2.0/bin/MORE.COM` (an `MSVER` build, 4,364 bytes with its buffer) also has the two instructions, so the published source is not the revision either binary was built from |
| S2 | the messages are at 1F0h-0219h, the buffer at 021Ah | `EXTRN MORETXT, BADVER, CRLFTXT, BUFFER` (in `MOREMES.ASM`, not in this directory) |

`IBMVER` decides the rest, as the binary shows: MAXROW 25 (not 24 for `KANJI`), MAXCOL from INT 10h AH=0Fh, only BEL not counted (the `MSVER` build does not count any byte below 20h and has no INT 10h call).

## The rewrite

`src/lib.rs` follows these steps with the same calls in the same order (30h, 09h, INT 10h AH=0Fh, 09h, 45h, 3Eh, 45h, then 3Fh, 02h, 09h, 0Ch/01h, 09h, INT 20h), the same messages, the same 4,096-byte reads and the same byte arithmetic on CURROW and CURCOL (8-bit, wrapping as `inc byte ptr` does). Deliberate differences:

| ID | Difference | Why | Effect |
|---|---|---|---|
| D1 | The buffer is a static array in the rewrite's `.bss` (`main.rs`), not at 021Ah | the shared `.COM` runtime of `system/ibm_pc/platform` | none: the same size and the same reads |
| D2 | MAXROW, MAXCOL, CURROW, CURCOL are locals of `run`, starting at the values the original has after its start (25, the INT 10h columns, 1, 1); the file's MAXROW of 24 is never used by either | the core owns its data | none |
| D3 | INT 10h AH=0Fh is made by `more_video_columns` in `src/video.s` (the shared `ibm_pc_dos` crate has INT 21h only), which returns AH | no inline assembly on the msp430 Rust route | none: the same BIOS call |
| D4 | The stack is where `crt0_com.s` puts it (the top of the program's memory, at most the top of the segment); the original leaves SP where DOS put it | the shared runtime | none |

No `ASSUMPTION` is open: every rule above is in the binary.
