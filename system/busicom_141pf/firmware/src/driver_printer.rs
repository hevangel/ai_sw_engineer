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
use crate::print::{Printer, Row, SymA, SymB, BLANK, PR};
use crate::ram::{srd, swr};
#[cfg(target_os = "none")]
use crate::{bcd::Num, print::POINT};

#[cfg(target_os = "none")]
extern "C" {
    fn __pr_row(src: u8, syms: u8);
    fn __pr_print();
}

/// The red-ink latch for the current line: status S3 of the print register.
const RED_LATCH: u8 = PR | 3;

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

/// Symbol-column A glyph (original-ROM trace glyph) at each drum sector
/// (ai_hw_engineer board spec §4.2; sector 7 is the rounded `^` mark).
pub const DRUM_SYMBOL_A: [&str; SECTORS] = [
    "◇",  // 0 subtotal / diamond
    "+",  // 1
    "-",  // 2
    "×",  // 3 multiply
    "÷",  // 4 divide
    "M+", // 5
    "M-", // 6
    "^",  // 7 rounded-up mark
    "=",  // 8
    "√",  // 9 sqrt
    "%",  // 10
    "C",  // 11 clear
    "R",  // 12 recall memory
];

/// Symbol-column B glyph (original-ROM trace glyph) at each drum sector.
pub const DRUM_SYMBOL_B: [&str; SECTORS] = [
    "#",   // 0 reference
    "*",   // 1 total star
    "I",   // 2
    "II",  // 3
    "III", // 4
    "M+",  // 5
    "M-",  // 6
    "T",   // 7
    "K",   // 8
    "E",   // 9
    "Ex",  // 10 exchange
    "C",   // 11
    "M",   // 12 clear / recall-memory companion
];

/// Rounded-up `^` mark: column A, drum sector 7.
pub const ROUNDED_SECTOR: usize = 7;

/// Drum sector for a column-A glyph, or `None` for [`SymA::None`].
pub fn sym_a_sector(sym: SymA) -> Option<usize> {
    match sym {
        SymA::None => None,
        s => Some(s as usize),
    }
}

/// Drum sector for a column-B glyph, or `None` for [`SymB::None`].
pub fn sym_b_sector(sym: SymB) -> Option<usize> {
    match sym {
        SymB::None => None,
        s => Some(s as usize),
    }
}

/// Hammer-word bits shifted per word (bits 19..18 and 2 are unused).
const WORD_BITS: u8 = 20;

/// The drum-printer driver.
pub struct PrinterDriver<'a, H: Hal> {
    hal: &'a mut H,
}

impl<'a, H: Hal> PrinterDriver<'a, H> {
    pub fn new(hal: &'a mut H) -> Self {
        // A fresh line per key: the red latch starts clear (host driver only;
        // `__pr_row` keeps the line's red in a register).
        #[cfg(not(target_os = "none"))]
        swr(RED_LATCH, 0);
        PrinterDriver { hal }
    }

    /// Advance the paper one line (RAM0 bit 3 rising edge). Clears the red latch.
    pub fn advance_paper(&mut self) {
        self.pulse_ram0(RAM0_ADVANCE);
        swr(RED_LATCH, 0);
    }

    /// Step the drum one sector by sampling the TEST half-spin line once. Each
    /// call corresponds to one sector arriving under the hammers; the board
    /// model advances its drum sector on the same sample, so the driver's
    /// sector index and the physical sector stay in lock-step.
    fn step_sector(&mut self) {
        let _ = self.hal.test_pin();
    }

    /// Shift one hammer-word bit into the printer 4003 chain: set the serial
    /// data line (a 1 fires), pulse the printer clock (active low) low→high.
    fn shift_printer_bit(&mut self, bit: bool) {
        // Idle: clock high. Present the data bit, drop the
        // clock, raise it to clock the bit in — the 4003 shifts on the
        // high→low→high clock the firmware emits (external/chips shifts on the
        // high→low pulse edge).
        // A 1 in the hammer word fires: the printer chain takes the data line
        // as is (unlike the keyboard chain, which selects a row with a 0).
        let data = if bit { ROM0_SERIAL_DATA } else { 0 };
        self.hal.rom_port_write(ROM_SHIFT, data | ROM0_PR_CLOCK);
        self.hal.rom_port_write(ROM_SHIFT, data);
        self.hal.rom_port_write(ROM_SHIFT, data | ROM0_PR_CLOCK);
    }

    /// Whether hammer-word bit `b` fires at drum sector `s` for `row`: bit 0 =
    /// symbol column A, bit 1 = column B, bits 3..=17 = numeric columns 0..14
    /// (the column's sector is in the print register).
    fn fires(row: &Row, b: u8, s: u8) -> bool {
        match b {
            0 => row.sym_a_sector() == s,
            1 => row.sym_b_sector() == s,
            NUMERIC_LSB..=17 => Row::col_sector(b - NUMERIC_LSB) == s,
            _ => false,
        }
    }

    /// Shift the 20-bit hammer word for sector `s` MSB-first into the chain
    /// (`s` = [`BLANK`] shifts an all-zero word). Returns whether any bit was set.
    #[inline(never)]
    fn shift_hammer_word(&mut self, row: &Row, s: u8) -> bool {
        let mut any = false;
        let mut b = WORD_BITS;
        while b > 0 {
            b -= 1;
            let bit = Self::fires(row, b, s);
            any |= bit;
            self.shift_printer_bit(bit);
        }
        any
    }

    /// Whether anything fires at sector `s` (decides whether to shift a word).
    fn sector_used(row: &Row, s: u8) -> bool {
        let mut b = 0;
        while b < WORD_BITS {
            if Self::fires(row, b, s) {
                return true;
            }
            b += 1;
        }
        false
    }

    /// Fire the hammers: RAM0 bit 1 rising edge, red latched in bit 0.
    fn fire_hammer(&mut self) {
        let red = if srd(RED_LATCH) != 0 { RAM0_RED } else { 0 };
        self.hal.ram_port_write(RAM_PRINTER, red); // hammer low
        self.hal.ram_port_write(RAM_PRINTER, red | RAM0_HAMMER); // rising edge
        self.hal.ram_port_write(RAM_PRINTER, red); // release
    }

    /// Pulse a RAM0 edge bit low→high→low (for paper advance).
    fn pulse_ram0(&mut self, bit: u8) {
        let red = if srd(RED_LATCH) != 0 { RAM0_RED } else { 0 };
        self.hal.ram_port_write(RAM_PRINTER, red);
        self.hal.ram_port_write(RAM_PRINTER, red | bit);
        self.hal.ram_port_write(RAM_PRINTER, red);
    }
}

impl<H: Hal> Printer for PrinterDriver<'_, H> {
    // Target: one `__pr_row` call per row (asm/drivers.s): it builds the
    // print register as `Row` does and prints it on the drum. Arguments: the
    // number's register, or 0xF0 + the fill value; symbol A << 4 | symbol B.
    #[cfg(target_os = "none")]
    #[inline(always)]
    fn number(&mut self, n: Num, sym_a: SymA, sym_b: SymB) {
        unsafe { __pr_row(n.0, ((sym_a as u8) << 4) + sym_b as u8) }
    }

    #[cfg(target_os = "none")]
    #[inline(always)]
    fn blank(&mut self, sym_a: SymA, sym_b: SymB) {
        unsafe { __pr_row(0xF0 + BLANK, ((sym_a as u8) << 4) + sym_b as u8) }
    }

    #[cfg(target_os = "none")]
    #[inline(always)]
    fn overflow(&mut self) {
        unsafe { __pr_row(0xF0 + POINT, (BLANK << 4) + BLANK) }
    }

    #[cfg(target_os = "none")]
    #[inline(always)]
    fn print(&mut self, _row: &Row) {
        // Target: `__pr_print` in asm/drivers.s prints the print register,
        // synchronised to the drum (this function's twin below models the
        // drum as one sector per TEST sample, spec M3-P1).
        unsafe { __pr_print() }
    }

    #[cfg(not(target_os = "none"))]
    #[inline(never)]
    fn print(&mut self, row: &Row) {
        // Red ink is latched for the whole line before the drum sweeps.
        if row.red() {
            swr(RED_LATCH, 1);
            self.hal.ram_port_write(RAM_PRINTER, RAM0_RED);
        }
        // One drum revolution: 13 sectors, each gated by a TEST half-spin. At
        // sector s, shift the columns that want sector s and fire the hammers.
        let mut s = 0;
        while s < SECTORS as u8 {
            self.step_sector();
            if Self::sector_used(row, s) {
                self.shift_hammer_word(row, s);
                self.fire_hammer();
            } else if s == 0 {
                // Shift a cleared word once so the chain is defined even for a
                // row whose first firing sector is later.
                self.shift_hammer_word(row, BLANK + 1);
            }
            s += 1;
        }
        // Advance to a fresh paper line (clears the red latch).
        self.advance_paper();
    }
}
