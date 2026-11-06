# SORT test plan

The oracle is the original `SORT.EXE` of PC DOS 2.00 run by PC DOS 2.00 on MartyPC's IBM 5160 with
the original XT BIOS (`scripts/capture_oracle.sh` → `oracle/transcripts/`). Each case
(`scripts/cases.py`) is an input file (exact bytes) and a command line; what is compared is every
screen row from the command line to the next `A>` prompt, empty rows included.

| Case | Input / command | What it checks (spec section) |
|---|---|---|
| c01 | fruit, `sort <fruit.txt` | byte order: space, digits, capitals, lower case; an empty line first; no CR LF after the last line (Output) |
| c02 | fruit, `/r` | reverse order (Sort) |
| c03, c05, c06 | columns, `/+3`, `/R/+3`, `/+0` | the key from column n; `/+0` = whole line (Command line) |
| c04 | `+5` without a switch character | the first parse state accepts `+` |
| c07, c08 | equal keys, `/+3` and `/r /+3` | ties keep input order; reversed, they come out last first |
| c09 | lines shorter than the column, `/+5` | short lines compare as spaces |
| c10, c11, c12, c13 | ends in CR LF / no newline / ^Z only / empty | the end-of-input trim and the extra empty record (Input, Records) |
| c14 | bytes 7Fh, 82h | unsigned comparison above 7Fh |
| c15 | `type fruit.txt \| sort /r` | DOS 2 pipe as standard input |
| c16 | `/+70000` | column arithmetic modulo 65536 |

| Level | Runs | Expected | Command |
|---|---|---|---|
| Host | the Rust core with a DOS model over the case inputs, output drawn as the console draws it | the oracle rows | `cargo test` |
| Original × Rust emulator | the original under PC DOS 2.00 on `system/ibm_pc/emulator` (5150 board, 64 KiB) | the oracle rows | `cargo test --release --test rust_emulator -- --ignored` |
| Rewrite × Rust emulator | `build/sort.exe` there | the oracle rows | same |
| Rewrite × MartyPC 5160 | `build/sort.exe` in place of SORT.EXE on the same disk | the oracle rows | `capture_oracle.sh` with `PROGRAM=`, then `compare.py` |
| Original × MartyPC 5160 | the oracle itself | — | `capture_oracle.sh` |

Not covered: input over the buffer (D1: different limits by design), a write error (needs a full
disk), DOS before 2.00 (needs DOS 1.x, where `.EXE` handling differs), the exit code (not visible
on the screen; ERRORLEVEL in a batch file could show it).
