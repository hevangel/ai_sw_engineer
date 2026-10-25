# MIC 01 monitor final report

| Check | Result |
|---|---|
| Host replay | 7/7 transcripts (6 shared, 1 host-only) |
| RTL replay, test layout | 7/7 (6 shared, 1 target-only) |
| ROM image vs EPROM | 3065 / 2048 bytes: does not fit (+1017) |
| RAM (MON-10) | 0x0700–0x0745: 70 bytes of frames and state (the original: 0x07E0–0x07FF) |
| Call depth (MON-10) | 4 of 7 levels |
| Secondary emulator | not run |

## Known gaps and limits

- The ROM image is 50% over the EPROM. The SCELBAL work (llvm-i8008 known
  bits for page-aligned data, a non-returning error path, asm for carry
  loops; `../basic/plans/implementation_plan.md`) shows where the bytes go;
  the monitor has not had that pass yet.
- Behavioral, from the manual: the formats the manual leaves open are
  assumptions MON-1..9.

## Lessons

- A manual as the only oracle leaves the formats open; every choice is in
  the ledger so a recovered ROM or figure can check it.
