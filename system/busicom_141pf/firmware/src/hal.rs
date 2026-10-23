//! Machine I/O of the Busicom 141-PF board at the 4004 **port** level
//! (milestone M3). The drivers in [`crate::driver_keyboard`],
//! [`crate::driver_printer`] and [`crate::driver_lamps`] touch hardware ONLY
//! through this trait; the host test implements it with a board model, and the
//! future 4004 target entry (`main.rs`, M4) implements it with real
//! `SRC`+`WRR`/`RDR`/`WMP` and `JCN`-on-`TEST` sequences.
//!
//! Port semantics are the ai_hw_engineer board spec (§3) and RTL
//! (`system/busicom_141pf/src/busicom_141pf.sv`), cross-checked against
//! V. Ilmer's board model (`external/busicom` `keyboard.rs`/`printer.rs`, no
//! license — read and run, never copied). The arithmetic core does not use
//! this trait; it talks to [`crate::Printer`].
//!
//! ## ROM ports (`SRC` selects the chip, `WRR`/`RDR` move a nibble)
//!
//! | Port | Dir | Bit | Signal |
//! |---|---|---|---|
//! | ROM0 | out | 0 | 4003 #0 (keyboard) shift clock — active-low pulse |
//! | ROM0 | out | 1 | serial data to 4003 #0 and #1 — keyboard: a **0** selects the row; printer: a **1** fires the hammer |
//! | ROM0 | out | 2 | 4003 #1/#2 (printer chain) shift clock — active low |
//! | ROM1 | in  | 3:0 | keyboard matrix column nibble (row set by 4003 #0) |
//! | ROM2 | in  | 0 | printer drum index — one pulse per revolution |
//! | ROM2 | in  | 3 | paper-advance button |
//!
//! ## RAM output ports (`DCL`+`SRC` select the chip, `WMP` writes the nibble)
//!
//! | Port | Bit | Signal |
//! |---|---|---|
//! | RAM0 (CM-RAM0, P0=0) | 0 | print red — latched per line |
//! | RAM0 | 1 | fire hammers — rising edge prints the drum char in selected cols |
//! | RAM0 | 3 | advance paper — rising edge |
//! | RAM1 (CM-RAM0, P0=1) | 0/1/2 | Memory / Overflow / Negative lamps |
//!
//! ## TEST pin
//! The drum rotation drives the CPU `TEST` input: it toggles every drum
//! half-spin. The printer driver busy-waits on it (`JCN`/`JTN`) to step the
//! drum one sector at a time.

/// ROM chip carrying the 4003 shift clocks and serial data (ROM0).
pub const ROM_SHIFT: u8 = 0;
/// ROM chip whose input port is the keyboard column nibble (ROM1).
pub const ROM_KEYBOARD: u8 = 1;
/// ROM chip whose input port carries the drum index and paper button (ROM2).
pub const ROM_PRINTER_IN: u8 = 2;

/// RAM chip (CM-RAM0, P0=0) carrying red / hammer / advance (RAM0).
pub const RAM_PRINTER: u8 = 0;
/// RAM chip (CM-RAM0, P0=1) carrying the three lamps (RAM1).
pub const RAM_LAMPS: u8 = 1;

// ROM0 output bits (clocks are active-low pulses).
/// ROM0 bit 0: keyboard 4003 #0 shift clock.
pub const ROM0_KB_CLOCK: u8 = 1 << 0;
/// ROM0 bit 1: serial data to both 4003 chains (keyboard: 0 selects; printer: 1 fires).
pub const ROM0_SERIAL_DATA: u8 = 1 << 1;
/// ROM0 bit 2: printer 4003 #1/#2 shift clock.
pub const ROM0_PR_CLOCK: u8 = 1 << 2;

// ROM2 input bits.
/// ROM2 bit 0: drum index pulse.
pub const ROM2_DRUM_INDEX: u8 = 1 << 0;
/// ROM2 bit 3: paper-advance button.
pub const ROM2_PAPER_BTN: u8 = 1 << 3;

// RAM0 output bits.
/// RAM0 bit 0: print red (per-line latch).
pub const RAM0_RED: u8 = 1 << 0;
/// RAM0 bit 1: fire hammers (rising edge).
pub const RAM0_HAMMER: u8 = 1 << 1;
/// RAM0 bit 3: advance paper (rising edge).
pub const RAM0_ADVANCE: u8 = 1 << 3;

// RAM1 output bits (lamps).
/// RAM1 bit 0: Memory lamp.
pub const RAM1_MEMORY: u8 = 1 << 0;
/// RAM1 bit 1: Overflow lamp.
pub const RAM1_OVERFLOW: u8 = 1 << 1;
/// RAM1 bit 2: Negative (minus-sign) lamp.
pub const RAM1_NEGATIVE: u8 = 1 << 2;
/// RAM1 bit 3: the second bit of the WR sign-status character (`WR.S0.bit1`).
///
/// The annotated firmware listing documents RAM1 as `bit3 = not used`, yet the
/// status-light routine writes `WMP` with the nibble
/// `8·WR.S0.bit1 + 4·WR.S0.bit0 + 2·overflow + (MR≠0)`, so bit 3 carries
/// `WR.S0.bit1`. The original ROM therefore drives this "unused" bit, and the
/// port trace captures it. See `driver_lamps` for how it is reproduced.
pub const RAM1_STATUS: u8 = 1 << 3;

/// The board as the 4004 drivers see it: four port operations and the TEST pin.
///
/// `chip` is the strapped chip number (0-based). On the target a method is one
/// `SRC` (to select `chip`'s port) followed by a `WRR`/`RDR`/`WMP`; `test_pin`
/// is a `JCN`/`JTN` read of the `TEST` input. Only the low 4 bits of a nibble
/// are meaningful.
pub trait Hal {
    /// `SRC chip` + `WRR`: write a ROM chip's 4-bit output port.
    fn rom_port_write(&mut self, chip: u8, value: u8);
    /// `SRC chip` + `RDR`: read a ROM chip's 4-bit input port.
    fn rom_port_read(&mut self, chip: u8) -> u8;
    /// `DCL`/`SRC chip` + `WMP`: write a RAM chip's 4-bit output port.
    fn ram_port_write(&mut self, chip: u8, value: u8);
    /// `JCN`/`JTN` on the `TEST` pin: the current drum half-spin level.
    fn test_pin(&mut self) -> bool;
}
