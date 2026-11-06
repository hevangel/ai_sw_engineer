# COMP (PC DOS 2.00): behaviour specification

Derived from the original `COMP.COM` (SHA-256 `4b30a36d...`, [../original/provenance.md](../original/provenance.md)) by reading all of its 2,523 bytes disassembled with GNU objdump 2.42 (`objdump -D -b binary -m i8086 -M intel --adjust-vma=0x100 COMP.COM`). REA (spec/software-lifecycle.md) was not used: it needs a Ghidra/IDA/Hopper provider, none is installed, and the program is small enough to read in full. No source has been published. Offsets are the original's addresses (CS:0100h is the first byte of the file). Observations are marked (O), derivations from them (D); the rules that `../scripts/cases.py` exercises are named by case and checked against the oracle.

## Memory layout of the original (O)

| Address | Use |
|---|---|
| 005Ch | the FCB DOS parsed from the first parameter at load: the pattern for search first/next |
| 0100h-05F1h | code |
| 05F2h | the second name's drive and pattern (12 bytes) |
| 05FEh, 06E8h | `X:\` and the current directory of each name's drive (function 47h), to change back to |
| 0641h, 072Bh | the two names as typed: `X:` (the default drive unless given), the text in capitals, NUL; the word after each (06C1h, 07ABh) is where its last path separator is, 0 for none |
| 06C3h | the DTA of the searches, then the first file's FCB (opened in place) |
| 07ADh | the second file's FCB |
| 07D4h-07E7h | the large buffer's size and segment, what is left in it, the two FCB pointers, the flags (07E0h), the 32-bit offset, the mismatch count, 8 hex digits |
| 07EEh-07F2h | the default drive (1 = A), `A:` `\` NUL (`\` is the path separator, 07F1h) |
| 07F3h-0957h | messages, the hex digits table |
| 0958h-1957h | the initialization code, then (once it has run) the console input buffer and the 4 KiB piece buffer |
| SP = 0100h | the stack is the top of the PSP |

## Start (958h-A01h)

- (O) Function 30h: AX must be exactly 0002h (DOS 2.00; 2.10 is refused): else `Incorrect DOS version` (AA7h) and INT 20h.
- (O) AX at entry (DOS sets AL/AH to FFh for an invalid drive in the first/second parameter): `(AH | AL) == FFh` is `Invalid drive specification` (817h) and INT 20h.
- (O) Function 37h: if the switch character is not `/`, the path separator becomes `/` (07F1h). `/` is a name delimiter only while the separator is `\`.
- (O) The large buffer: segment = CS + 196h (the paragraph after the 4 KiB piece buffer). If that is not below the top of memory (PSP:0002h) with at least 100h paragraphs to spare: `Insufficient memory` (AC2h), INT 20h. Size = min(free paragraphs, F00h) x 16, rounded down to a multiple of 4 KiB (at most F000h = 61,440 bytes; 4 KiB at least).
- (O) Function 19h: the default drive. The command tail (PSP:0080h count, text from 0081h) is split into two words (438h) into 0641h and 072Bh; the second is parsed with function 29h AL=01h into 05F2h, and FFh there is `Invalid drive specification`, INT 20h.
- (O) 438h, a word: skips delimiters (CR, space, `,`, `;`, `=`, tab, `/` as above); if the character after the first is not `:`, writes the default drive and `:` first; copies to the next delimiter or the end of the count, `a`-`z` in capitals, notes the last `\` or separator, ends with NUL. If the first word ended at a CR the scan goes back to it.

## The names (106h-148h, 402h)

- (O) If the first name is empty: `Enter primary file name` (8D2h), function 0Ah (127 characters at most, buffer at 958h), the text split by 438h and parsed into 005Ch by function 29h AL=01h. FFh is `Invalid drive specification` and the prompt again; an empty answer prompts again.
- (O) If the second name is empty: `Enter 2nd file name or drive id` (8EEh), the same into 05F2h; prompted again only for an invalid drive (an empty answer stands: case p07 answers both).
- (D) Function 0Ah echoes the keys and the CR, not an LF, so each message starts with CR LF (p07).
- (O) An FCB drive of 0 becomes the default drive's number.

## Directory and pattern of a name (49Ch)

- (O) Saves `X:\` + the current directory of the FCB's drive (function 47h) for the way back.
- (O) Empty name: a blank pattern becomes 11 `?`. Else function 3Bh on the whole name: if it is a directory (made current), the pattern is 11 `?`.
- (O) Else, with no separator in it, the name is emptied (no directory: none is changed or restored later) and a blank pattern becomes `?`. With one, the text after the last separator is parsed into the FCB (29h AL=02h; nothing after it is 11 `?`), the name is cut at the separator (after it for `X:\`), and 3Bh makes that directory current; failure, or a separator with no file name, is `- Invalid path`.
- (D) `comp a.txt a:`: 3Bh on `A:` fails under PC DOS 2.00 and there is no separator, so the second pattern is 11 `?` and the second name has no directory: `A:A       .TXT and A:A       .TXT` (p09).

## Each pair (149h-3D9h)

- (O) Function 11h on 005Ch with the DTA at 06C3h; none: CR LF LF LF (912h), the first name shown (595h), `- File not found` (8ACh), on to "Compare more" (p10). An invalid first path is shown the same way with `- Invalid path` (8C0h).
- (O) For each file found: function 0Fh on the DTA (the result is not checked), the first drive's directory restored, record size 1 and random record 0, the large buffer filled (528h), CR LF LF LF and the first name shown.
- (O) The second file's name: its pattern with each `?` replaced by the first file's character in the same place (`*.bak` against `X1.DAT` is `X1.BAK`, p08; a drive alone is the first file's name, p09). `and `, the second name shown, CR LF. On the first file only, 49Ch is done for the second name; on later ones its directory is just made current again.
- (O) 595h shows `X:`, the directory part (if any) with a separator after it, the 8-character name, `.` (a blank when the extension starts with a blank), the 3-character extension, a blank: `A:A       .TXT ` (p01).
- (O) Invalid second path: the second name again and `- Invalid path`, straight to "Compare more" without restoring the first drive's directory.
- (O) Function 0Fh on the second; its directory restored; not found: CR LF, the name, `- File not found` (p05).
- (O) Sizes from the two FCBs (offset 10h, 32 bits) differ: `Files are different sizes` (917h, p04); nothing is compared.
- (O) Then flags = 10h (later pairs), the first directory made current, function 12h for the next file (p08 has two pairs); at the end the first directory restored.

## The compare (26Dh-3A8h)

- (O) One file is in the large buffer, the other is read 4 KiB at a time (27h into 958h) and compared with it by `repz cmpsb`. When the large buffer is used up and its last fill was not short, the files swap roles and the other one fills it.
- (O) Each difference: `Compare error at offset ` (848h, by function 02h), the offset from the start of the files in hexadecimal without leading zeros (one `0` for zero), then `File 1 = hh` and `File 2 = hh` (the first file's byte, the second's) (p02). After the 10th difference: `10 Mismatches - ending compare` (7F3h) and the rest is not compared (p03).
- (O) At the end: if the last fill of the large buffer was short and its last byte is not 1Ah, `Eof mark not found` (880h, p06); then, if no byte differed, `Files compare ok` (897h, p01).
- (D) The ^Z checked is that of the file whose block was read last into the large buffer: the first when the size divided by the buffer size (rounded down) is even. For an empty file, or a size that is a multiple of the buffer, the byte checked is the one at offset FFFFh of the buffer segment, which the read did not write.

## Compare more (3DDh-401h)

- (O) `Compare more files (Y/N)? ` (936h), function 0Ch AL=01h (flushes type-ahead, reads a key with echo); the key OR 20h: `y` prompts for both names again (10Dh), `n` is INT 20h with nothing more printed (every case), anything else asks again.
- (D) COMMAND then prints CR LF and the prompt, so `A>` is on the row after `? n`.

## The rewrite

`src/lib.rs` follows these steps with the same DOS functions (FCB search, open and random block read, 29h, 3Bh, 47h, 0Ah, 0Ch, 09h and 02h), in the same order, with the same messages. Deliberate differences:

| ID | Difference | Why | Effect |
|---|---|---|---|
| D1 | The large buffer is the memory between the end of the rewrite (`__bss_end`, rounded up as the original rounds) and 2 KiB below the top of the 64 KiB segment, where `crt0_com.s` puts the stack; the original's starts at CS+196h and may reach past the segment | the shared `.COM` runtime keeps the stack at the top of the segment | the buffer is 53,248 bytes (D000h) where the original's is 61,440 (F000h) when memory is plentiful; for files over 52 KiB the file whose last block is checked for the ^Z can differ, and the `Insufficient memory` threshold is different; no case observes either |
| D2 | DOS's FCB at PSP:005Ch is copied (drive and name, 12 bytes) into the core's own FCB, which search first/next then use | the core owns its data | none |
| D3 | The state is a Rust struct starting all zero; `run` sets the initial values the original has in its image (the separator `\`) | to keep it in `.bss` | none |
| D4 | The `File 1 =`/`File 2 =` messages are patched in copies, not in place | no self-modifying data | none |
| D5 | The stack is the runtime's, at the top of the segment, not SP = 0100h in the PSP | the shared runtime | none |
