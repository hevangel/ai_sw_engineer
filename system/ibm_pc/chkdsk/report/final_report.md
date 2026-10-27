# chkdsk final report

The Rust `CHKDSK.COM` for PC DOS 1.00 on the IBM PC 5150, built by `llvm-i8086`, against the original `CHKDSK.COM` of PC DOS 1.00.

| Check | Result |
|---|---|
| Host replay (the core against a model of DOS, screen and disk afterwards) | 24 / 24 transcripts identical (`cargo test`) |
| Target image size | `bin/chkdsk.com` 3501 bytes (original 1395); no ROM budget (a .COM in a 64 KiB segment) |
| RAM use | one segment: 3.5 KB image, about 3.5 KB of tables and stack frames (owner table 684 B, 64 records of 44 B, FAT and search state) |
| Rust emulator running the Rust `.COM` under the real PC DOS 1.00 | 24 / 24 cases: same rows after the command as the original on the RTL, same disk B afterwards (`tests/target_dos.rs`) |
| RTL running the Rust `.COM` under the real PC DOS 1.00 | 8 / 8 recorded cases identical to the original on the RTL, screen and disk (`tests/rtl_target.rs`) |
| Negative controls | 7 wrong variants of the core, all caught after cases 019-024 were added (three survived the first 18) |
| Original binary | PC DOS 1.00 `CHKDSK.COM`, SHA-256 in [provenance](../original/provenance.md); not in the repository |

## Known gaps and limits

- The oracle is 24 disks of the 160 KiB single-sided format with 512-byte sectors and one sector per cluster. Other media (the original accepts only media byte FEh) are not in play, and the cluster-size shift is therefore never exercised with a value other than 0.
- Only 8 of the 24 cases ran on the RTL with the Rust program (every kind of repair among them); the other 16 ran on the Rust emulator, which matches the RTL on the platform's other tests but is a second implementation.
- Not reproduced (spec, "Not reproduced"): a first cluster of 1 (the original corrupts its own stack), directories above 64 entries and disks above 340 clusters (the Rust tables are fixed), a quotient above 16 bits in the size check.
- The critical-error handler is reproduced only for its write-protect case (the case the program can meet); the other branch (chain to the previous handler) is not exercised by any test.
- The date stamped on repaired files is the one typed at the DOS prompt (10-14-81) in all recordings; DOS supplies it, the program does not.
- Hand-made mutants stand in for mutation testing (`cargo-mutants` is not installed in the images).

## Lessons

- Reading the disassembly was enough for the structure but wrong in details that only the oracle exposed: what the search returns (every entry that is not E5h), the "Invalid parameter" condition, how a size is repaired (open, block write of zero records, close, with DOS stamping the date) and the second space in two messages.
- Running the Rust program under real DOS found what no host model could: the original installs a critical-error handler around its final disk reset; without it the program, checking the write-protected drive A, ends in `Write protect error writing drive A`.
- Compiling the title exposed a backend bug: `llvm.memset` and `memcpy` with an `i64` length (LLVM makes one for the odd-sized tail of a struct initialiser) crashed `llc` with "Do not know how to expand this operator's operand". Fixed in `I8086SelectionDAGInfo` (the length is truncated to the 16-bit count) with a FileCheck test.
- The boundary disks (019-024) were added because hand-made wrong variants of the core survived the first 18 cases: a threshold off by one is only seen by a disk that sits exactly on it.
- The Rust program is 2.5 times the original, mostly the runtime it brings (`dos_call`, the compiler runtime library, the critical-error handler) and the unoptimised-by-hand code of `core`.
