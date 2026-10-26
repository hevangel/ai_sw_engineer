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

## Layers

1. `pcdisk` unit tests (done): format round trip, real-disk listing.
2. Oracle replay on the host: the Rust core against every transcript.
3. Target: the Rust `CHKDSK.COM` on the Rust emulator, compared with the oracle (screen and disk).
4. Target on the RTL for the same disks, compared with the original (screen, disk, and not just the end state: the register state at exit when the two runs are identical programs).
5. Negative controls: a deliberately changed message, count or repair must fail the comparison.
6. Mutation testing of the portable core (`cargo-mutants`) against the transcripts.
