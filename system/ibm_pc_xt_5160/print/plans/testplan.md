# PRINT test plan

The oracle is the original `PRINT.COM` of PC DOS 2.00 run by PC DOS 2.00 on 86Box's IBM PC/XT (the 1982-11-08 XT
BIOS, 256 KiB, CGA) with the Generic Text Printer on LPT1 (`../scripts/capture_dos.sh` with `ORACLE=86box`, run
by `../scripts/xt_86box.py` → `oracle/transcripts/`). MartyPC is not used: it keeps its parallel port private.
Each case (`scripts/cases.py`) boots with PC DOS 2.00 disk 1 plus the case's files in drive A, types a PRINT
command at `A>`, answers the list-device question (`Name of list device`) with Enter or `lpt1`, then lets the
machine settle until the printing has finished. Every level builds the disk the same way (the tests with the
pcdisk library, `tests/common/mod.rs`; 86Box with the pcdisk tool). The output of `cases.py` is split on `\n`
only.

## What is compared

- The screen, empty rows included, right-trimmed: the rows from the case's first command line (`A>` + the
  command) through the screen's last row that is only `A>`, in the oracle and in the run (`compared` in
  `tests/common/mod.rs`, `scripts/compare.py`).
- The printed lines, all of them, exactly: the oracle's `#lpt|` lines are the page files 86Box's text printer
  wrote, split on `\n` (a form feed shown as `<FF>`). On 86Box both sides are such files (`compare.py`). On the
  host and the Rust emulator the bytes sent to the printer are rendered the way 86Box's `prt_text.c` lays out a
  page (`render_text_printer` in `tests/common/mod.rs`, spec H4; its own test in `tests/oracle_replay.rs`):
  80 x 66 pages, CR without auto feed, tabs to multiples of 8, a page written at FF, at line 66 or at the
  time-out, without its current (unterminated) line.
- No case looks at the queue while a file is printing: how far the background printing has got at a given
  moment depends on each emulator's timing. Each case's status is shown right after the files were queued,
  before the first byte is printed.

| Case | Command (keys at the device question) | What it checks (spec section) |
|---|---|---|
| p01 | `print fruit.txt` (Enter) | the question, the install, the status; the file's bytes to ^Z and a form feed (the slice) |
| p02 | `print fruit.txt tabs.txt` | two files, `is in queue`; tabs expanded to spaces (the column); a form feed after each file; the last line without CR LF is not on 86Box's page |
| p03 | `print /d:prn fruit.txt` | PRINT 2.00 has no `/D`: `/d` is one switch character, `Invalid parameter`; `:prn` is a name (2901h skips the `:`), so the device PRN is queued (its read gives no data: only a form feed) |
| p04 | `print long.txt` | 70 lines, four records of 512 bytes; 86Box's page of 66 lines |
| p05 | `print ff.txt` | a form feed inside the file is passed on |
| p06 | `print *.txt` | a pattern: every file found (11h/12h), in directory order |
| p07 | `print nofile.txt` | `File not found` with the drive and name; nothing queued |
| p08 | `print` | the install alone; `PRINT queue is empty` |
| p09 | `print /x fruit.txt` | an unknown switch: `Invalid parameter`, the file still queued |
| p10 | `print fruit.txt` (`lpt1`) | a typed device name (lower case: opened as LPT1, the LPT table matched after it is made capital) |

`tests/oracle_replay.rs` also checks, from the spec, what no case shows: a second run using the resident copy (no
question, the queue listed), `/T` (the printer's `All files canceled by operator`), `/C` on the files before and
after it (a file not at the head moved behind the last entry, the head's `Canceled by operator`), the queue of 10
and `PRINT queue is full` once, the column (LF does not reset it, BS) and a file without ^Z (the record's zeros,
then the form feed), a typed name that is not a device, the LPT and COM tables and the INT 17h/14h/5 answers, a
read error reported on the printer, the version check; and the printer renderer on byte strings.

| Level | Runs | Expected | Command |
|---|---|---|---|
| Host | the Rust core (`run`) and the resident model (`resident`) in front of a model of DOS 2.00 and COMMAND over the pcdisk image (spec H1-H4), console output drawn as the console draws it | the oracle's rows and printed lines | `cargo test` (drive A is PC DOS 2.00 disk 1 with `XT_ROMS`, else a blank disk) |
| Original × Rust emulator | the original under PC DOS 2.00 on `system/ibm_pc/emulator` (5150 board, `Machine::new(256)`, `attach_printer`), run idle after the last prompt until the printer has taken nothing for 15 s of machine time | the oracle's rows and printed lines | `cargo test --release --test rust_emulator -- --ignored` |
| Rewrite × Rust emulator | `build/print.com` as PRINT.COM there; also its layout against the host's image (`the_build_has_the_layout_the_host_replay_models`) | the same | same |
| Rewrite × 86Box | `build/print.com` as PRINT.COM on the same disk | the same | `capture_dos.sh` with `ORACLE=86box PROGRAM=`, then `compare.py` |
| Original × 86Box | the oracle itself | — | `capture_dos.sh` with `ORACLE=86box` |

Not covered by a case: the cancel switches and a second run (host only), a device that is busy or off line
(`Errors on list device`, the 20000-call drop), INT 24h during a read, COM devices, the INT 13h/17h/14h/5 hooks
against another program, INT 28h against the timer (which slice prints which byte), the rewrite and the original
mixed (spec D2).
