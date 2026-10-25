# SCELBAL — behavior spec

SCELBAL ("SCientific ELementary BAsic Language"), Mark G. Arnold and Nat
Wadsworth, for the SCELBI 8H/8B and other 8008 machines. The listing's
header dates the program to 1974 and carries "Copyright 1975 Scelbi Computer
Consulting, Inc."; it was published in book form as *SCELBAL: A Higher-Level
Language for 8008/8080 Systems*. Exact release dates are not settled by our
sources.

## Sources

- The program: `original/sc1.lst` and `original/sc1.asm` (`original/fetch.sh`,
  SHA-256 in `original/*.sha256`), Thomas E. Jones's AS8 assembly of Steve
  Loboyko's 2001 transcription, with the three published patches, as
  corrected by Mark Arnold in 2012. Labels and octal page-offset addresses in
  this spec and in the Rust comments are the listing's (`EXEC` at 010 266,
  `FPADD` at 020 211, ...).
- The oracle is that program: `scripts/build_original.py` takes the bytes
  of the listing and overlays the Micral N teletype (`oracle/micral_io.asm`);
  `scripts/capture_oracle.py` runs it on the ai_hw_engineer RTL and records
  the transcripts in `oracle/transcripts/`.

## Behavior

The interpreter is a line editor and an immediate-mode BASIC. After `READY`
(DC4, CR LF, `READY`, CR LF LF) it reads a line, echoing each key, with
RUBOUT (prints `\`, drops the last character), control-C (`^C` error) and LF
ignored; CR ends the line and prints CR LF. Lines are at most 79 characters
(80 is a `BG` error).

- **Commands** (the whole line): `LIST`, `RUN`, `SCR` (clear program,
  variables and arrays), `SAVE` and `LOAD` (user hooks; this build has none,
  so both just print READY).
- **Program lines**: a line number (digits, compared as strings: shorter is
  smaller) stores the line in order, replaces a line with the same number,
  or with nothing after it deletes that line. Lines are stored as typed.
- **Statements** (keywords are recognised by accumulating the non-space
  characters after the line number until they spell one):
  `REM`, `LET` (also implied, `X=...`, and `A(I)=...`), `PRINT`, `INPUT`,
  `IF ... THEN line|statement` (`GOTO` may stand for `THEN`), `GOTO`,
  `GOSUB`/`RETURN` (8 levels), `FOR ... TO ... [STEP ...]`/`NEXT var`,
  `DIM`, `END`. A statement without a line number runs at once.
- **Expressions**: numbers (`1`, `.5`, `1.5E-3`), variables (one letter or
  letter and character; 19 at most), array elements `A(i)`, operators
  `^` (repeated multiplication by the integer part), `* /`, `+ -`, the
  relations `< = > <= >= <>` (also `=< => ><`), parentheses, unary minus
  (not after `*` or `^`). True is 1.0, false 0.
- **Functions**: `INT`, `SGN`, `ABS`, `SQR`, `RND` (argument ignored),
  `TAB(n)` and `CHR(n)` (print, no value printed), `UDF` (halts: the user's
  machine code hook).
- **PRINT**: fields separated by `,` (next multiple of 16 columns) or `;`;
  text in `"` or `'`; a trailing `,` or `;` suppresses the CR LF. A number
  prints with a sign column (space or `-`), in fixed point when its binary
  exponent allows (`1.0`, `123456.0`, `1234567.`) and otherwise as
  `0.ddddddd` with `E±nn`; seven digits.
- **INPUT**: `?` and a number per variable; a name ending in `$` reads one
  key and stores its code (with bit 7 set: `A` is 193).
- **Numbers**: SCELBAL floating point, four bytes: a 24-bit two's complement
  mantissa and an exponent byte (`src/fp.rs`). The arithmetic, the decimal
  conversions and their rounding are the listing's, bit for bit, quirks
  included (`8388607` prints `838860.5`: the input routine drops digits it
  has no room for).
- **Errors** print two letters, ` AT LINE n` when a program line was
  running, CR LF, then READY: `BG` too big (line, table, program), `SY`
  syntax, `I(` parentheses, `IQ` quotes, `IN` number, `DZ` divide by zero,
  `FX` too large to fix, `SQ` square root of a negative, `UN` undefined
  line, `IF`, `LE`, `GS`, `RT`, `FE`, `FN` (FOR/NEXT), `FA` function or
  array, `OR` subscript, `AF` array format, `DE` dimension, `^C`.

Limits that come from the layout, as in the original: 19 variables, 8 GOSUB
levels, 15 operators pending in an expression, 7 parentheses, FOR loops not
popped by a jump out of the loop (a later `NEXT` of the outer loop then
gives `FN`), arrays of one-letter names sharing 64 values on page 57.

## Memory

The Rust keeps the original's data at its original addresses, mapped
(`src/mem.rs`): page 01 (floating point registers and constants, command
words, READY), page 26 (line buffer, symbol buffers, parser stacks,
pointers), page 27 (keywords, GOSUB and FOR stacks, variables), the program
pages from 033 and the array page 057. Pointers stay page and offset bytes,
so overflows and scans behave as the original's. On the Micral N the three
fixed pages and the program pages are a 256-aligned static in the image; the
array page is 0x3700, the boot ROM's buffer while it loads.

## Target

- Errors: the original jumps to `EXEC` from wherever an error is found. The
  Rust does the same (`fail`): the target jumps to `basic_restart`
  (`asm/start.s`, a linker `RESET` point), abandoning the calls in
  progress, which the 8008's circular stack allows; the host unwinds with a
  panic that the caller catches.
- Call depth: the statements are inlined into the executive and the parser,
  operators and functions into `eval`, so the deepest chain fits 7 levels.
- The five multi-byte carry loops (`ROTATL`, `ROTATR`, `COMPLM`, `ADDER`,
  `SUBBER`) are assembly on the target (`asm/fp.s`): compiled Rust has no
  access to the carry flag and spent 80-160 bytes on each. `src/fp.rs`
  holds the Rust versions, used on the host, which define the contracts.

## Observable I/O

See [io.md](io.md).

## Assumption ledger

| ID | Assumption | Why the docs don't settle it | How it is validated |
|---|---|---|---|
| BAS-1 | Keys arrive 7-bit and get bit 7 set (mark parity, as SCELBAL compares them); each key is echoed by the input routine, as the SCELBI bit-banged `CINP` echoes its bits; output has bit 7 cleared | the listing's I/O routines drive a SCELBI serial port, not the Micral N teletype | the oracle runs the listing with exactly these routines (`oracle/micral_io.asm`); the transcripts show the echo |
| BAS-2 | A session starts with `SCR` | the listing leaves the end-of-program pointer zero until the first `SCR`; entering a line before it moves memory from address 0 upwards over the program (the original hangs). The array page is also undefined until `SCR` | every transcript starts with `SCR` |
| BAS-3 | Addresses outside SCELBAL's data pages read 0 and ignore writes | the original would read its own code bytes there; only scans that run off a table in error paths (`FA` with no arrays) reach them | no transcript depends on it; the `FA` transcript does not |
| BAS-4 | The program buffer is 10 pages (2560 bytes), not the listing's 18: a longer program is a `BG` error sooner | the listing's 12 KiB layout with its 4.5 KiB buffer does not fit the Micral N with the Rust code (see the report) | the transcripts' programs fit; the limit is the listing's own `INSERT` check |
