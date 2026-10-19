# 2026-10-19 (cont.) — llvm-i4004: runtime complete, pipeline wired, size blocker found

- Captured: 2026-10-19 (UTC); Kiro Crew subagent, `ai-sw-engineer:latest`
  container + `ai-sw-llvm-build` volume. Branch `system/busicom_141pf-on-4004`.
  No PR (acceptance test not reachable — see the finding).
- Model usage: exact per-model token totals not exposed; not recorded.

## Work done (committed + pushed; every llc/llvm-mc/chips call timeout-wrapped)
1. **i16 shift + mul/div runtime** (`rt/shift16.s`, `rt/arith16.s`):
   `__ashlhi3 __lshrhi3 __ashrhi3 __mulhi3 __udivhi3 __umodhi3`. `__udivmod16`
   saves its loop counter at SSP with ACC+P7 only (an earlier `__pop_byte`
   restore clobbered P1 = the remainder; caught by the chips test).
2. **mem intrinsics -> RAM helpers**: implemented
   `EmitTargetCodeForMemcpy/Memmove/Memset` in `I4004SelectionDAGInfo` so
   `llvm.mem*` call `__memcpy_ram/__memset_ram/__memmove_ram` with SRC address
   bytes in P1/P2 and a NIBBLE count (= 2 x bytes). Upgraded `rt/mem.s` to
   advance a full 8-bit SRC address so copies/fills cross 4002 register/chip
   boundaries. Harness gained `gen_mem_test.py` + `--wide` i16 cases.
   **chips execution tests: 1351/1351 pass.**
3. **Full branch analysis** on `I4004InstrInfo`
   (analyzeBranch/insertBranch/removeBranch/reverseBranchCondition) modelling
   the conditional BRCC pseudos. Fixes 39 "Undefined temporary symbol .LBB*"
   object-emission failures: MachineBlockPlacement had been deleting blocks
   still targeted by a JCN. The whole firmware now compiles to an object.
4. **`build_image.sh`** implemented (was the exit-2 stub): firmware IR ->
   `llc -function-sections -filetype=obj` -> `llvm-mc` crt0 + rt + hal ->
   `i4004_pack.py` with the board linker script. Runs end to end to the link.

## HEADLINE FINDING — the compiled firmware does NOT fit the 4004 (~11x over)
At link, the image cannot be built: total firmware `.text` = **45,714 bytes**
(43 functions) vs the 4004's **4096-byte** ROM maximum. Largest function
`target::run` = 8,570 B (alone > 2x the whole address space); `divide` 5,138;
`sqrt_at` 4,283. Separately, every function > 256 B needs far-conditional-branch
relaxation (JCN/ISZ/FIN are page-local). A generic SSA backend lowering no_std
Rust emits ~10x the original 1,280-byte hand-written ROM.

The acceptance test (toolchain-compiled firmware runs on the 4004, matches the
tape) is **not reachable by finishing the backend/linker** — it needs a decision:
(a) a far more compact code model (not credible for 11x), (b) a 4004-sized
firmware rewrite, or (c) accept component-level validation (43/43 lower, runtime
1351/1351 on chips) as the Rust->4004 result. Documented in
`failure_notes/2026-10-19-busicom-firmware-too-big-for-4004.md` and the report
Status. 11 firmware functions are <=256 B and could be run individually on chips
to demonstrate (c) when a decision is made.

## Validation this session
- chips rt execution: **1351/1351**; lit **26/26**; firmware host **42/42 exact**
  (unchanged); whole firmware compiles to a relocatable object (43/43).
