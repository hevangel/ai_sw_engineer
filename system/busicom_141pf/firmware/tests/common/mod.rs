//! Shared host **board model** for the port-level tests.
//!
//! Both `tests/port_level.rs` (replay vs `oracle/manual_examples.json`) and
//! `tests/port_trace.rs` (replay vs the original-ROM `oracle/rom_port_trace.txt`)
//! drive the SAME M3 machine drivers through the SAME board model defined here,
//! so neither test can quietly validate against a different model than the
//! other. The model implements [`software::hal::Hal`] (4003 shift registers,
//! keyboard matrix, drum/TEST generator, hammer/red/advance/lamp decode) and is
//! an independent reimplementation of the documented board behaviour
//! (ai_hw_engineer board spec §3–4 and `src/busicom_141pf.sv`), cross-checked
//! against V. Ilmer's `external/busicom` + `external/chips` (no license; read
//! and run, never copied — see `oracle/port_level_validation.md`).

use software::driver_keyboard::{KeyBuffer, KeyboardDriver};
use software::driver_lamps::write_lamps;
use software::driver_printer::{
    PrinterDriver, DRUM_NUMERIC, NUMERIC_LSB, ROUNDED_SECTOR, SECTORS, SYMBOL_A_BIT, SYMBOL_B_BIT,
};
use software::hal::*;
use software::print::COLUMNS;
use software::{Calculator, Key, Lamps, Rounding, Switches};

// ---------------------------------------------------------------------------
// 10-bit 4003 shift register model (external/chips `shifter4003` semantics:
// the bit shifts in on the clock high→low pulse edge).
// ---------------------------------------------------------------------------
#[derive(Default, Clone, Copy)]
pub struct Shifter {
    data: u16,
    pulse: bool,
}

impl Shifter {
    /// `in_bit` serial input, `pulse` the (active) clock level. Returns the MSB
    /// (serial out) *before* any shift, for cascading.
    pub fn step(&mut self, in_bit: bool, pulse: bool) -> bool {
        let out = (self.data >> 9) & 1 == 1;
        if self.pulse && !pulse {
            self.data = ((self.data << 1) & 0x3FF) | in_bit as u16;
        }
        self.pulse = pulse;
        out
    }

    pub fn parallel(&self) -> u16 {
        self.data
    }
}

// ---------------------------------------------------------------------------
// A row reconstructed from the hammer words captured over one revolution.
//
// This keeps the two symbol columns *and* the rounded mark as separate drum
// sectors, which is richer than the single collapsed `symbol` string the oracle
// JSON profile uses. `port_level.rs` collapses it back to the oracle shape;
// `port_trace.rs` maps each sector to the original-ROM trace glyph. Decoding the
// two columns independently is why the trace test can tell a symbol-A divergence
// from a symbol-B one.
// ---------------------------------------------------------------------------
#[derive(Debug, PartialEq, Clone)]
pub struct CapturedRow {
    /// Numeric columns, trimmed; `None` for the dotted overflow line.
    pub value: Option<String>,
    /// Drum sector printed in symbol column A (excludes the rounded mark).
    pub sym_a: Option<usize>,
    /// Drum sector printed in symbol column B.
    pub sym_b: Option<usize>,
    /// The rounded-up `^` mark (column A, sector 7) fired on this line.
    pub rounded: bool,
    /// Red ink latched for this line.
    pub red: bool,
}

// ---------------------------------------------------------------------------
// Board model implementing the port-level Hal.
// ---------------------------------------------------------------------------
pub struct Board {
    // Keyboard
    kb_shift: Shifter,
    keys_mask: u32, // bit i set = scancode (129+i) pressed
    dp: u8,
    rounding: u8,
    // Printer shift chain (two cascaded 4003s → 20-bit word).
    pr_lo: Shifter,
    pr_hi: Shifter,
    // Drum / TEST
    test_level: bool,
    drum_sector: usize, // sector currently under the hammers (0..12)
    // RAM0 edge detection
    ram0_prev: u8,
    red_latched: bool,
    // Captured print for the current line, indexed by column.
    line_numeric: [u8; COLUMNS], // b' ' until a hammer sets it
    line_sym_a: Option<usize>,   // drum sector printed in symbol col A
    line_sym_b: Option<usize>,   // drum sector printed in symbol col B
    line_rounded: bool,
    line_red: bool,
    line_touched: bool,
    // Finished rows handed back to the test.
    pub rows: Vec<CapturedRow>,
    // Lamp latches (bits 0/1/2 decoded) and the raw RAM1 nibble last written
    // (all four bits, so the trace test can assert bit 3).
    pub lamps: Lamps,
    pub lamp_nibble: u8,
}

impl Board {
    pub fn new() -> Self {
        Board {
            kb_shift: Shifter::default(),
            keys_mask: 0,
            dp: 0,
            rounding: 0,
            pr_lo: Shifter::default(),
            pr_hi: Shifter::default(),
            test_level: false,
            drum_sector: SECTORS - 1, // first test_pin() poll presents sector 0
            ram0_prev: 0,
            red_latched: false,
            line_numeric: [b' '; COLUMNS],
            line_sym_a: None,
            line_sym_b: None,
            line_rounded: false,
            line_red: false,
            line_touched: false,
            rows: Vec::new(),
            lamps: Lamps::default(),
            lamp_nibble: 0,
        }
    }

    /// Press exactly one key (one-hot) and set the selector switches.
    fn set_input(&mut self, key: Option<Key>, sw: Switches) {
        self.keys_mask = match key {
            Some(k) => 1 << (k as u8 - 129),
            None => 0,
        };
        self.dp = sw.dp;
        self.rounding = match sw.rounding {
            Rounding::Float => 0,
            Rounding::Round => 1,
            Rounding::Truncate => 8,
        };
    }

    /// Keyboard column nibble for the row the 4003 #0 currently selects (the
    /// single-low bit position). Rows 8/9 return the selector values.
    fn kb_columns(&self) -> u8 {
        let q = self.kb_shift.parallel() & 0x3FF;
        if q.count_ones() != 1 {
            return 0; // no row (or malformed) selected
        }
        let row = q.trailing_zeros() as u8;
        match row {
            0..=7 => {
                let base = 129 + 4 * row;
                let mut nibble = 0u8;
                for col in 0..4 {
                    if self.keys_mask & (1 << (base + col - 129)) != 0 {
                        nibble |= 1 << col;
                    }
                }
                nibble
            }
            8 => self.dp & 0xF,
            9 => self.rounding & 0xF,
            _ => 0,
        }
    }

    /// Advance the virtual drum one sector and toggle TEST.
    fn tick_drum(&mut self) {
        self.test_level = !self.test_level;
        self.drum_sector = (self.drum_sector + 1) % SECTORS;
    }

    /// Decode the 20-bit hammer word captured at a hammer edge into the current
    /// line, using the drum sector under the hammers at that instant.
    fn capture_hammer(&mut self) {
        let word = (self.pr_hi.parallel() as u32) << 10 | self.pr_lo.parallel() as u32;
        let sector = self.drum_sector;
        self.line_touched = true;
        if self.red_latched {
            self.line_red = true;
        }
        for c in 0..COLUMNS {
            if word & (1 << (NUMERIC_LSB as usize + c)) != 0 {
                self.line_numeric[c] = DRUM_NUMERIC[sector];
            }
        }
        if word & (1 << SYMBOL_A_BIT) != 0 {
            if sector == ROUNDED_SECTOR {
                self.line_rounded = true;
            } else {
                self.line_sym_a = Some(sector);
            }
        }
        if word & (1 << SYMBOL_B_BIT) != 0 {
            self.line_sym_b = Some(sector);
        }
    }

    /// Finish the current line (paper advance) and push a reconstructed row.
    fn finish_line(&mut self) {
        if !self.line_touched {
            return;
        }
        let all_dots = self.line_numeric.iter().all(|&c| c == b'.');
        let value = if all_dots {
            None
        } else {
            let s: String = self.line_numeric.iter().map(|&c| c as char).collect();
            Some(s.trim().to_string())
        };
        self.rows.push(CapturedRow {
            value,
            sym_a: self.line_sym_a,
            sym_b: self.line_sym_b,
            rounded: self.line_rounded,
            red: self.line_red,
        });
        self.line_numeric = [b' '; COLUMNS];
        self.line_sym_a = None;
        self.line_sym_b = None;
        self.line_rounded = false;
        self.line_red = false;
        self.line_touched = false;
        self.red_latched = false;
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

impl Hal for Board {
    fn rom_port_write(&mut self, chip: u8, value: u8) {
        if chip != ROM_SHIFT {
            return;
        }
        let data_in = value & ROM0_SERIAL_DATA == 0;
        let kb_pulse = value & ROM0_KB_CLOCK == 0;
        let pr_pulse = value & ROM0_PR_CLOCK == 0;
        self.kb_shift.step(data_in, kb_pulse);
        let lo_out = self.pr_lo.step(data_in, pr_pulse);
        self.pr_hi.step(lo_out, pr_pulse);
    }

    fn rom_port_read(&mut self, chip: u8) -> u8 {
        match chip {
            ROM_KEYBOARD => self.kb_columns(),
            ROM_PRINTER_IN => {
                let mut n = 0;
                if self.drum_sector == 0 {
                    n |= ROM2_DRUM_INDEX;
                }
                n
            }
            _ => 0,
        }
    }

    fn ram_port_write(&mut self, chip: u8, value: u8) {
        match chip {
            RAM_PRINTER => {
                if value & RAM0_RED != 0 {
                    self.red_latched = true;
                }
                let hammer_edge = self.ram0_prev & RAM0_HAMMER == 0 && value & RAM0_HAMMER != 0;
                let advance_edge = self.ram0_prev & RAM0_ADVANCE == 0 && value & RAM0_ADVANCE != 0;
                if hammer_edge {
                    self.capture_hammer();
                }
                if advance_edge {
                    self.finish_line();
                }
                self.ram0_prev = value;
            }
            RAM_LAMPS => {
                self.lamp_nibble = value & 0xF;
                self.lamps = Lamps {
                    memory: value & RAM1_MEMORY != 0,
                    overflow: value & RAM1_OVERFLOW != 0,
                    negative: value & RAM1_NEGATIVE != 0,
                };
            }
            _ => {}
        }
    }

    fn test_pin(&mut self) -> bool {
        self.tick_drum();
        self.test_level
    }
}

/// Feed one key through the keyboard scanner, then run it on the calculator and
/// drive the printer, all port-level. Asserts the scanner recovered the key and
/// switches, so a keyboard-scan-order divergence fails here, not silently.
pub fn press_through_ports(board: &mut Board, calc: &mut Calculator, key: Key, sw: Switches) {
    board.set_input(Some(key), sw);
    let scanned_sw;
    let got;
    {
        let mut kb = KeyboardDriver::new(board);
        let mut buf = KeyBuffer::new();
        scanned_sw = kb.scan_pass(&mut buf);
        got = buf.pop();
    }
    assert_eq!(got, Some(key), "keyboard scan did not recover the key");
    assert_eq!(scanned_sw, sw, "keyboard scan did not recover the switches");
    board.set_input(None, sw);

    {
        let mut printer = PrinterDriver::new(board);
        calc.press(key, scanned_sw, &mut printer);
    }
    write_lamps(board, calc.lamps());
}
