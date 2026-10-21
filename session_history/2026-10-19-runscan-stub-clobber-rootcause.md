# 2026-10-19 — run→scan_pass stall ROOT-CAUSED by chips PC-trace: the per-page `jin p0` stub (STUB_OFF=0xFC) clobbers multi-page function code

Branch `wip/dcl-frames`. Continues `2026-10-19-firmware-fits-runs-42run-noprint.md`
(firmware links+runs, 0/42, prints nothing; prior agent concluded the
`run`(bank32)→`scan_pass`(bank33) cross-bank far-call "never lands" but did NOT
trace it — it lacked a disassembler).

Model-call evidence: one KiroCrew subagent session (model Auto/server-selected);
Kiro exposes no per-model token totals. Many `docker run`/`docker exec` against
`ai-sw-engineer:latest` with the warm `ai-sw-llvm-build` volume, mounted at `/ws`.

## Traced it (not theorised) — the brief's step 1

Built an instruction-level PC trace harness on the unmodified `external/chips`
model (`build/dcl/trace_main.rs` + `trace.sh`): a `log::Log` hook captures
`CPU::print()`'s per-cycle `PC/Acc/Carry/Test` + `R0..R15` debug lines and the
Board's `Opcode:` line, tags each record with the live 8-bit bank latch, and
symbolises every PC against the image `.map`. Ran example 1-1, gated recording
to open at the first bank-32 (`run`) entry.

**Finding (PROVEN):** `run` NEVER reaches its `scan_pass` call site (run+0x255 =
PC 0xa55), never reaches the far-thunk (0xd63), never reaches `__soft_ret`
(0x700), and the latch NEVER leaves bank 32 across 8M cycles. The prior "far
call never lands" theory is WRONG — execution never gets that far.

The trace's first-seen skeleton ends at:
```
@2542 bank32 PC=8fb run+251 op=e9  (rdm)
@2543 bank32 PC=8fc run+252 op=31  <-- 0x31 = JIN P0  (the per-page stub byte!)
@2544 bank32 PC=801 run+1          <-- jin p0 jumped to (page8):(R0R1) = 0x801
```
`run` executes linearly to PC 0x8fc, hits a stray `jin p0`, and jumps back to
run+1 — an infinite self-loop in run's prologue (churning `__memset_ram`), so
banks 33-42 run 0 instructions, 0 `__bus_*` HAL calls, nothing prints → 0/42.

## Root cause (confirmed against the image bytes)

Object `run+0xfc` is `61` (`inc r1`); the IMAGE byte at run+0xfc (bank32 PC
0x8fc) is `31` (`jin p0`). The packer's `_write_softcall` writes the per-page
`jin p0` return stub at STUB_OFF=0xFC on **every** ROM page — including pages a
MULTI-page banked function occupies — **clobbering that function's real
instruction** at offset 0xFC and making it fall through into `jin p0`. `run`
spans pages 8–d; 0x8fc/0x9fc/0xafc/0xbfc/0xcfc/0xdfc are all `0x31`.

`i4004_pack.py::_stub_pad` already reserved 0xFC for SINGLE-page sections but
explicitly returned 0 (did nothing) for multi-page functions, on the false
assumption that "a multi-page function's own control flow does not fall into an
interior 0xFC". The trace disproves that: run's straight-line register-reload
sequence runs right through 0x8fc.

ABI/design context: the soft-return dispatch (`abi.md` §4.4.1/§4.4.2,
`memory_map.md` "ROM bank switching") REQUIRES a `jin p0` at a fixed per-page
offset so `__srtab[page]`→`jun 0x{page}FC`→`jin p0` can reconstruct a return's
low byte while keeping the page. So the stub genuinely must exist on every
RETURN-TARGET page — and code must be kept OFF 0xFC there.

## Fix implemented (`llvm-i4004/asm/i4004_pack.py`)

`_reserve_page_stub(obj, sec, base, stub_pages)`: walks a banked section's
instructions; on a return-target page (`_compute_stub_pages()` = the set of
pages that are `.Lret` targets, derived from the R_I4004_PAGE4 relocs) whose
code would occupy 0xFC, splices an unconditional `JUN`+NOP bridge at the latest
instruction boundary ≤0xFA so 0xFC..0xFF become a clean gap the stub owns and
control jumps over it. Idempotent (skips an already-bridged page). Runs inside
the `_place_rom_banked` re-pack fix-point so growth re-packs; `_write_softcall`
now writes the stub ONLY on `self.stub_pages`. `_insn_len` added (4004 1/2-byte
opcode lengths). Regression: `run_softcall_exec`, `run_widebank_exec`,
`run_softcall_banked_exec` all still PASS.

## OPEN — capacity interaction blocks the full firmware build

The bridge inserts ~6 bytes per bridged page, shifting downstream code. That
shift moves some page-local JCN/ISZ (PAGE8) branch targets across a page
boundary, and `_page_align_section` then **pads** each offending instruction to
the next page start. MEASURED (PADBG) on `mul_div_key`: three pads of **249,
251, 59** bytes = +559 — the pad is `PAGE - (op_off % PAGE)`, so a branch whose
opcode sits ~7 bytes into a page is pushed a whole ~249 bytes to the next page.
That is the entire overflow: raw 1790 + 7 bridges (~42) = 1832 FITS; the +559 of
page-align padding pushes it to 2391 > the 2048-byte bank (common 0x800 + bank
0x800 = the 4096-byte 12-bit-PC window). Same story for `sector_masks` /
`shift_hammer_word`. The reservation itself is correct (exec suite PASS; image
byte-inspected on configs that link).

**The real lever is `_page_align_section`'s response, not the bridge.** It
*pads* a now-page-crossing page-local branch by up to ~255 bytes; it should
instead **RELAX** it (the backend's own scheme): `JCN cc,tgt` → `JCN !cc,.+4 ;
JUN tgt` (+2 bytes, page-independent), and the ISZ analogue `ISZ Rn,tgt` →
`ISZ Rn,.+4 ; JUN tgt`. With +2 instead of +249, every function fits.

## Next session — make it fit, then run the 42

Pick one (first is cheapest/most correct):
1. **Relax, don't pad**: teach `_page_align_section` to emit the inverted-JCN +
   JUN (and ISZ + JUN) relaxation for a page-crossing page-local branch instead
   of the up-to-255-byte NOP pad. Needs the 4004 JCN condition-nibble invert
   (flip the C/not bits) and the ISZ form. This removes the amplification
   entirely; the stub reservation then costs only ~6 B/return-page.
2. **Compiler-side reservation**: have the backend/`I4004BranchRelax` emit a
   1-byte hole (or a JUN bridge) at each page's 0xFC so the packer never has to
   shift code — the cleanest long-term fix (no page-align fight at all).
3. **Split the 3 functions** in the firmware (`#[inline(never)]`, as the M-series
   frame-shrink did) so each sub-function + its bridges stays < 2048.

Then: `run_image.sh secondary build/fwimg/firmware.bin out --board xl` +
`compare.sh` with `I4004_ASSERT_SRC_DCL=1`, report N/42 and the first
divergence. Expect `run` to now reach 0xa55 → the far-thunk → `__soft_ret` →
bank 33 `scan_pass` → the first `__bus_*` HAL call (keyboard scan), which the
trace can re-confirm with `build/dcl/trace.sh` (COMPACT=1 milestone report).

## Reproduce
- Build: `BOARD=xl BANKS=44 COMMON_BYTES=0x800 BANK_BYTES=0x800 bash
  llvm-i4004/scripts/build_image.sh system/busicom_141pf/firmware build/fwimg
  system/busicom_141pf/platform/link_xl.ld` (currently fails at mul_div_key).
- Trace (the diagnostic that cracked this): `EX=1-1 MAXCYC=8000000 COMPACT=1
  bash build/dcl/trace.sh` → `build/dcl/trace/trace.sym` (FIRST-SEEN table +
  MILESTONES: scan_pass call / far-thunk / __soft_ret / bank33). Gitignored
  under build/dcl/.
- Confirm the clobber on a linked image: `python3 build/dcl/confirm_stub.py`.
