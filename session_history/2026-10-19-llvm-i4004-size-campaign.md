# 2026-10-19 (cont.) — llvm-i4004 size campaign: backend floor ~41 KB, next is firmware co-design

- Captured: 2026-10-19 (UTC); Kiro Crew subagent, ai-sw-engineer container +
  ai-sw-llvm-build volume. Branch `system/busicom_141pf-on-4004`. No PR.
- Goal: shrink the compiled firmware from 45,714 B to <= 4,096 B so it fits the 4004.

## Done this run (all committed + pushed; lit 26/26, host 42/42 exact throughout)
1. Profiled the image (`llvm-i4004/scripts/size_profile.py`): `fim` was 33%
   (14,996 B), ~5,400 `fim;src` pointer setups — address materialization was the
   dominant cost.
2. `I4004Peephole` (new post-expansion pass): drop redundant `FIM pN,imm` /
   `SRC pN`. Caught a real bug first (INC/ISZ modelled use-only). 45,714 -> 45,320.
3. SPILL8/16 + RELOAD8/16: materialize the slot address ONCE and INC across the
   consecutive chars instead of a fresh FIM per nibble. **45,320 -> 41,424 B
   (-8.6%)** — the biggest single win.

## Analytical conclusion (recorded in report/final_report.md)
After the FIM fixes, `fim` fell 7,301 -> 3,405 and the residual is mostly
`MOVi8/16` VALUE materialization, not addresses. The image is now dominated by
GENUINE per-nibble memory access: 3,716 `src rdm xch` + 2,502 `src ld wrm` +
4,427 `inc;src`. These cannot shrink per-access at the backend. **~41 KB is the
codegen floor; the remaining 10x must come from reducing the NUMBER of accesses
— a firmware-shape problem, not a backend peephole problem.**

Note: changing `Num` to one-digit-per-BYTE does NOT help — a u8 is stored as 2
RAM chars regardless, so it doubles footprint. The real fix is one digit per
NIBBLE via native-register primitives (Tier A).

## Next step (Tier A, the biggest lever) — start here
Store each `Num`/digit register as a handle to ONE 4002 register (16 nibbles,
one digit per nibble, as the original ROM does) and move the hot digit loops
(`add_mag`, `sub_mag`, `cmp_mag`, `shl`, `shr`, `inc_mag`) into libi4004rt block
primitives (`reg_add_digits`/`reg_sub_digits`/`reg_cmp`/`reg_shift`/`reg_copy`/
`reg_clear`) — each is one SRC + a tight RDM/WRM/INC loop. Keep the arithmetic
logic in Rust; a plain-Rust host impl of the same `extern "C"` API keeps host
tests 42/42 exact. Then Tier B: split `target::run`/`divide`/`sqrt_at` into
`#[inline(never)]` pieces sharing one loop core and drop the `[u8; WIDE=34]`
stack scratch for register-backed scratch. A MachineOutliner would help the
repeated nibble chains but MUST use the software return stack (JMS outlining
overflows the 3-level hardware stack).

## Validation
lit 26/26; firmware host `cargo test` 42/42 exact; whole firmware compiles to a
relocatable object (43/43). Size 41,424 B (from 45,714).
