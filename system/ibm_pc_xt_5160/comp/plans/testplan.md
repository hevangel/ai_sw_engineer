# COMP test plan

The oracle is the original `COMP.COM` of PC DOS 2.00 run by PC DOS 2.00 on MartyPC's IBM 5160 with the original XT BIOS (`../scripts/capture_dos.sh` → `oracle/transcripts/`). Each case (`scripts/cases.py`) is a set of files added to drive A (exact bytes), a command line, and pairs of (text to wait for, keys to type): every case answers `n` to `Compare more files (Y/N)?`, and p07 also answers the two name prompts.

## What is compared

The transcripts are the 25 screen rows at the end (rows start with `|`, header lines with `#`). The rule, the same in the host replay (`tests/oracle_replay.rs`), the Rust emulator test (`tests/rust_emulator.rs`) and `scripts/compare.py`:

- When the oracle's screen shows the command line (`A>` and the command), the rows from the one after it to the next `A>` row, empty rows included, must be equal.
- When it does not (p03: the ten differences scroll it off the 25 rows), the oracle's rows from the top through its last `A>` row must equal as many rows of the run ending at the run's last `A>` row. The host replay renders the command line, COMP's output, COMMAND's CR LF and `A>`, and takes the tail of that.

| Case | Files / command | What it checks (spec section) |
|---|---|---|
| p01 | A.TXT, SAME.TXT; `comp a.txt same.txt` | equal files: the name display, `Files compare ok`, the gaps (Each pair, The compare) |
| p02 | A.TXT, THREE.TXT | three differences: hex offsets without leading zeros (`A`, `1E`), the bytes of each file |
| p03 | A.TXT, MANY.TXT | the 10-difference limit; the command line scrolls off (tail rule) |
| p04 | A.TXT, SHORT.TXT | `Files are different sizes`, nothing compared |
| p05 | `comp a.txt nofile.txt` | the second file missing: its name twice, `- File not found` |
| p06 | BIN.DAT, BIN2.DAT (no ^Z) | `Eof mark not found`, then `Files compare ok` |
| p07 | `comp` and the two prompts answered | the prompts, function 0Ah's echo (CR without LF) |
| p08 | `comp *.dat *.bak` (4 files) | search first/next, `?` of the second pattern from the first name, two pairs |
| p09 | `comp a.txt a:` | a drive alone as the second name (3Bh on `A:` fails; the pattern is all `?`) |
| p10 | `comp nofile.txt a.txt` | the first file missing: `- File not found`, straight to Compare more |

| Level | Runs | Expected | Command |
|---|---|---|---|
| Host | the Rust core with a model of DOS 2.00's FCB calls over the case files, output and echo drawn as the console draws them | the oracle rows | `cargo test` |
| Original × Rust emulator | the original under PC DOS 2.00 on `system/ibm_pc/emulator` (5150 board, 64 KiB) | the oracle rows | `cargo test --release --test rust_emulator -- --ignored` |
| Rewrite × Rust emulator | `build/comp.com` in place of COMP.COM there | the oracle rows | same |
| Rewrite × MartyPC 5160 | `build/comp.com` in place of COMP.COM on the same disk | the oracle rows | `capture_dos.sh` with `PROGRAM=` and `NAME=COMP.COM`, then `compare.py` |
| Original × MartyPC 5160 | the oracle itself | — | `capture_dos.sh` (never with `OUT` left at its default for a rewrite) |

Not covered: paths with directories and `- Invalid path` (the case disks have no subdirectories), an invalid drive, `Y` to Compare more, files over 52 KiB (spec D1: the buffer sizes differ by design), an empty file (the ^Z check reads a byte the read did not write), DOS other than 2.00, `Insufficient memory`.
