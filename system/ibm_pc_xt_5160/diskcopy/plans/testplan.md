# DISKCOPY test plan

The oracle is the original `DISKCOPY.COM` of PC DOS 2.00 run by PC DOS 2.00 on MartyPC's IBM 5160 (256 KiB) with the
original XT BIOS (`../scripts/capture_dos.sh` with `SAVE_B=1` → `oracle/transcripts/`). Each case
(`scripts/cases.py`; its output is split on `\n` only, since a key may be a space) is a command typed at `A>`,
the disks, and (wait text, keys) pairs for what is typed after it. Drive A is PC DOS 2.00 disk 1 or, with the setup
item `a:system360` (every A-to-B case), its double-sided 360 KiB copy (`pcdisk syscopy`, the library's
`Disk::system_copy` with `GEOMETRY_360`: the files in directory order with their attributes, disk 1's boot record
with the 360 KiB BPB), plus the setup's plain file names (d04: `EXTRA.TXT`). Disk 1 itself is one-sided, and
reading its head 1 is where MartyPC and the hardware differ (spec "Head 1 of a one-sided image"), so no case copies
it. Drive B is the image cases.py writes (`b:image:ZERO360.IMG`: 360 KiB of zeros) or a blank 360 KiB disk built
with pcdisk's steps (`b:mkdir:SUB,b:add:ROOT.TXT=EXTRA.TXT`); `-` leaves drive B empty. Every level builds the
disks the same way (`tests/common/mod.rs`; the rewrite is added to drive A after the syscopy, as capture_dos.sh
does). The keys are single keys (0C01h); a wait text starting with `$` means the last non-blank row ends with the
rest (`until endswith`, the screen having changed since the previous wait): d05 and d08, whose prompts repeat.
d05 needs `MARTY_CYCLES=1500000000` on MartyPC (`run_all.sh` uses it for every case).

## What is compared

**The screen**, empty rows included, right-trimmed:

- if the oracle's screen shows the case's command line (`A>` + the command), the rows from the one after it to the
  next `A>`, in the oracle and in the run (from the run's last such command line);
- otherwise (the output scrolled it off; no current case) the oracle's rows
  from the top through its final `A>` against the same number of rows of the run ending with its final `A>`.

**Drive B**: the SHA-256 of drive B's raw image after the run against the transcript's `# drive B after:`, by this
rule. A run of the original, and a case that does not copy drive A (d08 copies B to B), is compared as it is. A
run of the rewrite that copies drive A had the rewrite on drive A as DISKCOPY.COM (it replaces the original
there), so its drive A, and the copy of it, differs from the oracle's in DISKCOPY.COM's sectors, the FAT and the
directory. Before hashing, every sector of drive B that equals the run's drive A sector it was copied from (same
cylinder, head, sector) is replaced by the original's drive A sector (the case's drive A with the original
DISKCOPY.COM, as `capture_dos.sh` builds it without `PROGRAM`). A sector that was not copied right stays as it is and fails the hash;
everything else is compared byte for byte. For the original the rule changes nothing. The same rule is in
`tests/common/mod.rs` (`normalized`) and `scripts/compare.py` (with the run's work directory and `build/original_a/`,
which `run_all.sh` builds as capture_dos.sh builds drive A without `PROGRAM`).

| Case | Disks / command / keys | What it checks (spec section) |
|---|---|---|
| d01 | A: system360; B: zeros; `diskcopy a: b:`, space, `n` | the copy of a two-sided disk with two drives: `Copying 9 sectors per track, 2 side(s)` at head 1 of cylinder 0, two passes of 20 cylinders, `Copy complete`; drive B is drive A (Read pass, Write pass) |
| d03 | A: system360; B: a formatted disk with files; same | the same over a disk that had files (all of it is overwritten) |
| d04 | A: system360 + EXTRA.TXT; B: zeros; same | the copy follows drive A's contents |
| d05 | d01, then `y`, space, `n` | `Copy another`: a second copy with the checks and CAP made again (217h) |
| d06 | `diskcopy a: q:` | `Invalid drive specification` from AX at entry (Start) |
| d07 | d01 with `diskcopy a: b: x` | a third parameter is ignored (Start) |
| d08 | B: a two-sided disk; `diskcopy b: b:`, four spaces, `n` | one drive: four swaps (two passes of 20 cylinders, spec Memory), `Copy complete`, the disk the same after (Copy) |

`tests/oracle_replay.rs` also checks, from the spec, what no case shows: a file name in either FCB and `/2`
(`Invalid parameter`), the two passes of a two-sided disk with one drive, and `/1` copying head 0 only.

| Level | Runs | Expected | Command |
|---|---|---|---|
| Host | the Rust core with a model of the oracle machine: INT 13h on the images as MartyPC's 5160 answers (20h for a head the image lacks, 09h across 64 KiB, 04h past the track) into 256 KiB of memory, PSP at 0600h, top 4000h, the buffer from the original's 9A0h | the oracle rows; drive B with `XT_ROMS` (drive A blank without it) | `cargo test` |
| Original × Rust emulator | the original under PC DOS 2.00 on `system/ibm_pc/emulator` (5150 board, `Machine::new(256)`, two drives) | the oracle, screen and drive B | `cargo test --release --test rust_emulator -- --ignored` |
| Rewrite × Rust emulator | `build/diskcopy.com` as DISKCOPY.COM there | the oracle, screen and drive B by the rule; and the same as the original there | same |
| Rewrite × MartyPC 5160 | `build/diskcopy.com` as DISKCOPY.COM on the same disk, `SAVE_B=1` | the oracle, screen and drive B | `capture_dos.sh` with `PROGRAM=`, then `compare.py` with the work directory |
| Original × MartyPC 5160 | the oracle itself | — | `capture_dos.sh` (never run without `PROGRAM` except to re-capture the oracle) |

Not covered by a case: an 8-sector source, a one-sided source (disk 1 itself, see above), unreadable source
tracks, a target that needs formatting (`Formatting while copying`), write, verify and format errors, a
write-protected target, not-ready drives, `Incompatible drive types`, one-drive machines, less memory than 256 KiB,
DOS 1.x.
