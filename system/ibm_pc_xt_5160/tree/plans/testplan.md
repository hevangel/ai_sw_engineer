# TREE test plan

The oracle is the original `TREE.COM` of PC DOS 2.00 run by PC DOS 2.00 on MartyPC's IBM 5160 with
the original XT BIOS (`../scripts/capture_dos.sh` → `oracle/transcripts/`). Each case
(`scripts/cases.py`) is a command typed at `A>` and a list of steps that build a blank 360 KiB disk
for drive B with pcdisk (`b:mkdir:PATH`, `b:label:LABEL`, `b:add:PATH=FILE`; `-`: drive B empty).
Every level builds that disk the same way (the host and Rust-emulator tests with the pcdisk
library, `tests/common/mod.rs`; MartyPC with the pcdisk tool).

## What is compared

Empty rows included, right-trimmed:

- if the oracle's screen shows the command line (a row `A>` + text), the rows from the one after it
  to the next `A>`, in the oracle and in the run (the run's last command line);
- otherwise (t02, t03, t04, t05, t08: the output is longer than the screen and scrolled the command
  off) the oracle's rows from the top through its final `A>` against the same number of rows of
  the run ending with its final `A>`.

The same rule is in `tests/common/mod.rs` (`compared`) and `scripts/compare.py`. The host replay
draws the session as `A>` + the command, CR LF, TREE's output, COMMAND's CR LF and `A>`, unscrolled,
and takes its last rows.

| Case | Disk B / command | What it checks (spec section) |
|---|---|---|
| t01 | a file only, `tree b:` | no subdirectory: `No sub-directories exist`, no header (Main) |
| t02 | SUB1, SUB2\INNER\DEEP, `tree b:` | preorder walk, siblings after a subtree, `None`, the block layout (Main, Listing) |
| t03 | t02 + files, `tree b: /f` | the files pass, `NAME    .EXT`, files of the root not listed (Listing) |
| t04 | t02 + label, `tree b:` | a volume label in the root is not a subdirectory (search attribute 10h) |
| t05 | t03 + label, `tree b: /F` | `/F` in capitals (Initialisation) |
| t06 | `tree q:` | the invalid drive check on AX (Initialisation) |
| t07 | `tree b: /x` | `Invalid parameter` |
| t08 | A\X, B, C, `tree b:` | returning up two levels to the next sibling (Paths) |
| t09 | `tree` (drive A, PC DOS 2.00 disk 1) | the default drive |

`tests/oracle_replay.rs` also checks, from the spec, the parts the scrolled cases do not show: the
header with `???????????` and with a label, a name without an extension, the indented second name.

| Level | Runs | Expected | Command |
|---|---|---|---|
| Host | the Rust core with a DOS model over the pcdisk images (searches in directory order, the search position kept in the FCB), output drawn as the console draws it | the oracle rows | `cargo test` |
| Original × Rust emulator | the original under PC DOS 2.00 on `system/ibm_pc/emulator` (5150 board, 64 KiB, two drives) | the oracle rows | `cargo test --release --test rust_emulator -- --ignored` |
| Rewrite × Rust emulator | `build/tree.com` in place of TREE.COM there | the oracle rows | same |
| Rewrite × MartyPC 5160 | `build/tree.com` in place of TREE.COM on the same disk | the oracle rows | `capture_dos.sh` with `PROGRAM=`, then `compare.py` |
| Original × MartyPC 5160 | the oracle itself | — | `capture_dos.sh` |

Not covered by a case: the header and label as shown on MartyPC (scrolled off; checked on the host
against the spec), `Invalid path` (needs a path over DOS's limit), a list over the chunk size
(spec D1, the original's endless `None`), a 12-character directory name inside a path, a switch
character other than `/`, DOS other than 2.x, Ctrl-Break.
