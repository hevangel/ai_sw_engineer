# ASSIGN test plan

The oracle is the original `ASSIGN.COM` of PC DOS 2.00 run by PC DOS 2.00 on MartyPC's IBM 5160 with the original
XT BIOS and 256 KiB (`../scripts/capture_dos.sh` → `oracle/transcripts/`). Each case (`scripts/cases.py`) boots
with PC DOS 2.00 disk 1 plus `ATEXT.TXT` in drive A and a blank 360 KiB disk holding `ONLY.TXT` in drive B, types
an ASSIGN command at `A>`, then further command lines (more ASSIGN commands, `dir` of a pattern, `type`), each once
the last non-blank row ends with `A>` (`$A>`). The screen is all that is compared: what DIR and TYPE show is where
the redirection is visible. Every level builds the disks the same way (the tests with the pcdisk library,
`tests/common/mod.rs`; MartyPC with the pcdisk tool). The output of `cases.py` is split on `\n` only.

## What is compared

Empty rows included, right-trimmed: the rows from the case's first command line (`A>` + the command) through the
screen's last row that is only `A>`, in the oracle and in the run. Every transcript shows its command line (in s01
the boot's rows have scrolled off above it). The same rule is in `tests/common/mod.rs` (`compared`) and
`scripts/compare.py`.

| Case | Commands after the first | What it checks (spec section) |
|---|---|---|
| s01 `assign a=b` | `dir a:*.txt`, `assign`, `dir a:*.txt` | FCB searches, 47h, 36h go to B under A's letter; ASSIGN.COM is then looked for on B (`Bad command`), so the second DIR is still B's (the hook, Consequences) |
| s02 `assign a=b b=a` | `type a:only.txt`, `type b:atext.txt`, `assign` | two pairs; path letters translated both ways (the parse, the hook) |
| s03 `assign c=b` | `type c:only.txt`, `dir c:*.txt`, `assign` | a letter DOS does not have: 29h is not translated (`Invalid drive specification`); `assign` alone found and reset (Finding) |
| s04 `assign A:=b:` | `type a:only.txt`, `assign` | a colon before `=` is `Invalid parameter` (no CR LF); nothing installed |
| s05 `assign a=b` | `assign c=b`, `type a:atext.txt`, `type c:only.txt`, `assign` | ASSIGN cannot be found while A is on B; A stays on B |
| s06 `assign a=q` | `type a:atext.txt` | a drive above H is `Invalid parameter` |
| s07 `assign a` | `assign a=`, `assign ab=c`, `type a:atext.txt` | no `=` installs the identity; `=` then CR is an error; `ab=c` is B = C (only the character before `=` counts) |
| s08 `assign` | `type a:atext.txt` | the identity table passes everything through |

`tests/oracle_replay.rs` also checks, from the spec, what no case shows: the parse of `xy ab=c a=b a=h` and of five
errors, a second run replacing the resident table and keeping 10Bh (and the `B>` prompt after the identity is put
back), FCB drive 0 coming back as the logical drive, an extended FCB, 0Eh/19h/47h, the signed lookup (entry 0 is the
`JMP`'s 02h, 9 is 10Bh), INT 25h/26h, the equipment routine, the DOS 1 message.

| Level | Runs | Expected | Command |
|---|---|---|---|
| Host | the Rust core (`run`) and the hook model (`hook`) in front of a model of DOS 2.00 and COMMAND over the pcdisk images (spec H1-H3), output drawn as the console draws it | the oracle rows | `cargo test` (drive A is PC DOS 2.00 disk 1 with `XT_ROMS`, else a blank disk) |
| Original × Rust emulator | the original under PC DOS 2.00 on `system/ibm_pc/emulator` (5150 board, `Machine::new(256)`, two drives) | the oracle rows | `cargo test --release --test rust_emulator -- --ignored` |
| Rewrite × Rust emulator | `build/assign.com` as ASSIGN.COM there | the oracle rows | same |
| Rewrite × MartyPC 5160 | `build/assign.com` as ASSIGN.COM on the same disks | the oracle rows | `capture_dos.sh` with `PROGRAM=`, then `compare.py` |
| Original × MartyPC 5160 | the oracle itself | — | `capture_dos.sh` |

Not covered by a case: INT 25h/26h (no case runs a program that reads sectors), EXEC with a drive letter, 0Eh to
another drive while assigned, the equipment routine on a one-drive machine, DOS 1, a second copy installed behind
another INT 21h hook, the rewrite and the original mixed (spec D2: they do not recognise each other's copy).
