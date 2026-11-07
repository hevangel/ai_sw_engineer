# FIND test plan

The oracle is the original `FIND.EXE` of PC DOS 2.00 run by PC DOS 2.00 on MartyPC's IBM 5160 with the original
XT BIOS (`../scripts/capture_dos.sh` → `oracle/transcripts/`). Each case (`scripts/cases.py`) is a set of input
files (exact bytes) and a command line; what is compared is every screen row from the command line to the next
`A>` prompt, empty rows included.

| Case | Input / command | What it checks (spec section) |
|---|---|---|
| f01 | fruit, `find "an" fruit.txt` | heading, matching lines with their line ends (Files, Matching) |
| f02, f05, f06 | `/v`, `/c /v`, `/N /V` | inversion; empty line 7 never matches; the ^Z "line" 10 (What DOS adds); upper-case options |
| f03, f18 | `/c` | the count on the heading line; long lines (200, 300 bytes) |
| f04, f17 | `/n` | `[n]` line numbers; a last line without newline gets CR LF (Reading) |
| f07, f22 | `"Apple"`, `" "` | case-sensitive; a blank string |
| f08, f09 | two files; a missing file | per-file headings; `File not found <name>` and the scan goes on |
| f10, f11, f20 | no string, unquoted string, no arguments | `Syntax error`, `Invalid number of parameters` (Command line) |
| f12, f13 | `""""`, `""` | a doubled quote; an empty string ends silently |
| f14, f21 | `<fruit.txt` (with `/c`) | standard input: no heading, no `: ` before the count |
| f15 | `type fruit.txt \| find /n "e"` | a DOS 2 pipe as standard input |
| f16 | `/x` | `Invalid Parameter X`, then the search runs |
| f19 | a line after ^Z | the console stops a write at ^Z |

Host-only tests (`tests/oracle_replay.rs`) for rules no screen case shows: exit codes (2, 1, 0); `/c/n` and `/cn`;
capitalization of bytes 80h-FFh; a lone LF always matches, `x LF` searches nothing; line numbers running ahead
after a 4,096-byte refill.

| Level | Runs | Expected | Command |
|---|---|---|---|
| Host | the Rust core with a DOS model over the case files, output drawn as the console draws it (writes stop at ^Z) | the oracle rows | `cargo test` |
| Original × Rust emulator | the original under PC DOS 2.00 on `system/ibm_pc/emulator` (5150 board, 64 KiB) | the oracle rows | `cargo test --release --test rust_emulator -- --ignored` |
| Rewrite × Rust emulator | `build/find.exe` there as FIND.EXE | the oracle rows | same |
| Rewrite × MartyPC 5160 | `build/find.exe` in place of FIND.EXE on the same disk | the oracle rows | `capture_dos.sh` with `PROGRAM=` and `NAME=FIND.EXE`, then `compare.py` |
| Original × MartyPC 5160 | the oracle itself | — | `capture_dos.sh` |

Not covered on screen: files over 4,096 bytes (host test only), a read error, DOS before 2.00, the exit code (not
visible on the screen; ERRORLEVEL in a batch file could show it), a line over 4,096 bytes (the original hangs).
