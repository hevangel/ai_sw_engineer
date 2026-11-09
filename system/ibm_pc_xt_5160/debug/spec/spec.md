# DEBUG (PC DOS 2.00): behaviour specification

Derived from the original `DEBUG.COM` (11,904 bytes, SHA-256 `3b5f8d71...`,
[../original/provenance.md](../original/provenance.md)), Microsoft's DEBUG for MS-DOS 2.0 built
with `IBMVER`, by reading it disassembled with GNU objdump 2.42
(`objdump -D -b binary -m i8086 -M intel --adjust-vma=0x100 DEBUG.COM`), side by side with
Microsoft's MIT-licensed source ([../reference/ms-dos-2.0/](../reference/ms-dos-2.0/README.md)),
whose names are used here. REA (spec/software-lifecycle.md) was not used: no Ghidra/IDA/Hopper
provider is installed, and the program is read in full against its matching source. Offsets are the
`.COM`'s (CS:100h is the first byte). Observations (O), derivations (D), inferences (I).

## Layout (O)

| Offset | Contents |
|---|---|
| 100h | `JMP SHORT 108h` (over a 3-byte NONAME; IBM's build has no `"Vers 2.30"` header) |
| 108h-23Eh | the start (DSTRT..COMMAND): version check, TrapParity, function 26h, the stack and screen setup, PARSE, the command-line file load |
| 208h-279h | COMMAND (the `-` prompt, the dispatch), TERMINATE/RESTART/DABORT |
| 281h-35Dh | INBUF (uppercases outside quotes), CRLF, PRINTMES, SCANP/SCANB, OUT, RBUFIN (function 0Ah), RPRBUF (function 09h) |
| 2E3h | HEXADD (H) |
| 324h-367h | OUT16/HEX/DIGIT/BLANK/TAB |
| 368h | COMTAB: 26 words, one per letter A-Z, the command routine or PERR |
| 40Bh | DUMP (D) |
| 480h-4F8h | MOVE (M), FILL (F), SEARCH (S) |
| 4FAh-58Dh | GETHEX/GETHX/HEXIN, LIST/LISTITEM, GETEOL, PERR/PERROR |
| 59Bh-5F5h | ADDRESS/GET_ADDRESS (segment:offset, segment registers), GETLIST |
| 5F6h-69Ch | ENTER (E) |
| 6A2h-746h | REG/DISPREG/DISPFLAGS (R) |
| 74Ch-862h | REG set register and flag edit |
| 863h-9B0h | TRACE (T), ZTRACE (P), DEXIT/REENTER/BREAKFIX/CONTC, SETADD |
| 9B1h-9FBh | GO (G), INPUT (I), OUTPUT (O) |
| 9FCh-B56h | SKIP_FILE/PREPNAME/OPEN_A_FILE/DELIM, IFHEX/IFEXE |
| B57h-D54h | LOAD (L)/DWRITE (W)/DEFIO/the absolute-disk and handle I/O, RDWR |
| D55h-EC5h | EXELOAD/DEBUG_FOUND/EXECERR, HEX2BIN |
| EC9h-F0Fh | COMPARE (C) |
| F11h-1484h | UNASSEM (U)/DISASLN and every disassembler opcode routine; the DISTAB, mnemonic and 8087 tables are at 1EB3h-277Dh |
| 14A2h-1E84h | ASSEM (A), the line assembler, and its tables |
| 1E85h-1EB2h | DRVERR (the disk-error message) |
| 29CEh-2A05h | the opcode dispatch tables (SHFTAB, IMMTAB, GRP1TAB, GRP2TAB, SEGTAB, FLAGTAB) |
| 2A06h | REGTAB (`AXBXCX...IPPC`), the flag names |
| 2AE2h-2CFFh | the register save area (AXSAVE..FSAVE), DSIZ/NOREGL/DISPB, the line buffer, the save flags |
| 2CE0h-2D3Fh | the switch character, parse error, DISADD/ASMADD/DEFDUMP, the disassembler work area, BYTEBUF |
| 2F08h | DATAEND; `(DATAEND+15)>>4 = 2F1h` paragraphs is the size the start uses for function 26h |

## Start (108h-234h) (O)

- Function 30h: `AH.AL` below 0200h is `Incorrect DOS version` + INT 20h. TrapParity (IBMJAPVER
  false: a no-op `RET`). Function 51h: USER_PROC_PDB = the current PDB (DEBUG's own).
  `SP = STACK`; PARSERR = AL (the invalid-drive flag).
- Function 26h with `DX = CS + (DATAEND+15)>>4`: create the user program segment just past DEBUG.
  On the oracle machine CS (= DG = the PSP) is 0600h, so the user segment is 08F1h, the segment
  the register display shows. DSSAVE/ESSAVE/SSSAVE/CSSAVE = that segment; DISADD/ASMADD/DEFDUMP =
  it:0100h. DTA = 80h.
- The stack (SAVSTK): `AX = [user segment:6]`; if `>= FFF0h` use it, so SPSAVE = AX - 2 = FFEEh
  (the register display's SP), and the user segment's words 6 and 8 are adjusted to reserve
  DEBUG's paragraphs. A word 0 is written at the stack top.
- Function 10h (INT 10h AH=0Fh): 40 columns make DSIZ=7, NOREGL=4, DISPB=64 (the 40-column
  display); otherwise the 80-column defaults stay. SET_TERMINATE_VECTOR (function 25h, INT 22h to
  TERMINATE) and INT 23h to DABORT.
- 2901h parses the first command-line parameter into the FCB at 5Ch; PREPNAME copies the tail to
  81h and parses FCB1/FCB2. A non-blank name runs DEFIO (the command-line file load).

## The command loop (208h-234h) (O)

The `-` prompt (OUT), INBUF (function 0Ah then CR LF), SCANB. A null line re-prompts. The first
non-blank character, upper-cased, must be `A`-`Z`; `SHL`, `CBW`, `CALL CS:[BX+COMTAB]` dispatches to
the routine (or PERR for an unassigned letter). `B J K P V X Y Z` and the lower half are errors
(the IBM build has no P/ZTRACE user command and no X/SETUDEV: those entries are PERR).

## Commands (O, with the oracle cases noted)

- **R** register display (DISPREG): two lines (NOREGL then the rest of 13 registers as `NN=hhhh  `),
  a blank, DISPFLAGS (8 two-letter flag names), CR LF, then one disassembled line at CS:IP with, if
  the instruction has a memory operand, `SREG:index=value` after the 35-character operand field
  (`DS:0000=CD`). `R <reg>` prints `NN hhhh`, CR LF, `:`, and reads a new value; `R F` prints the
  flags, `-`, and reads flag mnemonics (a repeated flag is `DF Error`, an unknown one `BF Error`,
  an unknown register `BR Error`). (g01, g08, g09, g12)
- **D** dump (DUMP): per 16-byte row, the address, two spaces, the bytes (`-` between bytes 7 and
  8), three spaces, 16 ASCII characters (non-printing shown as `.`); the default length is DISPB
  (128). (g02, g09)
- **E** enter: the list form stores the bytes/strings that follow the address; the interactive form
  (no list) prompts byte by byte. (g02, g04, g10)
- **F** fill: the range filled by the list, repeating. (g03, g04)
- **S** search: the address of each occurrence of the list in the range, one per line. (g03)
- **M** move / **C** compare: a block moved (overlap-safe); compare prints
  `addr1  v1  v2  addr2` for each mismatch. (g04)
- **H** hex add and subtract: `sum  difference`, 16-bit wrapping. (g05)
- **A** assemble / **U** unassemble: A prompts with the address and assembles each typed line until
  an empty line; U disassembles DISPB/4 bytes of whole instructions. (g06, g07, g12)
- **T** trace / **G** go: T single-steps (the trap flag) TCOUNT times, showing the registers between;
  G runs to a breakpoint (INT 3) or to the program's termination (`Program terminated normally`
  through INT 22h). (g07)
- **N** name / **W** write: N sets the file name; W creates it and writes BXSAVE:CXSAVE bytes from
  CS:0100h, printing `Writing hhhh bytes`. (g10)
- **L** load: the command-line form reads a `.COM`/binary into CS:0100h (CX = its size) or
  EXEC-loads a `.EXE`. (g09, g12)
- **Q** quit: INT 21h AH=4Ch. (every case)
- An unrecognised command or a bad parameter is `<spaces>^ Error`, the `^` under the offending
  character (PERR/PERROR). (g11)

## Deliberate differences from the original (the rewrite follows the binary except as noted)

- **D1 — load-high `.EXE` named DEBUG.COM.** Like EDLIN (spec/io.md), the rewrite ships as a
  load-high MZ file (`scripts/mkexe.py`) so its own code and data sit at the top of memory and the
  program segment prefix's segment (DG, 0600h on the oracle machine) stays the original's world.
  Function 26h with `DG + 2F1h` then builds the user segment at the original's 08F1h, so a debugged
  program gets the original's segments. The rewrite's code is larger than the original's, which is
  why it cannot simply load low (the original's code ends exactly at the user segment).
- **D2 — the machine side in `src/debug.s`.** INT 21h/10h, function 26h, far memory on an arbitrary
  segment, and the trace/go engine (the original's DEXIT/REENTER: set INT 1/INT 3/INT 22h, IRET
  into the program, save the registers on re-entry, catch termination) are in assembly; the Rust
  core keeps the register save area and command state.
- **D-ASM — the line assembler** encodes the instruction forms the oracle cases use
  (`MOV`/`ADD`/`INC`/`JMP`/`INT` and the other integer forms), producing the original's bytes; a
  form it does not encode is `^ Error`, as the original reports an un-assemblable line. The
  original's full table-driven assembler (including the 8087) is not reproduced.
- **D-DIS — 8087 disassembly.** The integer disassembler is the original's, table for table; an
  8087 opcode (D8h-DFh) is rendered as `DB xx`. No case disassembles an 8087 instruction.
- **D-EXEC — manual EXE load.** The original's EXELOAD shrinks DEBUG's memory block (SETBLOCK) and
  calls function 4Bh (EXEC, load without executing). A load-high rewrite that did so would free the
  memory its own code runs in. Instead the rewrite reads the `.EXE` itself, creates the program's
  PSP at `DG + 305h` (0905h on the oracle machine, where the original's EXEC places it), loads the
  image at PSP+10h, applies the relocations, and sets the registers from the header. This gives the
  original's segments (0905/0915/0939 for SORT.EXE) without the hazard.
