//! Busicom 141-PF calculator firmware (1971), reimplemented in Rust.
//!
//! `no_std`, no allocation, no integer wider than `u8` in the arithmetic.
//! This is the machine-independent core: keys in, printed rows and lamps out.
//! The 4004 drivers (keyboard scan, printer drum, lamps) sit behind `hal`.
//!
//! Behavior is specified in `spec/spec.md`; the oracle is the original ROM's
//! output for the 42 manual examples (`oracle/manual_examples.json`).
#![no_std]

pub mod bcd;
pub mod hal;
pub mod keys;
pub mod print;

pub use bcd::Num;
pub use keys::{Key, Lamps, Rounding, Switches};
pub use print::{Printer, Row, Symbol};

use bcd::CAPACITY;

/// The last function key class, which decides what `=` and `ST` mean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Idle,
    AddSub,
}

/// All firmware state: the five working registers, memory and flags.
#[derive(Clone, Debug)]
pub struct Calculator {
    /// Entry register as typed (scale = fractional digits typed).
    entry: Num,
    /// Digits are being typed into `entry` (or CE set it to an explicit zero).
    entering: bool,
    /// `.` was typed in the current entry.
    point: bool,
    /// Operand repeated by `+`/`-` with no new entry.
    last: Num,
    subtotal: Num,
    total: Num,
    memory: Num,
    mode: Mode,
    lamps: Lamps,
}

impl Default for Calculator {
    fn default() -> Self {
        Self::new()
    }
}

impl Calculator {
    pub const fn new() -> Self {
        Calculator {
            entry: Num::ZERO,
            entering: false,
            point: false,
            last: Num::ZERO,
            subtotal: Num::ZERO,
            total: Num::ZERO,
            memory: Num::ZERO,
            mode: Mode::Idle,
            lamps: Lamps {
                memory: false,
                overflow: false,
                negative: false,
            },
        }
    }

    pub fn lamps(&self) -> Lamps {
        self.lamps
    }

    /// Process one key with the selector positions read at that moment.
    pub fn press<P: Printer>(&mut self, key: Key, sw: Switches, out: &mut P) {
        if let Some(digit) = key.digit() {
            self.digit(digit);
            return;
        }
        match key {
            Key::D00 => {
                self.digit(0);
                self.digit(0);
            }
            Key::Point => {
                self.begin_entry();
                self.point = true;
            }
            Key::Sign => {
                self.begin_entry();
                self.entry.neg = !self.entry.neg;
            }
            Key::Ce => {
                self.entry = Num::ZERO;
                self.entering = true;
                self.point = false;
                self.lamps.overflow = false;
            }
            Key::C => {
                let memory = self.memory;
                let memory_lamp = self.lamps.memory;
                *self = Calculator::new();
                self.memory = memory;
                self.lamps.memory = memory_lamp;
                // ASSUMPTION (spec A3): C prints its symbol on an empty row.
                out.print(&Row::blank(Symbol::Clear));
            }
            Key::Plus => self.add_sub(false, sw, out),
            Key::Minus => self.add_sub(true, sw, out),
            Key::St => self.subtotal_key(sw, out),
            Key::Eq => self.equals(sw, out),
            // M2: multiplication, division, percent, exchange, memory, square root.
            _ => {}
        }
    }

    fn begin_entry(&mut self) {
        if !self.entering {
            self.entry = Num::ZERO;
            self.point = false;
            self.entering = true;
        }
    }

    fn digit(&mut self, digit: u8) {
        self.begin_entry();
        // Count typed digits, including leading zeros after the point.
        let typed = if self.entry.digits() > self.entry.scale {
            self.entry.digits()
        } else {
            self.entry.scale
        };
        if typed >= CAPACITY {
            return;
        }
        self.entry.shl(1);
        self.entry.d[0] = digit;
        if self.point {
            self.entry.scale += 1;
        }
    }

    /// The operand for a function key: the new entry, else the last operand.
    fn operand(&mut self) -> Num {
        if self.entering {
            self.entering = false;
            self.last = self.entry;
        }
        self.last
    }

    fn overflow<P: Printer>(&mut self, out: &mut P) {
        self.lamps.overflow = true;
        out.print(&Row::overflow());
    }

    fn add_sub<P: Printer>(&mut self, minus: bool, sw: Switches, out: &mut P) {
        let mut value = self.operand();
        // ASSUMPTION (spec A1): extra fractional digits are truncated.
        let (ok, _, _) = value.rescale(sw.dp);
        if !ok || value.width() > CAPACITY {
            self.overflow(out);
            return;
        }
        out.print(&Row::number(
            &value,
            if minus { Symbol::Minus } else { Symbol::Plus },
        ));
        match (
            self.subtotal.add_signed(&value, minus),
            self.total.add_signed(&value, minus),
        ) {
            (Some(s), Some(t)) => {
                self.subtotal = s;
                self.total = t;
                self.mode = Mode::AddSub;
            }
            _ => self.overflow(out),
        }
    }

    fn subtotal_key<P: Printer>(&mut self, sw: Switches, out: &mut P) {
        if self.entering {
            // Reference number / date: printed as typed, changes nothing.
            self.entering = false;
            out.print(&Row::number(&self.entry, Symbol::Reference));
            return;
        }
        if self.mode == Mode::AddSub {
            let row = self.result_row(self.subtotal, Symbol::SubTotal, sw);
            out.print(&row);
            self.subtotal = Num::ZERO;
        }
    }

    fn equals<P: Printer>(&mut self, sw: Switches, out: &mut P) {
        if self.mode == Mode::AddSub {
            let row = self.result_row(self.total, Symbol::Total, sw);
            out.print(&row);
            self.subtotal = Num::ZERO;
            self.total = Num::ZERO;
            self.mode = Mode::Idle;
        }
    }

    /// Format an accumulator at the selected DP, applying the rounding switch.
    fn result_row(&mut self, mut n: Num, symbol: Symbol, sw: Switches) -> Row {
        let mut rounded = false;
        let (ok, first, _) = n.rescale(sw.dp);
        // ASSUMPTION (spec A4): FL truncates like IN for accumulator totals.
        if ok && sw.rounding == Rounding::Round && first >= 5 {
            rounded = n.inc_mag();
        }
        let mut row = Row::number(&n, symbol);
        row.rounded = rounded;
        self.lamps.negative = n.neg;
        row
    }
}
