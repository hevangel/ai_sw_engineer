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
| `rom_port_write(chip, nibble)` | `SRC` + `WRR` | ROM0: 4003 clocks and serial data |
| `rom_port_read(chip)` | `SRC` + `RDR` | ROM1 keyboard columns, ROM2 drum index / paper button |
| `ram_port_write(chip, nibble)` | `DCL`/`SRC` + `WMP` | RAM0 printer control, RAM1 lamps |
| `test_pin()` | `JCN` on TEST | drum half-spin sync |

The printer driver turns a `Row` into the 13-sector hammer sequence; the keyboard driver scans 4003 #0 rows into `Key`s and the selector rows into `Switches`.
