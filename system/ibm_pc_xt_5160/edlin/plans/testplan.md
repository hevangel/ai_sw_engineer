# EDLIN test plan

The oracle is the original `EDLIN.COM` of PC DOS 2.00 run by PC DOS 2.00 on MartyPC's IBM 5160 (256 KiB) with the
original XT BIOS (`../scripts/capture_dos.sh` → `oracle/transcripts/`). Each case (`scripts/cases.py`; its output is
split on `\n` only, since a key sequence ends with `\r`) is a command typed at `A>`, the files added to drive A (PC
DOS 2.00 disk 1, with the program under test as EDLIN.COM: for a rewrite run `pcdisk add` replaces the original
there), and (wait text, keys) pairs for what is typed after it. A wait text starting with `$` means the last
non-blank row ends with the rest, the screen having changed since the previous wait (`until endswith`): `$*` for
EDLIN's command and insert prompts, `$?` for `O.K.?` and `Abort edit (Y/N)?`, `$A>` for COMMAND's prompt. `^Z` and
`^C` in the keys are Ctrl+Z and Ctrl+C. Every editing case ends with E or Q and then TYPEs the file(s) at the next
`A>`, so the screen shows what EDLIN wrote. Every level builds the disks the same way (`tests/common/mod.rs`
`disk_a`, the same images as capture_dos.sh).

## What is compared

**The screen**, empty rows included, right-trimmed:

- if the oracle's screen shows the case's command line (`A>` + the command), the rows from the one after it to the
  next row that is only `A>` (rows like `A>type fruit.txt` are inside), in the oracle and in the run (from the
  run's last such command line): every case but e06 and e08;
- otherwise (the output scrolled it off: e06, e08) the oracle's rows from the top through its final `A>` against
  the same number of rows of the run ending with its final `A>`.

The rule is in `tests/common/mod.rs` (`compared`) and `scripts/compare.py`. On the Rust emulator the files EDLIN
leaves on drive A are compared too: the Rust build's must be the original's, byte for byte (every file but
EDLIN.COM itself). MartyPC does not write its disks back, so there the files are seen through TYPE only.

| Case | Files / command / keys | What it checks (spec section) |
|---|---|---|
| e01 | `edlin new.txt`: `i`, two lines, `^Z`, `l`, `e`; TYPE | `New file`, INSERT to ^Z (`^Z` echoed), E on a new file (Start, INSERT, ENDED) |
| e02 | FRUIT.TXT: `2d`, `l`, `e`; TYPE the file and FRUIT.BAK | `End of input file`, DELETE, CURRENT after it, the .BAK (DELETE, ENDED) |
| e03 | `1,5san`, `3,5?serr` and `y`, `q` and `y` | S without and with `?` (`O.K.?`, function 01h), QUIT (SEARCH, QUERY) |
| e04 | `1,5rerr^Zxx`, `l`, `e` | R with ^Z between the strings, the lines shown, CURRENT = the line before the last change (REPLACE) |
| e05 | `3i`, a line, `^C`, `l`, `e` | Ctrl-C in INSERT: DOS's `^C` CR LF, ABORTINS's CR LF and END_INS (Ctrl-C) |
| e06 | `1,2,5m`, `l`, `1,1,1,2c`, `l`, `e` | BLKMOVE: a move past the block, a copy with a count (scrolls) |
| e07 | `1,3d`, `q` and `y`; TYPE | Q leaves the file as it was (QUIT) |
| e08 | LONG.TXT (40 lines): `1p`, `p`, `q` and `y` | PAGE: 23 lines, then to the end, CURRENT the last shown (scrolls) |
| e09 | FRUIT.TXT, OTHER.TXT: `3tother.txt`, `l`, `e`; TYPE | MERGE: the last byte dropped and no ^Z at the end, so E writes memory's byte (`[`) (MERGE) |
| e10 | `2`, `BANANA`, `.,#l`, `e` | NOCOM: a line edited with the template; `.` and `#` (Editing, GETNUM) |
| e11 | LONG.TXT: `20`, an empty answer, `-2,+2l`, `#l`, `35,l`, `q` and `y` | an empty edit keeps the line; `-n`, `+n`, `#`, an empty second number (LIST) |
| e12 | FRUIT.TXT: `zz`, `9d`, `l`, `q` and `y` | `Entry error`; DELETE past the end does nothing |
| e13 | `edlin` | `File name must be specified` |
| e14 | KEEP.BAK: `edlin keep.bak` | `Cannot edit .BAK file--rename file` |

`tests/oracle_replay.rs` also checks, from the spec, what no case shows: an empty file becomes one empty line and E
writes CR LF ^Z; M without a destination is `Entry error`; T of a file without ^Z drops its LF and E writes the
byte memory holds at ENDTXT; `/B` keeps the text after a ^Z (shown as `^Z`) and without it the text stops there;
^V and a letter insert a control character; Ctrl-C at the command prompt (ABORTCOM); with 200 bytes of text memory
(PSP word 6 = START + 201) the start reads three quarters, prints no `End of input file`, and E still writes the
whole 40-line file (WRT and FILLBUF alternating).

| Level | Runs | Expected | Command |
|---|---|---|---|
| Host | the Rust core with a model of the oracle machine's DOS session: the PSP's 64 KiB as EDLIN finds it (the PSP COMMAND builds, PSP word 6 = FFF0h, COMMAND.COM's bytes from the boot above it: with `XT_ROMS`, else zeros and e09 not compared), the FCB calls on a map of files, the console echo of functions 0Ah and 01h, Ctrl-C, TYPE | the oracle rows | `cargo test` |
| Original × Rust emulator | the original under PC DOS 2.00 on `system/ibm_pc/emulator` (5150 board, `Machine::new(256)`) | the oracle rows | `cargo test --release --test rust_emulator -- --ignored` |
| Rewrite × Rust emulator | `build/edlin.com` as EDLIN.COM there | the oracle rows; the same screen and the same files as the original there | same |
| Rewrite × MartyPC 5160 | `build/edlin.com` as EDLIN.COM on the same disks | the oracle rows | `capture_dos.sh` with `PROGRAM=`, then `compare.py` |
| Original × MartyPC 5160 | the oracle itself | — | `capture_dos.sh` (never run without `PROGRAM` except to re-capture the oracle) |

Not covered by a case: W and A as commands (E's own writes and reads with little memory are in the host test), files larger than memory, a name
with a directory, `Disk full`, critical errors, `/B` and ^V on an emulator, the function keys of the templates,
DOS 1.x.
