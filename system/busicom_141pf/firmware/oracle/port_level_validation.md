# Port-level validation (milestone M3)

The M3 machine drivers (`src/driver_keyboard.rs`, `src/driver_printer.rs`,
`src/driver_lamps.rs`) talk to the board only through the port-level HAL
(`src/hal.rs`). The host test `tests/port_level.rs` implements that HAL with a
**board model** and replays all 42 manual examples through
`keyboard scan → Calculator → printer drum → board decode`. For that round trip
to be meaningful, the board model's port protocol must match the real machine.

## How the board model was validated against the original ROM

The authoritative port-level behaviour is the **original 1280-byte ROM** run on
V. Ilmer's MCS-4 chip models, exactly as the oracle was captured
(`oracle/README.md`). We re-ran it ourselves and scored it against the oracle:

1. `external/chips` is pinned at `1bc3d5781474ef9d7e7305af91071a723797f222`
   (the commit `ai_hw_engineer/system/busicom_141pf/scripts/check_reference.py`
   requires). No license — we run it, we do not copy it.
2. `ai_hw_engineer/scripts/reference_oracle.rs` drives
   `spec/reference/rom_141pf_combined.bin` on `chips::mcs4::Board`, wiring the
   board the same way `external/busicom` (`keyboard.rs`/`printer.rs`) does:
   - keyboard: a single marker bit walked through 4003 #0 (ROM0 bit 0 clock,
     bit 1 data, both active low); the selected row's column nibble appears on
     ROM1; rows Q8/Q9 return the decimal / rounding selectors;
   - printer: a 20-bit hammer word shifted into the cascaded 4003 #1+#2 chain
     (ROM0 bit 2 clock); hammers fire on the RAM0 bit 1 rising edge, printing
     the drum character under the hammers (`(spin/2+12) % 13`) into the selected
     columns; red = RAM0 bit 0, paper advance = RAM0 bit 3 rising edge;
   - lamps: RAM1 bits 0/1/2 = memory / overflow / negative;
   - TEST toggles every drum half-spin; the drum index pulses on ROM2 bit 0.
3. Replaying the 42 examples and comparing with `check_reference.py`'s
   `compare()` against the recovered-ROM profile gives **42 / 42 PASS**.

The captured trace is committed as `rom_port_trace.txt` (no ROM binary). The M3
drivers + board model reproduce the identical port protocol — same keyboard
shift walk, same 20-bit hammer chain and drum-sector decode, same RAM0/RAM1
semantics — and `tests/port_level.rs` passes the same 42 / 42.

## Board-model simplifications (documented, not hidden)

- **Drum pacing granularity.** The real board / RTL ticks TEST every drum
  *half-spin* (two half-spins per sector, index every 26 half-spins). The host
  board model advances one sector per `test_pin()` sample and the printer driver
  samples TEST once per sector, so the two stay in lock-step without modelling
  sub-sector analog timing (a non-goal here, matching the RTL spec §5.1). The
  reconstructed `{value, symbol, red, rounded}` row is identical either way.
- **8-key buffer.** `driver_keyboard::KeyBuffer` implements the documented
  8-word input FIFO. The replay presses one key per scan pass (the examples are
  sequential), so the buffer never overflows; the depth is exercised by its own
  unit test. The RTL virtual platform drops the buffer entirely (its §5.1
  deviation); this driver keeps it, which is closer to the real machine.

## Reproduce

```bash
# host-side git pin check (chips must be clean at the PIN above), then:
docker run --rm -v <m3>:/workspace -v <hw>:/ai_hw_engineer \
  -w /ai_hw_engineer/system/busicom_141pf ai-sw-engineer:latest bash -lc '
  python3 scripts/check_reference.py --chips /workspace/external/chips \
    --profile recovered-rom'   # → 42× "PASS <id>"
```



## Direct comparison against the original-ROM trace (`tests/port_trace.rs`)

The validation above checks the host **board model** against the original ROM
(42/42 via `check_reference.py`). It does **not** by itself re-check our Rust
drivers against the ROM: `tests/port_level.rs` compares the driver+board round
trip against `oracle/manual_examples.json` (the scanned manual, recovered-ROM
profile), whose `expect.tape` is a per-step **suffix**. `tests/port_trace.rs`
closes that gap by replaying all 42 examples through the drivers + board model
and comparing the produced port events directly against `rom_port_trace.txt`.

Both tests share ONE board model (`tests/common/mod.rs`), so neither can quietly
validate against a different model.

### What it compares (exact, no relaxations)

Grouped exactly as the trace groups them (`#RESET`, then one group per `#STEP`),
for EVERY group:

- **the full ordered row stream** equals the trace's — same row count, same
  order, and per row the same numeric value string, the same symbol-column-A
  glyph, the same symbol-column-B glyph, and the same red-ink flag (value `None`
  = dotted overflow line; an empty value is the blank operation-correction row);
- **the full 4-bit RAM1 lamp nibble** per step equals the trace's `LAMPS|n`,
  including the memory (bit 0), overflow (bit 1), minus-sign (bit 2) and
  `WR.S0.bit1` status (bit 3) bits.

There are NO value / glyph / lamp relaxations: the earlier subsequence,
glyph-subset, zero-normalisation and lamp-mask tolerances have been removed.
Cycle timing, keyboard-scan repetition, TEST-poll repetition and shift-bit
timing are not part of the trace (it records board OUTPUT events only); the
keyboard scan order is still asserted per press by `common::press_through_ports`
(it must recover the exact key + switches), and the board advances one drum
sector per `test_pin()` sample (ledger M3-P1), which reconstructs a row identical
to the half-spin-paced ROM capture.

### Property asserted

> For every one of the 42 examples, the full ordered stream of printed rows our
> drivers emit is **identical** to the original ROM's trace rows (value, both
> symbol-column glyphs, and red ink all equal), and the per-step RAM1 lamp
> nibble matches the ROM in all four bits.

This is **42/42 exact** today. Mutation checks confirm it bites: removing the
`M+`/`M-` echo fails 3-6/9-1/10-1/11-2 on row count; swapping the two symbol
columns in the printer driver fails nearly every example; clearing RAM1 bit 3
fails every overflow row (nibble 10) and negative-total row (nibble 12).

### How the ROM's full tape is now reproduced (former A6–A9 debt, closed)

The core previously emitted a thinner tape than the ROM; `spec/spec.md` ASSUMPTIONs
A6–A9 are now **reproduced**, not gated:

- **A6** recalled/chain/mixed `×`/`÷` operand echoes and the blank op-correction
  row (`Calculator::mul_div_key`), with the ROM's fresh-vs-DP operand scaling;
- **A7** the bare `M+`/`M-` key echo, DP-aligned (`Calculator::memory_add`);
- **A8** two independent symbol columns via `print::Row { sym_a, sym_b }` (the
  rounded `^` is `SymA::Rounded`): `C`+`M` for CM, `R`+`M` for RM, the `M=±`
  result in column B, the rounded mark in column A beside the result in B;
- **A9** the minus-sign lamp driven from WR's sign at idle and RAM1 bit 3
  (`WR.S0.bit1`), both sourced to the annotated listing's status-light routine
  (`8·WR.S0.bit1 + 4·WR.S0.bit0 + 2·overflow + (MR≠0)`).

The JSON-oracle tests (`manual_examples.rs`, `port_level.rs`) fold the two
symbol columns back to the manual oracle's single token (`Row::oracle_symbol`),
so the sanctioned `oracle/manual_examples.json` is unchanged and still 42/42.

One escaped defect the trace earlier caught was fixed before this work: `C`
printed a blank row instead of `0`
(`failure_notes/2026-10-18-busicom_141pf-clear-prints-zero.md`, spec A3). The
A6–A9 reproduction is written up in
`failure_notes/2026-10-18-busicom_141pf-partial-tape.md`.
