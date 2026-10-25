# 2026-10-24 — SCELBAL on the Micral N, byte for byte; Micral N docs current

Branch `llvm/i8008`. Continues the session that built llvm-i8008, the Micral
N harness, the boot ROM, the monitor and Shooting Stars (commits 53c4c58 to
7bb1e51).

## Outcome

- **SCELBAL** (system/micral_n/basic): the oracle is the 1974-75 listing's
  own bytes with a Micral N teletype patch, run on the RTL; 10 transcripts.
  The Rust interpreter follows the listing routine by routine over its
  memory layout, floating point included. Host 10/10, RTL 10/10 loaded
  directly and booted from disk by the Rust boot ROM. Image 13826 B of the
  14080 B load area; program buffer 9 pages instead of 18 (BAS-4).
- All four Micral N titles rebuilt with today's backend and re-verified
  through their scripts: boot 245 B; Shooting Stars 4/4 direct and booted;
  monitor 7/7 in the test layout, ROM image 3065/2048 B (open).
- llvm-i8008: lit 6/6, `exec_tests.py 30` 4650/4650, smoke program OK.

## Work

- llvm-i8008:
  - known bits for aligned globals (`computeKnownBitsForTargetNode`), so a
    256-aligned table indexes as `H = page, L = i`
  - `rt/mem.s` memset reads its value from D (escaped defect, failure note)
  - linker `RESET` directive: restart points with their own depth budget
  - `retarget_ir.py` type checks no longer match value names (`%x.i128`)
  - `codegen.md` and `abi.md` brought up to date (leaf convention, IPRA,
    data page, A tracking, byte pairs, the above)
- SCELBAL size: 15.8 KB of code to 10.7 KB (non-returning errors, aligned
  RAM, single call sites for parser/operators, the RAM image as data, asm
  carry primitives, the array page at 0x3700).
- Docs and scripts for every Micral N title (they were still templates):
  README, test plan, implementation plan, report, oracle README; build,
  run_sim, compare, run_all wired to the real flow. `cargo fmt` and
  clippy clean on all four.
- Failure notes: memset value register; the RTL's RTc decode (fixed in
  ai_hw_engineer, uncommitted there as agreed).

## Open

- Monitor ROM fit: done later in the session (below).
- Secondary emulator (MO5 Virtual Micral N) not in `external/` yet.
- The ai_hw_engineer RTc fix awaits its maintainers.

## Later: the monitor fits its 2 KiB ROM

At the user's request ("work on fitting the monitor into its 2 KB ROM").

- MIC 01: 3065 to **2040 bytes**; the ROM image links at 0x3800 with the
  entries at 0x3B30 / 0x3B49 and runs 7/7 on the RTL from the ROM (the test
  layout is gone). Host 7/7. Steps in `monitor/report/size_history.md`:
  `?` restarts the command loop (linker RESET); hold patching in asm behind
  the Hal; flags saved as separate bytes ahead of A..L; the range commands
  on two statics; checksums in hex8/tape_byte; the cold-start loop in the
  gap between the entries.
- llvm-i8008: one section per data object (forced in the TargetMachine);
  page-indexed loads/stores of globals of at most 256 bytes (`LDPG8`,
  `STPG8`, `LEAPG`); `%hi`/`%lo` bytes as `MOV8sym`; compares as values
  without branches (`SETCC*`); best-fit code placement around PLACEd
  sections; retarget_ir accepts code addresses taken as integers. Lit 6/6
  (new checks), exec_tests 4650/4650.
- Every title shrank: boot 241 B, Shooting Stars 2458 B, SCELBAL code
  10495 B, which gave SCELBAL a tenth program page (2560 B). All re-verified
  on the RTL through their scripts.
