# Busicom 141-PF firmware — I/O contract

Two layers, so the arithmetic core can be proven on the host before any 4004 driver exists.

## Core interface (`src/lib.rs`, used by host tests and later by the target drivers)

| Rust | Meaning | Oracle field |
|---|---|---|
| `Calculator::press(Key, Switches, &mut impl Printer)` | one debounced key with the current selector positions | `steps[].keys`, `switches` |
| `Printer::print(&Row)` | one printed paper row | `expect.tape[]` |
| `Calculator::lamps() -> Lamps` | memory / overflow / negative | `expect.lamps` |

`Row::value()` renders the 15 numeric columns as the oracle's `value` string (leading blanks stripped; `None` for the dotted overflow line); `Symbol::as_str()` gives the oracle's `symbol`.

## Machine interface (`src/hal.rs`, target drivers — milestone M3)

| Hal method | 4004 instruction(s) | Device |
|---|---|---|
| `rom_port_write(chip, nibble)` | `SRC chip` + `WRR` | ROM0: 4003 clocks (bit0 kb, bit2 printer) + serial data (bit1, active low) |
| `rom_port_read(chip)` | `SRC chip` + `RDR` | ROM1 keyboard columns, ROM2 drum index (bit0) / paper button (bit3) |
| `ram_port_write(chip, nibble)` | `DCL`/`SRC chip` + `WMP` | RAM0 printer (bit0 red, bit1 hammer, bit3 advance), RAM1 lamps (bit0/1/2 = mem/ovf/neg) |
| `test_pin()` | `JCN`/`JTN` on TEST | drum half-spin sync |

Chip-select and bit constants live in `src/hal.rs` (`ROM_SHIFT`, `ROM_KEYBOARD`,
`ROM_PRINTER_IN`, `RAM_PRINTER`, `RAM_LAMPS`, and the `ROM0_*`/`ROM2_*`/`RAM0_*`/
`RAM1_*` masks).

### Drivers (`src/driver_*.rs`)

- **`driver_keyboard`** — scans by walking a single selection bit through
  4003 #0 (shift via ROM0 bit0 clock + bit1 data), reads the ROM1 column nibble
  per row, decodes scancode `129 + 4*row + col` → `Key`; rows Q8/Q9 give the
  decimal / rounding selectors (`Switches`). An 8-deep `KeyBuffer` FIFO holds
  scanned keys (the real machine's 8-word input buffer) and drops a 9th like the
  hardware.
- **`driver_printer`** — a `Printer`, so the existing `Calculator` drives it
  unchanged. Turns a `Row` into a per-drum-sector 20-bit hammer word (bits 3..=17
  the 15 numeric columns, bit 0 symbol A, bit 1 symbol B), steps the 13 drum
  sectors on `test_pin()`, shifts each sector's word into the 4003 #1+#2 chain
  (ROM0 bit2 clock) and fires the hammers (RAM0 bit1 rising edge). Red latches
  RAM0 bit0; paper advance pulses RAM0 bit3. Drum sector tables (`DRUM_NUMERIC`,
  `DRUM_SYMBOL_A/B`) are public so the host board model shares them.
- **`driver_lamps`** — packs `Lamps` into the RAM1 nibble (`WMP`).

The printer driver turns a `Row` into the 13-sector hammer sequence; the
keyboard driver scans 4003 #0 rows into `Key`s and the selector rows into
`Switches`.

### Port-level test (`tests/port_level.rs`)

A host **board model** implements `Hal` (4003 shift registers, keyboard matrix,
drum/TEST generator, hammer/red/advance/lamp decode) and the 42 examples replay
through `keyboard scan → Calculator → printer drum → board decode → Row`. It is
an independent reimplementation of the documented board behaviour, validated
against the original ROM on `external/chips` (see
`oracle/port_level_validation.md`, `oracle/rom_port_trace.txt`). Ratchet:
`oracle/port_level_pass.txt` (42/42).
