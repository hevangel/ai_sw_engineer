# SCELBAL test plan

| Level | What | Oracle | Command | Image |
|---|---|---|---|---|
| Host replay | all transcripts through the Rust interpreter and a mock teletype | the original on the RTL | `cargo test --release` | ai-sw-engineer |
| Target, RAM | `build/basic.bin` at 0x0000 on the RTL | same transcripts | `scripts/run_sim.sh ram` | ai-hw-engineer |
| Target, boot | `build/disk.img` loaded by the Rust boot ROM | same transcripts | `scripts/run_sim.sh boot` | ai-hw-engineer |
| Oracle check | the original replays its own transcripts | — | `run_tty.py --ram build/original.bin oracle/transcripts/*.txt` | ai-hw-engineer |
| Cross-check | a second emulator | same transcripts | not done: no 8008 emulator in `external/` runs the Micral N teletype yet | — |

Transcripts (`scripts/capture_oracle.py`):

| Transcript | Covers |
|---|---|
| 01_print_numbers | number formats (fixed, float, exponents, rounding), arithmetic, precedence, relations, PRINT separators |
| 02_program_edit | entering out of order, replace, delete, LIST, RUN, END, SAVE/LOAD |
| 03_functions | INT SGN ABS SQR RND TAB CHR, TAB back-spacing |
| 04_loops_subroutines | nested FOR with STEP, GOSUB/RETURN, IF THEN line/statement, direct GOSUB and IF GOTO |
| 05_input | INPUT numbers, `$` keys, input running into later lines |
| 06_errors | DZ SQ I( IQ SY UN RT FN FE IF LE FA IN DE FX BG, rubout, control-C |
| 07_arrays | DIM, array LET, INPUT into an element, OR |
| 08_primes | jumping out of a FOR loop (FN, as the original) |
| 09_program | a longer program: tables, sums, factorial, odd primes, LIST |
| 10_limits | two-letter names, the variable table limit (BG), a 79+ character line, delete and LIST |

Coverage gaps: UDF (halts), the program buffer limit (BAS-4 changes it), a
GOSUB overflow (GS), unary minus after `/`.
