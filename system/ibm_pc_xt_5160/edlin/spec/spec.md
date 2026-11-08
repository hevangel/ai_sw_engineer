# EDLIN (PC DOS 2.00): behaviour specification

Derived from the original `EDLIN.COM` (4,608 bytes, SHA-256 `d4ea0212...`, [../original/provenance.md](../original/provenance.md)),
Microsoft's revision 12 of EDLIN 2.0 by M. A. Ulloa, by reading it in full disassembled with GNU objdump 2.42
(`objdump -D -b binary -m i8086 -M intel --adjust-vma=0x100 EDLIN.COM`), side by side with Microsoft's MIT-licensed
source of revision 14 ([../reference/ms-dos-2.0/](../reference/ms-dos-2.0/README.md)), whose names are used here; the
README there lists where the binary differs from that source. REA (spec/software-lifecycle.md) was not used: it
needs a Ghidra/IDA/Hopper provider, none is installed here, and the program is about 3,400 bytes of code read in
full against its source. Offsets are the `.COM`'s (CS:100h is the first byte). Observations are marked (O),
derivations from them (D), inferences not run (I).

## Layout (O)

| Offset | Contents |
|---|---|
| 100h | `JMP 108h`; 102h NONAME (`File name must be specified`, xERROR) |
| 108h-32Eh | the start (SIMPED to SAVEND) |
| 32Fh-48Eh | COMMAND, PARSE, COMOVER, SKIP, CHKRANGE, COMERR, GETNUM |
| 48Fh | COMTAB `QTCMWASRDLPIE;` CR (15 bytes); 49Eh TABLE, 15 words indexed by CX after `REPNE SCASB` |
| 4BCh-E58h | the routines (below) |
| E5Ah-ED0h | IBM's copyright text |
| ED1h-10C1h | the constants (CONST): `BAK`, the revision string, the flags, the names and strings (table below) |
| 10C2h-1293h | the messages (`$`-terminated) |
| 1294h-14B8h | the variables (DATA); 1294h-12FFh are still in the file (44 bytes of another program's text, then zeros), the rest is memory the load does not touch |
| 14B9h-1538h | the stack (SP = 1539h) |
| 1539h | START: the text, to LAST = PSP word 6 - 1 (FFEFh on the oracle machine) |

| Data | Offset | Use |
|---|---|---|
| fourth, loadmod | EF1h, EF2h | the fourth parameter is being read; `/B` given |
| hardch | EF3h | the INT 24h vector found |
| the_root, fudge, user_drive | EF7h, EF8h, EF9h | the file's directory is the root; the directory was changed; the drive at the start |
| optchar, dirchar | EFAh, EFBh | `-` and `/`, or `/` and `\` when the switch character (3700h) is `/` |
| userdir | EFDh | the drive's current directory (`\`, then the DPB's text), ASCIZ, 66 bytes |
| fname_buffer | F3Fh | the file name from the tail, ASCIZ (128 bytes) |
| TXT1, TXT2 | FBFh, 1040h | the search and replace strings, each a count byte and 128 bytes |
| DELFLG | 10C1h | NAME.BAK has been deleted |
| PARAM1-PARAM4 | 1294h | the command's numbers |
| OLDLEN, NEWLEN, LSTFND, LSTNUM, NUMPOS, SRCHCNT | 129Ch-12A6h | the search state |
| CURRENT, POINTER | 12A8h, 12AAh | the current line and its start |
| ONE4TH, THREE4TH, LAST, ENDTXT | 12ACh-12B2h | a quarter of memory, START + three quarters, the last usable byte, the text's ^Z |
| COPYSIZ, COPYPTR | 12B4h, 12B6h | BLKMOVE's block size and the byte below its copy |
| COMLINE, LASTLIN | 12B8h, 12BAh | the next byte of the command line; the line past the last (PAGE) |
| COMBUF | 12BCh | the command line, function 0Ah, 128 bytes |
| EDITBUF | 133Eh | a line, function 0Ah, 255 bytes; LOADBUF puts the line edited here as the template |
| EOL | 1440h | 0Ah |
| FCB2, FCB3 | 1441h, 1466h | NAME.$$$ (and NAME.BAK for its deletion); the file T merges |
| QFLG, HAVEOF, ENDING, SRCHFLG | 14B0h-14B3h | `?` given; the file's end was read (AL of the read); E is running; S (not R) |
| amnt_req, olddat, srchmod, MOVFLG | 14B4h-14B8h | the bytes asked for; keep the old string when none is given; from CURRENT + 1; M (not C) |

## Start (108h-32Eh)

- (O) ENDING = 0, SP = 1539h, AX pushed. Function 30h: AL below 2 (`JGE`, signed) is `Incorrect DOS version`.
  3700h: DL = `/` makes dirchar and userdir `\` and optchar `/`.
- (O) The tail from 81h: spaces skipped (KILL_BL); CR is NONAME. The name is copied to fname_buffer up to a space,
  CR or optchar; after a space, more spaces are skipped and optchar may follow. After optchar the letter must be
  `B` or `b` (loadmod = 1), else `Invalid Parameter`. The name is NUL-terminated.
- (O) 177h: `STD; REPNZ SCASB` backwards over the name for dirchar. None: the FCB COMMAND built at 5Ch is used and
  AL of the AX pushed at the start (FFh: COMMAND found an invalid drive) must be 0 (same_dir, 21Bh). Found:
  the_root = 1 if the separator is the name's first byte or follows `:`; it becomes a NUL (the directory part),
  fudge = 1; 19h gives user_drive; a drive in PSP:5Ch is selected (0Eh, AL = FFh is `Invalid drive or file name`);
  1Fh (the default DPB, DS:BX): its current directory cluster (1Ch) not 0 copies its text (1Eh) after the `\` of
  userdir; CHDIR (3Bh) to `\` (the_root) or the directory part, CF is `Invalid drive or file name`; INT 24h is
  hooked (3524h, 2524h to INT_24); 2901h parses the name part into PSP:5Ch and its AL must be 0.
- (O) A blank name (PSP:5Dh) is NONAME; the extension `BAK` is `Cannot edit .BAK file--rename file`. Every such
  error is xERROR (63Dh): the message (no CR LF), REST_DIR, INT 20h.
- (O) Open (0Fh) PSP:5Ch; HAVEOF = AL. Not found: create (16h; AL not 0 is `File Creation Error`, CR LF), close
  (10h), delete (13h) it again (so a directory of that name is refused), `New file` CR LF.
- (O) FCB2 = PSP:5Ch's drive and name with `$$$`; create it (16h); while that fails, DELBAK once, then
  `No room in directory for file`.
- (O) SETUP (2A4h): random record 0 and record size 1 in both FCBs, DTA = START, LAST = PSP word 6 - 1. An existing
  file: ONE4TH = (LAST - START) / 4, THREE4TH = START + 3 x that, a random block read (27h) of 3/4 of memory at START,
  SCANEOF. SAVEND: a ^Z at the end, ENDTXT there, COMBUF's size 128, EDITBUF's 255, EOL = 0Ah, POINTER = START,
  CURRENT = 1, PARAM1 = 1; an existing file: APPEND, which for a file that fitted reads nothing more and prints
  `End of input file` (every case on an existing file shows it).
- (D) A new file leaves ONE4TH and THREE4TH as the file's bytes at 12ACh: 6176h, 696Ch (`va`, `li` of
  `Invalid`); only W without a number reads ONE4TH then.

## The command loop (32Fh-3E0h)

- (O) COMMAND: SP reset, INT 23h to ABORTCOM (D5Ah), `*` (function 02h), a line into COMBUF (0Ah), LF; COMLINE =
  COMBUF + 2.
- (O) PARSE (351h): PARAM2-4 = 0, fourth = 0, QFLG = 0; four times GETNUM into PARAM*n*, spaces skipped, a `,`
  passed. Then spaces; `?` sets QFLG; a letter above 5Fh is ANDed with 5Fh; not in COMTAB is `Entry error` CR LF
  (COMERR: COMMAND). PARAM2 not 0 and below PARAM1 is an entry error. COMLINE = after the letter; the routine.
- (O) TABLE by the index from the end: CR and `;` NOCOM; E ENDED; I INSERT; P PAGE; L LIST; D DELETE; R REPLACE;
  S SEARCH (both from CURRENT + 1, srchmod 1); A APPEND; W EWRITE; M and C BLKMOVE; T MERGE; Q QUIT.
- (O) COMOVER (3C2h): after a routine, spaces skipped; CR: COMMAND; `;` or ^Z: the next command after it; anything
  else: the next command from it.
- (O) GETNUM (3FEh): `.` CURRENT, `#` FFFEh, `+n` CURRENT + n, `-n` CURRENT - n (1 if negative, `JNS`), digits
  (6553 or more before a digit is an entry error, so at most 65,529; a number 0 is one), nothing 0. The fourth
  parameter (the count) takes digits only. CHKRANGE (3E7h): a default line above a given PARAM2 is an entry error.

## Lines (646h-790h)

- (O) FINDLIN (646h): line BX (0: past the last) from CURRENT/POINTER if BX is at or after CURRENT, else from 1 at
  START; SCANLN counts LFs (`REPNE SCASB`) to ENDTXT. DX the line reached, DI its start, ZF if it is BX.
- (O) SHOWNUM (676h): a space, the number in five columns with leading zeros as spaces (CONV10, BCD), `:`, `*` for
  CURRENT else a space. DISPLAY (757h): each line after SHOWNUM, its bytes with function 02h, a control character
  other than TAB, CR and LF as `^` and the character + 40h; stops at ENDTXT or after DI lines.
- (O) LIST (72Bh): from PARAM1, or CURRENT - 11 (at least 1); PARAM2 - PARAM1 + 1 lines, or 23. A start past the
  end shows nothing (case e11, `#l`).
- (O) PAGE (6D3h): from PARAM1, or CURRENT + 1 (CURRENT if it is 1), to PARAM2 or 23 lines, at most the last line;
  CURRENT and POINTER become the last line shown, then the lines are shown (case e08).
- (O) LOADBUF (791h): a line into EDITBUF + 2 with its CR, at most 254 bytes (a longer line is cut, its CR kept);
  DX its length. UNQUOTE (D89h): each ^V (16h) is removed and the next byte, if 40h-5Fh, becomes 00h-1Fh.

## Editing (9D1h-AA4h)

- (O) NOCOM (A0Bh): COMLINE back one byte; the line PARAM1, or CURRENT + 1 (CHKRANGE), becomes CURRENT; not there or
  at the end: nothing more. Else LOADBUF, the line shown (DISPONE), SHOWNUM again, function 0Ah into EDITBUF with the
  line as the template, LF; an empty answer keeps the line; else UNQUOTE and REPLACE with the length **read before
  UNQUOTE** (D: a ^V makes it copy one more byte, the CR).
- (O) REPLACE (A68h): new bytes for old, the text moved (`REP MOVSB` up or down); an ENDTXT that would reach LAST is
  MEMERR (`Insufficient memory` with CR LF before and after, COMMAND).
- (O) DELETE (9D1h): PARAM1 or CURRENT (CHKRANGE) to PARAM2 or itself; a first line past the end does nothing; an
  empty range is an entry error; CURRENT = the first line (case e12: `9d` on 5 lines).

## Insert, merge, move, copy (B96h-CEDh)

- (O) MOVEFILE (B96h): the text from DI to its ^Z moved up to end at DX; POINTER = DI, CURRENT = BX, ENDTXT = the
  free byte below the moved text (SETPTS).
- (O) INSERT (C59h): INT 23h to ABORTINS (CB8h); the line PARAM1 or CURRENT (CHKRANGE); the text from it moved to
  LAST. Each line: SHOWNUM, 0Ah into EDITBUF (its template whatever it holds), LF; a line starting with ^Z ends
  (END_INS); else UNQUOTE, the line, CR and LF into the gap. The memory test compares EDITBUF's end with the gap
  (see the reference README): it does not fail in practice. END_INS (CCDh): the text above the gap moved down to
  POINTER, INT 23h back to ABORTCOM.
- (O) MERGE (BBAh, T): 2901h parses the name after T (AL not 0: `Invalid drive or file name` CR LF, COMMAND), COMLINE
  after it; open (0Fh; `File not found` CR LF); INT 23h to ABORTMERGE (CB1h); the line PARAM1 or CURRENT; MOVEFILE
  to LAST; DTA = POINTER, a random block read of the whole gap. AL = 1 (the end was reached): its bytes but the last
  are kept; else `Not enough room to merge the entire file` and none are. RESTORE (C37h): the moved text (without
  its last byte, the old ^Z) down after them, ENDTXT after it **with no ^Z stored** (the byte there is whatever
  memory held); close, DTA = START. CURRENT stays the merge line. INT 23h is left at ABORTMERGE until COMMAND.
- (D) Case e09 merges OTHER.TXT (`one`, `two`, ^Z) before line 3 of FRUIT.TXT: the ^Z is dropped; ENDTXT is START +
  51 (offset 156Ch), which on the oracle machine holds 5Bh, `[` (a byte of COMMAND.COM, see below), and E writes it
  as the file's last byte, so TYPE shows `[` after the last line.
- (O) BLKMOVE (AB9h, M and C; reference README): PARAM3 0 is an entry error (DEST is loaded but COMERR prints
  BADCOM, so `Must specify destination line number` is never printed); PARAM1 and PARAM2 default to CURRENT; PARAM3
  inside (PARAM1, PARAM2] is an entry error; the block copied below LAST; M: PARAM3 lowered by the block's lines if
  past PARAM2, DELETE; then per copy MOVEFILE to the copy's start, the copy, the rest down; C repeats PARAM4 times.

## Search and replace (7B3h-9D0h)

- (O) FNDFIRST (8B6h): GETTEXT (99Bh) reads the string after the letter to ^Z or CR (^V quotes the next byte: a
  quoted CR still ends) into TXT1; empty keeps the previous string (olddat) or, after a ^Z, empties it. A ^Z (or for
  S anything) starts TXT2's text: the rest to CR, the same rules; for R, COMLINE moves past it. The range: PARAM1 or
  CURRENT + 1 (CHKRANGE) to PARAM2 (the line after it) or the end; empty or shorter than the string: `Not found`.
  FNDNEXT (94Bh): `REPNE SCASB` for the first byte, `REPE CMPSB` for the rest; LSTNUM and NUMPOS the match's line.
- (O) SEARCH (854h): each match's line shown (DISPONE); QUERY (E37h): without `?` yes; with it `O.K.? `, a key
  (function 01h), CR LF; CR, `Y`, `y` are yes: CURRENT = the line (PUTCURS), done; else the next match, at the end
  `Not found`.
- (O) REPLACE (7B3h, R): per match, the new line's length over 254 is `Line too long`, done; the line is printed
  with the new text (function 02h, no `^` conversion), QUERY; yes: CURRENT = the line **before** the match's
  (PUTCURS with LSTNUM not yet advanced, case e04), REPLACE in the text, the search goes on after the new text.

## The file (4BCh-645h, CEEh-D88h, E13h)

- (O) SCANEOF (4BCh): without `/B` the read is cut at its first ^Z; with `/B` only a ^Z as its last byte is dropped,
  and a full read (as many bytes as asked) is not the end. At the end CHECK_END (4FCh) adds CR LF unless the text's
  last byte is LF, or the text is empty (the byte below START, the stack, is never LF): an empty file is one empty
  line.
- (O) APPEND (51Eh, A; at the start with PARAM1 = 1, from FILLBUF with FFFFh): HAVEOF set: `End of input file`.
  Without a number and ENDTXT at THREE4TH or above: nothing. Else a read of LAST - ENDTXT bytes at ENDTXT (none
  free: MEMERR), SCANEOF; PARAM1 lines kept (without a number: to 3/4 and one line more), a partial last line
  dropped, a word 001Ah stored at the end, the random record moved back over what was not kept. All lines kept:
  HAVEOF = 0; else `End of input file` if the end was read, nothing during E, else MEMERR.
- (O) EWRITE (5CCh, W): PARAM1 lines, or without a number the lines before the last quarter (ONE4TH) of the text;
  WRTADD (5F4h): DELBAK the first time (NAME.BAK deleted, 13h), a random block write (28h) of FCB2 from START (AL
  not 0: close, `Disk full-- write not completed`, xERROR); the rest of the text moved to START, CURRENT = 1.
- (O) ENDED (CF7h, E): ENDING = 1; WRT of every line, FILLBUF (APPEND of everything) until HAVEOF; one byte more
  written, **the byte at ENDTXT** (the ^Z, or after a T whatever is there); close; NAME renamed NAME.BAK (17h, the
  old .BAK deleted at the first write) and NAME.$$$ renamed NAME; REST_DIR; INT 20h.
- (O) QUIT (E13h, Q): `Abort edit (Y/N)? `, a key (function 01h, echoed), ANDed with 5Fh: `Y`: FCB2 closed and deleted,
  REST_DIR, INT 20h (the original file untouched, case e07); else CR LF.

## Ctrl-C and critical errors

- (O) ABORTCOM (D5Ah): the segments and SP reset, STI, CR LF, COMMAND. ABORTINS (CB8h): the same, CR LF, END_INS,
  COMOVER (the rest of the command line). ABORTMERGE (CB1h): DTA = START, then ABORTINS.
- (O) DOS's own part (case e05, `^C` at an insert prompt): function 0Ah echoes `^C`, CR LF, then INT 23h; EDLIN's CR
  LF makes an empty row before the `*`.
- (O) INT_24 (DF5h): the old handler is called with a return to INT_24_BACK (E07h); its Abort (AL = 2) runs REST_DIR
  and INT 20h, anything else is passed back to DOS. REST_DIR (DD1h): only when fudge: INT 24h restored, CHDIR to
  userdir, user_drive selected.

## Console behaviour the cases show (O)

DOS's function 0Ah echoes what is typed, a control character as `^` and the letter (`^Z` in e01 and e04), and CR as
a return to column 0; EDLIN then prints LF. Function 01h echoes the key. TYPE writes the file to the console up to
its first ^Z. COMMAND prints CR LF before each `A>`.

## The emulators (O)

The original on our Rust emulator (5150 board, `Machine::new(256)`) gives the oracle's screen on all 14 cases.
Both machines boot PC DOS 2.00 the same way: COMMAND.COM's file is loaded at 0534h:0100h (physical 5440h) and its bytes
stay in memory where EDLIN's PSP (0600h) and text later are; offset `o` of that segment holds COMMAND.COM's byte
`o` + BC0h (checked byte for byte on the Rust emulator at EDLIN's first prompt from 1539h to 393Fh, the end of COMMAND.COM,
but for the 43 bytes the start wrote; zeros after, but for the `.COM` stack at FFFBh-FFFDh). The `[` of e09 is COMMAND.COM's byte 212Ch on both.

## The rewrite

`src/lib.rs` follows these routines with the same DOS calls in the same order, the same data at the same offsets
of the same segment for everything DOS reads or writes and for the text, and the same messages. Deliberate
differences:

| ID | Difference | Why | Effect |
|---|---|---|---|
| D1 | The rewrite (12,156 bytes) is a **load-high `.EXE` named EDLIN.COM** (`scripts/mkexe.py`: an MZ header with MINALLOC = MAXALLOC = 0, .bss and a 2 KiB stack stored as zeros). PC DOS 2.00 runs it as an `.EXE` (by the `MZ` signature) and puts it at the top of memory (3CB6h:0000h, just below the EXEC code of COMMAND's transient part); `src/start.s` keeps the PSP's segment, and the core keeps the original's data group in that segment at the original's offsets (EF1h-10C1h, COMBUF, EDITBUF, EOL, FCB2, FCB3, the text from 1539h to PSP word 6 - 1) | the rewrite (10 KiB of code) is larger than the 5,177 bytes below START, so as a `.COM` it would overwrite the memory the original's text lies on, and e09 writes a byte of that memory to its file. Loaded high, the PSP's segment holds what it holds for the original | the same text memory (LAST, the 3/4 rules, `Insufficient memory`) and the same bytes in it. The PSP's segment below START holds what memory had instead of EDLIN's code: APPEND's backward scan for an LF, which can run below START only for a line longer than all memory, sees other bytes. COMBUF's template bytes 12BDh-12FFh are set to the original's file bytes (74h, zeros) |
| D2 | `src/edlin.s` makes the INT 21h calls (DS = ES = the PSP's segment), the string instructions on that segment, INT 20h (a far jump to PSP:0000) and the vectors; a Ctrl-C (INT 23h) ends the running `edlin_dos` call and the core runs the handler the original has installed (ABORTCOM/ABORTINS/ABORTMERGE) as a return through its routines instead of a jump with SP reset | Rust, no inline assembly | none: the same output and state |
| D3 | The original's scalars (1294h-12BBh, 14B0h-14B8h, EF1h-EF2h, 10C1h) are Rust fields, not bytes of the segment, and its stack (14B9h-1538h) is the rewrite's own; CHECK_END's look at the byte below START, the original's stack (the high byte of a return address, 02h or 03h), is "not LF" | the rewrite's code and stack are not in that segment | none |
| D4 | For a new file ONE4TH and THREE4TH start as 6176h and 696Ch, the original's file bytes there | they are not set for a new file | none |
| D5 | Messages are printed from the rewrite's own memory (09h with DS = its segment); `BAK` and `$$$` are stored in the FCBs from the rewrite's constants | the original's ED1h-1293h are code and text there | none: the same characters |

Assumption ledger: none (`src/lib.rs` has no `ASSUMPTION`).
