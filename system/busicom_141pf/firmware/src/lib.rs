//! Busicom 141-PF calculator firmware (1971), reimplemented in Rust.
//!
//! `no_std`, no allocation, no integer wider than `u8` for digit values in the
//! arithmetic. This is the machine-independent core: keys in, printed rows and
//! lamps out. The 4004 drivers (keyboard scan, printer drum, lamps) sit behind
//! `hal` (milestone M3).
//!
//! Behaviour is specified in `spec/spec.md`; the oracle is the recovered ROM's
//! output for the 42 manual examples (`oracle/manual_examples.json`). The state
//! machine follows the annotated disassembly's operation table (§4.4): working
//! register `WR`, dividend/multiplicand `DR`, constant `CR`, result `RR`,
//! subtotal `SR`, main total `TR`, memory `MR`, with the `MODE`/`MOP`/`MENT`
//! states that decide what `=`, `*`, `/`, `%` and the memory keys do.
#![no_std]

pub mod bcd;
pub mod driver_keyboard;
pub mod driver_lamps;
pub mod driver_printer;
pub mod hal;
pub mod keys;
pub mod print;
pub mod regops;

pub use bcd::Num;
pub use keys::{Key, Lamps, Rounding, Switches};
pub use print::{Printer, Row, SymA, SymB};

use bcd::CAPACITY;

/// `MODE` (disassembly `RR.S2`): what the last arithmetic step was, which
/// decides how `=` and the subtotal/total keys behave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// A fresh number is available (new entry, or a finished result).
    New,
    /// The last operation was multiply or divide.
    MulDiv,
    /// The last operation was addition or subtraction.
    AddSub,
}

/// `MOP` (disassembly `DR.S2`): the pending multiply/divide operation, and
/// whether a constant operand is being reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MulOp {
    /// No multiply/divide started (default behaviour is divide-by-`DR`).
    None,
    /// `DR` is multiplied by the next operand.
    Mul,
    /// `DR` is divided by the next operand.
    Div,
    /// Constant divide: the divisor is held in `CR`.
    ConstDiv,
    /// Constant multiply: the multiplicand is held in `CR`.
    ConstMul,
}

/// All firmware state: the working registers, memory and flags.
#[derive(Clone, Debug)]
pub struct Calculator {
    /// Working / entry register (`WR`): the number being typed or last result.
    wr: Num,
    /// Dividend / multiplicand register (`DR`): first operand of mul/div.
    dr: Num,
    /// Constant register (`CR`): constant multiplicand/divisor.
    cr: Num,
    /// Subtotal accumulator (`SR`).
    sr: Num,
    /// Main total accumulator (`TR`).
    tr: Num,
    /// Memory register (`MR`).
    mr: Num,
    /// Digits are being typed into `wr` (or CE set it to an explicit zero).
    entering: bool,
    /// `.` was typed in the current entry.
    point: bool,
    /// The pending mul/div operand (`DR`) came from a freshly typed entry. This
    /// controls the scale of a later repeated `=`/`M=±` operand row: entry-fed
    /// operands print at their entered scale, repeat-fed ones at the DP.
    op_from_entry: bool,
    mode: Mode,
    mop: MulOp,
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
            wr: Num::ZERO,
            dr: Num::ZERO,
            cr: Num::ZERO,
            sr: Num::ZERO,
            tr: Num::ZERO,
            mr: Num::ZERO,
            entering: false,
            point: false,
            op_from_entry: false,
            mode: Mode::New,
            mop: MulOp::None,
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
    ///
    /// After the key is handled, the front-panel **minus-sign lamp** is set from
    /// the working register's sign, exactly as the original firmware's
    /// status-light routine does: the annotated listing (§ status-light
    /// handling, `010 jms $173 … 01d wmp`) builds the RAM1 nibble as
    /// `8·WR.S0.bit1 + 4·WR.S0.bit0 + 2·overflow + (MR≠0)`, i.e. the minus lamp
    /// (bit 2) is `WR.S0.bit0` — the sign of the number currently in WR, read at
    /// idle — NOT the accumulator's running sign. A `+`/`-` operand leaves the
    /// (positive) entry in WR so the lamp stays off; a negative total/result
    /// leaves that value in WR so the lamp lights. See `driver_lamps` for bit 3
    /// (`WR.S0.bit1`).
    #[inline(never)]
    pub fn press<P: Printer>(&mut self, key: Key, sw: Switches, out: &mut P) {
        self.dispatch(key, sw, out);
        // The minus-sign lamp tracks the sign of WR at idle (listing: bit 2 =
        // WR.S0.bit0). Digit entry does not reach here with a function result,
        // and WR already holds whatever the last function left.
        self.lamps.negative = self.wr.neg;
    }

    #[inline(never)]
    fn dispatch<P: Printer>(&mut self, key: Key, sw: Switches, out: &mut P) {
        // At overflow, only CE and C respond (disassembly: engine starts only
        // for those two keys when SR.S2 overflow is set).
        if self.lamps.overflow && !matches!(key, Key::Ce | Key::C) {
            return;
        }
        if let Some(digit) = key.digit() {
            self.digit(digit);
            return;
        }
        // The non-digit key match is extracted to keep dispatch under a ROM
        // bank (the 4004 12-bit PC caps a function at one bank window).
        self.dispatch_key(key, sw, out);
    }

    #[inline(never)]
    fn dispatch_key<P: Printer>(&mut self, key: Key, sw: Switches, out: &mut P) {
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
                self.wr.neg = !self.wr.neg;
            }
            Key::Ce => {
                self.wr = Num::ZERO;
                self.entering = true;
                self.point = false;
                self.lamps.overflow = false;
                self.mode = Mode::New;
            }
            Key::C => self.clear(out),
            // Operation keys are handled in a separate #[inline(never)] helper
            // so neither half exceeds a 4004 ROM bank window.
            _ => self.dispatch_op_key(key, sw, out),
        }
    }

    #[inline(never)]
    fn dispatch_op_key<P: Printer>(&mut self, key: Key, sw: Switches, out: &mut P) {
        match key {
            Key::Plus => self.add_sub(false, sw, out),
            Key::Minus => self.add_sub(true, sw, out),
            Key::Mul => self.mul_div_key(MulOp::Mul, sw, out),
            Key::Div => self.mul_div_key(MulOp::Div, sw, out),
            Key::Eq => self.equals(sw, out),
            Key::Percent => self.percent(sw, out),
            Key::Ex => self.exchange(out),
            Key::St => self.subtotal_key(sw, out),
            Key::MPlus => self.memory_add(false, sw, out),
            Key::MMinus => self.memory_add(true, sw, out),
            Key::MEqPlus => self.memory_eq(false, sw, out),
            Key::MEqMinus => self.memory_eq(true, sw, out),
            Key::Rm => self.recall_memory(sw, out),
            Key::Cm => self.clear_memory(sw, out),
            Key::Sqrt => self.sqrt(sw, out),
            // entry/digit keys never reach here (handled by dispatch_key).
            _ => {}
        }
    }

    // --- number entry ---------------------------------------------------

    fn begin_entry(&mut self) {
        // A function key leaves MODE != New; the next digit starts a new entry.
        if !self.entering {
            self.wr = Num::ZERO;
            self.point = false;
            self.entering = true;
            self.mode = Mode::New;
        }
    }

    #[inline(never)]
    fn digit(&mut self, digit: u8) {
        self.begin_entry();
        let typed = if self.wr.digits() > self.wr.scale {
            self.wr.digits()
        } else {
            self.wr.scale
        };
        if typed >= CAPACITY {
            return;
        }
        self.wr.shl(1);
        self.wr.set_digit(0, digit);
        if self.point {
            self.wr.scale += 1;
        }
    }

    /// Latch the entry as the operand for a function key.
    fn take_entry(&mut self) {
        self.entering = false;
        self.point = false;
    }

    // --- clear ----------------------------------------------------------

    #[inline(never)]
    fn clear<P: Printer>(&mut self, out: &mut P) {
        // C clears WR, DR, SR, TR and the overflow flag; memory is preserved.
        self.wr = Num::ZERO;
        self.dr = Num::ZERO;
        self.sr = Num::ZERO;
        self.tr = Num::ZERO;
        self.cr = Num::ZERO;
        self.entering = false;
        self.point = false;
        self.mode = Mode::New;
        self.mop = MulOp::None;
        self.op_from_entry = false;
        self.lamps.overflow = false;
        self.lamps.negative = false;
        // The original ROM prints the cleared value `0` with the C symbol, not
        // a blank row (verified against oracle/rom_port_trace.txt; was spec A3's
        // guessed blank row — see failure_notes/2026-10-18-clear-prints-zero.md).
        out.print(&Row::num_a(&Num::ZERO, SymA::Clear));
    }

    #[inline(never)]
    fn overflow<P: Printer>(&mut self, out: &mut P) {
        self.lamps.overflow = true;
        out.print(&Row::overflow());
    }

    // --- addition / subtraction ----------------------------------------

    #[inline(never)]
    fn add_sub<P: Printer>(&mut self, minus: bool, sw: Switches, out: &mut P) {
        // Operand: the new entry, else repeat the last working value.
        if self.entering {
            self.take_entry();
        }
        let mut value = self.wr;
        // ASSUMPTION (spec A1): extra fractional digits are truncated on align.
        let (ok, _, _) = value.rescale(sw.dp);
        if !ok || value.width() > CAPACITY {
            self.overflow(out);
            return;
        }
        self.wr = value;
        out.print(&Row::num_a(
            &value,
            if minus { SymA::Minus } else { SymA::Plus },
        ));
        match (
            self.sr.add_signed(&value, minus),
            self.tr.add_signed(&value, minus),
        ) {
            (Some(s), Some(t)) => {
                self.sr = s;
                self.tr = t;
                self.mode = Mode::AddSub;
            }
            _ => self.overflow(out),
        }
    }

    // --- multiply / divide keys ----------------------------------------

    /// `*` or `/`: establish or chain a multiply/divide operation.
    ///
    /// Operand-echo rows (spec A6, from `oracle/rom_port_trace.txt`): the ROM
    /// prints the first operand with the op glyph not only for a freshly typed
    /// entry but also when the operand was *recalled or accumulated* (e.g.
    /// after `RM` or `M+`, 9-1 `68.94 ×`, 3-6 `123.45 ÷`), and when the running
    /// main total is pulled into a mixed calculation (`119.47 ×`). Pressing a
    /// second op key with no new entry is an **operation correction**: the ROM
    /// reprints only the op glyph with a BLANK value (2-8 `× ÷ ×`), the operand
    /// already latched.
    #[inline(never)]
    fn mul_div_chain<P: Printer>(&mut self, op: MulOp, entered: bool, sw: Switches, out: &mut P) {
        // Chain: a new op after a pending multiply/divide. Echo the operand in
        // WR with the new op glyph, then compute the pending product/quotient,
        // whose result feeds the next op. Extracted from mul_div_key so each
        // stays under a 4004 ROM bank window.
        let shown = operand_shown(&self.wr, entered, sw.dp);
        out.print(&Row::num_a(&shown, sym_for(op)));
        if let Some(r) = self.compute() {
            self.wr = r;
            self.dr = r;
            self.cr = r;
            self.mop = op;
            self.mode = Mode::MulDiv;
            self.op_from_entry = false; // operand is a computed result
        } else {
            self.overflow(out);
        }
    }

    #[inline(never)]
    fn mul_div_key<P: Printer>(&mut self, op: MulOp, sw: Switches, out: &mut P) {
        let entered = self.entering;
        if entered {
            self.take_entry();
        }
        match self.mode {
            Mode::AddSub => {
                // Mixed calc: recall the main total as the first operand,
                // clear the accumulators, and print it with the op symbol. A
                // recalled total is a non-fresh operand → aligned to DP.
                self.wr = self.tr;
                self.sr = Num::ZERO;
                self.tr = Num::ZERO;
                let shown = operand_shown(&self.wr, false, sw.dp);
                out.print(&Row::num_a(&shown, sym_for(op)));
                self.dr = self.wr;
                self.cr = self.wr;
                self.mop = op;
                self.mode = Mode::MulDiv;
                self.op_from_entry = false; // operand recalled, not typed
            }
            Mode::New if matches!(self.mop, MulOp::Mul | MulOp::Div) => {
                // Chain (extracted to keep mul_div_key under a ROM bank).
                self.mul_div_chain(op, entered, sw, out);
            }
            Mode::MulDiv if !entered => {
                // Operation correction: an op key pressed straight after another
                // with no new entry. The operand stays latched; the ROM reprints
                // only the op glyph with a blank value (spec A6).
                out.print(&Row::blank(sym_for(op), SymB::None));
                self.mop = op;
            }
            _ => {
                // First operand: a freshly typed entry, OR a value left in WR by
                // a recall/accumulate/result (spec A6 echo). A fresh entry
                // echoes at its entered scale; a recalled/accumulated operand is
                // aligned to DP (10-1 `10.00 ×` after `10 M+`; 9-1 `11 ×` keeps
                // its typed `11`).
                let shown = operand_shown(&self.wr, entered, sw.dp);
                self.dr = self.wr;
                self.cr = self.wr;
                self.mop = op;
                self.mode = Mode::MulDiv;
                self.op_from_entry = entered;
                out.print(&Row::num_a(&shown, sym_for(op)));
            }
        }
    }

    /// Evaluate `DR (op) WR` (or the constant variant) into a floating result.
    #[inline(never)]
    fn compute(&mut self) -> Option<Num> {
        match self.mop {
            MulOp::Mul => self.dr.mul(&self.wr),
            MulOp::Div => self.dr.div_float(&self.wr),
            MulOp::ConstMul => {
                // DR = WR, WR = CR, result = DR * WR
                let a = self.wr;
                self.dr = a;
                self.wr = self.cr;
                a.mul(&self.cr)
            }
            MulOp::ConstDiv => {
                let a = self.wr;
                self.dr = a;
                self.wr = self.cr;
                a.div_float(&self.cr)
            }
            MulOp::None => None,
        }
    }

    /// Division for a final result row: floating (13 significant digits) in FL,
    /// else computed to DP + guard fractional digits so the rounding switch can
    /// round the last place. Multiplication is always exact, so only division
    /// needs the mode-aware precision.
    fn div_result(a: &Num, b: &Num, sw: Switches) -> Option<Num> {
        if sw.rounding == Rounding::Float {
            a.div_float(b)
        } else {
            // Guard digit below DP so round_at(dp) rounds correctly, bounded by
            // the register width.
            let frac = (sw.dp + 1).min(bcd::CAPACITY);
            a.div_fixed(b, frac)
        }
    }

    // --- equals ---------------------------------------------------------

    #[inline(never)]
    fn equals<P: Printer>(&mut self, sw: Switches, out: &mut P) {
        if self.mode == Mode::AddSub {
            // Recall and clear the main total.
            let total = self.tr;
            let (shown, rounded) = total.round_at(sw.dp, sw.rounding == Rounding::Round);
            let row = total_row(&shown, rounded);
            out.print(&row);
            self.wr = total;
            self.sr = Num::ZERO;
            self.tr = Num::ZERO;
            self.mode = Mode::New;
            return;
        }
        // Multiply/divide result (or raise-to-power with repeated `=`).
        let fresh = self.entering;
        if self.entering {
            self.take_entry();
        }
        self.print_operand(fresh, sw, out);
        self.do_mul_div_equals(sw, out, SymB::Total, None);
    }

    /// Print the `=`/`M=±` second-operand row. A freshly typed entry prints at
    /// its entered scale. A repeated operand (no new entry) prints at the
    /// entered scale when the pending operation itself began from a typed entry
    /// (e.g. `8 * M=+` → `8`), otherwise rescaled to the current DP (e.g. a
    /// repeat chain `2 M+ * =` → `2.000000`).
    #[inline(never)]
    fn print_operand<P: Printer>(&self, fresh: bool, sw: Switches, out: &mut P) {
        let shown = if fresh || self.op_from_entry {
            self.wr
        } else {
            let mut n = self.wr;
            let _ = n.rescale(sw.dp);
            n
        };
        out.print(&Row::num_a(&shown, SymA::Equals));
    }

    /// Execute the pending mul/div for `=`/`%`/`M=±`, print the result row with
    /// `symbol`, and update constant-operation state. `scale_pct` scales the
    /// result for percent (divide → ×100, multiply → ÷100 already folded).
    #[inline(never)]
    fn compute_mul_div(&mut self, sw: Switches, pct: Option<MulOp>) -> Option<Num> {
        // Determine the concrete operation and whether a constant is reused.
        match self.mop {
            MulOp::Mul => {
                let r = self.dr.mul(&self.wr);
                // After `=`, the multiplicand constant stays in CR (const mul).
                self.mop = MulOp::ConstMul;
                r
            }
            MulOp::Div => {
                let r = if pct.is_some() {
                    self.dr.div_float(&self.wr)
                } else {
                    Self::div_result(&self.dr, &self.wr, sw)
                };
                // Divisor becomes the constant (const div).
                self.cr = self.wr;
                self.mop = MulOp::ConstDiv;
                r
            }
            MulOp::ConstMul => {
                // DR = WR (new multiplier), WR = CR, result = DR * CR
                let a = self.wr;
                let r = a.mul(&self.cr);
                self.dr = a;
                r
            }
            MulOp::ConstDiv => {
                let a = self.wr;
                let r = if pct.is_some() {
                    a.div_float(&self.cr)
                } else {
                    Self::div_result(&a, &self.cr, sw)
                };
                self.dr = a;
                r
            }
            MulOp::None => {
                // Default operation is division (DR / WR); 0/0 overflows.
                if pct.is_some() {
                    self.dr.div_float(&self.wr)
                } else {
                    Self::div_result(&self.dr, &self.wr, sw)
                }
            }
        }
    }

    #[inline(never)]
    fn do_mul_div_equals<P: Printer>(
        &mut self,
        sw: Switches,
        out: &mut P,
        sym_b: SymB,
        pct: Option<MulOp>,
    ) -> Option<Num> {
        // Operation selection (which product/quotient, and the const-operand
        // bookkeeping) is extracted into its own #[inline(never)] helper so
        // do_mul_div_equals stays under a 4004 ROM bank.
        let result = self.compute_mul_div(sw, pct);
        let mut value = match result {
            Some(v) => v,
            None => {
                self.overflow(out);
                return None;
            }
        };
        // Percent scaling.
        if let Some(op) = pct {
            value = match op {
                MulOp::Div | MulOp::ConstDiv => scale_pow(&value, 2), // ×100
                _ => value,                                           // mul already ÷100 by operand
            };
            // Re-truncate a scaled division to 13 significant digits.
            if matches!(op, MulOp::Div | MulOp::ConstDiv) && sw.rounding == Rounding::Float {
                value = trunc_sig(&value, bcd::FLOAT_SIG);
            }
        }
        self.mode = Mode::New;
        // Format with DP + rounding; the SHOWN (rounded) value becomes WR so a
        // following +/- or chain aligns to the printed number, not the raw
        // high-precision intermediate.
        let (shown, rounded) = if sw.rounding == Rounding::Float {
            (value, false)
        } else {
            value.round_at(sw.dp, sw.rounding == Rounding::Round)
        };
        if shown.int_digits() > CAPACITY || shown.width() > CAPACITY {
            self.overflow(out);
            return None;
        }
        self.wr = shown;
        let row = result_row(&shown, rounded, sym_b);
        out.print(&row);
        Some(shown)
    }

    // --- percent --------------------------------------------------------

    #[inline(never)]
    fn percent<P: Printer>(&mut self, sw: Switches, out: &mut P) {
        if self.entering {
            self.take_entry();
        }
        // The entered operand prints with `%`.
        out.print(&Row::num_a(&self.wr, SymA::Percent));
        // For percent-multiply, the operand means "× (n/100)": fold ÷100 into
        // the operand before multiplying. For percent-divide, scale ×100 after.
        let op = self.mop;
        if matches!(op, MulOp::Mul | MulOp::ConstMul) {
            // divide the entered operand by 100 first
            self.wr = scale_pow(&self.wr, -2);
        }
        self.do_mul_div_equals(sw, out, SymB::Total, Some(op));
    }

    // --- exchange -------------------------------------------------------

    #[inline(never)]
    fn exchange<P: Printer>(&mut self, out: &mut P) {
        if self.entering {
            self.take_entry();
        }
        // CR=WR, RR=DR, DR=WR, WR=RR  → swap WR and DR, CR tracks the new DR.
        out.print(&Row::num_b(&self.wr, SymB::Exchange));
        let tmp = self.dr;
        self.cr = self.wr;
        self.dr = self.wr;
        self.wr = tmp;
        self.mode = Mode::New;
    }

    // --- subtotal / reference ------------------------------------------

    #[inline(never)]
    fn subtotal_key<P: Printer>(&mut self, sw: Switches, out: &mut P) {
        if self.entering {
            // Reference number / date: printed as typed, changes nothing.
            self.take_entry();
            out.print(&Row::num_b(&self.wr, SymB::Reference));
            return;
        }
        match self.mode {
            Mode::AddSub => {
                let (shown, rounded) = self.sr.round_at(sw.dp, sw.rounding == Rounding::Round);
                out.print(&subtotal_row(&shown, rounded));
                self.sr = Num::ZERO;
            }
            Mode::New if matches!(self.mop, MulOp::Mul | MulOp::Div) => {
                // Intermediate chain result: print the pending product/quotient.
                if let Some(r) = self.compute() {
                    out.print(&Row::num_a(&r, SymA::SubTotal));
                    self.wr = r;
                    self.dr = r;
                    self.cr = r;
                }
            }
            Mode::MulDiv => {
                // After a chain `*`/`/` the running product is in DR; print it.
                out.print(&Row::num_a(&self.dr, SymA::SubTotal));
            }
            _ => {}
        }
    }

    // --- memory ---------------------------------------------------------

    #[inline(never)]
    fn memory_add<P: Printer>(&mut self, minus: bool, sw: Switches, out: &mut P) {
        if self.entering {
            self.take_entry();
        }
        // The ROM echoes a bare `M+`/`M-` key: it prints the current operand
        // with the memory-key glyph in symbol column A (spec A7, from
        // oracle/rom_port_trace.txt — e.g. 9-1 `6.89 M-`, 10-1 `10.00 M+`,
        // 11-2 `2.000000 M+`). The operand is aligned to the decimal selector
        // before printing, exactly like a `+`/`-` operand (10-1 prints the
        // typed `10` as `10.00` at DP 2; 11-2 prints `2` as `2.000000` at DP 6).
        let mut shown = self.wr;
        let _ = shown.rescale(sw.dp);
        out.print(&Row::num_a(
            &shown,
            if minus { SymA::MemMinus } else { SymA::MemPlus },
        ));
        if let Some(m) = self.mr.add_signed(&self.wr, minus) {
            self.mr = m;
            self.lamps.memory = !self.mr.is_zero();
            self.mode = Mode::New;
        } else {
            self.overflow(out);
        }
    }

    /// `M=+` / `M=-`: compute the pending mul/div (printing the second operand
    /// with `=`, then the product/quotient with `M+`/`M-`), then accumulate the
    /// product/quotient into memory (the printed row is the product, not the
    /// running memory sum — the oracle prints the product; `CM` shows the sum).
    #[inline(never)]
    fn memory_eq<P: Printer>(&mut self, minus: bool, sw: Switches, out: &mut P) {
        let fresh = self.entering;
        if self.entering {
            self.take_entry();
        }
        self.print_operand(fresh, sw, out);
        let value = match self.compute_equals(out) {
            Some(v) => v,
            None => return,
        };
        // Format the product/quotient for the printed row (DP + rounding).
        let (shown, rounded) = if sw.rounding == Rounding::Float {
            (value, false)
        } else {
            value.round_at(sw.dp, sw.rounding == Rounding::Round)
        };
        if shown.width() > CAPACITY {
            self.overflow(out);
            return;
        }
        // Accumulate the rounded value into memory (matches the printed row).
        match self.mr.add_signed(&shown, minus) {
            Some(m) => self.mr = m,
            None => {
                self.overflow(out);
                return;
            }
        }
        self.lamps.memory = !self.mr.is_zero();
        // The printed result goes in symbol column B (`M+`/`M-`); a rounded
        // result adds the `^` mark in column A (spec A8).
        let sym_b = if minus { SymB::MemMinus } else { SymB::MemPlus };
        out.print(&result_row(&shown, rounded, sym_b));
    }

    /// Execute the pending mul/div for `M=±`, returning the raw floating result
    /// without printing. Updates constant-operation state like `=`.
    #[inline(never)]
    fn compute_equals<P: Printer>(&mut self, out: &mut P) -> Option<Num> {
        let result = match self.mop {
            MulOp::Mul => {
                let r = self.dr.mul(&self.wr);
                self.mop = MulOp::ConstMul;
                r
            }
            MulOp::Div => {
                let r = self.dr.div_float(&self.wr);
                self.cr = self.wr;
                self.mop = MulOp::ConstDiv;
                r
            }
            MulOp::ConstMul => {
                let a = self.wr;
                let r = a.mul(&self.cr);
                self.dr = a;
                r
            }
            MulOp::ConstDiv => {
                let a = self.wr;
                let r = a.div_float(&self.cr);
                self.dr = a;
                r
            }
            MulOp::None => self.dr.div_float(&self.wr),
        };
        match result {
            Some(v) => {
                self.wr = v;
                self.mode = Mode::New;
                Some(v)
            }
            None => {
                self.overflow(out);
                None
            }
        }
    }

    #[inline(never)]
    fn recall_memory<P: Printer>(&mut self, sw: Switches, out: &mut P) {
        self.wr = self.mr;
        self.entering = false;
        self.mode = Mode::New;
        let (shown, _rounded) = self.mr.round_at(sw.dp, sw.rounding == Rounding::Round);
        // RM prints `R` in column A and `M` in column B (spec A8); never the
        // rounded-up mark (it is a recall, not a result). A zero register prints
        // as the bare `0` the ROM shows (scale 0), not DP-padded.
        out.print(&Row::number(&zero_as_bare(shown), SymA::Recall, SymB::Mem));
    }

    #[inline(never)]
    fn clear_memory<P: Printer>(&mut self, sw: Switches, out: &mut P) {
        let value = self.mr;
        let (shown, _rounded) = value.round_at(sw.dp, sw.rounding == Rounding::Round);
        // CM prints `C` in column A and `M` in column B (spec A8); a zero
        // register prints as the bare `0` (scale 0), matching the ROM.
        out.print(&Row::number(&zero_as_bare(shown), SymA::Clear, SymB::Mem));
        self.wr = value;
        self.mr = Num::ZERO;
        self.lamps.memory = false;
        self.entering = false;
        self.mode = Mode::New;
    }

    // --- square root ----------------------------------------------------

    #[inline(never)]
    fn sqrt<P: Printer>(&mut self, sw: Switches, out: &mut P) {
        let fresh = self.entering;
        if self.entering {
            self.take_entry();
        }
        // The radicand prints with the SQRT symbol: a freshly typed entry at
        // its entered scale, otherwise rescaled to the current DP.
        let radicand = if fresh {
            self.wr
        } else {
            let mut n = self.wr;
            let _ = n.rescale(sw.dp);
            n
        };
        out.print(&Row::num_a(&radicand, SymA::Sqrt));

        // Root precision: FL fills to ~14 significant digits; fixed modes show
        // DP fractional digits (rounded per the switch).
        let int_q = self.wr.int_digits().div_ceil(2); // integer digits of the root
        let root = if sw.rounding == Rounding::Float {
            let frac = CAPACITY.saturating_sub(int_q).min(CAPACITY);
            self.wr.sqrt_at(frac)
        } else {
            // Compute one guard digit, then round/truncate to DP.
            let guard = (sw.dp + 1).min(CAPACITY.saturating_sub(int_q));
            let (r, _mark) = self
                .wr
                .sqrt_at(guard)
                .round_at(sw.dp, sw.rounding == Rounding::Round);
            r
        };
        self.wr = root;
        self.cr = root;
        self.mop = MulOp::None;
        self.mode = Mode::New;
        // Square-root result rows never carry the rounded-up mark.
        out.print(&Row::num_a(&root, SymA::Sqrt));
    }
}

/// The value an operand-echo row prints (spec A6/M2-O1): a freshly typed
/// operand keeps its entered scale; a recalled/accumulated operand (no new
/// entry since) is aligned to the decimal selector, exactly as the ROM's
/// operand print does (9-1 `11 ×` typed vs 10-1 `10.00 ×` recalled after M+).
fn operand_shown(n: &Num, fresh: bool, dp: u8) -> Num {
    if fresh {
        *n
    } else {
        let mut v = *n;
        let _ = v.rescale(dp);
        v
    }
}

/// A number for display where a zero magnitude collapses to the bare `0` the
/// ROM prints (scale 0), instead of being padded to the decimal selector. The
/// original register holds a zero at scale 0, so `C`/`CM`/`RM` of an empty
/// register print `0`, while a `+`/`-` operand of zero is DP-aligned by the
/// add path and keeps its padding (e.g. 14-3 `0.00000000 +`).
fn zero_as_bare(n: Num) -> Num {
    if n.is_zero() {
        Num::ZERO
    } else {
        n
    }
}

/// Symbol printed in column A when a `*`/`/` operand row is emitted.
fn sym_for(op: MulOp) -> SymA {
    match op {
        MulOp::Mul | MulOp::ConstMul => SymA::Multiply,
        _ => SymA::Divide,
    }
}

/// A main-total row (`=` / `ST`-total): `*` in column B, with the rounded-up
/// `^` mark in column A when the value was rounded (spec A8).
fn total_row(shown: &Num, rounded: bool) -> Row {
    result_row(shown, rounded, SymB::Total)
}

/// A subtotal row: `◇` in column A. A subtotal is never rounded in the oracle
/// (the diamond and the `^` mark are the same physical column, so they cannot
/// coexist); the mark is therefore not printed on a subtotal.
fn subtotal_row(shown: &Num, _rounded: bool) -> Row {
    Row::num_a(shown, SymA::SubTotal)
}

/// A result row carrying `sym_b` (total `*`, or `M+`/`M-` for `M=±`), plus the
/// rounded-up `^` mark in column A when `rounded` (spec A8: e.g. `56.09 ^ *`,
/// `12.72 ^ M+`).
#[inline(never)]
fn result_row(shown: &Num, rounded: bool, sym_b: SymB) -> Row {
    let sym_a = if rounded { SymA::Rounded } else { SymA::None };
    Row::number(shown, sym_a, sym_b)
}

/// Multiply a number by `10^pow` (pow may be negative), truncating fractional
/// digits that fall below scale 0. Used for percent scaling.
fn scale_pow(n: &Num, pow: i16) -> Num {
    let mut r = *n;
    if pow >= 0 {
        // increase magnitude: shift digits left, scale unchanged if room,
        // otherwise reduce scale.
        let mut k = pow as u8;
        while k > 0 {
            if r.scale > 0 {
                r.scale -= 1;
            } else {
                r.shl(1);
            }
            k -= 1;
        }
    } else {
        let k = (-pow) as u8;
        r.scale += k;
    }
    if r.is_zero() {
        r.neg = false;
        r.scale = 0;
    }
    r
}

/// Truncate a number to `sig` significant digits (drop low fractional digits).
fn trunc_sig(n: &Num, sig: u8) -> Num {
    let mut r = *n;
    let total = r.digits();
    if total > sig {
        let drop = (total - sig).min(r.scale);
        if drop > 0 {
            let _ = r.shr(drop);
            r.scale -= drop;
        }
    }
    r
}
// (square-root fractional width is computed inline in Calculator::sqrt)
