# llvm-i4004 final report

## STATUS (2026-10-21) — WRAP-UP: the 42-example target run is signed off as architecturally out of reach; every verified milestone is delivered

Final branch state: `wip/dcl-frames` (superset of `system/busicom_141pf-on-4004`),
merged to `main` 2026-10-21. Sign-off log:
`session_history/2026-10-21-option-a-trace-and-option-b-scope-signoff.md`.

### What works (all execution-verified)

| Component | Evidence |
|---|---|
| Busicom 141-PF firmware (host) | **42/42** historical manual examples byte-for-byte vs the original-ROM port trace (`tests/port_trace.rs`); port-level 42/42; 12/12 unit; fmt/clippy clean |
| llvm-i4004 toolchain | complete custom LLVM target: MC layer (encode/disassemble/relocate), CodeGen lowering **43/43** firmware functions, lit suites **28/28** |
| libi4004rt runtime | **1,351/1,351** chips execution tests (i8/i16 arithmetic, shifts, bitwise, memory) |
| Soft-call convention | software return stack (8-bit SSP) beats the 3-level hardware stack; flat and cross-bank forms (bank nibble in the return token, ROM-bank latch, `__tobank` stubs) execution-verified on chips — depth-8 chains, i16 returns, latch toggling forward and back |
| Banking | whole firmware links into a banked ROM (25 banks at item 5; **BANKS=48, 100,352 B** after branch relaxation); DCL-banked per-function RAM frames (CM-RAM banks 1–3) green; the banked-frame firmware executes end-to-end |
| Packer (`i4004_pack.py`) | per-page `jin p0` stubs, `__soft_ret`, `__srtab`, far-thunks, and compact page-crossing branch relaxation (4-byte JCN / 6-byte ISZ inverted forms; replaced NOP padding, −559 B in `mul_div_key` alone) |

### What does not work — the honest limit

**The full 42-example run on the simulated target never printed.** The 48-bank
image executes end-to-end but produces 0/42 printed rows. Three root causes
were peeled off in turn:

1. the per-page `jin p0` stub clobbering multi-page function code — **fixed**
   (`bc2f208`);
2. pathological NOP padding on page-crossing branches — **fixed** by the
   relaxation above (`2cf2be9`);
3. **confirmed final blocker**: `external/chips`' 4002 model latches SRC address
   registers on every chip, ignoring the CM-RAM (DCL) bank line
   (`failure_notes/2026-10-19-chips-4002-src-latch-ignores-dcl.md`). The
   DCL-banked frames (banks 1–3) clobber the bank-0 SSP stack chip's address
   latch, so `__soft_ret` pops corrupted tokens: the `run→scan_pass` cross-bank
   call returns to `main`, and banks 33–42 execute 0 instructions / 0 HAL
   calls. The authoritative ai_hw_engineer RTL gates CM-RAM correctly, but its
   board caps at 16 × 4001 ROM = 4 KB and architecturally cannot load the
   48-bank image, so it cannot arbitrate either.

**Underlying architectural limit**: the compiled firmware is ~48 KB — about
**11× the 4004's 4,096-byte ROM ceiling**
(`failure_notes/2026-10-19-busicom-firmware-too-big-for-4004.md`). The banking
machinery (far-calls, soft return stack, DCL frames, banked stubs, ROM bank
latch) was built and proven piece by piece, but the resulting 48-bank ROM +
4-bank RAM paging system exceeds what the MCS-4 architecture and the available
simulators can carry. Option B (user sign-off, 2026-10-21) fixes the scope
boundary here.

**Open items, recorded not resolved**:

- chips 4002 SRC-latch deviation: a faithful fix needs the CM-RAM check at
  I/O time — either rewrite the RAM path in `chips_trace.rs` (option a) or use
  the RTL 4002 as the exec oracle (option b); see the failure note.
- SSP-relative per-call frames (fixes the shared spill-slot clobber under
  recursion): diagnosed, proven on a `sumdown` test, **reverted** — the
  firmware call graph is acyclic, so it is a latent risk only.
- the unverified i16-bitwise-as-two-qi3-calls experiment from
  `llvm/i4004-codegen-wip`: superseded by the verified hi3-helper
  implementation; preserved in `main` history via an `-s ours` merge, content
  intentionally not applied.

### Sign-off

Delivered and verified: host firmware 42/42 exact; complete 4004 LLVM
toolchain; libi4004rt 1,351/1,351; banking and soft-call mechanisms proven in
isolation on chips; 4 failure-note post-mortems and 25 session histories. The
42-example on-target run is blocked by target architecture and simulator
fidelity, not by an unresolved compiler defect.

---

## STATUS (2026-10-19): soft-call convention done & green; whole-firmware relink BLOCKED by 2 oversized functions

The software-return-stack call convention is implemented end to end and
**execution-verified on external/chips**, including the 8-bit SSP pointer that
lifts the depth ceiling to the firmware's real 6+ depth:

1. **MC** — `%lo8(sym)`/`%page(sym)` specifiers → `fixup_lo8`/`fixup_page4` →
   `R_I4004_LO8`/`R_I4004_PAGE4`, resolved by the AsmBackend and the packer.
2. **Packer** (`i4004_pack.py --softcall`) — synthesizes the per-page `jin p0`
   stub (0xFC), `__soft_ret`, `__srtab`, and the 8-bit-SSP push leaf helpers
   `__push_off`/`__push_pg`; asserts the JMS leaf-depth ≤ 3 rule. Both the FLAT
   and the BANKED path synthesize the common-region return dispatch.
3. **Backend** — `I4004ExpandPseudos` lowers a compiled call to the CALLSOFT
   sequence (`fim p0,%lo8; jms __push_off; fim p0,%page; jms __push_pg; jun
   callee`, P0-only staging so i16 args in D1/D2 survive) and every compiled
   return to `jun __soft_ret`; leaf rt/HAL helpers keep `jms`.
4. **Chips execution** (`tests/exec/run_softcall_exec.sh`, GREEN) — a **depth-8**
   call chain (24 token nibbles, crossing 4002 registers via the 8-bit SSP), a
   value preserved across a soft call, and an **i16 return** through a 2-deep
   call. lit 28/28; host firmware `cargo test` 42/42. `crt0` now soft-enters
   `main`; `build_image.sh` passes `--softcall` (+ optional `BANKS`).

### Item 5 (relink whole firmware) — reached the real blocker
The whole firmware compiles, assembles, resolves every relocation, and **passes
the leaf-depth assertion** through the soft-call pipeline. FLAT size = **48,801
bytes** (`.text` = 47,347 B across 62 functions). It needs banking, and the
banked `--softcall` synthesis works (builds per-bank images with the common
return dispatch + per-page stubs). The 4004's 12-bit PC forces
**common_bytes + bank_bytes ≤ 4096**: the runtime + crt0 + HAL + soft-call page
need ≈ 1,390 B of common code, so with `common 0x800 + bank 0x800` the bank
window is 2,048 B. Four compiled functions originally exceeded a window that
size; they were split `#[inline(never)]` (host `cargo test` stays 42/42 exact):
`bcd::Num::divide` 2,401→1,939 (MSD scan folded into `assemble_quotient`),
`Calculator::do_mul_div_equals` 2,380→under (extracted `compute_mul_div`),
`driver_printer sector_masks` 2,202→under (extracted `numeric_masks`),
`Calculator::dispatch` 2,184→under (extracted `dispatch_key`). Now 0 of 64
functions exceed 2,048 B (largest 1,958).

### Item 5 — DONE: the whole firmware relinks into a banked ROM image
`build_image.sh --board xl` with `BANKS=25 COMMON_BYTES=0x800 BANK_BYTES=0x800`
links the whole firmware + runtime + HAL + soft-call dispatch:
- **image size 53,248 bytes = common 0x800 (2,048) + 25 × 0x800 (2,048)**;
- **25 banks** (the minimum; `.text` 47,567 B bin-packed into 2,048-byte windows);
- **per-bank fill ≈ 2,040–2,048 / 2,048** (banks ~99 % full).
The soft-call return dispatch lives once in the common region (mapped in every
bank); every page of every bank carries the `jin p0` stub.

### Item 6 — cross-bank soft-call FIXED & green on chips; 42-run blocked on common budget
The cross-bank soft-call forward switch is implemented and EXECUTION-VERIFIED:
1. **Token + latch (done, verified).** The return token is 4 nibbles (off.lo,
   off.hi, page, **bank**); `__push_pg` pushes the caller's bank from the
   `__cur_bank` RAM mirror (SRC 0x0F). `__soft_ret` pops the bank nibble and
   writes it to the ROM bank latch (4001 chip-3 port, SRC 0x30, WRR) and
   `__cur_bank`, restoring the caller's bank before dispatch.
2. **`__tobank` forward stubs (done, verified).** The packer synthesizes one
   common set-latch stub per distinct cross-bank callee, keyed by
   **(target_addr, callee_bank)** — a banked PC (e.g. 0x800) exists in every
   bank, so keying by address alone collided — and rewrites every cross-bank
   `jun` (including a COMMON caller's, e.g. crt0's `jun main`) to the matching
   stub. The old `__bankcall_*` JMS trampolines are gone.
3. **Bank-reload runner (done, wired).** `platform/scripts/chips_trace.rs` and
   `tests/exec/run_softcall_banked_exec.sh` split a banked image into common +
   per-bank windows, boot bank 0, and reload the banked ROM chips on a chip-3
   latch write.

**PROVEN GREEN** (`run_softcall_banked_exec.sh`): a depth-8 chain ACROSS banks
and an **i16 return through a cross-bank call** -> `8 0 0 0 6 0 4 0`, the latch
toggling forward (to the callee bank) and back (restored from the token in
`__soft_ret`). The earlier "return runs away" symptom was three bugs, all fixed:
a sub-page test bank window omitting the return-page `jin p0` stub; a COMMON
caller -> banked callee not detected as cross-bank (no latch set); and
address-only stub keying colliding across banks. lit 28/28; flat soft-call exec
green; host firmware `cargo test` 42/42.

**Blocker to the 42-example run — common-region stub budget.** The whole
firmware relinks banked, but the ~64 distinct cross-bank `__tobank` stubs
(~768 B) + the runtime/crt0/HAL (~1,390 B) + the soft-call dispatch page do not
fit the common region within the 12-bit-PC ceiling (common + bank ≤ 4096; a
2,048 B common leaves only 2,048 for the bank window, which the largest function
fills). Resolving it needs EITHER a more compact banking scheme (shrink the
per-callee stub, or share a set-latch helper) OR splitting ~8 functions below a
~1,536 B window (common 0xA00 + bank 0x600). The mechanism and the runner are
ready; this is the remaining resource-packing step before the 42-example
`run_image.sh secondary --board xl` + `compare.sh`.

### Item 4 — recursion/large-frame value-spill: diagnosed, fix reverted
The backend spills frame slots to a FIXED RAM region (chip0/reg3 @ 0x30) shared
by every function, so a caller's spilled value is clobbered by a callee that
spills (and by recursion). An SSP-relative per-call frame (prologue reserves
`2*frameSize` chars on the soft stack, spills address SSP-relative) FIXES
recursion (`sumdown(5)=15` verified on chips) but (a) bloated `divide` past the
2,048 B bank window and (b) had an edge-case bug when a spill byte straddled a
4002 register boundary. It was REVERTED because the firmware call graph is
**acyclic** (no recursion) — so the fix is not needed for the 42 examples,
though the shared-slot clobber on deep non-recursive paths is a latent risk the
42-run will expose if present. The proper fix is the compact SSP-relative frame.
See `session_history/2026-10-19-softcall-items-1-4-green.md`.

### (historical) chips_trace.rs — still to wire
The 53 KB banked image + 42-example run needs the same latch-reload wired into
`platform/scripts/chips_trace.rs`'s cycle loop (pattern in
`run_softcall_banked_exec.sh` / `rt/tests/bank_test.rs`); blocked behind the
return-dispatch bug above.

A recursion/large-frame value-spill bug in register allocation (`sumdown`
returns a wrong value) is also open — independent of the soft-call mechanism.
See `session_history/2026-10-19-softcall-items-1-4-green.md`.


## TOP FINDING (2026-10-19): the compiled firmware overflows the 3-level hardware stack — it cannot run correctly, banking or not

The 4004 has only a **3-level** hardware return stack (`JMS` pushes, `BBL`
pops). The backend lowers EVERY compiled call to `JMS` (395 `jms` in the
firmware `.s`), but the firmware's call graph is far deeper than 3:
`run → press → dispatch → add_sub → add_mag → reg_add` is already 6 deep, and
mul/div/sqrt paths go deeper. So a linked image — flat OR banked — would
**overflow the hardware return stack at runtime and corrupt the return chain.**
No compiled image can run correctly until this is fixed. This is now the #1
blocker to a running image, ahead of size/banking.

**The fix (already specified in `abi.md` §4.4, never implemented): the
software-return-stack calling convention.** A compiled call pushes a 3-nibble
12-bit return token onto the P7 software stack and `JUN`s the callee (not
`JMS`); the callee returns via a common `__soft_ret` that pops the token and
`JIN`s back. This bypasses the hardware stack, so arbitrary call depth works.
`JMS`/`BBL` stay ONLY for leaf libi4004rt/HAL helpers that keep total hardware
depth ≤ 3 (asserted in the packer). Banking folds in cleanly: a cross-bank call
is the SAME soft-stack mechanism with a bank nibble in the token + a latch write
in `__soft_ret`, which also removes the JMS-trampoline depth wall ("Wall 2").

**Key piece — the return dispatch (indirect 12-bit jump).** `JIN` sets PC[7:0]
from a pair but keeps the current page. So `__soft_ret` reaches an arbitrary
return page via a page-indexed table of `JUN`s to a one-byte `JIN P0` stub
reserved at a fixed offset on EVERY ROM page (packer-reserved). Build +
chips-test this return-dispatch core FIRST (hand-written depth-8 chain), then
wire the backend `CALL`→`CALLSOFT` lowering. This reframes the project: a
RUNNING whole-firmware image is gated on the software-call convention.

## SOFT-CALL INTEGRATION RECIPE (items 1–3) — exact, verified encodings

The whole-firmware fix for the hardware-stack overflow. 40 of 43 firmware
functions are non-leaf (contain `jms`); only 3 are leaves — so nearly every
call site and every return is affected. Build as ONE unit (LLVM rebuild in the
middle is expected). Verified byte encodings (llvm-mc): `jin p0` = `0x31` (1 B);
`jun 0xKFC` = `0x4K 0xFC`; `fim p1,imm8` = `0x22 imm8`. `__push_ret` already
landed (`rt/softcall.s`, 13 B).

**(1) Packer synthesis — reserve BEFORE placement, emit AFTER.** Add
`self.soft_calls`. In `_place_rom`/`_place_rom_banked` RESERVE a 2-byte window
at `STUB_OFF=0xFC` on every ROM page (common + every bank) so no real section
lands there (bump placement past it like the page-align pass), plus a
page-local block for `__soft_ret`+`__srtab`. After `_apply()` (and per bank)
write: `rom_image[page*256+0xFC]=0x31` (`jin p0`) per page; `__srtab[k]` =
`jun 0x{k}FC` = `0x4<k> 0xFC`; `__soft_ret` body = the PROVEN `softcall_mp.s`
sequence emitted with the packer's known `&__srtab` (no symbolic nibble math —
it was brittle). Register `__soft_ret`/`__push_ret` as linker-defined globals.
Banking: `__soft_ret` also writes the latch from the token's bank nibble; the 41
`__bankcall_*` JMS trampolines are then DELETED (cross-bank = soft call + latch)
→ dissolves banking Wall 2.

**(2) Backend lowering (needs LLVM rebuild).** `I4004ExpandPseudos::expandMI`:
replace `CALL→JMS` with CALLSOFT: make MCSymbol `.Lret` after the call; emit
`fim p1,<lo8 .Lret>; fim <pagereg>,<page .Lret>; jms __push_ret; jun <callee>;
.Lret:`. Replace `RETpseudo→BBL` with `JUN __soft_ret` for NON-LEAF functions;
leaf functions keep `BBL`. NEW MC fixups needed: `R_I4004_IMM8_LO` (code label
low 8 → FIM) and `R_I4004_IMM4_PAGE` (code label page nibble → FIM) in the
MCCodeEmitter/AsmBackend + packer `_patch` — the main new MC work.

**(3) Packer leaf-depth assertion.** Walk the JMS-reachable subgraph; assert
depth ≤ 3; fail the link otherwise.

**(4) chips tests:** depth-8 plain + cross-bank; recursion depth 5; return
values in P1/D1 (extend the `softcall_mp.s` harness).

## HEADLINE FINDING (2026-10-19): the compiled firmware is ~11x too big for the 4004

### Size-reduction campaign (in progress) — instruction-mix profile of the 45,714 B

**Size progression (whole firmware `.text`, measured after each landing):**

| Step | bytes | delta | note |
|---|---|---|---|
| baseline (43/43 lower) | 45,714 | — | |
| redundant FIM/SRC peephole | 45,320 | −394 | block-local dedupe |
| spill/reload FIM-once + INC | 41,424 | −3,896 | biggest single win so far |
| regops block primitives (add/sub/cmp/is_zero) | 40,736 | −688 | Tier-A, chips-tested 10/10 |
| **target** | **≤ 4,096** | | 4004 ROM max |

**GATE CLEARED: far-branch relaxation DONE (option a).** `I4004BranchRelax`
(after ExpandPseudos) rewrites every conditional branch page-independently:
`JCN cc,Dest -> JCN !cc,.Lcont ; JUN Dest ; .Lcont:` and `ISZ Rn,Dest -> ISZ
Rn,.Ltaken ; JUN .Lcont ; .Ltaken: JUN Dest`. build_image.sh dropped
`--pad-pages`. lit 26/26; **execution-tested on chips** with a 647-byte
3-page function whose loop back-edge and compares straddle pages
(`llvm-i4004/tests/exec/run_relax_test.sh`: result correct). Cost +1,378 B
(40,736 -> 42,114). The xl link now gets PAST the page-local error and fails
only on TOTAL SIZE: `ROM image 43,552 bytes exceeds budget 4096`. So the whole
firmware is now LINKABLE given enough ROM.

**Item 2 (Tier-3 bank-switched ROM) — scope + a key constraint.** At 43,552 B
the image needs ~11 x 4 KiB banks. Banking design: a bank latch on a spare 4001
output port; bank 0's low page(s) are COMMON (crt0, HAL glue, hot libi4004rt,
cross-bank trampolines); a cross-bank call is `JMS trampoline` in the common
page that saves the caller bank on the P7 soft stack, sets the latch, `JMS`es
the target, restores the latch, `BBL`s; the packer assigns functions to banks
(call-graph-aware) and rewrites cross-bank calls. **Constraint:** the native
4004 12-bit PC already addresses 16 x 4001 = 4096 B with NO banking, which the
unmodified `external/chips` model supports (chip_index = addr[11:8]); banking is
only needed ABOVE 4096 B. The chips model cannot be modified (no license), so
chips banking must be done in the ADAPTER (`platform/scripts/chips_trace.rs`):
intercept the latch write and reload `Board.roms[]` with the selected bank's
4 KiB. The RTL xl top adds the latch to its ROM decode. Both then memory-tested
(code in each bank, calls across banks) before the 42-example run.

NOTE: Tiers A/B aim to shrink the image toward 4096 B; if they reach it, banking
is unnecessary (16 native ROMs suffice) and the image runs directly. Banking is
the fallback to get a RUNNING image before the size work completes.

Still ~10x over on total size too; the structural levers (MachineOutliner via the
SOFTWARE stack, base-pointer addressing, more regops conversions of the heavy
mul/div/sqrt WIDE=34 scratch) continue after the gate. `size_profile.py` over the
whole firmware `.s` is re-run after each step; the original baseline mix was:

**Where the remaining ~41 KB is (post-spill-fix analysis).** After the FIM
reductions, `fim` fell from 7,301 to 3,405 (p0 5,340 -> 1,444); the residual
`fim` is mostly `MOVi8/MOVi16` VALUE materialization (loading constants into
pairs), not addresses, so it is near-minimal. The image is now dominated by
GENUINE memory-access instructions: 3,716 `src rdm xch` (i8 reads) + 2,502
`src ld wrm` (i8 writes) + 4,427 `inc;src` (walking an object one char at a
time), i.e. one SRC+RDM/WRM+XCH per nibble touched. These cannot be shrunk
per-access at the backend — each byte genuinely needs its SRC and RDM/WRM.
**So the backend addressing peepholes are largely exhausted (~41 KB floor from
codegen alone); the remaining 10x must come from reducing the NUMBER of
accesses, which is a firmware-shape problem:**
  - Tier A (biggest): store each `Num`/digit register as a handle to ONE 4002
    register and do digit work through libi4004rt block primitives
    (reg_add_digits / reg_shift / reg_cmp / reg_copy / reg_clear), so a 16-digit
    operation is one SRC + a tight RDM/WRM/INC loop instead of dozens of
    separately-addressed packed-byte loads/stores. The arithmetic logic stays in
    Rust; a plain-Rust host impl of the same API keeps host tests 42/42 exact.
  - Tier B: split `target::run` (8.5 KB), `divide`, `sqrt_at` into
    `#[inline(never)]` pieces sharing one digit-loop core; u8 counters; drop the
    `[u8; WIDE=34]` scratch buffers (huge stack traffic) for register-backed
    scratch.
  - A MachineOutliner would help the repeated nibble-chain sequences but must
    use the SOFTWARE return stack, not JMS: the firmware is already <=3 deep by
    design and JMS-outlining would overflow the 3-level hardware stack.

`size_profile.py` over the whole firmware `.s`
(36,975 instructions, 45,714 bytes):

| Mnemonic | count | ~bytes | share |
|---|---|---|---|
| `fim` (2B)  | 7,498 | 14,996 | **33%** |
| `src`       | 6,218 |  6,218 | 14% |
| `xch`       | 5,951 |  5,951 | 13% |
| `ld`        | 5,597 |  5,597 | 12% |
| `rdm`       | 3,716 |  3,716 | 8% |
| `wrm`       | 2,502 |  2,502 | 5% |
| `add`/`clc`/`sub`/`cmc` | 3,422 | 3,422 | 7% |
| `jcn`/`jun`/`jms` (2B)  | 1,241 | 2,482 | 5% |
| other       | — | ~4,800 | 11% |

Patterns: **5,401 `fim ; src` pointer setups**, 3,716 `src rdm xch` byte-reads,
2,502 `src ld wrm` byte-writes, 746 `clc ld sub` compare nibbles, 326 `clc ld
add`. **The dominant cost is pointer-address materialization**: every RAM byte
access re-emits `fim pN,<const>` (2 B) + `src` even for consecutive cells of the
same object. `fim` alone is a third of the image. The reduction tiers target
this first (peephole redundant `fim`/`src`, walk adjacent cells with `inc`,
route the load/store + i16-compare sequences through JMS helpers, MachineOutliner,
cheaper spills), then firmware code shape, then (last resort) a documented
xl ROM bank-switch.

## HEADLINE FINDING (2026-10-19): the compiled firmware is ~11x too big for the 4004

The whole firmware now compiles to a relocatable object (43/43 functions, the
mem/i16 runtime is complete and chips-tested, build_image.sh drives the full
pipeline), but it **cannot be linked into a runnable ROM image**, for a reason
no amount of packing or branch work fixes:

- Total firmware `.text` = **45,714 bytes across 43 functions**
  (`llc -function-sections -filetype=obj`), versus the 4004's **4096-byte**
  architectural ROM maximum (16 x 4001). That is **~11.2x over**.
- The single largest function, `target::run`, is **8,570 bytes** — it alone is
  more than twice the entire address space. `bcd::Num::divide` is 5,138,
  `sqrt_at` 4,283, the printer `print` 3,712, `Num::mul` 2,664. Eleven
  functions exceed 1 KB.
- Independently, the 4004's `JCN`/`ISZ`/`FIN` are **page-local** (branch only
  within the current 256-byte ROM page), so EVERY function larger than 256
  bytes also needs far-conditional-branch relaxation (`JCN !cc,$+ ; JUN far`)
  before its own internal control flow is even encodable. All 43 functions but
  the smallest exceed 256 bytes.

Conclusion: the Rust Busicom firmware, compiled through this generic i4004
backend, does not fit the Intel 4004 — not by a tunable margin but by an order
of magnitude. The acceptance test ("the toolchain-compiled firmware runs on the
4004 and reproduces the tape") is therefore **not reachable** by finishing the
backend/linker alone. It needs one of:
  (a) a dramatically more compact code model (realistically cannot recover 11x);
  (b) a substantially rewritten, 4004-sized firmware (small enough that the
      compiled image fits 4 KB) — a redesign, not a tweak, and arguably at odds
      with the "compile the existing Rust firmware" premise;
  (c) accepting the stock hand-written 1280-byte ROM as the only thing that
      fits, and treating the Rust->4004 path as validated at the
      component/function level (which it now is) rather than whole-image.

A decision on (a)/(b)/(c) is needed before more backend work is worthwhile; the
far-branch relaxation and the per-function page-split are only worth building
once there is a firmware that could fit. See
`failure_notes/2026-10-19-busicom-firmware-too-big-for-4004.md`.

Everything below is the component-level progress, which is real and green.

## Status (2026-10-19, branch `system/busicom_141pf-on-4004`): whole firmware now lowers 43/43; NOT yet linked/run

### Done this session (all committed + pushed to the branch, lit 26/26 green)
1. **Reverted the hanging cherry-pick 7f191be.** Its i16 AND/OR/XOR split
   (TRUNCATE/SRL(i16)/SHL(i16)/ZEXT sub-nodes) re-entered the i16 shift custom
   lowering and `llc` spun forever (the 4–6 h "hangs" the parent killed).
   i16 bitwise now lowers directly to `__{and,or,xor}hi3`. Regression guard:
   `llvm-i4004/tests/codegen/bitwise_noloop.ll`, and `gen_worklist.py` now wraps
   each `llc` in a `LLC_TIMEOUT` (default 30 s) so a loop is a recorded
   `hang:infinite-loop` failure, never an unbounded wait. **Every `llc`/`llvm-mc`
   invocation in this work is `timeout`-wrapped; treat exit 124 as a bug.**
2. **Fixed the `$d2` verifier abort** (`Using an undefined physical register
   $d2`). A callee returning `{ i16, ptr }` puts its 2nd word in D2 = P4:P5, but
   P4/P5 were callee-saved and the CALL pseudo did not define them. D1 and D2 are
   now both **caller-saved** i16 arg/return pairs; only **P6** stays
   callee-saved (abi.md §4.3 updated; `CSR_I4004`, `getCalleeSavedRegs`, CALL
   `Defs` updated).
3. **Implemented branch analysis** (`analyzeBranch`/`insertBranch`/`removeBranch`
   on `I4004InstrInfo`) — the BranchFolder aborted with "Target didn't implement
   insertBranch" when tail-merging. Only the unconditional `BR`(→JUN) is modelled;
   the conditional pseudos are reported unanalyzable. Fixes `Row::number`.
4. **Narrowed firmware `bcd::add_small` `v: u32` → `u16`** (both callers pass
   `< 100`; the loop only shrinks it). This removed the `__udivsi3`/`__mulsi3`
   i32 libcalls whose operands exceeded the double-pair register file
   ("ran out of registers"). **Host tests stay 42/42 exact** (`cargo test`:
   12 unit + manual_examples + port_level + port_trace all green).

**Result: `gen_worklist.py` now reports 43/43 firmware functions select cleanly
(was 39/43), 0 fail, 0 hang.** The whole module emits `.s` via `llc` and all
runtime/crt0/hal `.s` files assemble with our `llvm-mc`.

### The remaining path to the acceptance test (NOT done — next session resumes here)
The end-to-end image is **not built or run yet**. Concrete blockers, in order:

A. **`llvm-i4004/scripts/build_image.sh` is still the exit-2 stub.** It must chain
   `llc -mtriple=i4004 -filetype=obj` (firmware) + `llvm-mc` (crt0.s, the runtime
   `.s`, hal.s) → `asm/i4004_pack.py --board xl` (link_xl.ld, `--pad-pages
   --pad 4096`). All the pieces exist; only the driver is missing.

B. **libi4004rt i16 (`hi3`) helpers — DONE and execution-tested.** All the i16
   helpers the firmware calls now exist and pass on `external/chips`
   (`rt/tests/run_rt_tests.sh quick`: **1347/1347 pass**, 0 fail — qi3, i16
   bitwise, i16 shifts, i16 mul/udiv/umod, each vs a host oracle; the harness
   `gen_rt_test.py --wide` drives i16 args in D1/D2 + a 4-nibble i16 result and
   inits SSP):
   - `llvm-i4004/rt/logic16.s`: `__andhi3 __orhi3 __xorhi3` (bit-serial per byte).
   - `llvm-i4004/rt/shift16.s`: `__ashlhi3 __lshrhi3 __ashrhi3` (one-bit loop;
     n>=16 clamps to 0 / sign-fill).
   - `llvm-i4004/rt/arith16.s`: `__mulhi3` (shift-and-add, terminate on a==0),
     `__udivmod16` (16-iter restoring division; trial = compare-only SUB chain;
     R12/P6 counter saved/restored at SSP via ACC+P7 so the remainder in P0:P1
     and P6's callee-saved contract both survive), `__udivhi3`, `__umodhi3`.

   **Still TODO — the `memset`/`memcpy`/`memmove` entry points.** The firmware's
   `llvm.mem*` intrinsics lower (via RTLIB) to standard C-ABI calls the runtime
   does not provide under those names, and with a DIFFERENT ABI and count unit
   than the existing `__memset_ram`/`__memcpy_ram` (which count nibbles and take
   P1/P2/P3). The lowered ABI, confirmed by compiling tiny cases:
     - `memset`: dst = D1 (P2:P3), val = P1 (i8), len = D2 (P4:P5), len in BYTES.
     - `memcpy`: dst = D1 (P2:P3), src = D2 (P4:P5), len spilled to the outgoing-
       arg RAM slot, len in BYTES.
   A byte at RAM address C occupies TWO consecutive 4002 chars (char C = low
   nibble, C+1 = high nibble; see `I4004ExpandPseudos` LDRAMi8/STRAMi8), so N
   bytes = 2N chars and the address advances 2 chars per byte. **Key finding /
   open design point:** the SRC char index is a 4-bit nibble that `INC` wraps
   within one 4002 register (16 chars = 8 bytes), so a `memcpy`/`memset` over a
   10-byte `bcd::Num` (20 chars) CROSSES a register boundary and must advance
   the SRC register bits [5:4], not just the char nibble. The right place to
   settle this is together with the xl memory map / `build_image.sh` (A): decide
   whether each `bcd::Num`/array is placed to fit one register or the mem helper
   is made to carry the full SRC address across register/chip boundaries. Then
   write `memset`/`memcpy`/`memmove` in a new `rt/mem_rtlib.s` matching the
   lowered ABI above and `--wide`-test them on chips. (A bare `.set memset,
   __memset_ram` alias is WRONG — different ABI, different arg regs, and the
   byte-vs-nibble count.)

C. Only then: `run_image.sh secondary --board xl` over the 42 examples, diff with
   `platform/scripts/compare.sh` against `firmware/oracle/rom_port_trace.txt`,
   debug divergences against the (exact) host firmware, then `run_image.sh
   primary --board xl` on the ai_hw_engineer RTL. Then report image size vs
   1280/4096 B and RAM use; reduce if over 4096.

Nothing below this line was changed this session; it is the prior milestone-3b
report, kept for history.

---

## Status: milestone 3b — CodeGen compiles 39/43 Busicom firmware functions

`llc -mtriple=i4004` lowers LLVM IR to real MCS-4 assembly our own `llvm-mc`
accepts, following the AVR model (i8 legal in a register pair `PR8`; i16 /
pointers legal in a double-pair `DR16` = two 4004 register pairs; i16 arithmetic
expanded to i8 nibble ops). Built on LLVM 23.1's `CodeGenTargetMachineImpl`:
`I4004TargetMachine`/`Subtarget`/`RegisterInfo`/`InstrInfo`/`FrameLowering`/
`ISelLowering`/`ISelDAGToDAG`/`AsmPrinter`/`MCInstLower` + `I4004CallingConv.td`
+ a post-RA `I4004ExpandPseudos` pass. Datalayout is the authoritative `abi.md`
§1 string; the overlay registers the i4004 arch + 16-bit pointer width in
`Triple.cpp`.

### Firmware coverage (the acceptance test)

Against the real firmware IR (`system/busicom_141pf/firmware/build/
firmware_i4004.ll`, produced by the firmware agent), **39 of 43 defined
functions compile** through `llc -mtriple=i4004` (per-function via
`llvm-extract`). The remaining 2 (plus 2 non-extractable declarations) hit one
known bug: an i16 value allocated to the double-pair `D2` (= the callee-saved
`P4:P5`) trips the callee-saved **super-register** save/restore in PEI
("undefined physical register $d2"). Restricting `DR16` to `D1` alone fixes
those 2 but regresses many others to heavy spilling, so `D1,D2` is kept and the
proper DR16 callee-saved spill is a documented follow-up.

### Selection + lowering implemented

- i8 add/sub (ACC nibble chains, carry via CMC), i16 add/sub (4-nibble chain).
- i8/i16 RAM load/store (addrspace 0: `SRC`+`RDM`/`WRM`, two 4002 chars/byte);
  global addresses via `LEA_GA` (low byte `R_I4004_IMM8`, high byte 0 — globals
  in the low 256 bytes, a linker/`%hi` follow-up).
- compares + conditional branches (`BR_CC` → SUB chain + `JCN`; EQ/NE per-nibble,
  ULT/UGE via final borrow, UGT/ULE by operand swap), `SELECT_CC` → a CFG-diamond
  custom inserter (four compare×value width forms), unconditional `br` → `JUN`.
- calls → `JMS` (hardware stack), register args P1–P3 and **stack arguments**
  (RAM outgoing-arg area via frame-index stores); i16 args/returns in `D1`/`D2`.
- zext/anyext i8→i16, trunc via `EXTRACT_SUBREG`; i16 constants (`MOVi16`);
  frame-index addressing (`LEA_FI`); register spill/reload to 4002 RAM
  (`SPILL8`/`16`, `RELOAD8`/`16`).
- AND/OR/XOR and variable shifts → libi4004rt libcalls (`__and/or/xor` +
  `__ashl/__lshr/__ashr` `qi3`/`hi3`); mul/div/rem (i8/i16/i32) and
  memcpy/memset/memmove → the standard RTLIB impls registered in
  `Subtarget::initLibcallLoweringInfo` (abi.md §6 / the runtime agent's PR #15).

### Verified
- **lit/FileCheck 24/24** (16 MC + 8 CodeGen).
- **Execution on external/chips: 40/40** — `exec_fn.py` (i8 add/sub) + `exec_i16.py`
  (i16 add/sub) run `llc`-compiled code on the real `mcs4::Board` vs the host.
  The i8 subtract borrow-chain bug (found here, not by FileCheck) is fixed.
- The BCD digit-add (`isel_bcd_probe.sh`) compiles AND assembles to an ELF object
  (`bcd_add_digit` = 102 bytes).

### Sizes (honest)
No single linked firmware ROM image yet — that needs the linker / image packer
(`asm/i4004_pack.py` from the runtime agent) to resolve the page-local `JCN`/
`JUN`/`FIN` relocations and lay out 256-byte pages (milestone 5). Per-function
`.text` is measurable now (e.g. BCD digit-add 102 bytes); a whole-image size vs
the 1280-byte original / 4096-byte max waits on linking.

### Remaining blockers to a running firmware
1. The D2 callee-saved super-register spill (2 functions).
2. Linker / image packer: resolve `R_I4004_PAGE8`/`ADDR12`/`FIN8`, 256-byte page
   layout, link against libi4004rt + crt0 (milestone 5).
3. Then: run the linked image on external/chips vs the oracle tape.

---
### Historical: what compiles today (early backbone)

| Construct | Lowering | Verified |
|---|---|---|
| empty fn | — | `llc -filetype=null` accepts |
| `ret void` / `ret i8 c` | `FIM p1, c` (i8 in P1, abi.md sec.4.2) + `BBL 0` | lit + exec |
| `ret i16` | split to two i8 (P1:P2) | lit (`elf_machine`), compiles |
| `add i8` | `CLC; LD; ADD; XCH` per nibble, carry chained | **exec 10/10 on chips** |
| `sub i8` | `CLC; LD; SUB; XCH` low, `CMC`, then high nibble | **exec 10/10 on chips** |
| args (i8) | P1, P2, P3 then RAM-stack slot (abi.md sec.4.1) | lit + exec |
| pair copy | `LD/XCH` x2 (`copyPhysReg` -> `COPYpr`) | compiles, used by RA |
| unconditional `br` | `JUN <mbb>` (addr12 reloc resolves the label) | lit |
| ELF object | `-filetype=obj` carries `e_machine 0x4004` | lit |
| MC round-trip | `llc` `.s` re-assembled by independent `llvm-mc` | lit |

Multiply/divide/shift are set to libcalls (`__mulqi3`, `__udivqi3`, ... into a
future `libi4004rt`, abi.md sec.6). AND/OR/XOR are `Expand` (the 4004 has no
native bitwise ALU).

### Verification (in the ai-sw-engineer container)

- **lit/FileCheck: 22/22** — 16 MC tests + 6 CodeGen tests
  (`tests/codegen/{ret_const,add_i8,sub_i8,branch,elf_machine,mc_roundtrip}.ll`).
- **Execution on external/chips: 20/20** — `scripts/exec_fn.py` compiles
  `add8`/`sub8` with `llc`, runs the compiled MCS-4 code on chips' real
  `mcs4::Board` (`tools/chips_xcheck/run_fn`) for 10 input pairs each, and
  compares against the host. This is an EXECUTION test, not FileCheck: it caught
  and fixed a real backend bug — the i8 subtract borrow chain was wrong on every
  case (both an STC-once and a CLC-once form failed; the correct form is CLC
  then CMC between nibbles, because the 4004 SUB uses `ACC + ~R + ~CY` so the
  carry-out polarity must be flipped between nibbles).
- MC differential disassembler cross-check on chips still green (unchanged).

Both lit and the execution test are wired into `scripts/run_tests.sh`.

### Honest blockers (remaining to run the firmware)

The yardstick `scripts/isel_bcd_probe.sh` (the BCD digit-add) does **not** yet
compile. The probe advances past target lookup and `br`, then stops at:

```
LLVM ERROR: Do not know how to expand this operator's operand!
```

Isolated to two missing layers (minimal IR reproduces each):

1. **Loads/stores to 4002 RAM** (`load i8, ptr` / `store`). addrspace-0 memory
   access via `SRC` + `RDM`/`WRM` (two nibbles per byte) is not lowered yet;
   the exact AS-0 pointer bit-packing is abi.md §7 open-question 2. This is the
   gateway to any firmware code that touches memory (the BCD `a[i]` accesses).
2. **Comparisons / conditional branches** (`icmp` + `br i1`). `SETCC` to the
   4004's single `ACC==0` / `CY` test and `BR_CC` lowering are not built; the
   `BRZ`/`BRNZ` pseudos and their `JCN` expansion are in place awaiting the
   `SETCC` lowering that feeds them.

Not started beyond those: i16/i32 expansion to byte ops, the `libi4004rt`
mul/div/shift helpers, calls deeper than a leaf with the software return stack,
a linker/ROM image packer, and rustc custom-target integration (plan step 7).
So there is still **no compiled firmware image and no real BCD `.text` size**;
the original ROM is 1280 bytes (4096 max) and that comparison must wait until
loads/stores + compares land and `isel_bcd_probe.sh` emits assembly.

### Where this stops

At a verified arithmetic/control-flow/return/call-ABI backbone with lit +
on-silicon-model execution tests, and a BCD probe that pinpoints the two
remaining selection layers (memory + compares). The next session resumes by
implementing addrspace-0 load/store (SRC+RDM/WRM) and SETCC/BR_CC, re-running
`isel_bcd_probe.sh` after each, then reporting the BCD `.text` byte size.

---

# llvm-i4004 final report (milestone 3a, historical)

## Status: milestone 3a (disassembler + independent validation) complete

Milestone 2 (MC layer) is complete and milestone 3a adds the **disassembler**,
a **differential encoding cross-check that executes every encoding on an outside
emulator**, and the **R_I4004_FIN8** relocation test. `llvm-mc -triple=i4004
--disassemble` and `llvm-objdump -d` now decode all 46 instructions, and an
assemble -> disassemble -> reassemble round trip is byte-identical. Instruction
selection / codegen is the next milestone (3b/4), tracked on branch
`llvm/i4004-isel`.

### Milestone 3a results

| Check | Result |
|---|---|
| lit suite (`tests/mc/`, incl. `tests/mc/disasm/`) | **16 / 16 pass** |
| Disassembler coverage | all 46 opcodes, both 1- and 2-byte forms, all 16 JCN conditions, all 8 pairs |
| Round trip (asm -> obj -> disasm -> re-asm -> obj) | **byte-identical** across 52 instruction instances (`disasm/roundtrip.s`) |
| Differential disassembler cross-check | every encoding **executed on external/chips** and the observed effect matched the effect predicted from OUR disassembly: **no disagreements** (336 data/ALU/IO assertions + 17 control-transfer landings). See §"Differential cross-check" below. RTL half not run (no simulator here). |
| Undefined opcodes 0xFE/0xFF, 0x01-0x0F | decoder refuses (invalid-encoding warning, no mnemonic); executed on chips as a cross-check — all are NOP-equivalent **except 0xFF, which makes chips panic** (finding below) |
| `R_I4004_FIN8` relocation | now emitted and asserted via `.reloc` (`tests/mc/reloc_fin8.s`); all four `R_I4004_*` target relocs are `.reloc`-nameable |

### Disassembler design

`target/lib/Target/I4004/Disassembler/I4004Disassembler.cpp` is a **hand-written
variable-length decoder**, not a TableGen `-gen-disassembler` table: the MCS-4
stream mixes 1- and 2-byte instructions and TableGen's fixed-length decoder
assumes a single width. The decoder dispatches on the OPR high nibble, one
`case` per `isa.md` row, reads a second program byte for JCN/FIM/JUN/JMS/ISZ,
and returns `Fail` (Size=1) on a truncated two-word instruction or an undefined
opcode so a caller can resynchronise. CMake auto-registers it via
`Disassemblers.def` once the `Disassembler/` subdir exists (a clean reconfigure
is needed the first time).

### Differential cross-check — the headline validation (corrected)

> **Correction to the original milestone-3a claim.** The first version of this
> section (and `scripts/crosscheck_disasm.py` as merged in PR #8) claimed the
> disassembler "matches two independent decoders (external/chips AND the
> ai_hw_engineer RTL), zero disagreements". **That claim was overstated.** The
> old script did not execute anything and did not compare operands. It compared
> llvm-mc's *mnemonic* against `oracle_dispatch()` — a hand-written Python
> opcode table *in that same script* — and merely regex-checked that the chips
> source and the RTL source still contained a few expected tokens / localparams.
> Register numbers, pairs, JCN conditions, immediates and addresses were never
> checked, and neither chips nor the RTL was ever run. It was, in effect, our
> own table checked against itself. The text below describes what replaced it.

The real check now lives in the Rust tool `tools/chips_xcheck` (driven by
`scripts/crosscheck_disasm.py`, wired into `run_tests.sh`). For every 4004
encoding it:

1. disassembles the bytes with OUR `llvm-mc --disassemble` (mnemonic **and**
   operands);
2. predicts the full machine effect from THAT disassembly using the MCS-4
   manual semantics in `spec/isa.md`, **parameterised by the operands we
   printed** (e.g. `add r3` predicts `ACC ← ACC + R3 + CY`, `fim p5, 0x2A`
   predicts `R10 ← 2, R11 ← A`);
3. **EXECUTES the same bytes on `external/chips`** (its real `mcs4::Board`;
   read & run only, never copied) and recovers the resulting state —
   accumulator, carry, all 16 index registers, RAM/ROM port and character
   writes — through the chip's own instructions (a short readout program that
   copies hidden CPU state into 4002 RAM, which chips exposes publicly);
4. asserts prediction == observation. Control transfers (JUN/JMS/JCN/ISZ/JIN)
   are checked with a landing harness: a marker on the fall-through path and a
   marker at the branch target reveal which path chips actually took.

Because the expected value comes from EXECUTING chips, a wrong operand field or
a swapped mnemonic in our disassembler makes step 2 disagree with step 3 — a
genuine finding. This is verified by two mutation tests (below).

**Coverage.** 336 data/ALU/IO effect assertions (every defined first byte, plus
operand-field sweeps: all 16 register indices for INC/ADD/SUB/LD/XCH, all 16
immediates for LDM/BBL, all 8 FIM pairs over a spread of data bytes) and 17
control-transfer landings (JUN, JMS, the full 16-row JCN condition matrix over
ACC/CY, ISZ with zero/non-zero results, JIN). **Result: no disagreements** —
every encoding our disassembler decoded, executed on chips, behaves exactly as
our mnemonic+operands predict.

**Mutation sanity (the check can fail).** With `MUTATE=swap_acc` (our printer
emits `iac`↔`dac` swapped, i.e. two accumulator-group ops transposed) the check
reports exactly the 2 expected findings — chips increments where we predicted a
decrement and vice-versa. With `MUTATE=reg_off1` (every register operand
printed off by one) it reports 160 findings across the register sweeps. Both
revert cleanly to zero. So the green result is not a harness that cannot fail.

**What is proven vs not:**

- *Proven:* for every DEFINED encoding, our disassembler's mnemonic **and
  operands** correctly describe what the chips 4004 emulator actually does to
  the accumulator, carry, named register(s), and RAM/ROM ports. Operand
  correctness (register index, pair, JCN condition selection, immediate value,
  branch target) is executed, not assumed.
- *Not proven here:* the **ai_hw_engineer RTL half**. The brief asks to also run
  each encoding on `B:\ai_hw_engineer\design\intel_4004` and treat chips-vs-RTL
  disagreements as findings. The RTL source (`src/intel_4004.sv`) is present,
  but **no Verilog simulator is available** — neither `verilator` nor
  `iverilog` is installed in the `ai-sw-engineer` container or on the host, and
  the only prebuilt `obj_dir/` under ai_hw_engineer is for the 8273, not the
  4004, and there is no 4004 ISS or Python model. So chips is currently the only
  *executed* oracle; the RTL comparison is deferred until a simulator is wired
  into the container (tracked in `plans/testplan.md`). chips itself is validated
  upstream against MAME on the Busicom ROMs (per `isa.md` §10), so it is a
  strong single oracle, but it is one oracle, not two.

### Findings from execution (chips vs the spec)

1. **0xFF makes chips panic.** `spec/isa.md` §8 says the two undefined opcodes
   `0xFE` and `0xFF` "execute as NOP". Executed on chips: `0xFE` is a true NOP
   (its decoder has an explicit "does nothing" arm), but **`0xFF` hits an
   `unreachable!("Unknown opcode: F{:X}")` and aborts** — chips' accumulator-
   group match has no arm for modifier `0xF`. Our disassembler refuses both
   (safe). Resolution: this is a chips implementation gap, not a defect in our
   backend or a reason to change `isa.md` (the manual's "executes as NOP" is a
   documented reconstruction; chips simply never implemented 0xFF). Recorded;
   our conservative refusal is correct either way.
2. **JCN TEST-pin polarity.** For `JCN` condition `0x1` (TEST bit only, no
   invert), chips jumps when the TEST pin is **HIGH** and falls through when it
   is LOW. `isa.md` §4 describes the TEST condition as "TEST pin = 0". This is a
   chips-vs-manual semantic disagreement on the TEST line's active level. It
   does **not** affect our disassembler, which only prints the 4-bit condition
   field (verified correct for all 16 conditions) and does not interpret it;
   resolving the polarity needs the RTL as the tie-breaker, so it is left open
   and noted here rather than silently resolved.

The harness recovers hidden CPU state by appending readout instructions; it is
run twice with two different scratch-pointer register pairs so that a register
used as the readout pointer in one pass is read cleanly in the other (an
earlier single-pass version mis-reported R14/R15 as the pointer's char index —
a harness bug, caught and fixed before any result was reported).

---

## Status: milestone 2 (MC layer) complete

`llvm-mc -triple=i4004 -show-encoding` assembles **all 46 Intel 4004
instructions** to the manufacturer opcode table in `spec/isa.md`; the object
writer stamps the private `e_machine 0x4004` and emits the `R_I4004_*`
relocations from `spec/abi.md`. No instruction selection / codegen yet
(milestone 3).

| Check | Result |
|---|---|
| MC encode / error / reloc / ELF tests | **10 / 10 pass** (`tests/mc/`) |
| Instructions assembled (`-show-encoding`) | **46 / 46** opcodes, bytes match `isa.md` |
| ELF `e_machine` | `EM_I4004 (0x4004)`, `elf32-i4004`, ELFCLASS32, LSB |
| Relocations | `R_I4004_ADDR12`, `R_I4004_PAGE8`, `R_I4004_IMM8` emitted for unresolved JUN/JMS/JCN/ISZ/FIM |
| Codegen tests | n/a (milestone 3) |
| Smoke programs on simulator | n/a (milestone 3+) |
| Titles built with this backend | none yet |

### Test breakdown (`llvm-i4004/tests/mc/`)

| File | Covers |
|---|---|
| `basic.s` | the 16 `OPR=0x0..0xD` rows (NOP, JCN, FIM, SRC, FIN, JIN, JUN, JMS, INC, ISZ, ADD, SUB, LD, XCH, BBL, LDM) |
| `ram_rom.s` | the 16 `OPR=0xE` rows (`wrm`=0xE0 .. `rd3`=0xEF) |
| `accgroup.s` | the 14 defined `OPR=0xF` rows (`clb`=0xF0 .. `dcl`=0xFD) |
| `jcn_cond.s` | JCN condition mnemonics (Z/C/T/NZ/NC) + hex nibble + `jmp` alias |
| `pairs.s` | all 8 pair encodings for FIM/SRC/FIN/JIN + `rNrM` aliases |
| `reloc.s` | symbolic two-word operands -> `fixup_addr12` / `fixup_page8` |
| `elf.s` | `-filetype=obj` + `llvm-readobj`: `e_machine 0x4004` + `R_I4004_*` |
| `errors/err_imm_range.s` | `ldm 16`, `bbl 16`, `fim p0, 256` rejected |
| `errors/err_pair_vs_reg.s` | `src r3`, `add p2`, `fin r5` rejected |
| `errors/err_undefined.s` | undefined `0xFE/0xFF` have no mnemonic; bad mnemonics rejected |

## Build

- **LLVM**: `external/llvm-project` submodule pinned to **`llvmorg-23.1.3`**
  (shallow). This is the LLVM version of the repo's pinned nightly rustc
  (`rustc +nightly-2026-10-15 -vV` -> `LLVM version: 23.1.3`), verified in the
  container. The tag exists, so no fallback was needed.
- **Overlay, not in-place edits**: the 4004 target lives in
  `llvm-i4004/target/` and is applied onto a scratch copy of the submodule by
  `scripts/apply_overlay.py` (idempotent, anchored string edits to Triple /
  ELF / ELFDumper infra; the submodule itself is never modified).
- **Command** (from the repo root on the Windows host):

  ```
  docker run --rm --user root \
    -v B:\ai_sw_engineer_mc:/workspace \
    -v ai-sw-llvm-build:/build \
    -w /workspace ai-sw-engineer:latest \
    bash -lc 'chown -R mos:mos /build; su mos -c "llvm-i4004/scripts/build_llvm.sh"'
  ```

  The LLVM source copy and the build tree live in the Docker **named volume**
  `ai-sw-llvm-build` (`/build`), not on the slow Windows bind mount. Built with
  `LLVM_EXPERIMENTAL_TARGETS_TO_BUILD=I4004`, `LLVM_TARGETS_TO_BUILD=""`,
  `CMAKE_BUILD_TYPE=Release`, `LLVM_ENABLE_ASSERTIONS=ON`, Ninja, host `gcc`
  (the image's `clang` defaults to the `mos` bare-metal target), `-j16`.
- **Tools built & installed** into `llvm-i4004/install/bin` (gitignored):
  `llvm-mc`, `llvm-tblgen`, `FileCheck`, `not`, `count`, `llvm-readobj`,
  `llvm-objdump`.
- **Build time**: the first clean build (tblgen + the minimal LLVM MC/Support
  stack + the I4004 libraries + the seven tools) is roughly **12-20 min** at
  `-j16` on the 32-core host; a no-op incremental re-run (overlay + ninja
  up-to-date check) is **~38 s**. The one-time source copy from the Windows
  bind mount into the volume adds several minutes and is cached thereafter.
- **Run the tests**:

  ```
  docker run --rm -v B:\ai_sw_engineer_mc:/workspace -w /workspace \
    ai-sw-engineer:latest bash -lc 'llvm-i4004/scripts/run_tests.sh'
  ```

## What the MC layer contains

`llvm-i4004/target/lib/Target/I4004/`:

- `I4004.td`, `I4004RegisterInfo.td`, `I4004InstrFormats.td`,
  `I4004InstrInfo.td` - registers (ACC, CY, R0-R15 / GR4, P0-P7 / PR8) and one
  TableGen record per `isa.md` row with the exact `OPR:OPA` encoding.
- `MCTargetDesc/` - `I4004MCCodeEmitter` (byte emission + fixups),
  `I4004AsmBackend` (fixup kinds, relocation hand-off), `I4004ELFObjectWriter`
  (`e_machine 0x4004`, `R_I4004_*`), `I4004InstPrinter`, `I4004MCAsmInfo`,
  registration glue.
- `AsmParser/I4004AsmParser.cpp` - register / pair / JCN-condition parsing,
  operand-range validation.
- `TargetInfo/` + a minimal `I4004TargetMachine.cpp` stub so the target
  resolves as a proper LLVM component (no code generator yet).

## Known gaps

- **No instruction selection / codegen** (milestone 3): `I4004TargetMachine` is
  a stub providing only the component init hook; `llc` cannot lower IR yet.
- **Page-local cross-page checking** (`isa.md` sec.9 end-of-page quirk) is
  **not yet enforced** by the assembler for the `fixup_page8` / `fixup_fin8`
  relocations. The relocations are emitted correctly; the linker-side page
  identity check is deferred to the linker/image-packer milestone (5). An
  `err_page_cross.s` test is therefore not yet included (it would require that
  check or `ld.lld` placement to fail). Tracked here and in `plans/testplan.md`.
- **FIN pair-0 relocation** (`R_I4004_FIN8`) is defined but not yet exercised
  by a test (no `.word`/FIN constant-table assembly path until codegen needs
  it).
- **RTL half of the cross-check not executed**: the milestone-3a differential
  check (see "Differential cross-check") executes every encoding on
  `external/chips`, but the second oracle — the `ai_hw_engineer`
  `design/intel_4004` RTL — is **not run**, because no Verilog simulator
  (`verilator` / `iverilog`) is present in the container or on the host and
  there is no 4004 ISS. chips is validated upstream against MAME, so it is a
  strong single oracle, but chips-vs-RTL is still owed. Tracked in
  `plans/testplan.md`. (Note: this supersedes the milestone-2 note that no
  second oracle was available at all — `external/chips` is now checked out and
  executed.)
- **ELFDumper / Triple / ELF infra edits** are applied by the overlay script,
  not committed into the submodule. They are small and idempotent; if the
  pinned tag ever moves, re-verify the anchors in `scripts/apply_overlay.py`.

## Spec corrections / findings

- No `isa.md` / `abi.md` corrections were needed: the opcode table and
  relocation list were complete and the implemented encodings matched them
  exactly on the first passing build.
- **Finding (chips vs spec, 0xFF):** executing the undefined opcodes on chips
  shows `0xFE` is a true NOP but **`0xFF` panics** (`unreachable!("Unknown
  opcode: FF")`) — chips never implemented the `0xF?` modifier `0xF` arm, so it
  does not match `isa.md` §8's "executes as NOP". Our disassembler refuses both,
  which is safe. No backend change.
- **Finding (chips vs spec, JCN TEST polarity):** chips jumps on `JCN 0x1` when
  the TEST pin is HIGH; `isa.md` §4 phrases the TEST condition as "TEST pin =
  0". A semantic disagreement on the TEST line's active level, needing the RTL
  as tie-breaker. Does not affect the disassembler (condition field printed
  correctly for all 16 conditions).
- **Finding (toolchain):** the `ai-sw-engineer:latest` image's `/usr/local/bin/clang`
  defaults to `mos-unknown-unknown`, so building host LLVM with it fails at the
  CMake compiler check (it tries to link an MCS-6502 `crt0.o`). The backend
  build therefore uses host `gcc`/`g++`. Documented in `build_llvm.sh`.
- **Finding (Windows checkout):** git on the Windows host checks the LLVM
  submodule out with CRLF line endings, which breaks the `/bin/sh` helper
  scripts cmake executes (`config.guess`). `build_llvm.sh` strips CR from those
  scripts in the scratch copy before configuring.
- **Finding (LLVM 23.1 TableGen):** `-gen-instr-info` now requires every target
  to remap the standard `ptr_rc` pseudo-instructions onto a concrete register
  class via `RemapAllTargetPseudoPointerOperands<...>`, even for an MC-only
  target. Done with `PR8` (the pointer-capable pair class).
