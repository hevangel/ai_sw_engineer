# MIC 01 monitor final report

| Check | Result |
|---|---|
| Host replay | 7/7 transcripts (6 shared, 1 host-only) |
| RTL replay, from the ROM image at 0x3800 | 7/7 (6 shared, 1 target-only) |
| ROM image vs EPROM | 2040 / 2048 bytes |
| RAM (MON-10) | 0x0700–0x0731: 50 bytes of saved registers, holds, trampoline, statics and frames (the original: 0x07E0–0x07FF) |
| Call depth (MON-10) | 7 of 7 levels |
| Secondary emulator | not run |

## Known gaps and limits

- Behavioral, from the manual: the formats the manual leaves open are
  assumptions MON-1..9.
- 8 bytes of ROM are left. The next feature will need a size pass first.

## Lessons

- A manual as the only oracle leaves the formats open; every choice is in
  the ledger so a recovered ROM or figure can check it.
- Fitting 2 KiB took both sides: the monitor's own structure (restart on
  `?`, asm for code patching, data laid out for the 8008) and backend work
  that helped every title (page-indexed accesses, branch-free compares,
  placement around fixed entries). The size history has each step.
