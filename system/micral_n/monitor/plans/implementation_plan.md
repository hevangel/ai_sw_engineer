# MIC 01 monitor implementation plan

1. Spec from the manual, ledger MON-1..11 (done). 2. Transcripts from the
   manual (done). 3. Rust monitor; host replay (done). 4. Target in a test
   layout; RTL replay (done). 5. Fit the 2 KiB ROM (done: 2040 B, RTL replay
   from the ROM). 6. Report (done).

## Design decisions

- State in statics (a static costs no more than a frame slot on the 8008);
  counters `u8`; the entries, holds and the `G` trampoline in asm.
- The ROM fit (3065 to 2040 bytes, `report/size_history.md`): `?` restarts
  the command loop instead of returning through every caller; hold patching
  in asm next to the hold entries; the saved flags as separate bytes ahead
  of A..L, so `I`/`A`/`H` are one loop; the range commands share two
  statics; checksums accumulate in `hex8` / `tape_byte`; the cold-start loop
  in the gap between the two entries. In llvm-i8008: page-indexed loads and
  stores of small globals, compares as values without branches, best-fit
  code placement around the pinned entries.
