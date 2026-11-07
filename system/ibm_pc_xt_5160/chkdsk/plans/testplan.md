# CHKDSK test plan

The oracle is the original `CHKDSK.COM` of PC DOS 2.00 run by PC DOS 2.00 on MartyPC's IBM 5160 (256 KiB) with the
original XT BIOS (`../scripts/capture_dos.sh` → `oracle/transcripts/`). Each case (`scripts/cases.py`) is a command
typed at `A>`, a drive B disk, and (wait text, keys) pairs for what is typed after it. The drive B disk is either an
image cases.py writes byte by byte (`b:image:NAME.IMG`: a 360 KiB disk in pcdisk's layout with the damage the case
is about) or a blank 360 KiB disk built with pcdisk's steps (`b:mkdir:PATH`, `b:label:LABEL`, `b:add:PATH=FILE`);
`-` leaves drive B empty. Every level builds that disk the same way (`tests/common/mod.rs`). The Y/N prompts read a
line, so the answers are typed with Enter (`y\r`); a wait text starting with `$` means the last non-blank row ends
with the rest (`until endswith`): c08 types its second command at the `A>` after the first, and c15 types its
command lines at COMMAND's `B>` prompts.

## What is compared

Empty rows included, right-trimmed:

- if the oracle's screen shows the case's command line (`A>` + the command), the rows from the one after it to the
  next `A>`, in the oracle and in the run (from the run's last such command line);
- otherwise the oracle's rows from the top through its final `A>` against the same number of rows of the run ending
  with its final `A>`. This is c08: its first command (`chkdsk b:/f`) has scrolled off, and what is compared is the
  end of the /F run's report and the whole `chkdsk b:/v` run on the fixed disk, which proves the fix.

The same rule is in `tests/common/mod.rs` (`compared`) and `scripts/compare.py` (the command from the transcript's
`# oracle:` header). The host replay draws the session as COMMAND does: each command line after its prompt (`A>`, `B>`), CR LF, CHKDSK's output (with the keys
echoed as function 0Ah echoes them), COMMAND's CR LF and the next prompt (`X:` changes the default drive), unscrolled, and takes its last rows.

| Case | Disk B / command | What it checks (spec section) |
|---|---|---|
| c01 | EMPTYDIR, `chkdsk b:` | a directory's `.`/`..`, the report lines that are left out when 0 (Report) |
| c02 | SUB, SUB\INNER, three files, `chkdsk b:` | nested directories counted, file clusters (Walk, Report) |
| c03 | c02, `chkdsk b:/v` | `/V`: `Directory` lines and the indented file names, depth first (Walk) |
| c04 | c02 + label, `chkdsk b:` | the label line with pcdisk's zero date (`created  , 1980 12:00a`), the label counted as a 0-byte hidden file (Volume label) |
| c05 | ATTR.IMG, `chkdsk b:/v` | a dated label, hidden and system files, a zero-length file, the label listed by `/V` (Volume label, Walk) |
| c06 | LOST.IMG, `chkdsk b:`, `n` | lost chain: `Errors found`, `lost clusters found in`, N frees them (`would be freed`, LCLUS) (Lost clusters) |
| c07 | LOST.IMG, `chkdsk b:`, `y` | Y without `/F`: `would be in ... recovered files` (Lost clusters, Report) |
| c08 | LOST.IMG, `chkdsk b:/f`, `y`, then `chkdsk b:/v` | `/F`: FILE0000.CHK written in the first free root slot with the chain's size (INT 26h), seen by the second run |
| c09 | CROSS.IMG, `chkdsk b:` | the second pass: both files named at the shared cluster (Chains) |
| c10 | SIZE.IMG, `chkdsk b:` | sizes too big and too small for the chain (Chains) |
| c11 | INVALID.IMG, `chkdsk b:` | a link past the last cluster, then the size; a first cluster whose FAT entry is free (Chains) |
| c12 | FRAG.IMG, `chkdsk b:*.txt` | the file check after the report: a fragmented file's block count (CHECKFILES) |
| c13 | FRAG.IMG, `chkdsk b:contig.txt` | `All specified file(s) are contiguous.` |
| c14 | `chkdsk q:` | the invalid drive check on AL at entry (Start) |
| c15 | c02's disk; `b:`, then `a:chkdsk` at `B>`, then `a:` | no drive given: the target is the default drive B while the program runs from A; the default drive is B again at the end (DONE). Compared: the rows after `A>b:` to the next `A>` |

`tests/oracle_replay.rs` also checks, from the spec, what no case shows: a bad switch (`/x Invalid parameter`), and
`/F` on INVALID.IMG writing the fixes (the chain ended in both FAT copies, the sizes and the bad first cluster
corrected) after which CHKDSK reports nothing.

| Level | Runs | Expected | Command |
|---|---|---|---|
| Host | the Rust core with a DOS 2.00 model over the disk images (DPB with the current directory, searches in slot order with the entry number in the FCB, INT 25h/26h on the image, the oracle machine's memory: top 4000h, segment 0600h), output drawn as the console draws it | the oracle rows (drive A is PC DOS 2.00 disk 1 with `XT_ROMS`, else blank; no case checks it) | `cargo test` |
| Original × Rust emulator | the original under PC DOS 2.00 on `system/ibm_pc/emulator` (5150 board, `Machine::new(256)`, two drives) | the oracle rows | `cargo test --release --test rust_emulator -- --ignored` |
| Rewrite × Rust emulator | `build/chkdsk.com` in place of CHKDSK.COM there | the oracle rows | same |
| Rewrite × MartyPC 5160 | `build/chkdsk.com` in place of CHKDSK.COM on the same disk | the oracle rows | `capture_dos.sh` with `PROGRAM=`, then `compare.py` |
| Original × MartyPC 5160 | the oracle itself | — | `capture_dos.sh` |

The Rust emulator presents SW2 for its RAM size (`Board.sw2`, 6 for `Machine::new(256)`), so the 5150 BIOS, DOS
and CHKDSK see 256 KiB as on MartyPC. The rewrite is larger than the original (spec D5); no case checks the disk that
holds it.

Not covered by a case: `.`/`..` damage and recovery, a directory to convert to a file, `Probable non-DOS disk`, disk
errors and `Abort, Retry, Ignore?`, bad sectors (FF7h), more chains than free root entries, `File not found`, a
switch character other than `/`, DOS other than 2.00, Ctrl-Break and critical errors.
