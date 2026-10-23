# Four target-only defects the host board model could not see

- Area: system/busicom_141pf/firmware (`driver_printer.rs`, `driver_keyboard.rs`,
  the call structure of `lib.rs`), `tests/common/mod.rs` (host board model)
- Found by: reading the target simulators' harnesses
  (`platform/scripts/chips_trace.rs`, `platform/rtl/busicom_141pf_xl.sv`) and
  the packer's call-depth rule while preparing the first flat target run
  (issue #18). The firmware had never run on a target until then.

## What broke

1. **Printer data polarity.** The driver shifted a hammer-word 1 as data line
   *low*. Both simulators feed the printer 4003 chain the raw line
   (`data_in_i(rom0_port[1])`, chips_trace `p & 2 == 2`), so a 1 must be
   *high*. Only the keyboard chain selects with a 0. Every row would have
   printed inverted.
2. **Held keys repeated.** `scan_pass` enqueued any pressed key on every scan
   pass. The simulators hold a key for 64 drum half-spins, which is many
   passes, so one press would have become a burst of identical keys.
3. **Drum timing.** The driver sampled TEST once per sector and assumed the
   drum had moved (spec M3-P1). In both simulators the drum turns with time,
   and the fired character is the one under the hammers at that instant.
4. **Call depth 7.** `main → run → handler → helper → Row::number → __n_row →
   leaf` needed seven return addresses; the 4004 has three. The packer's
   depth check would only have fired at link, and the image never linked
   because it did not fit.

## Root cause and why our tests missed it

All four lived where the host twin is not the hardware. The host board model
(`tests/common/mod.rs`) was written next to the drivers and copied their
assumptions: the printer polarity, one scan per press, one sector per TEST
sample. `cargo test` ran the 42 examples through it and passed. The host has
no hardware stack. The target could not run until the image fit in 4 KB, so
the second oracle (the simulators) had never been consulted.

## Fix and re-verification

1. The driver drives a 1 as data high. The host model's printer chain now
   takes the line as is, like the RTL; `hal.rs` documents both chains.
2. Key-down edge detection: a key is taken only on a pass after a pass with
   no key down (status 0xF0 S0). The host test now scans twice while the key
   is held and asserts one event, then scans the release.
3. On the target the printer is `__pr_row`/`__pr_print` (asm/drivers.s). It
   waits for the drum index, then for each TEST rising edge (sector start),
   shifts and fires. The Rust driver stays the host twin under M3-P1.
4. `run` is inlined into `main`. Handlers are level 1. Anything that reaches
   an asm entry or the printer is inlined into its handler. `=`/`%` call the
   shared mul/div step from the dispatcher. Measured depth: 3
   (`build/callgraph.py`; the packer asserts it at link).

Host `cargo test` stays green. The target proof is the 42-example run on chips
and on the RTL once the image fits.

## Prevention

- A host model that shares a driver's assumptions cannot test those
  assumptions. Each board-model behaviour must cite the RTL or simulator line
  it copies.
- Count the call depth from the first function that lowers, not at link
  (AGENTS.md "Whole-program size gate": the same lesson for depth).
