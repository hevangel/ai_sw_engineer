//! Drum-printer driver (milestone M3).
//!
//! The Model-102 drum has 13 character sectors; one revolution presents every
//! sector under the hammers in turn. The firmware shifts a 20-bit hammer word
//! into the cascaded 4003 #1+#2 chain (one bit per hammer column), then fires
//! the hammers (RAM0 bit 1 rising edge) at the moment the drum shows the sector
//! whose character that column should print. Over a full revolution a column
//! fires at most once, so a decimal row needs up to 13 hammer pulses.
//!
//! This driver is a [`Printer`]: the existing [`crate::Calculator`] drives it
//! unchanged. It talks to the board only through [`Hal`], busy-waiting on
//! `test_pin()` for each drum half-spin exactly as the ROM does.
//!
//! Hammer-word layout (ai_hw_engineer board spec §4.2, cross-checked against
//! `external/busicom` `printer.rs`): bits 3..=17 are the 15 numeric columns
//! (bit 3 = leftmost), bit 0 = symbol column A, bit 1 = symbol column B,
//! bit 2 unused. The drum sector tables are the same source.

use crate::hal::{
    Hal, RAM0_ADVANCE, RAM0_HAMMER, RAM0_RED, RAM_PRINTER, ROM0_PR_CLOCK, ROM0_SERIAL_DATA,
    ROM_SHIFT,
};
use crate::print::{Printer, Row, Symbol, COLUMNS};

/// Drum sectors per revolution.
pub const SECTORS: usize = 13;
/// Hammer-word bit of the leftmost numeric column.
pub const NUMERIC_LSB: u8 = 3;
/// Hammer-word bit of symbol column A.
pub const SYMBOL_A_BIT: u8 = 0;
/// Hammer-word bit of symbol column B.
pub const SYMBOL_B_BIT: u8 = 1;

/// Numeric-column glyph at each drum sector (sectors 10,11 = `.`, 12 = `-`).
pub const DRUM_NUMERIC: [u8; SECTORS] = *b"0123456789..-";

/// Symbol-column A glyph (oracle `symbol` string) at each drum sector.
pub const DRUM_SYMBOL_A: [&str; SECTORS] = [
    "ST", // ◇ subtotal / diamond
    "+", "-", "*op", // × multiply operand
    "/",   // ÷ divide
    "M+", "M-", "^", // rounded-up mark
    "=", "SQRT", // √
    "%", "C", "RM", // R recall
];

/// Symbol-column B glyph (oracle `symbol` string) at each drum sector.
pub const DRUM_SYMBOL_B: [&str; SECTORS] = [
    "#", "*", // total star
    "I", "II", "III", "M+", "M-", "T", "K", "E", "EX", "C", "CM", // M (clear memory)
];

/// Where a [`Symbol`] prints on the drum: `(sector, is_symbol_column_b)`.
/// `None` for [`Symbol::None`] (no symbol hammer fires).
pub fn symbol_sector(sym: Symbol) -> Option<(usize, bool)> {
    // Column A sectors.
    let a = |s: usize| Some((s, false));
    let b = |s: usize| Some((s, true));
    match sym {
        Symbol::None => None,
        Symbol::SubTotal => a(0),
        Symbol::Plus => a(1),
        Symbol::Minus => a(2),
        Symbol::Multiply => a(3),
        Symbol::Divide => a(4),
        Symbol::MemPlus => a(5),
        Symbol::MemMinus => a(6),
        // `^` (rounded) rides column A sector 7; see `rounded` handling below.
        Symbol::Equals => a(8),
        Symbol::Sqrt => a(9),
        Symbol::Percent => a(10),
        Symbol::Clear => a(11),
        Symbol::RecallMemory => a(12),
        Symbol::Total => b(1),
        Symbol::Reference => b(0),
        Symbol::Exchange => b(10),
        Symbol::ClearMemory => b(12),
    }
}

/// Rounded-up `^` mark: column A, drum sector 7.
pub const ROUNDED_SECTOR: usize = 7;

/// The drum-printer driver.
pub struct PrinterDriver<'a, H: Hal> {
    hal: &'a mut H,
    red_latched: bool,
}

impl<'a, H: Hal> PrinterDriver<'a, H> {
    pub fn new(hal: &'a mut H) -> Self {
        PrinterDriver {
            hal,
            red_latched: false,
        }
    }

    /// Advance the paper one line (RAM0 bit 3 rising edge). Clears the red latch.
    pub fn advance_paper(&mut self) {
        self.pulse_ram0(RAM0_ADVANCE);
        self.red_latched = false;
    }

    /// Step the drum one sector by sampling the TEST half-spin line once. Each
    /// call corresponds to one sector arriving under the hammers; the board
    /// model advances its drum sector on the same sample, so the driver's
    /// sector index and the physical sector stay in lock-step.
    fn step_sector(&mut self) {
        let _ = self.hal.test_pin();
    }

    /// Shift one hammer-word bit into the printer 4003 chain: set the serial
    /// data line (active low), pulse the printer clock (active low) low→high.
    fn shift_printer_bit(&mut self, bit: bool) {
        // Idle: clock high, data high. Present data (active low), drop the
        // clock, raise it to clock the bit in — the 4003 shifts on the
        // high→low→high clock the firmware emits (external/chips shifts on the
        // high→low pulse edge).
        let data = if bit { 0 } else { ROM0_SERIAL_DATA };
        // clock high with data presented
        self.hal.rom_port_write(ROM_SHIFT, data | ROM0_PR_CLOCK);
        // clock low (active-low pulse) — the shift happens here
        self.hal.rom_port_write(ROM_SHIFT, data);
        // clock back high
        self.hal.rom_port_write(ROM_SHIFT, data | ROM0_PR_CLOCK);
    }

    /// Shift a full 20-bit hammer word MSB-first into the chain.
    fn shift_hammer_word(&mut self, word: u32) {
        for i in (0..20).rev() {
            self.shift_printer_bit((word >> i) & 1 == 1);
        }
    }

    /// Fire the hammers: RAM0 bit 1 rising edge, red latched in bit 0.
    fn fire_hammer(&mut self) {
        let red = if self.red_latched { RAM0_RED } else { 0 };
        self.hal.ram_port_write(RAM_PRINTER, red); // hammer low
        self.hal.ram_port_write(RAM_PRINTER, red | RAM0_HAMMER); // rising edge
        self.hal.ram_port_write(RAM_PRINTER, red); // release
    }

    /// Pulse a RAM0 edge bit low→high→low (for paper advance).
    fn pulse_ram0(&mut self, bit: u8) {
        let red = if self.red_latched { RAM0_RED } else { 0 };
        self.hal.ram_port_write(RAM_PRINTER, red);
        self.hal.ram_port_write(RAM_PRINTER, red | bit);
        self.hal.ram_port_write(RAM_PRINTER, red);
    }

    /// Which drum sector each hammer column needs for this row, as a per-sector
    /// 20-bit mask: `masks[s]` has a 1 for every column that fires at sector s.
    fn sector_masks(row: &Row) -> [u32; SECTORS] {
        let mut masks = [0u32; SECTORS];
        if row.overflow {
            // The dotted overflow line prints `.` in all 15 numeric columns.
            // `.` is drum sector 10.
            for c in 0..COLUMNS {
                masks[10] |= 1 << (NUMERIC_LSB as usize + c);
            }
            return masks;
        }
        // Numeric columns: map each glyph to its drum sector.
        for (c, &g) in row.cols.iter().enumerate() {
            let sector = match g {
                b'0'..=b'9' => (g - b'0') as usize,
                b'.' => 10,
                b'-' => 12,
                _ => continue, // blank column: no hammer
            };
            masks[sector] |= 1 << (NUMERIC_LSB as usize + c);
        }
        // Symbol column.
        if let Some((sector, col_b)) = symbol_sector(row.symbol) {
            let bit = if col_b { SYMBOL_B_BIT } else { SYMBOL_A_BIT };
            masks[sector] |= 1 << bit;
        }
        // Rounded-up `^` rides column A, sector 7.
        if row.rounded {
            masks[ROUNDED_SECTOR] |= 1 << SYMBOL_A_BIT;
        }
        masks
    }
}

impl<H: Hal> Printer for PrinterDriver<'_, H> {
    fn print(&mut self, row: &Row) {
        // Red ink is latched for the whole line before the drum sweeps.
        if row.red {
            self.red_latched = true;
            let red = RAM0_RED;
            self.hal.ram_port_write(RAM_PRINTER, red);
        }
        let masks = Self::sector_masks(row);
        // One drum revolution: 13 sectors, each gated by a TEST half-spin. At
        // sector s, shift the columns that want sector s and fire the hammers.
        for (s, &mask) in masks.iter().enumerate() {
            self.step_sector();
            if mask != 0 {
                self.shift_hammer_word(mask);
                self.fire_hammer();
            } else if s == 0 {
                // Shift a cleared word once so the chain is defined even for a
                // row whose first firing sector is later.
                self.shift_hammer_word(0);
            }
        }
        // Advance to a fresh paper line (clears the red latch).
        self.advance_paper();
    }
}
