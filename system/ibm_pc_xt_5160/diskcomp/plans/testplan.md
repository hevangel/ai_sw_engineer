# DISKCOMP test plan

The oracle is the original `DISKCOMP.COM` of PC DOS 2.00 run by PC DOS 2.00 on MartyPC's IBM 5160 (256 KiB) with the
original XT BIOS (`../scripts/capture_dos.sh` → `oracle/transcripts/`). Each case (`scripts/cases.py`; its output is
split on `\n` only, since a key may be a space) is a command typed at `A>`, the disks, and (wait text, keys) pairs
for what is typed after it. Drive A is, with the setup item `a:system360`, the double-sided 360 KiB copy of PC DOS
2.00 disk 1 (`pcdisk syscopy`, the library's `Disk::system_copy` with `GEOMETRY_360`), else disk 1 itself, with
the program under test as DISKCOMP.COM (for a rewrite run `pcdisk add` replaces the original there). Drive B is,
for `b:copy_a`, a copy of drive A as built so far (so it holds the same program under test), for
`b:image:ZERO360.IMG` 360 KiB of zeros, else a blank 360 KiB disk; then pcdisk's `b:` steps (`b:mkdir:SUB`,
`b:add:SUB\EXTRA.TXT=EXTRA.TXT`); `-` leaves drive B empty. Every level builds the disks the same way
(`tests/common/mod.rs` `disks`, which gives the same images as capture_dos.sh: checked against `build/oracle/`
on 2026-11-08). The keys are single keys (0C01h); a wait text starting with `$` means the last non-blank row ends
with the rest (`until endswith`, the screen having changed since the previous wait): k06 and k07, whose prompts
repeat. The longer cases need `MARTY_CYCLES=1500000000` on MartyPC (`run_all.sh` uses it for every case).

## What is compared

**The screen**, empty rows included, right-trimmed:

- if the oracle's screen shows the case's command line (`A>` + the command), the rows from the one after it to the
  next `A>`, in the oracle and in the run (from the run's last such command line): k01, k02, k03, k06, k07, k08;
- otherwise (the output scrolled it off: k04, k05) the oracle's rows from the top through its final `A>` against
  the same number of rows of the run ending with its final `A>`.

DISKCOMP writes nothing, so there is no drive B hash; the Rust-emulator runs also check that both disks are as
they were mounted. The rule is in `tests/common/mod.rs` (`compared`) and `scripts/compare.py`.

**Where the rewrite lands on the disks.** A rewrite run has the rewrite as DISKCOMP.COM on drive A and, by
`b:copy_a`, on drive B. `pcdisk add` frees the original's clusters 62-64 (cylinder 7) and takes the lowest free
clusters: those three, then from cluster 155, the first after BASICA.COM; clusters 155-157 are still in
cylinder 17 head 1 (sectors 315-323, which also hold the end of BASICA.COM). So with 3 to 5 clusters (2,049 to 5,120 bytes) the rewrite occupies the
original's tracks plus cylinder 17 head 1, which differs from a blank disk in the original's run too (BASICA.COM
ends there), and EXTRA.TXT (k02) and BIG.BIN (k03) still start in cylinder 17 head 1. Measured on the disks of
both MartyPC runs (`build/oracle/`, `build/rust_run/`), the tracks that differ between drive A and drive B are the
same in every case: k01 and k06 none, k02 (0,0) (17,1), k03 (0,0) (17,1) (18,0) (18,1), k04 all of cylinders
0-17 (36 tracks), k05 all 80. DISKCOMP reports each track that differs once, so the reported tracks are the same,
including those that scroll off the screen in k04 and k05. `oracle_replay.rs` checks this on every run with
`build/diskcomp.com` (`the_rewrite_on_the_disks_changes_no_reported_track`: the core's whole output on the
rewrite's disks against the original's, and the size bound). A rewrite of 2,048 bytes or less would move EXTRA.TXT
to cluster 64 (cylinder 7 head 1), one over 5,120 bytes would move it to cylinder 18: either changes k02's
report, so the bound is a requirement on the build (`build.sh` prints it).

| Case | Disks / command / keys | What it checks (spec section) |
|---|---|---|
| k01 | A: system360; B: copy of A; `diskcomp a: b:`, space, `n` | two drives, two sides: `Comparing 9 sectors per track, 2 side(s)` at head 1 of cylinder 0, two passes with the roles swapped, `Diskettes compare ok` (Compare, Read) |
| k02 | B: the copy plus EXTRA.TXT | `Compare error(s) on` / `Track 00, Side 0` (FAT and directory) and `Track 17, Side 1` (the file's cluster); no `compare ok` (Compare) |
| k03 | B: the copy plus BIG.BIN (10 KiB) | four tracks across cylinders 17-18, both sides, both halves of the compare (Compare) |
| k04 | B: a blank disk with SUB\EXTRA.TXT | every track of cylinders 0-17 differs, 18-39 do not (scrolls) |
| k05 | B: 360 KiB of zeros | all 80 tracks differ (scrolls) |
| k06 | k01, then `y`, space, `n` | `Compare more`: the checks, CAP and SMALL made again (28Bh) |
| k07 | A: disk 1; B: a blank disk with SUB\EXTRA.TXT; `diskcomp b: b:`, three spaces, `n` | one drive: first, second, first (the roles swap, so one swap a pass), `Diskettes compare ok` (the same disk) |
| k08 | `diskcomp a: q:` | `Invalid drive specification` from AX at entry (Start) |

`tests/oracle_replay.rs` also checks, from the spec, what no case shows: a file name in either FCB, `/2`, `/1/9`
and `q:` refused without a disk read; two sides with differences in cylinders 3 (side 1) and 25 (side 0) and the
order of the reads; `/1` (head 0 only, one-sided cylinders two at a time); `/8` (8 sectors, sector 9 ignored, and
`y` for a second compare); a one-sided image with MartyPC's status 20h for head 1 (a read error, compared as
two-sided).

| Level | Runs | Expected | Command |
|---|---|---|---|
| Host | the Rust core with a model of the oracle machine: INT 13h on the images as MartyPC's 5160 answers (20h for a head the image lacks, 09h across 64 KiB, 04h past the track) into 256 KiB of memory, PSP at 0600h, top 4000h, the buffers from the original's 810h, `REPZ CMPSB` on that memory | the oracle rows (with `XT_ROMS`); the whole output the same on the rewrite's disks (with `build/diskcomp.com`) | `cargo test` |
| Original × Rust emulator | the original under PC DOS 2.00 on `system/ibm_pc/emulator` (5150 board, `Machine::new(256)`, two drives) | the oracle rows; both disks unchanged | `cargo test --release --test rust_emulator -- --ignored` |
| Rewrite × Rust emulator | `build/diskcomp.com` as DISKCOMP.COM there | the oracle rows, and the same as the original there; both disks unchanged | same |
| Rewrite × MartyPC 5160 | `build/diskcomp.com` as DISKCOMP.COM on the same disks | the oracle rows | `capture_dos.sh` with `PROGRAM=`, then `compare.py` |
| Original × MartyPC 5160 | the oracle itself | — | `capture_dos.sh` (never run without `PROGRAM` except to re-capture the oracle) |

Not covered by a case: one-sided and 8-sector disks, `/1` and `/8` on an emulator, a second disk of another
format (`Incompatible diskette or drive types`), unreadable tracks, not-ready drives, one-drive machines, less
memory than 256 KiB, DOS 1.x.
