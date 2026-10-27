# chkdsk test plan

The oracle is the original `CHKDSK.COM` running under PC DOS 1.00 on the ai_hw_engineer RTL (`Vibm_pc`), never the Rust code ([escaped-defects](../../../../spec/escaped-defects.md)). Each case is: a disk image built with `pcdisk` (or a hand-damaged copy), the command line, and the expected screen text plus the final bytes of the disk.

## Disk set

| Case | Disk | Command | Exercises |
|---|---|---|---|
| D1 | the PC DOS 1.00 disk as shipped | `CHKDSK B:` (mounted in B:) | normal report, many files |
| D2 | blank disk from `pcdisk blank` | `CHKDSK B:` | no files, all space free |
| D3 | one file of each size class (0, 1, 511, 512, 513, 1024, 100000 bytes) | `CHKDSK B:` | size to cluster rounding |
| D4 | D3 with a file whose directory size is wrong | `CHKDSK B:` | *File size error*, repair, final disk |
| D5 | D3 with a chain that ends early / runs into a free cluster | `CHKDSK B:` | *Allocation error* |
| D6 | D3 with two files sharing a cluster | `CHKDSK B:` | *Files cross-linked* |
| D7 | D3 with clusters marked used and owned by nobody | `CHKDSK B:` | lost chains, *bytes disk space freed* |
| D8 | full disk | `CHKDSK B:` | zero available |
| D9 | media byte changed from FEh | `CHKDSK B:` | *Diskette not initialized* |
| D10 | any | `CHKDSK`, `CHKDSK C:`, `CHKDSK B: X` | drive and parameter errors |
| D11–D19 | boundary disks of `scripts/make_disks.py`: first cluster beyond the disk (400, 315), chain entry beyond the disk (0200h, 315), FF7h and FF8h in a chain, a loop, a size without clusters, one lost cluster, a lost last cluster | `CHKDSK B:` | each threshold of the checks, found by wrong variants of the core that survived earlier cases |

## Layers

1. `pcdisk` unit tests: format round trip, real-disk listing.
2. Oracle replay on the host (`tests/oracle_replay.rs`): the core against `HostDos` for every transcript; the test disks are rebuilt in Rust and checked against the recorded "before" hash, the disk afterwards against the recorded "after" hash.
3. Target on the Rust emulator (`tests/target_dos.rs`, `--ignored`): `bin/chkdsk.com` under the real PC DOS 1.00, screen and disk B against the original's transcripts.
4. Target on the RTL (`scripts/rtl_target.sh`, `oracle/rust_on_rtl/*.txt`, checked by `tests/rtl_target.rs`): the same, recorded on the RTL, against the original's.
5. Negative controls: seven deliberately wrong variants of the core (thresholds, boundaries, counters) each fail the replay; the transcripts of the boundary cases (019–024) were added because earlier variants survived.
6. Not done: mutation testing with `cargo-mutants` (not installed in the images), disks of other sizes (the DOS 1.00 media byte FEh only).
