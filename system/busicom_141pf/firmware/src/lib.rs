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

pub use bcd::Num;
pub use keys::{Key, Lamps, Rounding, Switches};
pub use print::{Printer, Row, Symbol};

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
    pub fn press<P: Printer>(&mut self, key: Key, sw: Switches, out: &mut P) {
        // At overflow, only CE and C respond (disassembly: engine starts only
        // for those two keys when SR.S2 overflow is set).
        if self.lamps.overflow && !matches!(key, Key::Ce | Key::C) {
            return;
        }
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
            Key::Plus => self.add_sub(false, sw, out),
            Key::Minus => self.add_sub(true, sw, out),
            Key::Mul => self.mul_div_key(MulOp::Mul, out),
            Key::Div => self.mul_div_key(MulOp::Div, out),
            Key::Eq => self.equals(sw, out),
            Key::Percent => self.percent(sw, out),
            Key::Ex => self.exchange(out),
            Key::St => self.subtotal_key(sw, out),
            Key::MPlus => self.memory_add(false, out),
            Key::MMinus => self.memory_add(true, out),
            Key::MEqPlus => self.memory_eq(false, sw, out),
            Key::MEqMinus => self.memory_eq(true, sw, out),
            Key::Rm => self.recall_memory(sw, out),
            Key::Cm => self.clear_memory(sw, out),
            Key::Sqrt => self.sqrt(sw, out),
            // Digit keys are handled above via `key.digit()`.
            Key::D0
            | Key::D1
            | Key::D2
            | Key::D3
            | Key::D4
            | Key::D5
            | Key::D6
            | Key::D7
            | Key::D8
            | Key::D9 => {}
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
        self.wr.d[0] = digit;
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
        // ASSUMPTION (spec A3): C prints its symbol on an empty row.
        out.print(&Row::blank(Symbol::Clear));
    }

    fn overflow<P: Printer>(&mut self, out: &mut P) {
        self.lamps.overflow = true;
        out.print(&Row::overflow());
    }

    // --- addition / subtraction ----------------------------------------

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
        out.print(&Row::number(
            &value,
            if minus { Symbol::Minus } else { Symbol::Plus },
        ));
        match (
            self.sr.add_signed(&value, minus),
            self.tr.add_signed(&value, minus),
        ) {
            (Some(s), Some(t)) => {
                self.sr = s;
                self.tr = t;
                self.mode = Mode::AddSub;
                self.lamps.negative = t.neg;
            }
            _ => self.overflow(out),
        }
    }

    // --- multiply / divide keys ----------------------------------------

    /// `*` or `/`: establish or chain a multiply/divide operation.
    fn mul_div_key<P: Printer>(&mut self, op: MulOp, out: &mut P) {
        let entered = self.entering;
        if entered {
            self.take_entry();
        }
        match self.mode {
            Mode::AddSub => {
                // Mixed calc: recall the main total as the first operand,
                // clear the accumulators, and print it with the op symbol.
                self.wr = self.tr;
                self.sr = Num::ZERO;
                self.tr = Num::ZERO;
                out.print(&Row::number(&self.wr, sym_for(op)));
                self.dr = self.wr;
                self.cr = self.wr;
                self.mop = op;
                self.mode = Mode::MulDiv;
                self.op_from_entry = false; // operand recalled, not typed
            }
            Mode::New if matches!(self.mop, MulOp::Mul | MulOp::Div) => {
                // Chain: compute the pending operation, result feeds the next.
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
            _ => {
                // First operand (also: new number after a constant op, or an
                // operation correction when the last key was mul/div).
                self.dr = self.wr;
                self.cr = self.wr;
                self.mop = op;
                self.mode = Mode::MulDiv;
                self.op_from_entry = entered;
                if entered {
                    out.print(&Row::number(&self.wr, sym_for(op)));
                }
            }
        }
    }

    /// Evaluate `DR (op) WR` (or the constant variant) into a floating result.
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

    fn equals<P: Printer>(&mut self, sw: Switches, out: &mut P) {
        if self.mode == Mode::AddSub {
            // Recall and clear the main total.
            let total = self.tr;
            let (shown, rounded) = total.round_at(sw.dp, sw.rounding == Rounding::Round);
            let mut row = Row::number(&shown, Symbol::Total);
            row.rounded = rounded;
            self.lamps.negative = shown.neg;
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
        self.do_mul_div_equals(sw, out, Symbol::Total, None);
    }

    /// Print the `=`/`M=±` second-operand row. A freshly typed entry prints at
    /// its entered scale. A repeated operand (no new entry) prints at the
    /// entered scale when the pending operation itself began from a typed entry
    /// (e.g. `8 * M=+` → `8`), otherwise rescaled to the current DP (e.g. a
    /// repeat chain `2 M+ * =` → `2.000000`).
    fn print_operand<P: Printer>(&self, fresh: bool, sw: Switches, out: &mut P) {
        let shown = if fresh || self.op_from_entry {
            self.wr
        } else {
            let mut n = self.wr;
            let _ = n.rescale(sw.dp);
            n
        };
        out.print(&Row::number(&shown, Symbol::Equals));
    }

    /// Execute the pending mul/div for `=`/`%`/`M=±`, print the result row with
    /// `symbol`, and update constant-operation state. `scale_pct` scales the
    /// result for percent (divide → ×100, multiply → ÷100 already folded).
    fn do_mul_div_equals<P: Printer>(
        &mut self,
        sw: Switches,
        out: &mut P,
        symbol: Symbol,
        pct: Option<MulOp>,
    ) -> Option<Num> {
        // Determine the concrete operation and whether a constant is reused.
        let result = match self.mop {
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
        };
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
        let mut row = Row::number(&shown, symbol);
        row.rounded = rounded;
        self.lamps.negative = shown.neg;
        out.print(&row);
        Some(shown)
    }

    // --- percent --------------------------------------------------------

    fn percent<P: Printer>(&mut self, sw: Switches, out: &mut P) {
        if self.entering {
            self.take_entry();
        }
        // The entered operand prints with `%`.
        out.print(&Row::number(&self.wr, Symbol::Percent));
        // For percent-multiply, the operand means "× (n/100)": fold ÷100 into
        // the operand before multiplying. For percent-divide, scale ×100 after.
        let op = self.mop;
        if matches!(op, MulOp::Mul | MulOp::ConstMul) {
            // divide the entered operand by 100 first
            self.wr = scale_pow(&self.wr, -2);
        }
        self.do_mul_div_equals(sw, out, Symbol::Total, Some(op));
    }

    // --- exchange -------------------------------------------------------

    fn exchange<P: Printer>(&mut self, out: &mut P) {
        if self.entering {
            self.take_entry();
        }
        // CR=WR, RR=DR, DR=WR, WR=RR  → swap WR and DR, CR tracks the new DR.
        out.print(&Row::number(&self.wr, Symbol::Exchange));
        let tmp = self.dr;
        self.cr = self.wr;
        self.dr = self.wr;
        self.wr = tmp;
        self.mode = Mode::New;
    }

    // --- subtotal / reference ------------------------------------------

    fn subtotal_key<P: Printer>(&mut self, sw: Switches, out: &mut P) {
        if self.entering {
            // Reference number / date: printed as typed, changes nothing.
            self.take_entry();
            out.print(&Row::number(&self.wr, Symbol::Reference));
            return;
        }
        match self.mode {
            Mode::AddSub => {
                let (shown, rounded) = self.sr.round_at(sw.dp, sw.rounding == Rounding::Round);
                let mut row = Row::number(&shown, Symbol::SubTotal);
                row.rounded = rounded;
                self.lamps.negative = shown.neg;
                out.print(&row);
                self.sr = Num::ZERO;
            }
            Mode::New if matches!(self.mop, MulOp::Mul | MulOp::Div) => {
                // Intermediate chain result: print the pending product/quotient.
                if let Some(r) = self.compute() {
                    out.print(&Row::number(&r, Symbol::SubTotal));
                    self.wr = r;
                    self.dr = r;
                    self.cr = r;
                }
            }
            Mode::MulDiv => {
                // After a chain `*`/`/` the running product is in DR; print it.
                out.print(&Row::number(&self.dr, Symbol::SubTotal));
            }
            _ => {}
        }
    }

    // --- memory ---------------------------------------------------------

    fn memory_add<P: Printer>(&mut self, minus: bool, out: &mut P) {
        if self.entering {
            self.take_entry();
        }
        if let Some(m) = self.mr.add_signed(&self.wr, minus) {
            self.mr = m;
            self.lamps.memory = !self.mr.is_zero();
            self.mode = Mode::New;
            let _ = out; // M+/M- print nothing on their own
        } else {
            self.overflow(out);
        }
    }

    /// `M=+` / `M=-`: compute the pending mul/div (printing the second operand
    /// with `=`, then the product/quotient with `M+`/`M-`), then accumulate the
    /// product/quotient into memory (the printed row is the product, not the
    /// running memory sum — the oracle prints the product; `CM` shows the sum).
    fn memory_eq<P: Printer>(&mut self, minus: bool, sw: Switches, out: &mut P) {
        let fresh = self.entering;
        if self.entering {
            self.take_entry();
        }
        self.print_operand(fresh, sw, out);
        let sym = if minus {
            Symbol::MemMinus
        } else {
            Symbol::MemPlus
        };
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
        let mut row = Row::number(&shown, sym);
        row.rounded = rounded;
        self.lamps.negative = shown.neg;
        out.print(&row);
    }

    /// Execute the pending mul/div for `M=±`, returning the raw floating result
    /// without printing. Updates constant-operation state like `=`.
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

    fn recall_memory<P: Printer>(&mut self, sw: Switches, out: &mut P) {
        self.wr = self.mr;
        self.entering = false;
        self.mode = Mode::New;
        let (shown, _rounded) = self.mr.round_at(sw.dp, sw.rounding == Rounding::Round);
        // RM/CM never carry the rounded-up mark (it is a recall, not a result).
        let row = Row::number(&shown, Symbol::RecallMemory);
        self.lamps.negative = shown.neg;
        out.print(&row);
    }

    fn clear_memory<P: Printer>(&mut self, sw: Switches, out: &mut P) {
        let value = self.mr;
        let (shown, _rounded) = value.round_at(sw.dp, sw.rounding == Rounding::Round);
        let row = Row::number(&shown, Symbol::ClearMemory);
        out.print(&row);
        self.wr = value;
        self.mr = Num::ZERO;
        self.lamps.memory = false;
        self.lamps.negative = shown.neg;
        self.entering = false;
        self.mode = Mode::New;
    }

    // --- square root ----------------------------------------------------

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
        out.print(&Row::number(&radicand, Symbol::Sqrt));

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
        self.lamps.negative = false;
        // Square-root result rows never carry the rounded-up mark.
        out.print(&Row::number(&root, Symbol::Sqrt));
    }
}

/// Symbol printed when a `*`/`/` operand row is emitted.
fn sym_for(op: MulOp) -> Symbol {
    match op {
        MulOp::Mul | MulOp::ConstMul => Symbol::Multiply,
        _ => Symbol::Divide,
    }
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
