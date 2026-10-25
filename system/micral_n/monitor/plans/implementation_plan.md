# MIC 01 monitor implementation plan

1. Spec from the manual, ledger MON-1..11 (done). 2. Transcripts from the
   manual (done). 3. Rust monitor; host replay (done). 4. Target in a test
   layout; RTL replay (done). 5. Fit the 2 KiB ROM (open: 3065 B).
6. Report (done).

## Design decisions

- State in statics (a static costs no more than a frame slot on the 8008);
  counters `u8`; the entries, holds and the `G` trampoline in asm.
- Next for the ROM fit: the SCELBAL techniques (page-aligned data, a
  non-returning error path for `?`, fewer call sites for big helpers).
