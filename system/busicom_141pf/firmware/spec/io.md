# Busicom 141-PF firmware — I/O contract

Two layers, so the arithmetic core can be proven on the host before any 4004
driver exists. On the target (issue #18), the drivers' hot paths are
hand-written 4004 routines in `asm/drivers.s`. Each one has a Rust twin
that the host tests run.

## Core interface (`src/lib.rs`, used by host tests and the target loop)

| Rust | Meaning | Oracle field |
|---|---|---|
| `Calculator::press(Key, Switches, &mut impl Printer)` | one debounced key with the current selector positions (host) | `steps[].keys`, `switches` |
| `Calculator::press_at(row, col, &mut impl Printer)` | the same, from a keyboard matrix position; the selectors are read from RAM (`SW_DP` 0xC2, `SW_ROUND` 0xC3), where the scan latched them (target loop) | same |
| `Printer::print(&Row)`, `Printer::number/blank/overflow` | one printed paper row | `expect.tape[]` |
| `Calculator::lamps() -> Lamps` | memory / overflow / negative | `expect.lamps` |

`Row::value()` renders the 15 numeric columns as the oracle's `value` string
(leading blanks stripped; `None` for the dotted overflow line). `SymA`/`SymB`
give the oracle's two symbol columns.

The calculator's state is not a Rust struct. It lives in 4002 RAM bank 0
(registers, flags, key FIFO, print register; see
`../platform/memory_map.md`), reached through `src/ram.rs`.

## Machine interface (`src/hal.rs`)

| Hal method | 4004 instruction(s) | Device |
|---|---|---|
| `rom_port_write(chip, nibble)` | `SRC chip` + `WRR` | ROM0: 4003 clocks (bit 0 keyboard, bit 2 printer) + serial data (bit 1) |
| `rom_port_read(chip)` | `SRC chip` + `RDR` | ROM1 keyboard columns, ROM2 drum index (bit 0) / paper button (bit 3) |
| `ram_port_write(chip, nibble)` | `SRC chip` + `WMP` | RAM0 printer (bit 0 red, bit 1 hammer, bit 3 advance), RAM1 lamps (bit 0/1/2 = mem/ovf/neg, bit 3 status, spec A9) |
| `test_pin()` | `JCN` on TEST | drum half-spin sync |

**Serial data polarity differs per chain.** On the keyboard chain (4003 #0)
the selected row is the one `0` bit. On the printer chain (4003 #1+#2), a `1`
fires that column's hammer
(`failure_notes/2026-10-22-busicom_141pf-target-only-driver-defects.md`).

Chip-select and bit constants live in `src/hal.rs` (`ROM_SHIFT`,
`ROM_KEYBOARD`, `ROM_PRINTER_IN`, `RAM_PRINTER`, `RAM_LAMPS`, and the
`ROM0_*`/`ROM2_*`/`RAM0_*`/`RAM1_*` masks). On the target, each `Hal` method is
one `__bus_*` runtime call (`src/main.rs`).

### Drivers

| Function | Target (`asm/drivers.s`) | Host twin (`src/driver_*.rs`) |
|---|---|---|
| Keyboard scan | `__kb_scan()` → `row << 4 \| col` of a new key, or 0xFF | `KeyboardDriver::scan_latch` |
| Print a row | `__pr_row(src, syms)` fills the print register (0xE0) and prints it; `__pr_print()` prints it as is | `Row::number/blank/overflow` + `PrinterDriver::print` |
| Lamps | `__bus_lamps()` writes the RAM1 nibble from the RAM flags | `Calculator::lamp_nibble` + `driver_lamps` |

- **Keyboard.** The scan walks a single `0` bit through 4003 #0 (ROM0 bit 0
  clock, bit 1 data) and reads the ROM1 column nibble per row. Rows Q0–Q7 are
  keys, scancode `129 + 4*row + col`. Rows Q8/Q9 are the decimal and rounding
  selectors, latched into `SW_DP`/`SW_ROUND` on every pass.
  - **One event per press:** a key counts once per press and release, using
    the key-down flag `KB_DOWN` (status 0xF0 S0) (spec M3-K2).
  - **Type-ahead:** the host keeps the original's 8-key FIFO in register 0xC0
    (`KeyBuffer`). The target scans only between keys, so there is no
    type-ahead (spec M3-K3).
- **Printer.** The print register holds each column's drum sector (0–9 digits,
  10 point, 12 minus, 15 blank), the sectors of symbol columns A and B, and
  the red flag.
  - **Per sector:** the driver shifts the 20-bit hammer word into the 4003
    #1+#2 chain (ROM0 bit 2 clock): bits 3–17 are numeric columns 14–0, bit 0
    symbol A, bit 1 symbol B. Then it fires the hammers (RAM0 bit 1 rising
    edge), with red held on RAM0 bit 0.
  - **Line end:** after sector 12 it waits one TEST edge, then pulses RAM0
    bit 3 to advance the paper.
  - **Timing:** sectors are found by counting TEST edges after the drum index,
    never by reading the TEST level (spec M3-P2/M3-P3).
- **Drum sector tables** (`DRUM_NUMERIC`, `DRUM_SYMBOL_A/B`) are public so the
  host board model shares them.

### Port-level tests (`tests/port_level.rs`, `tests/port_trace.rs`)

A host **board model** implements `Hal`: 4003 shift registers, keyboard
matrix, drum/TEST generator, and hammer/red/advance/lamp decode. The 42
examples replay through `keyboard scan → Calculator → printer drum → board
decode → Row`. `port_trace.rs` compares the full tape and lamp stream exactly
with the original ROM's `oracle/rom_port_trace.txt`.

The model steps the drum one sector per `test_pin()` sample (spec M3-P1). So
target drum timing is checked only by the image runs on external/chips and the
RTL (`../platform/scripts/run_image.sh` + `compare.sh`).
