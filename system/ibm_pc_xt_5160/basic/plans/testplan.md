# BASIC test plan

The oracle is the original pair — BASIC.COM D2.00 of PC DOS 2.00 on Cassette BASIC C1.10 in the XT's ROM — run
by PC DOS 2.00 on MartyPC's IBM 5160 (256 KiB, XT BIOS 1982-11-08) with
`../../scripts/capture_dos.sh` → `oracle/transcripts/` (31 cases, [../oracle/README.md](../oracle/README.md)).
Each case (`scripts/cases.py`; its output is split on `\n` only) is a command typed at `A>`, the files added to
drive A (PC DOS 2.00 disk 1, with the program under test as BASIC.COM: for a rewrite run `pcdisk add` replaces
the original there) and `b:` steps for a blank 360 KiB drive B, and (wait text, keys) pairs. A wait text
starting with `$` means the last non-blank row ends with the rest, once the screen has changed; `^X` in the
keys is Ctrl+X.

## What is compared

The screen, empty rows included, right-trimmed (`tests/common/mod.rs` `compared`, `scripts/compare.py`):

- if the oracle shows the case's command line (`A>` + the command), the rows after it to the next row that is
  only `A>` (BASIC clears the screen when it starts, so this applies only to cases whose output is followed by
  another command, none so far);
- otherwise, if the oracle ends at DOS (a row that is only `A>`), the oracle's rows from the top through its last
  `A>` against as many rows of the run ending with its last `A>`;
- otherwise (b29, b30 end inside BASIC) the whole screen.

## Levels

| Level | Runs | Expected | Command |
|---|---|---|---|
| Host | the Rust core (`software::run`) on a model of the machine: video memory, the case's keys in order, the 64 KiB data segment, and after SYSTEM what COMMAND prints (CR LF, `A>`, BIOS teletype) | the oracle rows | `cargo test` (`tests/oracle_replay.rs`) |
| Original × Rust emulator | the original under PC DOS 2.00 on `system/ibm_pc/emulator` (5150 board, 5150 BIOS, the XT's C1.10 ROM at F6000h, 256 KiB, CGA) | the oracle rows | `IBM_PC_ROMS=… XT_ROMS=… cargo test --release --test rust_emulator -- --ignored` |
| Rewrite × Rust emulator | `build/basic.com` as BASIC.COM there (same machine; it does not use the ROM) | the oracle rows | same |
| Rewrite × MartyPC 5160 | `build/basic.com` as BASIC.COM on the same disks | the oracle rows | `PROGRAM=build/basic.com NAME=BASIC.COM OUT=<dir> capture_dos.sh <basic> <cases>`, then `scripts/compare.py oracle/transcripts <dir>` |
| Original × MartyPC 5160 | the oracle itself | — | `capture_dos.sh` (never run without `PROGRAM` except to capture a new case) |

The host level models only the cases that start `basic` with no file and no drive B so far (no DOS file
calls in the model yet; M2/M3 add them). Each milestone's cases (`tests/common/mod.rs` `PASSING`) must be
identical in every level that runs them; the tests print the others and do not fail on them.

| Milestone | Cases |
|---|---|
| M1 (done 2026-11-10) | b14-b18, b29, b30, b31 (and b21, which passes ahead of M4) |
| M2 | + b01, b02, b03, b09, b10, b11, b12, b20, b27, b28 |
| M3 | + b04-b08, b13, b19, b25, b26 |
| M4 | + b22, b23, b24 |
