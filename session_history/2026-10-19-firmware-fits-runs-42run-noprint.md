# 2026-10-19 — firmware now FITS and RUNS end-to-end (banked frames); 42-run executes but prints nothing (0/42), diagnosed to an outer loop, not a crash

Branch `wip/dcl-frames`; to be merged into `system/busicom_141pf-on-4004`.
Continues `2026-10-19-dcl-frames-green-firmware-romwall.md`. Did the brief's
"option 1": shrink the spill-heavy BCD frames (firmware Rust is ours to change)
so every per-function frame < 256 chars and every function fits a ROM bank.

Model-call evidence: one KiroCrew subagent session (model Auto/server-selected);
Kiro exposes no per-model token totals. Many `docker run` calls against
`ai-sw-engineer:latest` with the warm `ai-sw-llvm-build` volume.

## Firmware refactor (commit: firmware(bcd) ...) — host 42/42 exact, clippy/fmt clean
Root cause of the oversized frames: fixed `[u8; WIDE=34]` scratch buffers
allocated on the stack AND returned by value (sret copies + multiple WIDE
buffers coexisting on one frame). Fixes, algorithm identical:
- build_div_operands / restoring_divide / trial_product / mul_accumulate: write
  into caller `&mut` scratch instead of returning `[u8; WIDE]` by value.
- Num::divide: new `long_divide` #[inline(never)] owns divd+divs, so divide
  carries only quo_int. Frame 356 -> 146.
- isqrt_digits: drop the 2nd WIDE `best` buffer (recompute trial for chosen x);
  extract the per-pair step into `isqrt_next_digit`. Frame 290 -> 220; code
  2097 -> under a bank.
- from_mag #[inline(never)] + `place_quotient_digits` extraction: Num::mul and
  assemble_quotient ~1975 -> ~1637/1731.
Result: largest frame 220 (<256), largest function ~1790 (<2048). host
`cargo test` stays 42/42 EXACT; `cargo clippy` adds no new warnings (6 pre-
existing `unsafe` ones in regops.rs remain); `cargo fmt --check` clean.

## Firmware LINKS and RUNS (the milestone prior sessions never reached)
`BOARD=xl COMMON_BYTES=0x800 BANK_BYTES=0x800 BANKS=44`: image 92,160 B, 44
banks used, common 2048/2048. 73 per-function frames, overlay 476 chars across
CM-RAM banks 1/2/3 (per-bank high-water 208/256/94, all <= 256). (BANKS=90 also
links at 186,368 B but exceeds run_image's 128 KiB cap; 44 is the tight fit.)

Fitting required common+bank <= 4096 (the 4004's 12-bit PC) AND bank >= largest
function. The largest function is ~1790, so common 0x800 + bank 0x800 = 4096
with ~room for the dispatch stubs + far thunks is the working point; larger
common forced common+bank > 4096 (ADDR12 target > 12 bits).

## 42-run plumbing fixed (commit: platform(42-run) ...)
chips_trace.rs HAND-BUILDS the 16-chip 4-bank RAM for ram_count > 4 (Board::new
panics past 4 chips; same workaround the exec harnesses use), and run_image.sh
passes ram_count=16 for xl. The firmware then RUNS end to end across bank
switches with all four CM-RAM banks present — no crash, 42 examples produced.

## OPEN: 42-run is 0/42 — firmware prints NOTHING (newly-exposed, separate bug)
`run_image.sh secondary --board xl` + `compare.sh`: 42 examples produced but
ZERO `ROW` lines (oracle has 438) and all `LAMPS|0`. The firmware never drives
the printer/lamp ports. Diagnosis so far (build/pchist/):
- NOT a crash or startup hang: over 300k cycles the firmware reaches banked
  calculator code up to PC 0xdfc in bank 32, and `__memset_ram` is ENTERED 122
  times (each call terminates) — so it is an OUTER loop repeatedly calling
  memset, executing broadly.
- 88% of sampled PCs are inside `__memset_ram` (0x492-0x4a9): the calculator
  logic churns through many array-zeroing memsets but makes no PRINTED progress.
- Printer-port (`rams[0].ports`) toggles: **0** across 300k cycles.
- The HAL printer path is VERIFIED correct in isolation: `__bus_ram_port_write`
  (platform/runtime/hal.s) does `ldm 0; dcl` (bank 0), builds SRC chip<<2, then
  `src; wmp` — so a hammer/advance write WOULD land on bank-0 chip-0's port,
  which chips_trace reads. So the firmware simply never REACHES a print call, or
  spins before it.

## SHARPENED ROOT CAUSE (build/halcall + build/halspine)
Instrumented HAL-entry PCs and per-bank execution over 500k cycles of ex 1-1:
- `run`'s entry (0x800, bank 32) executes exactly ONCE (NOT a misdispatch
  re-running Calculator::new).
- Bank-region execution is ONLY bank 43 = 36 PCs (one-time `main`→`run`
  startup) and bank 32 = 45,322 PCs (`run`). **Banks 33-42 execute ZERO
  instructions.** `scan_pass` is in **bank 33**; dispatchers in 34-36.
- ZERO `__bus_*` HAL calls (rom_port_read/write, test_pin, ram_port_write) —
  the keyboard is never scanned, nothing is printed. 88% of PCs are the common
  `__memset_ram` loop (0x488-0x4a9).

So crt0 → `main`(bank43) → `run`(bank32) works, but inside `run`'s loop the
cross-bank CALLSOFT `run`(bank32) → `scan_pass`(bank33) NEVER transfers to bank
33 — execution stays in bank 32 + common, churning memset. No keyboard read →
`run`'s `while buffer.pop()` drives nothing → 0/42, all rows empty. This is NOT
a key-hold/timing issue (the firmware never issues the first scan HAL call); it
is a **cross-bank far-call dispatch failure for `run`→`scan_pass`**. The backend
emits `jms __push_off; jms __push_pg; jun scan_pass`; the packer rewrites the
`jun` to a far-thunk (set bank 33 via `__soft_ret`, dispatch to scan_pass's
restore trampoline). That path does not land in bank 33, while the startup
cross-bank calls DO work — so the defect is in the far-call/soft-call-return
path for a CALLSOFT made FROM banked code (origin bank 32) to a high bank (33),
vs the common/bank-0-origin startup calls.

### Next session — localize the far-call dispatch
1. Instruction-aware disasm (NOT byte-linear — the 4004's mixed 1/2-byte ops
   desync a linear scan; build/disasm_banked.py is byte-linear and unreliable
   mid-function) of `run`'s `jun scan_pass` far-thunk; single-step it on chips
   and capture the latch-port writes (roms[3]/roms[4].ports): does it drive BOTH
   nibbles to bank 33 and `jin` into scan_pass's restore trampoline?
2. Diff against a WORKING cross-bank call (crt0→main) to see what origin-bank-32
   does differently.
3. `run_softcall_banked_exec` (every call cross-bank, depth 8) PASSES, so the
   generic far-call works at small scale; the firmware differs in bank NUMBER
   (33) and depth. Build a focused exec test: bank-0 `main` cross-calls a
   function in a HIGH bank (>=32) that then cross-calls common HAL, mirroring
   run→scan_pass→__bus_*, and bisect.

## Instrumentation left in build/ (gitignored)
- build/halcall/ : HAL-entry PC log + scan-period / key-seen measurement.
- build/halspine/ : per-bank execution distribution, run-entry count, tail PC
  histogram, HAL/soft_ret hit counts.
- build/pchist/ : PC-page histogram + __memset_ram entry count + max bank PC.
- build/disasm_banked.py : byte-linear banked disasm (make instruction-aware
  before trusting it mid-function).

Leading hypotheses for the next session (host-vs-chips differential — the host
`manual_examples_recovered_rom` test passes 42/42 on the SAME firmware logic, so
the divergence is in the compiled-target execution or the chips drum/TEST-pin
pacing, NOT the algorithm):
  1. The printer driver busy-waits on the `TEST` pin for the drum sector
     (hal.rs: "busy-waits on it (JCN/JTN) to step the drum"). If the compiled
     JCN-on-TEST polarity or the chips_trace `set_test_flag(spin%2==0)` pacing
     does not advance the firmware's drum state machine, it spins forever before
     the first hammer fire. chips_trace's TEST-pin note already flags the
     inverted JCN-C4 polarity; re-verify it end-to-end for the COMPILED firmware
     (the oracle capture used the original ROM, whose timing differs).
  2. A soft-call return in the print/drum path lands wrong at depth (re-check
     with the PC-trace control-flow spine as in build/trace/d3_clean.sh, now
     against the firmware image).
  3. Instrument a host-vs-chips HAL-call diff: log every `ram_port_write` /
     `rom_port_read` / `test_pin` call on both the host Printer mock and the
     chips run for example 1-1, and find the first divergence.

## Reproduce / continue
- Link: `BOARD=xl COMMON_BYTES=0x800 BANK_BYTES=0x800 BANKS=44
  bash llvm-i4004/scripts/build_image.sh <fw> <out> link_xl.ld`.
- 42-run: `COMMON_BYTES=0x800 BANK_BYTES=0x800 bash
  system/busicom_141pf/platform/scripts/run_image.sh secondary <img> <out>
  --board xl` then `compare.sh <out>`. Add `I4004_ASSERT_SRC_DCL=1` once the
  assertion is wired into chips_trace (currently only in the exec harnesses).
- PC histogram / progress: `build/pchist/run_pchist.sh <cycles>` (gitignored).
- Exec suite + host tests all green; see the firmware(bcd) and platform(42-run)
  commits.
