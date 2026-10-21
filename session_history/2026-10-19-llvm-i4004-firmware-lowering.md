# 2026-10-19 — llvm-i4004: unblock the whole firmware (43/43 lower) + i16 bitwise runtime

- Captured: 2026-10-19 (UTC); one Kiro Crew subagent session driving the
  `ai-sw-engineer:latest` container (LLVM 23.1.3 + the `ai-sw-llvm-build` volume).
- Model usage: exact per-model token totals are not exposed to the agent; not recorded.
- Branch: `system/busicom_141pf-on-4004` (off main @ 46d6176). No PR opened (the
  end-to-end acceptance test is not yet met — see the report's Status section).

## Goal
Get the Rust Busicom firmware, compiled by our toolchain into a ROM image, to
reproduce `firmware/oracle/rom_port_trace.txt` exactly on chips (xl board) then
the ai_hw_engineer RTL. Work to the end on one branch; one PR only when done.

## Work done (all committed + pushed; every llc/llvm-mc wrapped in `timeout`)
1. **Reverted the hanging cherry-pick 7f191be.** Its DAG-level i16 AND/OR/XOR
   split re-entered the i16 shift lowering and `llc` looped forever (the multi-
   hour "hangs" killed by the parent). i16 bitwise now lowers to `__{and,or,xor}hi3`.
   Regression guard `tests/codegen/bitwise_noloop.ll`; `gen_worklist.py` now caps
   each llc with `LLC_TIMEOUT` so a loop is a recorded failure, never a wait.
2. **Fixed the `$d2` MachineVerifier abort.** A callee returning `{ i16, ptr }`
   puts its 2nd word in D2 = P4:P5; P4/P5 were callee-saved and the CALL pseudo
   did not define them. Made D1,D2 both caller-saved i16 arg/return pairs (only
   P6 callee-saved); updated CSR, getCalleeSavedRegs, CALL Defs, abi.md §4.3.
3. **Implemented analyzeBranch/insertBranch/removeBranch** on I4004InstrInfo
   (the BranchFolder aborted tail-merging). Models the unconditional BR→JUN.
4. **Narrowed firmware `bcd::add_small` v: u32 → u16** (callers pass < 100),
   removing the i32 `__udivsi3`/`__mulsi3` libcalls that overran the register
   file. Host tests stay 42/42 exact.
5. **Added the i16 bitwise runtime helpers** `__andhi3`/`__orhi3`/`__xorhi3`
   (`llvm-i4004/rt/logic16.s`, bit-serial per byte) and extended the chips
   execution-test harness (`gen_rt_test.py --wide`, `run_rt_tests.sh`).

## Validation
- `gen_worklist.py`: firmware per-function lowering **39/43 → 43/43** select
  cleanly, 0 fail, 0 hang.
- lit: **26/26** (16 MC + 10 CodeGen, incl. bitwise_noloop).
- libi4004rt execution tests on external/chips: **867/867** pass (was 624),
  incl. 243 new i16 bitwise cases.
- Firmware host tests: **42/42 exact** after the u16 narrowing (`cargo test`:
  12 unit + manual_examples + port_level + port_trace).

## NOT done (next session resumes from report/final_report.md "Status")
- i16 shift/mul/udiv hi3 helpers (`__ashlhi3 __ashrhi3 __lshrhi3 __mulhi3
  __udivhi3`); the memset/memcpy vs __memset_ram/__memcpy_ram name+unit shim.
- `llvm-i4004/scripts/build_image.sh` (still the exit-2 stub) chaining
  llc→llvm-mc→i4004_pack.py --board xl.
- Link the whole image, run the 42 examples on chips (xl) then the RTL, compare
  vs the oracle, and report image size vs 1280/4096 B and RAM use.
