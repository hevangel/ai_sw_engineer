# The chips 4002 latches SRC on every chip, ignoring the CM-RAM (DCL) bank line

- Date: 2026-10-19
- Area: simulator (`external/chips` MCS-4 model) vs platform RTL (`../ai_hw_engineer/design/intel_4002`)
- Found by: a second oracle — the ai_hw_engineer RTL 4002 — while bringing up
  DCL-banked per-function frames (frames in CM-RAM banks 1-3).

## What broke

With spill/reload frames moved into CM-RAM banks 1-3 (DCL frame scheme), a deep
call chain that keeps a value live across a call corrupts the soft-call return
dispatch on the `chips` model (functions re-enter unboundedly). The byte-
identical program with frames in bank 0 runs correctly, and depth <= 2 runs
correctly even banked. The isolated DCL spill/reload/deref primitives all pass.

## Root cause — two emulators disagree with the datasheet

A 4002's SRC select code must be latched **only when that chip's CM-RAM bank
line is asserted** (MCS-4 datasheet; the CM-RAM line is driven by the last DCL).
An off-bank 4002 must ignore an SRC meant for another bank and keep its own
address register.

- **ai_hw_engineer RTL** (`design/intel_4002/src/intel_4002.sv`) is correct:
  the SRC load is guarded `if (is_src && cm_ram_i && phase == PhX2/X3)`, and
  `selected = cm_ram_i && (addr[7:6] == my_chip)`. CM-RAM gates both the latch
  and the response.
- **`external/chips`** (`src/mcs4/ram4002.rs::set_register_control`) is wrong:
  it sets `active = (hi >> 2) == page_mask` purely from the SRC ADDRESS, with no
  CM-RAM input, and `mcs4/mod.rs::run_cycle` calls `set_register_control` on
  EVERY ram chip unconditionally on an `ExecuteOut::SRC`. So a bank-1 frame SRC
  also rewrites the `active` / `register_index` / `character_index` latch of the
  same-chip-select bank-0 chip. (WRM/RDM themselves ARE DCL-gated by
  `control_lines.ram(i/4)`, so stored data is not cross-written between banks;
  the damage is to the per-chip ADDRESS LATCH.) A bank-0 RAM access that reuses
  a latch set before the frame access — rather than re-SRC'ing — then reads or
  writes the wrong character. The soft-call return token off.lo observed flip
  from 0x25 to 0x05 (a cleared 0x20 bit) and the dispatch jumped to a wrong
  return site.

Our tests missed it because every prior firmware build kept all addressable RAM
in bank 0 (DCL never left 0), so the CM-RAM-gating of the SRC latch was never
exercised — the deviation is invisible until a second bank is used.

`platform/scripts/chips_trace.rs` (the 42-example harness) builds its RAM via
`Board::new(_, ram_count)` and inherits the same model; it additionally cannot
even instantiate > 4 RAM chips (`Board::new` calls `u2::new(page)`, which panics
for page > 3), so a banked-frame firmware needs the 16-chip RAM built by hand
(as `tests/exec/run_ramdcl_exec.sh` does) regardless.

## Fix and re-verification

`external/` is read-only (pinned submodule), so the emulator is not patched in
the tree; the deviation is recorded as this finding. Status of the toolchain
fix (still OPEN):

Experiment 1 — minimal Board-level reproducer (`$KIROCREW_SCRATCH/build/alias.sh`):
a bank-1 frame SRC leaves the bank-0 stack chip's address LATCH clobbered but
does NOT cross-write its STORAGE (WRM/RDM are DCL-gated in `run_cycle`). So the
damage is to the per-chip address register, surfacing when a bank-0 op reuses a
latch across a frame access without re-SRC.

Experiment 2 — patched a COPY of chips to CM-RAM-gate the SRC latch
(`build/patch_chips.py`: track the DCL bank on the Board, only
`set_register_control` on chips whose bank line is active). Against that
"datasheet-ish" model: the depth-5 g chain starts RETURNING (char9 sentinel now
lands) and `rac` (depth-2 reload-after-call) stays green, BUT the deep value is
still wrong AND a previously-green case (`big-chain`, pass-x depth-5) REGRESSES
(d2 -> a2). So gating only the SRC LATCH is not a faithful RTL emulation: the
RTL evaluates `selected = cm_ram_i && addr[7:6]==my_chip` at I/O TIME using the
chip's CURRENT cm_ram and its latched addr, whereas the chips model decides
`active` at SRC time and never re-checks cm_ram. A faithful fix must move the
cm_ram check to the WRM/RDM instant (or rewrite the RAM path in our own
`chips_trace.rs`, which is OUR code — `external/chips` cannot be edited).

CONCLUSION: the banked-frame path is correct on real hardware (the RTL gates
correctly) but cannot be trusted on the current `chips` convenience model, and a
naive emulator patch regresses other cases. The remaining work is one of:
(a) a faithful datasheet RAM model in `platform/scripts/chips_trace.rs` used for
the exec suite + the 42-run (it already owns the ROM-bank swap; add correct
4-bank RAM with I/O-time cm_ram selection, replacing `Board`'s RAM path); or
(b) bring the RTL 4002 up as the exec oracle for the banked-frame tests.

## Prevention

- Treat the RTL 4002 as the authoritative RAM oracle for any multi-bank
  (DCL) behaviour; the `chips` crate is a convenience model that does not
  CM-RAM-gate the SRC latch.
- Any CM-RAM-bank feature must be execution-tested with >1 bank in use before
  it is trusted (bank 0 alone cannot exercise DCL gating).
