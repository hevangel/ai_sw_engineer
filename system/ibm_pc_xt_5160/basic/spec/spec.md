# IBM PC BASIC (Cassette BASIC C1.10 + Disk BASIC D2.00): behaviour specification

Phase 1 of [issue #301](https://github.com/hevangel/ai_sw_engineer/issues/301), written 2026-11-09. The
rewrite is **one** program, `BASIC.COM`, that behaves like the original pair — Disk BASIC D2.00 (BASIC.COM of
PC DOS 2.00, 16,256 bytes) running on top of Cassette BASIC C1.10 in the XT's ROM at F600:0000 — on the
committed cases, without needing the ROM. I/O contract, memory map and DOS calls: [io.md](io.md). Plan:
[../plans/implementation_plan.md](../plans/implementation_plan.md).

Sources, in order of authority:

1. **The oracle**: the IBM binaries running on MartyPC's IBM 5160 (`oracle/transcripts/`, 31 cases).
2. **The binaries themselves** (disassembled with `objdump -b binary -m i8086`, scripts in this document's
   notes; REA was not used in this phase, see U1): tables and strings are quoted from them with offsets.
3. **Microsoft's GW-BASIC source** ([../reference/README.md](../reference/README.md), MIT, commit `edf82c2`):
   the same code base as IBM's BASIC (its `IBMRES.ASM` builds "GW-BASICs with IBM PC compatible reserved word
   tokens"). It explains *how*; where it disagrees with the oracle, the oracle wins and the difference is
   listed in the ledger. Routine names below (`$FOUT`, `PTRGET`, ...) are GW-BASIC's.

Marks: (O) observed in an oracle case, (B) read from the binary, (G) taken from GW-BASIC's source and not yet
checked against the oracle, (U) unknown.

## 1. The original's structure (B)

- **Cassette BASIC C1.10** (32 KiB at F600:0000-7FFF, banner `The IBM Personal Computer Basic` /
  `Version C1.10 Copyright IBM Corp 1981` at ROM 4C9Bh) holds the interpreter: tokenizer, evaluator, MBF
  math, strings, screen editor, graphics primitives, cassette I/O, the reserved-word table (ALPTAB at ROM
  0103h, words from 0137h), error messages 1-30 and 50-67 (from ROM 03B5h), `?Redo from start` (1510h),
  the initial soft-key strings (4CEDh) and a 8Eh-byte block of initial RAM values (069Ah).
- **BASIC.COM D2.00** (PC DOS 2.00 disk 1, dated 1983-03-08) is a patch layer: `JMP 39E6h` to its start
  code, which checks DOS ≥ 2 (`Incorrect DOS version`), refuses to run as a child of itself (`You cannot run
  Basic as a Child of Basic`), sizes memory, copies the ROM's RAM block to DS:0000, and points the ROM's
  RAM hooks at its own code; a table of 54 far jumps (`EA xx xx 00 F6`, e.g. at 0E62h-0EDAh) enters ROM
  routines. It adds the disk devices and every FE/FD-prefixed statement and function (its own extended
  reserved-word table, ALPTAX at file offset 021Dh), messages 68-76, SHELL (`COMSPEC`, `Can't continue after
  SHELL`), `Advanced Feature` for BASICA-only statements, and the banner `Version D2.00 Copyright IBM Corp.
  1981, 1982, 1983`.
- The rewrite merges both: one tokenizer table, one statement dispatcher, no ROM calls. Nothing of the
  cassette-only behaviour (CAS1:, MOTOR) is required beyond what BASIC.COM exposes.

## 2. Start, banner, exit (O, B)

- `BASIC [file] [/F:n] [/S:n]` (only the defaults are covered by cases). With a file name BASIC loads and
  runs it (`.BAS` appended when the name has no extension).
- The screen is cleared; the banner is (b09, b29):
  ```
  The IBM Personal Computer Basic
  Version D2.00 Copyright IBM Corp. 1981, 1982, 1983
  61766 Bytes free

  Ok
  ```
  from row 1, with the function-key line on row 25 (§9). `61766` is the free memory of §3 on the oracle
  machine (256 KiB).
- `SYSTEM` closes all files and returns to DOS (INT 21h function 4Ch, BASIC.COM 295Fh); the key line is
  erased, the screen is otherwise left as is (b01-b07 end with the program's output, a blank row, then `A>`).
- `Incorrect DOS version` and the child-of-BASIC refusal are not covered by cases (DOS 2.00 only).

## 3. Memory layout of the data segment (B, O)

BASIC's data segment ("DS", DEF SEG's default) is `PSP + 403h` paragraphs, i.e. it overlaps the last bytes of
BASIC.COM's start code (discarded after start). Its size is `min(memory available after the PSP - 403h
paragraphs, 1000h)` paragraphs; SP starts at the top word (`size*16 - 2`, FFFEh with 256 KiB), the BASIC
stack is the 512 bytes below it, and the top of BASIC's memory (MEMSIZ, DS:030Ah) is `SP - 200h` = FDFEh.

| Offset | Contents | Evidence |
|---|---|---|
| 0000h-008Dh | initial values copied from ROM 069Ah (RND seed `52 C7 4F 80` at 0007h, ...) | B (3A51h-3A60h) |
| 008Eh-08D3h | zeroed at start (846h bytes) | B |
| 0030h | TXTTAB, the start of program text: **0CB6h** with the defaults /F:3 /S:128 | O b28 (`PEEK(&H30)+256*PEEK(&H31)` = 3254) |
| 0358h | VARTAB (end of program = start of simple variables) | O b28 (3456 after the 6-line MEM.BAS) |
| 01F9h | the command tail copied from PSP:0080h (50h bytes) | B |
| 030Ah | MEMSIZ-like top (SP - 200h) | B |
| 0350h | DS itself (the segment) | B |
| 0700h-0730h | BASIC.COM's own variables: environment segment (0723h), its size (0725h), saved INT 0, 4, 1Bh, 1Ch vectors (0712h-071Ah), the switch character (0721h), the video mode at start (0048h, 071Ah) | B (3A79h-3B4Ch) |
| ...-0CB5h | file control blocks for /F files with /S-byte buffers, then the 0 before the first line | U (the formula for TXTTAB from /F and /S: A3) |
| TXTTAB | program lines (§5), ending with a zero link | O |
| VARTAB | simple variables (§6), then arrays (ARYTAB), then free memory | O b27, b28 |
| ... FRETOP..MEMSIZ | string space, growing down | G |

- **Bytes free** = MEMSIZ - VARTAB - 2 at start? On the oracle: FDFEh - 0CB8h = F146h = 61766, with VARTAB =
  TXTTAB + 2 (the empty program's zero link). Formula as GW `FRE`: the space between the top of arrays
  (STREND) and the bottom of strings (FRETOP), less nothing at start. `FRE(0)` after MEM.BAS's line 30 is
  61526 (b28); `CLEAR ,40000` sets the top so that `FRE(0)` = 36032 (b28) (ASSUMPTION A1: MEMSIZ = n - 200h stack
  as GW's CLEAR; to confirm when implemented).
- `FRE("")` compacts strings first (b12: `FRE(0)`=60775, `FRE("")`=60790, `FRE(1)`=60790).
- The rewrite keeps this segment as a separate 64 KiB far segment with the original's offsets for everything
  a program can observe with PEEK, VARPTR, FRE and the protected-file encoding (TXTTAB, VARTAB, line links,
  variable entries); its own interpreter state lives in its Rust data segment (deliberate difference D2).

## 4. Tokens (B)

Statements and operators are one byte 81h-F4h; functions are two bytes `FF xx`; the disk extensions are `FE
xx` (statements) and `FD xx` (functions). Decoded from the ROM's ALPTAB and BASIC.COM's ALPTAX (script in
the phase-1 notes, U2); all agree with GW-BASIC's `IBMRES.ASM`.

| Token | Word | Token | Word | Token | Word | Token | Word |
|---|---|---|---|---|---|---|---|
| 81 | END | 9E | LLIST | B9 | RANDOMIZE | D4 | ERL |
| 82 | FOR | A0 | WIDTH | BA | OPEN | D5 | ERR |
| 83 | NEXT | A1 | ELSE | BB | CLOSE | D6 | STRING$ |
| 84 | DATA | A2 | TRON | BC | LOAD | D7 | USING |
| 85 | INPUT | A3 | TROFF | BD | MERGE | D8 | INSTR |
| 86 | DIM | A4 | SWAP | BE | SAVE | D9 | `'` (REM; stored `:` 8F D9) |
| 87 | READ | A5 | ERASE | BF | COLOR | DA | VARPTR |
| 88 | LET | A6 | EDIT | C0 | CLS | DB | CSRLIN |
| 89 | GOTO (also `GO TO`) | A7 | ERROR | C1 | MOTOR | DC | POINT |
| 8A | RUN | A8 | RESUME | C2 | BSAVE | DD | OFF |
| 8B | IF | A9 | DELETE | C3 | BLOAD | DE | INKEY$ |
| 8C | RESTORE | AA | AUTO | C4 | SOUND | E6 | `>` |
| 8D | GOSUB | AB | RENUM | C5 | BEEP | E7 | `=` |
| 8E | RETURN | AC | DEFSTR | C6 | PSET | E8 | `<` |
| 8F | REM | AD | DEFINT | C7 | PRESET | E9 | `+` |
| 90 | STOP | AE | DEFSNG | C8 | SCREEN | EA | `-` |
| 91 | PRINT (`?` too) | AF | DEFDBL | C9 | KEY | EB | `*` |
| 92 | CLEAR | B0 | LINE | CA | LOCATE | EC | `/` |
| 93 | LIST | B1 | WHILE | CC | TO | ED | `^` |
| 94 | NEW | B2 | WEND | CD | THEN | EE | AND |
| 95 | ON | B3 | CALL | CE | TAB( | EF | OR |
| 96 | WAIT | B7 | WRITE | CF | STEP | F0 | XOR |
| 97 | DEF | B8 | OPTION | D0 | USR | F1 | EQV |
| 98 | POKE | | | D1 | FN | F2 | IMP |
| 99 | CONT | | | D2 | SPC( | F3 | MOD |
| 9C | OUT | | | D3 | NOT | F4 | `\` |
| 9D | LPRINT | | | | | | |

9A, 9B, 9F, B4-B6, CB, E0-E5 are unused (padding in the dispatch tables; they list as garbage, G).

`FF` functions: 81 LEFT$, 82 RIGHT$, 83 MID$, 84 SGN, 85 INT, 86 ABS, 87 SQR, 88 RND, 89 SIN, 8A LOG, 8B EXP,
8C COS, 8D TAN, 8E ATN, 8F FRE, 90 INP, 91 POS, 92 LEN, 93 STR$, 94 VAL, 95 ASC, 96 CHR$, 97 PEEK, 98 SPACE$,
99 OCT$, 9A HEX$, 9B LPOS, 9C CINT, 9D CSNG, 9E CDBL, 9F FIX, A0 PEN, A1 STICK, A2 STRIG, A3 EOF, A4 LOC,
A5 LOF.

`FE` statements (BASIC.COM): 81 FILES, 82 FIELD, 83 SYSTEM, 84 NAME, 85 LSET, 86 RSET, 87 KILL, 88 PUT, 89 GET,
8A RESET, 8B COMMON, 8C CHAIN, 8D DATE$, 8E TIME$, 8F PAINT, 90 COM, 91 CIRCLE, 92 DRAW, 93 PLAY, 94 TIMER,
95 ERDEV, 96 IOCTL, 97 CHDIR, 98 MKDIR, 99 RMDIR, 9A SHELL, 9B ENVIRON, 9C VIEW, 9D WINDOW, 9E PMAP.
`FD` functions: 81 CVI, 82 CVS, 83 CVD, 84 MKI$, 85 MKS$, 86 MKD$.

In Disk BASIC the BASICA-only statements (PAINT, COM, CIRCLE, DRAW, PLAY, ...) give error 73 `Advanced
Feature` (O b18: `play "c"`). Which words exactly are "advanced" in D2.00: U3 (GW `ADVGRP`; to be read from
BASIC.COM's dispatch).

### Constants in program text (O b19, b20)

| Byte | Meaning |
|---|---|
| 11h-1Ah | the integers 0-9 |
| 0Fh n | integer 10-255 |
| 1Ch lo hi | integer 256-32767 (and a negative never occurs: `-` is the operator EAh) |
| 1Dh 4 bytes | single (MBF) |
| 1Fh 8 bytes | double (MBF) |
| 0Bh lo hi | octal constant `&O`, listed `&O7` |
| 0Ch lo hi | hex constant `&H`, listed `&H1F` |
| 0Eh lo hi | a line number (after GOTO, GOSUB, THEN, ELSE, RUN, LIST, RESTORE, RESUME, ERL comparisons ...) |
| 0Dh lo hi | a line *pointer* (a 0Eh line number replaced while running; turned back by SAVE/LIST, GW `SCCPTR`) |

Crunching rules seen (b20): `7%` is stored as the digit token `18h` and lists as `7`; keywords are upper-cased, variable names upper-cased, spaces kept as typed
(`40 A = B+C  :   REM  spaces`), `?` becomes PRINT, `1to10` splits into `1TO10`, `'` stores `:` REM `'`
(`3A 8F D9`) and lists as `'`. Numbers are re-listed from their stored value: `1e5` → `100000!`, `.5e-3`
→ `.0005`, `1.234567890123d2` → `123.4567890123#`, `123456789` → `123456789#` (more than 7 digits makes a
double), `65000` → `65000!` (does not fit an integer, so single), `12345!` keeps `!`, `7%` lists as `7`.
A line feed (Ctrl-J) inside a line is kept and lists as a line break (b21, `70 PRINT` / `"x"`).

## 5. Program lines (O b19, b28)

`link(2) number(2) tokens... 00`, links absolute offsets in DS to the next line, the program ends with a zero
link. Example (b28, line 10 at 0CB6h): `E4 0C 0A 00 91 20 FF 97 28 0C 30 00 29 E9 1C 00 01 EB FF 97 28 0C 31 ...`.
Lines are kept sorted; typing a number alone deletes that line (`Undefined line number` if it does not exist,
b21); typing an existing number replaces it.

## 6. Variables (O b27, b28; G for the rest)

- Types: integer (2 bytes), single (4, MBF), double (8, MBF), string (3-byte descriptor: length, offset).
  Suffixes `% ! # $`; default single; `DEFINT/DEFSNG/DEFDBL/DEFSTR` letter ranges. `A`, `A!` are the same
  variable; `A%`, `A#`, `A$` are distinct (b27).
- Names are significant to 40 characters (`ABCDEFGH` ≠ `ABCDEFGX`, b27).
- Simple variable entry (b27, b28): `type(1: 2,3,4,8) name(2) extra-length(1) extra-chars value`; VARPTR
  points at the value. Entries are appended in creation order from VARTAB (b28: A at 3456 → VARPTR 3460; B$
  3468; deltas -8 and -12 in b27).
- Array entry: `type name(2) extra-length extra-chars size(2) ndims(1) dims(2 each, last first?) values`
  (b28: `DIM C(3)` after A and B$ gives VARPTR(C(0)) = 3480 = 3471 + 9). Implicit dimension 10 (b27
  `X(10)=7`), `OPTION BASE 1`, `ERASE`, `Duplicate Definition` on re-DIM (b13 err 10), `Subscript out of
  range` (9).
- String space grows down from the top; garbage collection compacts (FRE("")). Strings at most 255 (`String
  too long`, 15). Literal strings in program text are referenced in place (G).
- `DEF FN` functions, single-line, parameters local (b27 `FNA$`, `FNB(X,Y)=X*Y+A`); undefined: error 18.
- `SWAP`, `COMMON`/`CHAIN` keep COMMON variables (b25: `42 kept 0`, C not common).

## 7. Numbers (O b01, b03, b10-b12; G for the algorithms)

### Formats

- **MBF single**: 4 bytes little-endian; byte 3 = exponent with bias 80h (0 means the value 0), byte 2 bit 7
  = sign, 23 mantissa bits with an implied leading 1 at the binary point: value = 0.1mmm…₂ × 2^(e-80h).
  The largest value, printed for an overflow, is 1.701412E+38 (exponent FFh, mantissa all ones).
- **MBF double**: 8 bytes, the same with 55 stored mantissa bits.
- Integers: 16-bit two's complement.

### Arithmetic

- The rewrite ports GW-BASIC's MATH1/MATH2 routines bit for bit (`$FADDS`, `$FMULS`, `$FDIVS`, the double
  forms, `$NORMS/$NORMD` with the extra rounding byte, `$INT`, `$FIN`, `$FOUT`, `$SIN`, `$LOG`, `$EXP`,
  `$SQR`, `$FPWR`, `$RND`). Single results are rounded from a 32-bit work mantissa (GW's FAC-1 byte), not
  IEEE: `CSNG(1#/3)` = `.3333334` and `CDBL(1/3)` = `.3333333432674408` (b03), `CDBL(.1)` =
  `.1000000014901161` (b10).
- Type of a result: the wider operand; `/` and `^` and the transcendental functions are single unless an
  operand is double; `\` and `MOD` are integer (operands rounded with CINT, overflow error 6 if outside
  -32768..32767). Integer `+ - *` overflowing 16 bits gives a single (`32767+1` = `32768`, b10).
- Rounding to integer (CINT, assignment to `%`, `\`, MOD operands): half away from zero (`CINT(2.5)`=3,
  `CINT(-2.5)`=-3, `CINT(3.5)`=4, `X%=3.7` → 4, b03, b10). `INT` floors, `FIX` truncates.
- `-7\2` = -3, `7 MOD -3` = 1, `-7 MOD 3` = -1 (b10). Logical operators on 16-bit integers: `5 IMP 3` = -5,
  `5 EQV 3` = -7, true = -1 (b10).
- `^`: `2^10`=1024, `2^.5`=1.414213 (not 1.414214), `2^31`=2.147484E+09, `10^-3`=9.999999E-04, `0^0`=1,
  `(-2)^3`=-8, `1.5^2.5`=2.755676 (b03, b10): computed as EXP(y*LOG(x)) except for integer exponents
  (repeated multiplication) — G `$FPWR`.
- **Float overflow and division by zero are warnings, not errors** (b03, b06, b09, b13): the message
  (`Overflow`, `Division by zero`) is printed at the cursor, then CR LF and an empty line? (O: one blank row
  follows), the result is ±1.701412E+38 (machine infinity) and execution continues; `ON ERROR` does not see
  them (b06, b13). Integer division by zero (`\0`) is the same warning (b13 line 220). Integer overflow on
  assignment (`X%=40000`) is the trappable error 6 (b13).
- RND: `RANDOMIZE 7` then RND gives `.6801813 .4909315`, `RND(0)` repeats the last (`.4909315`), `RND(-3)`
  reseeds (`.8188288`), then `.2677991` (b10). GW `$RND` with IBM's seed table (ROM data at DS:0007h, U4).

### FIN: numbers in text (G, O)

`$FIN` accumulates digits into the FAC (×10 + digit) and applies the exponent by repeated ×10 / ÷10 in the
working precision; this is why `1E+38` reads back as `9.999999E+37` (b10). More than 7 significant digits,
a `D` exponent or a `#` suffix makes a double; `!` forces single; `%` integer. `&H`/`&O`/`&` prefixes give
integers (≥ 8000h wraps negative: `&H8000` = -32768, `&HFFFF` = -1, b10). VAL ignores spaces inside the
number (`VAL(" 1 2 ")` = 12) and stops at the first bad character (`VAL("1E2x")` = 100); `VAL("&H10")` = 16.

### FOUT: PRINT and STR$ (O b01, b10, b12)

- Positive numbers get a leading space, negative a `-`; PRINT adds a trailing space after every number;
  STR$ has the leading space but no trailing one (`STR$(42)` = ` 42`, `STR$(-0)` = ` 0`).
- Leading zero before the point is dropped (`.5`, `-.5`, `.1428571`).
- Single: up to 7 significant digits, double up to 16, trailing zeros removed. Fixed notation when the number
  can be written with at most 7 (16) digits in total, counting the zeros after the point for a fraction
  (`999999`, `9999999`, `.0000001`, `.00000000015#`); otherwise scientific with a mantissa `d.dddddd`, `E`
  (single) or `D` (double), sign and two digits: `1E+07`, `1E+16`, `1.234567E-20`, `9.999999E-04`,
  `-1D+30`, `1D+16`, `1.234567890123457D+16`.
- Doubles whose value has at most 7 digits print as such (`100#` → `100`).
- Integers print in decimal; `-32768` fine; `ABS(-32768)` = `32768` (single).
- HEX$/OCT$ of a negative integer is two's complement (`FFFF`, `177777`); of a non-integer, rounded
  (`HEX$(1.7)` = `2`) (b12).

### PRINT USING (O b01, b11; G BIPRTU)

Field characters `#`, `.`, `,` (thousands separator anywhere left of the point), `+` (leading or trailing),
`-` trailing, `**` (fill), `$$`, `**$`, `^^^^` (four or five: E+nn / E+nnn — with five carets one caret
is printed literally? O: `##.##^^^^^` with 1.5E+30 gives ` 1.50E+30^`), `!`, `\  \` (n+2 characters), `&`,
`_` (literal next character). Observed:

| Format | Values | Output |
|---|---|---|
| `##.##` | 1.005; -1.5; 123.456 | ` 1.01-1.50%123.46` |
| `**#.#` | 2.3; -2.3 | `**2.3*-2.3` |
| `**$##.##` | 4.5 | `***$4.50` |
| `###.##-` | -12.3; 12.3 | ` 12.30- 12.30 ` |
| `#.##^^^^` | 12345; -.00012 | `0.12E+05-.12E-03` |
| `!` / `\  \` / `&!` | | `hw`, `abcdab  `, `abc` |
| `_####` | 5 | `#  5` |
| `Total: ###,###.##` | 1234567.891 | `Total: %1,234,567.89` |
| `##` | 12345; -5 | `%12345-5` |
| `+##  ##+` | 5;-5;5;-5 | ` +5   5- +5   5-` |
| `$$###.##` | -12.5; 12.5 | ` -$12.50  $12.50` |
| `.###` | .5555; 1 | `.556%1.000` |
| `#.#` | 1E+38 | `% 9.999999E+37` (an overflowing field prints `%` and the number as PRINT would) |
| `###.#` | 9999.95 | `%10000.0` |
| `a#b,` | 1; 2 | `a1b,a2b,` (the format is reused for each value) |
| `###.##  $$#,###.##  +#.###^^^^` | 3.14159; 1234.5; -.000123 | `  3.14   $1,234.50  -1.230E-04` |

## 8. Strings (O b01, b12)

LEFT$, RIGHT$, MID$ (function and statement; `MID$(A$,8)="Earth!!!"` replaces only up to the end: `HOWDY,
Earth`), INSTR (`INSTR(A$,"")` = 1, past the end 0), STRING$(n, code or string), SPACE$, SPC(0), LEN, ASC
(`ASC("")` error 5), CHR$, STR$, VAL, HEX$, OCT$, MKI$/MKS$/MKD$ and CVI/CVS/CVD (the raw bytes), comparisons
by character code then length (`"ab"<"abc"` true). Concatenation beyond 255 is error 15.

## 9. Screen, keyboard and the screen editor (O b08, b21-b24, b29, b30; G SCNDRV/SCNEDT)

- Text screen 80×25 (WIDTH 40 switches to 40 columns and clears, b30). Row 25 is the function-key line when
  KEY ON (the default): 10 (80 columns) or 5 (40 columns) fields of 8 columns, digit (`0` for F10) and the first
  6 characters of the key's string, control characters shown as a glyph (CR is shown as blank in the
  transcripts, O-K1): `1LIST   2RUN    3LOAD"  4SAVE"  5CONT   6,"LPT1 7TRON   8TROFF  9KEY    0SCREEN`.
- KEY LIST prints `F1 hello` ... `F10 SCREEN 0,0,0` (b24); `KEY n,"text"` redefines (b29).
- PRINT: `;` no space, `,` 14-column zones (b10 `1 ... 6`), TAB(n) moves to column n (next line if past),
  SPC(n); a line longer than the width wraps (b24: 85 `=` signs → 80 + 5). LOCATE row,col, CSRLIN, POS(0),
  SCREEN(row,col[,1]) (character / attribute: `a 97 7`, b24). CLS.
- **Screen editor** (direct mode and INPUT): the logical line under the cursor is entered with Enter wherever
  the cursor is. Control keys covered (b21): Ctrl-H backspace, Ctrl-K home, Ctrl-L clear screen, Ctrl-B
  previous word, Ctrl-F next word, Ctrl-N end of line, Ctrl-E erase to end of line, Ctrl-R insert mode,
  Ctrl-J line feed inside the logical line, Ctrl-C cancels the line being typed (and ends AUTO, b22).
  Typing over a listed line and pressing Enter re-enters it (b21: `^K2` turns `10` into `20`). Ctrl-C does
  **not** interrupt a running program (observed in a first draft of b21: a `GOTO` loop was not stopped);
  Ctrl-Break does (INT 1Bh), which the harness cannot type (U5).
- INPUT: prompt string then `? ` (`INPUT "";C` still shows `?`, b23); too few or too many values, or a
  non-number for a numeric variable, print `?Redo from start` and ask again (b23); quoted strings may contain
  commas. LINE INPUT takes the whole line. INPUT$(n[,#f]).

## 10. Statements and commands (O as listed; G otherwise)

Direct mode prompt `Ok`; `RUN`, `LIST [range]` (`list 20-`), `LOAD`, `SAVE`, `NEW`, `AUTO` (`10 `, `20 `
... shown, b22), `RENUM new,old,inc`, `DELETE`, `EDIT`, `CONT` (after STOP: `Break in 105`, then `cont`
continues, b22), `TRON` (`[100]a`), `TROFF`, `CLEAR`, `FILES`, `KILL`, `NAME`, `SYSTEM`, `SHELL` (not covered:
EXIT did not come back within a case). Program statements: everything in §4; covered by the cases: FOR/NEXT
(`NEXT J,I`, STEP .5, zero-trip loop leaves the variable at the start value: `FOR Q=1 TO 0` → Q=1, b27),
GOSUB/RETURN, WHILE/WEND, IF/THEN/ELSE, ON GOTO, READ/DATA/RESTORE, DIM/ERASE/OPTION BASE, DEF FN, SWAP,
ON ERROR GOTO/RESUME/RESUME NEXT/ERR/ERL/ERROR n, RANDOMIZE n, KEY, LOCATE, CLS, WIDTH, CHAIN [MERGE] with
COMMON (b25), all file statements (§11).

## 11. Files and devices (O b04-b07, b19, b25, b26; io.md for the DOS calls)

- `OPEN mode,#n,name[,reclen]` and `OPEN name FOR INPUT|OUTPUT|APPEND AS #n [LEN=]`; modes I, O, A, R.
  Default 3 files (`#4` is error 52, b26). Devices `SCRN:`, `KYBD:`, `LPT1:`-`LPT3:`, `COM1:`-`COM2:` (B
  strings), and files (default drive A:, paths allowed in D2.00).
- Sequential: PRINT# (as to the screen, with TAB and USING; numbers keep their spaces), WRITE# (quoted
  strings, comma separated, numbers without spaces: `1,"item 1",.25`), INPUT# (comma/CR separated),
  LINE INPUT#, INPUT$, EOF, LOC (records of 128 bytes: `LOC(2)` = 1 after reading 5 bytes, b26), LOF.
  Text written ends with CR LF per line and **a 1Ah is added at CLOSE** (G; TYPE shows the text, b04).
- Random: FIELD (`FIELD overflow` 50 if the fields exceed the record), LSET/RSET, PUT/GET [#n,record],
  LOC (last record), LOF (bytes, b05 `60`), `Bad record number` 63 for record 0.
- Errors seen: 52, 53, 54 (GET on a sequential file), 55 (OPEN of an open file; also NAME onto an open file,
  b26), 58, 62, 63, 64 (`BAD*.X`).
- `FILES [spec]`: prints the current directory (`B:\`), names as `NAME    .EXT` in columns of 18, then
  ` 360448 Bytes free` (disk free space, INT 21h 36h), b07.
- KILL, NAME ... AS.

### SAVE and LOAD formats (O b08, b19)

- **Tokenized** (default): `FF`, then the program bytes from TXTTAB to VARTAB (links as absolute DS offsets,
  line numbers in 0Eh form), then `1A`. B:T.BAS of b19 is 112 bytes = 1 + 110 + 1.
- **ASCII** (`,A`): the LIST text, each line ending CR LF, then `1A` (b08 shows the TYPE).
- **Protected** (`,P`): `FE`, the program bytes encoded, `1A`. Encoding (GW `PENCOD`, verified on all 110
  bytes of b19): two counters, `c1` from 11 down to 1 and `c2` from 13 down to 1, both decremented per byte and
  reloaded at 0; `enc = ((p - c2) XOR K1[c1] XOR K2[c2]) + c1` (mod 256), with `K1[1..11]` = `7C 88 59 74 E0 97
  26 77 C4 1D 1E` and `K2[1..13]` = `9A F7 19 83 24 63 43 83 75 CD 8D 84 A9` (GW's `$EXPCN` and `$LOGP`
  constant tables; found in the ROM at 6217h+1 and 6176h+1). LOAD decodes (the inverse); a protected program
  cannot be LISTed, EDITed or SAVEd (`Illegal function call`, b19).
- LOAD of an ASCII file tokenizes each line; MERGE and CHAIN MERGE need ASCII (`Bad file mode` otherwise, G).

## 12. Errors (O b06, b09, b13-b18, b31; B ROM 03B5h, BASIC.COM)

Direct mode prints the message; in a program `message in line` (`RESUME without error in 230`, b13), then
`Ok`. Unknown numbers print `Unprintable error`. `ON ERROR GOTO` traps all but the warnings of §7;
`RESUME` outside a handler is error 20 and is not trapped when ON ERROR is active (b13: it stopped the
program — G: the handler's own error is fatal).

| No. | Message | No. | Message | No. | Message |
|---|---|---|---|---|---|
| 1 | NEXT without FOR | 17 | Can't continue | 55 | File already open |
| 2 | Syntax error | 18 | Undefined user function | 57 | Device I/O Error |
| 3 | RETURN without GOSUB | 19 | No RESUME | 58 | File already exists |
| 4 | Out of DATA | 20 | RESUME without error | 61 | Disk full |
| 5 | Illegal function call | 22 | Missing operand | 62 | Input past end |
| 6 | Overflow | 23 | Line buffer overflow | 63 | Bad record number |
| 7 | Out of memory | 24 | Device Timeout | 64 | Bad file name |
| 8 | Undefined line number | 25 | Device Fault | 66 | Direct statement in file |
| 9 | Subscript out of range | 26 | FOR Without NEXT | 67 | Too many files |
| 10 | Duplicate Definition | 27 | Out of Paper | 68 | Device Unavailable |
| 11 | Division by zero | 29 | WHILE without WEND | 69 | Communication buffer overflow |
| 12 | Illegal direct | 30 | WEND without WHILE | 70 | Disk Write Protected |
| 13 | Type mismatch | 50 | FIELD overflow | 71 | Disk not Ready |
| 14 | Out of string space | 51 | Internal error | 72 | Disk Media Error |
| 15 | String too long | 52 | Bad file number | 73 | Advanced Feature |
| 16 | String formula too complex | 53 | File not found | 74 | Rename across disks |
| | | 54 | Bad file mode | 75 | Path/File access error |
| | | | | 76 | Path not found |

21, 28, 31-49, 56, 59, 60, 65, 77-255: `Unprintable error` (O for 21, 28, 56, 59, 60, 65, 255).
Every message in the table is observed in a case except 1-14 in direct form (seen as numbers in b13 and as
texts 2, 8, 11, 13 in b09) — the texts are the ROM's (B).

## 13. What is out of scope for the first milestones

Graphics statements (PSET, LINE, SCREEN 1/2, POINT, COLOR in graphics modes), SOUND/BEEP/MOTOR, COM: and LPT:
I/O, light pen and joystick, CALL/USR/BLOAD/BSAVE of machine code, SHELL. They are in the plan's last
milestone; none is covered by a committed case yet.

## Ledger

### Deliberate differences

| ID | Difference | Why |
|---|---|---|
| D1 | The rewrite is a multi-segment `.EXE` file named BASIC.COM (DOS runs it by its `MZ` signature), not a 16,256-byte `.COM` | its code does not fit one 64 KiB segment (implementation plan, toolchain findings) |
| D2 | Interpreter state (FAC, pointers, stacks) lives in the rewrite's own data segment; the BASIC data segment holds what programs can observe at the original's offsets (TXTTAB, VARTAB, program text, variables, strings, file buffers) | the original's 0000h-0CB5h work area is ROM/BASIC.COM internal; mirroring all of it is not required by any case. PEEKs into that area other than the documented words differ |
| D3 | No ROM calls; the ROM BASIC is not needed | the title's goal |

### Assumptions

| ID | Assumption |
|---|---|
| A1 | `CLEAR ,n` sets the top of BASIC memory to n and keeps the 512-byte stack below it as at start (b28 gives one data point) |
| A2 | The warning format for Overflow / Division by zero is message, CR LF, then one more CR LF (b03, b06, b09, b13 agree with it) |
| A3 | TXTTAB depends on /F and /S as GW-BASIC's GWINIT (file data blocks below the text); only the default (0CB6h) is checked |

### Observations needing care

| ID | Observation |
|---|---|
| O-K1 | The transcripts render control characters in the key line as blanks (MartyPC oracle's text screen); the rewrite writes the same characters to video memory, so both renderings agree |

### Unknowns

| ID | Unknown | Plan |
|---|---|---|
| U1 | REA was not run in phase 1 (no provider installed); the disassembly is objdump's | run REA before milestone M3 and commit its summary under `spec/` |
| U2 | The decoder scripts for the reserved-word tables were run ad hoc | commit them as `scripts/rom_tables.py` with milestone M1 (reads the pinned binaries, prints tables, never commits bytes beyond tables of words) |
| U3 | Which statements give `Advanced Feature` in D2.00 | read BASIC.COM's FE dispatch; add a case |
| U4 | The RND generator's exact algorithm and seed table in C1.10 | port GW `$RND`, check against b10 |
| U5 | Ctrl-Break handling (INT 1Bh) | not testable with the harness; implement as GW |
