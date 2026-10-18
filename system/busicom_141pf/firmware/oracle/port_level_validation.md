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

### What it compares (order-relevant observable events)

Grouped exactly as the trace groups them (`#RESET`, then one group per `#STEP`):

- **printed rows, in order** — per row: the numeric value (hammer word ↔ drum
  sector decode; `None` = dotted overflow line), the set of symbol-column
  glyphs, and red ink;
- **memory and overflow lamp bits per step** — asserted exactly.

Cycle timing, keyboard-scan repetition, TEST poll repetition and shift-bit
timing are deliberately NOT compared (our instruction stream is not the ROM's).

### Normalisations (documented; applied to both sides)

1. **Keyboard scan / idle polling** — the trace records board *output* only, so
   there is nothing to fold on its side; our scan order is still asserted via
   `KeyboardDriver::scan_pass` recovering the exact key + switches per press.
2. **Zero-register value** — a value whose digits are all zero is normalised to
   the bare `0` (the ROM prints `0`; our core formats zero to the DP width).
   Non-zero values keep every digit (`4.000000` is not folded to `4`).
3. **Symbol glyph set** — a row's symbol is the *set* of its two drum-column
   glyphs (plus the rounded `^`); a row matches when value and red are equal and
   our glyph set is a **subset** of the ROM row's (our single-`Symbol` model may
   carry fewer glyphs, never a wrong one).
4. **Drum half-spin granularity** — as M3-P1: one sector per `test_pin()` sample.

### Property asserted

> For every example, our emitted rows are an **in-order subsequence** of the ROM
> trace rows (value + red equal, our glyph set ⊆ the ROM row's), and the per-step
> memory/overflow lamp bits match exactly.

This proves our port output never *contradicts* the ROM — no wrong value, wrong
order, spurious row, or wrong memory/overflow lamp (a mutation of a single drum
glyph fails ~22/42 examples). It is **42/42** today.

### What it deliberately does NOT prove (documented debt, gated)

The original ROM prints a richer tape than our single-`Symbol`, JSON-oracle-shaped
core emits. These are recorded as `spec/spec.md` ASSUMPTIONs and gated (not
silently equated), not reproduced:

- **A6** recalled `×`/`÷` operand echo rows (e.g. 5-1 `12.4 ÷`, `0.55 ÷`);
- **A7** bare `M+`/`M-` key echo rows (e.g. 9-1 `6.89 M-`, 10-1 `10.00 M+`);
- **A8** the ROM's two independent symbol columns (`C`+`M` for CM, `R`+`M` for
  RM, `^`+`M+` for a rounded memory result, M=± result in column B);
- **A9** the negative lamp (bit 2) timing and an undocumented RAM1 bit 3 the ROM
  sets on overflow (nibble 10) and negative totals (nibble 12).

One escaped defect the trace DID catch was fixed: `C` printed a blank row instead
of `0` (`failure_notes/2026-10-18-busicom_141pf-clear-prints-zero.md`, spec A3).
