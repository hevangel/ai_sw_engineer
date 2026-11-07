# MORE test plan

The oracle is the original `MORE.COM` of PC DOS 2.00 run by PC DOS 2.00 on MartyPC's IBM 5160 with the original XT BIOS (`../scripts/capture_dos.sh` → `oracle/transcripts/`). Each case (`scripts/cases.py`) is a set of files added to drive A (exact bytes), a command line, and pairs of (text to wait for, keys to type). A wait text starting with `$` means: the last non-blank screen row ends with the rest (`-- More --`); in the Rust emulator's session language that is `until endswith -- More --`, and the session also requires the screen to have changed since the keys of the step were typed. The keys are a space, `x` or Enter.

## What is compared

The transcripts are the 25 screen rows at the end (rows start with `|`, header lines with `#`). The rule, the same in the host replay (`tests/oracle_replay.rs`), the Rust emulator test (`tests/rust_emulator.rs`) and `scripts/compare.py` (as in COMP):

- When the oracle's screen shows the command line (`A>` and the command of its `# oracle:` header: m01, m05, m06), the rows from the one after it to the next `A>` row, empty rows included, must be equal.
- When it does not (the other seven: the output scrolls it off the 25 rows), the oracle's rows from the top through its last `A>` row must equal as many rows of the run ending at the run's last `A>` row. The host replay draws the command line, MORE's output and the echo of the keys, COMMAND's CR LF and `A>` on a 25-row screen and takes the same rows.

| Case | Input / command / keys | What it checks (spec section) |
|---|---|---|
| m01 | 5 lines; `more < short.txt` | the CR LF at start, lines shown as they are, ^Z at the end (Start, The loop) |
| m02 | 60 lines; `more < long.txt`; space, space | the prompt after 24 rows, the empty row after it, 23 lines on later screens (What that shows) |
| m03 | the same through `type long.txt \| more`; `x`, Enter | a pipe as input; the key still read from the console (the handle swap); other keys |
| m04 | 30 lines of 100 characters; space, space | a line counted as two rows; the prompt in the middle of a line and its character shown again |
| m05 | TAB, BS and BEL | tab stops (DOS expands them), BS moves back, BEL not shown and not counted |
| m06 | text after a ^Z | ^Z ends MORE at once |
| m07 | 23 lines, no ^Z | no prompt; the end of the input |
| m08 | 24 lines | the prompt before the end is seen; two empty rows before `A>` |
| m09 | 25 lines | `-- More --`, an empty row, `Line 25` |
| m10 | 30 lines of exactly 80 characters | an 80-character line counts as two rows (CURCOL > MAXCOL after the increment) |

| Level | Runs | Expected | Command |
|---|---|---|---|
| Host | the Rust core with a model of DOS (handles 0-2, 45h/3Eh, 4,096-byte reads, the console: CGA 80x25 wrap and scroll, TAB to the next multiple of 8 by DOS's column, BS back, BEL not shown); each key is given only when MORE waits for one, after checking the case's wait text on the drawn screen | the oracle rows | `cargo test` |
| Original × Rust emulator | the original under PC DOS 2.00 on `system/ibm_pc/emulator` (5150 board, 64 KiB) | the oracle rows | `cargo test --release --test rust_emulator -- --ignored` |
| Rewrite × Rust emulator | `build/more.com` in place of MORE.COM there | the oracle rows | same |
| Rewrite × MartyPC 5160 | `build/more.com` in place of MORE.COM on the same disk | the oracle rows | `capture_dos.sh` with `PROGRAM=` and `NAME=MORE.COM`, then `compare.py` |
| Original × MartyPC 5160 | the oracle itself | — | `capture_dos.sh` (never without `PROGRAM=` for a rewrite: that would overwrite the oracle) |

Host-only tests (`tests/oracle_replay.rs`) for rules no screen case shows: the version check (DOS 1.x gets the message and nothing is read), a 40-column mode (MAXCOL from INT 10h), a line spanning two 4,096-byte reads.

Not covered on screen: DOS 1.x, a 40-column mode, read errors, output redirected (the key's echo then goes to the file), ^C at the prompt.
