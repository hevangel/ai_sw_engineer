# chkdsk implementation plan

| # | Step | Done when | Status |
|---|---|---|---|
| 1 | Choose the title; record provenance of the original | [README](../README.md), [provenance](../original/provenance.md) | done |
| 2 | FAT12 disk tool for the 160 KiB floppy | `platform/tools/pcdisk` tests pass on the real DOS disk | done |
| 3 | Spec from the disassembly | [spec](../spec/spec.md) with an assumption ledger | done (A1 open) |
| 4 | Oracle: run the original on the RTL | transcripts + final disk images for the disk set in the [testplan](testplan.md) | D1–D10 recorded (transcripts 001–014); replay needs the Rust core |
| 5 | Platform: COM-under-DOS variant | link script (org 100h), crt0 with PSP, `int 21h` HAL; a Rust `hello.com` prints through DOS on the Rust emulator and on the RTL | |
| 6 | Portable core `src/lib.rs` over a `Dos` trait; host replay | every oracle transcript replays byte-exact on the host | |
| 7 | Build with `llvm-i8086`, run under PC DOS 1.00 | the Rust CHKDSK.COM prints the same text and leaves the same disk as the original on every test disk, on the Rust emulator and on the RTL | |
| 8 | Report | `report/final_report.md` with the size against the original's 1395 bytes | |

## Design decisions

- The core never calls INT 21h itself: it uses a `Dos` trait whose operations are those of [io.md](../spec/io.md). The host implementation is an in-memory model of the DOS 1.00 calls the program uses (FAT info, directory search, FCB open/close/random write).
- Data widths follow the original: 16-bit cluster numbers, 32-bit byte counts (the original converts with double-dabble; the Rust version may use `u32` arithmetic, as long as the printed text is identical).
- Not reproduced unless the oracle shows it is observable: the exact memory layout of the work area (782h and up) and the original's stack depth. The memory figures printed at the end do depend on the load segment and top of memory, so they are taken from the PSP exactly as the original does.
